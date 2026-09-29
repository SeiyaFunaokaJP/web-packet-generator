//! TX jobs: frame sources through backends on dedicated threads.
//!
//! [`TxJob::start`] runs one [`FrameSource`] through one [`TxBackend`];
//! [`TxJob::start_workers`] runs several (source, backend) pairs in parallel — one
//! thread, backend instance (e.g. `AF_PACKET` socket / ring) and slice of the
//! stream per worker ([`crate::source::Rendered::split`]), each pacing at
//! `rate / N` ([`Rate::split`]). [`TxJob::start_on`] opens the backends by name.
//!
//! Both return a [`JobHandle`] that can [`pause`](JobHandle::pause) /
//! [`resume`](JobHandle::resume) / [`stop`](JobHandle::stop) every worker at once,
//! read live counters ([`JobHandle::stats`], summed over the workers), receive a
//! [`StatsSnapshot`] every [`JobConfig::snapshot_interval`] (100 ms by default;
//! [`JobHandle::snapshots`]) and [`wait`](JobHandle::wait) for the final
//! [`JobReport`].
//!
//! A small monitor thread publishes the snapshots, waits for
//! [`JobConfig::start_at`] (state [`JobState::Scheduled`]) before releasing the
//! workers, and joins them at the end. Each worker optionally pins itself to a core
//! ([`JobConfig::pin_cores`]; Linux `sched_setaffinity`, a no-op reported as
//! `pinned: false` elsewhere). Nothing busy loops while paused, scheduled or
//! stopped: pauses park the threads, and the pacer only spins for the last few
//! microseconds before a deadline. On every exit path each backend is closed before
//! its thread ends; a failing worker stops the others; dropping the handle stops
//! and joins the job.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::thread::{JoinHandle, Thread};
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};

use crate::rate::{ramp_fraction, Pacer, PacerConfig, Rate, RateAccuracy, RateUnit};
use crate::source::FrameSource;
use crate::{backend, Frame, TxBackend, TxError, TxOptions, TxStats};

/// Job parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct JobConfig {
    /// Stop after this many frames in total (`None` = until the source ends /
    /// duration). Split between workers like the stream.
    pub count: Option<u64>,
    /// Stop after this much active (not paused) time.
    pub duration: Option<Duration>,
    /// Target rate of the whole job (each of `N` workers paces at `rate / N`).
    pub rate: Rate,
    /// Frames per token-bucket grant (see [`PacerConfig::burst`]).
    pub burst: u32,
    /// Core to pin the (first) TX thread to when [`Self::pin_cores`] is empty.
    pub pin_core: Option<usize>,
    /// Core per worker: worker `i` is pinned to `pin_cores[i]` (workers beyond the
    /// list are not pinned).
    pub pin_cores: Vec<usize>,
    /// Number of TX workers [`TxJob::start_on`] opens (default 1). `start` /
    /// `start_workers` use as many workers as they are given.
    pub workers: usize,
    /// Frames per `send_batch` call (upper bound; pacing may send fewer).
    pub batch_size: usize,
    /// Pacer wait strategy etc. (`burst` / `ramp_up` are taken from this config).
    pub pacer: PacerConfig,
    /// Interval between [`StatsSnapshot`]s.
    pub snapshot_interval: Duration,
    /// Linear rate ramp from 0 to [`Self::rate`] over this much active time (needs
    /// a finite rate).
    pub ramp_up: Option<Duration>,
    /// Wall-clock time to start sending at; until then the job is
    /// [`JobState::Scheduled`] (a time in the past starts immediately).
    pub start_at: Option<SystemTime>,
}

impl Default for JobConfig {
    fn default() -> Self {
        Self {
            count: None,
            duration: None,
            rate: Rate::Unlimited,
            burst: 1,
            pin_core: None,
            pin_cores: Vec::new(),
            workers: 1,
            batch_size: 64,
            pacer: PacerConfig::default(),
            snapshot_interval: Duration::from_millis(100),
            ramp_up: None,
            start_at: None,
        }
    }
}

impl JobConfig {
    /// Core of worker `i`.
    fn core_of(&self, i: usize) -> Option<usize> {
        if self.pin_cores.is_empty() {
            (i == 0).then_some(self.pin_core).flatten()
        } else {
            self.pin_cores.get(i).copied()
        }
    }
}

/// Why a job ended normally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    /// [`JobConfig::count`] frames were sent.
    CountReached,
    /// [`JobConfig::duration`] elapsed.
    DurationElapsed,
    /// The source has no more frames.
    SourceExhausted,
    /// [`JobHandle::stop`] (or the handle was dropped).
    Stopped,
}

/// Lifecycle state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state", content = "detail")]
pub enum JobState {
    /// Waiting for [`JobConfig::start_at`].
    Scheduled,
    /// Sending.
    Running,
    /// Paused by [`JobHandle::pause`].
    Paused,
    /// Ended normally.
    Finished(FinishReason),
    /// Ended by a backend or source error.
    Failed(String),
}

impl JobState {
    /// Whether the job has ended.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, JobState::Finished(_) | JobState::Failed(_))
    }
}

/// Counters of one stream (see [`FrameSource::stream_index`]).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct StreamStats {
    /// Frames accepted by the backend.
    pub packets: u64,
    /// Octets accepted by the backend.
    pub bytes: u64,
    /// Frames per second over the last snapshot interval.
    pub pps: f64,
}

/// Periodic statistics (summed over all workers).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatsSnapshot {
    /// Time since the job started.
    pub t: Duration,
    /// Frames accepted by the backend.
    pub packets: u64,
    /// Octets accepted by the backend (without FCS).
    pub bytes: u64,
    /// Frames the backend did not accept.
    pub dropped: u64,
    /// Frames per second over the last interval.
    pub pps: f64,
    /// Layer-2 bits per second (frame bytes × 8) over the last interval.
    pub bps: f64,
    /// Layer-1 bits per second (incl. preamble, IFG, FCS, padding) over the last
    /// interval.
    pub l1_bps: f64,
    /// Achieved / target rate since the start, taking the ramp-up into account
    /// (`None` when unlimited).
    pub rate_achieved_ratio: Option<f64>,
    /// Per-stream counters.
    pub per_stream: Vec<StreamStats>,
    /// State at the time of the snapshot.
    pub state: JobState,
}

