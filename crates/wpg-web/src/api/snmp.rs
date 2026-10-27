//! Client of `/api/snmp` (the SNMP tool screen). Mirrors the response types of
//! `wpg_server::api::snmp` / `wpg_server::snmp` with lenient defaults.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{enc, get, json, post, request, ApiError, Body, Method};

/// `GET /api/snmp/status`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub(crate) struct SnmpStatus {
    #[serde(default)]
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) mib_dir: String,
    #[serde(default)]
    pub(crate) modules: usize,
    #[serde(default)]
    pub(crate) nodes: usize,
    #[serde(default)]
    pub(crate) receiver: Option<ReceiverStatus>,
    #[serde(default)]
    pub(crate) capture: Option<CaptureStatus>,
}

/// A rendered variable binding.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub(crate) struct VarBindOut {
    #[serde(default)]
    pub(crate) oid: String,
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) module: Option<String>,
    #[serde(default)]
    pub(crate) object: Option<String>,
    #[serde(default)]
    pub(crate) instance: String,
    #[serde(default, rename = "type")]
    pub(crate) value_type: String,
    #[serde(default)]
    pub(crate) syntax: Option<String>,
    #[serde(default)]
    pub(crate) value: String,
    #[serde(default)]
    pub(crate) raw: String,
    #[serde(default)]
    pub(crate) numeric: Option<u64>,
    #[serde(default)]
    pub(crate) units: Option<String>,
}

impl VarBindOut {
    /// Counter types (rates make sense).
    pub(crate) fn is_counter(&self) -> bool {
        matches!(self.value_type.as_str(), "Counter32" | "Counter64")
    }
}

/// A table column.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub(crate) struct TableColumn {
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) oid: String,
    #[serde(default)]
    pub(crate) syntax: String,
    #[serde(default)]
    pub(crate) counter: bool,
}

/// A table row.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub(crate) struct TableRow {
    #[serde(default)]
    pub(crate) instance: String,
    #[serde(default)]
    pub(crate) index: Vec<String>,
    #[serde(default)]
    pub(crate) cells: Vec<Option<String>>,
    #[serde(default)]
    pub(crate) numeric: Vec<Option<u64>>,
}

/// A conceptual table assembled by the server.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub(crate) struct TableOut {
    #[serde(default)]
    pub(crate) entry: String,
    #[serde(default)]
    pub(crate) table: Option<String>,
    #[serde(default)]
    pub(crate) index_names: Vec<String>,
    #[serde(default)]
    pub(crate) columns: Vec<TableColumn>,
    #[serde(default)]
    pub(crate) rows: Vec<TableRow>,
}

/// Result of an operation.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub(crate) struct SnmpResult {
    #[serde(default)]
    pub(crate) op: String,
    #[serde(default)]
    pub(crate) target: String,
    #[serde(default)]
    pub(crate) version: String,
    #[serde(default)]
    pub(crate) elapsed_ms: f64,
    #[serde(default)]
    pub(crate) error_status: i32,
    #[serde(default)]
    pub(crate) error_status_name: Option<String>,
    #[serde(default)]
    pub(crate) error_index: i32,
    #[serde(default)]
    pub(crate) varbinds: Vec<VarBindOut>,
    #[serde(default)]
    pub(crate) tables: Vec<TableOut>,
    #[serde(default)]
    pub(crate) truncated: bool,
    #[serde(default)]
    pub(crate) requests: u32,
    #[serde(default)]
    pub(crate) engine_id: Option<String>,
}

/// An autocompletion hit.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub(crate) struct SearchHit {
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) module: String,
    #[serde(default)]
    pub(crate) oid: String,
    #[serde(default)]
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) role: Option<String>,
    #[serde(default)]
    pub(crate) syntax: Option<String>,
}

/// A loaded MIB module.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub(crate) struct MibModule {
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) bundled: bool,
    #[serde(default)]
    pub(crate) file: Option<String>,
    #[serde(default)]
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) missing_imports: Vec<String>,
    #[serde(default)]
    pub(crate) errors: Vec<String>,
    #[serde(default)]
    pub(crate) nodes: usize,
}

