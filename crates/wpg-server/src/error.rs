//! API errors: `{ "error": { "code", "message", "details"? } }`.
//!
//! Handlers return [`ApiError`], which carries a stable machine `code`, an HTTP
//! status, a Fluent message key with arguments and optional details. The
//! response is first rendered in English; the [`localize`] middleware then
//! re-renders it in the language of the request's `Accept-Language` header
//! (the error travels in the response extensions).

use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{FromRequest, FromRequestParts, Path, Query, Request};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use wpg_i18n::{t_args, FluentArgs, Locale};
use wpg_template::{LintIssue, TemplateError};

use crate::localize::{lint_issue_json, request_locale};

/// Stable error codes (the `code` field).
pub mod codes {
    /// Unknown resource.
    pub const NOT_FOUND: &str = "not_found";
    /// Malformed request (body, query, path).
    pub const BAD_REQUEST: &str = "bad_request";
    /// The template does not parse / compile.
    pub const INVALID_TEMPLATE: &str = "invalid_template";
    /// The transmit policy forbids the job (interface / rate).
    pub const POLICY_VIOLATION: &str = "policy_violation";
    /// Writing to a read-only template library.
    pub const READ_ONLY: &str = "read_only";
    /// The job is not in a state that allows the action.
    pub const INVALID_STATE: &str = "invalid_state";
    /// Opening / driving the transmit backend failed.
    pub const BACKEND_ERROR: &str = "backend_error";
    /// Not logged in / wrong password.
    pub const UNAUTHORIZED: &str = "unauthorized";
    /// No password exists yet: complete `POST /api/auth/setup` first.
    pub const SETUP_REQUIRED: &str = "setup_required";
    /// Too many failed logins; `details.retry_after_secs`.
    pub const RATE_LIMITED: &str = "rate_limited";
    /// Missing / wrong `X-CSRF-Token`.
    pub const CSRF_FAILED: &str = "csrf_failed";
    /// Not allowed from this client (setup from a remote address, foreign
    /// WebSocket origin).
    pub const FORBIDDEN: &str = "forbidden";
    /// The setup screen was already completed.
    pub const ALREADY_CONFIGURED: &str = "already_configured";
    /// The web terminal is switched off (`terminal.enabled = false`).
    pub const TERMINAL_DISABLED: &str = "terminal_disabled";
    /// Anything else.
    pub const INTERNAL: &str = "internal";
}

/// Error details that need localisation at render time.
#[derive(Debug, Clone)]
pub enum Details {
    /// Arbitrary JSON (not localised).
    Json(Value),
    /// Template findings; each gets a localised `message`.
    Lint(Vec<LintIssue>),
}

/// An API error.
#[derive(Debug, Clone)]
pub struct ApiError {
    /// HTTP status.
    pub status: StatusCode,
    /// Machine code (see [`codes`]).
    pub code: &'static str,
    /// Fluent message key.
    pub key: &'static str,
    /// Fluent arguments.
    pub args: Vec<(&'static str, String)>,
    /// Optional details.
    pub details: Option<Details>,
}

/// `{ "error": ... }` body (OpenAPI schema of every error response).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ErrorBody {
    /// The error.
    pub error: ErrorInfo,
}

