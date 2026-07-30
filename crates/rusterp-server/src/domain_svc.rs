//! gRPC service adapters for catalog, sales, payments, inventory, modules, auth.

use std::sync::Arc;

use rusterp_auth::{AuthError, AuthRepository, ModuleInfo, ModuleStore};
use rusterp_catalog::{
    CatalogError, CatalogRepository, Category as DomCategory, NewCategory, NewProduct,
    Product as DomProduct, ProductType as DomProductType,
};
use rusterp_inventory::{InventoryError, InventoryRepository};
use rusterp_payments::{PaymentDirection as DomDir, PaymentError, PaymentsRepository};
use rusterp_sales::{
    DocumentKind as DomKind, DocumentStatus as DomStatus, NewSalesDocument, SalesError,
    SalesRepository,
};
use rusterp_proto::catalog::v1::catalog_service_server::CatalogService;
use rusterp_proto::catalog::v1::{
    Category, CreateCategoryRequest, CreateProductRequest, ListCategoriesRequest,
    ListCategoriesResponse, ListProductsRequest, ListProductsResponse, Product, ProductType,
};
use rusterp_proto::inventory::v1::inventory_service_server::InventoryService;
use rusterp_proto::inventory::v1::{
    CreateStockMoveRequest, CreateWarehouseRequest, ListStockLevelsRequest,
    ListStockLevelsResponse, ListStockMovesRequest, ListStockMovesResponse, ListWarehousesRequest,
    ListWarehousesResponse, StockLevel, StockMove, Warehouse,
};
use rusterp_proto::payment::v1::payment_service_server::PaymentService;
use rusterp_proto::payment::v1::{
    BankAccount, CreateAllocationRequest, CreateBankAccountRequest, CreatePaymentRequest,
    ListAllocationsRequest, ListAllocationsResponse, ListBankAccountsRequest,
    ListBankAccountsResponse, ListPaymentsRequest, ListPaymentsResponse, Payment,
    PaymentAllocation, PaymentDirection,
};
use rusterp_proto::platform::v1::auth_service_server::AuthService;
use rusterp_proto::platform::v1::module_service_server::ModuleService;
use rusterp_proto::platform::v1::{
    CreateUserRequest, ListModulesRequest, ListModulesResponse, ListPermissionsRequest,
    ListPermissionsResponse, ListRolesRequest, ListRolesResponse, ListUsersRequest,
    ListUsersResponse, ModuleInfo as ProtoModule, PermissionInfo, RoleInfo, SetModuleEnabledRequest,
    UserInfo,
};
use rusterp_proto::sales::v1::sales_service_server::SalesService;
use rusterp_proto::sales::v1::{
    CreateSalesDocumentRequest, DocumentKind, DocumentStatus, GetSalesDocumentRequest,
    GetSalesDocumentResponse, ListSalesDocumentsRequest, ListSalesDocumentsResponse,
    SalesDocument, SalesDocumentLine, SetSalesDocumentStatusRequest,
};
use tonic::{Request, Response, Status};

fn catalog_err(e: CatalogError) -> Status {
    match e {
        CatalogError::NotFound(m) => Status::not_found(m),
        CatalogError::Invalid(m) => Status::invalid_argument(m),
    }
}
fn sales_err(e: SalesError) -> Status {
    match e {
        SalesError::NotFound(m) => Status::not_found(m),
        SalesError::Invalid(m) => Status::invalid_argument(m),
    }
}
fn pay_err(e: PaymentError) -> Status {
    match e {
        PaymentError::NotFound(m) => Status::not_found(m),
        PaymentError::Invalid(m) => Status::invalid_argument(m),
    }
}
fn inv_err(e: InventoryError) -> Status {
    match e {
        InventoryError::NotFound(m) => Status::not_found(m),
        InventoryError::Invalid(m) => Status::invalid_argument(m),
        InventoryError::Disabled => Status::failed_precondition(e.to_string()),
    }
}
fn auth_err(e: AuthError) -> Status {
    match e {
        AuthError::NotFound(m) => Status::not_found(m),
        AuthError::Invalid(m) => Status::invalid_argument(m),
    }
}

#[derive(Clone)]
pub struct CatalogSvc {
    pub repo: Arc<dyn CatalogRepository>,
}

