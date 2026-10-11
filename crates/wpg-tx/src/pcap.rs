//! libpcap / Npcap backend (feature `pcap`).
//!
//! Portable fallback for every OS: `pcap_sendpacket` per frame. It is the only option on
//! Windows (Npcap) and macOS (BPF) and reasonably fast on Linux, but every frame costs a
//! system call, so expect far less than `afpacket` / `afxdp`.
//!
//! TODO(perf, Windows): Npcap exposes `pcap_sendqueue_alloc` / `pcap_sendqueue_queue` /
//! `pcap_sendqueue_transmit`, which hands a whole batch to the driver in one call and can
//! replay with the original timestamps. The `pcap` crate does not wrap it; it needs raw FFI
//! against `wpcap.dll` behind `#[cfg(windows)]`.
//!
//! Requirements: libpcap-dev on Linux, nothing on macOS, the Npcap runtime (plus the Npcap SDK
//! at build time, `wpcap.lib` on `LIB`) on Windows.

use pcap::{Active, Capture, Error as PcapError};

use crate::{Frame, TxBackend, TxError, TxOptions, TxStats};

/// Largest frame we ask libpcap to be able to handle (jumbo + headroom).
const SNAPLEN: i32 = 65_535;

/// Transmit through `pcap_sendpacket`.
#[derive(Default)]
pub struct PcapBackend {
    cap: Option<Capture<Active>>,
    stats: TxStats,
}

impl std::fmt::Debug for PcapBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PcapBackend")
            .field("open", &self.cap.is_some())
            .field("stats", &self.stats)
            .finish()
    }
}

impl PcapBackend {
    /// Creates a closed backend.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// Opens a [`PcapBackend`] on `iface`.
///
/// # Errors
/// [`TxError::Permission`] without raw-socket privileges, [`TxError::Interface`] for an
/// unknown device, otherwise the libpcap error text as [`TxError::Io`].
pub fn open(iface: &str, opts: &TxOptions) -> Result<Box<dyn TxBackend>, TxError> {
    let mut backend = PcapBackend::new();
    backend.open(iface, opts)?;
    Ok(Box::new(backend))
}

/// Maps an interface name to what libpcap expects. On Windows Npcap devices are called
/// `\Device\NPF_{GUID}`; `wpg ifaces` prints the bare `{GUID}`.
fn device_name(iface: &str, windows: bool) -> String {
    if windows && iface.starts_with('{') {
        format!("\\Device\\NPF_{iface}")
    } else {
        iface.to_owned()
    }
}

fn message_is_permission(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    [
        "not permitted",
        "permission",
        "access is denied",
        "access denied",
    ]
    .iter()
    .any(|p| m.contains(p))
}

fn message_is_no_device(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    [
        "no such device",
        "does not exist",
        "doesn't exist",
        "no such file",
        "can't find",
        "cannot find",
        "adapter",
    ]
    .iter()
    .any(|p| m.contains(p))
}

fn map_error(iface: &str, err: PcapError) -> TxError {
    match err {
        PcapError::PcapError(msg) => {
            if message_is_permission(&msg) {
                TxError::Permission(msg)
            } else if message_is_no_device(&msg) {
                TxError::Interface(iface.to_owned())
            } else {
                TxError::Io(std::io::Error::other(msg))
            }
        }
        PcapError::IoError(kind @ std::io::ErrorKind::PermissionDenied) => {
            TxError::Permission(format!("{iface}: {kind}"))
        }
        PcapError::IoError(std::io::ErrorKind::NotFound) => TxError::Interface(iface.to_owned()),
        other => TxError::Io(std::io::Error::other(other.to_string())),
    }
}

/// Send errors that mean "queue full, try later" rather than "backend is broken".
fn is_transient(err: &PcapError) -> bool {
    match err {
        PcapError::PcapError(msg) => {
            let m = msg.to_ascii_lowercase();
            m.contains("no buffer space")
                || m.contains("temporarily unavailable")
                || m.contains("would block")
        }
        _ => false,
    }
}

impl TxBackend for PcapBackend {
    fn name(&self) -> &'static str {
        "pcap"
    }

    fn open(&mut self, iface: &str, _opts: &TxOptions) -> Result<(), TxError> {
        if self.cap.is_some() {
            return Err(TxError::AlreadyOpen);
        }
        let dev = device_name(iface, cfg!(windows));
        let cap = Capture::from_device(dev.as_str())
            .map_err(|e| map_error(iface, e))?
            .promisc(false)
            .snaplen(SNAPLEN)
            .immediate_mode(true)
            .open()
            .map_err(|e| map_error(iface, e))?;
        self.cap = Some(cap);
        self.stats = TxStats::default();
        Ok(())
    }

