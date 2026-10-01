//! `/api/render`, `/api/parse`, `/api/lint`: packet preview and checks. The
//! browser does the same with wpg-core compiled to WASM; the server uses the very
//! same crates, so both agree byte for byte.

// utoipa's generated code spells out paths the lint considers redundant.
#![allow(unused_qualifications)]

use std::collections::BTreeMap;
use std::fmt::Write as _;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;
use wpg_core::stream::{Expansion, PlanConfig};
use wpg_i18n::Locale;
use wpg_template::{apply_overrides, compile, lint, LintLevel, Template};

use crate::error::{codes, ApiError, ApiJson, ApiResult};
use crate::localize::{diagnostic_json, lint_issue_json, request_locale, tr};
use crate::state::AppState;

/// Most packets one preview returns.
pub const MAX_PREVIEW: u64 = 256;

/// Parses a template given as an object (JSON form) or a string (YAML text).
///
/// # Errors
/// `invalid_template` (422) with the parser message.
pub fn template_from_value(v: &Value) -> Result<Template, ApiError> {
    match v {
        Value::String(s) => Template::from_yaml_str(s),
        other => Template::from_json_str(&other.to_string()),
    }
    .map_err(ApiError::template)
}

/// Lower-case hex without separators.
#[must_use]
pub fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Parses hex, ignoring whitespace, `:`, `-`, `,` and `0x` prefixes (Wireshark
/// "Copy as Hex Stream", `xxd -p`, `aa:bb:...`).
///
/// # Errors
/// A message for odd-length or non-hex input.
pub fn from_hex(text: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = text
        .split(|c: char| c.is_whitespace() || matches!(c, ':' | '-' | ','))
        .map(|w| {
            w.strip_prefix("0x")
                .or_else(|| w.strip_prefix("0X"))
                .unwrap_or(w)
        })
        .collect();
    if !cleaned.len().is_multiple_of(2) {
        return Err(format!("odd number of hex digits ({})", cleaned.len()));
    }
    (0..cleaned.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&cleaned[i..i + 2], 16)
                .map_err(|_| format!("invalid hex `{}` at offset {i}", &cleaned[i..i + 2]))
        })
        .collect()
}

/// `POST /api/render` request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct RenderRequest {
    /// Template: object (JSON form of `.wpg.yaml`) or YAML text.
    #[schema(value_type = Object)]
    pub template: Value,
    /// Variable values.
    #[serde(default)]
    pub vars: BTreeMap<String, String>,
    /// Packets to render (1..=256, default 1).
    #[serde(default)]
    pub count: Option<u64>,
    /// Stream index of the first packet (default 0).
    #[serde(default)]
    pub index: Option<u64>,
}

/// One rendered packet.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RenderedPacket {
    /// Stream index.
    pub index: u64,
    /// Frame bytes as lower-case hex (without FCS unless the template asks for one).
    pub bytes_hex: String,
    /// Length in octets.
    pub len: usize,
    /// Layer / field positions (`wpg_core::LayerLayout`).
    #[schema(value_type = Vec<Object>)]
    pub layout: Value,
    /// Padding / FCS after the layers.
    #[schema(value_type = Vec<Object>)]
    pub trailers: Value,
    /// Builder findings with localised `message`.
    #[schema(value_type = Vec<Object>)]
    pub diagnostics: Vec<Value>,
}

/// `POST /api/render` response.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RenderResponse {
    /// The packets.
    pub packets: Vec<RenderedPacket>,
    /// Packets in the whole stream (`null` = unbounded).
    pub cardinality: Option<u64>,
    /// How a job would expand the stream (pre-render vs on-the-fly).
    #[schema(value_type = Object)]
    pub plan: Value,
}

/// Renders packets of a template (preview).
#[utoipa::path(post, path = "/api/render", tag = "packets", request_body = RenderRequest,
    responses(
        (status = 200, description = "Rendered packets", body = RenderResponse),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody),
        (status = 422, description = "Invalid template (details: lint issues)", body = crate::error::ErrorBody)))]
