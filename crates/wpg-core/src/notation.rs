//! Human notation for field input (TODO §2.5).
//!
//! [`parse_value`] turns text such as `192.168.1.1`, `0x8100`, `0011.2233.4455` or
//! `49.0001.1921.6800.1001.00` into a [`FieldValue`] given the target [`FieldType`];
//! [`format_value`] is the inverse. [`parse_generator`] understands the range
//! shorthands of the GUI (`a-b`, CIDR, MAC wildcard, comma lists).

use std::fmt::Write as _;
use std::net::{Ipv4Addr, Ipv6Addr};

use thiserror::Error;

use crate::generator::Generator;
use crate::ir::{FieldType, Length};
use crate::packet::FieldValue;

/// Failure to interpret human notation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum NotationError {
    /// The text is not valid for the field type.
    #[error("invalid {expected}: `{input}`")]
    Invalid {
        /// What was expected (e.g. `IPv4 address`).
        expected: &'static str,
        /// The offending input.
        input: String,
    },
    /// The number does not fit the field width.
    #[error("value `{input}` does not fit in {bits} bits")]
    OutOfRange {
        /// The offending input.
        input: String,
        /// Field width.
        bits: u8,
    },
    /// The field type cannot be entered as text / used by this shorthand.
    #[error("unsupported field type for this notation: {0}")]
    Unsupported(&'static str),
}

fn invalid(expected: &'static str, input: &str) -> NotationError {
    NotationError::Invalid {
        expected,
        input: input.to_owned(),
    }
}

/// Parses an unsigned integer in decimal, `0x` hex or `0b` binary (`_` allowed).
///
/// # Errors
/// [`NotationError::Invalid`] on malformed text or overflow of `u128`.
pub fn parse_uint(s: &str) -> Result<u128, NotationError> {
    let t = s.trim();
    let clean: String = t.chars().filter(|c| *c != '_').collect();
    let (digits, radix) = if let Some(r) = clean
        .strip_prefix("0x")
        .or_else(|| clean.strip_prefix("0X"))
    {
        (r, 16)
    } else if let Some(r) = clean
        .strip_prefix("0b")
        .or_else(|| clean.strip_prefix("0B"))
    {
        (r, 2)
    } else {
        (clean.as_str(), 10)
    };
    if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
        return Err(invalid("integer", s));
    }
    u128::from_str_radix(digits, radix).map_err(|_| invalid("integer", s))
}

/// Parses a signed integer (optional leading `-`/`+`).
///
/// # Errors
/// [`NotationError::Invalid`] on malformed text or overflow.
pub fn parse_int(s: &str) -> Result<i128, NotationError> {
    let t = s.trim();
    let (neg, rest) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let mag = parse_uint(rest).map_err(|_| invalid("integer", s))?;
    let mag = i128::try_from(mag).map_err(|_| invalid("integer", s))?;
    Ok(if neg { -mag } else { mag })
}

/// Parses a MAC address: `00:11:22:33:44:55`, `00-11-22-33-44-55`, `0011.2233.4455`
/// or 12 bare hex digits.
///
/// # Errors
/// [`NotationError::Invalid`].
pub fn parse_mac(s: &str) -> Result<[u8; 6], NotationError> {
    let t = s.trim();
    let err = || invalid("MAC address", s);
    let hex: String = if t.contains(':') || t.contains('-') {
        let parts: Vec<&str> = t.split([':', '-']).collect();
        if parts.len() != 6 || parts.iter().any(|p| p.is_empty() || p.len() > 2) {
            return Err(err());
        }
        parts.iter().map(|p| format!("{p:0>2}")).collect()
    } else if t.contains('.') {
        let parts: Vec<&str> = t.split('.').collect();
        if parts.len() != 3 || parts.iter().any(|p| p.len() != 4) {
            return Err(err());
        }
        parts.concat()
    } else {
        t.to_owned()
    };
    if hex.len() != 12 {
        return Err(err());
    }
    let bytes = decode_hex(&hex).ok_or_else(err)?;
    let mut out = [0u8; 6];
    out.copy_from_slice(&bytes);
    Ok(out)
}

