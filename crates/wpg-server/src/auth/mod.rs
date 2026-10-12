//! Authentication (TODO §2.6, M3 "認証: パスワード (argon2id)、セッション、レート制限、
//! 未設定時 localhost 限定").
//!
//! * One admin account for now (the `users` table keeps a role for the P1
//!   multi-user work). Its password comes, highest priority first, from
//!   `--password`, `--password-file`, `WPG_PASSWORD`, `auth.password_hash`,
//!   `auth.password_file` ([`password::resolve`]) — written into the database as
//!   user `admin` at startup, ending its sessions when it changed — or from the
//!   first-run setup screen (`POST /api/auth/setup`) when none is configured.
//! * Until a password exists every protected endpoint answers 401
//!   `setup_required`, and the server refuses non-loopback addresses unless
//!   started with `--allow-insecure` (see [`crate::Server::bind`]); the setup
//!   request itself is only accepted from loopback clients in that case.
//! * Sessions and CSRF: [`session`]. Login rate limit: [`ratelimit`].
//! * [`require`] is the middleware in front of the API; public paths:
//!   [`is_public`].
//! * Audit trail: `tracing` events with target `wpg::audit` (login, logout,
//!   setup, job and capture actions) carrying the user and the client address.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{ConnectInfo, FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use thiserror::Error;
use utoipa::ToSchema;

use crate::db::{Db, DbError, NewSession, Role, SessionRow, User};
use crate::error::{codes, ApiError};
use crate::state::AppState;

pub mod password;
pub mod ratelimit;
pub mod session;

/// `tracing` target of audit events.
pub const AUDIT: &str = "wpg::audit";

/// Name of the admin account.
pub const ADMIN: &str = "admin";

/// Setup errors of [`Auth::new`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AuthError {
    /// Database error.
    #[error(transparent)]
    Db(#[from] DbError),
    /// Bad configured password.
    #[error(transparent)]
    Password(#[from] password::PasswordError),
}

/// The authenticated caller, as a request extension on protected routes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    /// User name.
    pub user: String,
    /// Role.
    pub role: Role,
    /// Client address (`local` for in-process requests).
    pub ip: String,
}

/// `user` of [`AuthStatus`].
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct UserInfo {
    /// User name.
    pub name: String,
    /// `admin`, `operator` or `viewer`.
    pub role: Role,
}

/// `GET /api/auth/status`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AuthStatus {
    /// A password exists (configured or set on the setup screen).
    pub configured: bool,
    /// The request carries a valid session.
    pub authenticated: bool,
    /// No password yet: show the setup screen (`POST /api/auth/setup`).
    pub setup_required: bool,
    /// The logged-in user.
    pub user: Option<UserInfo>,
    /// CSRF token to send as `X-CSRF-Token` on POST / PUT / DELETE (also in the
    /// `wpg_csrf` cookie); `null` when not logged in.
    pub csrf_token: Option<String>,
}

/// Client address extractor (`None` for in-process requests without
/// `ConnectInfo`, e.g. tests driving the router directly).
#[derive(Debug, Clone, Copy)]
pub struct ClientAddr(pub Option<SocketAddr>);

impl ClientAddr {
    fn of(ext: &axum::http::Extensions) -> Self {
        Self(ext.get::<ConnectInfo<SocketAddr>>().map(|c| c.0))
    }

    /// Address for logs / the rate limiter.
    #[must_use]
    pub fn ip(&self) -> String {
        self.0
            .map_or_else(|| "local".to_owned(), |a| a.ip().to_string())
    }

    /// Loopback (or in-process) client.
    #[must_use]
    pub fn is_local(&self) -> bool {
        self.0.is_none_or(|a| a.ip().is_loopback())
    }
}

impl<S: Send + Sync> FromRequestParts<S> for ClientAddr {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(Self::of(&parts.extensions))
    }
}

/// Unix seconds.
#[must_use]
pub fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

