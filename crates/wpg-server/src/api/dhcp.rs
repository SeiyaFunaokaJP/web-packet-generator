//! `/api/dhcp`: the DHCP server tool (ADR-0012). Settings (pools, options,
//! router advertisements), start / stop on the interface fixed at deploy time
//! (`dhcp.iface`) or chosen on the screen, leases and the activity log.

// utoipa's generated code spells out paths the lint considers redundant.
#![allow(unused_qualifications)]

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use utoipa::{IntoParams, ToSchema};
use wpg_dhcp::{Activity, Lease, Settings};

use crate::dhcp::{DhcpError, DhcpManager, DhcpStatus};
use crate::error::{codes, ApiError, ApiJson, ApiQuery, ApiResult};
use crate::state::AppState;

fn dhcp(st: &AppState) -> ApiResult<&DhcpManager> {
    if st.dhcp.enabled() {
        Ok(&st.dhcp)
    } else {
        Err(ApiError::new(
            StatusCode::NOT_FOUND,
            codes::NOT_FOUND,
            "api-error-dhcp-disabled",
        ))
    }
}

fn map_err(e: DhcpError) -> ApiError {
    match e {
        DhcpError::NoIface => ApiError::new(
            StatusCode::BAD_REQUEST,
            codes::BAD_REQUEST,
            "api-error-dhcp-no-iface",
        ),
        DhcpError::IfaceNotFound(i) => ApiError::new(
            StatusCode::BAD_REQUEST,
            codes::BAD_REQUEST,
            "api-error-dhcp-iface",
        )
        .arg("iface", i),
        DhcpError::Invalid(ref problems) => {
            let details = json!(problems
                .iter()
                .map(|p| json!({ "field": p.field, "message": p.message }))
                .collect::<Vec<_>>());
            ApiError::bad_request("api-error-dhcp-settings", &e).details(details)
        }
        DhcpError::Address { addr, source } => ApiError::new(
            StatusCode::BAD_GATEWAY,
            codes::BACKEND_ERROR,
            if source.kind() == std::io::ErrorKind::PermissionDenied {
                "api-error-dhcp-address-permission"
            } else {
                "api-error-dhcp-address"
            },
        )
        .arg("addr", addr)
        .arg("error", source),
        DhcpError::Socket(e) if e.kind() == std::io::ErrorKind::PermissionDenied => ApiError::new(
            StatusCode::BAD_GATEWAY,
            codes::BACKEND_ERROR,
            "api-error-dhcp-permission",
        )
        .arg("error", e),
        DhcpError::Socket(e) => ApiError::new(
            StatusCode::BAD_GATEWAY,
            codes::BACKEND_ERROR,
            "api-error-dhcp-start",
        )
        .arg("error", e),
        e @ DhcpError::Io { .. } => ApiError::internal(e),
    }
}

/// `PUT /api/dhcp` body.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct DhcpUpdate {
    /// Interface to answer on (ignored when `dhcp.iface` pins one; must be
    /// allowed by `tx.allowed_ifaces`).
    #[serde(default)]
    pub iface: Option<String>,
    /// Pools and options (`wpg_dhcp::Settings`: `v4`, `v6`).
    #[schema(value_type = Object)]
    pub settings: Settings,
}

/// `DELETE /api/dhcp/leases` query.
#[derive(Debug, Deserialize, IntoParams)]
pub struct LeaseQuery {
    /// The lease (`192.168.100.10`, `2001:db8::10`, `2001:db8:100::/56`);
    /// absent: every lease.
    #[serde(default)]
    pub key: Option<String>,
}

/// `GET /api/dhcp/activity` query.
#[derive(Debug, Deserialize, IntoParams)]
pub struct ActivityQuery {
    /// Most entries returned (newest first; default 200).
    #[serde(default)]
    pub limit: Option<usize>,
}

/// State, settings and counters.
#[utoipa::path(get, path = "/api/dhcp", tag = "dhcp",
    responses(
        (status = 200, description = "State", body = DhcpStatus),
        (status = 404, description = "`dhcp.enabled = false`", body = crate::error::ErrorBody)))]
pub async fn status(State(st): State<AppState>) -> ApiResult<Json<DhcpStatus>> {
    Ok(Json(dhcp(&st)?.status().await))
}

