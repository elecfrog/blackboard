use anyhow::{Context, Result};
use axum::body::{to_bytes, Body};
use axum::extract::State;
use axum::http::header::{CONTENT_LENGTH, HOST};
use axum::http::{HeaderMap, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use axum::Router;
use clap::Parser;
use reqwest::Client;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::net::TcpListener;
use tower_http::services::{ServeDir, ServeFile};

#[derive(Debug, Parser)]
#[command(
    name = "bb-server",
    about = "Blackboard web/API facade for local development"
)]
struct Cli {
    /// Accepted for dev.py compatibility. `bb_daemon` owns the workspace root.
    #[arg(long)]
    root: Option<PathBuf>,
    /// Address to bind, for example 127.0.0.1:3002.
    #[arg(long, default_value = "127.0.0.1:3002")]
    addr: SocketAddr,
    /// Directory containing built frontend assets.
    #[arg(long)]
    static_dir: Option<PathBuf>,
    /// URL of the local authoritative daemon.
    #[arg(long, default_value = "http://127.0.0.1:3001")]
    daemon_url: String,
}

#[derive(Clone)]
struct AppState {
    daemon_url: String,
    client: Client,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Some(root) = cli.root.as_ref() {
        eprintln!(
            "bb_server received --root {}; bb_daemon remains the authoritative workspace owner",
            root.display()
        );
    }

    let state = AppState {
        daemon_url: cli.daemon_url.trim_end_matches('/').to_string(),
        client: Client::new(),
    };

    let mut app = Router::new()
        .route("/healthz", get(healthz))
        .route("/api/{*path}", any(proxy_to_daemon))
        .route("/mcp", any(proxy_to_daemon))
        .route("/mcp/{*path}", any(proxy_to_daemon))
        .with_state(state);

    if let Some(static_dir) = cli.static_dir {
        let index = static_dir.join("index.html");
        app = app
            .fallback_service(ServeDir::new(static_dir).not_found_service(ServeFile::new(index)));
    }

    let listener = TcpListener::bind(cli.addr)
        .await
        .with_context(|| format!("failed to bind bb_server to {}", cli.addr))?;
    eprintln!(
        "bb_server listening on http://{} and proxying API/MCP to {}",
        cli.addr, cli.daemon_url
    );
    axum::serve(listener, app).await.context("bb_server failed")
}

async fn healthz() -> &'static str {
    "ok"
}

async fn proxy_to_daemon(State(state): State<AppState>, request: Request<Body>) -> Response {
    match proxy_request(&state, request).await {
        Ok(response) => response,
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            format!("bb_server proxy error: {error:#}"),
        )
            .into_response(),
    }
}

async fn proxy_request(state: &AppState, request: Request<Body>) -> Result<Response> {
    let method = request.method().clone();
    let path_and_query = request
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");
    let url = format!("{}{}", state.daemon_url, path_and_query);
    let headers = request.headers().clone();
    let body = to_bytes(request.into_body(), usize::MAX)
        .await
        .context("failed to read request body")?;

    let builder = copy_proxy_request_headers(state.client.request(method, url), &headers);
    let upstream = builder
        .body(body.to_vec())
        .send()
        .await
        .context("daemon request failed")?;

    let status = upstream.status();
    let headers = upstream.headers().clone();
    let body = upstream
        .bytes()
        .await
        .context("failed to read daemon body")?;
    let mut response = Response::builder().status(status);
    for (name, value) in &headers {
        if name != CONTENT_LENGTH {
            response = response.header(name, value);
        }
    }
    response
        .body(Body::from(body))
        .context("failed to build proxy response")
}

fn copy_proxy_request_headers(
    mut builder: reqwest::RequestBuilder,
    headers: &HeaderMap,
) -> reqwest::RequestBuilder {
    for (name, value) in headers {
        if name == HOST || name == CONTENT_LENGTH {
            continue;
        }
        builder = builder.header(name, value);
    }
    builder
}
