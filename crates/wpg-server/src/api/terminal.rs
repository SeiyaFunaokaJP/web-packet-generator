//! `/api/terminal`: the web terminal (ADR-0011): status, the session WebSocket,
//! session logs and the trusted SSH host keys.

// utoipa's generated code spells out paths the lint considers redundant.
#![allow(unused_qualifications)]

use axum::extract::ws::WebSocketUpgrade;
use axum::extract::State;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};

use crate::auth::{Principal, AUDIT};
use crate::db::Role;
use crate::error::{codes, ApiError, ApiPath, ApiResult};
use crate::state::AppState;
use crate::terminal::known_hosts::KnownHost;
use crate::terminal::recorder::{self, LogInfo};
use crate::terminal::TerminalStatus;

/// 403 `terminal_disabled`.
fn disabled() -> ApiError {
    ApiError::new(
        StatusCode::FORBIDDEN,
        codes::TERMINAL_DISABLED,
        "api-error-terminal-disabled",
    )
}

/// Configuration and running sessions (also when the terminal is disabled, so
/// the UI can explain how to enable it).
#[utoipa::path(get, path = "/api/terminal/status", tag = "terminal",
    responses((status = 200, description = "Terminal status", body = TerminalStatus)))]
pub async fn status(State(st): State<AppState>) -> Json<TerminalStatus> {
    Json(st.terminal.status())
}

/// Opens a terminal session (WebSocket). Send `connect` first; see
/// `TerminalClientMessage` / `TerminalServerMessage`. Remote output arrives as
/// binary frames, keystrokes may be sent as binary frames or `input` messages.
#[utoipa::path(get, path = "/api/terminal/ws", tag = "terminal",
    responses(
        (status = 101, description = "WebSocket: `TerminalServerMessage` JSON + binary output", body = crate::terminal::ServerMsg),
        (status = 403, description = "`terminal_disabled` (`terminal.enabled = false`) or `forbidden` (viewer role)", body = crate::error::ErrorBody)))]
pub async fn ws(
    State(st): State<AppState>,
    principal: Option<Extension<Principal>>,
    upgrade: WebSocketUpgrade,
) -> ApiResult<Response> {
    if !st.terminal.enabled() {
        return Err(disabled());
    }
    let who = principal.map_or_else(
        || Principal {
            user: crate::auth::ADMIN.to_owned(),
            role: Role::Admin,
            ip: "local".to_owned(),
        },
        |Extension(p)| p,
    );
    if who.role == Role::Viewer {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            codes::FORBIDDEN,
            "api-error-terminal-forbidden",
        ));
    }
    let term = st.terminal.clone();
    let shutdown = st.shutdown.clone();
    Ok(upgrade
        .max_message_size(1 << 20)
        .on_upgrade(move |socket| crate::terminal::session::run(term, shutdown, socket, who))
        .into_response())
}

/// Session logs, newest first.
#[utoipa::path(get, path = "/api/terminal/logs", tag = "terminal",
    responses((status = 200, description = "Logs", body = Vec<LogInfo>)))]
pub async fn logs(State(st): State<AppState>) -> ApiResult<Json<Vec<LogInfo>>> {
    let t = st.terminal.clone();
    tokio::task::spawn_blocking(move || recorder::list(t.log_dir(), &t.active_logs()))
        .await
        .map_err(ApiError::internal)?
        .map(Json)
        .map_err(ApiError::internal)
}

fn log_path(st: &AppState, name: &str) -> ApiResult<std::path::PathBuf> {
    recorder::path_of(st.terminal.log_dir(), name)
        .filter(|p| p.is_file())
        .ok_or_else(|| ApiError::not_found(format!("log {name}")))
}

/// Downloads a session log (`text/plain`, also while it is being written).
#[utoipa::path(get, path = "/api/terminal/logs/{name}", tag = "terminal",
    params(("name" = String, Path, description = "Log file name")),
    responses(
        (status = 200, description = "The log", content_type = "text/plain"),
        (status = 404, description = "Unknown log", body = crate::error::ErrorBody)))]
pub async fn get_log(
    State(st): State<AppState>,
    ApiPath(name): ApiPath<String>,
) -> ApiResult<Response> {
    let path = log_path(&st, &name)?;
    let bytes = tokio::fs::read(&path).await.map_err(ApiError::internal)?;
    let mut resp = bytes.into_response();
    let h = resp.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{name}\"")) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    Ok(resp)
}

/// Deletes a session log (not one that is being written).
#[utoipa::path(delete, path = "/api/terminal/logs/{name}", tag = "terminal",
    params(("name" = String, Path, description = "Log file name")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "Unknown log", body = crate::error::ErrorBody),
        (status = 409, description = "`invalid_state`: the session is still recording", body = crate::error::ErrorBody)))]
pub async fn delete_log(
    State(st): State<AppState>,
    principal: Option<Extension<Principal>>,
    ApiPath(name): ApiPath<String>,
) -> ApiResult<StatusCode> {
    let path = log_path(&st, &name)?;
    if st.terminal.active_logs().contains(&name) {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            codes::INVALID_STATE,
            "api-error-terminal-log-active",
        )
        .arg("file", &name));
    }
    tokio::fs::remove_file(&path)
        .await
        .map_err(ApiError::internal)?;
    if let Some(Extension(p)) = principal {
        tracing::info!(target: AUDIT, user = %p.user, ip = %p.ip, log = %name, "terminal log deleted");
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Trusted SSH host keys (`<data_dir>/terminal/known_hosts`).
#[utoipa::path(get, path = "/api/terminal/known-hosts", tag = "terminal",
    responses((status = 200, description = "Host keys", body = Vec<KnownHost>)))]
pub async fn known_hosts(State(st): State<AppState>) -> ApiResult<Json<Vec<KnownHost>>> {
    let t = st.terminal.clone();
    tokio::task::spawn_blocking(move || t.known_hosts().list())
        .await
        .map_err(ApiError::internal)?
        .map(Json)
        .map_err(ApiError::internal)
}

/// Forgets the host key(s) of `host`:`port` (after a legitimate key change).
#[utoipa::path(delete, path = "/api/terminal/known-hosts/{host}/{port}", tag = "terminal",
    params(
        ("host" = String, Path, description = "Host as stored"),
        ("port" = u16, Path, description = "Port")),
    responses(
        (status = 204, description = "Removed"),
        (status = 404, description = "No key stored", body = crate::error::ErrorBody)))]
pub async fn delete_known_host(
    State(st): State<AppState>,
    principal: Option<Extension<Principal>>,
    ApiPath((host, port)): ApiPath<(String, u16)>,
) -> ApiResult<StatusCode> {
    let t = st.terminal.clone();
    let h = host.clone();
    let removed = tokio::task::spawn_blocking(move || t.known_hosts().remove(&h, port))
        .await
        .map_err(ApiError::internal)?
        .map_err(ApiError::internal)?;
    if !removed {
        return Err(ApiError::not_found(format!("host key {host}:{port}")));
    }
    if let Some(Extension(p)) = principal {
        tracing::info!(target: AUDIT, user = %p.user, ip = %p.ip, host = %host, port, "terminal host key removed");
    }
    Ok(StatusCode::NO_CONTENT)
}
