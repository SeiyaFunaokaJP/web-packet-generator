//! # wpg-tx
//!
//! Transmit engine abstraction (TODO §2.2). Every backend implements [`TxBackend`];
//! the job manager only talks to the trait.
//!
//! Pieces:
//!
//! * backends: [`NullBackend`] (counts and discards), [`PcapFileBackend`] (writes
//!   pcapng / pcap instead of transmitting), `afpacket` (Linux, feature `afpacket`,
//!   on by default: TPACKET_V3 TX ring with a `sendmmsg` fallback), `pcap` (feature
//!   `pcap`), `afxdp` (Linux 5.10+, feature `afxdp`: AF_XDP socket per queue,
//!   zero-copy / copy chosen automatically, no libbpf); [`backend::open_by_name`] /
//!   [`backend::available`] form the registry;
//! * [`rate`]: rate strings (`1.5Mpps`, `10Gbps`, `50%`), rate → inter-frame
//!   interval, and the [`rate::Pacer`] (token bucket + hybrid sleep / spin /
//!   busy-poll wait on a [`clock::Clock`], TSC where it is invariant);
//! * [`pin`]: core pinning and NUMA / queue topology for multi-core sending;
//! * [`source`]: [`source::FrameSource`] implementations (pre-rendered, on-the-fly
//!   from a `wpg-core` [`wpg_core::stream::Expansion`], multi-stream [`source::Mixer`]);
//! * [`job`]: [`job::TxJob`] runs a source through a backend on a dedicated thread
//!   with pause / resume / stop and 100 ms statistics snapshots;
//! * [`bench`]: `wpg bench` (unlimited-rate synthetic frames).

use thiserror::Error;

#[cfg(all(feature = "afpacket", target_os = "linux"))]
pub mod afpacket;
#[cfg(all(feature = "afxdp", target_os = "linux"))]
pub mod afxdp;
pub mod backend;
pub mod bench;
pub mod clock;
pub mod job;
mod null;
#[cfg(feature = "pcap")]
pub mod pcap;
pub mod pcapfile;
pub mod pin;
pub mod rate;
pub mod source;
#[cfg(target_os = "linux")]
mod sys;

pub use null::NullBackend;

/// Serialises tests that transmit on `lo`: the loopback receive backlog
/// (`netdev_max_backlog`, 1000 frames) overflows when several of them burst at
/// once, and only one AF_XDP socket can bind `lo` queue 0.
#[cfg(all(test, target_os = "linux"))]
pub(crate) fn lo_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LO: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LO.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
pub use pcapfile::PcapFileBackend;

/// One Ethernet frame ready to transmit (without FCS).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Frame {
    data: Vec<u8>,
}

impl Frame {
    /// Wraps wire bytes.
    #[must_use]
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// An empty frame whose buffer can hold `cap` octets without reallocating.
    #[must_use]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            data: Vec::with_capacity(cap),
        }
    }

    /// Mutable access to the buffer, so a TX loop can refill frames in place
    /// (see [`source::FrameSource::next_frame`]).
    pub fn buf_mut(&mut self) -> &mut Vec<u8> {
        &mut self.data
    }

    /// Consumes the frame, returning its bytes.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }

    /// Wire bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Length in octets.
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Whether the frame has no bytes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl From<Vec<u8>> for Frame {
    fn from(data: Vec<u8>) -> Self {
        Self::new(data)
    }
}

/// Options passed to [`TxBackend::open`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxOptions {
    /// Preferred number of frames per `send_batch` call.
    pub batch_size: usize,
    /// TX ring depth (backends with rings only).
    pub ring_depth: usize,
    /// CPU core to pin the TX thread to.
    pub pin_core: Option<usize>,
    /// Bypass the kernel qdisc layer (`PACKET_QDISC_BYPASS`, `afpacket` only). Faster,
    /// but frames are dropped instead of queued when the NIC queue is full.
    pub qdisc_bypass: bool,
    /// Use a memory-mapped TX ring where the backend has one (`afpacket`:
    /// TPACKET_V3 `PACKET_TX_RING`); `false` forces the plain `sendmmsg` path.
    pub tx_ring: bool,
    /// Hardware queue to bind to (`afxdp`; default 0). Several workers use
    /// consecutive queues from here (see [`backend::open_workers`]).
    pub queue: Option<u32>,
    /// AF_XDP bind mode (`afxdp`).
    pub xdp_mode: XdpMode,
    /// Busy polling: `afxdp` sets `SO_PREFER_BUSY_POLL` / `SO_BUSY_POLL` so the
    /// sending thread drives the driver itself (kernel 5.11+); the job's pacer
    /// spins and services the backend instead of sleeping
    /// ([`rate::WaitMode::BusyPoll`]).
    pub busy_poll: bool,
}

