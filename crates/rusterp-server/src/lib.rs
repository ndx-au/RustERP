//! RustERP gRPC server library: service wiring over parties state.
//!
//! # Runtime
//!
//! Async stack is **tokio** + **tonic**. Persistence uses
//! [`rusterp_parties::PostgresPartyRepository`] (durable via PostgreSQL/sqlx).
//! **Auth is not enforced.**
//!
//! # Listen address
//!
//! Default: [`DEFAULT_LISTEN`] (`127.0.0.1:50051`).
//! Override via CLI `--listen <addr>` or env `RUSTERP_LISTEN` (CLI wins).

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::Arc;

use rusterp_parties::{
    Contact as DomainContact, InMemoryPartyRepository, NewContact, NewParty,
    Party as DomainParty, PartyError, PartyRepository, PartyRole as DomainRole, PartyUpdate,
};
use rusterp_storage::Storage;
use rusterp_proto::party::v1::party_service_server::PartyService;
use rusterp_proto::party::v1::{
    party_service_server::PartyServiceServer, AddContactRequest, Contact, CreatePartyRequest,
    GetPartyRequest, ListContactsRequest, ListContactsResponse, ListPartiesRequest,
    ListPartiesResponse, Party, PartyRole, UpdatePartyRequest,
};
use rusterp_proto::platform::v1::health_service_server::{HealthService, HealthServiceServer};
use rusterp_proto::platform::v1::{HealthCheckRequest, HealthCheckResponse};
use rusterp_proto::FILE_DESCRIPTOR_SET;
use tonic::{Request, Response, Status};

/// Default gRPC listen address (Phase 2).
pub const DEFAULT_LISTEN: &str = "127.0.0.1:50051";

/// Environment variable for listen override (CLI `--listen` takes precedence).
pub const LISTEN_ENV: &str = "RUSTERP_LISTEN";

/// Shared party store — backed by a trait object so the concrete repository
/// can be swapped without changing the service wiring.
pub type SharedRepo = Arc<dyn PartyRepository>;

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
    }
}

/// gRPC `PartyService` backed by a [`PartyRepository`].
#[derive(Clone)]
pub struct PartySvc {
    repo: SharedRepo,
}

impl PartySvc {
    pub fn new(repo: SharedRepo) -> Self {
        Self { repo }
    }
}

#[tonic::async_trait]
impl PartyService for PartySvc {
    async fn create_party(
        &self,
        request: Request<CreatePartyRequest>,
    ) -> Result<Response<Party>, Status> {
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
        _request: Request<ListPartiesRequest>,
    ) -> Result<Response<ListPartiesResponse>, Status> {
        let parties = self
            .repo
            .list_parties()
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
pub fn build_grpc_routes(storage: Arc<dyn Storage>, repo: SharedRepo) -> Result<tonic::service::Routes, Box<dyn std::error::Error + Send + Sync>> {
    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()?;

    let mut routes = tonic::service::Routes::builder();
    routes
        .add_service(reflection)
        .add_service(PartyServiceServer::new(PartySvc::new(repo.clone())))
        .add_service(HealthServiceServer::new(HealthSvc::new(storage.clone())));

    Ok(routes.routes())
}

/// Build the tonic TCP router with Party + Health + reflection (no bind).
pub fn build_router(storage: Arc<dyn Storage>, repo: SharedRepo) -> Result<tonic::transport::server::Router, Box<dyn std::error::Error + Send + Sync>> {
    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()?;

    let router = tonic::transport::Server::builder()
        .add_service(reflection)
        .add_service(PartyServiceServer::new(PartySvc::new(repo)))
        .add_service(HealthServiceServer::new(HealthSvc::new(storage)));

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
        let svc = PartySvc::new(repo);

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
            .list_parties(Request::new(ListPartiesRequest {}))
            .await
            .expect("list")
            .into_inner();
        assert_eq!(listed.parties.len(), 1);
        assert_eq!(listed.parties[0].id, created.id);
    }

    #[tokio::test]
    async fn party_service_update_and_contacts() {
        let repo = new_shared_repo();
        let svc = PartySvc::new(repo);

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
        let svc = PartySvc::new(new_shared_repo());
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
        let storage = build_test_storage();
        build_router(storage, new_shared_repo()).expect("router");
    }

    #[test]
    fn build_grpc_routes_succeeds() {
        let storage = build_test_storage();
        build_grpc_routes(storage, new_shared_repo()).expect("grpc routes");
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
}
