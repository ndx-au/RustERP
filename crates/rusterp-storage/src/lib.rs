//! Storage abstraction for RustERP.
//!
//! Intended backends:
//! - **SQLite** — default / recommended for most single-tenant instances
//! - **Litestream** — recommended ops path for SQLite replication/backup (external process;
//!   not a compile-time dependency of this crate)
//! - **PostgreSQL** — alternative backend for deployments that prefer it
//!
//! This crate currently exposes traits and **stubs only**. No live database drivers.

use std::fmt;

/// Backend kind selected for a tenant instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// Local SQLite database file (pair with Litestream in ops for replication).
    Sqlite,
    /// PostgreSQL server.
    Postgres,
}

impl fmt::Display for BackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendKind::Sqlite => write!(f, "sqlite"),
            BackendKind::Postgres => write!(f, "postgres"),
        }
    }
}

/// Minimal storage surface. Expanded when the first domain crate needs real I/O.
pub trait Storage: Send + Sync {
    /// Which backend this handle targets.
    fn backend_kind(&self) -> BackendKind;

    /// Lightweight health/readiness check (stub-friendly).
    fn ping(&self) -> Result<(), StorageError>;
}

/// Storage-layer error placeholder.
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

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for StorageError {}

/// SQLite stub backend.
///
/// Production deployments should run [Litestream](https://litestream.io/) (or equivalent)
/// alongside the SQLite file for continuous replication — that is an operational concern,
/// not wired into this type yet.
#[derive(Debug, Default)]
pub struct SqliteStorage {
    /// Path that would hold the database file once drivers are wired.
    pub path: String,
}

impl SqliteStorage {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}

impl Storage for SqliteStorage {
    fn backend_kind(&self) -> BackendKind {
        BackendKind::Sqlite
    }

    fn ping(&self) -> Result<(), StorageError> {
        // Stub: always healthy. Real open/pragma checks come later.
        Ok(())
    }
}

/// PostgreSQL stub backend.
#[derive(Debug, Default)]
pub struct PostgresStorage {
    /// Connection URI placeholder (never opened in Phase 0).
    pub connection_uri: String,
}

impl PostgresStorage {
    pub fn new(connection_uri: impl Into<String>) -> Self {
        Self {
            connection_uri: connection_uri.into(),
        }
    }
}

impl Storage for PostgresStorage {
    fn backend_kind(&self) -> BackendKind {
        BackendKind::Postgres
    }

    fn ping(&self) -> Result<(), StorageError> {
        // Stub: always healthy. Real connect/SELECT 1 comes later.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_stub_reports_backend_and_pings() {
        let db = SqliteStorage::new("rusterp.db");
        assert_eq!(db.backend_kind(), BackendKind::Sqlite);
        assert!(db.ping().is_ok());
    }

    #[test]
    fn postgres_stub_reports_backend_and_pings() {
        let db = PostgresStorage::new("postgres://localhost/rusterp");
        assert_eq!(db.backend_kind(), BackendKind::Postgres);
        assert!(db.ping().is_ok());
    }
}