/// How an AF_XDP socket is bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XdpMode {
    /// Zero-copy when the driver supports it, else copy mode.
    #[default]
    Auto,
    /// Copy mode (`XDP_COPY`): works on every interface, including `veth` and `lo`.
    Copy,
    /// Zero-copy (`XDP_ZEROCOPY`) or fail.
    ZeroCopy,
}

impl std::str::FromStr for XdpMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "auto" => Ok(XdpMode::Auto),
            "copy" => Ok(XdpMode::Copy),
            "zerocopy" | "zero-copy" | "zc" => Ok(XdpMode::ZeroCopy),
            _ => Err(format!("unknown AF_XDP mode `{s}` (auto, copy, zerocopy)")),
        }
    }
}

impl Default for TxOptions {
    fn default() -> Self {
        Self {
            batch_size: 64,
            ring_depth: 4096,
            pin_core: None,
            qdisc_bypass: false,
            tx_ring: true,
            queue: None,
            xdp_mode: XdpMode::Auto,
            busy_poll: false,
        }
    }
}

/// Cumulative counters of a backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct TxStats {
    /// Frames handed to the NIC / sink.
    pub packets: u64,
    /// Octets handed to the NIC / sink (without FCS).
    pub bytes: u64,
    /// Frames the backend could not queue.
    pub dropped: u64,
    /// Number of `send_batch` calls.
    pub batches: u64,
}

/// Errors from transmit backends.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TxError {
    /// `send_batch` / `stats` before `open` or after `close`.
    #[error("backend is not open")]
    NotOpen,
    /// `open` on an already open backend.
    #[error("backend is already open")]
    AlreadyOpen,
    /// The interface does not exist or cannot be used.
    #[error("interface `{0}` is not available")]
    Interface(String),
    /// Insufficient privileges (`CAP_NET_RAW`, Administrator, `/dev/bpf*`).
    #[error("permission denied: {0}")]
    Permission(String),
    /// The backend is unknown or not compiled into this build / not supported on
    /// this OS.
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// Invalid job / backend configuration.
    #[error("invalid configuration: {0}")]
    Config(String),
    /// Underlying OS error.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// A packet transmit backend (pcap / AF_PACKET / AF_XDP / DPDK / file / null).
pub trait TxBackend: Send {
    /// Short backend id (e.g. `null`, `afpacket`).
    fn name(&self) -> &'static str;

    /// Opens the backend on `iface`.
    ///
    /// # Errors
    /// Interface / permission / OS errors.
    fn open(&mut self, iface: &str, opts: &TxOptions) -> Result<(), TxError>;

    /// Transmits a batch; returns how many frames were accepted.
    ///
    /// # Errors
    /// [`TxError::NotOpen`] or OS errors.
    fn send_batch(&mut self, frames: &[Frame]) -> Result<usize, TxError>;

    /// Cumulative statistics since `open`.
    fn stats(&self) -> TxStats;

    /// Optional human-readable detail about how the backend transmits (e.g.
    /// `afpacket`: `TPACKET_V3 ring` or `sendmmsg`), shown by `wpg send` / `wpg bench`.
    fn detail(&self) -> Option<String> {
        None
    }

    /// Housekeeping while the pacer waits in [`rate::WaitMode::BusyPoll`] (e.g.
    /// `afxdp`: reap the completion ring and kick the driver). Default: nothing.
    ///
    /// # Errors
    /// OS errors.
    fn service(&mut self) -> Result<(), TxError> {
        Ok(())
    }

    /// Flushes and releases resources. Idempotent.
    ///
    /// # Errors
    /// OS errors while flushing.
    fn close(&mut self) -> Result<(), TxError>;
}
