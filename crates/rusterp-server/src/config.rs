//! Optional `rusterp-server.toml` — listen addresses and port-conflict policy.

use std::net::SocketAddr;
use std::path::PathBuf;

use serde::Deserialize;

use crate::http::{DEFAULT_HTTP_LISTEN, HTTP_LISTEN_ENV};
use rusterp_server::{DEFAULT_LISTEN, LISTEN_ENV};
use rusterp_storage::StorageConfig;

/// Env var pointing at a TOML config file.
pub const CONFIG_ENV: &str = "RUSTERP_CONFIG";

const DEFAULT_CONFIG_NAME: &str = "rusterp-server.toml";

/// Port conflict policy when a listen address is already bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PortConflictPolicy {
    /// SIGTERM prior instance (pidfile), then kill whatever still holds the port.
    Clobber,
    /// Exit with error if the port remains busy after own restart attempt.
    Fail,
}

impl Default for PortConflictPolicy {
    fn default() -> Self {
        Self::Clobber
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PortConflictConfig {
    #[serde(default)]
    pub policy: PortConflictPolicy,
    /// File storing the last server PID (for graceful self-restart).
    #[serde(default = "default_pid_file")]
    pub pid_file: PathBuf,
    #[serde(default = "default_graceful_secs")]
    pub graceful_secs: u64,
}

fn default_pid_file() -> PathBuf {
    PathBuf::from(".local/rusterp-server.pid")
}

fn default_graceful_secs() -> u64 {
    5
}

impl Default for PortConflictConfig {
    fn default() -> Self {
        Self {
            policy: PortConflictPolicy::Clobber,
            pid_file: default_pid_file(),
            graceful_secs: default_graceful_secs(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TcpConfig {
    pub listen: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct HttpConfig {
    pub listen: Option<String>,
    pub static_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default)]
    pub tcp: TcpConfig,
    #[serde(default)]
    pub http: HttpConfig,
    #[serde(default)]
    pub port_conflict: PortConflictConfig,
    #[serde(default)]
    pub storage: StorageConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            tcp: TcpConfig::default(),
            http: HttpConfig::default(),
            port_conflict: PortConflictConfig::default(),
            storage: StorageConfig::default(),
        }
    }
}

impl ServerConfig {
    /// Load config: optional file merged over defaults. Missing file is OK.
    pub fn load() -> Self {
        let path = resolve_config_path();
        let Some(path) = path else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(raw) => match toml::from_str(&raw) {
                Ok(cfg) => {
                    tracing::info!(path = %path.display(), "loaded server config");
                    cfg
                }
                Err(e) => {
                    tracing::warn!(path = %path.display(), "invalid config ({e}); using defaults");
                    Self::default()
                }
            },
            Err(e) => {
                tracing::warn!(path = %path.display(), "could not read config ({e}); using defaults");
                Self::default()
            }
        }
    }

    pub fn resolve_tcp_listen(
        &self,
        cli_override: Option<&str>,
    ) -> Result<SocketAddr, String> {
        let raw = cli_override
            .map(str::to_string)
            .or_else(|| self.tcp.listen.clone())
            .or_else(|| std::env::var(LISTEN_ENV).ok())
            .unwrap_or_else(|| DEFAULT_LISTEN.to_string());
        raw.parse::<SocketAddr>()
            .map_err(|e| format!("invalid TCP listen address {raw:?}: {e}"))
    }

    pub fn resolve_http_listen(
        &self,
        cli_override: Option<&str>,
    ) -> Result<SocketAddr, String> {
        let raw = cli_override
            .map(str::to_string)
            .or_else(|| self.http.listen.clone())
            .or_else(|| std::env::var(HTTP_LISTEN_ENV).ok())
            .unwrap_or_else(|| DEFAULT_HTTP_LISTEN.to_string());
        raw.parse::<SocketAddr>()
            .map_err(|e| format!("invalid HTTP listen address {raw:?}: {e}"))
    }

    pub fn resolve_static_dir(&self) -> Option<PathBuf> {
        if let Ok(p) = std::env::var("RUSTERP_STATIC") {
            return Some(PathBuf::from(p));
        }
        self.http.static_dir.as_ref().map(|p| {
            if p.is_absolute() {
                p.clone()
            } else {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(p)
            }
        })
    }

    /// Resolve storage configuration from env vars → TOML → defaults.
    pub fn resolve_storage_config(&self) -> StorageConfig {
        let postgres_url = std::env::var("RUSTERP_POSTGRES_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| self.storage.postgres_url.clone());

        let max_connections = std::env::var("RUSTERP_PG_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(self.storage.max_connections);

        let min_connections = std::env::var("RUSTERP_PG_MIN_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(self.storage.min_connections);

        let acquire_timeout_secs = std::env::var("RUSTERP_PG_ACQUIRE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(self.storage.acquire_timeout_secs);

        StorageConfig {
            postgres_url,
            max_connections,
            min_connections,
            acquire_timeout_secs,
            idle_timeout_secs: self.storage.idle_timeout_secs,
            max_lifetime_secs: self.storage.max_lifetime_secs,
        }
    }
}

fn resolve_config_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var(CONFIG_ENV) {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
        tracing::warn!(path = %path.display(), "{CONFIG_ENV} set but file missing");
    }
    for candidate in config_candidates() {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn config_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        out.push(cwd.join(DEFAULT_CONFIG_NAME));
    }
    out.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../{DEFAULT_CONFIG_NAME}")),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_is_clobber() {
        let cfg = ServerConfig::default();
        assert_eq!(cfg.port_conflict.policy, PortConflictPolicy::Clobber);
    }

    #[test]
    fn parse_example_toml() {
        let raw = r#"
[tcp]
listen = "0.0.0.0:50051"
[http]
listen = "0.0.0.0:8123"
[port_conflict]
policy = "fail"
"#;
        let cfg: ServerConfig = toml::from_str(raw).unwrap();
        assert_eq!(cfg.tcp.listen.as_deref(), Some("0.0.0.0:50051"));
        assert_eq!(cfg.http.listen.as_deref(), Some("0.0.0.0:8123"));
        assert_eq!(cfg.port_conflict.policy, PortConflictPolicy::Fail);
    }
}
