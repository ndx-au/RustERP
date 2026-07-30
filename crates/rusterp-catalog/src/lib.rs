//! Catalog domain: products and categories (PostgreSQL).

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductType {
    Stock,
    Service,
    Consumable,
}

#[derive(Debug, Clone)]
pub struct Product {
    pub id: String,
    pub sku: String,
    pub name: String,
    pub product_type: ProductType,
    pub category_id: Option<String>,
    pub uom_id: String,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct NewProduct {
    pub sku: String,
    pub name: String,
    pub product_type: ProductType,
    pub category_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewCategory {
    pub name: String,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ProductUpdate {
    pub sku: Option<String>,
    pub name: Option<String>,
    pub product_type: Option<ProductType>,
    pub category_id: Option<Option<String>>,
    pub active: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct CategoryUpdate {
    pub name: Option<String>,
    pub parent_id: Option<Option<String>>,
    pub active: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogError {
    NotFound(String),
    Invalid(String),
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(m) | Self::Invalid(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for CatalogError {}

#[async_trait]
pub trait CatalogRepository: Send + Sync {
    async fn create_product(&self, new: NewProduct) -> Result<Product, CatalogError>;
    async fn list_products(&self) -> Result<Vec<Product>, CatalogError>;
    async fn update_product(&self, id: &str, update: ProductUpdate) -> Result<Product, CatalogError>;
    async fn create_category(&self, new: NewCategory) -> Result<Category, CatalogError>;
    async fn list_categories(&self) -> Result<Vec<Category>, CatalogError>;
    async fn update_category(
        &self,
        id: &str,
        update: CategoryUpdate,
    ) -> Result<Category, CatalogError>;
}

pub struct PostgresCatalogRepository {
    pool: PgPool,
}

impl PostgresCatalogRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn ensure_ea_uom(&self) -> Result<String, CatalogError> {
        if let Some(id) = sqlx::query_scalar::<_, String>(
            "SELECT id::text FROM catalog.units_of_measure WHERE code = 'EA'",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CatalogError::Invalid(e.to_string()))?
        {
            return Ok(id);
        }
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO catalog.units_of_measure (id, code, name) VALUES ($1::uuid, 'EA', 'Each')",
        )
        .bind(&id)
        .execute(&self.pool)
        .await
        .map_err(|e| CatalogError::Invalid(e.to_string()))?;
        Ok(id)
    }

    fn type_to_db(t: ProductType) -> &'static str {
        match t {
            ProductType::Stock => "stock",
            ProductType::Service => "service",
            ProductType::Consumable => "consumable",
        }
    }

    fn type_from_db(s: &str) -> ProductType {
        match s {
            "service" => ProductType::Service,
            "consumable" => ProductType::Consumable,
            _ => ProductType::Stock,
        }
    }
}

#[async_trait]
impl CatalogRepository for PostgresCatalogRepository {
    async fn create_product(&self, new: NewProduct) -> Result<Product, CatalogError> {
        let sku = new.sku.trim().to_string();
        let name = new.name.trim().to_string();
        if sku.is_empty() || name.is_empty() {
            return Err(CatalogError::Invalid("sku and name required".into()));
        }
        let uom_id = self.ensure_ea_uom().await?;
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO catalog.products (id, type, sku, name, category_id, uom_id)
             VALUES ($1::uuid, $2::catalog.product_type, $3, $4, $5::uuid, $6::uuid)",
        )
        .bind(&id)
        .bind(Self::type_to_db(new.product_type))
        .bind(&sku)
        .bind(&name)
        .bind(&new.category_id)
        .bind(&uom_id)
        .execute(&self.pool)
        .await
        .map_err(|e| CatalogError::Invalid(format!("insert product: {e}")))?;
        Ok(Product {
            id,
            sku,
            name,
            product_type: new.product_type,
            category_id: new.category_id,
            uom_id,
            active: true,
        })
    }

    async fn list_products(&self) -> Result<Vec<Product>, CatalogError> {
        let rows = sqlx::query_as::<_, (String, String, String, String, Option<String>, String, bool)>(
            "SELECT id::text, sku, name, type::text, category_id::text, uom_id::text, active
             FROM catalog.products ORDER BY name, sku",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CatalogError::Invalid(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| Product {
                id: r.0,
                sku: r.1,
                name: r.2,
                product_type: Self::type_from_db(&r.3),
                category_id: r.4,
                uom_id: r.5,
                active: r.6,
            })
            .collect())
    }

    async fn update_product(&self, id: &str, update: ProductUpdate) -> Result<Product, CatalogError> {
        let current = sqlx::query_as::<_, (String, String, String, String, Option<String>, String, bool)>(
            "SELECT id::text, sku, name, type::text, category_id::text, uom_id::text, active
             FROM catalog.products WHERE id = $1::uuid",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CatalogError::Invalid(e.to_string()))?
        .ok_or_else(|| CatalogError::NotFound(format!("product {id}")))?;

        let sku = match update.sku {
            Some(s) => {
                let s = s.trim().to_string();
                if s.is_empty() {
                    return Err(CatalogError::Invalid("sku required".into()));
                }
                s
            }
            None => current.1,
        };
        let name = match update.name {
            Some(n) => {
                let n = n.trim().to_string();
                if n.is_empty() {
                    return Err(CatalogError::Invalid("name required".into()));
                }
                n
            }
            None => current.2,
        };
        let product_type = update
            .product_type
            .unwrap_or_else(|| Self::type_from_db(&current.3));
        let category_id = update.category_id.unwrap_or(current.4);
        let active = update.active.unwrap_or(current.6);

        sqlx::query(
            "UPDATE catalog.products SET sku = $1, name = $2, type = $3::catalog.product_type,
             category_id = $4::uuid, active = $5, row_version = row_version + 1
             WHERE id = $6::uuid",
        )
        .bind(&sku)
        .bind(&name)
        .bind(Self::type_to_db(product_type))
        .bind(&category_id)
        .bind(active)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| CatalogError::Invalid(e.to_string()))?;

        Ok(Product {
            id: current.0,
            sku,
            name,
            product_type,
            category_id,
            uom_id: current.5,
            active,
        })
    }

    async fn create_category(&self, new: NewCategory) -> Result<Category, CatalogError> {
        let name = new.name.trim().to_string();
        if name.is_empty() {
            return Err(CatalogError::Invalid("name required".into()));
        }
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO catalog.product_categories (id, name, parent_id)
             VALUES ($1::uuid, $2, $3::uuid)",
        )
        .bind(&id)
        .bind(&name)
        .bind(&new.parent_id)
        .execute(&self.pool)
        .await
        .map_err(|e| CatalogError::Invalid(e.to_string()))?;
        Ok(Category {
            id,
            name,
            parent_id: new.parent_id,
            active: true,
        })
    }

