//! `/api/ws` client: connects, subscribes to all jobs + logs, reconnects with
//! exponential back-off, and feeds the [`Live`] signals.

use std::collections::{HashMap, VecDeque};

use leptos::prelude::*;

use crate::api::{JobStatus, LogLine, RxStats, ServerEvent, Stats};
use crate::sparkline::{push_sample, SPARK_SAMPLES};

/// Log lines kept in the drawer.
pub(crate) const LOG_CAP: usize = 500;
/// Minimum spacing of sparkline samples (the server sends stats every 100 ms, so
/// 60 samples cover the last ~30 s).
pub(crate) const SAMPLE_EVERY_SECS: f64 = 0.5;

/// Connection state (the dot in the top bar).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WsStatus {
    Connecting,
    Open,
    Closed,
}

impl WsStatus {
    pub(crate) const fn label_key(self) -> &'static str {
        match self {
            WsStatus::Connecting => "ui-ws-connecting",
            WsStatus::Open => "ui-ws-open",
            WsStatus::Closed => "ui-ws-closed",
        }
    }
}

/// Live statistics of one job.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct JobLive {
    pub(crate) latest: Stats,
    pub(crate) pps: VecDeque<f64>,
    pub(crate) bps: VecDeque<f64>,
    last_t: Option<f64>,
}

impl JobLive {
    /// Records a snapshot; history samples are spaced by [`SAMPLE_EVERY_SECS`]
    /// (a restart — time going backwards — starts a new series).
    pub(crate) fn record(&mut self, s: Stats) {
        let t = s.t.as_secs_f64();
        let take = match self.last_t {
            Some(last) if t < last => {
                self.pps.clear();
                self.bps.clear();
                true
            }
            Some(last) => t - last >= SAMPLE_EVERY_SECS,
            None => true,
        };
        if take {
            push_sample(&mut self.pps, s.pps, SPARK_SAMPLES);
            push_sample(&mut self.bps, s.bps, SPARK_SAMPLES);
            self.last_t = Some(t);
        }
        self.latest = s;
    }
}

/// A log line with a stable key.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LogEntry {
    pub(crate) seq: u64,
    pub(crate) line: LogLine,
}

/// Signals fed by the WebSocket (context).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Live {
    pub(crate) status: RwSignal<WsStatus>,
    pub(crate) stats: RwSignal<HashMap<String, JobLive>>,
    pub(crate) job_states: RwSignal<HashMap<String, JobStatus>>,
    /// Bumped on every `job_state` (and on reconnect): job lists refetch.
    pub(crate) jobs_rev: RwSignal<u64>,
    /// Bumped on `templates_changed` (and on reconnect).
    pub(crate) templates_rev: RwSignal<u64>,
    pub(crate) logs: RwSignal<VecDeque<LogEntry>>,
    /// Latest receive statistics per capture (`rx_stats`).
    pub(crate) rx_stats: RwSignal<HashMap<String, RxStats>>,
    pub(crate) capture_states: RwSignal<HashMap<String, JobStatus>>,
    /// Bumped on every `capture_state` (and on reconnect): capture lists refetch.
    pub(crate) captures_rev: RwSignal<u64>,
    /// Bumped on every `snmp_trap` (and on reconnect): the trap list refetches.
    pub(crate) snmp_rev: RwSignal<u64>,
    log_seq: StoredValue<u64>,
}

impl Live {
    pub(crate) fn new() -> Self {
        Self {
            status: RwSignal::new(WsStatus::Connecting),
            stats: RwSignal::new(HashMap::new()),
            job_states: RwSignal::new(HashMap::new()),
            jobs_rev: RwSignal::new(0),
            templates_rev: RwSignal::new(0),
            logs: RwSignal::new(VecDeque::new()),
            rx_stats: RwSignal::new(HashMap::new()),
            capture_states: RwSignal::new(HashMap::new()),
            captures_rev: RwSignal::new(0),
            snmp_rev: RwSignal::new(0),
            log_seq: StoredValue::new(0),
        }
    }

