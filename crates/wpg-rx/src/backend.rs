//! Receive backends behind the [`RxBackend`] trait and the registry
//! ([`open_by_name`] / [`available`]).
//!
//! * `null`: receives nothing (tests, dry runs of the capture pipeline);
//! * `pcapfile`: replays a pcap / pcapng file (the interface is the file path) with
//!   its original timestamps, then reports the end of the input;
//! * `afpacket` (Linux): an `AF_PACKET` raw socket bound to the interface (`any` =
//!   every interface), optionally promiscuous; frames the host itself sent are
//!   skipped unless [`RxOptions::include_outgoing`] is set;
//! * `pcap` (feature `pcap`): libpcap / Npcap, every OS.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::RxError;

/// Every backend name, in the order shown to users.
pub const NAMES: &[&str] = &["null", "pcapfile", "afpacket", "pcap"];

/// Options passed to [`RxBackend::open`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RxOptions {
    /// Put the interface in promiscuous mode.
    pub promisc: bool,
    /// Also capture frames this host transmits (`afpacket`, `pcap`: direction
    /// filter). Off by default, so a frame sent and received on `lo` counts once.
    pub include_outgoing: bool,
    /// Octets kept of each frame (longer frames are truncated; the original length
    /// is still reported).
    pub snaplen: usize,
}

impl Default for RxOptions {
    fn default() -> Self {
        Self {
            promisc: false,
            include_outgoing: false,
            snaplen: 65_535,
        }
    }
}

/// One received frame.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RxPacket {
    /// Capture time, nanoseconds since the Unix epoch.
    pub ts_ns: u64,
    /// Original length on the wire (without FCS).
    pub len: usize,
    /// Captured octets (at most [`RxOptions::snaplen`]).
    pub data: Vec<u8>,
}

/// Result of [`RxBackend::recv`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RxStatus {
    /// Zero or more frames were appended; more may follow.
    Ok,
    /// The input is exhausted (`pcapfile`); no frames will follow.
    Eof,
}

/// Cumulative backend counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RxBackendStats {
    /// Frames delivered by the backend.
    pub received: u64,
    /// Frames the kernel / driver dropped before delivery (when known).
    pub dropped: u64,
}

/// A packet receive backend.
pub trait RxBackend: Send {
    /// Short backend id (`null`, `pcapfile`, `afpacket`, `pcap`).
    fn name(&self) -> &'static str;

    /// Opens the backend on `iface`.
    ///
    /// # Errors
    /// Interface / permission / OS errors.
    fn open(&mut self, iface: &str, opts: &RxOptions) -> Result<(), RxError>;

    /// Appends up to `max` frames to `out`, waiting at most `timeout` for the
    /// first one.
    ///
    /// # Errors
    /// [`RxError::NotOpen`] or OS errors.
    fn recv(
        &mut self,
        out: &mut Vec<RxPacket>,
        max: usize,
        timeout: Duration,
    ) -> Result<RxStatus, RxError>;

    /// Cumulative counters since `open`.
    fn stats(&self) -> RxBackendStats;

    /// Releases resources. Idempotent.
    ///
    /// # Errors
    /// OS errors.
    fn close(&mut self) -> Result<(), RxError>;
}

/// Whether a backend can be used in this build / on this OS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendInfo {
    /// Backend name.
    pub name: String,
    /// Compiled in and supported on this OS (privileges are not checked).
    pub available: bool,
    /// Why it is unavailable, or a note on requirements.
    pub reason: Option<String>,
}

fn info(name: &str, available: bool, reason: &str) -> BackendInfo {
    BackendInfo {
        name: name.to_owned(),
        available,
        reason: Some(reason.to_owned()),
    }
}