#[tonic::async_trait]
impl CatalogService for CatalogSvc {
    async fn create_product(
        &self,
        request: Request<CreateProductRequest>,
    ) -> Result<Response<Product>, Status> {
        let req = request.into_inner();
        let product_type = match ProductType::try_from(req.r#type).unwrap_or(ProductType::Stock) {
            ProductType::Service => DomProductType::Service,
            ProductType::Consumable => DomProductType::Consumable,
            _ => DomProductType::Stock,
        };
        let p = self
            .repo
            .create_product(NewProduct {
                sku: req.sku,
                name: req.name,
                product_type,
                category_id: req.category_id,
            })
            .await
            .map_err(catalog_err)?;
        Ok(Response::new(product_to_proto(p)))
    }

    async fn list_products(
        &self,
        _request: Request<ListProductsRequest>,
    ) -> Result<Response<ListProductsResponse>, Status> {
        let products = self
            .repo
            .list_products()
            .await
            .map_err(catalog_err)?
            .into_iter()
            .map(product_to_proto)
            .collect();
        Ok(Response::new(ListProductsResponse { products }))
    }

    async fn create_category(
        &self,
        request: Request<CreateCategoryRequest>,
    ) -> Result<Response<Category>, Status> {
        let req = request.into_inner();
        let c = self
            .repo
            .create_category(NewCategory {
                name: req.name,
                parent_id: req.parent_id,
            })
            .await
            .map_err(catalog_err)?;
        Ok(Response::new(category_to_proto(c)))
    }

    async fn list_categories(
        &self,
        _request: Request<ListCategoriesRequest>,
    ) -> Result<Response<ListCategoriesResponse>, Status> {
        let categories = self
            .repo
            .list_categories()
            .await
            .map_err(catalog_err)?
            .into_iter()
            .map(category_to_proto)
            .collect();
        Ok(Response::new(ListCategoriesResponse { categories }))
    }
}

fn product_to_proto(p: DomProduct) -> Product {
    Product {
        id: p.id,
        sku: p.sku,
        name: p.name,
        r#type: match p.product_type {
            DomProductType::Stock => ProductType::Stock as i32,
            DomProductType::Service => ProductType::Service as i32,
            DomProductType::Consumable => ProductType::Consumable as i32,
        },
        category_id: p.category_id,
        uom_id: p.uom_id,
        active: p.active,
    }
}

fn category_to_proto(c: DomCategory) -> Category {
    Category {
        id: c.id,
        name: c.name,
        parent_id: c.parent_id,
        active: c.active,
    }
}

#[derive(Clone)]
pub struct SalesSvc {
    pub repo: Arc<dyn SalesRepository>,
}

#[tonic::async_trait]
impl SalesService for SalesSvc {
    async fn create_sales_document(
        &self,
        request: Request<CreateSalesDocumentRequest>,
    ) -> Result<Response<SalesDocument>, Status> {
        let req = request.into_inner();
        let kind = match DocumentKind::try_from(req.kind).unwrap_or(DocumentKind::Quote) {
            DocumentKind::Order => DomKind::Order,
            DocumentKind::Invoice => DomKind::Invoice,
            DocumentKind::CreditNote => DomKind::CreditNote,
            _ => DomKind::Quote,
        };
        let doc = self
            .repo
            .create_document(NewSalesDocument {
                kind,
                party_id: req.party_id,
                description: req.description,
                unit_price_minor: req.unit_price_minor,
                product_id: req.product_id,
                notes: req.notes,
            })
            .await
            .map_err(sales_err)?;
        Ok(Response::new(sales_doc_to_proto(doc)))
    }

    async fn list_sales_documents(
        &self,
        request: Request<ListSalesDocumentsRequest>,
    ) -> Result<Response<ListSalesDocumentsResponse>, Status> {
        let filter = match DocumentKind::try_from(request.into_inner().kind_filter) {
            Ok(DocumentKind::Unspecified) | Err(_) => None,
            Ok(DocumentKind::Order) => Some(DomKind::Order),
            Ok(DocumentKind::Invoice) => Some(DomKind::Invoice),
            Ok(DocumentKind::CreditNote) => Some(DomKind::CreditNote),
            Ok(DocumentKind::Quote) => Some(DomKind::Quote),
        };
        let documents = self
            .repo
            .list_documents(filter)
            .await
            .map_err(sales_err)?
            .into_iter()
            .map(sales_doc_to_proto)
            .collect();
        Ok(Response::new(ListSalesDocumentsResponse { documents }))
    }