/// Parses an NSAP: hex digits, `.` / space separators ignored, optional `0x`.
///
/// # Errors
/// [`NotationError::Invalid`] unless 1..=20 whole octets.
pub fn parse_nsap(s: &str) -> Result<Vec<u8>, NotationError> {
    let t = s.trim();
    let t = t
        .strip_prefix("0x")
        .or_else(|| t.strip_prefix("0X"))
        .unwrap_or(t);
    let hex: String = t.chars().filter(|c| *c != '.' && *c != ' ').collect();
    let bytes = decode_hex(&hex).ok_or_else(|| invalid("NSAP address", s))?;
    if bytes.is_empty() || bytes.len() > 20 {
        return Err(invalid("NSAP address (1..=20 octets)", s));
    }
    Ok(bytes)
}

/// Parses raw bytes: `0xdeadbeef`, `de ad be ef` (also `:`/`,` separated) or `"ascii"`.
///
/// # Errors
/// [`NotationError::Invalid`].
pub fn parse_bytes(s: &str) -> Result<Vec<u8>, NotationError> {
    let t = s.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        return Ok(unescape(&t[1..t.len() - 1]).into_bytes());
    }
    let t2 = t
        .strip_prefix("0x")
        .or_else(|| t.strip_prefix("0X"))
        .unwrap_or(t);
    let hex: String = t2
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != ',')
        .collect();
    decode_hex(&hex).ok_or_else(|| invalid("bytes (0x.., `de ad`, or \"text\")", s))
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('0') => out.push('\0'),
                Some(o) => out.push(o),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) || !hex.is_ascii() {
        return None;
    }
    hex.as_bytes()
        .chunks(2)
        .map(|p| {
            let h = char::from(p[0]).to_digit(16)?;
            let l = char::from(p[1]).to_digit(16)?;
            u8::try_from(h * 16 + l).ok()
        })
        .collect()
}

fn mask(bits: u8) -> u128 {
    if bits >= 128 {
        u128::MAX
    } else {
        (1u128 << bits) - 1
    }
}

/// Parses `s` as a value of `ty`.
///
/// `enum`/`flags` accept numeric notation only (symbolic names are resolved by the
/// IR layer).
///
/// # Errors
/// [`NotationError`] when the text is malformed, out of range or the type is not
/// enterable as text.
pub fn parse_value(s: &str, ty: &FieldType) -> Result<FieldValue, NotationError> {
    match ty {
        FieldType::UInt { bits } | FieldType::Enum { bits, .. } | FieldType::Flags { bits, .. } => {
            let v = parse_uint(s)?;
            if v > mask(*bits) {
                return Err(NotationError::OutOfRange {
                    input: s.trim().to_owned(),
                    bits: *bits,
                });
            }
            u64::try_from(v)
                .map(FieldValue::UInt)
                .map_err(|_| NotationError::OutOfRange {
                    input: s.trim().to_owned(),
                    bits: *bits,
                })
        }
        FieldType::Int { bits } => {
            let v = parse_int(s)?;
            let half = 1i128 << (*bits - 1).min(126);
            if v < -half || v >= half {
                return Err(NotationError::OutOfRange {
                    input: s.trim().to_owned(),
                    bits: *bits,
                });
            }
            i64::try_from(v)
                .map(FieldValue::Int)
                .map_err(|_| NotationError::OutOfRange {
                    input: s.trim().to_owned(),
                    bits: *bits,
                })
        }
        FieldType::Ipv4 => s
            .trim()
            .parse::<Ipv4Addr>()
            .map(FieldValue::Ipv4)
            .map_err(|_| invalid("IPv4 address", s)),
        FieldType::Ipv6 => s
            .trim()
            .parse::<Ipv6Addr>()
            .map(FieldValue::Ipv6)
            .map_err(|_| invalid("IPv6 address", s)),
        FieldType::Mac => parse_mac(s).map(FieldValue::Mac),
        FieldType::Nsap => parse_nsap(s).map(FieldValue::Nsap),
        FieldType::Bytes { len } => {
            let b = parse_bytes(s)?;
            if let Length::Fixed { octets } = len {
                if b.len() != *octets as usize {
                    return Err(invalid("bytes of the fixed field length", s));
                }
            }
            Ok(FieldValue::Bytes(b))
        }
        FieldType::Raw => parse_bytes(s).map(FieldValue::Bytes),
        FieldType::CString => {
            let b = parse_bytes(s)?;
            if b.contains(&0) {
                return Err(invalid("string without NUL octets", s));
            }
            Ok(FieldValue::Bytes(b))
        }
        FieldType::Tlv { .. } | FieldType::List { .. } | FieldType::Padding { .. } => {
            Err(NotationError::Unsupported("composite field"))
        }
    }
}

