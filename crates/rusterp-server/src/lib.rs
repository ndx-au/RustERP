//! RustERP gRPC server library: service wiring over domain repositories.

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::Arc;

use rusterp_auth::{AuthRepository, ModuleStore};
use rusterp_catalog::CatalogRepository;
use rusterp_inventory::InventoryRepository;
use rusterp_parties::{
    Address as DomainAddress, AddressKind as DomainAddressKind, AddressUpdate,
    Contact as DomainContact, ContactUpdate, InMemoryPartyRepository, NewAddress, NewContact,
    NewParty, Party as DomainParty, PartyError, PartyRepository, PartyRole as DomainRole,
    PartyUpdate,
};
use rusterp_payments::PaymentsRepository;
use rusterp_proto::FILE_DESCRIPTOR_SET;
use rusterp_proto::party::v1::party_service_server::PartyService;
use rusterp_proto::party::v1::{
    party_service_server::PartyServiceServer, AddAddressRequest, AddContactRequest, Address,
    AddressKind, Contact, CreatePartyRequest, GetPartyRequest, ListAddressesRequest,
    ListAddressesResponse, ListContactsRequest, ListContactsResponse, ListPartiesRequest,
    ListPartiesResponse, Party, PartyRole, UpdateAddressRequest, UpdateContactRequest,
    UpdatePartyRequest,
};
use rusterp_proto::platform::v1::auth_service_server::AuthServiceServer;
use rusterp_proto::platform::v1::health_service_server::{HealthService, HealthServiceServer};
use rusterp_proto::platform::v1::module_service_server::ModuleServiceServer;
use rusterp_proto::platform::v1::{HealthCheckRequest, HealthCheckResponse};
use rusterp_proto::catalog::v1::catalog_service_server::CatalogServiceServer;
use rusterp_proto::inventory::v1::inventory_service_server::InventoryServiceServer;
use rusterp_proto::payment::v1::payment_service_server::PaymentServiceServer;
use rusterp_proto::sales::v1::sales_service_server::SalesServiceServer;
use rusterp_sales::SalesRepository;
use rusterp_storage::Storage;
use tonic::{Request, Response, Status};

mod domain_svc;
pub use domain_svc::{
    AuthSvc, CatalogSvc, InventorySvc, ModuleSvc, PaymentSvc, SalesSvc,
};

/// Default gRPC listen address.
pub const DEFAULT_LISTEN: &str = "127.0.0.1:50051";
pub const LISTEN_ENV: &str = "RUSTERP_LISTEN";
pub const AUTH_ENFORCE_ENV: &str = "RUSTERP_AUTH_ENFORCE";

pub type SharedRepo = Arc<dyn PartyRepository>;

/// Shared application state for all gRPC services.
#[derive(Clone)]
pub struct AppState {
    pub storage: Arc<dyn Storage>,
    pub parties: SharedRepo,
    pub catalog: Arc<dyn CatalogRepository>,
    pub sales: Arc<dyn SalesRepository>,
    pub payments: Arc<dyn PaymentsRepository>,
    pub inventory: Arc<dyn InventoryRepository>,
    pub auth: Arc<dyn AuthRepository>,
    pub modules: Arc<dyn ModuleStore>,
}

fn auth_enforce_enabled() -> bool {
    matches!(
        std::env::var(AUTH_ENFORCE_ENV).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes")
    )
}

async fn check_write_auth(
    meta: &tonic::metadata::MetadataMap,
    auth: &Arc<dyn AuthRepository>,
) -> Result<(), Status> {
    if !auth_enforce_enabled() {
        return Ok(());
    }
    let login = meta
        .get("x-rusterp-user")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if login.is_empty() {
        return Err(Status::unauthenticated(
            "x-rusterp-user metadata required when RUSTERP_AUTH_ENFORCE is set",
        ));
    }
    let ok = auth
        .user_login_active(login)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;
    if ok {
        Ok(())
    } else {
        Err(Status::permission_denied(format!(
            "unknown or inactive user: {login}"
        )))
    }
}

