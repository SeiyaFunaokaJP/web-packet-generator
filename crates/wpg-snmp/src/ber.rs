//! The BER (X.690) subset SNMP uses: single-octet tags, definite lengths in the
//! short and the long form, INTEGER, OCTET STRING, NULL, OBJECT IDENTIFIER,
//! SEQUENCE and the SMI application types.
//!
//! Encoding builds every TLV from its finished content (`tlv(tag, &content)`),
//! which keeps the code obvious; SNMP messages are small.

use crate::oid::Oid;

/// Tags.
pub mod tag {
    /// INTEGER.
    pub const INTEGER: u8 = 0x02;
    /// OCTET STRING.
    pub const OCTET_STRING: u8 = 0x04;
    /// NULL.
    pub const NULL: u8 = 0x05;
    /// OBJECT IDENTIFIER.
    pub const OID: u8 = 0x06;
    /// SEQUENCE (constructed).
    pub const SEQUENCE: u8 = 0x30;
    /// `[APPLICATION 0]` IpAddress.
    pub const IP_ADDRESS: u8 = 0x40;
    /// `[APPLICATION 1]` Counter32.
    pub const COUNTER32: u8 = 0x41;
    /// `[APPLICATION 2]` Gauge32 / Unsigned32.
    pub const GAUGE32: u8 = 0x42;
    /// `[APPLICATION 3]` TimeTicks.
    pub const TIMETICKS: u8 = 0x43;
    /// `[APPLICATION 4]` Opaque.
    pub const OPAQUE: u8 = 0x44;
    /// `[APPLICATION 5]` `NsapAddress` (SNMPv2 drafts; decoded as octets).
    pub const NSAP: u8 = 0x45;
    /// `[APPLICATION 6]` Counter64.
    pub const COUNTER64: u8 = 0x46;
    /// `[APPLICATION 7]` `UInteger32` (obsolete; decoded as Gauge32).
    pub const UINTEGER32: u8 = 0x47;
    /// `[0] IMPLICIT NULL` noSuchObject.
    pub const NO_SUCH_OBJECT: u8 = 0x80;
    /// `[1] IMPLICIT NULL` noSuchInstance.
    pub const NO_SUCH_INSTANCE: u8 = 0x81;
    /// `[2] IMPLICIT NULL` endOfMibView.
    pub const END_OF_MIB_VIEW: u8 = 0x82;
}

/// A decoding failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BerError {
    /// The input ended inside a TLV.
    #[error("truncated BER data at offset {0}")]
    Truncated(usize),
    /// Indefinite / oversized length.
    #[error("unsupported BER length at offset {0}")]
    BadLength(usize),
    /// Another tag than the expected one.
    #[error("expected tag 0x{expected:02x}, found 0x{found:02x} at offset {offset}")]
    UnexpectedTag {
        /// Expected tag.
        expected: u8,
        /// Found tag.
        found: u8,
        /// Offset of the tag.
        offset: usize,
    },
    /// An INTEGER that does not fit.
    #[error("integer out of range at offset {0}")]
    IntegerRange(usize),
    /// A malformed OBJECT IDENTIFIER.
    #[error("malformed OBJECT IDENTIFIER at offset {0}")]
    BadOid(usize),
    /// Anything else (`what` describes it).
    #[error("{0}")]
    Invalid(String),
}

// ------------------------------------------------------------------ encoding

/// Appends a definite length (short form below 128, else long form).
pub fn push_len(out: &mut Vec<u8>, len: usize) {
    if len < 0x80 {
        #[allow(clippy::cast_possible_truncation)] // < 0x80
        out.push(len as u8);
        return;
    }
    let bytes = len.to_be_bytes();
    let skip = bytes.iter().take_while(|b| **b == 0).count();
    #[allow(clippy::cast_possible_truncation)] // at most 8 octets
    out.push(0x80 | (bytes.len() - skip) as u8);
    out.extend_from_slice(&bytes[skip..]);
}

/// Number of octets of the tag + length header of a TLV with `len` content octets.
#[must_use]
pub fn header_len(len: usize) -> usize {
    let mut v = Vec::with_capacity(9);
    push_len(&mut v, len);
    1 + v.len()
}

/// One TLV.
#[must_use]
pub fn tlv(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(content.len() + 6);
    out.push(tag);
    push_len(&mut out, content.len());
    out.extend_from_slice(content);
    out
}

