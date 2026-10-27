//! `/api/snmp`: the SNMP manager tool (ADR-0010). Operations against agents
//! (Get / GetNext / GetBulk / Walk / Set, Trap / Inform), MIB management and
//! browsing, the trap receiver and the pcap recording.
//!
//! Pass phrases of SNMPv3 users travel in the request bodies only: they are
//! used for the request (or the running receiver) and never stored or logged.

// utoipa's generated code spells out paths the lint considers redundant.
#![allow(unused_qualifications)]

use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::{IntoParams, ToSchema};
use wpg_snmp::client::{Client, ClientConfig, Error as ClientError};
use wpg_snmp::mib::{build_tables, format_varbind, BaseType, Registry, Table};
use wpg_snmp::pcap::PcapFormat;
use wpg_snmp::pdu::{error_status_name, Pdu, Version};
use wpg_snmp::usm::{AuthProtocol, PrivProtocol, UsmError, UsmUser};
use wpg_snmp::{Oid, Value, VarBind};

use crate::error::{codes, ApiError, ApiJson, ApiPath, ApiQuery, ApiResult};
use crate::snmp::{
    CaptureStatus, MibError, ReceiverStatus, SnmpManager, TrapOut, UploadedFile, VarBindOut,
};
use crate::state::AppState;

/// Longest per-attempt timeout accepted.
const MAX_TIMEOUT_MS: u64 = 60_000;
/// Most retries accepted.
const MAX_RETRIES: u32 = 10;
/// Most bindings a walk may return.
const MAX_WALK: usize = 100_000;

fn snmp(st: &AppState) -> ApiResult<&SnmpManager> {
    if st.snmp.enabled() {
        Ok(&st.snmp)
    } else {
        Err(ApiError::new(
            StatusCode::NOT_FOUND,
            codes::NOT_FOUND,
            "api-error-snmp-disabled",
        ))
    }
}

/// SNMP protocol version.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SnmpVersion {
    /// SNMPv1.
    V1,
    /// SNMPv2c.
    #[default]
    V2c,
    /// SNMPv3 (USM).
    V3,
}

impl From<SnmpVersion> for Version {
    fn from(v: SnmpVersion) -> Self {
        match v {
            SnmpVersion::V1 => Version::V1,
            SnmpVersion::V2c => Version::V2c,
            SnmpVersion::V3 => Version::V3,
        }
    }
}

/// SNMPv3 parameters. The level follows from the pass phrases given:
/// none = noAuthNoPriv, `auth_password` = authNoPriv, both = authPriv.
#[derive(Clone, Default, Deserialize, ToSchema)]
pub struct V3In {
    /// User name.
    pub user: String,
    /// `MD5`, `SHA` (SHA-1), `SHA-224`, `SHA-256`, `SHA-384`, `SHA-512`
    /// (default `SHA`).
    #[serde(default)]
    pub auth_protocol: Option<String>,
    /// Authentication pass phrase (at least 8 characters).
    #[serde(default)]
    pub auth_password: Option<String>,
    /// `DES`, `AES` (AES-128), `AES-192`, `AES-256` (default `AES`).
    #[serde(default)]
    pub priv_protocol: Option<String>,
    /// Privacy pass phrase (at least 8 characters).
    #[serde(default)]
    pub priv_password: Option<String>,
    /// `contextName`.
    #[serde(default)]
    pub context_name: Option<String>,
    /// `contextEngineID` (hex; default: the agent's engine id).
    #[serde(default)]
    pub context_engine_id: Option<String>,
}

impl std::fmt::Debug for V3In {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("V3In")
            .field("user", &self.user)
            .field("auth_protocol", &self.auth_protocol)
            .field("priv_protocol", &self.priv_protocol)
            .finish_non_exhaustive()
    }
}

impl V3In {
    fn to_user(&self) -> ApiResult<UsmUser> {
        let usm = |e: UsmError| ApiError::bad_request("api-error-snmp-usm", e);
        let filled = |s: &Option<String>| s.as_deref().filter(|p| !p.is_empty()).map(str::to_owned);
        let auth = match filled(&self.auth_password) {
            Some(pw) => {
                let p = self
                    .auth_protocol
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("SHA");
                Some((p.parse::<AuthProtocol>().map_err(usm)?, pw))
            }
            None => None,
        };
        let privacy = match filled(&self.priv_password) {
            Some(pw) => {
                let p = self
                    .priv_protocol
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("AES");
                Some((p.parse::<PrivProtocol>().map_err(usm)?, pw))
            }
            None => None,
        };
        let u = UsmUser {
            name: self.user.clone(),
            auth,
            privacy,
        };
        u.validate().map_err(usm)?;
        if u.name.is_empty() {
            return Err(ApiError::bad_request(
                "api-error-snmp-usm",
                "user name is empty",
            ));
        }
        Ok(u)
    }
}

/// Where to send a request.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SnmpTarget {
    /// Host name or address.
    pub host: String,
    /// UDP port (default 161 for requests, 162 for notifications).
    #[serde(default)]
    pub port: Option<u16>,
    /// Version (default `v2c`).
    #[serde(default)]
    pub version: SnmpVersion,
    /// Community for v1 / v2c (default `public`).
    #[serde(default)]
    pub community: Option<String>,
    /// SNMPv3 parameters.
    #[serde(default)]
    pub v3: Option<V3In>,
    /// Wait per attempt in milliseconds (default 2000, at most 60000).
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Extra attempts (default 1, at most 10).
    #[serde(default)]
    pub retries: Option<u32>,
}

async fn resolve_addr(host: &str, port: u16) -> ApiResult<SocketAddr> {
    let h = host.trim().trim_start_matches('[').trim_end_matches(']');
    if h.is_empty() {
        return Err(ApiError::bad_request("api-error-snmp-target", "empty host"));
    }
    if let Ok(ip) = h.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, port));
    }
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host((h, port))
        .await
        .map_err(|e| ApiError::bad_request("api-error-snmp-target", format!("{h}: {e}")))?
        .collect();
    addrs
        .iter()
        .find(|a| a.is_ipv4())
        .or_else(|| addrs.first())
        .copied()
        .ok_or_else(|| ApiError::bad_request("api-error-snmp-target", h))
}

