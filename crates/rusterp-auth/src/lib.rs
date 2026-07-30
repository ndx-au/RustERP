//! Auth RBAC + core.modules store (PostgreSQL).

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct UserInfo {
    pub id: String,
    pub login: String,
    pub display_name: String,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct RoleInfo {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct PermissionInfo {
    pub id: String,
    pub resource: String,
    pub action: String,
}

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub always_on: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    NotFound(String),
    Invalid(String),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(m) | Self::Invalid(m) => write!(f, "{m}"),
        }
    }
}
impl std::error::Error for AuthError {}

/// Interim password hashing for local/dev only — NOT production-grade.
fn hex_encode(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn interim_password_hash(password: &str) -> String {
    let digest = Sha256::digest(password.as_bytes());
    format!("dev:{}", hex_encode(digest))
}

#[async_trait]
pub trait AuthRepository: Send + Sync {
    async fn list_users(&self) -> Result<Vec<UserInfo>, AuthError>;
    async fn list_roles(&self) -> Result<Vec<RoleInfo>, AuthError>;
    async fn list_permissions(&self) -> Result<Vec<PermissionInfo>, AuthError>;
    async fn create_user(
        &self,
        login: String,
        display_name: String,
        password: String,
    ) -> Result<UserInfo, AuthError>;
    async fn user_login_active(&self, login: &str) -> Result<bool, AuthError>;
}

#[async_trait]
pub trait ModuleStore: Send + Sync {
    async fn list_modules(&self) -> Result<Vec<ModuleInfo>, AuthError>;
    async fn set_module_enabled(&self, id: &str, enabled: bool) -> Result<ModuleInfo, AuthError>;
}

pub struct PostgresAuthRepository {
    pool: PgPool,
}

impl PostgresAuthRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuthRepository for PostgresAuthRepository {
    async fn list_users(&self) -> Result<Vec<UserInfo>, AuthError> {
        let rows = sqlx::query_as::<_, (String, String, String, bool)>(
            "SELECT id::text, login::text, display_name, active FROM auth.users ORDER BY login",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AuthError::Invalid(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| UserInfo {
                id: r.0,
                login: r.1,
                display_name: r.2,
                active: r.3,
            })
            .collect())
    }

    async fn list_roles(&self) -> Result<Vec<RoleInfo>, AuthError> {
        let rows = sqlx::query_as::<_, (String, String, Option<String>)>(
            "SELECT id::text, name, description FROM auth.roles ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AuthError::Invalid(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| RoleInfo {
                id: r.0,
                name: r.1,
                description: r.2.unwrap_or_default(),
            })
            .collect())
    }

    async fn list_permissions(&self) -> Result<Vec<PermissionInfo>, AuthError> {
        let rows = sqlx::query_as::<_, (String, String, String)>(
            "SELECT id::text, resource, action FROM auth.permissions ORDER BY resource, action",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AuthError::Invalid(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| PermissionInfo {
                id: r.0,
                resource: r.1,
                action: r.2,
            })
            .collect())
    }

    async fn create_user(
        &self,
        login: String,
        display_name: String,
        password: String,
    ) -> Result<UserInfo, AuthError> {
        let login = login.trim().to_lowercase();
        let display_name = display_name.trim().to_string();
        if login.is_empty() || display_name.is_empty() || password.is_empty() {
            return Err(AuthError::Invalid(
                "login, display_name, and password required".into(),
            ));
        }
        let id = Uuid::new_v4().to_string();
        let hash = interim_password_hash(&password);
        sqlx::query(
            "INSERT INTO auth.users (id, login, display_name, password_hash)
             VALUES ($1::uuid, $2, $3, $4)",
        )
        .bind(&id)
        .bind(&login)
        .bind(&display_name)
        .bind(&hash)
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::Invalid(e.to_string()))?;
        Ok(UserInfo {
            id,
            login,
            display_name,
            active: true,
        })
    }

    async fn user_login_active(&self, login: &str) -> Result<bool, AuthError> {
        let active = sqlx::query_scalar::<_, bool>(
            "SELECT active FROM auth.users WHERE login = $1 AND active = TRUE",
        )
        .bind(login.trim().to_lowercase())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::Invalid(e.to_string()))?;
        Ok(active.unwrap_or(false))
    }
}

#[async_trait]
impl ModuleStore for PostgresAuthRepository {
    async fn list_modules(&self) -> Result<Vec<ModuleInfo>, AuthError> {
        let rows = sqlx::query_as::<_, (String, String, bool, bool)>(
            "SELECT id, name, enabled, always_on FROM core.modules ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AuthError::Invalid(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| ModuleInfo {
                id: r.0,
                name: r.1,
                enabled: r.2,
                always_on: r.3,
            })
            .collect())
    }

    async fn set_module_enabled(&self, id: &str, enabled: bool) -> Result<ModuleInfo, AuthError> {
        let current = sqlx::query_as::<_, (String, String, bool, bool)>(
            "SELECT id, name, enabled, always_on FROM core.modules WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::Invalid(e.to_string()))?
        .ok_or_else(|| AuthError::NotFound(format!("module {id}")))?;
        if current.3 && !enabled {
            return Err(AuthError::Invalid(format!(
                "module {id} is always_on and cannot be disabled"
            )));
        }
        sqlx::query("UPDATE core.modules SET enabled = $1 WHERE id = $2")
            .bind(enabled)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| AuthError::Invalid(e.to_string()))?;
        Ok(ModuleInfo {
            id: current.0,
            name: current.1,
            enabled,
            always_on: current.3,
        })
    }
}
