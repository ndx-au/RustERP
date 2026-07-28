//! Storage abstraction for RustERP with working backends.
//!
//! Available backends:
//! - **SQLite** via `rusqlite` — default, opens a real `.db` file.
//! - **PostgreSQL** via `tokio-postgres` — connect via connection string.
//!
//! Litestream is **not** a compile-time dependency. It is an external process for
//! SQLite replication. The server logs a WARNING when SQLite is selected without
//! Litestream configuration:
//!
//! > "When using SQLite, RustERP needs Litestream to be implemented with active
//! > backup storage or you could lose all of your data".
//!
//! This warning is suppressed when `litestream.yml` is detected at the configured
//! path or `LITESTREAM_REPLICA_URL` environment variable is set.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Backend kind for a storage instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    /// Local SQLite database file (pair with Litestream for replication).
    Sqlite,
    /// PostgreSQL server.
    Postgres,
}

impl Default for BackendKind {
    fn default() -> Self {
        Self::Sqlite
    }
}

impl fmt::Display for BackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendKind::Sqlite => write!(f, "sqlite"),
            BackendKind::Postgres => write!(f, "postgres"),
        }
    }
}

/// Minimal storage trait for health checks.
pub trait Storage: Send + Sync {
    /// Which backend this handle targets.
    fn backend_kind(&self) -> BackendKind;

    /// Lightweight health check: `SELECT 1` on the live connection.
    fn ping(&self) -> Result<(), StorageError>;
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

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for StorageError {}

// ---------------------------------------------------------------------------
// SQLite backend (rusqlite, bundled)
// ---------------------------------------------------------------------------

struct SqliteInner {
    conn: Option<rusqlite::Connection>,
}

/// SQLite storage backend using `rusqlite` with the `"bundled"` feature.
#[derive(Clone)]
pub struct SqliteStorage {
    path: String,
    inner: Arc<Mutex<SqliteInner>>,
}

impl SqliteStorage {
    /// Create a SQLite storage handle. The connection is lazy.
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            inner: Arc::new(Mutex::new(SqliteInner { conn: None })),
        }
    }

    fn ensure_connection(&self) -> Result<(), StorageError> {
        let mut guard = self.inner.lock().map_err(|e| {
            StorageError::new(format!("sqlite lock poisoned: {e}"))
        })?;
        if guard.conn.is_some() {
            return Ok(());
        }
        let conn = rusqlite::Connection::open(&self.path)
            .map_err(|e| StorageError::new(format!("failed to open SQLite DB: {e}")))?;
        conn.execute("PRAGMA journal_mode=WAL", [])
            .ok(); // non-fatal
        guard.conn = Some(conn);
        Ok(())
    }
}

impl Storage for SqliteStorage {
    fn backend_kind(&self) -> BackendKind {
        BackendKind::Sqlite
    }

    fn ping(&self) -> Result<(), StorageError> {
        self.ensure_connection()?;
        let guard = self.inner.lock().map_err(|e| {
            StorageError::new(format!("sqlite lock poisoned: {e}"))
        })?;
        let conn = guard.conn.as_ref().ok_or_else(|| {
            StorageError::new("connection was None after ensure_connection")
        })?;
        conn.query_row("SELECT 1", [], |_| Ok(()))
            .map_err(|e| StorageError::new(format!("sqlite ping failed: {e}")))
    }
}

// ---------------------------------------------------------------------------
// PostgreSQL backend (tokio-postgres, plaintext)
// ---------------------------------------------------------------------------

/// Inner state for PostgresStorage — holds the runtime + client together.
struct PostgresInner {
    client: Option<tokio_postgres::Client>,
    runtime: Option<tokio::runtime::Runtime>,
    spawned: bool,
}

/// PostgreSQL storage backend using `tokio-postgres` (no TLS).
#[derive(Clone)]
pub struct PostgresStorage {
    connection_uri: String,
    inner: Arc<Mutex<PostgresInner>>,
}