    fn send_batch(&mut self, frames: &[Frame]) -> Result<usize, TxError> {
        let cap = self.cap.as_mut().ok_or(TxError::NotOpen)?;
        self.stats.batches += 1;
        let mut sent = 0usize;
        for frame in frames {
            match cap.sendpacket(frame.as_bytes()) {
                Ok(()) => {
                    sent += 1;
                    self.stats.packets += 1;
                    self.stats.bytes += frame.len() as u64;
                }
                // Queue full: drop this frame and carry on with the rest.
                Err(e) if is_transient(&e) => self.stats.dropped += 1,
                // Hard error: report what went out; only fail if nothing did.
                Err(e) => {
                    if sent == 0 {
                        return Err(map_error("", e));
                    }
                    break;
                }
            }
        }
        Ok(sent)
    }

    fn stats(&self) -> TxStats {
        self.stats
    }

    fn close(&mut self) -> Result<(), TxError> {
        self.cap = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_device_names() {
        assert_eq!(
            device_name("{1A2B-3C}", true),
            "\\Device\\NPF_{1A2B-3C}".to_owned()
        );
        assert_eq!(device_name("{1A2B-3C}", false), "{1A2B-3C}");
        assert_eq!(device_name("eth0", true), "eth0");
        assert_eq!(device_name("\\Device\\NPF_{X}", true), "\\Device\\NPF_{X}");
    }

    #[test]
    fn maps_errors() {
        assert!(matches!(
            map_error(
                "eth0",
                PcapError::PcapError("eth0: You don't have permission to capture on that device (socket: Operation not permitted)".into())
            ),
            TxError::Permission(_)
        ));
        assert!(matches!(
            map_error("nope0", PcapError::PcapError("nope0: No such device exists".into())),
            TxError::Interface(n) if n == "nope0"
        ));
        assert!(matches!(
            map_error("x", PcapError::PcapError("something else".into())),
            TxError::Io(_)
        ));
        assert!(matches!(
            map_error(
                "x",
                PcapError::IoError(std::io::ErrorKind::PermissionDenied)
            ),
            TxError::Permission(_)
        ));
        assert!(is_transient(&PcapError::PcapError(
            "send: No buffer space available".into()
        )));
        assert!(!is_transient(&PcapError::PcapError("boom".into())));
    }

    #[test]
    fn not_open_errors() {
        let mut b = PcapBackend::new();
        assert!(matches!(
            b.send_batch(&[Frame::new(vec![0; 60])]),
            Err(TxError::NotOpen)
        ));
        b.close().unwrap();
        assert_eq!(b.name(), "pcap");
    }

    #[cfg(target_os = "linux")]
    fn broadcast_frame() -> Frame {
        let mut v = vec![0xff; 6];
        v.extend_from_slice(&[0x02, 0, 0, 0, 0, 1]);
        v.extend_from_slice(&[0x88, 0xb5]); // local experimental EtherType
        v.resize(60, 0);
        Frame::new(v)
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn sends_on_loopback() {
        let mut b = PcapBackend::new();
        match b.open("lo", &TxOptions::default()) {
            Ok(()) => {}
            Err(TxError::Permission(m)) => {
                eprintln!("skipping: no permission ({m})");
                return;
            }
            Err(TxError::Interface(n)) => {
                eprintln!("skipping: no interface {n}");
                return;
            }
            Err(e) => panic!("unexpected open error: {e}"),
        }
        assert!(matches!(
            b.open("lo", &TxOptions::default()),
            Err(TxError::AlreadyOpen)
        ));
        let frames = vec![broadcast_frame(); 5];
        assert_eq!(b.send_batch(&frames).unwrap(), 5);
        let s = b.stats();
        assert_eq!((s.packets, s.bytes, s.batches), (5, 300, 1));
        b.close().unwrap();
        b.close().unwrap();
        assert!(matches!(b.send_batch(&frames), Err(TxError::NotOpen)));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn unknown_interface() {
        match open("wpgnope0", &TxOptions::default()) {
            Err(TxError::Interface(_) | TxError::Permission(_)) => {}
            other => panic!("unexpected: {:?}", other.map(|b| b.name())),
        }
    }
}
