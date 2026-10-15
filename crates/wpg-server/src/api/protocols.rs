//! `/api/protocols`: the protocol catalog (IR for GUI generation).

// utoipa's generated code spells out paths the lint considers redundant.
#![allow(unused_qualifications)]

use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use crate::error::{ApiError, ApiPath, ApiResult};
use crate::localize::{add_descriptions, request_locale};
use crate::state::AppState;

/// Catalog entry.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ProtocolSummary {
    /// Stable id (`ethernet2`, `ipv4`, ...).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Palette classification (`l2`, `l3`, `l4`, `payload`, ...).
    #[schema(value_type = Object)]
    pub layer: Value,
    /// Field names in wire order.
    pub fields: Vec<String>,
    /// Protocols this one can carry (its next-layer binding table).
    pub next: Vec<String>,
    /// Content generator instead of wire fields (payload).
    pub synth: bool,
}

/// The catalog in palette order.
#[utoipa::path(get, path = "/api/protocols", tag = "protocols",
    responses((status = 200, description = "Catalog summary", body = Vec<ProtocolSummary>)))]
pub async fn list(State(st): State<AppState>) -> Json<Vec<ProtocolSummary>> {
    Json(
        st.catalog
            .iter()
            .map(|p| {
                let mut next: Vec<String> = p
                    .next
                    .iter()
                    .flat_map(|b| {
                        b.table
                            .iter()
                            .map(|e| e.protocol.clone())
                            .chain(b.default.clone())
                    })
                    .collect();
                next.dedup();
                ProtocolSummary {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    layer: serde_json::to_value(p.layer).unwrap_or(Value::Null),
                    fields: p.fields.iter().map(|f| f.name.clone()).collect(),
                    next,
                    synth: p.synth.is_some(),
                }
            })
            .collect(),
    )
}

/// The user-defined protocol files loaded at startup (`[protocols] dirs`,
/// `--protocols-dir`): their definitions and bindings, so that the web UI can
/// add them to its in-browser catalog. Empty without extensions.
#[utoipa::path(get, path = "/api/protocols/extensions", tag = "protocols",
    responses((status = 200, description = "Extension files (`protocols`, `bind`, `source`)", body = Vec<Object>)))]
pub async fn extensions() -> ApiResult<Json<Value>> {
    let v =
        serde_json::to_value(wpg_protocols::extension::extensions()).map_err(ApiError::internal)?;
    Ok(Json(v))
}

/// The full definition (`wpg_core::ProtocolDef`): fields with types, defaults,
/// auto computations and bindings. Every `description_key` gets a localised
/// `description` next to it.
#[utoipa::path(get, path = "/api/protocols/{id}", tag = "protocols",
    params(("id" = String, Path, description = "Protocol id")),
    responses(
        (status = 200, description = "Protocol definition IR", body = Object),
        (status = 404, description = "Unknown protocol", body = crate::error::ErrorBody)))]
pub async fn get(
    State(st): State<AppState>,
    headers: HeaderMap,
    ApiPath(id): ApiPath<String>,
) -> ApiResult<Json<Value>> {
    let def = st
        .catalog
        .get(&id)
        .ok_or_else(|| ApiError::not_found(format!("protocol {id}")))?;
    let mut v = serde_json::to_value(def).map_err(ApiError::internal)?;
    add_descriptions(&mut v, request_locale(&headers));
    Ok(Json(v))
}
