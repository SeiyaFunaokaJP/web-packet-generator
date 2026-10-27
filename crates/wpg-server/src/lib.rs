//! # wpg-server
//!
//! axum + tokio HTTP server (spec §1, M3): the JSON API under `/api` ([`api`]),
//! a WebSocket event stream (`/api/ws`), the OpenAPI document
//! (`/api/openapi.json`, Swagger UI at `/api/docs/`) and the Leptos web UI (built
//! by Trunk into `crates/wpg-web/dist`) embedded with `rust-embed` and served at
//! `/` with SPA fallback.
//!
//! * [`config`]: `wpg.toml` + `WPG_*` environment + CLI overrides;
//! * [`db`]: the `SQLite` database (users, sessions, login attempts, job history);
//! * [`auth`]: argon2id password, cookie sessions + CSRF, login rate limit,
//!   first-run setup, audit events; [`auth::require`] guards the API router
//!   ([`api::router`]) while the static assets stay public;
//! * [`security`]: CSP and the other security headers;
//! * [`tls`]: HTTPS with given or self-signed certificates (rustls);
//! * [`store`]: file-backed template library with hot reload;
//! * [`jobs`]: job manager on top of `wpg-tx` with the transmit policy;
//! * [`snmp`]: the SNMP manager tool (MIBs, trap receiver, pcap recording);
//! * [`state::AppState`]: what the handlers share.

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
pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod events;
pub mod jobs;
pub mod localize;
pub mod rx;
pub mod security;
pub mod snmp;
pub mod state;
pub mod store;
pub mod tls;

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

/// Builds the whole application: API (behind authentication), OpenAPI / Swagger
/// UI, static assets and the middleware stack (request id, tracing, security
/// headers, compression, error localisation, CORS in dev mode).
pub fn app(state: AppState) -> Router {
    let dev = state.dev;
    let x_request_id = HeaderName::from_static("x-request-id");
    let mut router = api::router(state)
        .fallback(static_asset)
        .layer(axum::middleware::from_fn(error::localize))
        .layer(axum::middleware::from_fn(security::headers))
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
            HeaderName::from_static(auth::session::CSRF_HEADER),
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
    /// `--allow-insecure`: bind a non-loopback address although no password is
    /// configured (and accept the first-run setup from remote clients).
    pub allow_insecure: bool,
}

/// Errors of [`Server::bind`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BindError {
    /// No password and a non-loopback address without `--allow-insecure`.
    #[error("refusing to listen on {0} without a password (use a loopback address, set a password or pass --allow-insecure)")]
    Insecure(SocketAddr),
    /// State (data directory, database, password).
    #[error(transparent)]
    State(#[from] state::StateError),
    /// TLS certificate / key.
    #[error(transparent)]
    Tls(#[from] tls::TlsError),
    /// Socket.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// A bound, not yet running server.
#[derive(Debug)]
pub struct Server {
    listener: tokio::net::TcpListener,
    state: AppState,
    tls: Option<tls::TlsSetup>,
}

impl Server {
    /// Creates the state, sets up TLS and binds `config.server.bind`.
    ///
    /// Without any password (none configured, no user created on the setup
    /// screen) only loopback addresses are accepted unless
    /// [`ServeOptions::allow_insecure`] is set (spec §2.6 "未設定なら localhost
    /// のみ bind").
    ///
    /// # Errors
    /// [`BindError::Insecure`], data directory / database / password / TLS /
    /// bind errors.
    pub async fn bind(config: Config, opts: ServeOptions) -> Result<Self, BindError> {
        let bind = config.server.bind;
        let tls = tls::setup(&config)?;
        let state = AppState::new(config)?.with_dev(opts.dev);
        if state.auth.setup_required() && !bind.ip().is_loopback() {
            if !opts.allow_insecure {
                return Err(BindError::Insecure(bind));
            }
            tracing::warn!(
                "no password configured and listening on {bind} (--allow-insecure): \
                 anyone who reaches it can set the admin password"
            );
        }
        state.auth.set_secure(tls.is_some());
        state.auth.set_allow_insecure(opts.allow_insecure);
        if let Some(t) = &tls {
            tracing::info!(
                cert = %t.cert_path.display(),
                self_signed = t.self_signed,
                generated = t.generated,
                "TLS certificate SHA-256 fingerprint {}",
                t.fingerprint
            );
        }
        if opts.watch {
            state.watch_templates();
        }
        let listener = tokio::net::TcpListener::bind(bind).await?;
        state.snmp.start_from_config().await;
        Ok(Self {
            listener,
            state,
            tls,
        })
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

    /// The TLS setup (`None`: plain HTTP).
    #[must_use]
    pub fn tls(&self) -> Option<&tls::TlsSetup> {
        self.tls.as_ref()
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
        let make_service = app(self.state).into_make_service_with_connect_info::<SocketAddr>();
        let stop = async move {
            shutdown.await;
            tracing::info!("shutting down: stopping jobs");
            signal_state.begin_shutdown();
            signal_state.jobs.shutdown().await;
            signal_state.rx.shutdown().await;
            signal_state.snmp.stop_receiver().await;
        };
        let res = match self.tls {
            None => {
                axum::serve(self.listener, make_service)
                    .with_graceful_shutdown(stop)
                    .await
            }
            Some(tls) => {
                let handle = axum_server::Handle::new();
                let h = handle.clone();
                let stopper = tokio::spawn(async move {
                    stop.await;
                    h.graceful_shutdown(Some(Duration::from_secs(5)));
                });
                let rustls = axum_server::tls_rustls::RustlsConfig::from_config(tls.config);
                let res = axum_server::from_tcp_rustls(self.listener.into_std()?, rustls)?
                    .handle(handle)
                    .serve(make_service)
                    .await;
                stopper.abort();
                res
            }
        };
        state.jobs.shutdown().await;
        state.rx.shutdown().await;
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

    if path != "index.html" {
        if let Some(file) = WebAssets::get(path) {
            let mime = file.metadata.mimetype();
            let mut resp = file.data.into_owned().into_response();
            if let Ok(value) = HeaderValue::from_str(mime) {
                resp.headers_mut().insert(header::CONTENT_TYPE, value);
            }
            return resp;
        }
    }
    if let Some(index) = WebAssets::get("index.html") {
        let html = String::from_utf8_lossy(&index.data).into_owned();
        let csp = security::index_csp(&html);
        let mut resp = Html(html).into_response();
        if let Ok(v) = HeaderValue::from_str(&csp) {
            resp.headers_mut()
                .insert(header::CONTENT_SECURITY_POLICY, v);
        }
        resp.headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        return resp;
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
