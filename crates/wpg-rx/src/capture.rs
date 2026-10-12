//! Capture sessions: a backend, a filter, counters, the first N frames, an
//! optional pcapng file and signature verification, on a dedicated thread.
//!
//! [`Capture::start`] returns a [`CaptureHandle`] that can
//! [`stop`](CaptureHandle::stop) the session, read the latest [`RxSnapshot`] (published
//! every [`CaptureConfig::snapshot_interval`]), the kept frames
//! ([`CaptureHandle::packets`]) and [`wait`](CaptureHandle::wait) for the final
//! [`CaptureReport`]. Dropping the handle stops and joins the thread; the backend
//! is closed and the pcapng file flushed on every exit path.

use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use wpg_core::signature::Signature;
use wpg_template::PcapngWriter;

use crate::backend::{RxBackend, RxBackendStats, RxPacket, RxStatus};
use crate::filter::Filter;
use crate::verify::{Verifier, VerifyReport};
use crate::{RxCounters, RxError};

/// Capture parameters.
#[derive(Debug, Clone)]
pub struct CaptureConfig {
    /// Frames that count as "matched" (and are kept / saved / verified).
    pub filter: Filter,
    /// Stop after this many matching frames.
    pub count: Option<u64>,
    /// Stop after this much time.
    pub duration: Option<Duration>,
    /// Keep the first `keep` matching frames for display.
    pub keep: usize,
    /// Write the matching frames to this pcapng file.
    pub save: Option<PathBuf>,
    /// Check the signatures of matching frames ([`crate::verify`]).
    pub verify: bool,
    /// Frames per `recv` call.
    pub batch: usize,
    /// Interval between [`RxSnapshot`]s.
    pub snapshot_interval: Duration,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            filter: Filter::all(),
            count: None,
            duration: None,
            keep: 100,
            save: None,
            verify: false,
            batch: 256,
            snapshot_interval: Duration::from_millis(100),
        }
    }
}

/// Why a capture ended normally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    /// [`CaptureConfig::count`] matching frames were received.
    CountReached,
    /// [`CaptureConfig::duration`] elapsed.
    DurationElapsed,
    /// The input ended (`pcapfile`).
    EndOfInput,
    /// [`CaptureHandle::stop`] (or the handle was dropped).
    Stopped,
}

/// Lifecycle state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state", content = "detail")]
pub enum CaptureState {
    /// Receiving.
    Running,
    /// Ended normally.
    Finished(EndReason),
    /// Ended by a backend or file error.
    Failed(String),
}

impl CaptureState {
    /// Whether the capture has ended.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        !matches!(self, CaptureState::Running)
    }
}

/// Periodic statistics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RxSnapshot {
    /// Time since the capture started.
    pub t: Duration,
    /// Frames received.
    pub packets: u64,
    /// Octets received (original lengths, without FCS).
    pub bytes: u64,
    /// Frames matching the filter.
    pub matched: u64,
    /// Octets of the matching frames.
    pub matched_bytes: u64,
    /// Frames per second over the last interval (final report: average).
    pub pps: f64,
    /// Bits per second over the last interval (final report: average).
    pub bps: f64,
    /// Matching frames per second.
    pub matched_pps: f64,
    /// Matching bits per second.
    pub matched_bps: f64,
    /// Frames dropped by the kernel / driver (when the backend knows).
    pub dropped: u64,
    /// Frames written to the pcapng file.
    pub saved: u64,
    /// Frames kept for display.
    pub kept: usize,
    /// Signature verification (with [`CaptureConfig::verify`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify: Option<VerifyReport>,
    /// State at the time of the snapshot.
    pub state: CaptureState,
}

impl RxSnapshot {
    fn empty(verify: bool) -> Self {
        Self {
            t: Duration::ZERO,
            packets: 0,
            bytes: 0,
            matched: 0,
            matched_bytes: 0,
            pps: 0.0,
            bps: 0.0,
            matched_pps: 0.0,
            matched_bps: 0.0,
            dropped: 0,
            saved: 0,
            kept: 0,
            verify: verify.then(VerifyReport::default),
            state: CaptureState::Running,
        }
    }
}

/// Signature fields of a kept frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureInfo {
    /// Stream id.
    pub stream_id: u32,
    /// Sequence number.
    pub seq: u64,
    /// TX time, nanoseconds since the epoch.
    pub ts_ns: u64,
}

impl From<Signature> for SignatureInfo {
    fn from(s: Signature) -> Self {
        Self {
            stream_id: s.stream_id,
            seq: s.seq,
            ts_ns: s.ts_ns,
        }
    }
}

/// One of the first N matching frames.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedPacket {
    /// Position among the matching frames (0-based).
    pub index: u64,
    /// Capture time, nanoseconds since the epoch.
    pub ts_ns: u64,
    /// Original length.
    pub len: usize,
    /// Captured octets.
    pub data: Vec<u8>,
    /// The frame's signature, if it carries one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<SignatureInfo>,
}

