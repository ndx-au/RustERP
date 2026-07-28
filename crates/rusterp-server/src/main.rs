//! RustERP gRPC server binary.
//!
//! Serves `rusterp.party.v1.PartyService` and `rusterp.platform.v1.HealthService`
//! with a **SQLite-backed** party store. **Auth is not enforced.**
//!
//! **Dual transport:**
//! - TCP gRPC on `RUSTERP_LISTEN` (default `127.0.0.1:50051`) — grpcurl / API tools.
//! - HTTP + slozhn WebSocket on `RUSTERP_HTTP_LISTEN` (default `127.0.0.1:8123`) — browser UI.
//!
//! Optional [`rusterp-server.toml`](../../rusterp-server.toml) (or `RUSTERP_CONFIG`) sets
//! listen addresses and port-conflict policy (default: clobber).

mod config;
mod http;
mod port_guard;

use std::process;
use std::sync::{Arc, Mutex};

use rusterp_server::{
    build_grpc_routes, build_router, new_shared_repo, parse_listen_from_args, SharedRepo, LISTEN_ENV,
};
use rusterp_parties::SqlitePartyRepository;
use rusterp_storage::from_config;
use tokio_util::sync::CancellationToken;

use config::{ServerConfig, CONFIG_ENV};
use http::{parse_http_listen_from_args, serve_http, DEFAULT_HTTP_LISTEN, HTTP_LISTEN_ENV};
use port_guard::{ensure_ports_available, remove_pid_file, resolve_pid_file, write_pid_file};

fn print_usage() {
    eprintln!(
        "RustERP gRPC server (in-memory Parties; auth not enforced)\n\
         \n\
         Usage:\n\
           rusterp-server [--listen ADDR] [--http-listen ADDR]\n\
         \n\
         Options:\n\
           --listen, -l ADDR       TCP gRPC listen (default from config/env)\n\
           --http-listen, -H ADDR  HTTP + slozhn WS listen (default {DEFAULT_HTTP_LISTEN})\n\
           --help, -h              Show this help\n\
         \n\
         Env:\n\
           {LISTEN_ENV}            TCP gRPC listen if --listen is not set\n\
           {HTTP_LISTEN_ENV}       HTTP listen if --http-listen is not set\n\
           {CONFIG_ENV}            Path to rusterp-server.toml\n\
         \n\
         Config file (optional): rusterp-server.toml in cwd or repo root.\n\
         Port conflict default: restart self via pidfile, then clobber occupant.\n"
    );
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let file_cfg = ServerConfig::load();

    let args: Vec<String> = std::env::args().collect();
    let cli_listen = match parse_listen_from_args(&args) {
        Ok(v) => v,
        Err(msg) if msg == "help" => {
            print_usage();
            return Ok(());
        }
        Err(msg) => {
            eprintln!("error: {msg}");
            print_usage();
            std::process::exit(2);
        }
    };

    let cli_http_listen = match parse_http_listen_from_args(&args) {
        Ok(v) => v,
        Err(msg) if msg == "help" => {
            print_usage();
            return Ok(());
        }
        Err(msg) => {
            eprintln!("error: {msg}");
            print_usage();
            std::process::exit(2);
        }
    };

    let tcp_addr = file_cfg.resolve_tcp_listen(cli_listen.as_deref())?;
    let http_addr = file_cfg.resolve_http_listen(cli_http_listen.as_deref())?;

    if let Some(static_dir) = file_cfg.resolve_static_dir() {
        if static_dir.is_dir() {
            std::env::set_var("RUSTERP_STATIC", static_dir);
        }
    }

    let pid_path = resolve_pid_file(&file_cfg.port_conflict.pid_file);
    ensure_ports_available(&[tcp_addr, http_addr], &file_cfg.port_conflict)?;
    write_pid_file(&pid_path, process::id())?;

    let storage_cfg = file_cfg.resolve_storage_config();
    storage_cfg.warn_if_no_litestream();

    // Determine the storage backend and construct the party repository
    // accordingly. SQLite: create storage, run migrations, build
    // SqlitePartyRepository. Postgres is out of scope for this Spec so
    // we fall back to in-memory parties with a warning.
    let (storage, repo):
        (Arc<dyn rusterp_storage::Storage>, SharedRepo)
        = if storage_cfg.backend == rusterp_storage::BackendKind::Sqlite {
            let backend = rusterp_storage::SqliteStorage::new(&storage_cfg.sqlite_path)?;
            let conn = backend.conn_handle();
            {
                let guard = conn.lock().map_err(|e| {
                    format!("sqlite lock poisoned: {e}")
                })?;
                rusterp_storage::run_migrations(&guard).map_err(|e| {
                    format!("migrations failed: {e}")
                })?;
            }
            tracing::info!(
                "storage backend: sqlite (SQLite-backed parties)"
            );
            let parties_repo = Arc::new(Mutex::new(SqlitePartyRepository::new(conn)));
            (Arc::from(backend) as Arc<dyn rusterp_storage::Storage>, parties_repo)
        } else {
            let backend = from_config(&storage_cfg)?;
            tracing::warn!(
                "Postgres storage: parties are in-memory (SQLite parties are the \
                 active path; Postgres parties are out of scope for this Spec)"
            );
            (Arc::from(backend), new_shared_repo())
        };

    let tcp_router = build_router(storage.clone(), repo.clone())?;
    let grpc_routes = build_grpc_routes(storage, repo)?;

    let cancel = CancellationToken::new();

    tracing::info!("TCP gRPC listening on {tcp_addr} (in-memory; auth not enforced)");

    let tcp_handle = tokio::spawn(async move {
        if let Err(e) = tcp_router.serve(tcp_addr).await {
            tracing::error!("TCP gRPC server error: {e}");
        }
    });

    let http_cancel = cancel.clone();
    let http_handle = tokio::spawn(async move {
        if let Err(e) = serve_http(http_addr, grpc_routes, http_cancel).await {
            tracing::error!("HTTP server error: {e}");
        }
    });

    wait_for_shutdown(cancel.clone()).await;

    tcp_handle.abort();
    let _ = http_handle.await;
    remove_pid_file(&pid_path);

    tracing::info!("rusterp-server shut down");
    Ok(())
}

async fn wait_for_shutdown(cancel: CancellationToken) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
    cancel.cancel();
}