async fn connect(
    m: &SnmpManager,
    t: &SnmpTarget,
    default_port: u16,
) -> ApiResult<(Client, SocketAddr)> {
    let addr = resolve_addr(&t.host, t.port.unwrap_or(default_port)).await?;
    let mut cfg = ClientConfig::v2c(addr, t.community.as_deref().unwrap_or("public"));
    cfg.version = t.version.into();
    cfg.timeout = Duration::from_millis(t.timeout_ms.unwrap_or(2000).clamp(50, MAX_TIMEOUT_MS));
    cfg.retries = t.retries.unwrap_or(1).min(MAX_RETRIES);
    cfg.local_engine_id = Some(m.engine_id().to_vec());
    if t.version == SnmpVersion::V3 {
        let v3 = t.v3.as_ref().ok_or_else(|| {
            ApiError::bad_request("api-error-snmp-usm", "missing `v3` parameters")
        })?;
        cfg.user = Some(v3.to_user()?);
        cfg.context_name = v3.context_name.clone().unwrap_or_default();
        if let Some(e) = v3
            .context_engine_id
            .as_deref()
            .filter(|s| !s.trim().is_empty())
        {
            cfg.context_engine_id = Some(wpg_snmp::usm::parse_engine_id(e).ok_or_else(|| {
                ApiError::bad_request("api-error-snmp-usm", format!("contextEngineID {e}"))
            })?);
        }
    }
    let c = Client::connect(cfg, Some(m.tap()))
        .await
        .map_err(|e| client_error(e, addr))?;
    Ok((c, addr))
}

fn client_error(e: ClientError, target: SocketAddr) -> ApiError {
    match e {
        ClientError::Timeout { attempts, .. } => ApiError::new(
            StatusCode::GATEWAY_TIMEOUT,
            codes::SNMP,
            "api-error-snmp-timeout",
        )
        .arg("target", target)
        .arg("count", attempts)
        .details(json!({ "kind": "timeout" })),
        ClientError::Usm(
            u @ (UsmError::PasswordTooShort
            | UsmError::PrivWithoutAuth
            | UsmError::UnknownProtocol(_)),
        ) => ApiError::bad_request("api-error-snmp-usm", u),
        ClientError::Invalid(m) => ApiError::bad_request("api-error-snmp-request", m),
        ClientError::Report { name, oid } => ApiError::new(
            StatusCode::BAD_GATEWAY,
            codes::SNMP,
            "api-error-snmp-report",
        )
        .arg("error", &name)
        .details(json!({ "kind": "report", "report": name, "oid": oid.to_string() })),
        other => ApiError::new(
            StatusCode::BAD_GATEWAY,
            codes::SNMP,
            "api-error-snmp-request",
        )
        .arg("error", &other)
        .details(json!({ "kind": "error" })),
    }
}

fn resolve_oid(reg: &Registry, s: &str) -> ApiResult<Oid> {
    reg.resolve(s)
        .map_err(|e| ApiError::bad_request("api-error-snmp-oid", e))
}

/// A variable binding to send (Set, notifications).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct VarBindIn {
    /// Object instance: name (`sysName.0`, `IF-MIB::ifAdminStatus.2`) or
    /// dotted OID.
    pub oid: String,
    /// `auto` (default: from the MIB), or `i` / `integer`, `u` / `unsigned`,
    /// `c` / `counter32`, `C` / `counter64`, `t` / `timeticks`, `a` /
    /// `ipaddress`, `o` / `oid`, `s` / `string`, `x` / `hex`, `d` / `decimal`,
    /// `n` / `null`, `opaque`.
    #[serde(default, rename = "type")]
    pub value_type: Option<String>,
    /// Value text (enum labels such as `down` and object names are accepted).
    #[serde(default)]
    pub value: String,
}

fn enum_value(syn: Option<&wpg_snmp::mib::registry::ResolvedSyntax>, text: &str) -> Option<i64> {
    let t = text.trim();
    let label = t.split_once('(').map_or(t, |(l, _)| l).trim();
    syn?.named.iter().find(|(_, n)| n == label).map(|(v, _)| *v)
}

/// Builds a binding: names resolved, `auto` typed from the MIB.
fn parse_varbind(reg: &Registry, vb: &VarBindIn) -> ApiResult<VarBind> {
    let oid = resolve_oid(reg, &vb.oid)?;
    let syn = reg.lookup(&oid).and_then(|(n, _)| n.syntax.clone());
    let bad = |e: String| ApiError::bad_request("api-error-snmp-value", format!("{}: {e}", vb.oid));
    let ty = vb
        .value_type
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .unwrap_or("auto");
    let text = vb.value.as_str();
    let value = match ty {
        "auto" => {
            let base = syn
                .as_ref()
                .map(|s| s.base)
                .ok_or_else(|| bad("the MIB does not know this object; choose a type".into()))?;
            match base {
                BaseType::Integer => match enum_value(syn.as_ref(), text) {
                    Some(v) => Value::Integer(v),
                    None => Value::parse("i", text).map_err(|e| bad(e.to_string()))?,
                },
                BaseType::Unsigned => Value::parse("u", text).map_err(|e| bad(e.to_string()))?,
                BaseType::Counter32 => Value::parse("c", text).map_err(|e| bad(e.to_string()))?,
                BaseType::Counter64 => Value::parse("C", text).map_err(|e| bad(e.to_string()))?,
                BaseType::TimeTicks => Value::parse("t", text).map_err(|e| bad(e.to_string()))?,
                BaseType::IpAddress => Value::parse("a", text).map_err(|e| bad(e.to_string()))?,
                BaseType::ObjectIdentifier => Value::Oid(resolve_oid(reg, text)?),
                BaseType::OctetString => {
                    let hex_hint = syn
                        .as_ref()
                        .and_then(|s| s.display_hint.as_deref())
                        .is_some_and(|h| h.contains('x'));
                    if hex_hint {
                        Value::parse("x", text).map_err(|e| bad(e.to_string()))?
                    } else {
                        Value::parse("s", text).map_err(|e| bad(e.to_string()))?
                    }
                }
                BaseType::Bits => Value::parse("x", text).map_err(|e| bad(e.to_string()))?,
                BaseType::Opaque => Value::parse("opaque", text).map_err(|e| bad(e.to_string()))?,
                BaseType::Null => Value::Null,
                BaseType::Other => {
                    return Err(bad("not a scalar / column object; choose a type".into()))
                }
            }
        }
        "o" | "oid" | "OID" => Value::Oid(resolve_oid(reg, text)?),
        "i" | "integer" | "INTEGER" => match enum_value(syn.as_ref(), text) {
            Some(v) => Value::Integer(v),
            None => Value::parse("i", text).map_err(|e| bad(e.to_string()))?,
        },
        other => Value::parse(other, text).map_err(|e| bad(e.to_string()))?,
    };
    Ok(VarBind::new(oid, value))
}

