//! # wpg-tx
//!
//! Transmit engine abstraction (TODO §2.2). Every backend implements [`TxBackend`];
//! the job manager only talks to the trait.
//!
//! Available now: [`NullBackend`] (counts and discards — for tests and benchmarks).
//! Planned behind cargo features: `pcap`, `afpacket`, `afxdp`, `dpdk`.

use thiserror::Error;

mod null;

pub use null::NullBackend;

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
}

impl Default for TxOptions {
    fn default() -> Self {
        Self {
            batch_size: 64,
            ring_depth: 4096,
            pin_core: None,
        }
    }
}

/// Cumulative counters of a backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

    /// Flushes and releases resources. Idempotent.
    ///
    /// # Errors
    /// OS errors while flushing.
    fn close(&mut self) -> Result<(), TxError>;
}
