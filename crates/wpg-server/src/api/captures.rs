//! `/api/captures`: receive counters, the first N frames, pcapng download and
//! signature verification (ROADMAP M9).

// utoipa's generated code spells out paths the lint considers redundant.
#![allow(unused_qualifications)]

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use wpg_rx::capture::SignatureInfo;
use wpg_rx::Filter;

use crate::api::packets::to_hex;
use crate::error::{ApiError, ApiJson, ApiPath, ApiResult};
use crate::rx::{CaptureInfo, CaptureSpec};
use crate::state::AppState;

/// All captures (running and the last finished ones), newest first.
#[utoipa::path(get, path = "/api/captures", tag = "captures",
    responses((status = 200, description = "Captures", body = Vec<CaptureInfo>)))]
pub async fn list(State(st): State<AppState>) -> Json<Vec<CaptureInfo>> {
    Json(st.rx.list())
}

/// Starts a capture.
#[utoipa::path(post, path = "/api/captures", tag = "captures", request_body = CaptureSpec,
    responses(
        (status = 201, description = "Started", body = CaptureInfo),
        (status = 400, description = "Invalid spec or filter", body = crate::error::ErrorBody),
        (status = 404, description = "Unknown interface / input file", body = crate::error::ErrorBody),
        (status = 502, description = "`backend_error`: the backend could not be opened", body = crate::error::ErrorBody)))]
pub async fn create(
    State(st): State<AppState>,
    ApiJson(spec): ApiJson<CaptureSpec>,
) -> ApiResult<(StatusCode, Json<CaptureInfo>)> {
    let info = st.rx.create(spec).await?;
    Ok((StatusCode::CREATED, Json(info)))
}

/// One capture.
#[utoipa::path(get, path = "/api/captures/{id}", tag = "captures",
    params(("id" = String, Path, description = "Capture id")),
    responses(
        (status = 200, description = "Capture", body = CaptureInfo),
        (status = 404, description = "Unknown id", body = crate::error::ErrorBody)))]
pub async fn get(
    State(st): State<AppState>,
    ApiPath(id): ApiPath<String>,
) -> ApiResult<Json<CaptureInfo>> {
    st.rx.get(&id).map(Json)
}

/// Stops a running capture (asynchronously; watch `capture_state`).
#[utoipa::path(post, path = "/api/captures/{id}/stop", tag = "captures",
    params(("id" = String, Path, description = "Capture id")),
    responses(
        (status = 200, description = "Stopping", body = CaptureInfo),
        (status = 404, description = "Unknown id", body = crate::error::ErrorBody),
        (status = 409, description = "Already ended", body = crate::error::ErrorBody)))]
pub async fn stop(
    State(st): State<AppState>,
    ApiPath(id): ApiPath<String>,
) -> ApiResult<Json<CaptureInfo>> {
    st.rx.stop(&id).map(Json)
}

/// Removes a capture (a running one is stopped first; a saved pcapng file stays
/// on disk).
#[utoipa::path(delete, path = "/api/captures/{id}", tag = "captures",
    params(("id" = String, Path, description = "Capture id")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "Unknown id", body = crate::error::ErrorBody)))]
pub async fn delete(
    State(st): State<AppState>,
    ApiPath(id): ApiPath<String>,
) -> ApiResult<StatusCode> {
    st.rx.delete(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// One kept frame.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CapturedFrame {
    /// Position among the matching frames (0-based).
    pub index: u64,
    /// Capture time, nanoseconds since the Unix epoch.
    pub ts_ns: u64,
    /// Original length.
    pub len: usize,
    /// Captured octets as lower-case hex (decode with `POST /api/parse`).
    pub hex: String,
    /// The frame's signature (`stream_id`, `seq`, `ts_ns`), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub signature: Option<SignatureInfo>,
}

/// The first N matching frames of a capture (live while it runs).
#[utoipa::path(get, path = "/api/captures/{id}/packets", tag = "captures",
    params(("id" = String, Path, description = "Capture id")),
    responses(
        (status = 200, description = "Kept frames", body = Vec<CapturedFrame>),
        (status = 404, description = "Unknown id", body = crate::error::ErrorBody)))]
pub async fn packets(
    State(st): State<AppState>,
    ApiPath(id): ApiPath<String>,
) -> ApiResult<Json<Vec<CapturedFrame>>> {
    Ok(Json(
        st.rx
            .packets(&id)?
            .into_iter()
            .map(|p| CapturedFrame {
                index: p.index,
                ts_ns: p.ts_ns,
                len: p.len,
                hex: to_hex(&p.data),
                signature: p.signature,
            })
            .collect(),
    ))
}

/// The pcapng file of a finished capture started with `save`.
#[utoipa::path(get, path = "/api/captures/{id}/pcapng", tag = "captures",
    params(("id" = String, Path, description = "Capture id")),
    responses(
        (status = 200, description = "pcapng file", content_type = "application/octet-stream"),
        (status = 404, description = "Unknown id or nothing saved", body = crate::error::ErrorBody),
        (status = 409, description = "Still running", body = crate::error::ErrorBody)))]
pub async fn pcapng(
    State(st): State<AppState>,
    ApiPath(id): ApiPath<String>,
) -> ApiResult<Response> {
    let path = st.rx.pcapng_path(&id)?;
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| ApiError::not_found(format!("pcapng of capture {id}")))?;
    let mut resp = Body::from(bytes).into_response();
    let h = resp.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    let file = path.file_name().map_or_else(
        || format!("{id}.pcapng"),
        |f| f.to_string_lossy().into_owned(),
    );
    if let Ok(v) = HeaderValue::from_str(&format!(
        "attachment; filename=\"{}\"",
        file.replace(['"', '\\'], "_")
    )) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    Ok(resp)
}

/// `POST /api/captures/filter` request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct FilterCheck {
    /// Filter text.
    pub filter: String,
}

/// `POST /api/captures/filter` response.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct FilterCheckResult {
    /// The filter is valid.
    pub ok: bool,
    /// Normalised filter text (when valid).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized: Option<String>,
    /// What is wrong (when invalid).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Byte offset of the error (when invalid).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<usize>,
}

/// Checks a capture filter without starting a capture.
#[utoipa::path(post, path = "/api/captures/filter", tag = "captures", request_body = FilterCheck,
    responses((status = 200, description = "Result", body = FilterCheckResult)))]
pub async fn check_filter(ApiJson(req): ApiJson<FilterCheck>) -> Json<FilterCheckResult> {
    Json(match Filter::parse(&req.filter) {
        Ok(f) => FilterCheckResult {
            ok: true,
            normalized: Some(f.text().to_owned()),
            error: None,
            pos: None,
        },
        Err(e) => FilterCheckResult {
            ok: false,
            normalized: None,
            error: Some(e.message),
            pos: Some(e.pos),
        },
    })
}

/// Receive backend availability on this build / OS.
#[utoipa::path(get, path = "/api/captures/backends", tag = "captures",
    responses((status = 200, description = "Backends (`name`, `available`, `reason`)", body = Vec<Object>)))]
pub async fn backends() -> Json<Vec<wpg_rx::backend::BackendInfo>> {
    Json(wpg_rx::backend::available())
}