/// Result of an operation.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SnmpResult {
    /// `get`, `getnext`, `getbulk`, `walk`, `set`, `trap`, `inform`.
    pub op: String,
    /// The agent / receiver address.
    pub target: String,
    /// `v1`, `v2c`, `v3`.
    pub version: String,
    /// Round trip(s) in milliseconds.
    pub elapsed_ms: f64,
    /// `error-status` of the response (0 = noError).
    pub error_status: i32,
    /// Its name (`noSuchName`, `notWritable`, ...), when not 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_status_name: Option<String>,
    /// `error-index` (1-based binding, 0 = none).
    pub error_index: i32,
    /// The bindings (response, walk results, or the notification sent).
    pub varbinds: Vec<VarBindOut>,
    /// Conceptual tables found among the bindings (walks).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tables: Vec<TableOut>,
    /// The walk stopped at its limit.
    #[serde(default)]
    pub truncated: bool,
    /// Requests sent.
    pub requests: u32,
    /// The agent's engine id (v3, hex).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_id: Option<String>,
}

/// A conceptual table assembled from walk results.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TableOut {
    /// `IF-MIB::ifEntry`.
    pub entry: String,
    /// `IF-MIB::ifTable`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    /// INDEX objects.
    pub index_names: Vec<String>,
    /// Columns present, in OID order.
    pub columns: Vec<TableColumnOut>,
    /// Rows in instance order.
    pub rows: Vec<TableRowOut>,
}

/// A table column.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TableColumnOut {
    /// Descriptor.
    pub name: String,
    /// Column OID.
    pub oid: String,
    /// Syntax text.
    pub syntax: String,
    /// Counter column (rates make sense).
    pub counter: bool,
}

/// A table row.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TableRowOut {
    /// Instance sub-identifiers.
    pub instance: String,
    /// Decoded INDEX values (empty when not decodable).
    pub index: Vec<String>,
    /// Readable cells (one per column; `null` = absent).
    pub cells: Vec<Option<String>>,
    /// Numeric cells (integer types).
    pub numeric: Vec<Option<u64>>,
}

impl From<Table> for TableOut {
    fn from(t: Table) -> Self {
        Self {
            entry: t.entry,
            table: t.table,
            index_names: t.index_names,
            columns: t
                .columns
                .into_iter()
                .map(|c| TableColumnOut {
                    name: c.name,
                    oid: c.oid,
                    syntax: c.syntax,
                    counter: c.counter,
                })
                .collect(),
            rows: t
                .rows
                .into_iter()
                .map(|r| TableRowOut {
                    instance: r.instance,
                    index: r.index,
                    cells: r.cells,
                    numeric: r.numeric,
                })
                .collect(),
        }
    }
}

struct Outcome<'a> {
    op: &'a str,
    target: SocketAddr,
    version: SnmpVersion,
    started: Instant,
    pdu: Option<Pdu>,
    varbinds: Vec<VarBind>,
    tables: bool,
    truncated: bool,
    requests: u32,
    engine_id: Option<Vec<u8>>,
}

fn render(m: &SnmpManager, o: &Outcome<'_>) -> SnmpResult {
    let (status, index) = o
        .pdu
        .as_ref()
        .map_or((0, 0), |p| (p.error_status, p.error_index));
    m.with_registry(|reg| SnmpResult {
        op: o.op.to_owned(),
        target: o.target.to_string(),
        version: Version::from(o.version).name().to_owned(),
        elapsed_ms: o.started.elapsed().as_secs_f64() * 1000.0,
        error_status: status,
        error_status_name: (status != 0).then(|| error_status_name(status)),
        error_index: index,
        varbinds: o
            .varbinds
            .iter()
            .map(|vb| format_varbind(reg, vb).into())
            .collect(),
        tables: if o.tables {
            build_tables(reg, &o.varbinds)
                .into_iter()
                .map(Into::into)
                .collect()
        } else {
            Vec::new()
        },
        truncated: o.truncated,
        requests: o.requests,
        engine_id: o.engine_id.as_deref().map(wpg_snmp::usm::hex),
    })
}

/// Get / GetNext request body.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SnmpOidsRequest {
    /// The agent.
    pub target: SnmpTarget,
    /// Objects: names (`sysDescr.0`, `IF-MIB::ifDescr.1`) or dotted OIDs.
    pub oids: Vec<String>,
}

/// GetBulk request body.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SnmpBulkRequest {
    /// The agent.
    pub target: SnmpTarget,
    /// Objects (the first `non_repeaters` are fetched once).
    pub oids: Vec<String>,
    /// `non-repeaters` (default 0).
    #[serde(default)]
    pub non_repeaters: Option<i32>,
    /// `max-repetitions` (default 10).
    #[serde(default)]
    pub max_repetitions: Option<i32>,
}

/// Walk request body.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SnmpWalkRequest {
    /// The agent.
    pub target: SnmpTarget,
    /// Subtree root: name (`ifTable`, `system`) or dotted OID.
    pub oid: String,
    /// `max-repetitions` per GetBulk (default 20; v1 walks with GetNext).
    #[serde(default)]
    pub max_repetitions: Option<i32>,
    /// Most bindings returned (default 10000, at most 100000).
    #[serde(default)]
    pub limit: Option<usize>,
}

/// Set request body.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SnmpSetRequest {
    /// The agent.
    pub target: SnmpTarget,
    /// Bindings to set.
    pub varbinds: Vec<VarBindIn>,
}

/// Trap / Inform request body.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SnmpNotifyRequest {
    /// The receiver (port default 162).
    pub target: SnmpTarget,
    /// `snmpTrapOID` (`linkDown`, `IF-MIB::linkUp`, `1.3.6.1.6.3.1.1.5.1`).
    /// v1 traps are converted per RFC 3584.
    pub trap_oid: String,
    /// `sysUpTime` in hundredths of a second (default: the server's uptime).
    #[serde(default)]
    pub uptime: Option<u32>,
    /// Extra bindings.
    #[serde(default)]
    pub varbinds: Vec<VarBindIn>,
}

fn oids(m: &SnmpManager, list: &[String]) -> ApiResult<Vec<Oid>> {
    if list.is_empty() {
        return Err(ApiError::bad_request("api-error-snmp-oid", "no OID given"));
    }
    m.with_registry(|reg| list.iter().map(|s| resolve_oid(reg, s)).collect())
}

/// `GetRequest`.
#[utoipa::path(post, path = "/api/snmp/get", tag = "snmp", request_body = SnmpOidsRequest,
    responses(
        (status = 200, description = "Response (check `error_status`)", body = SnmpResult),
        (status = 400, description = "Bad target / OID / v3 settings", body = crate::error::ErrorBody),
        (status = 502, description = "`snmp_error`: Report PDU, authentication or decoding failure", body = crate::error::ErrorBody),
        (status = 504, description = "`snmp_error`: no response", body = crate::error::ErrorBody)))]
pub async fn get(
    State(st): State<AppState>,
    ApiJson(req): ApiJson<SnmpOidsRequest>,
) -> ApiResult<Json<SnmpResult>> {
    simple(&st, req, false).await
}