impl StatsSnapshot {
    fn empty(streams: usize) -> Self {
        Self {
            t: Duration::ZERO,
            packets: 0,
            bytes: 0,
            dropped: 0,
            pps: 0.0,
            bps: 0.0,
            l1_bps: 0.0,
            rate_achieved_ratio: None,
            per_stream: vec![StreamStats::default(); streams],
            state: JobState::Running,
        }
    }
}

/// Final counters of one worker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerReport {
    /// Worker index.
    pub index: usize,
    /// How the worker ended.
    pub state: JobState,
    /// Frames accepted by its backend.
    pub packets: u64,
    /// Octets accepted by its backend.
    pub bytes: u64,
    /// Frames its backend did not accept.
    pub dropped: u64,
    /// Its active (not paused) sending time.
    pub active: Duration,
    /// Requested core.
    pub core: Option<usize>,
    /// Whether it was pinned (`None` if no pinning was requested).
    pub pinned: Option<bool>,
}

/// Final result of a job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobReport {
    /// Terminal state.
    pub state: JobState,
    /// Counters at the end; `pps` / `bps` are averages over the active time.
    pub stats: StatsSnapshot,
    /// Pacing accuracy (summed over the workers).
    pub accuracy: RateAccuracy,
    /// Backend counters at the end (summed over the workers).
    pub backend: TxStats,
    /// Backend name.
    pub backend_name: String,
    /// [`TxBackend::detail`] of the first worker.
    pub backend_detail: Option<String>,
    /// Active (not paused) time (the longest of the workers).
    pub active: Duration,
    /// Whether the threads were pinned (`None` if no pinning was requested;
    /// `Some(false)` if any requested pin failed).
    pub pinned: Option<bool>,
    /// Pinning errors, if any (the workers still run unpinned).
    pub pin_error: Option<String>,
    /// Per-worker results.
    pub workers: Vec<WorkerReport>,
}

/// Live counters of one worker, on their own cache lines.
#[repr(align(128))]
struct Counters {
    packets: AtomicU64,
    bytes: AtomicU64,
    dropped: AtomicU64,
    /// Octets on the wire incl. padding / preamble / IFG / FCS.
    l1_bytes: AtomicU64,
    /// Attempted frames (charged to the pacer).
    attempted: AtomicU64,
    active_ns: AtomicU64,
    per_stream: Box<[(AtomicU64, AtomicU64)]>,
}

impl Counters {
    fn new(streams: usize) -> Self {
        Self {
            packets: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            l1_bytes: AtomicU64::new(0),
            attempted: AtomicU64::new(0),
            active_ns: AtomicU64::new(0),
            per_stream: (0..streams)
                .map(|_| (AtomicU64::new(0), AtomicU64::new(0)))
                .collect(),
        }
    }
}

struct Shared {
    stop: AtomicBool,
    pause: AtomicBool,
    /// Workers may send (set by the monitor after `start_at`).
    go: AtomicBool,
    paused_workers: AtomicUsize,
    live_workers: AtomicUsize,
    workers: Vec<Counters>,
    streams: usize,
    /// First failure (stops the other workers).
    failed: Mutex<Option<String>>,
    /// Terminal state, once known.
    done: Mutex<Option<JobState>>,
    latest: Mutex<StatsSnapshot>,
    threads: Mutex<Vec<Thread>>,
    monitor: OnceLock<Thread>,
    started: Instant,
    rate: Rate,
    burst: u32,
    ramp_up: Option<Duration>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Sums of the per-worker counters.
#[derive(Default)]
struct Totals {
    packets: u64,
    bytes: u64,
    dropped: u64,
    l1_bytes: u64,
    attempted: u64,
    active: Duration,
    per_stream: Vec<(u64, u64)>,
}

impl Shared {
    fn wake_all(&self) {
        for t in lock(&self.threads).iter() {
            t.unpark();
        }
        if let Some(m) = self.monitor.get() {
            m.unpark();
        }
    }

    fn state(&self) -> JobState {
        if let Some(s) = lock(&self.done).clone() {
            return s;
        }
        if !self.go.load(Ordering::SeqCst) {
            return JobState::Scheduled;
        }
        let live = self.live_workers.load(Ordering::SeqCst);
        if self.pause.load(Ordering::SeqCst)
            && live > 0
            && self.paused_workers.load(Ordering::SeqCst) >= live
        {
            JobState::Paused
        } else {
            JobState::Running
        }
    }

    fn fail(&self, msg: String) {
        lock(&self.failed).get_or_insert(msg);
        self.stop.store(true, Ordering::SeqCst);
        self.wake_all();
    }

    fn totals(&self) -> Totals {
        let mut t = Totals {
            per_stream: vec![(0, 0); self.streams],
            ..Totals::default()
        };
        for w in &self.workers {
            t.packets += w.packets.load(Ordering::Relaxed);
            t.bytes += w.bytes.load(Ordering::Relaxed);
            t.dropped += w.dropped.load(Ordering::Relaxed);
            t.l1_bytes += w.l1_bytes.load(Ordering::Relaxed);
            t.attempted += w.attempted.load(Ordering::Relaxed);
            t.active = t
                .active
                .max(Duration::from_nanos(w.active_ns.load(Ordering::Relaxed)));
            for (acc, (p, b)) in t.per_stream.iter_mut().zip(w.per_stream.iter()) {
                acc.0 += p.load(Ordering::Relaxed);
                acc.1 += b.load(Ordering::Relaxed);
            }
        }
        t
    }

