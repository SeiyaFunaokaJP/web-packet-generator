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

/// Reflected CRC-32 lookup table for `poly` (the reversed polynomial).
const fn crc_table(poly: u32) -> [u32; 256] {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        #[allow(clippy::cast_possible_truncation)]
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { poly ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
}

const CRC32_TABLE: [u32; 256] = crc_table(0xEDB8_8320);
const CRC32C_TABLE: [u32; 256] = crc_table(0x82F6_3B78);

fn crc_with(table: &[u32; 256], data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for b in data {
        #[allow(clippy::cast_possible_truncation)] // low byte on purpose
        let idx = usize::from((c as u8) ^ *b);
        c = table[idx] ^ (c >> 8);
    }
    !c
}

/// CRC-32 (IEEE 802.3, as used by the Ethernet FCS / zlib) of `data`.
///
/// On the wire the Ethernet FCS carries this value least significant octet first
/// (`crc32(frame).to_le_bytes()`).
#[must_use]
pub fn crc32(data: &[u8]) -> u32 {
    crc_with(&CRC32_TABLE, data)
}

/// CRC-32C (Castagnoli, RFC 3309 / SCTP, iSCSI) of `data`. SCTP transmits it least
/// significant octet first as well.
#[must_use]
pub fn crc32c(data: &[u8]) -> u32 {
    crc_with(&CRC32C_TABLE, data)
}

/// ISO 8473 (CLNP) / ISO 10589 (IS-IS) Fletcher checksum of `data`, whose two
/// checksum octets start at offset `pos` (they are treated as zero). Returns the
/// value to store there (`X << 8 | Y`); with it in place, both running sums of the
/// verification algorithm are zero. Out-of-range `pos` is clamped.
#[must_use]
pub fn fletcher_checksum(data: &[u8], pos: usize) -> u16 {
    let mut c0: u64 = 0;
    let mut c1: u64 = 0;
    for (i, b) in data.iter().enumerate() {
        let b = if i == pos || i == pos + 1 {
            0
        } else {
            u64::from(*b)
        };
        c0 = (c0 + b) % 255;
        c1 = (c1 + c0) % 255;
    }
    let len = data.len() as u64;
    // n: 1-based position of the first checksum octet.
    let n = (pos as u64 + 1).min(len.max(1));
    let k = (len - n) % 255; // L - n
                             // X = (L - n) * C0 - C1, Y = C1 - (L - n + 1) * C0   (mod 255)
    let mut x = (k * c0 + 255 - c1) % 255;
    let mut y = (c1 + 255 * 2 - ((k + 1) % 255) * c0 % 255) % 255;
    if x == 0 {
        x = 255;
    }
    if y == 0 {
        y = 255;
    }
    #[allow(clippy::cast_possible_truncation)] // both < 256
    let v = ((x as u16) << 8) | y as u16;
    v
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

    #[test]
    fn crc_check_values() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32c(b"123456789"), 0xE306_9283);
        assert_eq!(crc32(&[]), 0);
        // RFC 3720 B.4: 32 octets of zeros.
        assert_eq!(crc32c(&[0u8; 32]), 0x8A91_36AA);
    }

    /// ISO 8473 §6.11 verification: with the checksum in place, C0 and C1 are 0.
    fn fletcher_verifies(data: &[u8]) -> bool {
        let (mut c0, mut c1) = (0u32, 0u32);
        for b in data {
            c0 = (c0 + u32::from(*b)) % 255;
            c1 = (c1 + c0) % 255;
        }
        c0 == 0 && c1 == 0
    }

    #[test]
    fn fletcher_generation_verifies() {
        let mut x: u32 = 12345;
        for len in 2..200usize {
            for pos in [0, (len - 2) / 2, len - 2] {
                let mut data: Vec<u8> = (0..len)
                    .map(|_| {
                        x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
                        (x >> 16).to_be_bytes()[3]
                    })
                    .collect();
                let c = fletcher_checksum(&data, pos);
                data[pos..pos + 2].copy_from_slice(&c.to_be_bytes());
                assert!(fletcher_verifies(&data), "len {len} pos {pos}");
                assert_eq!(fletcher_checksum(&data, pos), c, "stable");
            }
        }
        // All-zero data: X and Y are 0 → transmitted as 0xff (never 0).
        assert_eq!(fletcher_checksum(&[0; 8], 2), 0xffff);
    }
}
