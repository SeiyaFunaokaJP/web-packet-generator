//! TX jobs: one [`FrameSource`] through one [`TxBackend`] on a dedicated thread.
//!
//! [`TxJob::start`] spawns the thread and returns a [`JobHandle`] that can
//! [`pause`](JobHandle::pause) / [`resume`](JobHandle::resume) /
//! [`stop`](JobHandle::stop) the job, read live counters ([`JobHandle::stats`]),
//! receive a [`StatsSnapshot`] every [`JobConfig::snapshot_interval`] (100 ms by
//! default; [`JobHandle::snapshots`]) and [`wait`](JobHandle::wait) for the final
//! [`JobReport`].
//!
//! The thread optionally pins itself to a core ([`JobConfig::pin_core`]; Linux
//! `sched_setaffinity`, a no-op reported as `pinned: false` elsewhere). It never busy
//! loops while paused or stopped: pauses park the thread, and the pacer only spins for
//! the last few microseconds before a deadline. On every exit path the backend is
//! closed before the thread ends, and dropping the handle stops and joins the job.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::rate::{Pacer, PacerConfig, Rate, RateAccuracy};
use crate::source::FrameSource;
use crate::{Frame, TxBackend, TxError, TxStats};

/// Job parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct JobConfig {
    /// Stop after this many frames (`None` = until the source ends / duration).
    pub count: Option<u64>,
    /// Stop after this much active (not paused) time.
    pub duration: Option<Duration>,
    /// Target rate.
    pub rate: Rate,
    /// Frames per token-bucket grant (see [`PacerConfig::burst`]).
    pub burst: u32,
    /// Core to pin the TX thread to.
    pub pin_core: Option<usize>,
    /// Frames per `send_batch` call (upper bound; pacing may send fewer).
    pub batch_size: usize,
    /// Pacer wait strategy etc. (`burst` is taken from [`JobConfig::burst`]).
    pub pacer: PacerConfig,
    /// Interval between [`StatsSnapshot`]s.
    pub snapshot_interval: Duration,
}