    /// Achieved / target since the start (ramp-aware). `None` until the target
    /// amounts to a few bursts, when the first grant would dominate the ratio.
    fn ratio(&self, t: &Totals) -> Option<f64> {
        let secs = t.active.as_secs_f64();
        let f = ramp_fraction(t.active, self.ramp_up);
        let min_frames = 4.0 * f64::from(self.burst.max(8)) * self.workers.len() as f64;
        let (done, expected, per_frame) = match self.rate.bits_per_second() {
            Some(bps) => (
                t.l1_bytes as f64 * 8.0,
                bps * secs * f,
                crate::rate::wire_bits(crate::rate::MIN_FRAME_LEN),
            ),
            None => (
                t.attempted as f64,
                self.rate.frames_per_second(crate::rate::MIN_FRAME_LEN)? * secs * f,
                1.0,
            ),
        };
        (expected >= min_frames * per_frame).then(|| done / expected)
    }
}

/// One worker's backend and source.
pub type WorkerSetup = (Box<dyn TxBackend>, Box<dyn FrameSource>);

/// Starts TX jobs.
#[derive(Debug)]
pub struct TxJob;

impl TxJob {
    /// Starts sending `source` through the (already open) `backend` on a new
    /// thread.
    ///
    /// # Errors
    /// [`TxError::Config`] for an invalid rate (e.g. an unresolved link
    /// percentage), a zero batch size, a ramp-up without a finite rate or
    /// `config.workers > 1` (use [`Self::start_workers`] / [`Self::start_on`]);
    /// [`TxError::Io`] if a thread cannot be spawned.
    pub fn start(
        backend: Box<dyn TxBackend>,
        source: Box<dyn FrameSource>,
        config: JobConfig,
    ) -> Result<JobHandle, TxError> {
        if config.workers > 1 {
            return Err(TxError::Config(format!(
                "{} workers need one backend and source each (TxJob::start_workers)",
                config.workers
            )));
        }
        Self::start_workers(vec![(backend, source)], config)
    }

    /// Opens `config.workers` instances of the backend `name` on `iface` (see
    /// [`backend::open_workers`]) and starts one worker per source; `sources` must
    /// hold one source per worker (e.g. from [`crate::source::Rendered::split`]),
    /// fewer sources start fewer workers.
    ///
    /// # Errors
    /// Backend open errors (e.g. [`TxError::Config`] for several workers on a
    /// file / pcap backend) and the errors of [`Self::start_workers`].
    pub fn start_on(
        name: &str,
        iface: &str,
        opts: &TxOptions,
        sources: Vec<Box<dyn FrameSource>>,
        config: JobConfig,
    ) -> Result<JobHandle, TxError> {
        let backends = backend::open_workers(name, iface, opts, sources.len().max(1))?;
        Self::start_workers(backends.into_iter().zip(sources).collect(), config)
    }

    /// Starts one worker thread per (backend, source) pair. The job's count and
    /// rate are split evenly between the workers; per-stream counters are summed
    /// (stream `i` of every worker counts as stream `i`).
    ///
    /// # Errors
    /// See [`Self::start`]; also [`TxError::Config`] for an empty list.
    pub fn start_workers(
        workers: Vec<WorkerSetup>,
        config: JobConfig,
    ) -> Result<JobHandle, TxError> {
        let n = workers.len();
        if n == 0 {
            return Err(TxError::Config(
                "a job needs at least one worker".to_owned(),
            ));
        }
        if config.batch_size == 0 {
            return Err(TxError::Config("batch size must be at least 1".to_owned()));
        }
        if config.ramp_up.is_some_and(|r| !r.is_zero()) && config.rate.is_unlimited() {
            return Err(TxError::Config(
                "ramp-up needs a finite rate (e.g. --rate 1Mpps)".to_owned(),
            ));
        }
        let pacer_cfg = PacerConfig {
            burst: config.burst.max(1),
            ramp_up: config.ramp_up.filter(|r| !r.is_zero()),
            ..config.pacer
        };
        let worker_rate = config.rate.split(n);
        // Validate now so the caller gets the error synchronously.
        Pacer::new(worker_rate, pacer_cfg)?;
        let streams = workers
            .iter()
            .map(|(_, s)| s.stream_count())
            .max()
            .unwrap_or(1)
            .max(1);
        let counts: Vec<Option<u64>> = match config.count {
            None => vec![None; n],
            Some(c) => {
                let mut v: Vec<Option<u64>> = wpg_core::stream::partition(c, n)
                    .into_iter()
                    .map(|r| Some(r.end - r.start))
                    .collect();
                v.resize(n, Some(0));
                v
            }
        };
        let shared = Arc::new(Shared {
            stop: AtomicBool::new(false),
            pause: AtomicBool::new(false),
            go: AtomicBool::new(false),
            paused_workers: AtomicUsize::new(0),
            live_workers: AtomicUsize::new(n),
            workers: (0..n).map(|_| Counters::new(streams)).collect(),
            streams,
            failed: Mutex::new(None),
            done: Mutex::new(None),
            latest: Mutex::new(StatsSnapshot::empty(streams)),
            threads: Mutex::new(Vec::new()),
            monitor: OnceLock::new(),
            started: Instant::now(),
            rate: config.rate,
            burst: pacer_cfg.burst,
            ramp_up: pacer_cfg.ramp_up,
        });
        let mut handles = Vec::with_capacity(n);
        for (i, ((backend, source), count)) in workers.into_iter().zip(counts).enumerate() {
            let w = Worker {
                index: i,
                sh: Arc::clone(&shared),
                backend,
                source,
                count,
                duration: config.duration,
                rate: worker_rate,
                pacer_cfg,
                batch: config.batch_size,
                core: config.core_of(i),
            };
            let spawned = std::thread::Builder::new()
                .name(format!("wpg-tx-{i}"))
                .spawn(move || w.run());
            match spawned {
                Ok(h) => {
                    lock(&shared.threads).push(h.thread().clone());
                    handles.push(h);
                }
                Err(e) => {
                    abort(&shared, handles);
                    return Err(e.into());
                }
            }
        }
        let (tx, rx) = mpsc::sync_channel(64);
        let sh = Arc::clone(&shared);
        let cfg = config;
        let monitor = std::thread::Builder::new()
            .name("wpg-tx-monitor".to_owned())
            .spawn(move || Monitor { sh, cfg, tx }.run(handles));
        let monitor = match monitor {
            Ok(m) => m,
            Err(e) => {
                // The worker handles moved into the failed closure were dropped
                // (detached); stop them anyway so their backends get closed.
                shared.stop.store(true, Ordering::SeqCst);
                shared.go.store(true, Ordering::SeqCst);
                shared.wake_all();
                return Err(e.into());
            }
        };
        let _ = shared.monitor.set(monitor.thread().clone());
        monitor.thread().unpark();
        Ok(JobHandle {
            shared,
            thread: Some(monitor),
            rx,
        })
    }
}

fn abort(sh: &Shared, handles: Vec<JoinHandle<WorkerOut>>) {
    sh.stop.store(true, Ordering::SeqCst);
    sh.go.store(true, Ordering::SeqCst);
    sh.wake_all();
    for h in handles {
        let _ = h.join();
    }
}

/// Controls a running job. Dropping it stops the job and joins the threads.
pub struct JobHandle {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<JobReport>>,
    rx: Receiver<StatsSnapshot>,
}

impl std::fmt::Debug for JobHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobHandle")
            .field("state", &self.state())
            .field("workers", &self.workers())
            .finish_non_exhaustive()
    }
}

