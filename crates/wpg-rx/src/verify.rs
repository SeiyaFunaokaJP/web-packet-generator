//! Signature verification: loss, reordering, duplicates and latency per stream
//! (spec §2.3, M9). The sender stamps frames with [`wpg_core::signature`]
//! (`wpg send --signature`, job option `signature`).
//!
//! Per stream id the [`Verifier`] tracks the lowest and highest sequence number
//! seen and the number of distinct sequence numbers received:
//!
//! * **lost** = `(highest - lowest + 1) - distinct` — an estimate: frames still in
//!   flight at the moment of the snapshot, and frames sent before the capture
//!   started (below the lowest sequence number seen), are not counted;
//! * **out of order** = frames whose sequence number is below the highest one seen
//!   before them;
//! * **duplicates** = sequence numbers seen twice within the last [`WINDOW`]
//!   sequence numbers (older repeats count as out of order);
//! * **latency** = capture time − TX time of the signature. Only meaningful when
//!   sender and receiver share a clock (same host, or PTP / NTP synchronised
//!   hosts); negative values mean the clocks disagree.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use wpg_core::signature::Signature;

/// Sequence-number window for duplicate detection.
pub const WINDOW: u64 = 1 << 16;

/// Verification result of one stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamReport {
    /// Stream id from the signature.
    pub stream_id: u32,
    /// Frames received with this stream id.
    pub received: u64,
    /// Lowest sequence number seen.
    pub first_seq: u64,
    /// Highest sequence number seen.
    pub last_seq: u64,
    /// Estimated lost frames (see the module docs).
    pub lost: u64,
    /// Frames received after a higher sequence number.
    pub out_of_order: u64,
    /// Repeated sequence numbers.
    pub duplicates: u64,
    /// Lowest latency, nanoseconds.
    pub latency_min_ns: i64,
    /// Mean latency, nanoseconds.
    pub latency_avg_ns: i64,
    /// Highest latency, nanoseconds.
    pub latency_max_ns: i64,
    /// Mean absolute difference of consecutive latencies (RFC 3550 style
    /// interarrival jitter, unsmoothed), nanoseconds.
    pub jitter_ns: i64,
}

impl StreamReport {
    /// Lost / expected (0 when nothing is expected).
    #[must_use]
    pub fn loss_ratio(&self) -> f64 {
        let expected = self.last_seq - self.first_seq + 1;
        self.lost as f64 / expected as f64
    }
}

/// Verification summary over all streams.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VerifyReport {
    /// Frames that carried a signature.
    pub signed: u64,
    /// Matching frames without a signature.
    pub unsigned: u64,
    /// Per stream, by stream id.
    pub streams: Vec<StreamReport>,
}

impl VerifyReport {
    /// Sum of the streams' estimated losses.
    #[must_use]
    pub fn lost(&self) -> u64 {
        self.streams.iter().map(|s| s.lost).sum()
    }

    /// Sum of the streams' out-of-order frames.
    #[must_use]
    pub fn out_of_order(&self) -> u64 {
        self.streams.iter().map(|s| s.out_of_order).sum()
    }

    /// Sum of the streams' duplicates.
    #[must_use]
    pub fn duplicates(&self) -> u64 {
        self.streams.iter().map(|s| s.duplicates).sum()
    }
}

#[derive(Debug, Clone)]
struct StreamState {
    received: u64,
    distinct: u64,
    min: u64,
    max: u64,
    out_of_order: u64,
    duplicates: u64,
    /// Bit `seq % WINDOW` is set when `seq` (within `WINDOW` of `max`) was seen.
    seen: Vec<u64>,
    lat_min: i64,
    lat_max: i64,
    lat_sum: i128,
    last_lat: Option<i64>,
    jitter_sum: u128,
    jitter_n: u64,
}

impl StreamState {
    fn new(seq: u64) -> Self {
        #[allow(clippy::cast_possible_truncation)] // WINDOW / 64 is small
        let words = (WINDOW / 64) as usize;
        Self {
            received: 0,
            distinct: 0,
            min: seq,
            max: seq,
            out_of_order: 0,
            duplicates: 0,
            seen: vec![0; words],
            lat_min: i64::MAX,
            lat_max: i64::MIN,
            lat_sum: 0,
            last_lat: None,
            jitter_sum: 0,
            jitter_n: 0,
        }
    }

    #[allow(clippy::cast_possible_truncation)] // indices are reduced modulo WINDOW
    fn bit(&mut self, seq: u64) -> (&mut u64, u64) {
        let i = seq % WINDOW;
        (&mut self.seen[(i / 64) as usize], 1 << (i % 64))
    }

    fn clear(&mut self, from: u64, to: u64) {
        if to - from >= WINDOW {
            self.seen.iter_mut().for_each(|w| *w = 0);
            return;
        }
        for s in from..=to {
            let (w, b) = self.bit(s);
            *w &= !b;
        }
    }

