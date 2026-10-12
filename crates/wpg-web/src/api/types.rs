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
    /// Stamp frames with a receive-verification signature.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) signature: bool,
    /// Signature stream id of the first stream (stream `i` gets `stream_id + i`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stream_id: Option<u32>,
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

/// One entry of `GET /api/captures/backends`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct RxBackendInfo {
    pub(crate) name: String,
    pub(crate) available: bool,
    pub(crate) reason: Option<String>,
}

/// `CaptureSpec` (`POST /api/captures`); also the `spec` of a capture.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct CaptureSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    /// `afpacket`, `pcap`, `pcapfile`, `null`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) backend: Option<String>,
    /// Interface, or a capture file name for `pcapfile`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) iface: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) filter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) duration: Option<String>,
    /// Matching frames kept for display.
    pub(crate) keep: usize,
    /// pcapng file name under the server's capture directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) save: Option<String>,
    pub(crate) verify: bool,
    pub(crate) promisc: bool,
    pub(crate) include_outgoing: bool,
}

/// Verification result of one signature stream (`wpg_rx::StreamReport`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct StreamVerify {
    pub(crate) stream_id: u32,
    pub(crate) received: u64,
    pub(crate) first_seq: u64,
    pub(crate) last_seq: u64,
    pub(crate) lost: u64,
    pub(crate) out_of_order: u64,
    pub(crate) duplicates: u64,
    pub(crate) latency_min_ns: i64,
    pub(crate) latency_avg_ns: i64,
    pub(crate) latency_max_ns: i64,
    pub(crate) jitter_ns: i64,
}

impl StreamVerify {
    /// Lost / expected (0 when nothing is expected).
    #[allow(clippy::cast_precision_loss)] // display only
    pub(crate) fn loss_ratio(&self) -> f64 {
        let expected = self
            .last_seq
            .saturating_sub(self.first_seq)
            .saturating_add(1);
        self.lost as f64 / expected as f64
    }
}

/// Verification summary (`wpg_rx::VerifyReport`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct VerifyReport {
    pub(crate) signed: u64,
    pub(crate) unsigned: u64,
    pub(crate) streams: Vec<StreamVerify>,
}

impl VerifyReport {
    /// Sum of the streams' estimated losses.
    pub(crate) fn lost(&self) -> u64 {
        self.streams.iter().map(|s| s.lost).sum()
    }
}

/// Receive statistics (`wpg_rx::RxSnapshot`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct RxStats {
    pub(crate) t: Dur,
    pub(crate) packets: u64,
    pub(crate) bytes: u64,
    pub(crate) matched: u64,
    pub(crate) matched_bytes: u64,
    pub(crate) pps: f64,
    pub(crate) bps: f64,
    pub(crate) matched_pps: f64,
    pub(crate) matched_bps: f64,
    pub(crate) dropped: u64,
    pub(crate) saved: u64,
    pub(crate) kept: usize,
    pub(crate) verify: Option<VerifyReport>,
    pub(crate) state: JobStatus,
}

/// `CaptureInfo`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct CaptureInfo {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) spec: CaptureSpec,
    /// `running`, `finished` (detail: end reason), `failed` (detail: error).
    pub(crate) status: JobStatus,
    pub(crate) backend: String,
    pub(crate) iface: String,
    /// Normalised filter (empty = every frame).
    pub(crate) filter: String,
    pub(crate) started: u64,
    pub(crate) finished: Option<u64>,
    pub(crate) stats: Option<RxStats>,
    /// A pcapng file was written.
    pub(crate) saved: bool,
}

/// Signature fields of a kept frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct SignatureInfo {
    pub(crate) stream_id: u32,
    pub(crate) seq: u64,
    pub(crate) ts_ns: u64,
}

/// One kept frame (`GET /api/captures/{id}/packets`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct CapturedFrame {
    pub(crate) index: u64,
    /// Capture time, nanoseconds since the Unix epoch.
    pub(crate) ts_ns: u64,
    /// Original length.
    pub(crate) len: usize,
    /// Captured octets as hex.
    pub(crate) hex: String,
    pub(crate) signature: Option<SignatureInfo>,
}