/// Guesses the field type of a bare literal (used for untyped cardinality queries).
#[must_use]
pub fn infer_type(s: &str) -> Option<FieldType> {
    let t = s.trim();
    if t.parse::<Ipv4Addr>().is_ok() {
        Some(FieldType::Ipv4)
    } else if t.parse::<Ipv6Addr>().is_ok() {
        Some(FieldType::Ipv6)
    } else if parse_mac(t).is_ok() && (t.contains(':') || t.contains('-') || t.contains('.')) {
        Some(FieldType::Mac)
    } else if parse_uint(t).is_ok() {
        Some(FieldType::UInt { bits: 64 })
    } else if parse_int(t).is_ok() {
        Some(FieldType::Int { bits: 64 })
    } else if parse_nsap(t).is_ok() && t.contains('.') {
        Some(FieldType::Nsap)
    } else {
        None
    }
}

/// Formats a value for display; the inverse of [`parse_value`].
#[must_use]
pub fn format_value(v: &FieldValue, ty: &FieldType) -> String {
    match v {
        FieldValue::UInt(x) => match ty {
            FieldType::Flags { .. } => format!("{x:#x}"),
            _ => x.to_string(),
        },
        FieldValue::Int(x) => x.to_string(),
        FieldValue::Ipv4(a) => a.to_string(),
        FieldValue::Ipv6(a) => a.to_string(),
        FieldValue::Mac(m) => format_mac(m),
        FieldValue::Nsap(b) => format_nsap(b),
        FieldValue::Bytes(b) if matches!(ty, FieldType::CString) => format_text(b),
        FieldValue::Bytes(b) => format_bytes(b),
        // Composite values have no human notation yet (the GUI edits them as trees).
        FieldValue::List(_) | FieldValue::Struct(_) | FieldValue::Tlv(_) => format!("{v:?}"),
    }
}

/// `00:11:22:33:44:55`.
#[must_use]
pub fn format_mac(m: &[u8; 6]) -> String {
    format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        m[0], m[1], m[2], m[3], m[4], m[5]
    )
}

/// `49.0001.1921.6800.1001.00`: AFI octet, 2-octet groups, final (NSEL) octet.
#[must_use]
pub fn format_nsap(b: &[u8]) -> String {
    let mut out = String::new();
    let n = b.len();
    for (i, x) in b.iter().enumerate() {
        if i > 0 && (i == 1 || i == n - 1 || (i - 1) % 2 == 0) {
            out.push('.');
        }
        let _ = write!(out, "{x:02x}");
    }
    out
}

/// An NSAP split the way IS-IS reads a NET: AFI, area (the rest of the area
/// address), 6-octet system ID and NSEL. See [`nsap_parts`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NsapParts<'a> {
    /// Authority and format identifier (first octet, e.g. `0x49` private).
    pub afi: u8,
    /// Area octets after the AFI (may be empty).
    pub area: &'a [u8],
    /// System ID (6 octets).
    pub system_id: &'a [u8],
    /// NSAP selector (last octet, `0x00` for a NET).
    pub nsel: u8,
}

/// Splits an NSAP into AFI / area / system ID / NSEL; `None` when it is shorter
/// than the 8 octets those need.
#[must_use]
pub fn nsap_parts(b: &[u8]) -> Option<NsapParts<'_>> {
    let (&afi, rest) = b.split_first()?;
    let (&nsel, rest) = rest.split_last()?;
    let split = rest.len().checked_sub(6)?;
    let (area, system_id) = rest.split_at(split);
    Some(NsapParts {
        afi,
        area,
        system_id,
        nsel,
    })
}

/// `de ad be ef`.
#[must_use]
pub fn format_bytes(b: &[u8]) -> String {
    let mut out = String::with_capacity(b.len() * 3);
    for (i, x) in b.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let _ = write!(out, "{x:02x}");
    }
    out
}

/// `"text"` (with `\\`, `\"`, `\n`, `\r`, `\t` escapes) when every octet is
/// printable ASCII or one of those controls, else [`format_bytes`]. The inverse of
/// the quoted form of [`parse_bytes`].
#[must_use]
pub fn format_text(b: &[u8]) -> String {
    if !b
        .iter()
        .all(|x| (0x20..0x7f).contains(x) || matches!(x, b'\n' | b'\r' | b'\t'))
    {
        return format_bytes(b);
    }
    let mut out = String::with_capacity(b.len() + 2);
    out.push('"');
    for x in b {
        match x {
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            _ => out.push(char::from(*x)),
        }
    }
    out.push('"');
    out
}