impl JobHandle {
    /// Pauses sending on every worker (the threads park; pause time does not count
    /// towards [`JobConfig::duration`], the ramp-up or the rate accuracy).
    pub fn pause(&self) {
        self.shared.pause.store(true, Ordering::SeqCst);
        self.shared.wake_all();
    }

    /// Resumes after [`Self::pause`].
    pub fn resume(&self) {
        self.shared.pause.store(false, Ordering::SeqCst);
        self.shared.wake_all();
    }

    /// Requests the job to stop (also a scheduled one); returns immediately (see
    /// [`Self::wait`]).
    pub fn stop(&self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        self.shared.wake_all();
    }

    /// Current state.
    #[must_use]
    pub fn state(&self) -> JobState {
        self.shared.state()
    }

    /// Number of worker threads.
    #[must_use]
    pub fn workers(&self) -> usize {
        self.shared.workers.len()
    }

    /// Whether the job has ended (the threads may still be closing the backends;
    /// [`Self::wait`] joins them).
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
    }

    /// Live statistics: cumulative counters summed over the workers now, rates
    /// from the latest snapshot.
    #[must_use]
    pub fn stats(&self) -> StatsSnapshot {
        let mut s = lock(&self.shared.latest).clone();
        let t = self.shared.totals();
        s.t = self.shared.started.elapsed();
        s.packets = t.packets;
        s.bytes = t.bytes;
        s.dropped = t.dropped;
        for (st, (p, b)) in s.per_stream.iter_mut().zip(&t.per_stream) {
            st.packets = *p;
            st.bytes = *b;
        }
        s.state = self.state();
        s
    }

    /// Snapshots published every [`JobConfig::snapshot_interval`] (bounded queue:
    /// if nobody reads, newer snapshots are dropped — use [`Self::stats`] for the
    /// latest). The channel disconnects when the job ends.
    #[must_use]
    pub fn snapshots(&self) -> &Receiver<StatsSnapshot> {
        &self.rx
    }

    /// Waits for the job to end and returns its report.
    ///
    /// # Panics
    /// Re-raises a panic of the monitor or a TX thread.
    #[must_use]
    pub fn wait(mut self) -> JobReport {
        let t = self.thread.take().expect("job thread already joined");
        match t.join() {
            Ok(r) => r,
            Err(p) => std::panic::resume_unwind(p),
        }
    }

    /// [`Self::stop`] followed by [`Self::wait`].
    #[must_use]
    pub fn stop_and_wait(self) -> JobReport {
        self.stop();
        self.wait()
    }
}

impl Drop for JobHandle {
    fn drop(&mut self) {
        if let Some(t) = self.thread.take() {
            self.stop();
            let _ = t.join();
        }
    }
}

/// What a worker thread returns.
struct WorkerOut {
    state: JobState,
    accuracy: Option<RateAccuracy>,
    backend: TxStats,
    name: &'static str,
    detail: Option<String>,
    pinned: Option<bool>,
    pin_error: Option<String>,
    active: Duration,
    core: Option<usize>,
}

struct Worker {
    index: usize,
    sh: Arc<Shared>,
    backend: Box<dyn TxBackend>,
    source: Box<dyn FrameSource>,
    count: Option<u64>,
    duration: Option<Duration>,
    rate: Rate,
    pacer_cfg: PacerConfig,
    batch: usize,
    core: Option<usize>,
}

impl Worker {
    fn counters(&self) -> &Counters {
        &self.sh.workers[self.index]
    }

    fn finish(
        mut self,
        state: JobState,
        pacer: Option<&Pacer>,
        pin: (Option<bool>, Option<String>),
    ) -> WorkerOut {
        // Sending is over: freeze the active time and accuracy before closing, so
        // backend teardown (e.g. unmapping a TX ring) does not count as sending.
        let active = pacer.map_or(Duration::ZERO, Pacer::active);
        let accuracy = pacer.map(Pacer::accuracy);
        self.counters()
            .active_ns
            .store(duration_ns(active), Ordering::Relaxed);
        let detail = self.backend.detail();
        let close_err = self.backend.close().err();
        let state = match (state, close_err) {
            (s @ JobState::Failed(_), _) | (s, None) => s,
            (_, Some(e)) => {
                let m = format!("{}: close: {e}", self.backend.name());
                lock(&self.sh.failed).get_or_insert(m.clone());
                JobState::Failed(m)
            }
        };
        let out = WorkerOut {
            state,
            accuracy,
            backend: self.backend.stats(),
            name: self.backend.name(),
            detail,
            pinned: pin.0,
            pin_error: pin.1,
            active,
            core: self.core,
        };
        self.sh.live_workers.fetch_sub(1, Ordering::SeqCst);
        if let Some(m) = self.sh.monitor.get() {
            m.unpark();
        }
        out
    }

