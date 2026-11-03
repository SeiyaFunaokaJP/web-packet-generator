//! `/api/protocols`: the protocol catalog (IR for GUI generation).

// utoipa's generated code spells out paths the lint considers redundant.
#![allow(unused_qualifications)]

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use crate::auth::Principal;
use crate::db::Role;
use crate::error::{codes, ApiError, ApiPath, ApiResult};
use crate::events::ServerEvent;
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
        st.catalog()
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

/// The user-defined protocol files in use (`[protocols] dirs`,
/// `--protocols-dir`; loaded at startup or by `POST /api/protocols/reload`):
/// their definitions and bindings, so that the web UI can add them to its
/// in-browser catalog. Empty without extensions.
#[utoipa::path(get, path = "/api/protocols/extensions", tag = "protocols",
    responses((status = 200, description = "Extension files (`protocols`, `bind`, `source`)", body = Vec<Object>)))]
pub async fn extensions() -> ApiResult<Json<Value>> {
    let v =
        serde_json::to_value(wpg_protocols::extension::extensions()).map_err(ApiError::internal)?;
    Ok(Json(v))
}

/// One loaded extension file.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ExtensionFile {
    /// The file.
    pub source: String,
    /// Ids of the protocols it defines.
    pub protocols: Vec<String>,
    /// Number of its `bind` entries.
    pub bindings: usize,
}

/// Result of `POST /api/protocols/reload`.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ReloadResult {
    /// Directories that were read.
    pub dirs: Vec<String>,
    /// The files now in use, in load order.
    pub files: Vec<ExtensionFile>,
}

/// Reads the user-defined protocol directories of the server again
/// (`[protocols] dirs`, `--protocols-dir`) and switches the catalog to what
/// they hold now; WebSocket clients get a `protocols_changed` event. Jobs that
/// are already running keep the catalog they started with. On an error nothing
/// changes. Admin only.
#[utoipa::path(post, path = "/api/protocols/reload", tag = "protocols",
    responses(
        (status = 200, description = "Loaded files", body = ReloadResult),
        (status = 403, description = "`forbidden`: not an admin", body = crate::error::ErrorBody),
        (status = 422, description = "`invalid_protocols`: a file does not load (`details.file`, `details.reason`)", body = crate::error::ErrorBody)))]
pub async fn reload(
    State(st): State<AppState>,
    principal: Option<Extension<Principal>>,
) -> ApiResult<Json<ReloadResult>> {
    if principal.is_some_and(|Extension(p)| p.role != Role::Admin) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            codes::FORBIDDEN,
            "api-error-protocols-forbidden",
        ));
    }
    let result = tokio::task::spawn_blocking(wpg_protocols::extension::reload)
        .await
        .map_err(ApiError::internal)?;
    let exts = match result {
        Ok(exts) => exts,
        Err(e) => {
            tracing::warn!("user-defined protocols not reloaded: {e}");
            return Err(ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                codes::INVALID_PROTOCOLS,
                "api-error-protocols-reload",
            )
            .arg("file", &e.source)
            .arg("error", &e.reason)
            .details(serde_json::json!({ "file": e.source, "reason": e.reason })));
        }
    };
    let files: Vec<ExtensionFile> = exts
        .iter()
        .map(|e| ExtensionFile {
            source: e.source.clone(),
            protocols: e.protocols.iter().map(|d| d.id.clone()).collect(),
            bindings: e.bind.len(),
        })
        .collect();
    tracing::info!(
        files = files.len(),
        protocols = files.iter().map(|f| f.protocols.len()).sum::<usize>(),
        "user-defined protocols reloaded"
    );
    let _ = st.events.send(ServerEvent::ProtocolsChanged);
    Ok(Json(ReloadResult {
        dirs: wpg_protocols::extension::dirs()
            .iter()
            .map(|d| d.display().to_string())
            .collect(),
        files,
    }))
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
        .catalog()
        .get(&id)
        .ok_or_else(|| ApiError::not_found(format!("protocol {id}")))?;
    let mut v = serde_json::to_value(def).map_err(ApiError::internal)?;
    add_descriptions(&mut v, request_locale(&headers));
    Ok(Json(v))
}