    async fn get_sales_document(
        &self,
        request: Request<GetSalesDocumentRequest>,
    ) -> Result<Response<GetSalesDocumentResponse>, Status> {
        let (doc, lines) = self
            .repo
            .get_document(&request.into_inner().id)
            .await
            .map_err(sales_err)?;
        Ok(Response::new(GetSalesDocumentResponse {
            document: Some(sales_doc_to_proto(doc)),
            lines: lines
                .into_iter()
                .map(|l| SalesDocumentLine {
                    id: l.id,
                    document_id: l.document_id,
                    line_no: l.line_no,
                    description: l.description,
                    unit_price_minor: l.unit_price_minor,
                    total_minor: l.total_minor,
                    product_id: l.product_id,
                })
                .collect(),
        }))
    }

    async fn set_sales_document_status(
        &self,
        request: Request<SetSalesDocumentStatusRequest>,
    ) -> Result<Response<SalesDocument>, Status> {
        let req = request.into_inner();
        let status = match DocumentStatus::try_from(req.status).unwrap_or(DocumentStatus::Draft) {
            DocumentStatus::Confirmed => DomStatus::Confirmed,
            DocumentStatus::Posted => DomStatus::Posted,
            DocumentStatus::Cancelled => DomStatus::Cancelled,
            _ => DomStatus::Draft,
        };
        let doc = self
            .repo
            .set_status(&req.id, status)
            .await
            .map_err(sales_err)?;
        Ok(Response::new(sales_doc_to_proto(doc)))
    }
}

fn sales_doc_to_proto(d: rusterp_sales::SalesDocument) -> SalesDocument {
    SalesDocument {
        id: d.id,
        kind: match d.kind {
            DomKind::Quote => DocumentKind::Quote as i32,
            DomKind::Order => DocumentKind::Order as i32,
            DomKind::Invoice => DocumentKind::Invoice as i32,
            DomKind::CreditNote => DocumentKind::CreditNote as i32,
        },
        status: match d.status {
            DomStatus::Draft => DocumentStatus::Draft as i32,
            DomStatus::Confirmed => DocumentStatus::Confirmed as i32,
            DomStatus::Posted => DocumentStatus::Posted as i32,
            DomStatus::Cancelled => DocumentStatus::Cancelled as i32,
        },
        number: d.number,
        party_id: d.party_id,
        currency: d.currency,
        total_minor: d.total_minor,
        notes: d.notes,
    }
}

#[derive(Clone)]
pub struct PaymentSvc {
    pub repo: Arc<dyn PaymentsRepository>,
}

#[tonic::async_trait]
impl PaymentService for PaymentSvc {
    async fn create_bank_account(
        &self,
        request: Request<CreateBankAccountRequest>,
    ) -> Result<Response<BankAccount>, Status> {
        let req = request.into_inner();
        let a = self
            .repo
            .create_bank_account(req.name, req.currency)
            .await
            .map_err(pay_err)?;
        Ok(Response::new(BankAccount {
            id: a.id,
            name: a.name,
            currency: a.currency,
            active: a.active,
        }))
    }

    async fn list_bank_accounts(
        &self,
        _request: Request<ListBankAccountsRequest>,
    ) -> Result<Response<ListBankAccountsResponse>, Status> {
        let accounts = self
            .repo
            .list_bank_accounts()
            .await
            .map_err(pay_err)?
            .into_iter()
            .map(|a| BankAccount {
                id: a.id,
                name: a.name,
                currency: a.currency,
                active: a.active,
            })
            .collect();
        Ok(Response::new(ListBankAccountsResponse { accounts }))
    }

    async fn create_payment(
        &self,
        request: Request<CreatePaymentRequest>,
    ) -> Result<Response<Payment>, Status> {
        let req = request.into_inner();
        let direction = match PaymentDirection::try_from(req.direction)
            .unwrap_or(PaymentDirection::Inbound)
        {
            PaymentDirection::Outbound => DomDir::Outbound,
            _ => DomDir::Inbound,
        };
        let p = self
            .repo
            .create_payment(
                direction,
                req.party_id,
                req.bank_account_id,
                req.amount_minor,
                req.currency,
                req.reference,
            )
            .await
            .map_err(pay_err)?;
        Ok(Response::new(Payment {
            id: p.id,
            direction: match p.direction {
                DomDir::Inbound => PaymentDirection::Inbound as i32,
                DomDir::Outbound => PaymentDirection::Outbound as i32,
            },
            party_id: p.party_id,
            bank_account_id: p.bank_account_id,
            amount_minor: p.amount_minor,
            currency: p.currency,
            reference: p.reference,
            status: p.status,
        }))
    }