/// Resolve listen address: `cli_override` → `RUSTERP_LISTEN` → [`DEFAULT_LISTEN`].
pub fn resolve_listen_addr(cli_override: Option<&str>) -> Result<SocketAddr, String> {
    let raw = cli_override
        .map(str::to_string)
        .or_else(|| std::env::var(LISTEN_ENV).ok())
        .unwrap_or_else(|| DEFAULT_LISTEN.to_string());
    raw.parse::<SocketAddr>()
        .map_err(|e| format!("invalid listen address {raw:?}: {e}"))
}

/// Parse CLI args for the server binary (`--listen` / `-l`, `--help`).
pub fn parse_listen_from_args(args: &[String]) -> Result<Option<String>, String> {
    let mut i = 1;
    let mut listen: Option<String> = None;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => return Err("help".into()),
            "--listen" | "-l" => {
                i += 1;
                let val = args
                    .get(i)
                    .ok_or_else(|| "missing value for --listen".to_string())?;
                listen = Some(val.clone());
            }
            other if other.starts_with("--listen=") => {
                listen = Some(other.trim_start_matches("--listen=").to_string());
            }
            "--http-listen" | "-H" => {
                i += 1;
                if args.get(i).is_none() {
                    return Err("missing value for --http-listen".to_string());
                }
            }
            other if other.starts_with("--http-listen=") => {}
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 1;
    }
    Ok(listen)
}

fn map_party_error(err: PartyError) -> Status {
    match err {
        PartyError::NotFound { entity, id } => {
            Status::not_found(format!("{entity} not found: {id}"))
        }
        PartyError::Invalid(msg) => Status::invalid_argument(msg),
    }
}

fn role_to_proto(role: DomainRole) -> PartyRole {
    match role {
        DomainRole::Customer => PartyRole::Customer,
        DomainRole::Supplier => PartyRole::Supplier,
        DomainRole::Prospect => PartyRole::Prospect,
    }
}

fn role_from_proto(role: PartyRole) -> Result<DomainRole, Status> {
    match role {
        PartyRole::Customer => Ok(DomainRole::Customer),
        PartyRole::Supplier => Ok(DomainRole::Supplier),
        PartyRole::Prospect => Ok(DomainRole::Prospect),
        PartyRole::Unspecified => Err(Status::invalid_argument(
            "party role must not be UNSPECIFIED",
        )),
    }
}

fn roles_from_proto(roles: &[i32]) -> Result<BTreeSet<DomainRole>, Status> {
    let mut set = BTreeSet::new();
    for &raw in roles {
        let role = PartyRole::try_from(raw).map_err(|_| {
            Status::invalid_argument(format!("unknown party role value: {raw}"))
        })?;
        set.insert(role_from_proto(role)?);
    }
    Ok(set)
}

fn party_to_proto(p: DomainParty) -> Party {
    Party {
        id: p.id,
        display_name: p.display_name,
        roles: p.roles.into_iter().map(|r| role_to_proto(r) as i32).collect(),
        created_at: p.created_at,
        active: p.active,
    }
}

fn contact_to_proto(c: DomainContact) -> Contact {
    Contact {
        id: c.id,
        party_id: c.party_id,
        name: c.name,
        email: c.email,
        phone: c.phone,
        active: c.active,
    }
}

fn kind_to_proto(kind: DomainAddressKind) -> AddressKind {
    match kind {
        DomainAddressKind::Billing => AddressKind::Billing,
        DomainAddressKind::Shipping => AddressKind::Shipping,
        DomainAddressKind::Other => AddressKind::Other,
    }
}

fn kind_from_proto(kind: AddressKind) -> Result<DomainAddressKind, Status> {
    match kind {
        AddressKind::Billing => Ok(DomainAddressKind::Billing),
        AddressKind::Shipping => Ok(DomainAddressKind::Shipping),
        AddressKind::Other => Ok(DomainAddressKind::Other),
        AddressKind::Unspecified => Ok(DomainAddressKind::Other),
    }
}

