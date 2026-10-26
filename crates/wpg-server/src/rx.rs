//! Capture manager (ROADMAP M9 "受信 / 検証").
//!
//! A capture is a [`CaptureSpec`]: a receive backend on an interface, a filter, an
//! optional packet / time limit, how many matching frames to keep for display, an
//! optional pcapng file under `<data_dir>/captures` and signature verification.
//! Creating one opens the backend and starts a [`wpg_rx::Capture`]; a tokio task
//! per capture samples it every 100 ms and publishes [`ServerEvent::RxStats`] /
//! [`ServerEvent::CaptureState`]. Finished captures stay listed (the last
//! [`KEEP_FINISHED`]) until deleted; their kept frames and pcapng file remain
//! available.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use wpg_rx::backend::{self, RxOptions};
use wpg_rx::capture::{CaptureState, CapturedPacket, RxSnapshot};
use wpg_rx::{Capture, CaptureConfig, CaptureHandle, Filter, RxError};
use wpg_tx::rate::parse_duration;

use crate::config::Config;
use crate::error::{codes, ApiError};
use crate::events::{EventBus, ServerEvent};
use crate::jobs::capture_path;

/// Finished captures kept in memory.
pub const KEEP_FINISHED: usize = 20;

/// Most frames a capture may keep for display.
pub const MAX_KEEP: usize = 10_000;

/// Default number of frames kept for display.
pub const DEFAULT_KEEP: usize = 100;

const fn default_keep() -> usize {
    DEFAULT_KEEP
}

/// What to capture (all fields are optional).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(default)]
pub struct CaptureSpec {
    /// Display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Backend: `afpacket`, `pcap`, `pcapfile`, `null` (default: `afpacket` on
    /// Linux, else `pcap`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    /// Interface (`any` = every interface with `afpacket`); for `pcapfile` the
    /// name of a capture file under `<data_dir>/captures` (e.g. a dry-run output).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iface: Option<String>,
    /// tcpdump-style filter (`udp port 5001 and wpg`); empty = every frame.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    /// Stop after this many matching frames.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    /// Stop after this long, e.g. `10s`, `2m`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    /// Matching frames kept for display (default 100, at most 10000).
    #[serde(default = "default_keep")]
    pub keep: usize,
    /// Save the matching frames as a pcapng file of this name under
    /// `<data_dir>/captures`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save: Option<String>,
    /// Check the signatures of matching frames (loss / order / latency).
    pub verify: bool,
    /// Promiscuous mode.
    pub promisc: bool,
    /// Also count frames this host transmits.
    pub include_outgoing: bool,
}

impl Default for CaptureSpec {
    fn default() -> Self {
        Self {
            name: None,
            backend: None,
            iface: None,
            filter: None,
            count: None,
            duration: None,
            keep: DEFAULT_KEEP,
            save: None,
            verify: false,
            promisc: false,
            include_outgoing: false,
        }
    }
}

/// A capture as returned by the API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct CaptureInfo {
    /// Capture id.
    pub id: String,
    /// Display name (`spec.name`, else `<backend>:<iface>`).
    pub name: String,
    /// The request.
    pub spec: CaptureSpec,
    /// `{ "state": "running" }`, `{ "state": "finished", "detail":
    /// "count_reached" | "duration_elapsed" | "end_of_input" | "stopped" }`,
    /// `{ "state": "failed", "detail": "<error>" }`.
    #[schema(value_type = Object)]
    pub status: CaptureState,
    /// Resolved backend.
    pub backend: String,
    /// Resolved interface (input path for `pcapfile`).
    pub iface: String,
    /// Normalised filter text (empty = every frame).
    pub filter: String,
    /// Start time, Unix milliseconds.
    pub started: u64,
    /// End time, Unix milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished: Option<u64>,
    /// Latest statistics (`wpg_rx::RxSnapshot`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub stats: Option<RxSnapshot>,
    /// Whether a pcapng file was written (download with
    /// `GET /api/captures/{id}/pcapng`).
    pub saved: bool,
}

