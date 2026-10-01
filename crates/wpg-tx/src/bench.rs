//! `wpg bench`: unlimited-rate transmission of one synthetic frame (or of any frame
//! sources, e.g. a template rendered on the fly), optionally on several workers.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::job::{JobConfig, JobHandle, JobReport, JobState, TxJob};
use crate::rate::Rate;
use crate::source::{FrameSource, PreRendered};
use crate::{TxError, TxOptions};

/// EtherType used by benchmark frames (IEEE 802 "local experimental 1").
pub const BENCH_ETHERTYPE: u16 = 0x88b5;

/// Benchmark result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchResult {
    /// Backend name.
    pub backend: String,
    /// How the backend transmitted (e.g. `TPACKET_V3 TX ring, ...`).
    pub detail: Option<String>,
    /// Interface (or output path).
    pub iface: String,
    /// Frame length in octets (without FCS).
    pub frame_size: usize,
    /// Frames per `send_batch`.
    pub batch_size: usize,
    /// TX workers (threads / backend instances).
    #[serde(default = "one")]
    pub workers: usize,
    /// Template benchmarked on the fly (`None`: synthetic frame).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    /// Frames accepted by the backend.
    pub packets: u64,
    /// Frames not accepted.
    pub dropped: u64,
    /// Frames per second.
    pub pps: f64,
    /// Layer-2 bits per second.
    pub bps: f64,
    /// Layer-1 bits per second (incl. preamble, IFG, FCS).
    pub l1_bps: f64,
    /// Measured time.
    pub elapsed: Duration,
}

fn one() -> usize {
    1
}

/// A synthetic Ethernet II frame of `frame_size` octets (min 14): locally
/// administered unicast addresses, EtherType [`BENCH_ETHERTYPE`], payload counting
/// up from 0.
#[must_use]
pub fn synthetic_frame(frame_size: usize) -> Vec<u8> {
    let len = frame_size.max(14);
    let mut f = Vec::with_capacity(len);
    f.extend_from_slice(&[0x02, 0, 0, 0, 0, 0x02]); // dst
    f.extend_from_slice(&[0x02, 0, 0, 0, 0, 0x01]); // src
    f.extend_from_slice(&BENCH_ETHERTYPE.to_be_bytes());
    f.extend((0..=255u8).cycle().take(len - 14));
    f
}

/// Benchmark options beyond the basics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchOptions {
    /// Backend options (batch size, ring depth, qdisc bypass, core).
    pub tx: TxOptions,
    /// TX workers, each with its own backend instance (default 1).
    pub workers: usize,
    /// Core per worker (see `JobConfig::pin_cores`).
    pub pin_cores: Vec<usize>,
}

impl Default for BenchOptions {
    fn default() -> Self {
        Self {
            tx: TxOptions {
                batch_size: 256,
                ..TxOptions::default()
            },
            workers: 1,
            pin_cores: Vec::new(),
        }
    }
}

/// Runs `backend_name` on `iface` at unlimited rate for `duration` with frames of
/// `frame_size` octets.
///
/// # Errors
/// Backend open errors, or the job failing.
pub fn run_bench(
    backend_name: &str,
    iface: &str,
    duration: Duration,
    frame_size: usize,
) -> Result<BenchResult, TxError> {
    run_bench_with(
        backend_name,
        iface,
        duration,
        frame_size,
        &BenchOptions::default(),
    )
}

/// [`run_bench`] with explicit options.
///
/// # Errors
/// See [`run_bench`].
pub fn run_bench_with(
    backend_name: &str,
    iface: &str,
    duration: Duration,
    frame_size: usize,
    opts: &BenchOptions,
) -> Result<BenchResult, TxError> {
    let job = start_bench(backend_name, iface, duration, frame_size, opts)?;
    bench_result(&job.wait(), iface, frame_size, opts)
}

/// Opens the backend and starts the benchmark job (for callers that want progress
/// output or Ctrl-C handling); finish with [`bench_result`].
///
/// # Errors
/// Backend open errors.
pub fn start_bench(
    backend_name: &str,
    iface: &str,
    duration: Duration,
    frame_size: usize,
    opts: &BenchOptions,
) -> Result<JobHandle, TxError> {
    let sources = PreRendered::new(vec![synthetic_frame(frame_size)], None)
        .split(opts.workers.max(1))
        .into_iter()
        .map(|s| Box::new(s) as Box<dyn FrameSource>)
        .collect();
    start_bench_sources(backend_name, iface, duration, sources, opts)
}

/// Like [`start_bench`] with caller-provided frame sources, one per worker (e.g. a
/// template rendered on the fly, split with `Rendered::split`).
///
/// # Errors
/// Backend open errors.
pub fn start_bench_sources(
    backend_name: &str,
    iface: &str,
    duration: Duration,
    sources: Vec<Box<dyn FrameSource>>,
    opts: &BenchOptions,
) -> Result<JobHandle, TxError> {
    TxJob::start_on(
        backend_name,
        iface,
        &opts.tx,
        sources,
        JobConfig {
            duration: Some(duration),
            rate: Rate::Unlimited,
            batch_size: opts.tx.batch_size.max(1),
            pin_core: opts.tx.pin_core,
            pin_cores: opts.pin_cores.clone(),
            workers: opts.workers.max(1),
            ..JobConfig::default()
        },
    )
}

/// Turns the report of a [`start_bench`] job into a [`BenchResult`].
///
/// # Errors
/// The job's failure message as [`TxError::Io`].
pub fn bench_result(
    r: &JobReport,
    iface: &str,
    frame_size: usize,
    opts: &BenchOptions,
) -> Result<BenchResult, TxError> {
    if let JobState::Failed(m) = &r.state {
        return Err(TxError::Io(std::io::Error::other(m.clone())));
    }
    Ok(BenchResult {
        backend: r.backend_name.clone(),
        detail: r.backend_detail.clone(),
        iface: iface.to_owned(),
        frame_size: frame_size.max(14),
        batch_size: opts.tx.batch_size.max(1),
        workers: r.workers.len().max(1),
        template: None,
        packets: r.stats.packets,
        dropped: r.stats.dropped,
        pps: r.stats.pps,
        bps: r.stats.bps,
        l1_bps: r.stats.l1_bps,
        elapsed: r.active,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_shape() {
        let f = synthetic_frame(64);
        assert_eq!(f.len(), 64);
        assert_eq!(&f[12..14], &[0x88, 0xb5]);
        assert_eq!(synthetic_frame(3).len(), 14);
    }

    #[test]
    fn null_bench_runs() {
        let r = run_bench("null", "null0", Duration::from_millis(100), 64).unwrap();
        assert_eq!(r.backend, "null");
        assert!(r.packets > 1000, "{r:?}");
        assert!(r.pps > 10_000.0, "{r:?}");
        assert!(r.elapsed >= Duration::from_millis(100));
    }

    #[test]
    fn null_bench_with_workers() {
        let opts = BenchOptions {
            workers: 3,
            ..BenchOptions::default()
        };
        let r = run_bench_with("null", "null0", Duration::from_millis(100), 64, &opts).unwrap();
        assert_eq!(r.workers, 3);
        assert!(r.packets > 1000, "{r:?}");
    }

    #[test]
    fn unknown_backend() {
        assert!(matches!(
            run_bench("nope", "x", Duration::from_millis(1), 64),
            Err(TxError::Unsupported(_))
        ));
    }
}
