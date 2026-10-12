//! # wpg-rx
//!
//! Receive-side counters and verification (TODO §2.3, M9):
//!
//! * [`backend`]: the [`RxBackend`] trait and its implementations — `null`,
//!   `pcapfile` (replays a capture file), `afpacket` (Linux) and `pcap` (feature
//!   `pcap`) — with a registry ([`backend::open_by_name`] / [`backend::available`]);
//! * [`filter`]: a tcpdump-style filter language evaluated in user space
//!   ([`Filter`]), identical on every backend;
//! * [`verify`]: signature checks (loss / reordering / duplicates / latency) of
//!   frames stamped by the sender ([`wpg_core::signature`]);
//! * [`capture`]: a capture session on its own thread with pps / bps counters,
//!   the first N frames, pcapng output and periodic [`capture::RxSnapshot`]s.

use thiserror::Error;

#[cfg(target_os = "linux")]
mod afpacket;
pub mod backend;
pub mod capture;
pub mod filter;
#[cfg(feature = "pcap")]
mod pcap;
mod pcapfile;
pub mod verify;

pub use backend::{RxBackend, RxOptions, RxPacket, RxStatus};
pub use capture::{Capture, CaptureConfig, CaptureHandle, CaptureReport, CaptureState, RxSnapshot};
pub use filter::{Filter, FilterError};
pub use verify::{Verifier, VerifyReport};

/// Errors from receive backends and capture sessions.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RxError {
    /// `recv` before `open` or after `close`.
    #[error("backend is not open")]
    NotOpen,
    /// `open` on an already open backend.
    #[error("backend is already open")]
    AlreadyOpen,
    /// The interface (or input file) does not exist.
    #[error("interface `{0}` is not available")]
    Interface(String),
    /// Insufficient privileges.
    #[error("permission denied: {0}")]
    Permission(String),
    /// Unknown backend, or not available in this build / on this OS.
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// Invalid configuration or input.
    #[error("invalid configuration: {0}")]
    Config(String),
    /// Underlying OS error.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Receive counters for one interface / filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RxCounters {
    /// Frames seen.
    pub packets: u64,
    /// Octets seen.
    pub bytes: u64,
    /// Frames matching the configured filter.
    pub matched: u64,
    /// Octets of the matching frames.
    pub matched_bytes: u64,
}

impl RxCounters {
    /// Creates zeroed counters.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            packets: 0,
            bytes: 0,
            matched: 0,
            matched_bytes: 0,
        }
    }

    /// Records one received frame of `len` octets.
    pub fn record(&mut self, len: usize, matched: bool) {
        self.packets += 1;
        self.bytes += len as u64;
        if matched {
            self.matched += 1;
            self.matched_bytes += len as u64;
        }
    }

    /// Difference `self - earlier`, for rate computation between two snapshots.
    #[must_use]
    pub const fn delta(&self, earlier: &Self) -> Self {
        Self {
            packets: self.packets.saturating_sub(earlier.packets),
            bytes: self.bytes.saturating_sub(earlier.bytes),
            matched: self.matched.saturating_sub(earlier.matched),
            matched_bytes: self.matched_bytes.saturating_sub(earlier.matched_bytes),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_delta() {
        let mut c = RxCounters::new();
        c.record(64, true);
        let snap = c;
        c.record(1500, false);
        assert_eq!(
            c,
            RxCounters {
                packets: 2,
                bytes: 1564,
                matched: 1,
                matched_bytes: 64,
            }
        );
        assert_eq!(
            c.delta(&snap),
            RxCounters {
                packets: 1,
                bytes: 1500,
                matched: 0,
                matched_bytes: 0,
            }
        );
    }
}