/// Per-file upload result.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub(crate) struct UploadedFile {
    #[serde(default)]
    pub(crate) file: String,
    #[serde(default)]
    pub(crate) modules: Vec<String>,
    #[serde(default)]
    pub(crate) errors: Vec<String>,
    #[serde(default)]
    pub(crate) stored: bool,
}

/// `POST /api/snmp/mibs` result.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub(crate) struct MibUploadResult {
    #[serde(default)]
    pub(crate) files: Vec<UploadedFile>,
    #[serde(default)]
    pub(crate) modules: Vec<MibModule>,
}

/// One tree entry.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub(crate) struct TreeChild {
    #[serde(default)]
    pub(crate) oid: String,
    #[serde(default)]
    pub(crate) arc: u32,
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) module: Option<String>,
    #[serde(default)]
    pub(crate) kind: Option<String>,
    #[serde(default)]
    pub(crate) role: Option<String>,
    #[serde(default)]
    pub(crate) has_children: bool,
}

/// Enumeration value.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub(crate) struct NamedNumber {
    #[serde(default)]
    pub(crate) value: i64,
    #[serde(default)]
    pub(crate) label: String,
}

/// Breadcrumb element.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub(crate) struct PathElem {
    #[serde(default)]
    pub(crate) oid: String,
    #[serde(default)]
    pub(crate) name: String,
}

/// MIB node details.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub(crate) struct NodeInfo {
    #[serde(default)]
    pub(crate) oid: String,
    #[serde(default)]
    pub(crate) instance: String,
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) module: String,
    #[serde(default)]
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) role: Option<String>,
    #[serde(default)]
    pub(crate) syntax: Option<String>,
    #[serde(default)]
    pub(crate) display_hint: Option<String>,
    #[serde(default)]
    pub(crate) enums: Vec<NamedNumber>,
    #[serde(default)]
    pub(crate) access: Option<String>,
    #[serde(default)]
    pub(crate) status: Option<String>,
    #[serde(default)]
    pub(crate) units: Option<String>,
    #[serde(default)]
    pub(crate) description: Option<String>,
    #[serde(default)]
    pub(crate) reference: Option<String>,
    #[serde(default)]
    pub(crate) defval: Option<String>,
    #[serde(default)]
    pub(crate) index: Vec<String>,
    #[serde(default)]
    pub(crate) augments: Option<String>,
    #[serde(default)]
    pub(crate) objects: Vec<String>,
    #[serde(default)]
    pub(crate) path: Vec<PathElem>,
}

/// Receiver state.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub(crate) struct ReceiverStatus {
    #[serde(default)]
    pub(crate) running: bool,
    #[serde(default)]
    pub(crate) listen: Option<String>,
    #[serde(default)]
    pub(crate) communities: Vec<String>,
    #[serde(default)]
    pub(crate) users: Vec<String>,
    #[serde(default)]
    pub(crate) engine_id: String,
    #[serde(default)]
    pub(crate) received: u64,
    #[serde(default)]
    pub(crate) from_config: bool,
}

/// v1 trap header.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub(crate) struct V1Header {
    #[serde(default)]
    pub(crate) enterprise: String,
    #[serde(default)]
    pub(crate) agent_addr: String,
    #[serde(default)]
    pub(crate) generic: i32,
    #[serde(default)]
    pub(crate) specific: i32,
}

