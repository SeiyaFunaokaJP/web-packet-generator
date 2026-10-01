//! JSON shapes of the wpg-server API (`/api/openapi.json`). Mirrors rather than
//! imports the server types: wpg-server / wpg-tx do not build for WASM. Every
//! struct is lenient (`#[serde(default)]`) so additive server changes never break
//! the UI.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// `GET /api/auth/status`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct AuthStatus {
    pub(crate) configured: bool,
    pub(crate) authenticated: bool,
    pub(crate) setup_required: bool,
    /// User name (or object) when logged in.
    pub(crate) user: Option<Value>,
    pub(crate) csrf_token: Option<String>,
}

/// `GET /api/version`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct VersionInfo {
    pub(crate) version: String,
    pub(crate) api_version: u32,
    pub(crate) os: String,
    pub(crate) arch: String,
    pub(crate) features: Vec<String>,
    pub(crate) locales: Vec<String>,
}

/// One entry of `GET /api/system/ifaces`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Iface {
    pub(crate) name: String,
    pub(crate) index: u32,
    pub(crate) description: Option<String>,
    pub(crate) driver: Option<String>,
    pub(crate) mac: Option<String>,
    pub(crate) mtu: Option<u32>,
    pub(crate) ipv4: Vec<String>,
    pub(crate) ipv6: Vec<String>,
    pub(crate) up: bool,
    pub(crate) running: bool,
    pub(crate) loopback: bool,
    pub(crate) speed_mbps: Option<u64>,
    pub(crate) usable: bool,
    pub(crate) allowed: bool,
}

/// `GET /api/system/doctor`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct DoctorReport {
    pub(crate) ok: bool,
    pub(crate) checks: Vec<DoctorCheck>,
}

/// One doctor check (`status`: `ok`, `warn`, `fail`, `skip`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct DoctorCheck {
    pub(crate) id: String,
    pub(crate) status: String,
    pub(crate) summary: String,
    pub(crate) detail: Option<String>,
    pub(crate) fix: Option<String>,
}

impl DoctorReport {
    /// (ok, warn, fail) counts.
    pub(crate) fn counts(&self) -> (usize, usize, usize) {
        let n = |s: &str| self.checks.iter().filter(|c| c.status == s).count();
        (n("ok"), n("warn"), n("fail"))
    }
}

/// `GET /api/system/backends`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Backends {
    pub(crate) default: String,
    pub(crate) backends: Vec<BackendItem>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct BackendItem {
    pub(crate) name: String,
    pub(crate) available: bool,
    pub(crate) reason: Option<String>,
    pub(crate) multi_worker: bool,
}

/// Template index entry.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct TemplateSummary {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) tags: Vec<String>,
    pub(crate) protocols: Vec<String>,
    pub(crate) author: Option<String>,
    /// `user` or `library`.
    pub(crate) source: String,
    pub(crate) path: String,
    pub(crate) writable: bool,
    pub(crate) favorite: bool,
    pub(crate) last_used: Option<u64>,
    pub(crate) modified: Option<u64>,
    pub(crate) error: Option<String>,
}

/// `GET /api/templates`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct TemplateList {
    pub(crate) templates: Vec<TemplateSummary>,
    pub(crate) recent: Vec<String>,
}

/// Filters of `GET /api/templates`.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct TemplateQuery {
    pub(crate) q: String,
    pub(crate) tag: Option<String>,
    pub(crate) favorite: bool,
    /// `user` / `library`.
    pub(crate) source: Option<String>,
}

/// A lint finding (`level`: `error` / `warning`, localised `message`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct LintIssue {
    pub(crate) level: String,
    pub(crate) path: String,
    pub(crate) key: String,
    pub(crate) message: String,
}

/// `POST /api/lint`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct LintResponse {
    pub(crate) ok: bool,
    pub(crate) errors: usize,
    pub(crate) warnings: usize,
    pub(crate) issues: Vec<LintIssue>,
}

/// `GET /api/templates/{id}` (and the write endpoints).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct TemplateDetail {
    pub(crate) summary: TemplateSummary,
    pub(crate) template: Value,
    pub(crate) issues: Vec<LintIssue>,
}

/// `{ "state": "finished", "detail": "count_reached" }`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct JobStatus {
    pub(crate) state: String,
    pub(crate) detail: Option<String>,
}

impl JobStatus {
    pub(crate) fn is_terminal(&self) -> bool {
        matches!(self.state.as_str(), "finished" | "failed")
    }
}