fn address_to_proto(a: DomainAddress) -> Address {
    Address {
        id: a.id,
        party_id: a.party_id,
        kind: kind_to_proto(a.kind) as i32,
        line1: a.line1,
        line2: a.line2,
        city: a.city,
        state_region: a.state_region,
        postal_code: a.postal_code,
        country: a.country,
        active: a.active,
    }
}

/// gRPC `PartyService` backed by a [`PartyRepository`].
#[derive(Clone)]
pub struct PartySvc {
    repo: SharedRepo,
    auth: Arc<dyn AuthRepository>,
}

impl PartySvc {
    pub fn new(repo: SharedRepo, auth: Arc<dyn AuthRepository>) -> Self {
        Self { repo, auth }
    }
}

#[tonic::async_trait]
impl PartyService for PartySvc {
    async fn create_party(
        &self,
        request: Request<CreatePartyRequest>,
    ) -> Result<Response<Party>, Status> {
        check_write_auth(request.metadata(), &self.auth).await?;
        let req = request.into_inner();
        let roles = roles_from_proto(&req.roles)?;
        let party = self
            .repo
            .create_party(NewParty {
                display_name: req.display_name,
                roles,
            })
            .await
            .map_err(map_party_error)?;
        Ok(Response::new(party_to_proto(party)))
    }

    async fn get_party(
        &self,
        request: Request<GetPartyRequest>,
    ) -> Result<Response<Party>, Status> {
        let id = request.into_inner().id;
        let party = self.repo.get_party(&id).await.map_err(map_party_error)?;
        Ok(Response::new(party_to_proto(party)))
    }

    async fn list_parties(
        &self,
        request: Request<ListPartiesRequest>,
    ) -> Result<Response<ListPartiesResponse>, Status> {
        let req = request.into_inner();
        let role_filter = match PartyRole::try_from(req.role_filter) {
            Ok(PartyRole::Unspecified) => None,
            Ok(role) => Some(role_from_proto(role)?),
            Err(_) if req.role_filter == 0 => None,
            Err(_) => {
                return Err(Status::invalid_argument(format!(
                    "unknown party role filter: {}",
                    req.role_filter
                )));
            }
        };
        let parties = self
            .repo
            .list_parties(role_filter)
            .await
            .map_err(map_party_error)?
            .into_iter()
            .map(party_to_proto)
            .collect();
        Ok(Response::new(ListPartiesResponse { parties }))
    }

    async fn update_party(
        &self,
        request: Request<UpdatePartyRequest>,
    ) -> Result<Response<Party>, Status> {
        check_write_auth(request.metadata(), &self.auth).await?;
        let req = request.into_inner();
        let mut update = PartyUpdate {
            display_name: req.display_name,
            active: req.active,
            roles: None,
        };
        if req.update_roles {
            update.roles = Some(roles_from_proto(&req.roles)?);
        }
        let party = self
            .repo
            .update_party(&req.id, update)
            .await
            .map_err(map_party_error)?;
        Ok(Response::new(party_to_proto(party)))
    }

    async fn add_contact(
        &self,
        request: Request<AddContactRequest>,
    ) -> Result<Response<Contact>, Status> {
        check_write_auth(request.metadata(), &self.auth).await?;
        let req = request.into_inner();
        let contact = self
            .repo
            .add_contact(
                &req.party_id,
                NewContact {
                    name: req.name,
                    email: req.email,
                    phone: req.phone,
                },
            )
            .await
            .map_err(map_party_error)?;
        Ok(Response::new(contact_to_proto(contact)))
    }

    async fn list_contacts(
        &self,
        request: Request<ListContactsRequest>,
    ) -> Result<Response<ListContactsResponse>, Status> {
        let party_id = request.into_inner().party_id;
        let contacts = self
            .repo
            .list_contacts(&party_id)
            .await
            .map_err(map_party_error)?
            .into_iter()
            .map(contact_to_proto)
            .collect();
        Ok(Response::new(ListContactsResponse { contacts }))
    }