    #[allow(clippy::too_many_lines)]
    fn run(mut self) -> WorkerOut {
        let pin = match self.core {
            None => (None, None),
            Some(core) => match crate::pin::pin_current_thread(core) {
                Ok(p) => (Some(p), None),
                Err(e) => (Some(false), Some(format!("cannot pin to core {core}: {e}"))),
            },
        };
        // Scheduled start: wait for the monitor's go (or a stop).
        while !self.sh.go.load(Ordering::SeqCst) {
            if self.sh.stop.load(Ordering::SeqCst) {
                return self.finish(JobState::Finished(FinishReason::Stopped), None, pin);
            }
            std::thread::park_timeout(Duration::from_millis(50));
        }
        // Validated in `start_workers`.
        let mut pacer = Pacer::new(self.rate, self.pacer_cfg).expect("validated rate");
        let batch = self.batch;
        let mut frames: Vec<Frame> = (0..batch).map(|_| Frame::with_capacity(2048)).collect();
        let streams = self.sh.streams;
        let mut stream_of = vec![0usize; batch];
        let mut stream_acc = vec![(0u64, 0u64); streams];
        let max_block = Duration::from_millis(20);
        let mut attempted: u64 = 0;
        let mut paused = false;

        let end_state = loop {
            if self.sh.stop.load(Ordering::Relaxed) {
                break JobState::Finished(FinishReason::Stopped);
            }
            if self.sh.pause.load(Ordering::Relaxed) {
                if !paused {
                    paused = true;
                    pacer.pause();
                    self.sh.paused_workers.fetch_add(1, Ordering::SeqCst);
                    if let Some(m) = self.sh.monitor.get() {
                        m.unpark();
                    }
                }
                std::thread::park_timeout(Duration::from_millis(100));
                continue;
            }
            if paused {
                paused = false;
                pacer.resume();
                self.sh.paused_workers.fetch_sub(1, Ordering::SeqCst);
            }
            let active = pacer.active();
            self.counters()
                .active_ns
                .store(duration_ns(active), Ordering::Relaxed);
            if self.duration.is_some_and(|d| active >= d) {
                break JobState::Finished(FinishReason::DurationElapsed);
            }
            let left = self.count.map(|c| c.saturating_sub(attempted));
            if left == Some(0) {
                break JobState::Finished(FinishReason::CountReached);
            }

            let block = match self.duration {
                Some(d) => max_block.min(d.saturating_sub(active)),
                None => max_block,
            };
            let Some(grant) = pacer.poll(block) else {
                continue;
            };
            let mut n = batch.min(usize::try_from(grant).unwrap_or(usize::MAX));
            if let Some(l) = left {
                n = n.min(usize::try_from(l).unwrap_or(usize::MAX));
            }
            let mut filled = 0;
            let mut exhausted = false;
            while filled < n {
                match self.source.next_frame(frames[filled].buf_mut()) {
                    Some(len) => {
                        pacer.charge(len);
                        stream_of[filled] = self.source.stream_index().min(streams - 1);
                        filled += 1;
                    }
                    None => {
                        exhausted = true;
                        break;
                    }
                }
            }
            if filled > 0 {
                attempted += filled as u64;
                let c = &self.sh.workers[self.index];
                c.attempted.fetch_add(filled as u64, Ordering::Relaxed);
                match self.backend.send_batch(&frames[..filled]) {
                    Ok(accepted) => {
                        let accepted = accepted.min(filled);
                        let mut bytes = 0u64;
                        let mut l1 = 0u64;
                        for (f, &s) in frames[..accepted].iter().zip(&stream_of) {
                            let l = f.len() as u64;
                            bytes += l;
                            l1 += (f.len().max(crate::rate::MIN_FRAME_LEN)
                                + crate::rate::WIRE_OVERHEAD)
                                as u64;
                            let acc = &mut stream_acc[s];
                            acc.0 += 1;
                            acc.1 += l;
                        }
                        for (acc, (p, b)) in stream_acc.iter_mut().zip(c.per_stream.iter()) {
                            if acc.0 > 0 {
                                p.fetch_add(acc.0, Ordering::Relaxed);
                                b.fetch_add(acc.1, Ordering::Relaxed);
                                *acc = (0, 0);
                            }
                        }
                        c.packets.fetch_add(accepted as u64, Ordering::Relaxed);
                        c.bytes.fetch_add(bytes, Ordering::Relaxed);
                        c.l1_bytes.fetch_add(l1, Ordering::Relaxed);
                        if accepted < filled {
                            c.dropped
                                .fetch_add((filled - accepted) as u64, Ordering::Relaxed);
                        }
                    }
                    Err(e) => {
                        let m = if self.sh.workers.len() > 1 {
                            format!("{} (worker {}): {e}", self.backend.name(), self.index)
                        } else {
                            format!("{}: {e}", self.backend.name())
                        };
                        self.sh.fail(m.clone());
                        break JobState::Failed(m);
                    }
                }
            }
            if exhausted {
                break match self.source.take_error() {
                    Some(e) => {
                        let m = format!("frame source: {e}");
                        self.sh.fail(m.clone());
                        JobState::Failed(m)
                    }
                    None => JobState::Finished(FinishReason::SourceExhausted),
                };
            }
        };
        if paused {
            self.sh.paused_workers.fetch_sub(1, Ordering::SeqCst);
        }
        self.finish(end_state, Some(&pacer), pin)
    }
}

fn duration_ns(d: Duration) -> u64 {
    u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)
}

struct Monitor {
    sh: Arc<Shared>,
    cfg: JobConfig,
    tx: SyncSender<StatsSnapshot>,
}

struct Tick {
    last_t: Instant,
    last: Totals,
}

impl Monitor {
    fn snapshot(&self, tick: &mut Tick, state: JobState) -> StatsSnapshot {
        let now = Instant::now();
        let dt = now.duration_since(tick.last_t).as_secs_f64().max(1e-9);
        let t = self.sh.totals();
        let per_stream = t
            .per_stream
            .iter()
            .zip(&tick.last.per_stream)
            .map(|(&(p, b), &(lp, _))| StreamStats {
                packets: p,
                bytes: b,
                pps: p.saturating_sub(lp) as f64 / dt,
            })
            .collect();
        let snap = StatsSnapshot {
            t: now.duration_since(self.sh.started),
            packets: t.packets,
            bytes: t.bytes,
            dropped: t.dropped,
            pps: t.packets.saturating_sub(tick.last.packets) as f64 / dt,
            bps: t.bytes.saturating_sub(tick.last.bytes) as f64 * 8.0 / dt,
            l1_bps: t.l1_bytes.saturating_sub(tick.last.l1_bytes) as f64 * 8.0 / dt,
            rate_achieved_ratio: self.sh.ratio(&t),
            per_stream,
            state,
        };
        tick.last_t = now;
        tick.last = t;
        snap
    }

    fn publish(&self, snap: StatsSnapshot) {
        *lock(&self.sh.latest) = snap.clone();
        match self.tx.try_send(snap) {
            Ok(()) | Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {}
        }
    }

