//! `pcapfile` receive backend: replays a pcap / pcapng file (Ethernet link type)
//! as if its frames had just been received, keeping their original timestamps.
//! Handy for offline verification of a capture taken elsewhere and for tests
//! without a NIC (`wpg send --backend pcapfile` → `wpg capture --backend pcapfile`).

use std::collections::VecDeque;
use std::time::Duration;

use wpg_template::pcapread::read_capture;

use crate::backend::{RxBackend, RxBackendStats, RxOptions, RxPacket, RxStatus};
use crate::RxError;

/// Replays a capture file.
#[derive(Debug, Default)]
pub(crate) struct PcapFileRx {
    frames: Option<VecDeque<RxPacket>>,
    stats: RxBackendStats,
}

impl RxBackend for PcapFileRx {
    fn name(&self) -> &'static str {
        "pcapfile"
    }

    fn open(&mut self, iface: &str, opts: &RxOptions) -> Result<(), RxError> {
        if self.frames.is_some() {
            return Err(RxError::AlreadyOpen);
        }
        let bytes = std::fs::read(iface).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => RxError::Interface(iface.to_owned()),
            _ => RxError::Io(std::io::Error::new(e.kind(), format!("{iface}: {e}"))),
        })?;
        let frames = read_capture(&bytes).map_err(|e| RxError::Config(format!("{iface}: {e}")))?;
        let snaplen = opts.snaplen.max(1);
        self.frames = Some(
            frames
                .into_iter()
                .map(|f| {
                    let len = f.data.len();
                    let mut data = f.data;
                    data.truncate(snaplen);
                    RxPacket {
                        ts_ns: f.ts_ns,
                        len,
                        data,
                    }
                })
                .collect(),
        );
        self.stats = RxBackendStats::default();
        Ok(())
    }

    fn recv(
        &mut self,
        out: &mut Vec<RxPacket>,
        max: usize,
        _timeout: Duration,
    ) -> Result<RxStatus, RxError> {
        let frames = self.frames.as_mut().ok_or(RxError::NotOpen)?;
        let n = max.min(frames.len());
        out.extend(frames.drain(..n));
        self.stats.received += n as u64;
        Ok(if frames.is_empty() {
            RxStatus::Eof
        } else {
            RxStatus::Ok
        })
    }

    fn stats(&self) -> RxBackendStats {
        self.stats
    }

    fn close(&mut self) -> Result<(), RxError> {
        self.frames = None;
        Ok(())
    }
}