    async fn update_contact(
        &self,
        request: Request<UpdateContactRequest>,
    ) -> Result<Response<Contact>, Status> {
        check_write_auth(request.metadata(), &self.auth).await?;
        let req = request.into_inner();
        let contact = self
            .repo
            .update_contact(
                &req.id,
                ContactUpdate {
                    name: req.name,
                    email: req.email.map(Some),
                    phone: req.phone.map(Some),
                    active: req.active,
                },
            )
            .await
            .map_err(map_party_error)?;
        Ok(Response::new(contact_to_proto(contact)))
    }

    async fn add_address(
        &self,
        request: Request<AddAddressRequest>,
    ) -> Result<Response<Address>, Status> {
        check_write_auth(request.metadata(), &self.auth).await?;
        let req = request.into_inner();
        let kind = AddressKind::try_from(req.kind)
            .map_err(|_| Status::invalid_argument(format!("unknown address kind: {}", req.kind)))?;
        let address = self
            .repo
            .add_address(
                &req.party_id,
                NewAddress {
                    kind: kind_from_proto(kind)?,
                    line1: req.line1,
                    line2: req.line2,
                    city: req.city,
                    state_region: req.state_region,
                    postal_code: req.postal_code,
                    country: if req.country.trim().is_empty() {
                        "AU".into()
                    } else {
                        req.country
                    },
                },
            )
            .await
            .map_err(map_party_error)?;
        Ok(Response::new(address_to_proto(address)))
    }

    async fn list_addresses(
        &self,
        request: Request<ListAddressesRequest>,
    ) -> Result<Response<ListAddressesResponse>, Status> {
        let party_id = request.into_inner().party_id;
        let addresses = self
            .repo
            .list_addresses(&party_id)
            .await
            .map_err(map_party_error)?
            .into_iter()
            .map(address_to_proto)
            .collect();
        Ok(Response::new(ListAddressesResponse { addresses }))
    }

    async fn update_address(
        &self,
        request: Request<UpdateAddressRequest>,
    ) -> Result<Response<Address>, Status> {
        check_write_auth(request.metadata(), &self.auth).await?;
        let req = request.into_inner();
        let kind = if let Some(k) = req.kind {
            Some(
                kind_from_proto(
                    AddressKind::try_from(k)
                        .map_err(|_| Status::invalid_argument(format!("unknown address kind: {k}")))?,
                )?,
            )
        } else {
            None
        };
        let address = self
            .repo
            .update_address(
                &req.id,
                AddressUpdate {
                    kind,
                    line1: req.line1,
                    line2: req.line2.map(Some),
                    city: req.city,
                    state_region: req.state_region.map(Some),
                    postal_code: req.postal_code.map(Some),
                    country: req.country,
                    active: req.active,
                },
            )
            .await
            .map_err(map_party_error)?;
        Ok(Response::new(address_to_proto(address)))
    }
}

/// Health service — reports database connectivity via storage ping.
#[derive(Clone)]
pub struct HealthSvc {
    storage: Arc<dyn Storage>,
}

impl HealthSvc {
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self { storage }
    }
}

#[tonic::async_trait]
impl HealthService for HealthSvc {
    async fn check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        self.storage
            .ping()
            .await
            .map_err(|e| Status::unavailable(e.message()))?;
        Ok(Response::new(HealthCheckResponse {
            status: "ok".into(),
        }))
    }
}

/// Build tonic service routes shared by TCP and slozhn HTTP transports.
pub fn build_grpc_routes(
    state: AppState,
) -> Result<tonic::service::Routes, Box<dyn std::error::Error + Send + Sync>> {
    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()?;

    let mut routes = tonic::service::Routes::builder();
    routes
        .add_service(reflection)
        .add_service(PartyServiceServer::new(PartySvc::new(
            state.parties.clone(),
            state.auth.clone(),
        )))
        .add_service(HealthServiceServer::new(HealthSvc::new(state.storage.clone())))
        .add_service(CatalogServiceServer::new(CatalogSvc {
            repo: state.catalog.clone(),
        }))
        .add_service(SalesServiceServer::new(SalesSvc {
            repo: state.sales.clone(),
        }))
        .add_service(PaymentServiceServer::new(PaymentSvc {
            repo: state.payments.clone(),
        }))
        .add_service(InventoryServiceServer::new(InventorySvc {
            repo: state.inventory.clone(),
        }))
        .add_service(ModuleServiceServer::new(ModuleSvc {
            store: state.modules.clone(),
        }))
        .add_service(AuthServiceServer::new(AuthSvc {
            repo: state.auth.clone(),
        }));

    Ok(routes.routes())
}