    fn run(self, handles: Vec<JoinHandle<WorkerOut>>) -> JobReport {
        let interval = self.cfg.snapshot_interval.max(Duration::from_millis(1));
        let mut tick = Tick {
            last_t: Instant::now(),
            last: Totals {
                per_stream: vec![(0, 0); self.sh.streams],
                ..Totals::default()
            },
        };
        let mut next_snap = Instant::now() + interval;

        // Scheduled start.
        if let Some(at) = self.cfg.start_at {
            loop {
                if self.sh.stop.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(rem) = at.duration_since(SystemTime::now()) else {
                    break;
                };
                if rem.is_zero() {
                    break;
                }
                let now = Instant::now();
                if now >= next_snap {
                    let s = self.snapshot(&mut tick, JobState::Scheduled);
                    self.publish(s);
                    next_snap = now + interval;
                }
                std::thread::park_timeout(rem.min(next_snap.saturating_duration_since(now)));
            }
        }
        tick.last_t = Instant::now();
        self.sh.go.store(true, Ordering::SeqCst);
        self.sh.wake_all();

        while self.sh.live_workers.load(Ordering::SeqCst) > 0 {
            let now = Instant::now();
            if now >= next_snap {
                let s = self.snapshot(&mut tick, self.sh.state());
                self.publish(s);
                next_snap = now + interval;
            }
            std::thread::park_timeout(next_snap.saturating_duration_since(Instant::now()));
        }

        let outs: Vec<WorkerOut> = handles
            .into_iter()
            .map(|h| match h.join() {
                Ok(o) => o,
                Err(p) => std::panic::resume_unwind(p),
            })
            .collect();
        self.report(&mut tick, &outs)
    }

    fn report(&self, tick: &mut Tick, outs: &[WorkerOut]) -> JobReport {
        let failed = lock(&self.sh.failed).clone();
        let state = if let Some(m) = failed {
            JobState::Failed(m)
        } else if let Some(s) = outs.iter().find_map(|o| match &o.state {
            s @ JobState::Failed(_) => Some(s.clone()),
            _ => None,
        }) {
            s
        } else {
            let has = |r: FinishReason| outs.iter().any(|o| o.state == JobState::Finished(r));
            let reason = [
                FinishReason::Stopped,
                FinishReason::DurationElapsed,
                FinishReason::CountReached,
                FinishReason::SourceExhausted,
            ]
            .into_iter()
            .find(|r| has(*r))
            .unwrap_or(FinishReason::Stopped);
            JobState::Finished(reason)
        };
        *lock(&self.sh.done) = Some(state.clone());

        let active = outs.iter().map(|o| o.active).max().unwrap_or_default();
        let secs = active.as_secs_f64().max(1e-9);
        let mut final_snap = self.snapshot(tick, state.clone());
        let t = self.sh.totals();
        final_snap.pps = final_snap.packets as f64 / secs;
        final_snap.bps = final_snap.bytes as f64 * 8.0 / secs;
        final_snap.l1_bps = t.l1_bytes as f64 * 8.0 / secs;
        for s in &mut final_snap.per_stream {
            s.pps = s.packets as f64 / secs;
        }
        self.publish(final_snap.clone());

        let accuracy = merge_accuracy(outs, self.sh.rate, active);
        let mut backend = TxStats::default();
        for o in outs {
            backend.packets += o.backend.packets;
            backend.bytes += o.backend.bytes;
            backend.dropped += o.backend.dropped;
            backend.batches += o.backend.batches;
        }
        let pinned = outs.iter().filter_map(|o| o.pinned).reduce(|a, b| a && b);
        let errors: Vec<String> = outs.iter().filter_map(|o| o.pin_error.clone()).collect();
        let workers = outs
            .iter()
            .enumerate()
            .map(|(i, o)| {
                let c = &self.sh.workers[i];
                WorkerReport {
                    index: i,
                    state: o.state.clone(),
                    packets: c.packets.load(Ordering::Relaxed),
                    bytes: c.bytes.load(Ordering::Relaxed),
                    dropped: c.dropped.load(Ordering::Relaxed),
                    active: o.active,
                    core: o.core,
                    pinned: o.pinned,
                }
            })
            .collect();
        JobReport {
            state,
            stats: final_snap,
            accuracy,
            backend,
            backend_name: outs.first().map_or("", |o| o.name).to_owned(),
            backend_detail: outs.first().and_then(|o| o.detail.clone()),
            active,
            pinned,
            pin_error: (!errors.is_empty()).then(|| errors.join("; ")),
            workers,
        }
    }
}