    fn record(&mut self, seq: u64, latency: i64) {
        self.received += 1;
        self.lat_min = self.lat_min.min(latency);
        self.lat_max = self.lat_max.max(latency);
        self.lat_sum += i128::from(latency);
        if let Some(prev) = self.last_lat {
            self.jitter_sum += u128::from(latency.abs_diff(prev));
            self.jitter_n += 1;
        }
        self.last_lat = Some(latency);

        if self.received == 1 {
            let (w, b) = self.bit(seq);
            *w |= b;
            self.distinct = 1;
            return;
        }
        if seq > self.max {
            self.clear(self.max + 1, seq);
            self.max = seq;
            let (w, b) = self.bit(seq);
            *w |= b;
            self.distinct += 1;
            return;
        }
        if self.max - seq < WINDOW {
            let (w, b) = self.bit(seq);
            if *w & b != 0 {
                self.duplicates += 1;
                return;
            }
            *w |= b;
        }
        if seq < self.min {
            self.min = seq;
        }
        self.distinct += 1;
        if seq < self.max {
            self.out_of_order += 1;
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    fn report(&self, stream_id: u32) -> StreamReport {
        let expected = self.max - self.min + 1;
        let n = self.received.max(1);
        StreamReport {
            stream_id,
            received: self.received,
            first_seq: self.min,
            last_seq: self.max,
            lost: expected.saturating_sub(self.distinct),
            out_of_order: self.out_of_order,
            duplicates: self.duplicates,
            latency_min_ns: if self.received == 0 { 0 } else { self.lat_min },
            latency_avg_ns: (self.lat_sum / i128::from(n)) as i64,
            latency_max_ns: if self.received == 0 { 0 } else { self.lat_max },
            jitter_ns: (self.jitter_sum / u128::from(self.jitter_n.max(1))) as i64,
        }
    }
}

/// Accumulates signatures of received frames.
#[derive(Debug, Clone, Default)]
pub struct Verifier {
    streams: BTreeMap<u32, StreamState>,
    signed: u64,
    unsigned: u64,
}

impl Verifier {
    /// Creates an empty verifier.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one received frame captured at `rx_ns` (nanoseconds since the
    /// epoch); returns its signature, if any.
    pub fn record_frame(&mut self, frame: &[u8], rx_ns: u64) -> Option<Signature> {
        let sig = wpg_core::signature::find(frame);
        match sig {
            Some(s) => self.record(&s, rx_ns),
            None => self.unsigned += 1,
        }
        sig
    }

    /// Records one signature captured at `rx_ns`.
    pub fn record(&mut self, sig: &Signature, rx_ns: u64) {
        self.signed += 1;
        #[allow(clippy::cast_possible_wrap)] // a wrapped difference is still the right sign
        let latency = rx_ns.wrapping_sub(sig.ts_ns) as i64;
        self.streams
            .entry(sig.stream_id)
            .or_insert_with(|| StreamState::new(sig.seq))
            .record(sig.seq, latency);
    }

    /// The current results.
    #[must_use]
    pub fn report(&self) -> VerifyReport {
        VerifyReport {
            signed: self.signed,
            unsigned: self.unsigned,
            streams: self.streams.iter().map(|(&id, s)| s.report(id)).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sig(stream_id: u32, seq: u64, ts_ns: u64) -> Signature {
        Signature {
            stream_id,
            seq,
            ts_ns,
        }
    }

    #[test]
    fn in_order_no_loss() {
        let mut v = Verifier::new();
        for i in 0..1000 {
            v.record(&sig(1, i, 1_000 * i), 1_000 * i + 500 + (i % 3) * 10);
        }
        let r = v.report();
        assert_eq!(r.signed, 1000);
        let s = &r.streams[0];
        assert_eq!((s.received, s.first_seq, s.last_seq), (1000, 0, 999));
        assert_eq!((s.lost, s.out_of_order, s.duplicates), (0, 0, 0));
        assert_eq!((s.latency_min_ns, s.latency_max_ns), (500, 520));
        assert!(s.jitter_ns > 0 && s.jitter_ns <= 20);
        assert!(s.loss_ratio().abs() < f64::EPSILON);
    }

    #[test]
    fn loss_reorder_duplicates() {
        let mut v = Verifier::new();
        // 0..10 with 3 and 7 missing, 5 and 4 swapped, 8 duplicated.
        for seq in [0, 1, 2, 5, 4, 6, 8, 8, 9] {
            v.record(&sig(9, seq, 0), 100);
        }
        // Another stream starting mid-way (capture started late): no loss counted.
        for seq in 100..110 {
            v.record(&sig(2, seq, 0), 100);
        }
        let r = v.report();
        assert_eq!(r.streams.len(), 2);
        let s2 = &r.streams[0];
        assert_eq!((s2.stream_id, s2.first_seq, s2.lost), (2, 100, 0));
        let s9 = &r.streams[1];
        assert_eq!(s9.received, 9);
        assert_eq!(s9.lost, 2, "{s9:?}");
        assert_eq!(s9.out_of_order, 1);
        assert_eq!(s9.duplicates, 1);
        assert_eq!((r.lost(), r.out_of_order(), r.duplicates()), (2, 1, 1));
        // A late frame fills a gap.
        v.record(&sig(9, 3, 0), 100);
        let s9 = v.report().streams[1].clone();
        assert_eq!((s9.lost, s9.out_of_order), (1, 2));
    }

    #[test]
    fn window_and_negative_latency() {
        let mut v = Verifier::new();
        v.record(&sig(1, 0, 1_000), 500);
        v.record(&sig(1, WINDOW * 3, 1_000), 2_000);
        // Old sequence number outside the window: out of order, not a duplicate.
        v.record(&sig(1, 0, 1_000), 1_000);
        let s = v.report().streams[0].clone();
        assert_eq!(s.latency_min_ns, -500);
        assert_eq!(s.duplicates, 0);
        assert_eq!(s.out_of_order, 1);
        let mut frame = vec![0u8; 60];
        assert!(v.record_frame(&frame, 0).is_none());
        frame[36..60].copy_from_slice(&sig(4, 1, 0).encode());
        assert_eq!(v.record_frame(&frame, 10), Some(sig(4, 1, 0)));
        assert_eq!((v.report().signed, v.report().unsigned), (4, 1));
    }
}