/// Options for CIDR expansion.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CidrOptions {
    /// Include the network (and IPv4 broadcast) address instead of only hosts.
    pub include_network_broadcast: bool,
}

/// Parses a range shorthand with default options ([`CidrOptions::default`]).
///
/// # Errors
/// See [`parse_generator_with`].
pub fn parse_generator(s: &str, ty: &FieldType) -> Result<Generator, NotationError> {
    parse_generator_with(s, ty, CidrOptions::default())
}

/// Parses GUI range shorthand into a [`Generator`]:
/// `a-b` / `a..b` / `a – b` (range), `1,5,9` (list), `10.0.0.0/24` (CIDR, IPv4/IPv6),
/// `00:11:22:*` (MAC wildcard: random low octets under the fixed prefix). A single
/// value yields a one-element list.
///
/// # Errors
/// [`NotationError`] if any part does not parse for `ty`.
pub fn parse_generator_with(
    s: &str,
    ty: &FieldType,
    opts: CidrOptions,
) -> Result<Generator, NotationError> {
    let t = s.trim();
    if t.is_empty() {
        return Err(invalid("value", s));
    }
    if matches!(ty, FieldType::Mac) && t.ends_with('*') {
        return mac_wildcard(t);
    }
    if !matches!(ty, FieldType::Bytes { .. }) && t.contains(',') {
        let mut values = Vec::new();
        for part in t.split(',') {
            let p = part.trim();
            parse_value(p, ty)?;
            values.push(p.to_owned());
        }
        return Ok(Generator::List { values });
    }
    if parse_value(t, ty).is_ok() {
        return Ok(Generator::List {
            values: vec![t.to_owned()],
        });
    }
    if let Some((addr, len)) = t.split_once('/') {
        return cidr(addr.trim(), len.trim(), ty, opts);
    }
    for sep in ["..", "–", "—"] {
        if let Some((a, b)) = t.split_once(sep) {
            return make_range(a, b, ty);
        }
    }
    // '-' may also be a MAC separator or a sign: try every split point.
    for (i, c) in t.char_indices() {
        if c == '-' && i > 0 {
            if let Ok(g) = make_range(&t[..i], &t[i + 1..], ty) {
                return Ok(g);
            }
        }
    }
    Err(invalid("value, range, CIDR or list", s))
}

fn make_range(a: &str, b: &str, ty: &FieldType) -> Result<Generator, NotationError> {
    let (a, b) = (a.trim(), b.trim());
    parse_value(a, ty)?;
    parse_value(b, ty)?;
    Ok(Generator::Range {
        start: a.to_owned(),
        end: b.to_owned(),
        step: 1,
    })
}

fn mac_wildcard(t: &str) -> Result<Generator, NotationError> {
    let prefix = t.trim_end_matches('*').trim_end_matches([':', '-', '.']);
    let parts: Vec<&str> = prefix.split([':', '-']).filter(|p| !p.is_empty()).collect();
    if parts.len() > 5
        || parts
            .iter()
            .any(|p| p.len() > 2 || u8::from_str_radix(p, 16).is_err())
    {
        return Err(invalid("MAC wildcard like `00:11:22:*`", t));
    }
    let oui = parts
        .iter()
        .map(|p| format!("{p:0>2}"))
        .collect::<Vec<_>>()
        .join(":");
    Ok(Generator::RandomMac { oui, seed: None })
}