    async fn list_payments(
        &self,
        _request: Request<ListPaymentsRequest>,
    ) -> Result<Response<ListPaymentsResponse>, Status> {
        let payments = self
            .repo
            .list_payments()
            .await
            .map_err(pay_err)?
            .into_iter()
            .map(|p| Payment {
                id: p.id,
                direction: match p.direction {
                    DomDir::Inbound => PaymentDirection::Inbound as i32,
                    DomDir::Outbound => PaymentDirection::Outbound as i32,
                },
                party_id: p.party_id,
                bank_account_id: p.bank_account_id,
                amount_minor: p.amount_minor,
                currency: p.currency,
                reference: p.reference,
                status: p.status,
            })
            .collect();
        Ok(Response::new(ListPaymentsResponse { payments }))
    }

    async fn create_allocation(
        &self,
        request: Request<CreateAllocationRequest>,
    ) -> Result<Response<PaymentAllocation>, Status> {
        let req = request.into_inner();
        let a = self
            .repo
            .create_allocation(req.payment_id, req.document_id, req.amount_minor)
            .await
            .map_err(pay_err)?;
        Ok(Response::new(PaymentAllocation {
            id: a.id,
            payment_id: a.payment_id,
            document_id: a.document_id,
            amount_minor: a.amount_minor,
        }))
    }

    async fn list_allocations(
        &self,
        request: Request<ListAllocationsRequest>,
    ) -> Result<Response<ListAllocationsResponse>, Status> {
        let allocations = self
            .repo
            .list_allocations(&request.into_inner().payment_id)
            .await
            .map_err(pay_err)?
            .into_iter()
            .map(|a| PaymentAllocation {
                id: a.id,
                payment_id: a.payment_id,
                document_id: a.document_id,
                amount_minor: a.amount_minor,
            })
            .collect();
        Ok(Response::new(ListAllocationsResponse { allocations }))
    }
}

#[derive(Clone)]
pub struct InventorySvc {
    pub repo: Arc<dyn InventoryRepository>,
}

#[tonic::async_trait]
impl InventoryService for InventorySvc {
    async fn create_warehouse(
        &self,
        request: Request<CreateWarehouseRequest>,
    ) -> Result<Response<Warehouse>, Status> {
        let req = request.into_inner();
        let w = self
            .repo
            .create_warehouse(req.code, req.name)
            .await
            .map_err(inv_err)?;
        Ok(Response::new(Warehouse {
            id: w.id,
            code: w.code,
            name: w.name,
            active: w.active,
        }))
    }

    async fn list_warehouses(
        &self,
        _request: Request<ListWarehousesRequest>,
    ) -> Result<Response<ListWarehousesResponse>, Status> {
        let warehouses = self
            .repo
            .list_warehouses()
            .await
            .map_err(inv_err)?
            .into_iter()
            .map(|w| Warehouse {
                id: w.id,
                code: w.code,
                name: w.name,
                active: w.active,
            })
            .collect();
        Ok(Response::new(ListWarehousesResponse { warehouses }))
    }

    async fn list_stock_levels(
        &self,
        request: Request<ListStockLevelsRequest>,
    ) -> Result<Response<ListStockLevelsResponse>, Status> {
        let levels = self
            .repo
            .list_stock_levels(request.into_inner().warehouse_id)
            .await
            .map_err(inv_err)?
            .into_iter()
            .map(|l| StockLevel {
                id: l.id,
                warehouse_id: l.warehouse_id,
                product_id: l.product_id,
                qty_on_hand: l.qty_on_hand,
                qty_reserved: l.qty_reserved,
            })
            .collect();
        Ok(Response::new(ListStockLevelsResponse { levels }))
    }

