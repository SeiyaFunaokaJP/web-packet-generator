//! Typed client of the wpg-server REST API (`gloo-net` fetch, same origin).
//!
//! * `Accept-Language` follows the UI language, so server errors
//!   (`{ "error": { "code", "message", "details" } }`) arrive localised;
//! * mutating requests carry `X-CSRF-Token` (from `/api/auth/status`, else the
//!   `wpg_csrf` cookie); a 403 whose code mentions CSRF refreshes the token and
//!   retries once;
//! * a 401 outside `/api/auth/*` calls the hook installed by [`crate::auth`], which
//!   re-checks the session and sends the user to `/login` (or `/setup`).

pub(crate) mod snmp;
pub(crate) mod terminal;
pub(crate) mod types;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gloo_net::http::{Request, RequestBuilder, Response};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use wpg_i18n::Locale;

use crate::i18n::I18n;
pub(crate) use types::*;

/// An API failure.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ApiError {
    /// Non-2xx response.
    Server {
        status: u16,
        /// Stable machine code (`not_found`, `invalid_template`, ...).
        code: String,
        /// Localised message from the server.
        message: String,
        details: Option<Value>,
        /// `Retry-After` (seconds) of a 429.
        retry_after: Option<u32>,
    },
    /// The request did not complete (offline, server down, CORS, ...).
    Network(String),
    /// The response body was not what we expected.
    Decode(String),
}

impl ApiError {
    /// Human readable message in the UI language.
    pub(crate) fn message(&self, i18n: I18n) -> String {
        match self {
            ApiError::Server {
                status, message, ..
            } => {
                if message.trim().is_empty() {
                    i18n.ta("ui-error-http", &[("id", status.to_string())])
                } else {
                    message.clone()
                }
            }
            ApiError::Network(e) => i18n.ta("ui-error-network", &[("error", e.clone())]),
            ApiError::Decode(e) => i18n.ta("ui-error-decode", &[("error", e.clone())]),
        }
    }

    pub(crate) fn status(&self) -> Option<u16> {
        match self {
            ApiError::Server { status, .. } => Some(*status),
            _ => None,
        }
    }

    pub(crate) fn code(&self) -> &str {
        match self {
            ApiError::Server { code, .. } => code,
            ApiError::Network(_) => "network",
            ApiError::Decode(_) => "decode",
        }
    }

    pub(crate) fn retry_after(&self) -> Option<u32> {
        match self {
            ApiError::Server { retry_after, .. } => *retry_after,
            _ => None,
        }
    }

    /// Lint findings in `details` (invalid templates).
    pub(crate) fn lint_issues(&self) -> Vec<LintIssue> {
        match self {
            ApiError::Server {
                details: Some(d @ Value::Array(_)),
                ..
            } => serde_json::from_value(d.clone()).unwrap_or_default(),
            _ => Vec::new(),
        }
    }
}

