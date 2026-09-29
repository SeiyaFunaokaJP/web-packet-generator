//! Big-endian (MSB first) bit-level reading and writing helpers.

/// Mask with the low `width` bits set (`width` ≤ 64).
pub(crate) const fn mask(width: u32) -> u64 {
    if width >= 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    }
}

/// Writes the low `width` bits of `value` at absolute bit offset `bit_off` of `buf`.
///
/// Returns `false` (and writes nothing) when the range is outside `buf`.
pub(crate) fn put_bits(buf: &mut [u8], bit_off: usize, width: u32, value: u64) -> bool {
    let w = width as usize;
    if width > 64 || bit_off.checked_add(w).is_none_or(|end| end > buf.len() * 8) {
        return false;
    }
    let value = value & mask(width);
    for i in 0..w {
        let bit = (value >> (w - 1 - i)) & 1;
        let pos = bit_off + i;
        let byte = &mut buf[pos / 8];
        let shift = 7 - (pos % 8);
        *byte = (*byte & !(1 << shift)) | (u8::from(bit == 1) << shift);
    }
    true
}

/// Reads `width` (≤ 64) bits at absolute bit offset `bit_off`; `None` when out of range.
pub(crate) fn get_bits(buf: &[u8], bit_off: usize, width: u32) -> Option<u64> {
    let w = width as usize;
    if width > 64 || bit_off.checked_add(w)? > buf.len() * 8 {
        return None;
    }
    if bit_off.is_multiple_of(8) && w.is_multiple_of(8) {
        let start = bit_off / 8;
        return Some(
            buf[start..start + w / 8]
                .iter()
                .fold(0u64, |acc, b| (acc << 8) | u64::from(*b)),
        );
    }
    let mut v = 0u64;
    for i in 0..w {
        let pos = bit_off + i;
        let bit = (buf[pos / 8] >> (7 - (pos % 8))) & 1;
        v = (v << 1) | u64::from(bit);
    }
    Some(v)
}

/// Growable MSB-first bit writer.
#[derive(Debug, Default)]
pub(crate) struct BitWriter {
    pub(crate) buf: Vec<u8>,
    pub(crate) bits: usize,
}

impl BitWriter {
    pub(crate) fn push_bits(&mut self, width: u32, value: u64) {
        let needed = (self.bits + width as usize).div_ceil(8);
        self.buf.resize(needed, 0);
        let ok = put_bits(&mut self.buf, self.bits, width, value);
        debug_assert!(ok);
        self.bits += width as usize;
    }

    pub(crate) fn push_bytes(&mut self, bytes: &[u8]) {
        if self.bits.is_multiple_of(8) {
            self.buf.extend_from_slice(bytes);
            self.bits += bytes.len() * 8;
        } else {
            for b in bytes {
                self.push_bits(8, u64::from(*b));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_across_bytes() {
        let mut w = BitWriter::default();
        w.push_bits(4, 4);
        w.push_bits(4, 5);
        w.push_bits(6, 0x2e);
        w.push_bits(2, 1);
        w.push_bits(3, 0b010);
        w.push_bits(13, 0x1234 & 0x1fff);
        assert_eq!(w.buf, [0x45, 0xb9, 0x52, 0x34]);
        assert_eq!(get_bits(&w.buf, 8, 6), Some(0x2e));
        assert_eq!(get_bits(&w.buf, 19, 13), Some(0x1234));
        assert_eq!(get_bits(&w.buf, 30, 3), None);
        assert!(!put_bits(&mut w.buf, 30, 3, 0));
        assert_eq!(get_bits(&[0xff; 8], 0, 64), Some(u64::MAX));
    }
}