/// `GetNextRequest`.
#[utoipa::path(post, path = "/api/snmp/getnext", tag = "snmp", request_body = SnmpOidsRequest,
    responses(
        (status = 200, description = "Response", body = SnmpResult),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody),
        (status = 502, description = "`snmp_error`", body = crate::error::ErrorBody),
        (status = 504, description = "`snmp_error`: no response", body = crate::error::ErrorBody)))]
pub async fn getnext(
    State(st): State<AppState>,
    ApiJson(req): ApiJson<SnmpOidsRequest>,
) -> ApiResult<Json<SnmpResult>> {
    simple(&st, req, true).await
}

async fn simple(st: &AppState, req: SnmpOidsRequest, next: bool) -> ApiResult<Json<SnmpResult>> {
    let m = snmp(st)?;
    let list = oids(m, &req.oids)?;
    let (mut c, addr) = connect(m, &req.target, 161).await?;
    let started = Instant::now();
    let pdu = if next {
        c.get_next(&list).await
    } else {
        c.get(&list).await
    }
    .map_err(|e| client_error(e, addr))?;
    Ok(Json(render(
        m,
        &Outcome {
            op: if next { "getnext" } else { "get" },
            target: addr,
            version: req.target.version,
            started,
            varbinds: pdu.varbinds.clone(),
            pdu: Some(pdu),
            tables: false,
            truncated: false,
            requests: 1,
            engine_id: c.engine_id().map(<[u8]>::to_vec),
        },
    )))
}

/// `GetBulkRequest` (v2c / v3).
#[utoipa::path(post, path = "/api/snmp/getbulk", tag = "snmp", request_body = SnmpBulkRequest,
    responses(
        (status = 200, description = "Response", body = SnmpResult),
        (status = 400, description = "Bad request (also: GetBulk on v1)", body = crate::error::ErrorBody),
        (status = 502, description = "`snmp_error`", body = crate::error::ErrorBody),
        (status = 504, description = "`snmp_error`: no response", body = crate::error::ErrorBody)))]
pub async fn getbulk(
    State(st): State<AppState>,
    ApiJson(req): ApiJson<SnmpBulkRequest>,
) -> ApiResult<Json<SnmpResult>> {
    let m = snmp(&st)?;
    let list = oids(m, &req.oids)?;
    let (mut c, addr) = connect(m, &req.target, 161).await?;
    let started = Instant::now();
    let pdu = c
        .get_bulk(
            req.non_repeaters.unwrap_or(0).max(0),
            req.max_repetitions.unwrap_or(10).clamp(1, 1000),
            &list,
        )
        .await
        .map_err(|e| client_error(e, addr))?;
    Ok(Json(render(
        m,
        &Outcome {
            op: "getbulk",
            target: addr,
            version: req.target.version,
            started,
            varbinds: pdu.varbinds.clone(),
            pdu: Some(pdu),
            tables: true,
            truncated: false,
            requests: 1,
            engine_id: c.engine_id().map(<[u8]>::to_vec),
        },
    )))
}

/// Walks a subtree (GetBulk on v2c / v3, GetNext on v1); conceptual tables in
/// the result are also returned as rows × columns.
#[utoipa::path(post, path = "/api/snmp/walk", tag = "snmp", request_body = SnmpWalkRequest,
    responses(
        (status = 200, description = "Bindings and tables", body = SnmpResult),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody),
        (status = 502, description = "`snmp_error`", body = crate::error::ErrorBody),
        (status = 504, description = "`snmp_error`: no response", body = crate::error::ErrorBody)))]
pub async fn walk(
    State(st): State<AppState>,
    ApiJson(req): ApiJson<SnmpWalkRequest>,
) -> ApiResult<Json<SnmpResult>> {
    let m = snmp(&st)?;
    let root = m.with_registry(|reg| resolve_oid(reg, &req.oid))?;
    let (mut c, addr) = connect(m, &req.target, 161).await?;
    let started = Instant::now();
    let w = c
        .walk(
            &root,
            req.max_repetitions.unwrap_or(20).clamp(1, 1000),
            req.limit.unwrap_or(10_000).clamp(1, MAX_WALK),
        )
        .await
        .map_err(|e| client_error(e, addr))?;
    Ok(Json(render(
        m,
        &Outcome {
            op: "walk",
            target: addr,
            version: req.target.version,
            started,
            pdu: None,
            varbinds: w.varbinds,
            tables: true,
            truncated: w.truncated,
            requests: w.requests,
            engine_id: c.engine_id().map(<[u8]>::to_vec),
        },
    )))
}

/// `SetRequest`.
#[utoipa::path(post, path = "/api/snmp/set", tag = "snmp", request_body = SnmpSetRequest,
    responses(
        (status = 200, description = "Response (check `error_status`: notWritable, wrongType, ...)", body = SnmpResult),
        (status = 400, description = "Bad target / OID / value", body = crate::error::ErrorBody),
        (status = 502, description = "`snmp_error`", body = crate::error::ErrorBody),
        (status = 504, description = "`snmp_error`: no response", body = crate::error::ErrorBody)))]
pub async fn set(
    State(st): State<AppState>,
    ApiJson(req): ApiJson<SnmpSetRequest>,
) -> ApiResult<Json<SnmpResult>> {
    let m = snmp(&st)?;
    if req.varbinds.is_empty() {
        return Err(ApiError::bad_request(
            "api-error-snmp-oid",
            "no binding given",
        ));
    }
    let vbs = m.with_registry(|reg| {
        req.varbinds
            .iter()
            .map(|v| parse_varbind(reg, v))
            .collect::<ApiResult<Vec<_>>>()
    })?;
    let (mut c, addr) = connect(m, &req.target, 161).await?;
    let started = Instant::now();
    let pdu = c.set(vbs).await.map_err(|e| client_error(e, addr))?;
    Ok(Json(render(
        m,
        &Outcome {
            op: "set",
            target: addr,
            version: req.target.version,
            started,
            varbinds: pdu.varbinds.clone(),
            pdu: Some(pdu),
            tables: false,
            truncated: false,
            requests: 1,
            engine_id: c.engine_id().map(<[u8]>::to_vec),
        },
    )))
}