/// `POST /api/captures/filter` response.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct FilterCheck {
    pub(crate) ok: bool,
    pub(crate) normalized: Option<String>,
    pub(crate) error: Option<String>,
    /// Byte offset of the error.
    pub(crate) pos: Option<usize>,
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
    RxStats {
        capture_id: String,
        snapshot: RxStats,
    },
    CaptureState {
        capture_id: String,
        state: JobStatus,
    },
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
        let ev: ServerEvent = serde_json::from_str(
            r#"{"type":"rx_stats","capture_id":"c","snapshot":{"t":{"secs":2,"nanos":0},
            "packets":5,"bytes":300,"matched":4,"matched_bytes":240,"pps":2.5,"bps":1200.0,
            "matched_pps":2.0,"matched_bps":960.0,"dropped":0,"saved":4,"kept":4,
            "verify":{"signed":4,"unsigned":0,"streams":[{"stream_id":7,"received":4,
            "first_seq":0,"last_seq":4,"lost":1,"out_of_order":0,"duplicates":0,
            "latency_min_ns":1000,"latency_avg_ns":1500,"latency_max_ns":2000,"jitter_ns":10}]},
            "state":{"state":"running"}}}"#,
        )
        .unwrap();
        let ServerEvent::RxStats {
            capture_id,
            snapshot,
        } = ev
        else {
            panic!("{ev:?}")
        };
        assert_eq!(capture_id, "c");
        assert_eq!(snapshot.matched, 4);
        let v = snapshot.verify.unwrap();
        assert_eq!(v.lost(), 1);
        assert!((v.streams[0].loss_ratio() - 0.2).abs() < 1e-9);
        let ev: ServerEvent = serde_json::from_str(
            r#"{"type":"capture_state","capture_id":"c","state":{"state":"finished","detail":"end_of_input"}}"#,
        )
        .unwrap();
        assert!(matches!(ev, ServerEvent::CaptureState { state, .. } if state.is_terminal()));
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

    #[test]
    fn capture_and_signature_bodies() {
        let spec = CaptureSpec {
            backend: Some("pcapfile".into()),
            iface: Some("run.pcapng".into()),
            keep: 100,
            verify: true,
            ..CaptureSpec::default()
        };
        assert_eq!(
            serde_json::to_value(&spec).unwrap(),
            serde_json::json!({
                "backend": "pcapfile", "iface": "run.pcapng", "keep": 100,
                "verify": true, "promisc": false, "include_outgoing": false
            })
        );
        let job = JobSpec {
            signature: true,
            stream_id: Some(5),
            ..JobSpec::default()
        };
        let v = serde_json::to_value(&job).unwrap();
        assert_eq!(v["signature"], true);
        assert_eq!(v["stream_id"], 5);
        let v = serde_json::to_value(JobSpec::default()).unwrap();
        assert!(v.get("signature").is_none() && v.get("stream_id").is_none());

        let info: CaptureInfo = serde_json::from_str(
            r#"{"id":"x","name":"pcapfile:run.pcapng","spec":{"backend":"pcapfile","iface":"run.pcapng","keep":100,"verify":true,"promisc":false,"include_outgoing":false},
            "status":{"state":"failed","detail":"boom"},"backend":"pcapfile","iface":"/d/captures/run.pcapng","filter":"","started":1,"saved":false}"#,
        )
        .unwrap();
        assert_eq!(info.status.detail.as_deref(), Some("boom"));
        assert!(info.spec.verify && info.stats.is_none());
        let f: Vec<CapturedFrame> = serde_json::from_str(
            r#"[{"index":0,"ts_ns":5,"len":60,"hex":"ff","signature":{"stream_id":1,"seq":2,"ts_ns":3}},{"index":1,"ts_ns":6,"len":60,"hex":"00"}]"#,
        )
        .unwrap();
        assert_eq!(f[0].signature.map(|s| s.seq), Some(2));
        assert!(f[1].signature.is_none());
    }
}