/// Build the tonic TCP router (no bind).
pub fn build_router(
    state: AppState,
) -> Result<tonic::transport::server::Router, Box<dyn std::error::Error + Send + Sync>> {
    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()?;

    let router = tonic::transport::Server::builder()
        .add_service(reflection)
        .add_service(PartyServiceServer::new(PartySvc::new(
            state.parties.clone(),
            state.auth.clone(),
        )))
        .add_service(HealthServiceServer::new(HealthSvc::new(state.storage.clone())))
        .add_service(CatalogServiceServer::new(CatalogSvc {
            repo: state.catalog.clone(),
        }))
        .add_service(SalesServiceServer::new(SalesSvc {
            repo: state.sales.clone(),
        }))
        .add_service(PaymentServiceServer::new(PaymentSvc {
            repo: state.payments.clone(),
        }))
        .add_service(InventoryServiceServer::new(InventorySvc {
            repo: state.inventory.clone(),
        }))
        .add_service(ModuleServiceServer::new(ModuleSvc {
            store: state.modules.clone(),
        }))
        .add_service(AuthServiceServer::new(AuthSvc {
            repo: state.auth.clone(),
        }));

    Ok(router)
}

/// Create a fresh shared repository backed by in-memory storage (for tests).
pub fn new_shared_repo() -> SharedRepo {
    Arc::new(InMemoryPartyRepository::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn party_service_create_get_list_wiring() {
        let repo = new_shared_repo();
        let svc = PartySvc::new(repo, test_auth());

        let created = svc
            .create_party(Request::new(CreatePartyRequest {
                display_name: "Acme Wiring Co".into(),
                roles: vec![PartyRole::Customer as i32, PartyRole::Supplier as i32],
            }))
            .await
            .expect("create")
            .into_inner();

        assert!(!created.id.is_empty());
        assert_eq!(created.display_name, "Acme Wiring Co");
        assert_eq!(created.roles.len(), 2);
        assert!(created.active);

        let fetched = svc
            .get_party(Request::new(GetPartyRequest {
                id: created.id.clone(),
            }))
            .await
            .expect("get")
            .into_inner();
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.display_name, "Acme Wiring Co");

        let listed = svc
            .list_parties(Request::new(ListPartiesRequest {
                role_filter: PartyRole::Unspecified as i32,
            }))
            .await
            .expect("list")
            .into_inner();
        assert_eq!(listed.parties.len(), 1);
        assert_eq!(listed.parties[0].id, created.id);
    }

    #[tokio::test]
    async fn list_parties_role_filter_and_addresses() {
        let repo = new_shared_repo();
        let svc = PartySvc::new(repo, test_auth());

        let _customer = svc
            .create_party(Request::new(CreatePartyRequest {
                display_name: "Only Customer".into(),
                roles: vec![PartyRole::Customer as i32],
            }))
            .await
            .expect("create customer");
        let supplier = svc
            .create_party(Request::new(CreatePartyRequest {
                display_name: "Only Supplier".into(),
                roles: vec![PartyRole::Supplier as i32],
            }))
            .await
            .expect("create supplier")
            .into_inner();

        let filtered = svc
            .list_parties(Request::new(ListPartiesRequest {
                role_filter: PartyRole::Supplier as i32,
            }))
            .await
            .expect("filter")
            .into_inner();
        assert_eq!(filtered.parties.len(), 1);
        assert_eq!(filtered.parties[0].id, supplier.id);

        let address = svc
            .add_address(Request::new(AddAddressRequest {
                party_id: supplier.id.clone(),
                kind: AddressKind::Billing as i32,
                line1: "1 Main St".into(),
                line2: None,
                city: "Sydney".into(),
                state_region: Some("NSW".into()),
                postal_code: Some("2000".into()),
                country: "AU".into(),
            }))
            .await
            .expect("add address")
            .into_inner();
        assert_eq!(address.city, "Sydney");

        let listed = svc
            .list_addresses(Request::new(ListAddressesRequest {
                party_id: supplier.id,
            }))
            .await
            .expect("list addresses")
            .into_inner();
        assert_eq!(listed.addresses.len(), 1);
        assert_eq!(listed.addresses[0].id, address.id);
    }

    #[tokio::test]
    async fn party_service_update_and_contacts() {
        let repo = new_shared_repo();
        let svc = PartySvc::new(repo, test_auth());

        let party = svc
            .create_party(Request::new(CreatePartyRequest {
                display_name: "Contact Host".into(),
                roles: vec![PartyRole::Prospect as i32],
            }))
            .await
            .expect("create")
            .into_inner();

        let updated = svc
            .update_party(Request::new(UpdatePartyRequest {
                id: party.id.clone(),
                display_name: Some("Contact Host Renamed".into()),
                active: Some(true),
                update_roles: true,
                roles: vec![PartyRole::Customer as i32],
            }))
            .await
            .expect("update")
            .into_inner();
        assert_eq!(updated.display_name, "Contact Host Renamed");
        assert_eq!(updated.roles, vec![PartyRole::Customer as i32]);

        let contact = svc
            .add_contact(Request::new(AddContactRequest {
                party_id: party.id.clone(),
                name: "Ada".into(),
                email: Some("ada@example.com".into()),
                phone: None,
            }))
            .await
            .expect("add contact")
            .into_inner();
        assert_eq!(contact.party_id, party.id);
        assert_eq!(contact.name, "Ada");

        let contacts = svc
            .list_contacts(Request::new(ListContactsRequest {
                party_id: party.id,
            }))
            .await
            .expect("list contacts")
            .into_inner();
        assert_eq!(contacts.contacts.len(), 1);
        assert_eq!(contacts.contacts[0].id, contact.id);
    }

    #[tokio::test]
    async fn party_service_unknown_id_is_not_found() {
        let svc = PartySvc::new(new_shared_repo(), test_auth());
        let err = svc
            .get_party(Request::new(GetPartyRequest {
                id: "missing".into(),
            }))
            .await
            .expect_err("not found");
        assert_eq!(err.code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn health_check_returns_ok() {
        let svc = HealthSvc::new(build_test_storage());
        let resp = svc
            .check(Request::new(HealthCheckRequest {}))
            .await
            .expect("check")
            .into_inner();
        assert_eq!(resp.status, "ok");
    }

    #[test]
    fn resolve_listen_default() {
        // Ensure default parses; do not rely on clearing env in parallel tests.
        let addr = DEFAULT_LISTEN.parse::<SocketAddr>().unwrap();
        assert_eq!(addr.port(), 50051);
    }

    #[test]
    fn parse_listen_flag() {
        let args = vec![
            "rusterp-server".into(),
            "--listen".into(),
            "127.0.0.1:60051".into(),
        ];
        let got = parse_listen_from_args(&args).unwrap();
        assert_eq!(got.as_deref(), Some("127.0.0.1:60051"));
    }

    #[test]
    fn build_router_succeeds() {
        build_router(test_app_state()).expect("router");
    }

    #[test]
    fn build_grpc_routes_succeeds() {
        build_grpc_routes(test_app_state()).expect("grpc routes");
    }

    fn test_auth() -> Arc<dyn AuthRepository> {
        Arc::new(StubAuth)
    }

    fn test_app_state() -> AppState {
        AppState {
            storage: build_test_storage(),
            parties: new_shared_repo(),
            catalog: Arc::new(StubCatalog),
            sales: Arc::new(StubSales),
            payments: Arc::new(StubPayments),
            inventory: Arc::new(StubInventory),
            auth: test_auth(),
            modules: Arc::new(StubAuth),
        }
    }

    fn build_test_storage() -> Arc<dyn Storage> {
        use async_trait::async_trait;
        use rusterp_storage::StorageError;

        struct OkStorage;

        #[async_trait]
        impl Storage for OkStorage {
            async fn ping(&self) -> Result<(), StorageError> {
                Ok(())
            }
        }

        Arc::new(OkStorage)
    }

    struct StubAuth;
    #[async_trait::async_trait]
    impl AuthRepository for StubAuth {
        async fn list_users(&self) -> Result<Vec<rusterp_auth::UserInfo>, rusterp_auth::AuthError> { Ok(vec![]) }
        async fn list_roles(&self) -> Result<Vec<rusterp_auth::RoleInfo>, rusterp_auth::AuthError> { Ok(vec![]) }
        async fn list_permissions(&self) -> Result<Vec<rusterp_auth::PermissionInfo>, rusterp_auth::AuthError> { Ok(vec![]) }
        async fn create_user(&self, _: String, _: String, _: String) -> Result<rusterp_auth::UserInfo, rusterp_auth::AuthError> {
            Err(rusterp_auth::AuthError::Invalid("stub".into()))
        }
        async fn update_user(&self, _: &str, _: Option<String>, _: Option<bool>, _: Option<String>) -> Result<rusterp_auth::UserInfo, rusterp_auth::AuthError> {
            Err(rusterp_auth::AuthError::Invalid("stub".into()))
        }
        async fn user_login_active(&self, _: &str) -> Result<bool, rusterp_auth::AuthError> { Ok(false) }
    }
    #[async_trait::async_trait]
    impl ModuleStore for StubAuth {
        async fn list_modules(&self) -> Result<Vec<rusterp_auth::ModuleInfo>, rusterp_auth::AuthError> { Ok(vec![]) }
        async fn set_module_enabled(&self, _: &str, _: bool) -> Result<rusterp_auth::ModuleInfo, rusterp_auth::AuthError> {
            Err(rusterp_auth::AuthError::Invalid("stub".into()))
        }
    }

    struct StubCatalog;
    #[async_trait::async_trait]
    impl CatalogRepository for StubCatalog {
        async fn create_product(&self, _: rusterp_catalog::NewProduct) -> Result<rusterp_catalog::Product, rusterp_catalog::CatalogError> {
            Err(rusterp_catalog::CatalogError::Invalid("stub".into()))
        }
        async fn list_products(&self) -> Result<Vec<rusterp_catalog::Product>, rusterp_catalog::CatalogError> { Ok(vec![]) }
        async fn update_product(&self, _: &str, _: rusterp_catalog::ProductUpdate) -> Result<rusterp_catalog::Product, rusterp_catalog::CatalogError> {
            Err(rusterp_catalog::CatalogError::Invalid("stub".into()))
        }
        async fn create_category(&self, _: rusterp_catalog::NewCategory) -> Result<rusterp_catalog::Category, rusterp_catalog::CatalogError> {
            Err(rusterp_catalog::CatalogError::Invalid("stub".into()))
        }
        async fn list_categories(&self) -> Result<Vec<rusterp_catalog::Category>, rusterp_catalog::CatalogError> { Ok(vec![]) }
        async fn update_category(&self, _: &str, _: rusterp_catalog::CategoryUpdate) -> Result<rusterp_catalog::Category, rusterp_catalog::CatalogError> {
            Err(rusterp_catalog::CatalogError::Invalid("stub".into()))
        }
    }

    struct StubSales;
    #[async_trait::async_trait]
    impl SalesRepository for StubSales {
        async fn create_document(&self, _: rusterp_sales::NewSalesDocument) -> Result<rusterp_sales::SalesDocument, rusterp_sales::SalesError> {
            Err(rusterp_sales::SalesError::Invalid("stub".into()))
        }
        async fn list_documents(&self, _: Option<rusterp_sales::DocumentKind>) -> Result<Vec<rusterp_sales::SalesDocument>, rusterp_sales::SalesError> { Ok(vec![]) }
        async fn get_document(&self, _: &str) -> Result<(rusterp_sales::SalesDocument, Vec<rusterp_sales::SalesDocumentLine>), rusterp_sales::SalesError> {
            Err(rusterp_sales::SalesError::NotFound("stub".into()))
        }
        async fn set_status(&self, _: &str, _: rusterp_sales::DocumentStatus) -> Result<rusterp_sales::SalesDocument, rusterp_sales::SalesError> {
            Err(rusterp_sales::SalesError::Invalid("stub".into()))
        }
        async fn update_document(&self, _: &str, _: Option<String>) -> Result<rusterp_sales::SalesDocument, rusterp_sales::SalesError> {
            Err(rusterp_sales::SalesError::Invalid("stub".into()))
        }
    }

    struct StubPayments;
    #[async_trait::async_trait]
    impl PaymentsRepository for StubPayments {
        async fn create_bank_account(&self, _: String, _: String) -> Result<rusterp_payments::BankAccount, rusterp_payments::PaymentError> {
            Err(rusterp_payments::PaymentError::Invalid("stub".into()))
        }
        async fn list_bank_accounts(&self) -> Result<Vec<rusterp_payments::BankAccount>, rusterp_payments::PaymentError> { Ok(vec![]) }
        async fn update_bank_account(&self, _: &str, _: Option<String>, _: Option<String>, _: Option<bool>) -> Result<rusterp_payments::BankAccount, rusterp_payments::PaymentError> {
            Err(rusterp_payments::PaymentError::Invalid("stub".into()))
        }
        async fn create_payment(&self, _: rusterp_payments::PaymentDirection, _: String, _: Option<String>, _: i64, _: String, _: String) -> Result<rusterp_payments::Payment, rusterp_payments::PaymentError> {
            Err(rusterp_payments::PaymentError::Invalid("stub".into()))
        }
        async fn list_payments(&self) -> Result<Vec<rusterp_payments::Payment>, rusterp_payments::PaymentError> { Ok(vec![]) }
        async fn update_payment(&self, _: &str, _: Option<String>, _: Option<Option<String>>) -> Result<rusterp_payments::Payment, rusterp_payments::PaymentError> {
            Err(rusterp_payments::PaymentError::Invalid("stub".into()))
        }
        async fn create_allocation(&self, _: String, _: String, _: i64) -> Result<rusterp_payments::PaymentAllocation, rusterp_payments::PaymentError> {
            Err(rusterp_payments::PaymentError::Invalid("stub".into()))
        }
        async fn list_allocations(&self, _: &str) -> Result<Vec<rusterp_payments::PaymentAllocation>, rusterp_payments::PaymentError> { Ok(vec![]) }
    }

    struct StubInventory;
    #[async_trait::async_trait]
    impl InventoryRepository for StubInventory {
        async fn is_enabled(&self) -> Result<bool, rusterp_inventory::InventoryError> { Ok(false) }
        async fn create_warehouse(&self, _: String, _: String) -> Result<rusterp_inventory::Warehouse, rusterp_inventory::InventoryError> {
            Err(rusterp_inventory::InventoryError::Disabled)
        }
        async fn list_warehouses(&self) -> Result<Vec<rusterp_inventory::Warehouse>, rusterp_inventory::InventoryError> { Ok(vec![]) }
        async fn update_warehouse(&self, _: &str, _: Option<String>, _: Option<String>, _: Option<bool>) -> Result<rusterp_inventory::Warehouse, rusterp_inventory::InventoryError> {
            Err(rusterp_inventory::InventoryError::Disabled)
        }
        async fn list_stock_levels(&self, _: Option<String>) -> Result<Vec<rusterp_inventory::StockLevel>, rusterp_inventory::InventoryError> { Ok(vec![]) }
        async fn create_stock_move(&self, _: String, _: String, _: Option<String>, _: Option<String>) -> Result<rusterp_inventory::StockMove, rusterp_inventory::InventoryError> {
            Err(rusterp_inventory::InventoryError::Disabled)
        }
        async fn list_stock_moves(&self) -> Result<Vec<rusterp_inventory::StockMove>, rusterp_inventory::InventoryError> { Ok(vec![]) }
    }
}