/// Final result of a capture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptureReport {
    /// Terminal state.
    pub state: CaptureState,
    /// Counters at the end; rates are averages over the whole capture.
    pub stats: RxSnapshot,
    /// Backend counters.
    pub backend: RxBackendStats,
    /// Backend name.
    pub backend_name: String,
    /// The pcapng file, if one was written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved_path: Option<PathBuf>,
}

struct Shared {
    stop: AtomicBool,
    latest: Mutex<RxSnapshot>,
    packets: Mutex<Vec<CapturedPacket>>,
    started: Instant,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Starts capture sessions.
#[derive(Debug)]
pub struct Capture;

impl Capture {
    /// Starts receiving from the (already open) `backend` on a new thread.
    ///
    /// # Errors
    /// [`RxError::Config`] for a zero batch size, [`RxError::Io`] if the pcapng
    /// file cannot be created or the thread cannot be spawned.
    pub fn start(
        backend: Box<dyn RxBackend>,
        config: CaptureConfig,
    ) -> Result<CaptureHandle, RxError> {
        if config.batch == 0 {
            return Err(RxError::Config("batch size must be at least 1".to_owned()));
        }
        let writer = match &config.save {
            Some(p) => {
                let f = File::create(p).map_err(|e| {
                    RxError::Io(std::io::Error::new(
                        e.kind(),
                        format!("{}: {e}", p.display()),
                    ))
                })?;
                Some(PcapngWriter::new(BufWriter::with_capacity(1 << 20, f))?)
            }
            None => None,
        };
        let shared = Arc::new(Shared {
            stop: AtomicBool::new(false),
            latest: Mutex::new(RxSnapshot::empty(config.verify)),
            packets: Mutex::new(Vec::new()),
            started: Instant::now(),
        });
        let sh = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("wpg-rx".to_owned())
            .spawn(move || {
                Session {
                    sh,
                    backend,
                    writer,
                    verifier: Verifier::new(),
                    counters: RxCounters::new(),
                    saved: 0,
                    kept: 0,
                    write_error: None,
                }
                .run(&config)
            })?;
        Ok(CaptureHandle {
            shared,
            thread: Some(thread),
        })
    }
}

/// Controls a running capture.
pub struct CaptureHandle {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<CaptureReport>>,
}

impl std::fmt::Debug for CaptureHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CaptureHandle")
            .field("state", &self.stats().state)
            .finish_non_exhaustive()
    }
}

impl CaptureHandle {
    /// Requests the capture to stop; returns immediately (see [`Self::wait`]).
    pub fn stop(&self) {
        self.shared.stop.store(true, Ordering::SeqCst);
    }

    /// Latest snapshot.
    #[must_use]
    pub fn stats(&self) -> RxSnapshot {
        lock(&self.shared.latest).clone()
    }

    /// The kept frames (at most [`CaptureConfig::keep`]).
    #[must_use]
    pub fn packets(&self) -> Vec<CapturedPacket> {
        lock(&self.shared.packets).clone()
    }

    /// Whether the capture thread has ended.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
    }

    /// Waits for the capture to end.
    #[must_use]
    pub fn wait(mut self) -> CaptureReport {
        self.join()
    }

    /// [`Self::stop`] + [`Self::wait`].
    #[must_use]
    pub fn stop_and_wait(self) -> CaptureReport {
        self.stop();
        self.wait()
    }

    fn join(&mut self) -> CaptureReport {
        let r = self.thread.take().map(JoinHandle::join);
        match r {
            Some(Ok(rep)) => rep,
            Some(Err(_)) | None => CaptureReport {
                state: CaptureState::Failed("capture thread panicked".to_owned()),
                stats: self.stats(),
                backend: RxBackendStats::default(),
                backend_name: String::new(),
                saved_path: None,
            },
        }
    }
}

impl Drop for CaptureHandle {
    fn drop(&mut self) {
        if self.thread.is_some() {
            self.stop();
            let _ = self.join();
        }
    }
}

struct Session {
    sh: Arc<Shared>,
    backend: Box<dyn RxBackend>,
    writer: Option<PcapngWriter<BufWriter<File>>>,
    verifier: Verifier,
    counters: RxCounters,
    saved: u64,
    kept: usize,
    write_error: Option<String>,
}

#[derive(Default, Clone, Copy)]
struct Mark {
    packets: u64,
    bytes: u64,
    matched: u64,
    matched_bytes: u64,
}

