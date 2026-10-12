//! `pcap` receive backend (feature `pcap`): libpcap / Npcap on every OS.
//!
//! The capture is opened in immediate mode with a short read timeout, so `recv`
//! returns promptly when the link is idle. Outgoing frames are filtered with
//! `pcap_setdirection(PCAP_D_IN)` where the platform supports it (not on Windows).

use std::time::Duration;

use pcap::{Active, Capture, Direction, Error as PcapError};

use crate::backend::{RxBackend, RxBackendStats, RxOptions, RxPacket, RxStatus};
use crate::RxError;

/// Read timeout handed to libpcap.
const READ_TIMEOUT_MS: i32 = 50;

/// Receives through libpcap / Npcap.
#[derive(Default)]
pub(crate) struct PcapRx {
    cap: Option<Capture<Active>>,
    stats: RxBackendStats,
}

impl std::fmt::Debug for PcapRx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PcapRx")
            .field("open", &self.cap.is_some())
            .field("stats", &self.stats)
            .finish()
    }
}

fn device_name(iface: &str) -> String {
    if cfg!(windows) && iface.starts_with('{') {
        format!("\\Device\\NPF_{iface}")
    } else {
        iface.to_owned()
    }
}

#[allow(clippy::needless_pass_by_value)] // used as `map_err(|e| map_error(.., e))`
fn map_error(iface: &str, err: PcapError) -> RxError {
    let msg = err.to_string();
    let m = msg.to_ascii_lowercase();
    if [
        "not permitted",
        "permission",
        "access is denied",
        "access denied",
    ]
    .iter()
    .any(|p| m.contains(p))
    {
        RxError::Permission(msg)
    } else if [
        "no such device",
        "does not exist",
        "doesn't exist",
        "can't find",
    ]
    .iter()
    .any(|p| m.contains(p))
    {
        RxError::Interface(iface.to_owned())
    } else {
        RxError::Io(std::io::Error::other(msg))
    }
}

impl RxBackend for PcapRx {
    fn name(&self) -> &'static str {
        "pcap"
    }

    fn open(&mut self, iface: &str, opts: &RxOptions) -> Result<(), RxError> {
        if self.cap.is_some() {
            return Err(RxError::AlreadyOpen);
        }
        let snaplen = i32::try_from(opts.snaplen).unwrap_or(i32::MAX);
        let cap = Capture::from_device(device_name(iface).as_str())
            .map_err(|e| map_error(iface, e))?
            .promisc(opts.promisc)
            .snaplen(snaplen)
            .timeout(READ_TIMEOUT_MS)
            .immediate_mode(true)
            .open()
            .map_err(|e| map_error(iface, e))?;
        if !opts.include_outgoing {
            // Not supported everywhere (Windows); then outgoing frames are counted too.
            let _ = cap.direction(Direction::In);
        }
        self.cap = Some(cap);
        self.stats = RxBackendStats::default();
        Ok(())
    }

    fn recv(
        &mut self,
        out: &mut Vec<RxPacket>,
        max: usize,
        _timeout: Duration,
    ) -> Result<RxStatus, RxError> {
        let cap = self.cap.as_mut().ok_or(RxError::NotOpen)?;
        let mut got = 0u64;
        for _ in 0..max {
            match cap.next_packet() {
                Ok(p) => {
                    let ts = p.header.ts;
                    let secs = u64::try_from(ts.tv_sec).unwrap_or(0);
                    let usecs = u64::try_from(ts.tv_usec).unwrap_or(0);
                    out.push(RxPacket {
                        ts_ns: secs * 1_000_000_000 + usecs * 1_000,
                        len: usize::try_from(p.header.len).unwrap_or(p.data.len()),
                        data: p.data.to_vec(),
                    });
                    got += 1;
                }
                Err(PcapError::TimeoutExpired) => break,
                Err(PcapError::NoMorePackets) => {
                    self.stats.received += got;
                    return Ok(RxStatus::Eof);
                }
                Err(e) => return Err(map_error("", e)),
            }
        }
        self.stats.received += got;
        if let Ok(s) = cap.stats() {
            self.stats.dropped = u64::from(s.dropped) + u64::from(s.if_dropped);
        }
        Ok(RxStatus::Ok)
    }

    fn stats(&self) -> RxBackendStats {
        self.stats
    }

    fn close(&mut self) -> Result<(), RxError> {
        self.cap = None;
        Ok(())
    }
}