struct Entry {
    info: CaptureInfo,
    handle: Option<CaptureHandle>,
    packets: Vec<CapturedPacket>,
    save_path: Option<PathBuf>,
}

struct Inner {
    captures: Mutex<BTreeMap<String, Entry>>,
    events: EventBus,
    config: Arc<Config>,
    interval: Duration,
}

impl Drop for Inner {
    fn drop(&mut self) {
        let caps = self
            .captures
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner);
        for e in caps.values() {
            if let Some(h) = &e.handle {
                h.stop();
            }
        }
    }
}

/// The capture manager (cheap to clone).
#[derive(Clone)]
pub struct RxManager {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for RxManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RxManager")
            .field("captures", &lock(&self.inner.captures).len())
            .finish_non_exhaustive()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn spec_error(e: impl ToString) -> ApiError {
    ApiError::bad_request("api-error-capture-spec", e)
}

fn rx_error(e: RxError) -> ApiError {
    match e {
        RxError::Interface(i) => ApiError::not_found(format!("interface {i}")),
        RxError::Config(m) | RxError::Unsupported(m) => spec_error(m),
        other => ApiError::new(
            StatusCode::BAD_GATEWAY,
            codes::BACKEND_ERROR,
            "api-error-capture-backend",
        )
        .arg("error", other),
    }
}

impl RxManager {
    /// Creates the manager.
    #[must_use]
    pub fn new(config: Arc<Config>, events: EventBus) -> Self {
        Self::with_interval(config, events, crate::jobs::SNAPSHOT_INTERVAL)
    }

    /// [`Self::new`] with a custom statistics cadence.
    #[must_use]
    pub fn with_interval(config: Arc<Config>, events: EventBus, interval: Duration) -> Self {
        Self {
            inner: Arc::new(Inner {
                captures: Mutex::new(BTreeMap::new()),
                events,
                config,
                interval,
            }),
        }
    }

    /// Validates `spec`, opens the backend and starts capturing.
    ///
    /// # Errors
    /// `bad_request` for an invalid spec / filter, `not_found` for an unknown
    /// interface or input file, `backend_error` when the backend cannot be opened.
    pub async fn create(&self, spec: CaptureSpec) -> Result<CaptureInfo, ApiError> {
        let cfg = &self.inner.config;
        let backend_name = spec
            .backend
            .clone()
            .unwrap_or_else(|| backend::default_name().to_owned());
        let iface = match backend_name.as_str() {
            "pcapfile" => {
                let name = spec
                    .iface
                    .as_deref()
                    .ok_or_else(|| spec_error("the pcapfile backend needs `iface` (file name)"))?;
                capture_path(&cfg.capture_dir(), name)?
                    .display()
                    .to_string()
            }
            "null" => spec.iface.clone().unwrap_or_else(|| "null0".to_owned()),
            _ => spec
                .iface
                .clone()
                .ok_or_else(|| spec_error(format!("backend `{backend_name}` needs `iface`")))?,
        };
        let filter = Filter::parse(spec.filter.as_deref().unwrap_or(""))
            .map_err(|e| ApiError::bad_request("api-error-capture-filter", e))?;
        let duration = spec
            .duration
            .as_deref()
            .map(|d| parse_duration(d).map_err(|e| spec_error(format!("duration: {e}"))))
            .transpose()?;
        if spec.keep > MAX_KEEP {
            return Err(spec_error(format!(
                "keep must be at most {MAX_KEEP} (got {})",
                spec.keep
            )));
        }
        if spec.count == Some(0) {
            return Err(spec_error("count must be at least 1"));
        }
        let save_path = spec
            .save
            .as_deref()
            .map(|n| capture_path(&cfg.capture_dir(), n))
            .transpose()?;
        if save_path.is_some()
            && save_path.as_ref().map(|p| p.display().to_string()) == Some(iface.clone())
        {
            return Err(spec_error("`save` must differ from the input file"));
        }
        let opts = RxOptions {
            promisc: spec.promisc,
            include_outgoing: spec.include_outgoing,
            ..RxOptions::default()
        };
        let config = CaptureConfig {
            filter: filter.clone(),
            count: spec.count,
            duration,
            keep: spec.keep,
            save: save_path.clone(),
            verify: spec.verify,
            snapshot_interval: self.inner.interval,
            ..CaptureConfig::default()
        };
        let (b, i) = (backend_name.clone(), iface.clone());
        let handle = tokio::task::spawn_blocking(move || {
            let backend = backend::open_by_name(&b, &i, &opts)?;
            Capture::start(backend, config)
        })
        .await
        .map_err(ApiError::internal)?
        .map_err(rx_error)?;
        let info = CaptureInfo {
            id: uuid::Uuid::new_v4().simple().to_string(),
            name: spec.name.clone().unwrap_or_else(|| {
                format!("{backend_name}:{}", spec.iface.as_deref().unwrap_or(&iface))
            }),
            spec,
            status: CaptureState::Running,
            backend: backend_name,
            iface,
            filter: filter.text().to_owned(),
            started: now_ms(),
            finished: None,
            stats: Some(handle.stats()),
            saved: save_path.is_some(),
        };
        let id = info.id.clone();
        lock(&self.inner.captures).insert(
            id.clone(),
            Entry {
                info: info.clone(),
                handle: Some(handle),
                packets: Vec::new(),
                save_path,
            },
        );
        let _ = self.inner.events.send(ServerEvent::CaptureState {
            capture_id: id.clone(),
            state: CaptureState::Running,
        });
        tracing::info!(capture = %id, backend = %info.backend, iface = %info.iface, "capture started");
        tokio::spawn(monitor(
            Arc::downgrade(&self.inner),
            id,
            self.inner.interval,
        ));
        self.evict();
        Ok(info)
    }

