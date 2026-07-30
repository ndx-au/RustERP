//! Inventory domain (toggleable via core.modules).

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Warehouse {
    pub id: String,
    pub code: String,
    pub name: String,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct StockLevel {
    pub id: String,
    pub warehouse_id: String,
    pub product_id: String,
    pub qty_on_hand: String,
    pub qty_reserved: String,
}

#[derive(Debug, Clone)]
pub struct StockMove {
    pub id: String,
    pub product_id: String,
    pub qty: String,
    pub from_warehouse_id: Option<String>,
    pub to_warehouse_id: Option<String>,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryError {
    NotFound(String),
    Invalid(String),
    Disabled,
}

impl std::fmt::Display for InventoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(m) | Self::Invalid(m) => write!(f, "{m}"),
            Self::Disabled => write!(f, "inventory module is not enabled"),
        }
    }
}
impl std::error::Error for InventoryError {}

#[async_trait]
pub trait InventoryRepository: Send + Sync {
    async fn is_enabled(&self) -> Result<bool, InventoryError>;
    async fn create_warehouse(&self, code: String, name: String) -> Result<Warehouse, InventoryError>;
    async fn list_warehouses(&self) -> Result<Vec<Warehouse>, InventoryError>;
    async fn update_warehouse(
        &self,
        id: &str,
        code: Option<String>,
        name: Option<String>,
        active: Option<bool>,
    ) -> Result<Warehouse, InventoryError>;
    async fn list_stock_levels(
        &self,
        warehouse_id: Option<String>,
    ) -> Result<Vec<StockLevel>, InventoryError>;
    async fn create_stock_move(
        &self,
        product_id: String,
        qty: String,
        from_warehouse_id: Option<String>,
        to_warehouse_id: Option<String>,
    ) -> Result<StockMove, InventoryError>;
    async fn list_stock_moves(&self) -> Result<Vec<StockMove>, InventoryError>;
}

pub struct PostgresInventoryRepository {
    pool: PgPool,
}

impl PostgresInventoryRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn require_enabled(&self) -> Result<(), InventoryError> {
        if self.is_enabled().await? {
            Ok(())
        } else {
            Err(InventoryError::Disabled)
        }
    }
}

#[async_trait]
impl InventoryRepository for PostgresInventoryRepository {
    async fn is_enabled(&self) -> Result<bool, InventoryError> {
        let enabled = sqlx::query_scalar::<_, bool>(
            "SELECT enabled FROM core.modules WHERE id = 'inventory'",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| InventoryError::Invalid(e.to_string()))?
        .unwrap_or(false);
        Ok(enabled)
    }

    async fn create_warehouse(
        &self,
        code: String,
        name: String,
    ) -> Result<Warehouse, InventoryError> {
        self.require_enabled().await?;
        let code = code.trim().to_string();
        let name = name.trim().to_string();
        if code.is_empty() || name.is_empty() {
            return Err(InventoryError::Invalid("code and name required".into()));
        }
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO inventory.warehouses (id, code, name) VALUES ($1::uuid, $2, $3)",
        )
        .bind(&id)
        .bind(&code)
        .bind(&name)
        .execute(&self.pool)
        .await
        .map_err(|e| InventoryError::Invalid(e.to_string()))?;
        Ok(Warehouse {
            id,
            code,
            name,
            active: true,
        })
    }

    async fn list_warehouses(&self) -> Result<Vec<Warehouse>, InventoryError> {
        let rows = sqlx::query_as::<_, (String, String, String, bool)>(
            "SELECT id::text, code, name, active FROM inventory.warehouses ORDER BY code",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| InventoryError::Invalid(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| Warehouse {
                id: r.0,
                code: r.1,
                name: r.2,
                active: r.3,
            })
            .collect())
    }

    async fn update_warehouse(
        &self,
        id: &str,
        code: Option<String>,
        name: Option<String>,
        active: Option<bool>,
    ) -> Result<Warehouse, InventoryError> {
        self.require_enabled().await?;
        let current = sqlx::query_as::<_, (String, String, String, bool)>(
            "SELECT id::text, code, name, active FROM inventory.warehouses WHERE id = $1::uuid",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| InventoryError::Invalid(e.to_string()))?
        .ok_or_else(|| InventoryError::NotFound(format!("warehouse {id}")))?;

        let code = match code {
            Some(c) => {
                let c = c.trim().to_string();
                if c.is_empty() {
                    return Err(InventoryError::Invalid("code required".into()));
                }
                c
            }
            None => current.1,
        };
        let name = match name {
            Some(n) => {
                let n = n.trim().to_string();
                if n.is_empty() {
                    return Err(InventoryError::Invalid("name required".into()));
                }
                n
            }
            None => current.2,
        };
        let active = active.unwrap_or(current.3);

        sqlx::query(
            "UPDATE inventory.warehouses SET code = $1, name = $2, active = $3,
             row_version = row_version + 1 WHERE id = $4::uuid",
        )
        .bind(&code)
        .bind(&name)
        .bind(active)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| InventoryError::Invalid(e.to_string()))?;

        Ok(Warehouse {
            id: current.0,
            code,
            name,
            active,
        })
    }

    async fn list_stock_levels(
        &self,
        warehouse_id: Option<String>,
    ) -> Result<Vec<StockLevel>, InventoryError> {
        let rows = if let Some(wid) = warehouse_id {
            sqlx::query_as::<_, (String, String, String, String, String)>(
                "SELECT id::text, warehouse_id::text, product_id::text, qty_on_hand::text, qty_reserved::text
                 FROM inventory.stock_levels WHERE warehouse_id = $1::uuid ORDER BY product_id",
            )
            .bind(wid)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, (String, String, String, String, String)>(
                "SELECT id::text, warehouse_id::text, product_id::text, qty_on_hand::text, qty_reserved::text
                 FROM inventory.stock_levels ORDER BY warehouse_id, product_id",
            )
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|e| InventoryError::Invalid(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| StockLevel {
                id: r.0,
                warehouse_id: r.1,
                product_id: r.2,
                qty_on_hand: r.3,
                qty_reserved: r.4,
            })
            .collect())
    }

    async fn create_stock_move(
        &self,
        product_id: String,
        qty: String,
        from_warehouse_id: Option<String>,
        to_warehouse_id: Option<String>,
    ) -> Result<StockMove, InventoryError> {
        self.require_enabled().await?;
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO inventory.stock_moves
             (id, product_id, qty, from_warehouse_id, to_warehouse_id, state)
             VALUES ($1::uuid, $2::uuid, $3::numeric, $4::uuid, $5::uuid, 'draft')",
        )
        .bind(&id)
        .bind(&product_id)
        .bind(&qty)
        .bind(&from_warehouse_id)
        .bind(&to_warehouse_id)
        .execute(&self.pool)
        .await
        .map_err(|e| InventoryError::Invalid(e.to_string()))?;
        Ok(StockMove {
            id,
            product_id,
            qty,
            from_warehouse_id,
            to_warehouse_id,
            state: "draft".into(),
        })
    }

    async fn list_stock_moves(&self) -> Result<Vec<StockMove>, InventoryError> {
        let rows = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>, String)>(
            "SELECT id::text, product_id::text, qty::text, from_warehouse_id::text, to_warehouse_id::text, state::text
             FROM inventory.stock_moves ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| InventoryError::Invalid(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| StockMove {
                id: r.0,
                product_id: r.1,
                qty: r.2,
                from_warehouse_id: r.3,
                to_warehouse_id: r.4,
                state: r.5,
            })
            .collect())
    }
}