impl Default for JobConfig {
    fn default() -> Self {
        Self {
            count: None,
            duration: None,
            rate: Rate::Unlimited,
            burst: 1,
            pin_core: None,
            batch_size: 64,
            pacer: PacerConfig::default(),
            snapshot_interval: Duration::from_millis(100),
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

/// Periodic statistics.
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
    /// Achieved / target rate since the start (`None` when unlimited).
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

/// Final result of a job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobReport {
    /// Terminal state.
    pub state: JobState,
    /// Counters at the end; `pps` / `bps` are averages over the active time.
    pub stats: StatsSnapshot,
    /// Pacing accuracy.
    pub accuracy: RateAccuracy,
    /// Backend counters at the end.
    pub backend: TxStats,
    /// Backend name.
    pub backend_name: String,
    /// [`TxBackend::detail`] (captured at start).
    pub backend_detail: Option<String>,
    /// Active (not paused) time.
    pub active: Duration,
    /// Whether the thread was pinned (`None` if no pinning was requested).
    pub pinned: Option<bool>,
    /// Pinning error, if any (the job still runs unpinned).
    pub pin_error: Option<String>,
}

struct Shared {
    stop: AtomicBool,
    pause: AtomicBool,
    packets: AtomicU64,
    bytes: AtomicU64,
    dropped: AtomicU64,
    per_stream: Vec<(AtomicU64, AtomicU64)>,
    state: Mutex<JobState>,
    latest: Mutex<StatsSnapshot>,
    started: Instant,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shared {
    fn set_state(&self, s: JobState) {
        *lock(&self.state) = s;
    }
}

/// Starts TX jobs.
#[derive(Debug)]
pub struct TxJob;

impl TxJob {
    /// Starts sending `source` through the (already open) `backend` on a new
    /// thread.
    ///
    /// # Errors
    /// [`TxError::Config`] for an invalid rate (e.g. an unresolved link
    /// percentage) or a zero batch size; [`TxError::Io`] if the thread cannot be
    /// spawned.
    pub fn start(
        backend: Box<dyn TxBackend>,
        source: Box<dyn FrameSource>,
        config: JobConfig,
    ) -> Result<JobHandle, TxError> {
        if config.batch_size == 0 {
            return Err(TxError::Config("batch size must be at least 1".to_owned()));
        }
        let pacer_cfg = PacerConfig {
            burst: config.burst.max(1),
            ..config.pacer
        };
        // Validate now so the caller gets the error synchronously.
        Pacer::new(config.rate, pacer_cfg)?;
        let streams = source.stream_count().max(1);
        let shared = Arc::new(Shared {
            stop: AtomicBool::new(false),
            pause: AtomicBool::new(false),
            packets: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            per_stream: (0..streams)
                .map(|_| (AtomicU64::new(0), AtomicU64::new(0)))
                .collect(),
            state: Mutex::new(JobState::Running),
            latest: Mutex::new(StatsSnapshot::empty(streams)),
            started: Instant::now(),
        });
        let (tx, rx) = mpsc::sync_channel(64);
        let sh = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("wpg-tx".to_owned())
            .spawn(move || {
                Worker {
                    sh,
                    backend,
                    source,
                    cfg: config,
                    pacer_cfg,
                    tx,
                }
                .run()
            })?;
        Ok(JobHandle {
            shared,
            thread: Some(thread),
            rx,
        })
    }
}

/// Controls a running job. Dropping it stops the job and joins the thread.
pub struct JobHandle {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<JobReport>>,
    rx: Receiver<StatsSnapshot>,
}

impl std::fmt::Debug for JobHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobHandle")
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}

impl JobHandle {
    /// Pauses sending (the thread parks; pause time does not count towards
    /// [`JobConfig::duration`] or the rate accuracy).
    pub fn pause(&self) {
        self.shared.pause.store(true, Ordering::SeqCst);
    }

    /// Resumes after [`Self::pause`].
    pub fn resume(&self) {
        self.shared.pause.store(false, Ordering::SeqCst);
        if let Some(t) = &self.thread {
            t.thread().unpark();
        }
    }

    /// Requests the job to stop; returns immediately (see [`Self::wait`]).
    pub fn stop(&self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        if let Some(t) = &self.thread {
            t.thread().unpark();
        }
    }

    /// Current state.
    #[must_use]
    pub fn state(&self) -> JobState {
        lock(&self.shared.state).clone()
    }

    /// Whether the job has ended (the thread may still be closing the backend;
    /// [`Self::wait`] joins it).
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
    }

    /// Live statistics: cumulative counters read now, rates from the latest
    /// snapshot.
    #[must_use]
    pub fn stats(&self) -> StatsSnapshot {
        let mut s = lock(&self.shared.latest).clone();
        s.t = self.shared.started.elapsed();
        s.packets = self.shared.packets.load(Ordering::Relaxed);
        s.bytes = self.shared.bytes.load(Ordering::Relaxed);
        s.dropped = self.shared.dropped.load(Ordering::Relaxed);
        for (st, (p, b)) in s.per_stream.iter_mut().zip(&self.shared.per_stream) {
            st.packets = p.load(Ordering::Relaxed);
            st.bytes = b.load(Ordering::Relaxed);
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
    /// Re-raises a panic of the TX thread.
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
            self.shared.stop.store(true, Ordering::SeqCst);
            t.thread().unpark();
            let _ = t.join();
        }
    }
}

struct Worker {
    sh: Arc<Shared>,
    backend: Box<dyn TxBackend>,
    source: Box<dyn FrameSource>,
    cfg: JobConfig,
    pacer_cfg: PacerConfig,
    tx: SyncSender<StatsSnapshot>,
}

struct Tick {
    last_t: Instant,
    last_packets: u64,
    last_bytes: u64,
    last_l1_bits: f64,
    last_stream: Vec<u64>,
    l1_bits: f64,
}

impl Worker {
    fn snapshot(&self, tick: &mut Tick, pacer: &Pacer, state: JobState) -> StatsSnapshot {
        let now = Instant::now();
        let dt = now.duration_since(tick.last_t).as_secs_f64().max(1e-9);
        let packets = self.sh.packets.load(Ordering::Relaxed);
        let bytes = self.sh.bytes.load(Ordering::Relaxed);
        let per_stream = self
            .sh
            .per_stream
            .iter()
            .zip(tick.last_stream.iter_mut())
            .map(|((p, b), last)| {
                let p = p.load(Ordering::Relaxed);
                let s = StreamStats {
                    packets: p,
                    bytes: b.load(Ordering::Relaxed),
                    pps: (p - *last) as f64 / dt,
                };
                *last = p;
                s
            })
            .collect();
        let snap = StatsSnapshot {
            t: now.duration_since(self.sh.started),
            packets,
            bytes,
            dropped: self.sh.dropped.load(Ordering::Relaxed),
            pps: (packets - tick.last_packets) as f64 / dt,
            bps: (bytes - tick.last_bytes) as f64 * 8.0 / dt,
            l1_bps: (tick.l1_bits - tick.last_l1_bits) / dt,
            rate_achieved_ratio: pacer.accuracy().ratio,
            per_stream,
            state,
        };
        tick.last_t = now;
        tick.last_packets = packets;
        tick.last_bytes = bytes;
        tick.last_l1_bits = tick.l1_bits;
        snap
    }

    fn publish(&self, snap: StatsSnapshot) {
        *lock(&self.sh.latest) = snap.clone();
        match self.tx.try_send(snap) {
            Ok(()) | Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {}
        }
    }