async fn notify(
    st: &AppState,
    req: SnmpNotifyRequest,
    inform: bool,
) -> ApiResult<Json<SnmpResult>> {
    let m = snmp(st)?;
    let (trap_oid, vbs) = m.with_registry(|reg| -> ApiResult<_> {
        let t = resolve_oid(reg, &req.trap_oid)?;
        let v = req
            .varbinds
            .iter()
            .map(|v| parse_varbind(reg, v))
            .collect::<ApiResult<Vec<_>>>()?;
        Ok((t, v))
    })?;
    let uptime = req.uptime.unwrap_or_else(|| m.uptime_ticks());
    let (mut c, addr) = connect(m, &req.target, 162).await?;
    let started = Instant::now();
    let ack = c
        .notify(&trap_oid, uptime, &vbs, inform)
        .await
        .map_err(|e| client_error(e, addr))?;
    let sent = Pdu::notification(wpg_snmp::PduType::TrapV2, 0, uptime, &trap_oid, &vbs);
    Ok(Json(render(
        m,
        &Outcome {
            op: if inform { "inform" } else { "trap" },
            target: addr,
            version: req.target.version,
            started,
            varbinds: ack.as_ref().map_or(sent.varbinds, |a| a.varbinds.clone()),
            pdu: ack,
            tables: false,
            truncated: false,
            requests: 1,
            engine_id: c.engine_id().map(<[u8]>::to_vec),
        },
    )))
}

/// Sends a trap (v1 Trap-PDU, SNMPv2-Trap, or a v3 trap with this server as
/// the authoritative engine). The result lists the bindings sent.
#[utoipa::path(post, path = "/api/snmp/trap", tag = "snmp", request_body = SnmpNotifyRequest,
    responses(
        (status = 200, description = "Sent", body = SnmpResult),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody)))]
pub async fn trap(
    State(st): State<AppState>,
    ApiJson(req): ApiJson<SnmpNotifyRequest>,
) -> ApiResult<Json<SnmpResult>> {
    notify(&st, req, false).await
}

/// Sends an `InformRequest` (v2c / v3) and waits for the acknowledgement.
#[utoipa::path(post, path = "/api/snmp/inform", tag = "snmp", request_body = SnmpNotifyRequest,
    responses(
        (status = 200, description = "Acknowledged (the Response's bindings)", body = SnmpResult),
        (status = 400, description = "Bad request (also: inform on v1)", body = crate::error::ErrorBody),
        (status = 502, description = "`snmp_error`", body = crate::error::ErrorBody),
        (status = 504, description = "`snmp_error`: not acknowledged", body = crate::error::ErrorBody)))]
pub async fn inform(
    State(st): State<AppState>,
    ApiJson(req): ApiJson<SnmpNotifyRequest>,
) -> ApiResult<Json<SnmpResult>> {
    notify(&st, req, true).await
}

// ------------------------------------------------------------------ status

/// `GET /api/snmp/status`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SnmpStatus {
    /// `snmp.enabled`.
    pub enabled: bool,
    /// Directory of uploaded MIBs.
    pub mib_dir: String,
    /// Loaded modules.
    pub modules: usize,
    /// Resolved MIB nodes.
    pub nodes: usize,
    /// Receiver state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receiver: Option<ReceiverStatus>,
    /// Recording state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture: Option<CaptureStatus>,
}

/// Whether the SNMP tool is enabled, with MIB / receiver / recording summaries.
#[utoipa::path(get, path = "/api/snmp/status", tag = "snmp",
    responses((status = 200, description = "Status", body = SnmpStatus)))]
pub async fn status(State(st): State<AppState>) -> Json<SnmpStatus> {
    let m = &st.snmp;
    if !m.enabled() {
        return Json(SnmpStatus {
            enabled: false,
            mib_dir: m.mib_dir().display().to_string(),
            modules: 0,
            nodes: 0,
            receiver: None,
            capture: None,
        });
    }
    let (modules, nodes) = m.with_registry(|r| (r.modules().len(), r.node_count()));
    Json(SnmpStatus {
        enabled: true,
        mib_dir: m.mib_dir().display().to_string(),
        modules,
        nodes,
        receiver: Some(m.receiver_status().await),
        capture: Some(m.capture_status()),
    })
}

// ------------------------------------------------------------------ MIBs

/// A loaded MIB module.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MibModuleOut {
    /// Module name.
    pub name: String,
    /// Bundled with wpg (cannot be deleted).
    pub bundled: bool,
    /// Uploaded file it came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// `ok`, `partial` (loaded with problems) or `error`.
    pub status: String,
    /// Modules named in IMPORTS that are not loaded (upload them).
    pub missing_imports: Vec<String>,
    /// Problems: parse warnings, unresolved definitions.
    pub errors: Vec<String>,
    /// Resolved definitions.
    pub nodes: usize,
}

fn modules_out(reg: &Registry) -> Vec<MibModuleOut> {
    reg.modules()
        .into_iter()
        .map(|m| MibModuleOut {
            bundled: m.source == wpg_snmp::mib::Source::Bundled,
            file: match &m.source {
                wpg_snmp::mib::Source::File(f) => Some(f.clone()),
                wpg_snmp::mib::Source::Bundled => None,
            },
            status: match m.status {
                wpg_snmp::mib::ModuleStatus::Ok => "ok",
                wpg_snmp::mib::ModuleStatus::Partial => "partial",
                wpg_snmp::mib::ModuleStatus::Error => "error",
            }
            .to_owned(),
            name: m.name,
            missing_imports: m.missing_imports,
            errors: m.errors,
            nodes: m.nodes,
        })
        .collect()
}

/// Loaded MIB modules with their load state.
#[utoipa::path(get, path = "/api/snmp/mibs", tag = "snmp",
    responses((status = 200, description = "Modules", body = Vec<MibModuleOut>)))]
pub async fn mibs(State(st): State<AppState>) -> ApiResult<Json<Vec<MibModuleOut>>> {
    let m = snmp(&st)?;
    Ok(Json(m.with_registry(modules_out)))
}

/// One file of an upload.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct MibFileIn {
    /// File name (`CISCO-SMI.my`; `.mib`, `.txt`, `.my`, `.smi`, `.asn`,
    /// `.asn1` or none).
    pub name: String,
    /// File content (text).
    pub content: String,
}

/// `POST /api/snmp/mibs` body.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct MibUpload {
    /// Files.
    pub files: Vec<MibFileIn>,
}

/// `POST /api/snmp/mibs` result.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MibUploadResult {
    /// Per file.
    pub files: Vec<UploadedFile>,
    /// The state of the uploaded modules after loading (missing imports are
    /// named here).
    pub modules: Vec<MibModuleOut>,
}

fn mib_error(e: MibError) -> ApiError {
    match e {
        MibError::BadName(n) => {
            ApiError::bad_request("api-error-snmp-mib", format!("{n}: file name / extension"))
                .arg("file", n)
        }
        MibError::TooLarge(n) => {
            ApiError::bad_request("api-error-snmp-mib", "too large").arg("file", n)
        }
        MibError::Bundled(n) => ApiError::new(
            StatusCode::CONFLICT,
            codes::READ_ONLY,
            "api-error-snmp-mib-bundled",
        )
        .arg("what", n),
        MibError::NotFound(n) => ApiError::not_found(format!("MIB module {n}")),
        MibError::Io { .. } => ApiError::internal(e),
    }
}

