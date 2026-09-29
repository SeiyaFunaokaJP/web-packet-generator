//! # wpg-server
//!
//! axum + tokio HTTP server: JSON API under `/api`, and the Leptos web UI (built by
//! Trunk into `crates/wpg-web/dist`) embedded with `rust-embed` and served at `/`.

use std::net::SocketAddr;

use axum::extract::Request;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use rust_embed::RustEmbed;
use serde::Serialize;
use wpg_core::ProtocolDef;
use wpg_i18n::{negotiate, t, t_args, FluentArgs, Locale};

/// Crate version reported by `/api/health`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Front-end assets produced by `trunk build` in `crates/wpg-web`.
///
/// `allow_missing` keeps the server buildable before the front-end has ever been built
/// (fresh clone, CI lint jobs); a built-in placeholder page is served instead.
#[derive(RustEmbed)]
#[folder = "../wpg-web/dist"]
#[allow_missing = true]
struct WebAssets;

/// Initialises `tracing` output (`RUST_LOG` aware, default `info`).
///
/// Safe to call more than once; later calls are no-ops.
pub fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    // `try_init` fails only if a global subscriber is already set — fine to ignore.
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

/// Builds the application router (exposed for tests and embedding).
pub fn router() -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/protocols", get(protocols))
        .route("/api/{*rest}", get(api_not_found))
        .fallback(static_asset)
}

/// Binds `bind` and serves until Ctrl-C.
///
/// # Errors
/// If the address cannot be bound or the server fails.
pub async fn run(bind: SocketAddr) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(bind).await?;
    let addr = listener.local_addr()?;
    let mut args = FluentArgs::new();
    args.set("addr", addr.to_string());
    tracing::info!("{}", t_args(Locale::En, "server-listening", &args));
    axum::serve(listener, router())
        .with_graceful_shutdown(shutdown_signal())
        .await
}

async fn shutdown_signal() {
    if let Err(e) = tokio::signal::ctrl_c().await {
        tracing::error!("failed to listen for Ctrl-C: {e}");
        std::future::pending::<()>().await;
    }
    tracing::info!("shutting down");
}

/// Locale requested by the client via `Accept-Language` (default English).
fn request_locale(headers: &HeaderMap) -> Locale {
    headers
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok())
        .map(negotiate)
        .unwrap_or_default()
}

/// `GET /api/health` response body.
#[derive(Debug, Serialize)]
pub struct Health {
    /// Always `"ok"` when the server answers.
    pub status: &'static str,
    /// Server version.
    pub version: &'static str,
    /// Negotiated locale.
    pub locale: &'static str,
    /// Localised human-readable status.
    pub message: String,
}

async fn health(headers: HeaderMap) -> Json<Health> {
    let locale = request_locale(&headers);
    Json(Health {
        status: "ok",
        version: VERSION,
        locale: locale.as_str(),
        message: t(locale, "health-ok"),
    })
}

async fn protocols() -> Json<&'static [ProtocolDef]> {
    Json(wpg_protocols::catalog().as_slice())
}

#[derive(Debug, Serialize)]
struct ApiError {
    error: String,
}

async fn api_not_found(headers: HeaderMap) -> Response {
    let locale = request_locale(&headers);
    (
        StatusCode::NOT_FOUND,
        Json(ApiError {
            error: t(locale, "error-not-found"),
        }),
    )
        .into_response()
}

/// Serves embedded front-end files; unknown paths get `index.html` (SPA routing).
async fn static_asset(uri: Uri, req: Request) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(file) = WebAssets::get(path) {
        let mime = file.metadata.mimetype();
        let mut resp = file.data.into_owned().into_response();
        if let Ok(value) = HeaderValue::from_str(mime) {
            resp.headers_mut().insert(header::CONTENT_TYPE, value);
        }
        return resp;
    }
    if let Some(index) = WebAssets::get("index.html") {
        return Html(index.data.into_owned()).into_response();
    }
    placeholder_page(request_locale(req.headers())).into_response()
}

fn placeholder_page(locale: Locale) -> Html<String> {
    Html(format!(
        "<!doctype html><html lang=\"{lang}\"><head><meta charset=\"utf-8\">\
         <title>{title}</title></head><body><h1>{title}</h1><p>{msg}</p></body></html>",
        lang = locale.as_str(),
        title = t(locale, "app-name"),
        msg = t(locale, "server-placeholder-ui"),
    ))
}