struct Inner {
    db: Db,
    ttl: i64,
    secure: AtomicBool,
    allow_insecure: AtomicBool,
    external: Option<&'static str>,
    limiter: ratelimit::RateLimiter,
    last_purge: AtomicI64,
}

/// Authentication service (cheap to clone).
#[derive(Clone)]
pub struct Auth {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Auth")
            .field("ttl", &self.inner.ttl)
            .field("external", &self.inner.external)
            .finish_non_exhaustive()
    }
}

impl Auth {
    /// Resolves the configured password (see the module docs) and stores it as
    /// the `admin` user.
    ///
    /// # Errors
    /// Invalid configured hash, unreadable password file, database errors.
    pub fn new(config: &crate::Config, db: Db) -> Result<Self, AuthError> {
        let external = password::resolve(config)?;
        if let Some(ext) = &external {
            match db.user_by_name(ADMIN)? {
                Some(u) if u.password_hash == ext.hash => {}
                Some(u) => {
                    // a hashed plain-text source gets a new salt on every start:
                    // keep the stored hash (and the sessions) when it still matches
                    let same = ext
                        .plain
                        .as_deref()
                        .is_some_and(|p| password::verify(p, &u.password_hash));
                    if !same {
                        db.set_password(u.id, &ext.hash)?;
                        tracing::info!(target: AUDIT, source = ext.source, "admin password changed by the configuration; sessions ended");
                    }
                }
                None => {
                    db.create_user(ADMIN, Role::Admin, &ext.hash, now_secs())?;
                }
            }
            tracing::info!("admin password from {}", ext.source);
        }
        let ttl = i64::try_from(config.auth.session_ttl_secs.max(1)).unwrap_or(i64::MAX);
        let _ = db.purge(now_secs());
        Ok(Self {
            inner: Arc::new(Inner {
                db,
                ttl,
                secure: AtomicBool::new(false),
                allow_insecure: AtomicBool::new(false),
                external: external.map(|e| e.source),
                limiter: ratelimit::RateLimiter::new(),
                last_purge: AtomicI64::new(now_secs()),
            }),
        })
    }

    /// Marks cookies `Secure` (the server speaks TLS).
    pub fn set_secure(&self, secure: bool) {
        self.inner.secure.store(secure, Ordering::Relaxed);
    }

    /// `--allow-insecure`: accept the setup request from any client.
    pub fn set_allow_insecure(&self, allow: bool) {
        self.inner.allow_insecure.store(allow, Ordering::Relaxed);
    }

    fn secure(&self) -> bool {
        self.inner.secure.load(Ordering::Relaxed)
    }

    /// Session lifetime in seconds.
    #[must_use]
    pub fn ttl(&self) -> i64 {
        self.inner.ttl
    }

    /// The database.
    #[must_use]
    pub fn db(&self) -> &Db {
        &self.inner.db
    }