/// Sums the workers' pacing accuracy (targets add up to the job's rate).
fn merge_accuracy(outs: &[WorkerOut], rate: Rate, active: Duration) -> RateAccuracy {
    let accs: Vec<&RateAccuracy> = outs.iter().filter_map(|o| o.accuracy.as_ref()).collect();
    let unit = accs.first().map_or(RateUnit::Pps, |a| a.unit);
    let target = accs
        .iter()
        .map(|a| a.target)
        .try_fold(0.0, |s, t| t.map(|t| s + t))
        .filter(|_| !rate.is_unlimited() && !accs.is_empty());
    let achieved: f64 = accs.iter().map(|a| a.achieved).sum();
    let ratio = target.and_then(|t| {
        (t > 0.0).then(|| {
            accs.iter()
                .map(|a| a.ratio.unwrap_or(0.0) * a.target.unwrap_or(0.0))
                .sum::<f64>()
                / t
        })
    });
    let waits = accs.len().max(1) as f64;
    RateAccuracy {
        target,
        achieved,
        unit,
        ratio,
        max_jitter: accs.iter().map(|a| a.max_jitter).max().unwrap_or_default(),
        mean_jitter: Duration::from_secs_f64(
            accs.iter()
                .map(|a| a.mean_jitter.as_secs_f64())
                .sum::<f64>()
                / waits,
        ),
        frames: accs.iter().map(|a| a.frames).sum(),
        active,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{MixMode, Mixer, PreRendered};
    use crate::{NullBackend, TxOptions};

    fn null() -> Box<dyn TxBackend> {
        let mut b = NullBackend::new();
        b.open("null0", &TxOptions::default()).unwrap();
        Box::new(b)
    }

    fn endless() -> Box<dyn FrameSource> {
        Box::new(PreRendered::new(vec![vec![0u8; 60]], None))
    }

    #[test]
    fn count_limited_unlimited_rate() {
        let h = TxJob::start(
            null(),
            endless(),
            JobConfig {
                count: Some(10_000),
                ..JobConfig::default()
            },
        )
        .unwrap();
        let r = h.wait();
        assert_eq!(r.state, JobState::Finished(FinishReason::CountReached));
        assert_eq!(r.stats.packets, 10_000);
        assert_eq!(r.stats.bytes, 600_000);
        assert_eq!(r.backend.packets, 10_000);
        assert_eq!(r.backend_name, "null");
        assert_eq!(r.stats.per_stream.len(), 1);
        assert_eq!(r.stats.per_stream[0].packets, 10_000);
        assert!(r.accuracy.ratio.is_none());
    }

    #[test]
    fn source_exhaustion_and_per_stream_counters() {
        let src = Mixer::new(MixMode::RoundRobin)
            .with(Box::new(PreRendered::new(vec![vec![1; 60]], Some(30))), 1)
            .with(Box::new(PreRendered::new(vec![vec![2; 100]], Some(10))), 1);
        let r = TxJob::start(null(), Box::new(src), JobConfig::default())
            .unwrap()
            .wait();
        assert_eq!(r.state, JobState::Finished(FinishReason::SourceExhausted));
        assert_eq!(r.stats.packets, 40);
        assert_eq!(r.stats.per_stream[0].packets, 30);
        assert_eq!(r.stats.per_stream[1].packets, 10);
        assert_eq!(r.stats.per_stream[1].bytes, 1000);
    }

    #[test]
    fn lifecycle_pause_resume_stop() {
        let h = TxJob::start(
            null(),
            endless(),
            JobConfig {
                rate: Rate::Pps(2000.0),
                snapshot_interval: Duration::from_millis(20),
                ..JobConfig::default()
            },
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(h.state(), JobState::Running);
        assert!(h.stats().packets > 0);

        h.pause();
        std::thread::sleep(Duration::from_millis(60));
        assert_eq!(h.state(), JobState::Paused);
        let frozen = h.stats().packets;
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(h.stats().packets, frozen, "no frames while paused");

        h.resume();
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(h.state(), JobState::Running);
        assert!(h.stats().packets > frozen);

        // Snapshots arrive.
        let snaps: Vec<_> = h.snapshots().try_iter().collect();
        assert!(snaps.len() >= 3, "{}", snaps.len());
        assert!(snaps.iter().any(|s| s.state == JobState::Paused));

        let t0 = Instant::now();
        let r = h.stop_and_wait();
        assert!(t0.elapsed() < Duration::from_millis(500));
        assert_eq!(r.state, JobState::Finished(FinishReason::Stopped));
        // ~2000 pps over ~200 ms active time, generously bounded.
        assert!(
            (200..=700).contains(&r.stats.packets),
            "{}",
            r.stats.packets
        );
        assert!(r.active < Duration::from_millis(400));
    }

    #[test]
    fn duration_limit_and_rate_ratio() {
        let r = TxJob::start(
            null(),
            endless(),
            JobConfig {
                rate: Rate::Pps(1000.0),
                duration: Some(Duration::from_millis(300)),
                ..JobConfig::default()
            },
        )
        .unwrap()
        .wait();
        assert_eq!(r.state, JobState::Finished(FinishReason::DurationElapsed));
        // Generous bounds: shared CI runners can deschedule the thread for longer
        // than the pacer's 10 ms backlog allowance.
        let ratio = r.stats.rate_achieved_ratio.unwrap();
        assert!((0.8..=1.15).contains(&ratio), "{ratio}");
        assert!(
            (230..=350).contains(&r.stats.packets),
            "{}",
            r.stats.packets
        );
    }

    #[test]
    fn drop_stops_the_job() {
        let h = TxJob::start(null(), endless(), JobConfig::default()).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        drop(h); // must not hang
    }

    #[test]
    fn invalid_config_is_rejected() {
        assert!(matches!(
            TxJob::start(
                null(),
                endless(),
                JobConfig {
                    rate: Rate::parse("50%").unwrap(),
                    ..JobConfig::default()
                }
            ),
            Err(TxError::Config(_))
        ));
        assert!(matches!(
            TxJob::start(
                null(),
                endless(),
                JobConfig {
                    batch_size: 0,
                    ..JobConfig::default()
                }
            ),
            Err(TxError::Config(_))
        ));
    }

    /// Records every frame it is given (shared between clones).
    #[derive(Clone, Default)]
    struct Recorder(Arc<Mutex<Vec<Vec<u8>>>>);

    impl TxBackend for Recorder {
        fn name(&self) -> &'static str {
            "recorder"
        }
        fn open(&mut self, _: &str, _: &TxOptions) -> Result<(), TxError> {
            Ok(())
        }
        fn send_batch(&mut self, frames: &[Frame]) -> Result<usize, TxError> {
            let mut v = lock(&self.0);
            v.extend(frames.iter().map(|f| f.as_bytes().to_vec()));
            Ok(frames.len())
        }
        fn stats(&self) -> TxStats {
            TxStats::default()
        }
        fn close(&mut self) -> Result<(), TxError> {
            Ok(())
        }
    }

    fn numbered(n: u32) -> Vec<Vec<u8>> {
        (0..n)
            .map(|i| {
                let mut f = vec![0u8; 60];
                f[..4].copy_from_slice(&i.to_be_bytes());
                f
            })
            .collect()
    }

    #[test]
    fn workers_send_every_frame_exactly_once() {
        let rec = Recorder::default();
        let workers: Vec<WorkerSetup> = PreRendered::new(numbered(10_000), Some(1))
            .split(4)
            .into_iter()
            .map(|s| {
                (
                    Box::new(rec.clone()) as Box<dyn TxBackend>,
                    Box::new(s) as Box<dyn FrameSource>,
                )
            })
            .collect();
        assert_eq!(workers.len(), 4);
        let r = TxJob::start_workers(workers, JobConfig::default())
            .unwrap()
            .wait();
        assert_eq!(r.state, JobState::Finished(FinishReason::SourceExhausted));
        assert_eq!(r.stats.packets, 10_000);
        assert_eq!(r.stats.per_stream[0].packets, 10_000);
        assert_eq!(r.workers.len(), 4);
        assert!(
            r.workers.iter().all(|w| w.packets == 2500),
            "{:?}",
            r.workers
        );
        let mut got = lock(&rec.0).clone();
        got.sort();
        assert_eq!(got, numbered(10_000));
    }

    #[test]
    fn workers_share_count_and_stop_cleanly() {
        let workers =
            |n: usize| -> Vec<WorkerSetup> { (0..n).map(|_| (null(), endless())).collect() };
        let r = TxJob::start_workers(
            workers(3),
            JobConfig {
                count: Some(10_001),
                ..JobConfig::default()
            },
        )
        .unwrap()
        .wait();
        assert_eq!(r.state, JobState::Finished(FinishReason::CountReached));
        assert_eq!(r.stats.packets, 10_001);
        assert_eq!(r.backend.packets, 10_001);
        let per: Vec<u64> = r.workers.iter().map(|w| w.packets).collect();
        assert_eq!(per, vec![3334, 3334, 3333]);

        // Endless, rate-limited: pause / resume / stop coordinate every worker.
        let h = TxJob::start_workers(
            workers(4),
            JobConfig {
                rate: Rate::Pps(4000.0),
                snapshot_interval: Duration::from_millis(20),
                ..JobConfig::default()
            },
        )
        .unwrap();
        assert_eq!(h.workers(), 4);
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(h.state(), JobState::Running);
        h.pause();
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(h.state(), JobState::Paused);
        let frozen = h.stats().packets;
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(h.stats().packets, frozen);
        h.resume();
        std::thread::sleep(Duration::from_millis(100));
        assert!(h.stats().packets > frozen);
        let t0 = Instant::now();
        let r = h.stop_and_wait();
        assert!(t0.elapsed() < Duration::from_millis(1000));
        assert_eq!(r.state, JobState::Finished(FinishReason::Stopped));
        assert!(r.workers.iter().all(|w| w.packets > 0), "{:?}", r.workers);
        // 4000 pps total over ~200 ms active.
        assert!(
            (300..=2000).contains(&r.stats.packets),
            "{}",
            r.stats.packets
        );
        let ratio = r.accuracy.ratio.unwrap();
        assert!((0.5..=1.5).contains(&ratio), "{ratio}");
    }

    #[test]
    fn failing_worker_stops_the_others() {
        let workers: Vec<WorkerSetup> = vec![
            (null(), endless()),
            (Box::new(NullBackend::new()), endless()), // not open: fails
            (null(), endless()),
        ];
        let t0 = Instant::now();
        let r = TxJob::start_workers(workers, JobConfig::default())
            .unwrap()
            .wait();
        assert!(t0.elapsed() < Duration::from_secs(2));
        assert!(
            matches!(r.state, JobState::Failed(ref m) if m.contains("worker 1")),
            "{:?}",
            r.state
        );
        assert!(matches!(
            TxJob::start(
                null(),
                endless(),
                JobConfig {
                    workers: 2,
                    ..JobConfig::default()
                }
            ),
            Err(TxError::Config(_))
        ));
    }

    #[test]
    fn ramp_up_starts_slow() {
        let h = TxJob::start(
            null(),
            endless(),
            JobConfig {
                rate: Rate::Pps(20_000.0),
                ramp_up: Some(Duration::from_millis(600)),
                duration: Some(Duration::from_millis(900)),
                ..JobConfig::default()
            },
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(150));
        // Full rate would be ~3000 by now; the ramp allows 20000 × 0.15² / 1.2 ≈ 375.
        let early = h.stats().packets;
        let r = h.wait();
        assert_eq!(r.state, JobState::Finished(FinishReason::DurationElapsed));
        assert!(early < 1500, "{early}");
        // 20000 × (0.9 − 0.3) = 12000 expected overall.
        assert!(
            (8_000..=14_000).contains(&r.stats.packets),
            "{}",
            r.stats.packets
        );
        let ratio = r.accuracy.ratio.unwrap();
        assert!((0.7..=1.2).contains(&ratio), "{ratio}");
        assert!(matches!(
            TxJob::start(
                null(),
                endless(),
                JobConfig {
                    ramp_up: Some(Duration::from_secs(1)),
                    ..JobConfig::default()
                }
            ),
            Err(TxError::Config(_))
        ));
    }

    #[test]
    fn scheduled_start_waits() {
        let t0 = Instant::now();
        let h = TxJob::start(
            null(),
            endless(),
            JobConfig {
                count: Some(100),
                start_at: Some(SystemTime::now() + Duration::from_millis(300)),
                snapshot_interval: Duration::from_millis(20),
                ..JobConfig::default()
            },
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(h.state(), JobState::Scheduled);
        assert_eq!(h.stats().packets, 0);
        let r = h.wait();
        assert!(
            t0.elapsed() >= Duration::from_millis(280),
            "{:?}",
            t0.elapsed()
        );
        assert_eq!(r.state, JobState::Finished(FinishReason::CountReached));
        assert_eq!(r.stats.packets, 100);

        // Stopping a scheduled job ends it without sending.
        let h = TxJob::start(
            null(),
            endless(),
            JobConfig {
                start_at: Some(SystemTime::now() + Duration::from_secs(3600)),
                ..JobConfig::default()
            },
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(50));
        let t0 = Instant::now();
        let r = h.stop_and_wait();
        assert!(t0.elapsed() < Duration::from_millis(500));
        assert_eq!(r.state, JobState::Finished(FinishReason::Stopped));
        assert_eq!(r.stats.packets, 0);
        // A start time in the past starts immediately.
        let r = TxJob::start(
            null(),
            endless(),
            JobConfig {
                count: Some(10),
                start_at: Some(SystemTime::now() - Duration::from_secs(10)),
                ..JobConfig::default()
            },
        )
        .unwrap()
        .wait();
        assert_eq!(r.stats.packets, 10);
    }

    #[test]
    fn backend_error_fails_the_job() {
        // A closed backend returns NotOpen.
        let r = TxJob::start(
            Box::new(NullBackend::new()),
            endless(),
            JobConfig::default(),
        )
        .unwrap()
        .wait();
        assert!(matches!(r.state, JobState::Failed(ref m) if m.contains("not open")));
    }
}