/// A received notification.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub(crate) struct TrapOut {
    #[serde(default)]
    pub(crate) id: u64,
    #[serde(default)]
    pub(crate) received_ms: u64,
    #[serde(default)]
    pub(crate) source: String,
    #[serde(default)]
    pub(crate) version: String,
    #[serde(default)]
    pub(crate) pdu_type: Option<String>,
    #[serde(default)]
    pub(crate) community: Option<String>,
    #[serde(default)]
    pub(crate) user: Option<String>,
    #[serde(default)]
    pub(crate) level: Option<String>,
    #[serde(default)]
    pub(crate) trap_oid: Option<String>,
    #[serde(default)]
    pub(crate) trap_name: Option<String>,
    #[serde(default)]
    pub(crate) uptime_text: Option<String>,
    #[serde(default)]
    pub(crate) v1: Option<V1Header>,
    #[serde(default)]
    pub(crate) varbinds: Vec<VarBindOut>,
    #[serde(default)]
    pub(crate) acknowledged: bool,
    #[serde(default)]
    pub(crate) error: Option<String>,
}

/// Recording state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub(crate) struct CaptureStatus {
    #[serde(default)]
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) frames: usize,
    #[serde(default)]
    pub(crate) dropped: u64,
}

/// A MIB file to upload.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct MibFileIn {
    pub(crate) name: String,
    pub(crate) content: String,
}

pub(crate) async fn status() -> Result<SnmpStatus, ApiError> {
    get("/api/snmp/status").await
}

/// `POST /api/snmp/{op}` (`get`, `getnext`, `getbulk`, `walk`, `set`, `trap`,
/// `inform`).
pub(crate) async fn operation(op: &str, body: &Value) -> Result<SnmpResult, ApiError> {
    post(&format!("/api/snmp/{}", enc(op)), body).await
}

pub(crate) async fn search(q: &str, limit: usize) -> Result<Vec<SearchHit>, ApiError> {
    get(&format!("/api/snmp/mibs/search?q={}&limit={limit}", enc(q))).await
}

pub(crate) async fn mibs() -> Result<Vec<MibModule>, ApiError> {
    get("/api/snmp/mibs").await
}

pub(crate) async fn upload(files: &[MibFileIn]) -> Result<MibUploadResult, ApiError> {
    post("/api/snmp/mibs", &serde_json::json!({ "files": files })).await
}

pub(crate) async fn delete_mib(module: &str) -> Result<(), ApiError> {
    request(
        Method::Delete,
        &format!("/api/snmp/mibs/{}", enc(module)),
        Body::Empty,
    )
    .await
    .map(|_| ())
}

pub(crate) async fn tree(oid: &str) -> Result<Vec<TreeChild>, ApiError> {
    if oid.is_empty() {
        get("/api/snmp/mibs/tree").await
    } else {
        get(&format!("/api/snmp/mibs/tree?oid={}", enc(oid))).await
    }
}

pub(crate) async fn node(id: &str) -> Result<NodeInfo, ApiError> {
    get(&format!("/api/snmp/mibs/node?id={}", enc(id))).await
}

pub(crate) async fn receiver() -> Result<ReceiverStatus, ApiError> {
    get("/api/snmp/receiver").await
}

pub(crate) async fn receiver_start(body: &Value) -> Result<ReceiverStatus, ApiError> {
    post("/api/snmp/receiver/start", body).await
}

pub(crate) async fn receiver_stop() -> Result<ReceiverStatus, ApiError> {
    post("/api/snmp/receiver/stop", &Value::Null).await
}

pub(crate) async fn traps() -> Result<Vec<TrapOut>, ApiError> {
    get("/api/snmp/traps").await
}

pub(crate) async fn clear_traps() -> Result<(), ApiError> {
    request(Method::Delete, "/api/snmp/traps", Body::Empty)
        .await
        .map(|_| ())
}

pub(crate) async fn capture() -> Result<CaptureStatus, ApiError> {
    get("/api/snmp/capture").await
}

pub(crate) async fn capture_control(
    enabled: Option<bool>,
    clear: bool,
) -> Result<CaptureStatus, ApiError> {
    let resp = request(
        Method::Post,
        "/api/snmp/capture",
        super::body_of(&serde_json::json!({ "enabled": enabled, "clear": clear }))?,
    )
    .await?;
    json(resp).await
}

/// URL of the recording download (a plain link downloads it).
pub(crate) fn capture_url(format: &str) -> String {
    format!("/api/snmp/capture/download?format={}", enc(format))
}