/// Uploads MIB files (stored under `<data_dir>/mibs`, loaded at once).
#[utoipa::path(post, path = "/api/snmp/mibs", tag = "snmp", request_body = MibUpload,
    responses(
        (status = 200, description = "Per-file result and module state", body = MibUploadResult),
        (status = 400, description = "Bad file name / too large", body = crate::error::ErrorBody)))]
pub async fn upload_mibs(
    State(st): State<AppState>,
    ApiJson(req): ApiJson<MibUpload>,
) -> ApiResult<Json<MibUploadResult>> {
    let m = snmp(&st)?.clone();
    if req.files.is_empty() {
        return Err(ApiError::bad_request("api-error-snmp-mib", "no file").arg("file", "-"));
    }
    let uploads: Vec<(String, String)> =
        req.files.into_iter().map(|f| (f.name, f.content)).collect();
    let files = tokio::task::spawn_blocking({
        let m = m.clone();
        move || m.upload(&uploads)
    })
    .await
    .map_err(ApiError::internal)?
    .map_err(mib_error)?;
    let names: Vec<&String> = files.iter().flat_map(|f| &f.modules).collect();
    let modules = m.with_registry(|r| {
        modules_out(r)
            .into_iter()
            .filter(|x| names.contains(&&x.name))
            .collect()
    });
    Ok(Json(MibUploadResult { files, modules }))
}

/// Removed modules.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MibDeleted {
    /// Modules removed with the file.
    pub removed: Vec<String>,
}

/// Deletes an uploaded module (its whole file). Bundled modules cannot be
/// deleted.
#[utoipa::path(delete, path = "/api/snmp/mibs/{module}", tag = "snmp",
    params(("module" = String, Path, description = "Module name")),
    responses(
        (status = 200, description = "Deleted", body = MibDeleted),
        (status = 404, description = "Unknown module", body = crate::error::ErrorBody),
        (status = 409, description = "`read_only`: bundled module", body = crate::error::ErrorBody)))]
pub async fn delete_mib(
    State(st): State<AppState>,
    ApiPath(module): ApiPath<String>,
) -> ApiResult<Json<MibDeleted>> {
    let m = snmp(&st)?.clone();
    let removed = tokio::task::spawn_blocking(move || m.delete_module(&module))
        .await
        .map_err(ApiError::internal)?
        .map_err(mib_error)?;
    Ok(Json(MibDeleted { removed }))
}

/// `?oid=` of the tree.
#[derive(Debug, Deserialize, IntoParams)]
pub struct TreeQuery {
    /// Parent (name or dotted OID; empty = the roots).
    #[serde(default)]
    pub oid: Option<String>,
}

/// A tree entry.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TreeChildOut {
    /// OID.
    pub oid: String,
    /// Last sub-identifier.
    pub arc: u32,
    /// Descriptor (absent for an undefined intermediate OID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Module.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    /// `object_type`, `object_identifier`, `notification_type`, ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Table / row / column / scalar for OBJECT-TYPEs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Something is defined below.
    pub has_children: bool,
}

fn kind_name(k: &wpg_snmp::mib::parser::DefKind) -> String {
    serde_json::to_value(k)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn role(reg: &Registry, n: &wpg_snmp::mib::Node) -> Option<String> {
    if n.kind != wpg_snmp::mib::parser::DefKind::ObjectType {
        return None;
    }
    Some(
        if n.is_table() {
            "table"
        } else if n.is_row() {
            "row"
        } else if n
            .oid
            .parent()
            .and_then(|p| reg.node(&p))
            .is_some_and(wpg_snmp::mib::Node::is_row)
        {
            "column"
        } else {
            "scalar"
        }
        .to_owned(),
    )
}

/// One level of the MIB tree.
#[utoipa::path(get, path = "/api/snmp/mibs/tree", tag = "snmp", params(TreeQuery),
    responses(
        (status = 200, description = "Children", body = Vec<TreeChildOut>),
        (status = 400, description = "Unknown name", body = crate::error::ErrorBody)))]
pub async fn tree(
    State(st): State<AppState>,
    ApiQuery(q): ApiQuery<TreeQuery>,
) -> ApiResult<Json<Vec<TreeChildOut>>> {
    let m = snmp(&st)?;
    m.with_registry(|reg| {
        let parent = match q.oid.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            None => Oid::default(),
            Some(s) => resolve_oid(reg, s)?,
        };
        Ok(Json(
            reg.children(&parent)
                .into_iter()
                .map(|c| {
                    let node = reg.node(&c.oid);
                    TreeChildOut {
                        oid: c.oid.to_string(),
                        arc: c.arc,
                        role: node.and_then(|n| role(reg, n)),
                        name: c.name,
                        module: c.module,
                        kind: c.kind.as_ref().map(kind_name),
                        has_children: c.has_children,
                    }
                })
                .collect(),
        ))
    })
}

/// `?id=` of a node.
#[derive(Debug, Deserialize, IntoParams)]
pub struct NodeQuery {
    /// Name (`ifOperStatus`, `IF-MIB::ifOperStatus`, `ifOperStatus.3`) or
    /// dotted OID.
    pub id: String,
}

/// An enumeration value / BITS position.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NamedNumber {
    /// Number.
    pub value: i64,
    /// Label.
    pub label: String,
}

/// Breadcrumb element.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PathElem {
    /// OID.
    pub oid: String,
    /// Name (or the number).
    pub name: String,
}

/// MIB node details.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NodeOut {
    /// OID of the definition.
    pub oid: String,
    /// Instance part of the requested id (`3` of `ifDescr.3`).
    pub instance: String,
    /// Descriptor.
    pub name: String,
    /// Module.
    pub module: String,
    /// `object_type`, `object_identifier`, ...
    pub kind: String,
    /// Table / row / column / scalar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// SYNTAX text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax: Option<String>,
    /// Base type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_type: Option<String>,
    /// Textual convention.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    /// DISPLAY-HINT.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_hint: Option<String>,
    /// Enumeration / BITS names.
    pub enums: Vec<NamedNumber>,
    /// MAX-ACCESS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<String>,
    /// STATUS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// UNITS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<String>,
    /// DESCRIPTION.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// REFERENCE.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// DEFVAL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defval: Option<String>,
    /// INDEX (of a row; AUGMENTS followed).
    pub index: Vec<String>,
    /// AUGMENTS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub augments: Option<String>,
    /// OBJECTS / VARIABLES (notifications).
    pub objects: Vec<String>,
    /// Path from the root.
    pub path: Vec<PathElem>,
}

/// Details of a MIB node (longest-prefix match for instances).
#[utoipa::path(get, path = "/api/snmp/mibs/node", tag = "snmp", params(NodeQuery),
    responses(
        (status = 200, description = "Node", body = NodeOut),
        (status = 400, description = "Unparsable id", body = crate::error::ErrorBody),
        (status = 404, description = "No MIB defines it", body = crate::error::ErrorBody)))]