/// Builds an [`ApiError`] from a non-2xx response body (pure; unit tested).
pub(crate) fn error_from_body(status: u16, body: &str, retry_header: Option<&str>) -> ApiError {
    let v: Option<Value> = serde_json::from_str(body).ok();
    let err = v.as_ref().and_then(|v| v.get("error"));
    let s = |k: &str| {
        err.and_then(|e| e.get(k))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    let details = err.and_then(|e| e.get("details")).cloned();
    let retry_after = retry_header
        .and_then(|h| h.trim().parse::<u32>().ok())
        .or_else(|| {
            let d = details.as_ref()?;
            ["retry_after_secs", "retry_after"]
                .iter()
                .find_map(|k| d.get(*k).and_then(Value::as_u64))
                .and_then(|n| u32::try_from(n).ok())
        });
    let fallback: String = body.chars().take(300).collect();
    ApiError::Server {
        status,
        code: s("code").unwrap_or_else(|| "http".to_owned()),
        message: s("message").unwrap_or(fallback),
        details,
        retry_after,
    }
}

thread_local! {
    static LOCALE: Cell<Locale> = const { Cell::new(Locale::En) };
    static CSRF: RefCell<Option<String>> = const { RefCell::new(None) };
    static ON_UNAUTHORIZED: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Language sent as `Accept-Language`.
pub(crate) fn set_locale(l: Locale) {
    LOCALE.with(|c| c.set(l));
}

/// CSRF token from `/api/auth/status`.
pub(crate) fn set_csrf(token: Option<String>) {
    CSRF.with(|c| *c.borrow_mut() = token);
}

/// Called on a 401 from a non-auth endpoint.
pub(crate) fn set_unauthorized_hook(f: impl Fn() + 'static) {
    ON_UNAUTHORIZED.with(|c| *c.borrow_mut() = Some(Rc::new(f)));
}

fn csrf_token() -> Option<String> {
    CSRF.with(|c| c.borrow().clone())
        .or_else(|| crate::dom::cookie("wpg_csrf"))
}

/// Percent-encodes a path segment / query value (RFC 3986 unreserved kept).
pub(crate) fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Method {
    Get,
    Post,
    Put,
    Delete,
}

#[derive(Debug, Clone)]
enum Body {
    Empty,
    Json(String),
    Text(String, &'static str),
}

fn builder(method: Method, path: &str) -> RequestBuilder {
    let b = match method {
        Method::Get => Request::get(path),
        Method::Post => Request::post(path),
        Method::Put => Request::put(path),
        Method::Delete => Request::delete(path),
    };
    let b = b
        .header("Accept", "application/json")
        .header("Accept-Language", LOCALE.with(Cell::get).as_str());
    match (method, csrf_token()) {
        (Method::Get, _) | (_, None) => b,
        (_, Some(tok)) => b.header("X-CSRF-Token", &tok),
    }
}

async fn send_once(method: Method, path: &str, body: &Body) -> Result<Response, ApiError> {
    let net = |e: gloo_net::Error| ApiError::Network(e.to_string());
    let b = builder(method, path);
    let req = match body {
        Body::Empty => b.build(),
        Body::Json(s) => b
            .header("Content-Type", "application/json")
            .body(s.as_str()),
        Body::Text(s, ct) => b.header("Content-Type", ct).body(s.as_str()),
    }
    .map_err(net)?;
    req.send().await.map_err(net)
}

async fn request(method: Method, path: &str, body: Body) -> Result<Response, ApiError> {
    let mut retried = false;
    loop {
        let resp = send_once(method, path, &body).await?;
        let status = resp.status();
        if (200..300).contains(&status) {
            return Ok(resp);
        }
        let retry = resp.headers().get("retry-after");
        let text = resp.text().await.unwrap_or_default();
        let err = error_from_body(status, &text, retry.as_deref());
        if status == 403 && method != Method::Get && !retried && err.code().contains("csrf") {
            retried = true;
            // Not `auth_status()`: that would make this future recursive.
            if let Ok(r) = send_once(Method::Get, "/api/auth/status", &Body::Empty).await {
                if let Ok(s) = json::<AuthStatus>(r).await {
                    set_csrf(s.csrf_token);
                    continue;
                }
            }
        }
        if status == 401 && !path.starts_with("/api/auth/") {
            if let Some(hook) = ON_UNAUTHORIZED.with(|c| c.borrow().clone()) {
                hook();
            }
        }
        return Err(err);
    }
}

async fn json<T: DeserializeOwned>(resp: Response) -> Result<T, ApiError> {
    let text = resp
        .text()
        .await
        .map_err(|e| ApiError::Network(e.to_string()))?;
    serde_json::from_str(&text).map_err(|e| ApiError::Decode(e.to_string()))
}

async fn get<T: DeserializeOwned>(path: &str) -> Result<T, ApiError> {
    json(request(Method::Get, path, Body::Empty).await?).await
}

fn body_of(v: &impl Serialize) -> Result<Body, ApiError> {
    serde_json::to_string(v)
        .map(Body::Json)
        .map_err(|e| ApiError::Decode(e.to_string()))
}

async fn post<T: DeserializeOwned>(path: &str, v: &impl Serialize) -> Result<T, ApiError> {
    json(request(Method::Post, path, body_of(v)?).await?).await
}

async fn post_empty(path: &str, v: Option<&Value>) -> Result<(), ApiError> {
    let body = match v {
        Some(v) => body_of(v)?,
        None => Body::Empty,
    };
    request(Method::Post, path, body).await.map(|_| ())
}

// ---------------------------------------------------------------- auth

/// `GET /api/auth/status`; `Ok(None)` when the server has no auth endpoints
/// (older builds: everything is open).
pub(crate) async fn auth_status() -> Result<Option<AuthStatus>, ApiError> {
    match get::<AuthStatus>("/api/auth/status").await {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.status() == Some(404) => Ok(None),
        Err(e) => Err(e),
    }
}

/// `POST /api/auth/setup` (first run).
pub(crate) async fn auth_setup(password: &str, confirm: &str) -> Result<(), ApiError> {
    let v = serde_json::json!({ "password": password, "confirm": confirm });
    post_empty("/api/auth/setup", Some(&v)).await
}

/// `POST /api/auth/login` (204; 401 wrong password; 429 rate limited).
pub(crate) async fn auth_login(password: &str) -> Result<(), ApiError> {
    let v = serde_json::json!({ "password": password });
    post_empty("/api/auth/login", Some(&v)).await
}

/// `POST /api/auth/logout`.
pub(crate) async fn auth_logout() -> Result<(), ApiError> {
    post_empty("/api/auth/logout", None).await
}

// ---------------------------------------------------------------- system

pub(crate) async fn version() -> Result<VersionInfo, ApiError> {
    get("/api/version").await
}

pub(crate) async fn ifaces(refresh: bool) -> Result<Vec<Iface>, ApiError> {
    get(if refresh {
        "/api/system/ifaces?refresh=true"
    } else {
        "/api/system/ifaces"
    })
    .await
}

pub(crate) async fn doctor() -> Result<DoctorReport, ApiError> {
    get("/api/system/doctor").await
}

pub(crate) async fn backends() -> Result<Backends, ApiError> {
    get("/api/system/backends").await
}

pub(crate) async fn overview() -> Result<Overview, ApiError> {
    get("/api/stats/overview").await
}

// ---------------------------------------------------------------- templates

/// `GET /api/templates` query string for `q`.
pub(crate) fn templates_url(q: &TemplateQuery) -> String {
    let mut parts = Vec::new();
    if !q.q.trim().is_empty() {
        parts.push(format!("q={}", enc(q.q.trim())));
    }
    if let Some(t) = &q.tag {
        parts.push(format!("tag={}", enc(t)));
    }
    if q.favorite {
        parts.push("favorite=true".to_owned());
    }
    if let Some(s) = &q.source {
        parts.push(format!("source={}", enc(s)));
    }
    if parts.is_empty() {
        "/api/templates".to_owned()
    } else {
        format!("/api/templates?{}", parts.join("&"))
    }
}

pub(crate) async fn templates(q: TemplateQuery) -> Result<TemplateList, ApiError> {
    get(&templates_url(&q)).await
}

/// `PUT /api/templates/{id}` with YAML text (the server accepts a JSON string).
pub(crate) async fn update_template_yaml(id: &str, yaml: &str) -> Result<TemplateDetail, ApiError> {
    let body = body_of(&Value::String(yaml.to_owned()))?;
    json(request(Method::Put, &format!("/api/templates/{}", enc(id)), body).await?).await
}

pub(crate) async fn delete_template(id: &str) -> Result<(), ApiError> {
    request(
        Method::Delete,
        &format!("/api/templates/{}", enc(id)),
        Body::Empty,
    )
    .await
    .map(|_| ())
}

pub(crate) async fn duplicate_template(id: &str) -> Result<TemplateDetail, ApiError> {
    post(
        &format!("/api/templates/{}/duplicate", enc(id)),
        &Value::Null,
    )
    .await
}

/// URL of `GET /api/templates/{id}/export` (a plain link downloads it).
pub(crate) fn export_url(id: &str) -> String {
    format!("/api/templates/{}/export", enc(id))
}

/// The `.wpg.yaml` text of a template.
pub(crate) async fn export_template(id: &str) -> Result<String, ApiError> {
    request(Method::Get, &export_url(id), Body::Empty)
        .await?
        .text()
        .await
        .map_err(|e| ApiError::Network(e.to_string()))
}

pub(crate) async fn import_template(yaml: String) -> Result<TemplateDetail, ApiError> {
    json(
        request(
            Method::Post,
            "/api/templates/import",
            Body::Text(yaml, "text/yaml; charset=utf-8"),
        )
        .await?,
    )
    .await
}

pub(crate) async fn set_favorite(id: &str, favorite: bool) -> Result<(), ApiError> {
    let v = serde_json::json!({ "favorite": favorite });
    post_empty(&format!("/api/templates/{}/favorite", enc(id)), Some(&v)).await
}

/// `POST /api/lint` with YAML text.
pub(crate) async fn lint_yaml(yaml: &str) -> Result<LintResponse, ApiError> {
    post("/api/lint", &serde_json::json!({ "template": yaml })).await
}

/// `POST /api/lint` with a template JSON object.
pub(crate) async fn lint_json(template: &Value) -> Result<LintResponse, ApiError> {
    post("/api/lint", &serde_json::json!({ "template": template })).await
}

// ---------------------------------------------------------------- builder

/// `GET /api/protocols`: the palette catalog.
pub(crate) async fn protocols() -> Result<Vec<ProtocolSummary>, ApiError> {
    get("/api/protocols").await
}

/// `GET /api/protocols/extensions`: user-defined protocol files of the server.
pub(crate) async fn protocol_extensions(
) -> Result<Vec<wpg_protocols::extension::Extension>, ApiError> {
    get("/api/protocols/extensions").await
}

/// `GET /api/templates/{id}` (loads a template into the builder).
pub(crate) async fn template(id: &str) -> Result<TemplateDetail, ApiError> {
    get(&format!("/api/templates/{}", enc(id))).await
}

/// `POST /api/render`: server-side render of a template (parity check + preview).
pub(crate) async fn render(
    template: &Value,
    count: u64,
    index: u64,
) -> Result<RenderResponse, ApiError> {
    post(
        "/api/render",
        &serde_json::json!({ "template": template, "count": count, "index": index }),
    )
    .await
}

/// `POST /api/templates`: create a new user template from its JSON form.
pub(crate) async fn create_template(template: &Value) -> Result<TemplateDetail, ApiError> {
    post("/api/templates", template).await
}

/// `PUT /api/templates/{id}` with a template JSON object.
pub(crate) async fn update_template_json(
    id: &str,
    template: &Value,
) -> Result<TemplateDetail, ApiError> {
    let body = body_of(template)?;
    json(request(Method::Put, &format!("/api/templates/{}", enc(id)), body).await?).await
}

// ---------------------------------------------------------------- jobs

pub(crate) async fn jobs() -> Result<Vec<JobInfo>, ApiError> {
    get("/api/jobs").await
}

pub(crate) async fn job_history() -> Result<Vec<JobInfo>, ApiError> {
    get("/api/jobs/history").await
}

pub(crate) async fn job(id: &str) -> Result<JobInfo, ApiError> {
    get(&format!("/api/jobs/{}", enc(id))).await
}

pub(crate) async fn create_job(body: &CreateJob) -> Result<JobInfo, ApiError> {
    post("/api/jobs", body).await
}

/// A job state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JobAction {
    Start,
    Pause,
    Resume,
    Stop,
}

impl JobAction {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            JobAction::Start => "start",
            JobAction::Pause => "pause",
            JobAction::Resume => "resume",
            JobAction::Stop => "stop",
        }
    }

    /// Whether the action applies to a job in `state`.
    pub(crate) fn allowed(self, state: &str) -> bool {
        match self {
            JobAction::Start => state == "created",
            JobAction::Pause => state == "running",
            JobAction::Resume => state == "paused",
            JobAction::Stop => matches!(state, "scheduled" | "running" | "paused"),
        }
    }
}