/// A serialised `std::time::Duration`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Dur {
    pub(crate) secs: u64,
    pub(crate) nanos: u32,
}

impl Dur {
    #[allow(clippy::cast_precision_loss)] // display only
    pub(crate) fn as_secs_f64(self) -> f64 {
        self.secs as f64 + f64::from(self.nanos) * 1e-9
    }
}

/// Per-stream counters.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct StreamStats {
    pub(crate) packets: u64,
    pub(crate) bytes: u64,
    pub(crate) pps: f64,
}

/// `wpg_tx::job::StatsSnapshot`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Stats {
    pub(crate) t: Dur,
    pub(crate) packets: u64,
    pub(crate) bytes: u64,
    pub(crate) dropped: u64,
    pub(crate) pps: f64,
    pub(crate) bps: f64,
    pub(crate) l1_bps: f64,
    pub(crate) rate_achieved_ratio: Option<f64>,
    pub(crate) per_stream: Vec<StreamStats>,
    pub(crate) state: JobStatus,
}

/// `wpg_tx::rate::RateAccuracy`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Accuracy {
    pub(crate) target: Option<f64>,
    pub(crate) achieved: f64,
    /// `pps` or `bps`.
    pub(crate) unit: String,
    pub(crate) ratio: Option<f64>,
    pub(crate) max_jitter: Dur,
    pub(crate) mean_jitter: Dur,
    pub(crate) frames: u64,
    pub(crate) active: Dur,
}

/// `wpg_tx::job::WorkerReport`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct WorkerReport {
    pub(crate) index: usize,
    pub(crate) state: JobStatus,
    pub(crate) packets: u64,
    pub(crate) bytes: u64,
    pub(crate) dropped: u64,
    pub(crate) active: Dur,
    pub(crate) core: Option<usize>,
    pub(crate) pinned: Option<bool>,
}

/// `wpg_tx::job::JobReport` (the parts the UI shows; the raw JSON is kept for
/// download).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Report {
    pub(crate) accuracy: Accuracy,
    pub(crate) backend_detail: Option<String>,
    pub(crate) pin_error: Option<String>,
    pub(crate) workers: Vec<WorkerReport>,
}

/// One stream of a job (request and response).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct JobStream {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) template_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) template: Option<Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) vars: BTreeMap<String, String>,
    pub(crate) weight: u32,
}

/// `JobSpec` + `start` (`POST /api/jobs`); also the `spec` of a job.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct JobSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    pub(crate) streams: Vec<JobStream>,
    /// `weighted`, `sequential`, `round_robin`.
    pub(crate) mix: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) iface: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) backend: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) rate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) duration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) burst: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) workers: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) dry_run: Option<String>,
}

/// `POST /api/jobs` body.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct CreateJob {
    #[serde(flatten)]
    pub(crate) spec: JobSpec,
    pub(crate) start: bool,
}

/// `JobInfo`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct JobInfo {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) spec: JobSpec,
    pub(crate) status: JobStatus,
    pub(crate) backend: String,
    pub(crate) iface: String,
    pub(crate) rate: String,
    pub(crate) workers: usize,
    pub(crate) created: u64,
    pub(crate) started: Option<u64>,
    pub(crate) finished: Option<u64>,
    pub(crate) stats: Option<Stats>,
    /// The raw report (for download); see [`JobInfo::report_view`].
    pub(crate) report: Option<Value>,
}

impl JobInfo {
    pub(crate) fn report_view(&self) -> Option<Report> {
        self.report
            .as_ref()
            .and_then(|r| serde_json::from_value(r.clone()).ok())
    }
}

/// `GET /api/stats/overview`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Overview {
    pub(crate) uptime_secs: u64,
    pub(crate) totals: Totals,
    pub(crate) jobs: Vec<JobBrief>,
    pub(crate) templates: usize,
    pub(crate) history: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Totals {
    pub(crate) active: usize,
    pub(crate) running: usize,
    pub(crate) pps: f64,
    pub(crate) bps: f64,
    pub(crate) l1_bps: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct JobBrief {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) status: JobStatus,
    pub(crate) backend: String,
    pub(crate) iface: String,
    pub(crate) packets: u64,
    pub(crate) pps: f64,
    pub(crate) bps: f64,
}