    /// Where the configured password came from (`None`: database / setup).
    #[must_use]
    pub fn external_source(&self) -> Option<&'static str> {
        self.inner.external
    }

    /// No user exists yet: the setup screen must be completed first.
    #[must_use]
    pub fn setup_required(&self) -> bool {
        self.inner.db.user_count().map_or(true, |n| n == 0)
    }

    fn maybe_purge(&self, now: i64) {
        let last = self.inner.last_purge.load(Ordering::Relaxed);
        if now - last >= 600
            && self
                .inner
                .last_purge
                .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
        {
            let _ = self.inner.db.purge(now);
        }
    }

    /// The live session of a request.
    #[must_use]
    pub fn session(&self, headers: &HeaderMap) -> Option<SessionRow> {
        let token = session::cookie(headers, session::SESSION_COOKIE)?;
        if token.len() != 64 {
            return None;
        }
        let now = now_secs();
        self.maybe_purge(now);
        match self.inner.db.session(&session::token_id(token), now) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("session lookup failed: {e}");
                None
            }
        }
    }

    /// `GET /api/auth/status` for a request.
    #[must_use]
    pub fn status(&self, headers: &HeaderMap) -> AuthStatus {
        let setup_required = self.setup_required();
        let s = if setup_required {
            None
        } else {
            self.session(headers)
        };
        AuthStatus {
            configured: !setup_required,
            authenticated: s.is_some(),
            setup_required,
            user: s.as_ref().map(|s| UserInfo {
                name: s.user.name.clone(),
                role: s.user.role,
            }),
            csrf_token: s.map(|s| s.csrf),
        }
    }

    /// Creates a session for `user`; returns the response headers setting the
    /// cookies.
    ///
    /// # Errors
    /// Database / random generator errors.
    pub fn start_session(
        &self,
        user: &User,
        addr: ClientAddr,
        headers: &HeaderMap,
    ) -> Result<HeaderMap, ApiError> {
        let token = session::random_token().map_err(ApiError::internal)?;
        let csrf = session::random_token().map_err(ApiError::internal)?;
        let now = now_secs();
        let ip = addr.ip();
        let ua = headers
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.chars().take(256).collect::<String>());
        self.inner
            .db
            .insert_session(&NewSession {
                id: &session::token_id(&token),
                user_id: user.id,
                csrf: &csrf,
                now,
                expires: now.saturating_add(self.inner.ttl),
                ip: Some(&ip),
                ua: ua.as_deref(),
            })
            .map_err(ApiError::internal)?;
        let mut out = HeaderMap::new();
        session::append_cookies(&mut out, &token, &csrf, self.inner.ttl, self.secure());
        Ok(out)
    }

    /// Checks a login. Rate limited per client address.
    ///
    /// # Errors
    /// 429 `rate_limited`, 401 `setup_required` / `unauthorized`.
    pub async fn login(
        &self,
        username: Option<&str>,
        password: String,
        addr: ClientAddr,
    ) -> Result<User, ApiError> {
        let ip = addr.ip();
        let now = std::time::Instant::now();
        if let Err(wait) = self.inner.limiter.check(&ip, now) {
            tracing::warn!(target: AUDIT, ip = %ip, "login refused: rate limited");
            return Err(rate_limited(wait.as_secs().max(1)));
        }
        if self.setup_required() {
            return Err(setup_required());
        }
        let name = username
            .filter(|n| !n.is_empty())
            .unwrap_or(ADMIN)
            .to_owned();
        let user = self
            .inner
            .db
            .user_by_name(&name)
            .map_err(ApiError::internal)?;
        let ok = {
            let hash = user.as_ref().map(|u| u.password_hash.clone());
            tokio::task::spawn_blocking(move || match hash {
                Some(h) => password::verify(&password, &h),
                None => {
                    password::dummy_verify(&password);
                    false
                }
            })
            .await
            .map_err(ApiError::internal)?
        };
        let _ = self
            .inner
            .db
            .record_attempt(&ip, Some(&name), now_secs(), ok);
        match user {
            Some(u) if ok => {
                self.inner.limiter.success(&ip);
                tracing::info!(target: AUDIT, user = %u.name, ip = %ip, "login");
                Ok(u)
            }
            _ => {
                let lock = self.inner.limiter.failure(&ip, now);
                tracing::warn!(target: AUDIT, user = %name, ip = %ip, locked_secs = lock.map(|d| d.as_secs()), "login failed");
                Err(unauthorized("auth-error-bad-credentials"))
            }
        }
    }

    /// First-run setup: creates the admin user with `password`.
    ///
    /// # Errors
    /// 409 `already_configured`, 403 `forbidden` (remote client without
    /// `--allow-insecure`), 400 on a short / mismatching password.
    pub async fn setup(
        &self,
        password: String,
        confirm: &str,
        addr: ClientAddr,
    ) -> Result<User, ApiError> {
        if !self.setup_required() {
            return Err(already_configured());
        }
        if !addr.is_local() && !self.inner.allow_insecure.load(Ordering::Relaxed) {
            tracing::warn!(target: AUDIT, ip = %addr.ip(), "setup refused: remote client");
            return Err(ApiError::new(
                StatusCode::FORBIDDEN,
                codes::FORBIDDEN,
                "auth-error-setup-remote",
            ));
        }
        if password != confirm {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                codes::BAD_REQUEST,
                "auth-error-password-mismatch",
            ));
        }
        if password.chars().count() < password::MIN_LEN {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                codes::BAD_REQUEST,
                "auth-error-password-short",
            )
            .arg("min", password::MIN_LEN));
        }
        let hash = tokio::task::spawn_blocking(move || password::hash(&password))
            .await
            .map_err(ApiError::internal)?
            .map_err(ApiError::internal)?;
        let user = self
            .inner
            .db
            .create_first_user(ADMIN, Role::Admin, &hash, now_secs())
            .map_err(ApiError::internal)?
            .ok_or_else(already_configured)?;
        tracing::info!(target: AUDIT, user = %user.name, ip = %addr.ip(), "admin password set on the setup screen");
        Ok(user)
    }

    /// Ends the session of a request (CSRF checked when it is live); returns the
    /// headers clearing the cookies.
    ///
    /// # Errors
    /// 403 `csrf_failed`.
    pub fn logout(&self, headers: &HeaderMap, addr: ClientAddr) -> Result<HeaderMap, ApiError> {
        if let Some(s) = self.session(headers) {
            check_csrf(headers, &s)?;
            let _ = self.inner.db.delete_session(&s.id);
            tracing::info!(target: AUDIT, user = %s.user.name, ip = %addr.ip(), "logout");
        }
        let mut out = HeaderMap::new();
        session::append_clear_cookies(&mut out, self.secure());
        Ok(out)
    }
}