pub(crate) async fn job_action(id: &str, action: JobAction) -> Result<JobInfo, ApiError> {
    post(
        &format!("/api/jobs/{}/{}", enc(id), action.as_str()),
        &Value::Null,
    )
    .await
}

pub(crate) async fn delete_job(id: &str) -> Result<(), ApiError> {
    request(
        Method::Delete,
        &format!("/api/jobs/{}", enc(id)),
        Body::Empty,
    )
    .await
    .map(|_| ())
}

// ---------------------------------------------------------------- captures

pub(crate) async fn captures() -> Result<Vec<CaptureInfo>, ApiError> {
    get("/api/captures").await
}

pub(crate) async fn capture(id: &str) -> Result<CaptureInfo, ApiError> {
    get(&format!("/api/captures/{}", enc(id))).await
}

pub(crate) async fn create_capture(spec: &CaptureSpec) -> Result<CaptureInfo, ApiError> {
    post("/api/captures", spec).await
}

pub(crate) async fn stop_capture(id: &str) -> Result<CaptureInfo, ApiError> {
    post(&format!("/api/captures/{}/stop", enc(id)), &Value::Null).await
}

pub(crate) async fn delete_capture(id: &str) -> Result<(), ApiError> {
    request(
        Method::Delete,
        &format!("/api/captures/{}", enc(id)),
        Body::Empty,
    )
    .await
    .map(|_| ())
}