pub async fn node(
    State(st): State<AppState>,
    ApiQuery(q): ApiQuery<NodeQuery>,
) -> ApiResult<Json<NodeOut>> {
    let m = snmp(&st)?;
    m.with_registry(|reg| {
        let oid = resolve_oid(reg, &q.id)?;
        let (n, rest) = reg
            .lookup(&oid)
            .ok_or_else(|| ApiError::not_found(format!("MIB node {}", q.id)))?;
        let mut path = Vec::new();
        let arcs = n.oid.arcs();
        for i in 1..=arcs.len() {
            let o = Oid::from(&arcs[..i]);
            let name = reg
                .node(&o)
                .map_or_else(|| arcs[i - 1].to_string(), |x| x.name.clone());
            path.push(PathElem {
                oid: o.to_string(),
                name,
            });
        }
        let s = n.syntax.as_ref();
        Ok(Json(NodeOut {
            oid: n.oid.to_string(),
            instance: rest
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join("."),
            name: n.name.clone(),
            module: n.module.clone(),
            kind: kind_name(&n.kind),
            role: role(reg, n),
            syntax: s.map(wpg_snmp::mib::registry::ResolvedSyntax::describe),
            base_type: s.map(|s| s.base.smi_name().to_owned()),
            type_name: s.and_then(|s| s.type_name.clone()),
            display_hint: s.and_then(|s| s.display_hint.clone()),
            enums: s
                .map(|s| {
                    s.named
                        .iter()
                        .map(|(v, l)| NamedNumber {
                            value: *v,
                            label: l.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            access: n.access.clone(),
            status: n.status.clone(),
            units: n.units.clone(),
            description: n.description.clone(),
            reference: n.reference.clone(),
            defval: n.defval.clone(),
            index: reg.index_of(n).iter().map(|i| i.name.clone()).collect(),
            augments: n.augments.clone(),
            objects: n.objects.clone(),
            path,
        }))
    })
}

/// `?q=&limit=` of the search.
#[derive(Debug, Deserialize, IntoParams)]
pub struct SearchQuery {
    /// Part of a descriptor (`ifOper`, `IF-MIB::ifH`).
    #[serde(default)]
    pub q: String,
    /// Most hits (default 20, at most 200).
    #[serde(default)]
    pub limit: Option<usize>,
}

/// A search hit.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SearchHit {
    /// Descriptor.
    pub name: String,
    /// Module.
    pub module: String,
    /// OID.
    pub oid: String,
    /// Kind.
    pub kind: String,
    /// Table / row / column / scalar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// SYNTAX text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax: Option<String>,
}

/// Finds MIB objects by name (for OID autocompletion).
#[utoipa::path(get, path = "/api/snmp/mibs/search", tag = "snmp", params(SearchQuery),
    responses((status = 200, description = "Hits, prefix matches first", body = Vec<SearchHit>)))]
pub async fn search(
    State(st): State<AppState>,
    ApiQuery(q): ApiQuery<SearchQuery>,
) -> ApiResult<Json<Vec<SearchHit>>> {
    let m = snmp(&st)?;
    let limit = q.limit.unwrap_or(20).clamp(1, 200);
    Ok(Json(m.with_registry(|reg| {
        if q.q.trim().is_empty() {
            return Vec::new();
        }
        reg.search(&q.q, limit)
            .into_iter()
            .map(|n| SearchHit {
                name: n.name.clone(),
                module: n.module.clone(),
                oid: n.oid.to_string(),
                kind: kind_name(&n.kind),
                role: role(reg, n),
                syntax: n
                    .syntax
                    .as_ref()
                    .map(wpg_snmp::mib::registry::ResolvedSyntax::describe),
            })
            .collect()
    })))
}

// ------------------------------------------------------------------ receiver

/// A v3 user of the receiver.
#[derive(Clone, Default, Deserialize, ToSchema)]
pub struct ReceiverUserIn {
    /// User name.
    pub user: String,
    /// Authentication protocol (default `SHA`).
    #[serde(default)]
    pub auth_protocol: Option<String>,
    /// Authentication pass phrase.
    #[serde(default)]
    pub auth_password: Option<String>,
    /// Privacy protocol (default `AES`).
    #[serde(default)]
    pub priv_protocol: Option<String>,
    /// Privacy pass phrase.
    #[serde(default)]
    pub priv_password: Option<String>,
}

impl std::fmt::Debug for ReceiverUserIn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReceiverUserIn")
            .field("user", &self.user)
            .finish_non_exhaustive()
    }
}

/// `POST /api/snmp/receiver/start` body.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ReceiverStart {
    /// Listen address (`0.0.0.0:162`, `[::]:1162`).
    pub listen: String,
    /// Accepted communities (empty = any; others are listed with an error).
    #[serde(default)]
    pub communities: Vec<String>,
    /// v3 users (kept in memory while the receiver runs).
    #[serde(default)]
    pub users: Vec<ReceiverUserIn>,
}

/// Receiver state.
#[utoipa::path(get, path = "/api/snmp/receiver", tag = "snmp",
    responses((status = 200, description = "State", body = ReceiverStatus)))]
pub async fn receiver(State(st): State<AppState>) -> ApiResult<Json<ReceiverStatus>> {
    Ok(Json(snmp(&st)?.receiver_status().await))
}

/// Starts (or restarts) the trap / inform receiver.
#[utoipa::path(post, path = "/api/snmp/receiver/start", tag = "snmp", request_body = ReceiverStart,
    responses(
        (status = 200, description = "Listening", body = ReceiverStatus),
        (status = 400, description = "Bad address / users", body = crate::error::ErrorBody),
        (status = 502, description = "`backend_error`: cannot bind (privileged port, in use)", body = crate::error::ErrorBody)))]
pub async fn receiver_start(
    State(st): State<AppState>,
    ApiJson(req): ApiJson<ReceiverStart>,
) -> ApiResult<Json<ReceiverStatus>> {
    let m = snmp(&st)?;
    let listen: SocketAddr = req.listen.trim().parse().map_err(|e| {
        ApiError::bad_request("api-error-snmp-target", format!("{}: {e}", req.listen))
    })?;
    let users = req
        .users
        .iter()
        .map(|u| {
            V3In {
                user: u.user.clone(),
                auth_protocol: u.auth_protocol.clone(),
                auth_password: u.auth_password.clone(),
                priv_protocol: u.priv_protocol.clone(),
                priv_password: u.priv_password.clone(),
                ..V3In::default()
            }
            .to_user()
        })
        .collect::<ApiResult<Vec<_>>>()?;
    let communities = req
        .communities
        .into_iter()
        .map(|c| c.trim().to_owned())
        .filter(|c| !c.is_empty())
        .collect();
    match m.start_receiver(listen, communities, users, false).await {
        Ok(s) => Ok(Json(s)),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Err(ApiError::new(
            StatusCode::BAD_GATEWAY,
            codes::BACKEND_ERROR,
            "api-error-snmp-bind-permission",
        )
        .arg("addr", listen)),
        Err(e) => Err(ApiError::new(
            StatusCode::BAD_GATEWAY,
            codes::BACKEND_ERROR,
            "api-error-snmp-bind",
        )
        .arg("addr", listen)
        .arg("error", e)),
    }
}