fn unauthorized(key: &'static str) -> ApiError {
    ApiError::new(StatusCode::UNAUTHORIZED, codes::UNAUTHORIZED, key)
}

fn setup_required() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        codes::SETUP_REQUIRED,
        "auth-error-setup-required",
    )
}

fn already_configured() -> ApiError {
    ApiError::new(
        StatusCode::CONFLICT,
        codes::ALREADY_CONFIGURED,
        "auth-error-already-configured",
    )
}

fn rate_limited(secs: u64) -> ApiError {
    ApiError::new(
        StatusCode::TOO_MANY_REQUESTS,
        codes::RATE_LIMITED,
        "auth-error-rate-limited",
    )
    .arg("secs", secs)
    .details(serde_json::json!({ "retry_after_secs": secs }))
}

fn check_csrf(headers: &HeaderMap, s: &SessionRow) -> Result<(), ApiError> {
    let sent = headers
        .get(session::CSRF_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if session::ct_eq(sent, &s.csrf) {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::FORBIDDEN,
            codes::CSRF_FAILED,
            "auth-error-csrf",
        ))
    }
}

/// API paths reachable without a session.
#[must_use]
pub fn is_public(path: &str) -> bool {
    matches!(
        path,
        "/api/health" | "/api/version" | "/api/openapi.json" | "/api/docs"
    ) || path.starts_with("/api/auth/")
        || path.starts_with("/api/docs/")
}

fn is_mutating(m: &Method) -> bool {
    !matches!(*m, Method::GET | Method::HEAD | Method::OPTIONS)
}

/// Same-origin check of a WebSocket upgrade (cross-site WebSocket hijacking):
/// an `Origin` header, when present, must name the `Host` the request went to
/// or the configured public URL (`server.base_url`, for reverse proxies that
/// rewrite `Host`).
fn same_origin(headers: &HeaderMap, base_url: Option<&str>) -> bool {
    let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else {
        return true;
    };
    if base_url.is_some_and(|b| b.trim_end_matches('/').eq_ignore_ascii_case(origin)) {
        return true;
    }
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    origin
        .split_once("://")
        .is_some_and(|(_, rest)| rest.trim_end_matches('/').eq_ignore_ascii_case(host))
}

