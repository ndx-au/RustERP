//! PostgreSQL storage for RustERP via sqlx.
//!
//! Provides a tuned [`PgPool`](DbPool), schema migrations, and a minimal
//! [`Storage`] health-check trait.

mod pool;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

pub use pool::{connect, log_server_version, DbPool};

/// Minimal storage trait for health checks.
#[async_trait]
pub trait Storage: Send + Sync {
    /// Lightweight health check: `SELECT 1` on the live connection pool.
    async fn ping(&self) -> Result<(), StorageError>;
}

/// Storage-layer errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageError {
    message: String,
}

impl StorageError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for StorageError {}

/// PostgreSQL storage backend backed by a shared sqlx pool.
#[derive(Clone)]
pub struct PostgresStorage {
    pool: PgPool,
}

impl PostgresStorage {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl Storage for PostgresStorage {
    async fn ping(&self) -> Result<(), StorageError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|e| StorageError::new(format!("postgres ping failed: {e}")))
    }
}

/// Configuration for the PostgreSQL storage backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// PostgreSQL connection URI (required at runtime).
    #[serde(default)]
    pub postgres_url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,
    #[serde(default = "default_acquire_timeout_secs")]
    pub acquire_timeout_secs: u64,
    #[serde(default = "default_idle_timeout_secs")]
    pub idle_timeout_secs: u64,
    #[serde(default = "default_max_lifetime_secs")]
    pub max_lifetime_secs: u64,
}

fn default_max_connections() -> u32 {
    20
}

fn default_min_connections() -> u32 {
    2
}

fn default_acquire_timeout_secs() -> u64 {
    3
}

fn default_idle_timeout_secs() -> u64 {
    600
}

fn default_max_lifetime_secs() -> u64 {
    1800
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            postgres_url: String::new(),
            max_connections: default_max_connections(),
            min_connections: default_min_connections(),
            acquire_timeout_secs: default_acquire_timeout_secs(),
            idle_timeout_secs: default_idle_timeout_secs(),
            max_lifetime_secs: default_max_lifetime_secs(),
        }
    }
}

impl StorageConfig {
    /// Return an error when no connection URI is configured.
    pub fn require_postgres_url(&self) -> Result<(), StorageError> {
        if self.postgres_url.trim().is_empty() {
            return Err(StorageError::new(
                "postgres_url is required (set RUSTERP_POSTGRES_URL or [storage].postgres_url)",
            ));
        }
        Ok(())
    }
}

/// Run schema migrations against the pool.
pub async fn run_migrations(pool: &PgPool) -> Result<(), StorageError> {
    let migrations_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let migrator = sqlx::migrate::Migrator::new(migrations_dir.as_path())
        .await
        .map_err(|e| StorageError::new(format!("migration load failed: {e}")))?;
    migrator
        .run(pool)
        .await
        .map_err(|e| StorageError::new(format!("migration failed: {e}")))
}

/// Connect to PostgreSQL, run migrations, and return storage + pool.
pub async fn bootstrap(cfg: &StorageConfig) -> Result<(PostgresStorage, PgPool), StorageError> {
    cfg.require_postgres_url()?;
    let pool = connect(cfg).await?;
    run_migrations(&pool).await?;
    log_server_version(&pool).await;
    Ok((PostgresStorage::new(pool.clone()), pool))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_config_defaults() {
        let cfg = StorageConfig::default();
        assert_eq!(cfg.max_connections, 20);
        assert_eq!(cfg.min_connections, 2);
        assert_eq!(cfg.acquire_timeout_secs, 3);
    }

    #[test]
    fn require_postgres_url_fails_when_empty() {
        let cfg = StorageConfig::default();
        assert!(cfg.require_postgres_url().is_err());
    }

    #[tokio::test]
    async fn pg_integration_skips_without_url() {
        let url = std::env::var("RUSTERP_POSTGRES_URL")
            .ok()
            .unwrap_or_default();
        if url.is_empty() {
            eprintln!("skipping Postgres integration test: RUSTERP_POSTGRES_URL not set");
            return;
        }
        let cfg = StorageConfig {
            postgres_url: url,
            ..StorageConfig::default()
        };
        let (storage, _pool) = bootstrap(&cfg).await.expect("bootstrap");
        storage.ping().await.expect("ping");
    }
}
