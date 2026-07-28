//! HTTP server: slozhn gRPC-over-WebSocket + optional static WASM shell.

use std::net::SocketAddr;
use std::path::PathBuf;

use axum::http::{header, HeaderValue};
use axum::Router;
use tokio_util::sync::CancellationToken;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

/// Default HTTP listen address (slozhn + static WASM).
pub const DEFAULT_HTTP_LISTEN: &str = "127.0.0.1:8123";

/// Environment variable for HTTP listen override (CLI `--http-listen` wins).
pub const HTTP_LISTEN_ENV: &str = "RUSTERP_HTTP_LISTEN";

/// Parse CLI args for `--http-listen` / `-H`.
pub fn parse_http_listen_from_args(args: &[String]) -> Result<Option<String>, String> {
    let mut i = 1;
    let mut listen: Option<String> = None;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => return Err("help".into()),
            "--http-listen" | "-H" => {
                i += 1;
                let val = args
                    .get(i)
                    .ok_or_else(|| "missing value for --http-listen".to_string())?;
                listen = Some(val.clone());
            }
            other if other.starts_with("--http-listen=") => {
                listen = Some(other.trim_start_matches("--http-listen=").to_string());
            }
            "--listen" | "-l" => {
                i += 1;
                if args.get(i).is_none() {
                    return Err("missing value for --listen".to_string());
                }
            }
            other if other.starts_with("--listen=") => {}
            _ => {}
        }
        i += 1;
    }
    Ok(listen)
}

/// Serve axum HTTP (slozhn `/rpc` + optional static) until shutdown.
pub async fn serve_http(
    addr: SocketAddr,
    grpc: tonic::service::Routes,
    cancel: CancellationToken,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let session = slozhn::server::SessionManager::new(Default::default());

    let mut app = Router::new().route(
        "/rpc",
        slozhn::server::grpc_ws_session(grpc, session),
    );

    let static_dir = static_dir();
    if static_dir.exists() {
        let index = static_dir.join("index.html");
        let spa = ServeDir::new(&static_dir)
            .append_index_html_on_directories(true)
            .not_found_service(ServeFile::new(index));
        app = app.fallback_service(spa);
        tracing::info!(?static_dir, "serving static WASM shell");
    } else {
        tracing::warn!(
            "no static dir at {} — API-only HTTP mode (run dist/deploy-ui-stack.sh or trunk build into dist/ui/)",
            static_dir.display()
        );
        app = app.route("/", axum::routing::get(api_only_index));
    }

    app = app
        .layer(TraceLayer::new_for_http())
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        ));

    tracing::info!("HTTP listener on http://{addr}");
    tracing::info!("  gRPC-over-WebSocket: ws://{addr}/rpc");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            cancel.cancelled().await;
        })
        .await?;
    Ok(())
}

fn static_dir() -> PathBuf {
    if let Ok(p) = std::env::var("RUSTERP_STATIC") {
        return PathBuf::from(p);
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        manifest.join("../../dist/ui"),
        manifest.join("dist/ui"),
        PathBuf::from("dist/ui"),
    ];
    for c in candidates {
        if c.join("index.html").is_file() {
            return c;
        }
    }
    manifest.join("../../dist/ui")
}

async fn api_only_index() -> axum::response::Html<&'static str> {
    axum::response::Html(
        r#"<!doctype html>
<html><head><meta charset="utf-8"><title>RustERP</title>
<style>
 body{font-family:system-ui,sans-serif;max-width:42rem;margin:3rem auto;padding:0 1rem;line-height:1.5;color:#1a1a1a}
 code{background:#f2f2f2;padding:.1rem .35rem;border-radius:4px}
</style></head>
<body>
<h1>RustERP server is up</h1>
<p>gRPC-over-WebSocket endpoint: <code>/rpc</code></p>
<p>TCP gRPC (grpcurl): default <code>127.0.0.1:50051</code></p>
<p>Build the reference WASM UI into <code>dist/ui/</code> for same-origin browser access.</p>
</body></html>"#,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_http_listen_parses() {
        DEFAULT_HTTP_LISTEN
            .parse::<SocketAddr>()
            .expect("default HTTP listen");
    }

    #[test]
    fn parse_http_listen_flag() {
        let args = vec![
            "rusterp-server".into(),
            "--http-listen".into(),
            "127.0.0.1:9090".into(),
        ];
        let got = parse_http_listen_from_args(&args).unwrap();
        assert_eq!(got.as_deref(), Some("127.0.0.1:9090"));
    }
}
