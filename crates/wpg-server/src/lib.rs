//! # wpg-server
//!
//! axum + tokio HTTP server (TODO §1, M3): the JSON API under `/api` ([`api`]),
//! a WebSocket event stream (`/api/ws`), the OpenAPI document
//! (`/api/openapi.json`, Swagger UI at `/api/docs/`) and the Leptos web UI (built
//! by Trunk into `crates/wpg-web/dist`) embedded with `rust-embed` and served at
//! `/` with SPA fallback.
//!
//! * [`config`]: `wpg.toml` + `WPG_*` environment + CLI overrides;
//! * [`store`]: file-backed template library with hot reload;
//! * [`jobs`]: job manager on top of `wpg-tx` with the transmit policy;
//! * [`state::AppState`]: what the handlers share.
//!
//! Authentication / TLS are not enforced yet (the `auth` config section is stored
//! for the upcoming work); the API router is built by [`api::router`] so that an
//! auth layer can wrap it without touching the static assets.

use std::net::SocketAddr;
use std::time::Duration;

use axum::extract::Request;
use axum::http::{header, HeaderName, HeaderValue, Method, Uri};
use axum::response::{Html, IntoResponse, Response};
use axum::Router;
use rust_embed::RustEmbed;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;
use wpg_i18n::{t, Locale};

pub mod api;
pub mod config;
pub mod error;
pub mod events;
pub mod jobs;
pub mod localize;
pub mod state;
pub mod store;

pub use config::{Config, ConfigError, Overrides};
pub use state::AppState;

/// Crate version reported by `/api/health`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Origins of the Trunk dev server allowed by CORS in `--dev` mode.
pub const DEV_ORIGINS: &[&str] = &["http://localhost:8081", "http://127.0.0.1:8081"];

/// Front-end assets produced by `trunk build` in `crates/wpg-web`.
///
/// `allow_missing` keeps the server buildable before the front-end has ever been built
/// (fresh clone, CI lint jobs); a built-in placeholder page is served instead.
#[derive(RustEmbed)]
#[folder = "../wpg-web/dist"]
#[allow_missing = true]
struct WebAssets;

/// Installs the global `tracing` subscriber from `cfg` (console / file, pretty /
/// JSON, `RUST_LOG` aware) with the WebSocket log sink attached. Keep the guard
/// alive until exit.
///
/// # Errors
/// Invalid level, unusable log file, or a subscriber already installed.
pub fn init_tracing(cfg: &config::LogConfig) -> Result<wpg_log::LogGuard, wpg_log::LogError> {
    wpg_log::init_with_sink(cfg, Some(state::log_channel().clone()))
}

/// Builds the whole application: API, OpenAPI / Swagger UI, static assets and the
/// middleware stack (request id, tracing, compression, error localisation, CORS
/// in dev mode).
pub fn app(state: AppState) -> Router {
    let dev = state.dev;
    let x_request_id = HeaderName::from_static("x-request-id");
    let mut router = api::router(state)
        .fallback(static_asset)
        .layer(axum::middleware::from_fn(error::localize))
        .layer(CompressionLayer::new());
    if dev {
        router = router.layer(dev_cors());
    }
    router
        .layer(PropagateRequestIdLayer::new(x_request_id.clone()))
        .layer(TraceLayer::new_for_http())
        .layer(SetRequestIdLayer::new(x_request_id, MakeRequestUuid))
}

fn dev_cors() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(
            DEV_ORIGINS
                .iter()
                .filter_map(|o| HeaderValue::from_str(o).ok())
                .collect::<Vec<_>>(),
        )
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::ACCEPT,
            header::ACCEPT_LANGUAGE,
            header::AUTHORIZATION,
        ])
        .allow_credentials(true)
        .max_age(Duration::from_secs(600))
}

/// Options of [`Server::bind`] that are not configuration.
#[derive(Debug, Clone, Copy, Default)]
pub struct ServeOptions {
    /// Development mode: CORS for the Trunk dev server on :8081.
    pub dev: bool,
    /// Watch the template libraries for changes.
    pub watch: bool,
}

/// A bound, not yet running server.
#[derive(Debug)]
pub struct Server {
    listener: tokio::net::TcpListener,
    state: AppState,
}

impl Server {
    /// Creates the state and binds `config.server.bind`.
    ///
    /// # Errors
    /// Data directory / bind errors.
    pub async fn bind(config: Config, opts: ServeOptions) -> std::io::Result<Self> {
        let bind = config.server.bind;
        let state = AppState::new(config)
            .map_err(std::io::Error::other)?
            .with_dev(opts.dev);
        if opts.watch {
            state.watch_templates();
        }
        let listener = tokio::net::TcpListener::bind(bind).await?;
        Ok(Self { listener, state })
    }

    /// The bound address (the real port when binding port 0).
    ///
    /// # Errors
    /// Socket errors.
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// The shared state.
    #[must_use]
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Serves until `shutdown` completes, then stops all jobs (their reports go to
    /// the history) and closes the WebSocket sessions.
    ///
    /// # Errors
    /// Server errors.
    pub async fn run_until(
        self,
        shutdown: impl std::future::Future<Output = ()> + Send + 'static,
    ) -> std::io::Result<()> {
        let state = self.state.clone();
        let signal_state = self.state.clone();
        let res = axum::serve(self.listener, app(self.state))
            .with_graceful_shutdown(async move {
                shutdown.await;
                tracing::info!("shutting down: stopping jobs");
                signal_state.begin_shutdown();
                signal_state.jobs.shutdown().await;
            })
            .await;
        state.jobs.shutdown().await;
        res
    }

    /// Serves until Ctrl-C / SIGTERM.
    ///
    /// # Errors
    /// Server errors.
    pub async fn run(self) -> std::io::Result<()> {
        self.run_until(shutdown_signal()).await
    }
}

/// Resolves on Ctrl-C (and SIGTERM on Unix).
pub async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!("failed to listen for Ctrl-C: {e}");
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = term => {},
    }
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
    placeholder_page(localize::request_locale(req.headers())).into_response()
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