    #[allow(clippy::too_many_lines)]
    fn run(mut self) -> JobReport {
        let (pinned, pin_error) = match self.cfg.pin_core {
            None => (None, None),
            Some(core) => match crate::pin::pin_current_thread(core) {
                Ok(p) => (Some(p), None),
                Err(e) => (Some(false), Some(format!("cannot pin to core {core}: {e}"))),
            },
        };
        let backend_detail = self.backend.detail();
        // Validated in `start`.
        let mut pacer = Pacer::new(self.cfg.rate, self.pacer_cfg).expect("validated rate");
        let batch = self.cfg.batch_size;
        let mut frames: Vec<Frame> = (0..batch).map(|_| Frame::with_capacity(2048)).collect();
        let mut stream_of = vec![0usize; batch];
        let streams = self.sh.per_stream.len();
        let mut tick = Tick {
            last_t: Instant::now(),
            last_packets: 0,
            last_bytes: 0,
            last_l1_bits: 0.0,
            last_stream: vec![0; streams],
            l1_bits: 0.0,
        };
        let interval = self.cfg.snapshot_interval.max(Duration::from_millis(1));
        let max_block = interval.min(Duration::from_millis(20));
        let mut next_snap = Instant::now() + interval;
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
                    self.sh.set_state(JobState::Paused);
                    let s = self.snapshot(&mut tick, &pacer, JobState::Paused);
                    self.publish(s);
                    next_snap = Instant::now() + interval;
                }
                std::thread::park_timeout(interval);
                if Instant::now() >= next_snap {
                    let s = self.snapshot(&mut tick, &pacer, JobState::Paused);
                    self.publish(s);
                    next_snap = Instant::now() + interval;
                }
                continue;
            }
            if paused {
                paused = false;
                pacer.resume();
                self.sh.set_state(JobState::Running);
            }
            if self.cfg.duration.is_some_and(|d| pacer.active() >= d) {
                break JobState::Finished(FinishReason::DurationElapsed);
            }
            let left = self.cfg.count.map(|c| c.saturating_sub(attempted));
            if left == Some(0) {
                break JobState::Finished(FinishReason::CountReached);
            }

            let now = Instant::now();
            if now >= next_snap {
                let s = self.snapshot(&mut tick, &pacer, JobState::Running);
                self.publish(s);
                next_snap = now + interval;
            }

            let block = match self.cfg.duration {
                Some(d) => max_block.min(d.saturating_sub(pacer.active())),
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
                match self.backend.send_batch(&frames[..filled]) {
                    Ok(accepted) => {
                        let accepted = accepted.min(filled);
                        let mut bytes = 0u64;
                        for (f, &s) in frames[..accepted].iter().zip(&stream_of) {
                            let l = f.len() as u64;
                            bytes += l;
                            tick.l1_bits += crate::rate::wire_bits(f.len());
                            let (p, b) = &self.sh.per_stream[s];
                            p.fetch_add(1, Ordering::Relaxed);
                            b.fetch_add(l, Ordering::Relaxed);
                        }
                        self.sh
                            .packets
                            .fetch_add(accepted as u64, Ordering::Relaxed);
                        self.sh.bytes.fetch_add(bytes, Ordering::Relaxed);
                        if accepted < filled {
                            self.sh
                                .dropped
                                .fetch_add((filled - accepted) as u64, Ordering::Relaxed);
                        }
                    }
                    Err(e) => break JobState::Failed(format!("{}: {e}", self.backend.name())),
                }
            }
            if exhausted {
                break match self.source.take_error() {
                    Some(e) => JobState::Failed(format!("frame source: {e}")),
                    None => JobState::Finished(FinishReason::SourceExhausted),
                };
            }
        };

        let close_err = self.backend.close().err();
        let state = match (end_state, close_err) {
            (s @ JobState::Failed(_), _) | (s, None) => s,
            (_, Some(e)) => JobState::Failed(format!("{}: close: {e}", self.backend.name())),
        };
        self.sh.set_state(state.clone());

        let accuracy = pacer.accuracy();
        let active = pacer.active();
        let secs = active.as_secs_f64().max(1e-9);
        let mut final_snap = self.snapshot(&mut tick, &pacer, state.clone());
        final_snap.pps = final_snap.packets as f64 / secs;
        final_snap.bps = final_snap.bytes as f64 * 8.0 / secs;
        final_snap.l1_bps = tick.l1_bits / secs;
        for s in &mut final_snap.per_stream {
            s.pps = s.packets as f64 / secs;
        }
        self.publish(final_snap.clone());
        JobReport {
            state,
            stats: final_snap,
            accuracy,
            backend: self.backend.stats(),
            backend_name: self.backend.name().to_owned(),
            backend_detail,
            active,
            pinned,
            pin_error,
        }
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
        let ratio = r.stats.rate_achieved_ratio.unwrap();
        assert!((0.9..=1.1).contains(&ratio), "{ratio}");
        assert!(
            (250..=350).contains(&r.stats.packets),
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