/// Stops the receiver.
#[utoipa::path(post, path = "/api/snmp/receiver/stop", tag = "snmp",
    responses((status = 200, description = "Stopped", body = ReceiverStatus)))]
pub async fn receiver_stop(State(st): State<AppState>) -> ApiResult<Json<ReceiverStatus>> {
    let m = snmp(&st)?;
    m.stop_receiver().await;
    Ok(Json(m.receiver_status().await))
}

/// `?limit=` of the trap list.
#[derive(Debug, Deserialize, IntoParams)]
pub struct TrapsQuery {
    /// Most notifications (default 200, at most 1000).
    #[serde(default)]
    pub limit: Option<usize>,
}

/// Received notifications, newest first (also pushed as `snmp_trap` events on
/// the WebSocket).
#[utoipa::path(get, path = "/api/snmp/traps", tag = "snmp", params(TrapsQuery),
    responses((status = 200, description = "Notifications", body = Vec<TrapOut>)))]
pub async fn traps(
    State(st): State<AppState>,
    ApiQuery(q): ApiQuery<TrapsQuery>,
) -> ApiResult<Json<Vec<TrapOut>>> {
    Ok(Json(snmp(&st)?.traps(
        q.limit.unwrap_or(200).clamp(1, crate::snmp::MAX_TRAPS),
    )))
}

/// Forgets the received notifications.
#[utoipa::path(delete, path = "/api/snmp/traps", tag = "snmp",
    responses((status = 204, description = "Cleared")))]
pub async fn clear_traps(State(st): State<AppState>) -> ApiResult<StatusCode> {
    snmp(&st)?.clear_traps();
    Ok(StatusCode::NO_CONTENT)
}

// ------------------------------------------------------------------ pcap

/// `POST /api/snmp/capture` body.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CaptureControl {
    /// Start (`true`) / stop (`false`) recording.
    #[serde(default)]
    pub enabled: Option<bool>,
    /// Discard the recorded frames first.
    #[serde(default)]
    pub clear: bool,
}

/// Recording state.
#[utoipa::path(get, path = "/api/snmp/capture", tag = "snmp",
    responses((status = 200, description = "State", body = CaptureStatus)))]
pub async fn capture(State(st): State<AppState>) -> ApiResult<Json<CaptureStatus>> {
    Ok(Json(snmp(&st)?.capture_status()))
}

/// Starts / stops the recording of every SNMP datagram this server sends or
/// receives (requests, responses, traps), or clears it.
#[utoipa::path(post, path = "/api/snmp/capture", tag = "snmp", request_body = CaptureControl,
    responses((status = 200, description = "State", body = CaptureStatus)))]
pub async fn capture_control(
    State(st): State<AppState>,
    ApiJson(req): ApiJson<CaptureControl>,
) -> ApiResult<Json<CaptureStatus>> {
    let m = snmp(&st)?;
    if req.clear {
        m.recorder().clear();
    }
    if let Some(on) = req.enabled {
        m.recorder().set_enabled(on);
    }
    Ok(Json(m.capture_status()))
}

/// `?format=` of the download.
#[derive(Debug, Deserialize, IntoParams)]
pub struct DownloadQuery {
    /// `pcap` (default) or `pcapng`.
    #[serde(default)]
    pub format: Option<String>,
}

/// The recorded frames as a pcap / pcapng file (Ethernet / IP / UDP
/// synthesised around the real datagrams; placeholder MAC addresses).
#[utoipa::path(get, path = "/api/snmp/capture/download", tag = "snmp", params(DownloadQuery),
    responses(
        (status = 200, description = "pcap / pcapng file", content_type = "application/octet-stream"),
        (status = 400, description = "Unknown format", body = crate::error::ErrorBody)))]
pub async fn capture_download(
    State(st): State<AppState>,
    ApiQuery(q): ApiQuery<DownloadQuery>,
) -> ApiResult<Response> {
    let m = snmp(&st)?;
    let (format, ext) = match q.format.as_deref().unwrap_or("pcap") {
        "pcap" => (PcapFormat::Pcap, "pcap"),
        "pcapng" => (PcapFormat::Pcapng, "pcapng"),
        other => {
            return Err(ApiError::bad_request(
                "api-error-invalid-query",
                format!("format={other}"),
            ))
        }
    };
    let bytes = m.recorder().export(format);
    let mut resp = Body::from(bytes).into_response();
    let h = resp.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"wpg-snmp-{secs}.{ext}\""))
    {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varbind_typing() {
        let reg = Registry::bundled();
        let p = |oid: &str, ty: Option<&str>, v: &str| {
            parse_varbind(
                &reg,
                &VarBindIn {
                    oid: oid.into(),
                    value_type: ty.map(str::to_owned),
                    value: v.into(),
                },
            )
        };
        assert_eq!(
            p("ifAdminStatus.2", None, "down").unwrap().value,
            Value::Integer(2)
        );
        assert_eq!(
            p("ifAdminStatus.2", None, "testing(3)").unwrap().value,
            Value::Integer(3)
        );
        assert_eq!(
            p("ifAdminStatus.2", Some("i"), "up").unwrap().value,
            Value::Integer(1)
        );
        assert_eq!(
            p("sysName.0", None, "rtr").unwrap().value,
            Value::OctetString(b"rtr".to_vec())
        );
        assert_eq!(
            p("sysObjectID.0", Some("o"), "ifIndex").unwrap().value,
            Value::Oid("1.3.6.1.2.1.2.2.1.1".parse().unwrap())
        );
        assert_eq!(
            p("1.3.6.1.4.1.1.1.0", Some("u"), "5").unwrap().value,
            Value::Gauge32(5)
        );
        assert!(
            p("1.3.6.1.4.1.1.1.0", None, "5").is_err(),
            "unknown object needs a type"
        );
        assert!(p("ifAdminStatus.2", None, "sideways").is_err());
        assert_eq!(
            p("ifPhysAddress.1", None, "00:11:22:33:44:55")
                .unwrap()
                .value,
            Value::OctetString(vec![0, 0x11, 0x22, 0x33, 0x44, 0x55])
        );
    }
}
