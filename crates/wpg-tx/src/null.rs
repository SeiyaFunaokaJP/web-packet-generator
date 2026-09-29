//! `null` backend: counts frames and discards them.

use crate::{Frame, TxBackend, TxError, TxOptions, TxStats};

/// Discards every frame while keeping accurate statistics. Useful for tests and for
/// measuring generator throughput without a NIC (`wpg bench --backend null`).
#[derive(Debug, Default)]
pub struct NullBackend {
    iface: Option<String>,
    stats: TxStats,
}

impl NullBackend {
    /// Creates a closed backend.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Interface given to `open`, if open.
    #[must_use]
    pub fn iface(&self) -> Option<&str> {
        self.iface.as_deref()
    }
}

impl TxBackend for NullBackend {
    fn name(&self) -> &'static str {
        "null"
    }

    fn open(&mut self, iface: &str, _opts: &TxOptions) -> Result<(), TxError> {
        if self.iface.is_some() {
            return Err(TxError::AlreadyOpen);
        }
        self.iface = Some(iface.to_owned());
        self.stats = TxStats::default();
        Ok(())
    }

    fn send_batch(&mut self, frames: &[Frame]) -> Result<usize, TxError> {
        if self.iface.is_none() {
            return Err(TxError::NotOpen);
        }
        self.stats.batches += 1;
        self.stats.packets += frames.len() as u64;
        self.stats.bytes += frames.iter().map(|f| f.len() as u64).sum::<u64>();
        Ok(frames.len())
    }

    fn stats(&self) -> TxStats {
        self.stats
    }

    fn close(&mut self) -> Result<(), TxError> {
        self.iface = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_frames_and_bytes() {
        let mut tx = NullBackend::new();
        let frames = vec![Frame::new(vec![0; 64]), Frame::new(vec![0; 1500])];

        assert!(matches!(tx.send_batch(&frames), Err(TxError::NotOpen)));

        tx.open("null0", &TxOptions::default()).unwrap();
        assert!(matches!(
            tx.open("null0", &TxOptions::default()),
            Err(TxError::AlreadyOpen)
        ));
        assert_eq!(tx.iface(), Some("null0"));
        assert_eq!(tx.send_batch(&frames).unwrap(), 2);
        assert_eq!(tx.send_batch(&frames[..1]).unwrap(), 1);

        let stats = tx.stats();
        assert_eq!(
            stats,
            TxStats {
                packets: 3,
                bytes: 64 + 1500 + 64,
                dropped: 0,
                batches: 2,
            }
        );

        tx.close().unwrap();
        tx.close().unwrap();
        assert!(matches!(tx.send_batch(&frames), Err(TxError::NotOpen)));
    }

    #[test]
    fn usable_as_trait_object() {
        let mut backend: Box<dyn TxBackend> = Box::new(NullBackend::new());
        assert_eq!(backend.name(), "null");
        backend.open("x", &TxOptions::default()).unwrap();
        assert_eq!(
            backend.send_batch(&[Frame::from(vec![1, 2, 3])]).unwrap(),
            1
        );
        assert_eq!(backend.stats().bytes, 3);
    }
}