/// A constructed TLV from encoded parts.
#[must_use]
pub fn seq(tag: u8, parts: &[&[u8]]) -> Vec<u8> {
    tlv(tag, &parts.concat())
}

/// Minimal two's complement content of a signed integer.
#[must_use]
pub fn int_content(v: i64) -> Vec<u8> {
    let b = v.to_be_bytes();
    let mut start = 0;
    while start < 7 {
        let (cur, next) = (b[start], b[start + 1]);
        if (cur == 0 && next & 0x80 == 0) || (cur == 0xff && next & 0x80 != 0) {
            start += 1;
        } else {
            break;
        }
    }
    b[start..].to_vec()
}

/// Minimal content of an unsigned integer (a leading 0 keeps it positive).
#[must_use]
pub fn uint_content(v: u64) -> Vec<u8> {
    let b = v.to_be_bytes();
    let skip = b.iter().take_while(|x| **x == 0).count().min(7);
    let mut out = Vec::with_capacity(9);
    if b[skip] & 0x80 != 0 {
        out.push(0);
    }
    out.extend_from_slice(&b[skip..]);
    out
}

/// INTEGER TLV.
#[must_use]
pub fn integer(v: i64) -> Vec<u8> {
    tlv(tag::INTEGER, &int_content(v))
}

/// OCTET STRING TLV.
#[must_use]
pub fn octets(v: &[u8]) -> Vec<u8> {
    tlv(tag::OCTET_STRING, v)
}

/// NULL TLV.
#[must_use]
pub fn null() -> Vec<u8> {
    vec![tag::NULL, 0]
}

/// Content of an OBJECT IDENTIFIER (first two arcs combined, base-128 arcs).
/// OIDs shorter than two arcs are padded with zeros (`0.0`).
#[must_use]
pub fn oid_content(oid: &Oid) -> Vec<u8> {
    let a = oid.arcs();
    let first = a.first().copied().unwrap_or(0);
    let second = a.get(1).copied().unwrap_or(0);
    let mut out = Vec::with_capacity(a.len() + 4);
    push_base128(&mut out, u64::from(first) * 40 + u64::from(second));
    for arc in a.iter().skip(2) {
        push_base128(&mut out, u64::from(*arc));
    }
    out
}

fn push_base128(out: &mut Vec<u8>, v: u64) {
    let mut tmp = [0u8; 10];
    let mut n = 0;
    let mut v = v;
    loop {
        #[allow(clippy::cast_possible_truncation)] // masked
        let b = (v & 0x7f) as u8;
        tmp[n] = b;
        n += 1;
        v >>= 7;
        if v == 0 {
            break;
        }
    }
    for i in (0..n).rev() {
        out.push(if i == 0 { tmp[i] } else { tmp[i] | 0x80 });
    }
}

/// OBJECT IDENTIFIER TLV.
#[must_use]
pub fn oid(oid: &Oid) -> Vec<u8> {
    tlv(tag::OID, &oid_content(oid))
}

// ------------------------------------------------------------------ decoding

/// One decoded TLV.
#[derive(Debug, Clone, Copy)]
pub struct Tlv<'a> {
    /// Tag octet.
    pub tag: u8,
    /// Content octets.
    pub content: &'a [u8],
    /// Offset of the tag in the reader's input.
    pub offset: usize,
    /// Offset of the content in the reader's input.
    pub content_offset: usize,
}

/// Sequential TLV reader over a byte slice.
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    /// Offset of `data[0]` in the outermost input (for error messages and the
    /// position of the v3 authentication parameters).
    base: usize,
}