/// The first N matching frames of a capture.
pub(crate) async fn capture_packets(id: &str) -> Result<Vec<CapturedFrame>, ApiError> {
    get(&format!("/api/captures/{}/packets", enc(id))).await
}

/// URL of the pcapng download (a plain link downloads it).
pub(crate) fn capture_pcapng_url(id: &str) -> String {
    format!("/api/captures/{}/pcapng", enc(id))
}

/// `POST /api/captures/filter`: validates a capture filter.
pub(crate) async fn check_filter(filter: &str) -> Result<FilterCheck, ApiError> {
    post(
        "/api/captures/filter",
        &serde_json::json!({ "filter": filter }),
    )
    .await
}

pub(crate) async fn rx_backends() -> Result<Vec<RxBackendInfo>, ApiError> {
    get("/api/captures/backends").await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_errors() {
        let e = error_from_body(
            422,
            r#"{"error":{"code":"invalid_template","message":"bad","details":[{"level":"error","path":"layers[0]","key":"k","message":"m"}]}}"#,
            None,
        );
        assert_eq!(e.code(), "invalid_template");
        assert_eq!(e.status(), Some(422));
        assert_eq!(e.lint_issues().len(), 1);
        assert_eq!(e.lint_issues()[0].path, "layers[0]");

        let e = error_from_body(
            429,
            r#"{"error":{"code":"rate_limited","message":"slow down"}}"#,
            Some("17"),
        );
        assert_eq!(e.retry_after(), Some(17));
        let e = error_from_body(
            429,
            r#"{"error":{"code":"rate_limited","message":"x","details":{"retry_after_secs":5}}}"#,
            None,
        );
        assert_eq!(e.retry_after(), Some(5));

        let e = error_from_body(502, "Bad Gateway", None);
        assert_eq!(e.code(), "http");
        assert!(matches!(e, ApiError::Server { ref message, .. } if message == "Bad Gateway"));
    }

    #[test]
    fn urls() {
        assert_eq!(enc("a b/c"), "a%20b%2Fc");
        assert_eq!(enc("udp-src_sweep.1~"), "udp-src_sweep.1~");
        assert_eq!(enc("日"), "%E6%97%A5");
        assert_eq!(capture_pcapng_url("ab 1"), "/api/captures/ab%201/pcapng");
        assert_eq!(templates_url(&TemplateQuery::default()), "/api/templates");
        let q = TemplateQuery {
            q: " arp sweep ".into(),
            tag: Some("l2".into()),
            favorite: true,
            source: Some("user".into()),
        };
        assert_eq!(
            templates_url(&q),
            "/api/templates?q=arp%20sweep&tag=l2&favorite=true&source=user"
        );
    }

    #[test]
    fn job_actions() {
        assert!(JobAction::Start.allowed("created"));
        assert!(!JobAction::Start.allowed("running"));
        assert!(JobAction::Pause.allowed("running"));
        assert!(JobAction::Resume.allowed("paused"));
        assert!(JobAction::Stop.allowed("scheduled"));
        assert!(!JobAction::Stop.allowed("finished"));
    }
}
