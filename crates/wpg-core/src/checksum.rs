//! Checksum algorithms used by auto-computed fields.

/// Accumulates the RFC 1071 one's complement sum over several chunks, treating them
/// as one contiguous byte string (odd lengths carry over between chunks).
#[derive(Debug, Clone, Default)]
pub struct InternetSum {
    sum: u64,
    odd: Option<u8>,
}

impl InternetSum {
    /// Creates an empty accumulator.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds bytes.
    pub fn add(&mut self, mut data: &[u8]) -> &mut Self {
        if let Some(hi) = self.odd.take() {
            if let Some((&lo, rest)) = data.split_first() {
                self.sum += u64::from(u16::from_be_bytes([hi, lo]));
                data = rest;
            } else {
                self.odd = Some(hi);
                return self;
            }
        }
        let mut chunks = data.chunks_exact(2);
        for c in &mut chunks {
            self.sum += u64::from(u16::from_be_bytes([c[0], c[1]]));
        }
        if let [last] = chunks.remainder() {
            self.odd = Some(*last);
        }
        self
    }

    /// Final checksum: one's complement of the folded sum (odd byte zero padded).
    #[must_use]
    pub fn finish(&self) -> u16 {
        let mut sum = self.sum;
        if let Some(hi) = self.odd {
            sum += u64::from(hi) << 8;
        }
        while sum > 0xffff {
            sum = (sum & 0xffff) + (sum >> 16);
        }
        #[allow(clippy::cast_possible_truncation)] // folded to 16 bits above
        let folded = sum as u16;
        !folded
    }
}

/// RFC 1071 Internet checksum of `data`.
#[must_use]
pub fn internet_checksum(data: &[u8]) -> u16 {
    InternetSum::new().add(data).finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc1071_example() {
        // RFC 1071 §3 example: sum of 0001 f203 f4f5 f6f7 = ddf2 → checksum 220d.
        assert_eq!(
            internet_checksum(&[0x00, 0x01, 0xf2, 0x03, 0xf4, 0xf5, 0xf6, 0xf7]),
            0x220d
        );
    }

    #[test]
    fn chunking_is_transparent() {
        let data: Vec<u8> = (0..=200u8).collect();
        let whole = internet_checksum(&data);
        let mut s = InternetSum::new();
        s.add(&data[..3]).add(&[]).add(&data[3..8]).add(&data[8..]);
        assert_eq!(s.finish(), whole);
        assert_eq!(internet_checksum(&[]), 0xffff);
    }
}