impl<'a> Reader<'a> {
    /// A reader over `data`.
    #[must_use]
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            base: 0,
        }
    }

    /// A reader over the content of `t` (offsets stay absolute).
    #[must_use]
    pub fn inner(t: &Tlv<'a>) -> Self {
        Self {
            data: t.content,
            pos: 0,
            base: t.content_offset,
        }
    }

    /// Whether everything was read.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pos >= self.data.len()
    }

    /// The next tag without consuming it.
    #[must_use]
    pub fn peek_tag(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    /// Reads one TLV.
    ///
    /// # Errors
    /// Truncated input, an indefinite or oversized length.
    pub fn read(&mut self) -> Result<Tlv<'a>, BerError> {
        let start = self.pos;
        let abs = self.base + start;
        let tag = *self.data.get(self.pos).ok_or(BerError::Truncated(abs))?;
        if tag & 0x1f == 0x1f {
            return Err(BerError::Invalid(format!(
                "multi-octet tag at offset {abs} is not used by SNMP"
            )));
        }
        self.pos += 1;
        let first = *self
            .data
            .get(self.pos)
            .ok_or(BerError::Truncated(abs + 1))?;
        self.pos += 1;
        let len = if first < 0x80 {
            usize::from(first)
        } else {
            let n = usize::from(first & 0x7f);
            if n == 0 || n > 4 {
                return Err(BerError::BadLength(abs + 1));
            }
            let bytes = self
                .data
                .get(self.pos..self.pos + n)
                .ok_or(BerError::Truncated(abs + 2))?;
            self.pos += n;
            bytes.iter().fold(0usize, |a, b| (a << 8) | usize::from(*b))
        };
        let content_start = self.pos;
        let content = self
            .data
            .get(content_start..content_start + len)
            .ok_or(BerError::Truncated(self.base + content_start))?;
        self.pos += len;
        Ok(Tlv {
            tag,
            content,
            offset: abs,
            content_offset: self.base + content_start,
        })
    }

    /// Reads a TLV with tag `expected`.
    ///
    /// # Errors
    /// As [`Reader::read`], or another tag.
    pub fn expect(&mut self, expected: u8) -> Result<Tlv<'a>, BerError> {
        let t = self.read()?;
        if t.tag == expected {
            Ok(t)
        } else {
            Err(BerError::UnexpectedTag {
                expected,
                found: t.tag,
                offset: t.offset,
            })
        }
    }

    /// Reads an INTEGER.
    ///
    /// # Errors
    /// Wrong tag or out of the `i64` range.
    pub fn integer(&mut self) -> Result<i64, BerError> {
        let t = self.expect(tag::INTEGER)?;
        decode_int(t.content, t.offset)
    }

    /// Reads an INTEGER that must fit an `i32`.
    ///
    /// # Errors
    /// Wrong tag or out of range.
    pub fn int32(&mut self) -> Result<i32, BerError> {
        let off = self.base + self.pos;
        i32::try_from(self.integer()?).map_err(|_| BerError::IntegerRange(off))
    }

    /// Reads an OCTET STRING.
    ///
    /// # Errors
    /// Wrong tag.
    pub fn octets(&mut self) -> Result<&'a [u8], BerError> {
        Ok(self.expect(tag::OCTET_STRING)?.content)
    }

    /// Reads an OBJECT IDENTIFIER.
    ///
    /// # Errors
    /// Wrong tag or malformed content.
    pub fn oid(&mut self) -> Result<Oid, BerError> {
        let t = self.expect(tag::OID)?;
        decode_oid(t.content, t.offset)
    }
}

/// Signed two's complement INTEGER content.
///
/// # Errors
/// Empty or longer than 8 octets (9 with a leading zero is accepted when the
/// value still fits).
pub fn decode_int(c: &[u8], offset: usize) -> Result<i64, BerError> {
    if c.is_empty() || c.len() > 9 || (c.len() == 9 && c[0] != 0) {
        return Err(BerError::IntegerRange(offset));
    }
    let mut v: i64 = if c[0] & 0x80 != 0 { -1 } else { 0 };
    for b in c {
        v = (v << 8) | i64::from(*b);
    }
    if c.len() == 9 && v < 0 {
        return Err(BerError::IntegerRange(offset));
    }
    Ok(v)
}

/// Unsigned INTEGER content (Counter32/64, Gauge32, TimeTicks). Agents
/// sometimes send these as negative numbers; the low bits are kept then.
///
/// # Errors
/// Empty or longer than 9 octets.
pub fn decode_uint(c: &[u8], offset: usize) -> Result<u64, BerError> {
    if c.is_empty() || c.len() > 9 || (c.len() == 9 && c[0] != 0) {
        return Err(BerError::IntegerRange(offset));
    }
    Ok(c.iter().fold(0u64, |a, b| (a << 8) | u64::from(*b)))
}