pub async fn render(
    State(st): State<AppState>,
    headers: HeaderMap,
    ApiJson(req): ApiJson<RenderRequest>,
) -> ApiResult<Json<RenderResponse>> {
    let locale = request_locale(&headers);
    let count = req.count.unwrap_or(1);
    if count == 0 || count > MAX_PREVIEW {
        return Err(ApiError::bad_request("api-error-count-range", count).arg("max", MAX_PREVIEW));
    }
    let catalog = st.catalog;
    tokio::task::spawn_blocking(move || render_blocking(&req, count, catalog, locale))
        .await
        .map_err(ApiError::internal)?
        .map(Json)
}

fn render_blocking(
    req: &RenderRequest,
    count: u64,
    catalog: &'static wpg_protocols::Catalog,
    locale: Locale,
) -> ApiResult<RenderResponse> {
    let template = template_from_value(&req.template)?;
    let compiled = compile(&template, &req.vars, catalog).map_err(ApiError::template)?;
    let exp = Expansion::new(&compiled.stream, None).map_err(|e| ApiError::template(e.into()))?;
    let cardinality = exp.len();
    let start = req.index.unwrap_or(0);
    let mut packets = Vec::new();
    let mut first_len = None;
    for i in start..start.saturating_add(count) {
        if cardinality.is_some_and(|n| i >= n) {
            break;
        }
        let mut p = compiled.packet.clone();
        if !exp.is_empty() {
            let o = exp.nth(i).map_err(|e| ApiError::template(e.into()))?;
            apply_overrides(&mut p, &o).map_err(ApiError::template)?;
        }
        let built = p.build(catalog).map_err(|e| ApiError::template(e.into()))?;
        first_len.get_or_insert(built.bytes.len());
        packets.push(RenderedPacket {
            index: i,
            bytes_hex: to_hex(&built.bytes),
            len: built.bytes.len(),
            layout: serde_json::to_value(&built.layout).map_err(ApiError::internal)?,
            trailers: serde_json::to_value(&built.trailers).map_err(ApiError::internal)?,
            diagnostics: built
                .diagnostics
                .iter()
                .map(|d| diagnostic_json(d, locale))
                .collect(),
        });
    }
    let plan = exp.plan(first_len.unwrap_or(0), &PlanConfig::default());
    Ok(RenderResponse {
        packets,
        cardinality,
        plan: serde_json::to_value(plan).map_err(ApiError::internal)?,
    })
}

/// `POST /api/parse` request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ParseRequest {
    /// Frame bytes as hex (whitespace, `:` / `-` separators and `0x` allowed).
    pub bytes_hex: String,
    /// Protocol of the first octet (default `ethernet2`).
    #[serde(default)]
    pub first_protocol: Option<String>,
    /// The frame ends with a 4-octet FCS.
    #[serde(default)]
    pub fcs: bool,
}

/// `POST /api/parse` response.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ParseResponse {
    /// Decoded packet (`wpg_core::Packet`: layers with Manual field values).
    #[schema(value_type = Object)]
    pub packet: Value,
    /// Layer / field positions.
    #[schema(value_type = Vec<Object>)]
    pub layout: Value,
    /// Padding / FCS.
    #[schema(value_type = Vec<Object>)]
    pub trailers: Value,
    /// Parse and build findings with localised `message`.
    #[schema(value_type = Vec<Object>)]
    pub diagnostics: Vec<Value>,
    /// Frame length in octets.
    pub len: usize,
    /// Normalised hex of the input.
    pub bytes_hex: String,
}

/// Decodes raw bytes into layers (Hex paste import).
#[utoipa::path(post, path = "/api/parse", tag = "packets", request_body = ParseRequest,
    responses(
        (status = 200, description = "Decoded packet", body = ParseResponse),
        (status = 400, description = "Invalid hex", body = crate::error::ErrorBody),
        (status = 422, description = "Cannot be decoded", body = crate::error::ErrorBody)))]