/// Middleware in front of the API: session + CSRF enforcement, sliding expiry,
/// audit of job actions.
pub async fn require(State(st): State<AppState>, mut req: Request, next: Next) -> Response {
    let path = req.uri().path().to_owned();
    if !path.starts_with("/api") || is_public(&path) {
        return next.run(req).await;
    }
    let auth = &st.auth;
    if auth.setup_required() {
        return setup_required().into_response();
    }
    let Some(s) = auth.session(req.headers()) else {
        return unauthorized("auth-error-unauthorized").into_response();
    };
    let method = req.method().clone();
    if is_mutating(&method) {
        if let Err(e) = check_csrf(req.headers(), &s) {
            return e.into_response();
        }
    }
    if path == "/api/ws"
        && !st.dev
        && !same_origin(req.headers(), st.config.server.base_url.as_deref())
    {
        return ApiError::new(StatusCode::FORBIDDEN, codes::FORBIDDEN, "auth-error-origin")
            .into_response();
    }
    let addr = ClientAddr::of(req.extensions());
    let principal = Principal {
        user: s.user.name.clone(),
        role: s.user.role,
        ip: addr.ip(),
    };
    req.extensions_mut().insert(principal.clone());

    // sliding expiry
    let now = now_secs();
    let refresh = now - s.last_seen >= session::TOUCH_INTERVAL_SECS;
    if refresh {
        let _ = auth
            .db()
            .touch_session(&s.id, now, now.saturating_add(auth.ttl()));
    }
    let token = session::cookie(req.headers(), session::SESSION_COOKIE)
        .unwrap_or_default()
        .to_owned();

    let mut resp = next.run(req).await;
    if refresh {
        session::append_cookies(
            resp.headers_mut(),
            &token,
            &s.csrf,
            auth.ttl(),
            auth.secure(),
        );
    }
    if is_mutating(&method) && path.starts_with("/api/jobs") {
        let action = match (method.as_str(), path.rsplit('/').next().unwrap_or("")) {
            ("POST", "jobs") => "create",
            ("DELETE", _) => "delete",
            ("POST", a) => a,
            _ => "other",
        };
        tracing::info!(
            target: AUDIT,
            user = %principal.user,
            ip = %principal.ip,
            action,
            path = %path,
            status = resp.status().as_u16(),
            "job {action}"
        );
    }
    if is_mutating(&method) && path.starts_with("/api/captures") && path != "/api/captures/filter" {
        let action = match (method.as_str(), path.rsplit('/').next().unwrap_or("")) {
            ("POST", "captures") => "start",
            ("DELETE", _) => "delete",
            ("POST", a) => a,
            _ => "other",
        };
        tracing::info!(
            target: AUDIT,
            user = %principal.user,
            ip = %principal.ip,
            action,
            path = %path,
            status = resp.status().as_u16(),
            "capture {action}"
        );
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn public_paths() {
        for p in [
            "/api/health",
            "/api/version",
            "/api/auth/status",
            "/api/auth/login",
            "/api/openapi.json",
            "/api/docs",
            "/api/docs/index.html",
        ] {
            assert!(is_public(p), "{p}");
        }
        for p in [
            "/api/jobs",
            "/api/ws",
            "/api/healthz",
            "/api/docsx",
            "/api/auth",
        ] {
            assert!(!is_public(p), "{p}");
        }
    }

    #[test]
    fn origin_check() {
        let mut h = HeaderMap::new();
        assert!(same_origin(&h, None));
        h.insert(header::HOST, HeaderValue::from_static("127.0.0.1:8080"));
        let mut origin = |o: &'static str| {
            h.insert(header::ORIGIN, HeaderValue::from_static(o));
            h.clone()
        };
        assert!(same_origin(&origin("http://127.0.0.1:8080"), None));
        assert!(!same_origin(&origin("https://evil.example"), None));
        let proxied = origin("https://wpg.example.net");
        assert!(!same_origin(&proxied, None));
        assert!(same_origin(&proxied, Some("https://wpg.example.net/")));
    }
}