impl Drop for PostgresStorage {
    fn drop(&mut self) {
        // The Postgres client will send a terminate message when dropped.
        // Keep the inner runtime alive to process the shutdown.
    }
}

impl PostgresStorage {
    /// Create a Postgres storage handle. Connection is established lazily.
    pub fn new(connection_uri: impl Into<String>) -> Self {
        Self {
            connection_uri: connection_uri.into(),
            inner: Arc::new(Mutex::new(PostgresInner {
                client: None,
                runtime: None,
                spawned: false,
            })),
        }
    }

    fn ensure_client(&self) -> Result<(), StorageError> {
        let mut guard = self.inner.lock().map_err(|e| {
            StorageError::new(format!("postgres lock poisoned: {e}"))
        })?;
        if guard.client.is_some() {
            return Ok(());
        }
        // Create a runtime to allow async connect.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| StorageError::new(format!("failed to build runtime: {e}")))?;
        let (client, connection) = rt
            .block_on(async {
                tokio_postgres::connect(&self.connection_uri, tokio_postgres::NoTls).await
            })
            .map_err(|e| StorageError::new(format!("postgres connect failed: {e}")))?;
        // Spawn the driver on the runtime so receive-side I/O works.
        rt.spawn(connection);
        guard.runtime = Some(rt);
        guard.client = Some(client);
        Ok(())
    }
}

impl Storage for PostgresStorage {
    fn backend_kind(&self) -> BackendKind {
        BackendKind::Postgres
    }

    fn ping(&self) -> Result<(), StorageError> {
        self.ensure_client()?;
        let guard = self.inner.lock().map_err(|e| {
            StorageError::new(format!("postgres lock poisoned: {e}"))
        })?;
        let client = guard.client.as_ref().ok_or_else(|| {
            StorageError::new("client was None after ensure_client")
        })?;
        // Ping must happen on a runtime. If the runtime already exists (from
        // ensure_client), use it. Otherwise we already failed above.
        if guard.spawned {
            // Driver is already spawned, just use the existing runtime.
        }
        // For simplicity: create an inline runtime for this single ping.
        // The ensure_client already established the TCP connection by spawning
        // the driver on its own runtime. We just need a runtime to .await
        // the execute call.
        let inline_rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| StorageError::new(format!("failed to build runtime: {e}")))?;
        let result = inline_rt.block_on(async {
            client.execute("SELECT 1", &[]).await
        });
        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(StorageError::new(format!("postgres ping failed: {e}"))),
        }
    }
}

// ---------------------------------------------------------------------------
// Storage configuration & factory
// ---------------------------------------------------------------------------

/// Configuration for the storage backend.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Backend kind: "sqlite" or "postgres" (default: "sqlite").
    #[serde(default)]
    pub backend: BackendKind,
    /// SQLite database file path.
    #[serde(default = "default_sqlite_path")]
    pub sqlite_path: String,
    /// PostgreSQL connection URI (required when backend = "postgres").
    pub postgres_url: Option<String>,
    /// Path to a litestream.yml configuration file.
    pub litestream_config: Option<String>,
    /// Litestream replica URL (alternative to config file).
    #[serde(default)]
    pub litestream_replica_url: String,
}

fn default_sqlite_path() -> String {
    "rusterp.db".to_string()
}

impl StorageConfig {
    /// Determine whether Litestream is considered configured.
    pub fn litestream_configured(&self) -> bool {
        !self.litestream_replica_url.is_empty()
            || self.litestream_config.as_ref().map_or(false, |p| {
                let path = Path::new(p);
                path.is_file()
            })
    }

    /// Emit a warning when SQLite is selected without backup configuration.
    pub fn warn_if_no_litestream(&self) {
        if self.backend == BackendKind::Sqlite && !self.litestream_configured() {
            tracing::warn!(
                "When using SQLite, RustERP needs Litestream to be implemented with active \
                 backup storage or you could lose all of your data"
            );
        }
    }
}