    /// Applies one server message.
    pub(crate) fn handle(self, text: &str) {
        let Ok(ev) = serde_json::from_str::<ServerEvent>(text) else {
            return;
        };
        match ev {
            ServerEvent::Stats { job_id, snapshot } => {
                self.stats
                    .update(|m| m.entry(job_id).or_default().record(snapshot));
            }
            ServerEvent::JobState { job_id, state } => {
                self.job_states.update(|m| {
                    m.insert(job_id, state);
                });
                self.jobs_rev.update(|r| *r += 1);
            }
            ServerEvent::Log(line) => {
                let seq = self.log_seq.get_value() + 1;
                self.log_seq.set_value(seq);
                self.logs.update(|l| {
                    l.push_back(LogEntry { seq, line });
                    while l.len() > LOG_CAP {
                        l.pop_front();
                    }
                });
            }
            ServerEvent::TemplatesChanged => self.templates_rev.update(|r| *r += 1),
            ServerEvent::RxStats {
                capture_id,
                snapshot,
            } => {
                self.rx_stats.update(|m| {
                    m.insert(capture_id, snapshot);
                });
            }
            ServerEvent::CaptureState { capture_id, state } => {
                self.capture_states.update(|m| {
                    m.insert(capture_id, state);
                });
                self.captures_rev.update(|r| *r += 1);
            }
            ServerEvent::SnmpTrap { .. } => self.snmp_rev.update(|r| *r += 1),
            ServerEvent::Unknown => {}
        }
    }

    /// Latest stats of a job (tracked).
    pub(crate) fn job(self, id: &str) -> Option<JobLive> {
        self.stats.with(|m| m.get(id).cloned())
    }

    /// Latest receive statistics of a capture (tracked).
    pub(crate) fn capture(self, id: &str) -> Option<RxStats> {
        self.rx_stats.with(|m| m.get(id).cloned())
    }

    /// Forgets a deleted capture.
    pub(crate) fn forget_capture(self, id: &str) {
        self.rx_stats.update(|m| {
            m.remove(id);
        });
        self.capture_states.update(|m| {
            m.remove(id);
        });
    }

    /// Forgets a deleted job.
    pub(crate) fn forget(self, id: &str) {
        self.stats.update(|m| {
            m.remove(id);
        });
    }
}

/// The [`Live`] context.
pub(crate) fn use_live() -> Live {
    expect_context::<Live>()
}

/// Reconnect delay after `attempt` consecutive failures: 0.5 s doubling to 30 s.
pub(crate) fn backoff_ms(attempt: u32) -> u32 {
    let exp = attempt.saturating_sub(1).min(16);
    500u32.saturating_mul(1 << exp).min(30_000)
}