    async fn create_stock_move(
        &self,
        request: Request<CreateStockMoveRequest>,
    ) -> Result<Response<StockMove>, Status> {
        let req = request.into_inner();
        let m = self
            .repo
            .create_stock_move(
                req.product_id,
                req.qty,
                req.from_warehouse_id,
                req.to_warehouse_id,
            )
            .await
            .map_err(inv_err)?;
        Ok(Response::new(StockMove {
            id: m.id,
            product_id: m.product_id,
            qty: m.qty,
            from_warehouse_id: m.from_warehouse_id,
            to_warehouse_id: m.to_warehouse_id,
            state: m.state,
        }))
    }

    async fn list_stock_moves(
        &self,
        _request: Request<ListStockMovesRequest>,
    ) -> Result<Response<ListStockMovesResponse>, Status> {
        let moves = self
            .repo
            .list_stock_moves()
            .await
            .map_err(inv_err)?
            .into_iter()
            .map(|m| StockMove {
                id: m.id,
                product_id: m.product_id,
                qty: m.qty,
                from_warehouse_id: m.from_warehouse_id,
                to_warehouse_id: m.to_warehouse_id,
                state: m.state,
            })
            .collect();
        Ok(Response::new(ListStockMovesResponse { moves }))
    }
}

#[derive(Clone)]
pub struct ModuleSvc {
    pub store: Arc<dyn ModuleStore>,
}

#[tonic::async_trait]
impl ModuleService for ModuleSvc {
    async fn list_modules(
        &self,
        _request: Request<ListModulesRequest>,
    ) -> Result<Response<ListModulesResponse>, Status> {
        let modules = self
            .store
            .list_modules()
            .await
            .map_err(auth_err)?
            .into_iter()
            .map(module_to_proto)
            .collect();
        Ok(Response::new(ListModulesResponse { modules }))
    }

    async fn set_module_enabled(
        &self,
        request: Request<SetModuleEnabledRequest>,
    ) -> Result<Response<ProtoModule>, Status> {
        let req = request.into_inner();
        let m = self
            .store
            .set_module_enabled(&req.id, req.enabled)
            .await
            .map_err(auth_err)?;
        Ok(Response::new(module_to_proto(m)))
    }
}

fn module_to_proto(m: ModuleInfo) -> ProtoModule {
    ProtoModule {
        id: m.id,
        name: m.name,
        enabled: m.enabled,
        always_on: m.always_on,
    }
}

#[derive(Clone)]
pub struct AuthSvc {
    pub repo: Arc<dyn AuthRepository>,
}

#[tonic::async_trait]
impl AuthService for AuthSvc {
    async fn list_users(
        &self,
        _request: Request<ListUsersRequest>,
    ) -> Result<Response<ListUsersResponse>, Status> {
        let users = self
            .repo
            .list_users()
            .await
            .map_err(auth_err)?
            .into_iter()
            .map(|u| UserInfo {
                id: u.id,
                login: u.login,
                display_name: u.display_name,
                active: u.active,
            })
            .collect();
        Ok(Response::new(ListUsersResponse { users }))
    }

    async fn list_roles(
        &self,
        _request: Request<ListRolesRequest>,
    ) -> Result<Response<ListRolesResponse>, Status> {
        let roles = self
            .repo
            .list_roles()
            .await
            .map_err(auth_err)?
            .into_iter()
            .map(|r| RoleInfo {
                id: r.id,
                name: r.name,
                description: r.description,
            })
            .collect();
        Ok(Response::new(ListRolesResponse { roles }))
    }

    async fn list_permissions(
        &self,
        _request: Request<ListPermissionsRequest>,
    ) -> Result<Response<ListPermissionsResponse>, Status> {
        let permissions = self
            .repo
            .list_permissions()
            .await
            .map_err(auth_err)?
            .into_iter()
            .map(|p| PermissionInfo {
                id: p.id,
                resource: p.resource,
                action: p.action,
            })
            .collect();
        Ok(Response::new(ListPermissionsResponse { permissions }))
    }

    async fn create_user(
        &self,
        request: Request<CreateUserRequest>,
    ) -> Result<Response<UserInfo>, Status> {
        let req = request.into_inner();
        let u = self
            .repo
            .create_user(req.login, req.display_name, req.password)
            .await
            .map_err(auth_err)?;
        Ok(Response::new(UserInfo {
            id: u.id,
            login: u.login,
            display_name: u.display_name,
            active: u.active,
        }))
    }
}
