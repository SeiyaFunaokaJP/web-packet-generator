//! # wpg-rx
//!
//! Receive-side verification (TODO §2.3). v1 is intentionally minimal: counters now,
//! BPF filter matching and signature (magic + seq) checks later (M9).

/// Receive counters for one interface / filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RxCounters {
    /// Frames seen.
    pub packets: u64,
    /// Octets seen.
    pub bytes: u64,
    /// Frames matching the configured filter.
    pub matched: u64,
}

impl RxCounters {
    /// Creates zeroed counters.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            packets: 0,
            bytes: 0,
            matched: 0,
        }
    }

    /// Records one received frame of `len` octets.
    pub fn record(&mut self, len: usize, matched: bool) {
        self.packets += 1;
        self.bytes += len as u64;
        self.matched += u64::from(matched);
    }

    /// Difference `self - earlier`, for rate computation between two snapshots.
    #[must_use]
    pub const fn delta(&self, earlier: &Self) -> Self {
        Self {
            packets: self.packets.saturating_sub(earlier.packets),
            bytes: self.bytes.saturating_sub(earlier.bytes),
            matched: self.matched.saturating_sub(earlier.matched),
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
                matched: 1
            }
        );
        assert_eq!(
            c.delta(&snap),
            RxCounters {
                packets: 1,
                bytes: 1500,
                matched: 0
            }
        );
    }
}