fn cidr(
    addr: &str,
    len: &str,
    ty: &FieldType,
    opts: CidrOptions,
) -> Result<Generator, NotationError> {
    let plen: u32 = len.parse().map_err(|_| invalid("prefix length", len))?;
    match ty {
        FieldType::Ipv4 => {
            let a: Ipv4Addr = addr.parse().map_err(|_| invalid("IPv4 address", addr))?;
            if plen > 32 {
                return Err(invalid("prefix length 0..=32", len));
            }
            let host_mask = u32::MAX.checked_shr(plen).unwrap_or(0);
            let net = u32::from(a) & !host_mask;
            let bc = net | host_mask;
            let (lo, hi) = if opts.include_network_broadcast || plen >= 31 {
                (net, bc)
            } else {
                (net + 1, bc - 1)
            };
            Ok(Generator::Range {
                start: Ipv4Addr::from(lo).to_string(),
                end: Ipv4Addr::from(hi).to_string(),
                step: 1,
            })
        }
        FieldType::Ipv6 => {
            let a: Ipv6Addr = addr.parse().map_err(|_| invalid("IPv6 address", addr))?;
            if plen > 128 {
                return Err(invalid("prefix length 0..=128", len));
            }
            let host_mask = u128::MAX.checked_shr(plen).unwrap_or(0);
            let net = u128::from(a) & !host_mask;
            let top = net | host_mask;
            let lo = if opts.include_network_broadcast || plen >= 127 {
                net
            } else {
                net + 1
            };
            Ok(Generator::Range {
                start: Ipv6Addr::from(lo).to_string(),
                end: Ipv6Addr::from(top).to_string(),
                step: 1,
            })
        }
        _ => Err(NotationError::Unsupported("CIDR needs an IPv4/IPv6 field")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(bits: u8) -> FieldType {
        FieldType::UInt { bits }
    }

    #[test]
    fn nsap_parts_split() {
        let b = parse_nsap("49.0001.1921.6800.1001.00").unwrap();
        let p = nsap_parts(&b).unwrap();
        assert_eq!(p.afi, 0x49);
        assert_eq!(p.area, &[0, 1]);
        assert_eq!(p.system_id, &[0x19, 0x21, 0x68, 0, 0x10, 1]);
        assert_eq!(p.nsel, 0);
        let p = nsap_parts(&b[2..]).unwrap();
        assert!(p.area.is_empty());
        assert_eq!(nsap_parts(&b[3..]), None);
        assert_eq!(nsap_parts(&[]), None);
    }

    #[test]
    fn integers() {
        assert_eq!(parse_value("42", &u(8)), Ok(FieldValue::UInt(42)));
        assert_eq!(parse_value("0x8100", &u(16)), Ok(FieldValue::UInt(0x8100)));
        assert_eq!(parse_value("0b1010", &u(4)), Ok(FieldValue::UInt(10)));
        assert_eq!(parse_value("1_000", &u(16)), Ok(FieldValue::UInt(1000)));
        assert!(matches!(
            parse_value("256", &u(8)),
            Err(NotationError::OutOfRange { .. })
        ));
        assert!(parse_value("abc", &u(8)).is_err());
        assert!(parse_value("", &u(8)).is_err());
        let i8t = FieldType::Int { bits: 8 };
        assert_eq!(parse_value("-128", &i8t), Ok(FieldValue::Int(-128)));
        assert!(parse_value("128", &i8t).is_err());
        assert_eq!(
            parse_value("0xffffffffffffffff", &u(64)),
            Ok(FieldValue::UInt(u64::MAX))
        );
    }

    #[test]
    fn addresses() {
        assert_eq!(
            parse_value("10.0.0.1", &FieldType::Ipv4),
            Ok(FieldValue::Ipv4(Ipv4Addr::new(10, 0, 0, 1)))
        );
        assert!(parse_value("10.0.0.256", &FieldType::Ipv4).is_err());
        assert!(parse_value("fe80::1", &FieldType::Ipv6).is_ok());
        let m = FieldValue::Mac([0, 0x11, 0x22, 0x33, 0x44, 0x55]);
        for s in [
            "00:11:22:33:44:55",
            "00-11-22-33-44-55",
            "0011.2233.4455",
            "001122334455",
        ] {
            assert_eq!(parse_value(s, &FieldType::Mac), Ok(m.clone()), "{s}");
        }
        assert!(parse_value("00:11:22", &FieldType::Mac).is_err());
        assert_eq!(
            parse_value("49.0001.1921.6800.1001.00", &FieldType::Nsap),
            Ok(FieldValue::Nsap(vec![
                0x49, 0, 1, 0x19, 0x21, 0x68, 0, 0x10, 1, 0
            ]))
        );
        assert!(parse_value("49.001", &FieldType::Nsap).is_err());
    }

    #[test]
    fn bytes() {
        let ty = FieldType::Bytes { len: Length::ToEnd };
        let want = FieldValue::Bytes(vec![0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(parse_value("0xdeadbeef", &ty), Ok(want.clone()));
        assert_eq!(parse_value("de ad be ef", &ty), Ok(want));
        assert_eq!(
            parse_value("\"hi\\n\"", &ty),
            Ok(FieldValue::Bytes(vec![b'h', b'i', b'\n']))
        );
        let fixed = FieldType::Bytes {
            len: Length::Fixed { octets: 2 },
        };
        assert!(parse_value("00", &fixed).is_err());
    }

    #[test]
    fn round_trips() {
        let cases: Vec<(FieldType, FieldValue)> = vec![
            (u(16), FieldValue::UInt(65535)),
            (FieldType::Int { bits: 12 }, FieldValue::Int(-5)),
            (FieldType::Ipv4, FieldValue::Ipv4(Ipv4Addr::new(1, 2, 3, 4))),
            (
                FieldType::Ipv6,
                FieldValue::Ipv6("2001:db8::1".parse().unwrap()),
            ),
            (FieldType::Mac, FieldValue::Mac([1, 2, 3, 4, 5, 6])),
            (FieldType::Nsap, FieldValue::Nsap(vec![0x49])),
            (FieldType::Nsap, FieldValue::Nsap(vec![0x49, 0])),
            (FieldType::Nsap, FieldValue::Nsap(vec![0x49, 0, 1, 2])),
            (
                FieldType::Nsap,
                FieldValue::Nsap(vec![0x49, 0, 1, 0x19, 0x21, 0x68, 0, 0x10, 1, 0]),
            ),
            (FieldType::Raw, FieldValue::Bytes(vec![1, 2, 255])),
            (FieldType::Raw, FieldValue::Bytes(vec![])),
            (
                FieldType::Flags {
                    bits: 8,
                    flags: vec![],
                },
                FieldValue::UInt(0x12),
            ),
        ];
        for (ty, v) in cases {
            let s = format_value(&v, &ty);
            assert_eq!(parse_value(&s, &ty), Ok(v), "text `{s}`");
        }
        assert_eq!(
            format_nsap(&[0x49, 0, 1, 0x19, 0x21, 0x68, 0, 0x10, 1, 0]),
            "49.0001.1921.6800.1001.00"
        );
    }

    #[test]
    fn shorthand_ranges() {
        let g = parse_generator("192.168.1.1-192.168.5.254", &FieldType::Ipv4).unwrap();
        assert_eq!(
            g,
            Generator::Range {
                start: "192.168.1.1".into(),
                end: "192.168.5.254".into(),
                step: 1
            }
        );
        assert!(matches!(
            parse_generator("1-100", &u(8)).unwrap(),
            Generator::Range { .. }
        ));
        assert_eq!(
            parse_generator("1,5,9", &u(8)).unwrap(),
            Generator::List {
                values: vec!["1".into(), "5".into(), "9".into()]
            }
        );
        // MAC with '-' separators is a single value, and a range still splits.
        assert!(matches!(
            parse_generator("00-11-22-33-44-55", &FieldType::Mac).unwrap(),
            Generator::List { .. }
        ));
        assert!(matches!(
            parse_generator("00:11:22:33:44:00-00:11:22:33:44:ff", &FieldType::Mac).unwrap(),
            Generator::Range { .. }
        ));
        assert!(matches!(
            parse_generator("-5--1", &FieldType::Int { bits: 8 }).unwrap(),
            Generator::Range { .. }
        ));
        assert!(parse_generator("zzz", &FieldType::Ipv4).is_err());
    }

    #[test]
    fn shorthand_cidr_and_wildcard() {
        let g = parse_generator("10.0.0.0/24", &FieldType::Ipv4).unwrap();
        assert_eq!(
            g,
            Generator::Range {
                start: "10.0.0.1".into(),
                end: "10.0.0.254".into(),
                step: 1
            }
        );
        let g = parse_generator_with(
            "10.0.0.7/24",
            &FieldType::Ipv4,
            CidrOptions {
                include_network_broadcast: true,
            },
        )
        .unwrap();
        assert_eq!(
            g,
            Generator::Range {
                start: "10.0.0.0".into(),
                end: "10.0.0.255".into(),
                step: 1
            }
        );
        assert_eq!(
            parse_generator("10.0.0.1/32", &FieldType::Ipv4).unwrap(),
            Generator::Range {
                start: "10.0.0.1".into(),
                end: "10.0.0.1".into(),
                step: 1
            }
        );
        let g = parse_generator("2001:db8::/126", &FieldType::Ipv6).unwrap();
        assert_eq!(
            g,
            Generator::Range {
                start: "2001:db8::1".into(),
                end: "2001:db8::3".into(),
                step: 1
            }
        );
        assert_eq!(
            parse_generator("00:11:22:*", &FieldType::Mac).unwrap(),
            Generator::RandomMac {
                oui: "00:11:22".into(),
                seed: None
            }
        );
        assert!(parse_generator("10.0.0.0/33", &FieldType::Ipv4).is_err());
    }
}
