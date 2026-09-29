//! `/api/auth/*`: status, first-run setup, login, logout (see [`crate::auth`]).

// utoipa's generated code spells out paths the lint considers redundant.
#![allow(unused_qualifications)]

use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::auth::{AuthStatus, ClientAddr};
use crate::error::{codes, ApiError, ApiJson};
use crate::state::AppState;

/// `POST /api/auth/setup` body.
#[derive(Deserialize, ToSchema)]
pub struct SetupRequest {
    /// The new admin password (at least 8 characters).
    pub password: String,
    /// The same password again.
    pub confirm: String,
}

/// `POST /api/auth/login` body.
#[derive(Deserialize, ToSchema)]
pub struct LoginRequest {
    /// Password.
    pub password: String,
    /// User name (default `admin`).
    #[serde(default)]
    pub username: Option<String>,
}

impl std::fmt::Debug for SetupRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SetupRequest { .. }")
    }
}

impl std::fmt::Debug for LoginRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoginRequest")
            .field("username", &self.username)
            .finish_non_exhaustive()
    }
}

fn no_content(headers: HeaderMap) -> Response {
    let mut resp = StatusCode::NO_CONTENT.into_response();
    resp.headers_mut().extend(headers);
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    resp
}

fn error_response(e: ApiError) -> Response {
    let retry = (e.code == codes::RATE_LIMITED)
        .then(|| {
            e.args
                .iter()
                .find(|(k, _)| *k == "secs")
                .map(|(_, v)| v.clone())
        })
        .flatten();
    let mut resp = e.into_response();
    if let Some(v) = retry.and_then(|v| HeaderValue::from_str(&v).ok()) {
        resp.headers_mut().insert(header::RETRY_AFTER, v);
    }
    resp
}

/// Authentication state of the caller (never requires a session).
#[utoipa::path(get, path = "/api/auth/status", tag = "auth", security(()),
    responses((status = 200, description = "State", body = AuthStatus)))]
pub async fn status(State(st): State<AppState>, headers: HeaderMap) -> Response {
    let mut resp = Json(st.auth.status(&headers)).into_response();
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    resp
}

/// First-run setup: sets the admin password and logs in. Only while no
/// password is configured, and only from a loopback client unless the server
/// runs with `--allow-insecure`. Requires `Content-Type: application/json`.
#[utoipa::path(post, path = "/api/auth/setup", tag = "auth", security(()),
    request_body = SetupRequest,
    responses(
        (status = 204, description = "Password set; `Set-Cookie`: `wpg_session`, `wpg_csrf`"),
        (status = 400, description = "Passwords differ / too short", body = crate::error::ErrorBody),
        (status = 403, description = "`forbidden`: remote client", body = crate::error::ErrorBody),
        (status = 409, description = "`already_configured`", body = crate::error::ErrorBody),
        (status = 415, description = "Not JSON", body = crate::error::ErrorBody)))]
pub async fn setup(
    State(st): State<AppState>,
    addr: ClientAddr,
    headers: HeaderMap,
    ApiJson(req): ApiJson<SetupRequest>,
) -> Response {
    let result = async {
        let user = st.auth.setup(req.password, &req.confirm, addr).await?;
        st.auth.start_session(&user, addr, &headers)
    }
    .await;
    match result {
        Ok(h) => no_content(h),
        Err(e) => error_response(e),
    }
}

/// Logs in (rate limited per client address: 5 failures a minute, then an
/// exponentially growing lock-out up to 15 minutes). Requires
/// `Content-Type: application/json`.
#[utoipa::path(post, path = "/api/auth/login", tag = "auth", security(()),
    request_body = LoginRequest,
    responses(
        (status = 204, description = "Logged in; `Set-Cookie`: `wpg_session`, `wpg_csrf`"),
        (status = 401, description = "`unauthorized` / `setup_required`", body = crate::error::ErrorBody),
        (status = 415, description = "Not JSON", body = crate::error::ErrorBody),
        (status = 429, description = "`rate_limited`: `details.retry_after_secs`, `Retry-After`", body = crate::error::ErrorBody)))]
pub async fn login(
    State(st): State<AppState>,
    addr: ClientAddr,
    headers: HeaderMap,
    ApiJson(req): ApiJson<LoginRequest>,
) -> Response {
    let result = async {
        let user = st
            .auth
            .login(req.username.as_deref(), req.password, addr)
            .await?;
        st.auth.start_session(&user, addr, &headers)
    }
    .await;
    match result {
        Ok(h) => no_content(h),
        Err(e) => error_response(e),
    }
}

/// Logs out (needs `X-CSRF-Token` when the session is live) and clears the
/// cookies.
#[utoipa::path(post, path = "/api/auth/logout", tag = "auth",
    responses(
        (status = 204, description = "Logged out"),
        (status = 403, description = "`csrf_failed`", body = crate::error::ErrorBody)))]
pub async fn logout(State(st): State<AppState>, addr: ClientAddr, headers: HeaderMap) -> Response {
    match st.auth.logout(&headers, addr) {
        Ok(h) => no_content(h),
        Err(e) => error_response(e),
    }
}