    async fn list_categories(&self) -> Result<Vec<Category>, CatalogError> {
        let rows = sqlx::query_as::<_, (String, String, Option<String>, bool)>(
            "SELECT id::text, name, parent_id::text, active FROM catalog.product_categories ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CatalogError::Invalid(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| Category {
                id: r.0,
                name: r.1,
                parent_id: r.2,
                active: r.3,
            })
            .collect())
    }

    async fn update_category(
        &self,
        id: &str,
        update: CategoryUpdate,
    ) -> Result<Category, CatalogError> {
        let current = sqlx::query_as::<_, (String, String, Option<String>, bool)>(
            "SELECT id::text, name, parent_id::text, active FROM catalog.product_categories
             WHERE id = $1::uuid",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CatalogError::Invalid(e.to_string()))?
        .ok_or_else(|| CatalogError::NotFound(format!("category {id}")))?;

        let name = match update.name {
            Some(n) => {
                let n = n.trim().to_string();
                if n.is_empty() {
                    return Err(CatalogError::Invalid("name required".into()));
                }
                n
            }
            None => current.1,
        };
        let parent_id = update.parent_id.unwrap_or(current.2);
        let active = update.active.unwrap_or(current.3);

        sqlx::query(
            "UPDATE catalog.product_categories SET name = $1, parent_id = $2::uuid, active = $3,
             row_version = row_version + 1 WHERE id = $4::uuid",
        )
        .bind(&name)
        .bind(&parent_id)
        .bind(active)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| CatalogError::Invalid(e.to_string()))?;

        Ok(Category {
            id: current.0,
            name,
            parent_id,
            active,
        })
    }
}