/// A server log line (WebSocket `log`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct LogLine {
    pub(crate) ts: String,
    /// `TRACE` … `ERROR`.
    pub(crate) level: String,
    pub(crate) target: String,
    pub(crate) message: String,
    pub(crate) fields: serde_json::Map<String, Value>,
}

/// One entry of `GET /api/protocols` (the palette).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct ProtocolSummary {
    /// Stable id (`ethernet2`, `ipv4`, ...).
    pub(crate) id: String,
    /// Display name.
    pub(crate) name: String,
    /// Palette classification (`l2`, `l3`, `l4`, `l7`, `payload`).
    pub(crate) layer: String,
    /// Field names in wire order.
    pub(crate) fields: Vec<String>,
    /// Protocols this one can carry (its next-layer binding table).
    pub(crate) next: Vec<String>,
    /// Content generator instead of wire fields (payload).
    pub(crate) synth: bool,
}

/// One packet of `POST /api/render`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct RenderedPacket {
    /// Stream index.
    pub(crate) index: u64,
    /// Frame bytes as lower-case hex.
    pub(crate) bytes_hex: String,
    /// Length in octets.
    pub(crate) len: usize,
    /// Layer / field positions (`wpg_core::LayerLayout`).
    pub(crate) layout: Value,
    /// Padding / FCS after the layers.
    pub(crate) trailers: Value,
    /// Builder findings with localised `message`.
    pub(crate) diagnostics: Vec<Value>,
}

/// `POST /api/render` response.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct RenderResponse {
    /// The packets.
    pub(crate) packets: Vec<RenderedPacket>,
    /// Packets in the whole stream (`null` = unbounded).
    pub(crate) cardinality: Option<u64>,
    /// How a job would expand the stream (pre-render vs on-the-fly).
    pub(crate) plan: Value,
}

/// Server → client WebSocket message.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ServerEvent {
    Stats {
        job_id: String,
        snapshot: Stats,
    },
    JobState {
        job_id: String,
        state: JobStatus,
    },
    Log(LogLine),
    TemplatesChanged,
    #[serde(other)]
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_server_events() {
        let ev: ServerEvent = serde_json::from_str(
            r#"{"type":"stats","job_id":"a","snapshot":{"t":{"secs":1,"nanos":500000000},
            "packets":10,"bytes":600,"dropped":0,"pps":10.0,"bps":4800.0,"l1_bps":6720.0,
            "rate_achieved_ratio":null,"per_stream":[{"packets":10,"bytes":600,"pps":10.0}],
            "state":{"state":"running"}}}"#,
        )
        .unwrap();
        let ServerEvent::Stats { job_id, snapshot } = ev else {
            panic!("{ev:?}")
        };
        assert_eq!(job_id, "a");
        assert!((snapshot.t.as_secs_f64() - 1.5).abs() < 1e-9);
        assert_eq!(snapshot.state.state, "running");

        let ev: ServerEvent = serde_json::from_str(
            r#"{"type":"job_state","job_id":"a","state":{"state":"finished","detail":"stopped"}}"#,
        )
        .unwrap();
        assert!(matches!(ev, ServerEvent::JobState { state, .. } if state.is_terminal()));

        let ev: ServerEvent = serde_json::from_str(
            r#"{"type":"log","ts":"2026-09-29T00:00:00.000Z","level":"INFO","target":"wpg","message":"hi"}"#,
        )
        .unwrap();
        assert!(matches!(ev, ServerEvent::Log(l) if l.message == "hi"));
        let ev: ServerEvent = serde_json::from_str(r#"{"type":"templates_changed"}"#).unwrap();
        assert_eq!(ev, ServerEvent::TemplatesChanged);
        let ev: ServerEvent = serde_json::from_str(r#"{"type":"future_thing","x":1}"#).unwrap();
        assert_eq!(ev, ServerEvent::Unknown);
    }

    #[test]
    fn create_job_body() {
        let body = CreateJob {
            spec: JobSpec {
                streams: vec![JobStream {
                    template_id: Some("arp".into()),
                    weight: 1,
                    ..JobStream::default()
                }],
                mix: "weighted".into(),
                backend: Some("null".into()),
                rate: Some("1Mpps".into()),
                ..JobSpec::default()
            },
            start: false,
        };
        assert_eq!(
            serde_json::to_value(&body).unwrap(),
            serde_json::json!({
                "streams": [{"template_id": "arp", "weight": 1}],
                "mix": "weighted", "backend": "null", "rate": "1Mpps", "start": false
            })
        );
    }
}