/// Saves the settings; a running server restarts with them.
#[utoipa::path(put, path = "/api/dhcp", tag = "dhcp", request_body = DhcpUpdate,
    responses(
        (status = 200, description = "Saved", body = DhcpStatus),
        (status = 400, description = "Invalid settings (`details`: `[{ field, message }]`)", body = crate::error::ErrorBody),
        (status = 403, description = "`policy_violation`: interface not in `tx.allowed_ifaces`", body = crate::error::ErrorBody),
        (status = 502, description = "`backend_error`: the restart failed", body = crate::error::ErrorBody)))]
pub async fn update(
    State(st): State<AppState>,
    ApiJson(req): ApiJson<DhcpUpdate>,
) -> ApiResult<Json<DhcpStatus>> {
    let m = dhcp(&st)?;
    let iface = if m.pinned_iface().is_some() {
        None
    } else {
        req.iface.map(|i| i.trim().to_owned())
    };
    if let Some(i) = iface.as_deref().filter(|i| !i.is_empty()) {
        if !st.iface_allowed(i) {
            return Err(ApiError::policy("api-error-dhcp-iface-policy").arg("iface", i));
        }
    }
    m.update(iface, req.settings)
        .await
        .map(Json)
        .map_err(map_err)
}

/// Starts (or restarts) the server; it also starts at boot until stopped.
#[utoipa::path(post, path = "/api/dhcp/start", tag = "dhcp",
    responses(
        (status = 200, description = "Running", body = DhcpStatus),
        (status = 400, description = "No / unknown interface, invalid settings", body = crate::error::ErrorBody),
        (status = 502, description = "`backend_error`: sockets (ports 67 / 547 need CAP_NET_BIND_SERVICE, RA CAP_NET_RAW; Linux only)", body = crate::error::ErrorBody)))]
pub async fn start(State(st): State<AppState>) -> ApiResult<Json<DhcpStatus>> {
    dhcp(&st)?.start().await.map(Json).map_err(map_err)
}

/// Stops the server (and its start at boot).
#[utoipa::path(post, path = "/api/dhcp/stop", tag = "dhcp",
    responses((status = 200, description = "Stopped", body = DhcpStatus)))]
pub async fn stop(State(st): State<AppState>) -> ApiResult<Json<DhcpStatus>> {
    let m = dhcp(&st)?;
    m.stop(true).await;
    Ok(Json(m.status().await))
}

/// Leases (`wpg_dhcp::Lease`: `kind` `v4` / `na` / `pd`, `addr`, `plen`,
/// `client`, `iaid`, `mac`, `hostname`, `state`, `expires`, `updated`).
#[utoipa::path(get, path = "/api/dhcp/leases", tag = "dhcp",
    responses((status = 200, description = "Leases", body = Vec<Object>)))]
pub async fn leases(State(st): State<AppState>) -> ApiResult<Json<Vec<Lease>>> {
    Ok(Json(dhcp(&st)?.leases()))
}

/// Deletes one lease (`key`) or all of them. The client keeps using the
/// address until it renews.
#[utoipa::path(delete, path = "/api/dhcp/leases", tag = "dhcp", params(LeaseQuery),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "No such lease", body = crate::error::ErrorBody)))]
pub async fn delete_leases(
    State(st): State<AppState>,
    ApiQuery(q): ApiQuery<LeaseQuery>,
) -> ApiResult<StatusCode> {
    let m = dhcp(&st)?;
    let key = q.key.as_deref().map(str::trim);
    if m.delete_leases(key) || key.is_none() {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found(format!(
            "lease {}",
            key.unwrap_or_default()
        )))
    }
}

/// Recent requests and replies, newest first (`wpg_dhcp::Activity`).
#[utoipa::path(get, path = "/api/dhcp/activity", tag = "dhcp", params(ActivityQuery),
    responses((status = 200, description = "Activity", body = Vec<Object>)))]
pub async fn activity(
    State(st): State<AppState>,
    ApiQuery(q): ApiQuery<ActivityQuery>,
) -> ApiResult<Json<Vec<Activity>>> {
    Ok(Json(dhcp(&st)?.activity(q.limit.unwrap_or(200))))
}

/// Clears the activity log.
#[utoipa::path(delete, path = "/api/dhcp/activity", tag = "dhcp",
    responses((status = 204, description = "Cleared")))]
pub async fn clear_activity(State(st): State<AppState>) -> ApiResult<StatusCode> {
    dhcp(&st)?.clear_activity();
    Ok(StatusCode::NO_CONTENT)
}
