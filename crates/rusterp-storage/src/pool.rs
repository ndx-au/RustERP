//! PostgreSQL connection pool via sqlx.

use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::{StorageConfig, StorageError};

/// Shared PostgreSQL connection pool handle.
pub type DbPool = PgPool;

/// Build a tuned connection pool from storage configuration.
pub async fn connect(cfg: &StorageConfig) -> Result<PgPool, StorageError> {
    PgPoolOptions::new()
        .max_connections(cfg.max_connections)
        .min_connections(cfg.min_connections)
        .acquire_timeout(Duration::from_secs(cfg.acquire_timeout_secs))
        .idle_timeout(Duration::from_secs(cfg.idle_timeout_secs))
        .max_lifetime(Duration::from_secs(cfg.max_lifetime_secs))
        .connect(&cfg.postgres_url)
        .await
        .map_err(|e| StorageError::new(format!("postgres connect failed: {e}")))
}

/// Log the PostgreSQL server version after a successful connect.
pub async fn log_server_version(pool: &PgPool) {
    match sqlx::query_scalar::<_, String>("SHOW server_version")
        .fetch_one(pool)
        .await
    {
        Ok(version) => tracing::info!(%version, "postgresql server version"),
        Err(e) => tracing::warn!("could not read postgresql server version: {e}"),
    }
}