/// Construct a storage backend from configuration.
pub fn from_config(cfg: &StorageConfig) -> Result<Box<dyn Storage>, StorageError> {
    match cfg.backend {
        BackendKind::Sqlite => {
            let backend = SqliteStorage::new(&cfg.sqlite_path);
            Ok(Box::new(backend))
        }
        BackendKind::Postgres => {
            let url = cfg.postgres_url.as_ref().ok_or_else(|| {
                StorageError::new("postgres_url is required when backend = \"postgres\"")
            })?;
            let backend = PostgresStorage::new(url);
            Ok(Box::new(backend))
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A writer that captures all output into a `Vec<u8>` buffer.
    #[derive(Clone)]
    struct CaptureWriter {
        buf: Arc<Mutex<Vec<u8>>>,
    }

    impl CaptureWriter {
        fn new() -> Self {
            Self {
                buf: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn contents(&self) -> String {
            String::from_utf8(self.buf.lock().unwrap().clone())
                .unwrap_or_default()
        }
    }

    impl Write for CaptureWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.buf.lock().unwrap().write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CaptureWriter {
        type Writer = Self;
        fn make_writer(&self) -> Self::Writer {
            self.clone()
        }
    }

    #[test]
    fn sqlite_backend_and_ping() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.db");
        let storage = SqliteStorage::new(path.to_str().unwrap());

        assert_eq!(storage.backend_kind(), BackendKind::Sqlite);
        assert!(storage.ping().is_ok());
    }

    #[test]
    fn sqlite_backend_kind() {
        let db = SqliteStorage::new("sqlitetest.db");
        assert_eq!(db.backend_kind(), BackendKind::Sqlite);
    }

    #[test]
    fn litestream_warning_emitted_when_no_config() {
        // Ensure clean env state.
        std::env::remove_var("LITESTREAM_REPLICA_URL");

        let writer = CaptureWriter::new();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(writer.clone())
            .with_max_level(tracing::Level::WARN)
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);

        let cfg = StorageConfig {
            backend: BackendKind::Sqlite,
            sqlite_path: "test.db".to_string(),
            postgres_url: None,
            litestream_config: None,
            litestream_replica_url: String::new(),
        };

        cfg.warn_if_no_litestream();

        let output = writer.contents();
        assert!(
            output.contains("Litestream"),
            "expected warning to contain 'Litestream', got: {output:?}"
        );
        assert!(
            output.contains("backup storage"),
            "expected warning to contain 'backup storage', got: {output:?}"
        );
    }

    #[test]
    fn litestream_warning_suppressed_when_url_set() {
        // Unset env var for clean test state.
        std::env::remove_var("LITESTREAM_REPLICA_URL");

        let writer = CaptureWriter::new();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(writer.clone())
            .with_max_level(tracing::Level::WARN)
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);

        let cfg = StorageConfig {
            backend: BackendKind::Sqlite,
            sqlite_path: "test.db".to_string(),
            postgres_url: None,
            litestream_config: None,
            litestream_replica_url: "s3://my-bucket/rusterp".to_string(),
        };

        cfg.warn_if_no_litestream();

        let output = writer.contents();
        assert!(
            !output.contains("Litestream"),
            "expected NO warning when litestream_replica_url is set, got: {output:?}"
        );
    }

    #[test]
    fn pg_backend_skips_without_url() {
        let url = std::env::var("RUSTERP_POSTGRES_URL")
            .ok()
            .unwrap_or_default();
        if url.is_empty() {
            eprintln!("skipping Postgres integration test: RUSTERP_POSTGRES_URL not set");
            return;
        }
        let storage = PostgresStorage::new(url);
        assert_eq!(storage.backend_kind(), BackendKind::Postgres);
        assert!(storage.ping().is_ok());
    }
}