thread_local! {
    /// Set to skip the current back-off (e.g. right after logging in).
    static WAKE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Ends the current reconnect back-off early.
pub(crate) fn reconnect_now() {
    WAKE.with(|w| w.set(true));
}

/// Starts the connection loop (browser only).
pub(crate) fn connect(live: Live) {
    #[cfg(target_arch = "wasm32")]
    leptos::task::spawn_local(run(live));
    #[cfg(not(target_arch = "wasm32"))]
    let _ = live;
}

#[cfg(target_arch = "wasm32")]
async fn run(live: Live) {
    use futures_util::future::{select, Either};
    use futures_util::{SinkExt, StreamExt};
    use gloo_net::websocket::{futures::WebSocket, Message};
    use gloo_timers::future::TimeoutFuture;

    const SUBSCRIBE: &str = r#"{"type":"subscribe","jobs":"all","logs":true}"#;
    let mut attempt = 0u32;
    loop {
        live.status.set(WsStatus::Connecting);
        if let Ok(mut ws) = WebSocket::open(&crate::dom::ws_url("/api/ws")) {
            // The sink waits for the open event; do not wait forever.
            let sent = select(
                Box::pin(ws.send(Message::Text(SUBSCRIBE.to_owned()))),
                TimeoutFuture::new(10_000),
            )
            .await;
            if matches!(sent, Either::Left((Ok(()), _))) {
                live.status.set(WsStatus::Open);
                attempt = 0;
                // Catch up on whatever happened while we were away.
                live.jobs_rev.update(|r| *r += 1);
                live.templates_rev.update(|r| *r += 1);
                live.captures_rev.update(|r| *r += 1);
                live.snmp_rev.update(|r| *r += 1);
                while let Some(msg) = ws.next().await {
                    match msg {
                        Ok(Message::Text(t)) => live.handle(&t),
                        Ok(Message::Bytes(_)) => {}
                        Err(_) => break,
                    }
                }
            }
        }
        live.status.set(WsStatus::Closed);
        attempt = attempt.saturating_add(1);
        WAKE.with(|w| w.set(false));
        let mut waited = 0;
        while waited < backoff_ms(attempt) && !WAKE.with(std::cell::Cell::take) {
            TimeoutFuture::new(250).await;
            waited += 250;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Dur;

    #[test]
    fn backoff() {
        assert_eq!(backoff_ms(1), 500);
        assert_eq!(backoff_ms(2), 1000);
        assert_eq!(backoff_ms(3), 2000);
        assert_eq!(backoff_ms(7), 30_000);
        assert_eq!(backoff_ms(u32::MAX), 30_000);
        assert_eq!(backoff_ms(0), 500);
    }

    fn snap(t_ms: u64, pps: f64) -> Stats {
        Stats {
            t: Dur {
                secs: t_ms / 1000,
                nanos: u32::try_from((t_ms % 1000) * 1_000_000).unwrap(),
            },
            pps,
            ..Stats::default()
        }
    }

    #[test]
    fn samples_are_spaced_and_reset_on_restart() {
        let mut j = JobLive::default();
        for i in 0..10 {
            j.record(snap(i * 100, f64::from(u32::try_from(i).unwrap())));
        }
        // t = 0, 0.5 s.
        assert_eq!(j.pps.iter().copied().collect::<Vec<_>>(), vec![0.0, 5.0]);
        assert!((j.latest.pps - 9.0).abs() < f64::EPSILON);
        j.record(snap(0, 1.0));
        assert_eq!(j.pps.len(), 1);
    }

    #[test]
    fn handles_events() {
        let owner = Owner::new();
        owner.set();
        let live = Live::new();
        live.handle(r#"{"type":"stats","job_id":"j","snapshot":{"pps":3.0}}"#);
        assert_eq!(live.job("j").map(|j| j.latest.pps), Some(3.0));
        live.handle(r#"{"type":"job_state","job_id":"j","state":{"state":"paused"}}"#);
        assert_eq!(live.jobs_rev.get_untracked(), 1);
        for _ in 0..(LOG_CAP + 5) {
            live.handle(r#"{"type":"log","level":"INFO","message":"m"}"#);
        }
        assert_eq!(live.logs.with_untracked(VecDeque::len), LOG_CAP);
        live.handle(r#"{"type":"templates_changed"}"#);
        assert_eq!(live.templates_rev.get_untracked(), 1);
        live.handle(r#"{"type":"rx_stats","capture_id":"c","snapshot":{"matched":9}}"#);
        assert_eq!(live.capture("c").map(|s| s.matched), Some(9));
        live.handle(r#"{"type":"capture_state","capture_id":"c","state":{"state":"finished","detail":"stopped"}}"#);
        assert_eq!(live.captures_rev.get_untracked(), 1);
        live.forget_capture("c");
        assert!(live.capture("c").is_none());
        live.handle("not json");
        live.forget("j");
        assert!(live.job("j").is_none());
    }
}