/// Availability of every backend in [`NAMES`].
#[must_use]
pub fn available() -> Vec<BackendInfo> {
    vec![
        info("null", true, "receives nothing (tests)"),
        info(
            "pcapfile",
            true,
            "replays a pcap / pcapng file; the interface is the file path",
        ),
        if cfg!(target_os = "linux") {
            info(
                "afpacket",
                true,
                "needs CAP_NET_RAW (setcap cap_net_raw,cap_net_admin+ep)",
            )
        } else {
            info("afpacket", false, "Linux only")
        },
        if cfg!(feature = "pcap") {
            info("pcap", true, "libpcap / Npcap")
        } else {
            info(
                "pcap",
                false,
                "not compiled in (build with --features pcap)",
            )
        },
    ]
}

/// The default backend for real interfaces (`afpacket` on Linux, else `pcap` when
/// compiled in, else `null`).
#[must_use]
pub fn default_name() -> &'static str {
    if cfg!(target_os = "linux") {
        "afpacket"
    } else if cfg!(feature = "pcap") {
        "pcap"
    } else {
        "null"
    }
}

/// Creates and opens the backend `name` on `iface`.
///
/// # Errors
/// [`RxError::Unsupported`] for an unknown or unavailable backend, otherwise the
/// errors of its `open`.
pub fn open_by_name(
    name: &str,
    iface: &str,
    opts: &RxOptions,
) -> Result<Box<dyn RxBackend>, RxError> {
    let mut b: Box<dyn RxBackend> = match name {
        "null" => Box::new(NullRx::default()),
        "pcapfile" => Box::new(crate::pcapfile::PcapFileRx::default()),
        #[cfg(target_os = "linux")]
        "afpacket" => Box::new(crate::afpacket::AfPacketRx::default()),
        #[cfg(feature = "pcap")]
        "pcap" => Box::new(crate::pcap::PcapRx::default()),
        _ => {
            let why = available()
                .into_iter()
                .find(|b| b.name == name)
                .and_then(|b| b.reason)
                .unwrap_or_else(|| format!("known backends: {}", NAMES.join(", ")));
            return Err(RxError::Unsupported(format!("backend `{name}`: {why}")));
        }
    };
    b.open(iface, opts)?;
    Ok(b)
}

/// The `null` backend: never receives anything.
#[derive(Debug, Default)]
pub struct NullRx {
    open: bool,
}

impl RxBackend for NullRx {
    fn name(&self) -> &'static str {
        "null"
    }

    fn open(&mut self, _iface: &str, _opts: &RxOptions) -> Result<(), RxError> {
        if self.open {
            return Err(RxError::AlreadyOpen);
        }
        self.open = true;
        Ok(())
    }

    fn recv(
        &mut self,
        _out: &mut Vec<RxPacket>,
        _max: usize,
        timeout: Duration,
    ) -> Result<RxStatus, RxError> {
        if !self.open {
            return Err(RxError::NotOpen);
        }
        std::thread::sleep(timeout);
        Ok(RxStatus::Ok)
    }

    fn stats(&self) -> RxBackendStats {
        RxBackendStats::default()
    }

    fn close(&mut self) -> Result<(), RxError> {
        self.open = false;
        Ok(())
    }
}

/// Current wall-clock time in nanoseconds since the Unix epoch.
#[must_use]
pub fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry() {
        let names: Vec<String> = available().into_iter().map(|b| b.name).collect();
        assert_eq!(names, NAMES);
        let mut b = open_by_name("null", "x", &RxOptions::default()).unwrap();
        assert_eq!(b.name(), "null");
        let mut out = Vec::new();
        assert_eq!(
            b.recv(&mut out, 10, Duration::from_millis(1)).unwrap(),
            RxStatus::Ok
        );
        assert!(out.is_empty());
        assert!(matches!(
            b.open("x", &RxOptions::default()),
            Err(RxError::AlreadyOpen)
        ));
        b.close().unwrap();
        assert!(matches!(
            b.recv(&mut out, 1, Duration::ZERO),
            Err(RxError::NotOpen)
        ));
        assert!(matches!(
            open_by_name("bogus", "x", &RxOptions::default()),
            Err(RxError::Unsupported(m)) if m.contains("bogus")
        ));
        #[cfg(not(feature = "pcap"))]
        assert!(matches!(
            open_by_name("pcap", "x", &RxOptions::default()),
            Err(RxError::Unsupported(_))
        ));
    }
}