/// OBJECT IDENTIFIER content.
///
/// # Errors
/// Empty content, a dangling continuation octet or an arc beyond `u32`.
pub fn decode_oid(c: &[u8], offset: usize) -> Result<Oid, BerError> {
    if c.is_empty() {
        return Err(BerError::BadOid(offset));
    }
    let mut arcs = Vec::with_capacity(c.len() + 1);
    let mut acc: u64 = 0;
    let mut first = true;
    for (i, b) in c.iter().enumerate() {
        acc = (acc << 7) | u64::from(b & 0x7f);
        if acc > u64::from(u32::MAX) * 40 + 39 {
            return Err(BerError::BadOid(offset));
        }
        if b & 0x80 == 0 {
            if first {
                let (x, y) = if acc < 40 {
                    (0, acc)
                } else if acc < 80 {
                    (1, acc - 40)
                } else {
                    (2, acc - 80)
                };
                arcs.push(u32::try_from(x).map_err(|_| BerError::BadOid(offset))?);
                arcs.push(u32::try_from(y).map_err(|_| BerError::BadOid(offset))?);
                first = false;
            } else {
                arcs.push(u32::try_from(acc).map_err(|_| BerError::BadOid(offset))?);
            }
            acc = 0;
        } else if i == c.len() - 1 {
            return Err(BerError::BadOid(offset));
        }
    }
    Ok(Oid::new(arcs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths() {
        let mut v = Vec::new();
        push_len(&mut v, 0x7f);
        assert_eq!(v, [0x7f]);
        v.clear();
        push_len(&mut v, 0x80);
        assert_eq!(v, [0x81, 0x80]);
        v.clear();
        push_len(&mut v, 0x1234);
        assert_eq!(v, [0x82, 0x12, 0x34]);
        assert_eq!(header_len(300), 4);
        // long form round trip
        let content = vec![0xab; 300];
        let enc = octets(&content);
        assert_eq!(&enc[..4], &[0x04, 0x82, 0x01, 0x2c]);
        assert_eq!(Reader::new(&enc).octets().unwrap(), &content[..]);
        // long form for a short length is still accepted
        assert_eq!(Reader::new(&[0x02, 0x81, 0x01, 0x05]).integer().unwrap(), 5);
        assert!(matches!(
            Reader::new(&[0x04, 0x80, 0x00]).read(),
            Err(BerError::BadLength(_))
        ));
        assert!(matches!(
            Reader::new(&[0x04, 0x05, 0x00]).read(),
            Err(BerError::Truncated(_))
        ));
    }

    #[test]
    fn integers() {
        for (v, enc) in [
            (0i64, vec![0x00]),
            (127, vec![0x7f]),
            (128, vec![0x00, 0x80]),
            (256, vec![0x01, 0x00]),
            (-1, vec![0xff]),
            (-128, vec![0x80]),
            (-129, vec![0xff, 0x7f]),
            (i64::from(i32::MAX), vec![0x7f, 0xff, 0xff, 0xff]),
            (i64::from(i32::MIN), vec![0x80, 0x00, 0x00, 0x00]),
        ] {
            assert_eq!(int_content(v), enc, "{v}");
            assert_eq!(decode_int(&enc, 0).unwrap(), v);
        }
        assert_eq!(uint_content(0), [0]);
        assert_eq!(uint_content(0xffff_ffff), [0, 0xff, 0xff, 0xff, 0xff]);
        assert_eq!(uint_content(u64::MAX).len(), 9);
        assert_eq!(decode_uint(&uint_content(u64::MAX), 0).unwrap(), u64::MAX);
        assert!(decode_int(&[], 0).is_err());
    }

    #[test]
    fn oids() {
        let o: Oid = "1.3.6.1.4.1.2680.1.2.7.3.2.0".parse().unwrap();
        let c = oid_content(&o);
        assert_eq!(
            c,
            [0x2b, 6, 1, 4, 1, 0x94, 0x78, 1, 2, 7, 3, 2, 0],
            "{c:02x?}"
        );
        assert_eq!(decode_oid(&c, 0).unwrap(), o);
        let big: Oid = "2.999.4294967295".parse().unwrap();
        assert_eq!(decode_oid(&oid_content(&big), 0).unwrap(), big);
        assert!(decode_oid(&[0x2b, 0x86], 0).is_err());
        assert_eq!(oid_content(&Oid::default()), [0]);
    }
}