    /// Every capture, newest first.
    #[must_use]
    pub fn list(&self) -> Vec<CaptureInfo> {
        let caps = lock(&self.inner.captures);
        let mut v: Vec<CaptureInfo> = caps.values().map(|e| e.info.clone()).collect();
        v.sort_by(|a, b| b.started.cmp(&a.started).then_with(|| b.id.cmp(&a.id)));
        v
    }

    /// One capture.
    ///
    /// # Errors
    /// `not_found`.
    pub fn get(&self, id: &str) -> Result<CaptureInfo, ApiError> {
        lock(&self.inner.captures)
            .get(id)
            .map(|e| e.info.clone())
            .ok_or_else(|| ApiError::not_found(format!("capture {id}")))
    }

    /// The kept frames (live while running).
    ///
    /// # Errors
    /// `not_found`.
    pub fn packets(&self, id: &str) -> Result<Vec<CapturedPacket>, ApiError> {
        let caps = lock(&self.inner.captures);
        let e = caps
            .get(id)
            .ok_or_else(|| ApiError::not_found(format!("capture {id}")))?;
        Ok(match &e.handle {
            Some(h) => h.packets(),
            None => e.packets.clone(),
        })
    }

    /// Path of the pcapng file of a finished capture.
    ///
    /// # Errors
    /// `not_found` (unknown id or nothing saved), `invalid_state` while running.
    pub fn pcapng_path(&self, id: &str) -> Result<PathBuf, ApiError> {
        let caps = lock(&self.inner.captures);
        let e = caps
            .get(id)
            .ok_or_else(|| ApiError::not_found(format!("capture {id}")))?;
        if e.handle.is_some() {
            return Err(state_error(id, "running", "download"));
        }
        e.save_path
            .clone()
            .ok_or_else(|| ApiError::not_found(format!("pcapng of capture {id}")))
    }

    /// Requests a running capture to stop.
    ///
    /// # Errors
    /// `not_found`, `invalid_state` if it already ended.
    pub fn stop(&self, id: &str) -> Result<CaptureInfo, ApiError> {
        let caps = lock(&self.inner.captures);
        let e = caps
            .get(id)
            .ok_or_else(|| ApiError::not_found(format!("capture {id}")))?;
        match &e.handle {
            Some(h) => {
                h.stop();
                Ok(e.info.clone())
            }
            None => Err(state_error(id, state_name(&e.info.status), "stop")),
        }
    }