impl Session {
    fn snapshot(
        &self,
        cfg: &CaptureConfig,
        since: &Mark,
        dt: f64,
        state: CaptureState,
    ) -> RxSnapshot {
        let c = &self.counters;
        let dt = dt.max(1e-9);
        RxSnapshot {
            t: self.sh.started.elapsed(),
            packets: c.packets,
            bytes: c.bytes,
            matched: c.matched,
            matched_bytes: c.matched_bytes,
            pps: c.packets.saturating_sub(since.packets) as f64 / dt,
            bps: c.bytes.saturating_sub(since.bytes) as f64 * 8.0 / dt,
            matched_pps: c.matched.saturating_sub(since.matched) as f64 / dt,
            matched_bps: c.matched_bytes.saturating_sub(since.matched_bytes) as f64 * 8.0 / dt,
            dropped: self.backend.stats().dropped,
            saved: self.saved,
            kept: self.kept,
            verify: cfg.verify.then(|| self.verifier.report()),
            state,
        }
    }

    fn mark(&self) -> Mark {
        Mark {
            packets: self.counters.packets,
            bytes: self.counters.bytes,
            matched: self.counters.matched,
            matched_bytes: self.counters.matched_bytes,
        }
    }

    /// Handles one frame; returns `true` when the count is reached.
    fn handle(&mut self, cfg: &CaptureConfig, p: RxPacket) -> bool {
        let matched = cfg.filter.matches(&p.data);
        self.counters.record(p.len, matched);
        if !matched {
            return false;
        }
        let index = self.counters.matched - 1;
        let sig = if cfg.verify {
            self.verifier.record_frame(&p.data, p.ts_ns)
        } else if self.kept < cfg.keep {
            wpg_core::signature::find(&p.data)
        } else {
            None
        };
        if let Some(w) = &mut self.writer {
            match w.write_packet(p.ts_ns / 1_000, &p.data) {
                Ok(()) => self.saved += 1,
                Err(e) => {
                    self.write_error = Some(format!("pcapng: {e}"));
                }
            }
        }
        if self.kept < cfg.keep {
            self.kept += 1;
            lock(&self.sh.packets).push(CapturedPacket {
                index,
                ts_ns: p.ts_ns,
                len: p.len,
                data: p.data,
                signature: sig.map(SignatureInfo::from),
            });
        }
        cfg.count.is_some_and(|c| self.counters.matched >= c)
    }

    fn run(mut self, cfg: &CaptureConfig) -> CaptureReport {
        let interval = cfg.snapshot_interval.max(Duration::from_millis(1));
        let max_block = Duration::from_millis(20);
        let mut buf: Vec<RxPacket> = Vec::with_capacity(cfg.batch);
        let mut last = (Instant::now(), self.mark());
        let state = 'outer: loop {
            if self.sh.stop.load(Ordering::Relaxed) {
                break CaptureState::Finished(EndReason::Stopped);
            }
            if cfg.count.is_some_and(|c| self.counters.matched >= c) {
                break CaptureState::Finished(EndReason::CountReached);
            }
            let elapsed = self.sh.started.elapsed();
            if cfg.duration.is_some_and(|d| elapsed >= d) {
                break CaptureState::Finished(EndReason::DurationElapsed);
            }
            let block = cfg
                .duration
                .map_or(max_block, |d| max_block.min(d.saturating_sub(elapsed)));
            buf.clear();
            let status = match self.backend.recv(&mut buf, cfg.batch, block) {
                Ok(s) => s,
                Err(e) => break CaptureState::Failed(format!("{}: {e}", self.backend.name())),
            };
            for p in buf.drain(..) {
                if self.handle(cfg, p) {
                    break 'outer CaptureState::Finished(EndReason::CountReached);
                }
            }
            if let Some(e) = self.write_error.take() {
                break CaptureState::Failed(e);
            }
            if status == RxStatus::Eof {
                break CaptureState::Finished(EndReason::EndOfInput);
            }
            let now = Instant::now();
            if now.duration_since(last.0) >= interval {
                let snap = self.snapshot(
                    cfg,
                    &last.1,
                    now.duration_since(last.0).as_secs_f64(),
                    CaptureState::Running,
                );
                *lock(&self.sh.latest) = snap;
                last = (now, self.mark());
            }
        };
        self.finish(cfg, state)
    }

    fn finish(mut self, cfg: &CaptureConfig, mut state: CaptureState) -> CaptureReport {
        if let Some(w) = self.writer.take() {
            if let Err(e) = w.finish() {
                if !matches!(state, CaptureState::Failed(_)) {
                    state = CaptureState::Failed(format!("pcapng: {e}"));
                }
            }
        }
        let backend = self.backend.stats();
        let name = self.backend.name().to_owned();
        if let Err(e) = self.backend.close() {
            if !matches!(state, CaptureState::Failed(_)) {
                state = CaptureState::Failed(format!("{name}: close: {e}"));
            }
        }
        let total = self.sh.started.elapsed().as_secs_f64();
        let mut stats = self.snapshot(cfg, &Mark::default(), total, state.clone());
        stats.dropped = backend.dropped;
        *lock(&self.sh.latest) = stats.clone();
        CaptureReport {
            state,
            stats,
            backend,
            backend_name: name,
            saved_path: cfg.save.clone(),
        }
    }
}