pub async fn parse(
    State(st): State<AppState>,
    headers: HeaderMap,
    ApiJson(req): ApiJson<ParseRequest>,
) -> ApiResult<Json<ParseResponse>> {
    let locale = request_locale(&headers);
    let bytes =
        from_hex(&req.bytes_hex).map_err(|e| ApiError::bad_request("api-error-invalid-hex", e))?;
    let first = req.first_protocol.as_deref().unwrap_or("ethernet2");
    if st.catalog.get(first).is_none() {
        return Err(ApiError::not_found(format!("protocol {first}")));
    }
    let parsed = if req.fcs {
        wpg_core::parse_with_fcs(&bytes, first, st.catalog)
    } else {
        wpg_core::parse(&bytes, first, st.catalog)
    }
    .map_err(|e| {
        ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            codes::BAD_REQUEST,
            "api-error-parse-failed",
        )
        .arg("error", e)
    })?;
    Ok(Json(ParseResponse {
        packet: serde_json::to_value(&parsed.packet).map_err(ApiError::internal)?,
        layout: serde_json::to_value(&parsed.layout).map_err(ApiError::internal)?,
        trailers: serde_json::to_value(&parsed.trailers).map_err(ApiError::internal)?,
        diagnostics: parsed
            .diagnostics
            .iter()
            .map(|d| diagnostic_json(d, locale))
            .collect(),
        len: bytes.len(),
        bytes_hex: to_hex(&bytes),
    }))
}

/// `POST /api/lint` request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct LintRequest {
    /// Template: object or YAML text.
    #[schema(value_type = Object)]
    pub template: Value,
}

/// `POST /api/lint` response.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LintResponse {
    /// No errors (warnings allowed).
    pub ok: bool,
    /// Number of errors.
    pub errors: usize,
    /// Number of warnings.
    pub warnings: usize,
    /// Findings (`level`, `path`, `key`, `args`, localised `message`).
    #[schema(value_type = Vec<Object>)]
    pub issues: Vec<Value>,
}

/// Checks a template (`wpg template lint`). A template that does not even parse
/// is reported as a single error at path `""`.
#[utoipa::path(post, path = "/api/lint", tag = "packets", request_body = LintRequest,
    responses(
        (status = 200, description = "Lint result", body = LintResponse),
        (status = 400, description = "Bad request", body = crate::error::ErrorBody)))]
pub async fn lint_template(
    State(st): State<AppState>,
    headers: HeaderMap,
    ApiJson(req): ApiJson<LintRequest>,
) -> Json<LintResponse> {
    let locale = request_locale(&headers);
    let t = match &req.template {
        Value::String(s) => Template::from_yaml_str(s),
        other => Template::from_json_str(&other.to_string()),
    };
    let issues: Vec<Value> = match t {
        Ok(t) => lint(&t, st.catalog)
            .iter()
            .map(|i| lint_issue_json(i, locale))
            .collect(),
        Err(e) => vec![json!({
            "level": LintLevel::Error,
            "path": "",
            "key": "lint-parse-error",
            "args": { "error": e.to_string() },
            "message": tr(locale, "lint-parse-error", &[("error", e.to_string())]),
        })],
    };
    let count = |l: &str| issues.iter().filter(|i| i["level"] == l).count();
    let errors = count("error");
    Json(LintResponse {
        ok: errors == 0,
        errors,
        warnings: count("warning"),
        issues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        assert_eq!(
            from_hex("0x01 02:03-0A,ff").unwrap(),
            vec![1, 2, 3, 10, 255]
        );
        assert_eq!(to_hex(&[0, 0xab]), "00ab");
        assert!(from_hex("abc").is_err());
        assert!(from_hex("zz").is_err());
    }
}
