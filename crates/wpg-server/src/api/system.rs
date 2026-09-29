//! `/api/health`, `/api/version`, `/api/system/*`.

// utoipa's generated code spells out paths the lint considers redundant.
#![allow(unused_qualifications)]

use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::{IntoParams, ToSchema};
use wpg_i18n::{t, t_args, FluentArgs};
use wpg_sys::doctor;

use crate::error::{ApiError, ApiQuery, ApiResult};
use crate::localize::request_locale;
use crate::state::AppState;
use crate::VERSION;

/// `GET /api/health` response body.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Health {
    /// Always `"ok"` when the server answers.
    pub status: String,
    /// Server version.
    pub version: String,
    /// Negotiated locale.
    pub locale: String,
    /// Localised human-readable status.
    pub message: String,
    /// Seconds since the server started.
    pub uptime_secs: u64,
}

/// Liveness probe.
#[utoipa::path(get, path = "/api/health", tag = "system",
    responses((status = 200, description = "The server is up", body = Health)))]
pub async fn health(State(st): State<AppState>, headers: HeaderMap) -> Json<Health> {
    let locale = request_locale(&headers);
    Json(Health {
        status: "ok".to_owned(),
        version: VERSION.to_owned(),
        locale: locale.as_str().to_owned(),
        message: t(locale, "health-ok"),
        uptime_secs: st.started_at.elapsed().as_secs(),
    })
}

/// `GET /api/version` response body.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VersionInfo {
    /// Server version.
    pub version: String,
    /// REST API version (bumped on incompatible changes).
    pub api_version: u32,
    /// Target OS.
    pub os: String,
    /// Target CPU architecture.
    pub arch: String,
    /// Optional features compiled in (`pcap`, ...).
    pub features: Vec<String>,
    /// Supported UI / message languages.
    pub locales: Vec<String>,
}

/// Build information.
#[utoipa::path(get, path = "/api/version", tag = "system",
    responses((status = 200, description = "Version and build info", body = VersionInfo)))]
pub async fn version() -> Json<VersionInfo> {
    let mut features = Vec::new();
    if cfg!(feature = "pcap") {
        features.push("pcap".to_owned());
    }
    Json(VersionInfo {
        version: VERSION.to_owned(),
        api_version: 1,
        os: std::env::consts::OS.to_owned(),
        arch: std::env::consts::ARCH.to_owned(),
        features,
        locales: wpg_i18n::Locale::ALL
            .iter()
            .map(|l| l.as_str().to_owned())
            .collect(),
    })
}

/// Query of `GET /api/system/ifaces`.
#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct IfacesQuery {
    /// Re-read the interfaces instead of using the few-seconds cache.
    #[serde(default)]
    pub refresh: bool,
}

/// Network interfaces with `usable` (up, running, not loopback) and `allowed`
/// (by `tx.allowed_ifaces`).
#[utoipa::path(get, path = "/api/system/ifaces", tag = "system", params(IfacesQuery),
    responses(
        (status = 200, description = "Interfaces (`wpg ifaces --json` plus `usable` / `allowed`)", body = Vec<Object>),
        (status = 500, description = "Enumeration failed", body = crate::error::ErrorBody)))]
pub async fn ifaces(
    State(st): State<AppState>,
    ApiQuery(q): ApiQuery<IfacesQuery>,
) -> ApiResult<Json<Vec<Value>>> {
    let list = tokio::task::spawn_blocking({
        let st = st.clone();
        move || st.ifaces(q.refresh)
    })
    .await
    .map_err(ApiError::internal)?
    .map_err(ApiError::internal)?;
    Ok(Json(
        list.into_iter()
            .map(|i| {
                let mut v = serde_json::to_value(&i).unwrap_or(Value::Null);
                if let Value::Object(m) = &mut v {
                    m.insert("usable".into(), i.is_usable().into());
                    m.insert("allowed".into(), st.iface_allowed(&i.name).into());
                }
                v
            })
            .collect(),
    ))
}

/// Self-diagnosis (`wpg doctor --json`), with localised `summary` / `fix`.
#[utoipa::path(get, path = "/api/system/doctor", tag = "system",
    responses((status = 200, description = "`{ ok, locale, checks: [...] }`", body = Object)))]
pub async fn doctor(headers: HeaderMap) -> ApiResult<Json<Value>> {
    let locale = request_locale(&headers);
    let report = tokio::task::spawn_blocking(doctor::run)
        .await
        .map_err(ApiError::internal)?;
    let checks: Vec<Value> = report
        .checks
        .iter()
        .map(|c| {
            let mut v = serde_json::to_value(c).unwrap_or(Value::Null);
            if let Value::Object(m) = &mut v {
                m.insert("summary".into(), Value::String(t(locale, c.summary_key)));
                let fix = c.fix_key.map(|k| {
                    let mut a = FluentArgs::new();
                    a.set("hint", c.fix_hint.clone().unwrap_or_default());
                    t_args(locale, k, &a)
                });
                m.insert("fix".into(), fix.map_or(Value::Null, Value::String));
            }
            v
        })
        .collect();
    Ok(Json(json!({
        "ok": !report.has_failures(),
        "locale": locale.as_str(),
        "checks": checks,
    })))
}

/// One transmit backend.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BackendItem {
    /// Name (`null`, `pcapfile`, `afpacket`, `pcap`, `afxdp`, `dpdk`).
    pub name: String,
    /// Compiled in and supported on this OS (privileges not checked).
    pub available: bool,
    /// Why it is unavailable / requirements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Several TX workers per job.
    pub multi_worker: bool,
}

/// `GET /api/system/backends` response body.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Backends {
    /// Backend used when a job names none.
    pub default: String,
    /// All backends.
    pub backends: Vec<BackendItem>,
}

/// Transmit backends of this build.
#[utoipa::path(get, path = "/api/system/backends", tag = "system",
    responses((status = 200, description = "Backends", body = Backends)))]
pub async fn backends(State(st): State<AppState>) -> Json<Backends> {
    Json(Backends {
        default: st.config.default_backend(),
        backends: wpg_tx::backend::available()
            .into_iter()
            .map(|b| BackendItem {
                multi_worker: wpg_tx::backend::supports_workers(&b.name),
                name: b.name,
                available: b.available,
                reason: b.reason,
            })
            .collect(),
    })
}