    /// Removes a capture (stopping it first). The pcapng file is kept.
    ///
    /// # Errors
    /// `not_found`.
    pub async fn delete(&self, id: &str) -> Result<(), ApiError> {
        let entry = lock(&self.inner.captures)
            .remove(id)
            .ok_or_else(|| ApiError::not_found(format!("capture {id}")))?;
        if let Some(h) = entry.handle {
            tokio::task::spawn_blocking(move || h.stop_and_wait())
                .await
                .map_err(ApiError::internal)?;
        }
        Ok(())
    }

    /// Stops every capture and waits for them (server shutdown).
    pub async fn shutdown(&self) {
        let handles: Vec<CaptureHandle> = lock(&self.inner.captures)
            .values_mut()
            .filter_map(|e| e.handle.take())
            .collect();
        for h in &handles {
            h.stop();
        }
        let _ = tokio::task::spawn_blocking(move || {
            for h in handles {
                let _ = h.wait();
            }
        })
        .await;
    }

    /// Number of running captures.
    #[must_use]
    pub fn running(&self) -> usize {
        lock(&self.inner.captures)
            .values()
            .filter(|e| e.handle.is_some())
            .count()
    }

    fn evict(&self) {
        let mut caps = lock(&self.inner.captures);
        let mut done: Vec<(u64, String)> = caps
            .values()
            .filter(|e| e.handle.is_none())
            .map(|e| (e.info.finished.unwrap_or(0), e.info.id.clone()))
            .collect();
        if done.len() > KEEP_FINISHED {
            done.sort();
            for (_, id) in done.iter().take(done.len() - KEEP_FINISHED) {
                caps.remove(id);
            }
        }
    }
}

fn state_name(s: &CaptureState) -> &'static str {
    match s {
        CaptureState::Running => "running",
        CaptureState::Finished(_) => "finished",
        CaptureState::Failed(_) => "failed",
    }
}

fn state_error(id: &str, state: &str, action: &str) -> ApiError {
    ApiError::new(
        StatusCode::CONFLICT,
        codes::INVALID_STATE,
        "api-error-capture-state",
    )
    .arg("id", id)
    .arg("state", state)
    .arg("action", action)
}

/// Samples a running capture until it ends.
async fn monitor(weak: Weak<Inner>, id: String, interval: Duration) {
    let mut tick = tokio::time::interval(interval);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tick.tick().await;
        let Some(inner) = weak.upgrade() else { return };
        let handle = {
            let mut caps = lock(&inner.captures);
            let Some(e) = caps.get_mut(&id) else { return };
            let Some(h) = e.handle.as_ref() else { return };
            let snap = h.stats();
            e.info.stats = Some(snap.clone());
            let _ = inner.events.send(ServerEvent::RxStats {
                capture_id: id.clone(),
                snapshot: snap,
            });
            if h.is_finished() {
                e.handle.take()
            } else {
                None
            }
        };
        if let Some(h) = handle {
            let packets = h.packets();
            // `wait` itself turns a panicked capture thread into a `Failed` report.
            let report = tokio::task::spawn_blocking(move || h.wait()).await;
            let state = match &report {
                Ok(r) => r.state.clone(),
                Err(e) => CaptureState::Failed(format!("capture task: {e}")),
            };
            if let Some(e) = lock(&inner.captures).get_mut(&id) {
                e.info.status = state.clone();
                if let Ok(r) = &report {
                    e.info.stats = Some(r.stats.clone());
                }
                e.info.finished = Some(now_ms());
                e.packets = packets;
            }
            if let Ok(r) = report {
                let _ = inner.events.send(ServerEvent::RxStats {
                    capture_id: id.clone(),
                    snapshot: r.stats,
                });
            }
            let _ = inner.events.send(ServerEvent::CaptureState {
                capture_id: id.clone(),
                state,
            });
            tracing::info!(capture = %id, "capture ended");
            return;
        }
    }
}