/// Error object.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ErrorInfo {
    /// Stable machine code: `not_found`, `bad_request`, `invalid_template`,
    /// `policy_violation`, `read_only`, `invalid_state`, `backend_error`,
    /// `unauthorized`, `setup_required`, `rate_limited`, `csrf_failed`,
    /// `forbidden`, `already_configured`, `terminal_disabled`, `internal`.
    pub code: String,
    /// Human readable message in the language of `Accept-Language`.
    pub message: String,
    /// Extra data, e.g. the lint issues of an invalid template.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl ApiError {
    /// Creates an error.
    #[must_use]
    pub fn new(status: StatusCode, code: &'static str, key: &'static str) -> Self {
        Self {
            status,
            code,
            key,
            args: Vec::new(),
            details: None,
        }
    }

    /// Adds a Fluent argument.
    #[must_use]
    #[allow(clippy::needless_pass_by_value)] // takes owned and borrowed values alike
    pub fn arg(mut self, name: &'static str, value: impl ToString) -> Self {
        self.args.push((name, value.to_string()));
        self
    }

    /// Attaches JSON details.
    #[must_use]
    pub fn details(mut self, v: Value) -> Self {
        self.details = Some(Details::Json(v));
        self
    }

    /// 404 for `what` (e.g. `template foo`).
    #[must_use]
    pub fn not_found(what: impl ToString) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            codes::NOT_FOUND,
            "api-error-not-found",
        )
        .arg("what", what)
    }

    /// 400 with a message key taking `$error`.
    #[must_use]
    pub fn bad_request(key: &'static str, error: impl ToString) -> Self {
        Self::new(StatusCode::BAD_REQUEST, codes::BAD_REQUEST, key).arg("error", error)
    }

    /// 403 `policy_violation`.
    #[must_use]
    pub fn policy(key: &'static str) -> Self {
        Self::new(StatusCode::FORBIDDEN, codes::POLICY_VIOLATION, key)
    }

    /// 409 `invalid_state`.
    #[must_use]
    pub fn invalid_state(id: &str, state: &str, action: &str) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            codes::INVALID_STATE,
            "api-error-invalid-state",
        )
        .arg("id", id)
        .arg("state", state)
        .arg("action", action)
    }

    /// 500 `internal`.
    #[must_use]
    pub fn internal(error: impl ToString) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            codes::INTERNAL,
            "api-error-internal",
        )
        .arg("error", error)
    }

    /// 422 `invalid_template` from a template error (lint issues become details).
    #[must_use]
    pub fn template(e: TemplateError) -> Self {
        let err = Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            codes::INVALID_TEMPLATE,
            "api-error-invalid-template",
        )
        .arg("error", &e);
        match e {
            TemplateError::Lint(issues) => Self {
                details: Some(Details::Lint(issues)),
                ..err
            },
            _ => err,
        }
    }

    /// The localised body.
    #[must_use]
    pub fn body(&self, locale: Locale) -> ErrorBody {
        let mut a = FluentArgs::new();
        for (k, v) in &self.args {
            a.set(*k, v.clone());
        }
        let details = self.details.as_ref().map(|d| match d {
            Details::Json(v) => v.clone(),
            Details::Lint(issues) => {
                Value::Array(issues.iter().map(|i| lint_issue_json(i, locale)).collect())
            }
        });
        ErrorBody {
            error: ErrorInfo {
                code: self.code.to_owned(),
                message: t_args(locale, self.key, &a),
                details,
            },
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.body(Locale::En).error.message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut resp = (self.status, Json(self.body(Locale::En))).into_response();
        resp.extensions_mut().insert(self);
        resp
    }
}

impl From<JsonRejection> for ApiError {
    fn from(r: JsonRejection) -> Self {
        let mut e = Self::bad_request("api-error-invalid-body", r.body_text());
        if r.status() == StatusCode::UNSUPPORTED_MEDIA_TYPE {
            e.status = StatusCode::UNSUPPORTED_MEDIA_TYPE;
        }
        e
    }
}

impl From<QueryRejection> for ApiError {
    fn from(r: QueryRejection) -> Self {
        Self::bad_request("api-error-invalid-query", r.body_text())
    }
}

impl From<PathRejection> for ApiError {
    fn from(r: PathRejection) -> Self {
        Self::bad_request("api-error-invalid-query", r.body_text())
    }
}

/// `Json` extractor whose rejections are [`ApiError`]s.
#[derive(Debug, Clone, Copy, Default, FromRequest)]
#[from_request(via(Json), rejection(ApiError))]
pub struct ApiJson<T>(pub T);

impl<T: Serialize> IntoResponse for ApiJson<T> {
    fn into_response(self) -> Response {
        Json(self.0).into_response()
    }
}

/// `Query` extractor whose rejections are [`ApiError`]s.
#[derive(Debug, Clone, Copy, Default, FromRequestParts)]
#[from_request(via(Query), rejection(ApiError))]
pub struct ApiQuery<T>(pub T);

/// `Path` extractor whose rejections are [`ApiError`]s.
#[derive(Debug, Clone, Copy, Default, FromRequestParts)]
#[from_request(via(Path), rejection(ApiError))]
pub struct ApiPath<T>(pub T);

/// Result type of the handlers.
pub type ApiResult<T> = Result<T, ApiError>;

/// Middleware: re-renders [`ApiError`] responses in the request's language.
pub async fn localize(req: Request, next: Next) -> Response {
    let locale = request_locale(req.headers());
    let mut resp = next.run(req).await;
    match resp.extensions_mut().remove::<ApiError>() {
        Some(err) if locale != Locale::En => {
            let mut new = (err.status, Json(err.body(locale))).into_response();
            for (k, v) in resp.headers() {
                if k != axum::http::header::CONTENT_LENGTH {
                    new.headers_mut().insert(k.clone(), v.clone());
                }
            }
            new
        }
        _ => resp,
    }
}
