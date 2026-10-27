//! SMI values (`ObjectSyntax` of RFC 2578 plus the SNMPv2 exceptions) and
//! variable bindings.

use std::fmt;
use std::net::Ipv4Addr;

use crate::ber::{self, tag, BerError, Reader, Tlv};
use crate::oid::Oid;

/// A variable binding value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// INTEGER / Integer32 (enumerations included).
    Integer(i64),
    /// OCTET STRING (text, binary, BITS, display-hinted TCs).
    OctetString(Vec<u8>),
    /// NULL (requests).
    Null,
    /// OBJECT IDENTIFIER.
    Oid(Oid),
    /// IpAddress.
    IpAddress([u8; 4]),
    /// Counter32.
    Counter32(u32),
    /// Gauge32 / Unsigned32.
    Gauge32(u32),
    /// TimeTicks (hundredths of a second).
    TimeTicks(u32),
    /// Opaque (BER-wrapped arbitrary data).
    Opaque(Vec<u8>),
    /// Counter64 (SNMPv2 and later).
    Counter64(u64),
    /// `noSuchObject` exception.
    NoSuchObject,
    /// `noSuchInstance` exception.
    NoSuchInstance,
    /// `endOfMibView` exception.
    EndOfMibView,
}

/// A variable binding (`name`, `value`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarBind {
    /// Object instance.
    pub oid: Oid,
    /// Value (`Null` in requests).
    pub value: Value,
}

impl VarBind {
    /// A binding.
    #[must_use]
    pub fn new(oid: Oid, value: Value) -> Self {
        Self { oid, value }
    }

    /// A binding with a NULL value (Get / GetNext / GetBulk requests).
    #[must_use]
    pub fn null(oid: Oid) -> Self {
        Self {
            oid,
            value: Value::Null,
        }
    }
}

/// Error of [`Value::parse`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("cannot use `{text}` as {ty}")]
pub struct ValueParseError {
    /// The requested type.
    pub ty: String,
    /// The text.
    pub text: String,
}

impl Value {
    /// Type name as in the SMI (`INTEGER`, `Counter32`, ...).
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Integer(_) => "INTEGER",
            Value::OctetString(_) => "OCTET STRING",
            Value::Null => "NULL",
            Value::Oid(_) => "OBJECT IDENTIFIER",
            Value::IpAddress(_) => "IpAddress",
            Value::Counter32(_) => "Counter32",
            Value::Gauge32(_) => "Gauge32",
            Value::TimeTicks(_) => "TimeTicks",
            Value::Opaque(_) => "Opaque",
            Value::Counter64(_) => "Counter64",
            Value::NoSuchObject => "noSuchObject",
            Value::NoSuchInstance => "noSuchInstance",
            Value::EndOfMibView => "endOfMibView",
        }
    }

    /// Whether this is one of the SNMPv2 exceptions.
    #[must_use]
    pub fn is_exception(&self) -> bool {
        matches!(
            self,
            Value::NoSuchObject | Value::NoSuchInstance | Value::EndOfMibView
        )
    }

    /// Numeric value of the integer types (for rate calculation, enum lookup).
    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::Integer(v) => u64::try_from(*v).ok(),
            Value::Counter32(v) | Value::Gauge32(v) | Value::TimeTicks(v) => Some(u64::from(*v)),
            Value::Counter64(v) => Some(*v),
            _ => None,
        }
    }

    /// BER encoding.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        match self {
            Value::Integer(v) => ber::integer(*v),
            Value::OctetString(v) => ber::octets(v),
            Value::Null => ber::null(),
            Value::Oid(o) => ber::oid(o),
            Value::IpAddress(a) => ber::tlv(tag::IP_ADDRESS, a),
            Value::Counter32(v) => ber::tlv(tag::COUNTER32, &ber::uint_content(u64::from(*v))),
            Value::Gauge32(v) => ber::tlv(tag::GAUGE32, &ber::uint_content(u64::from(*v))),
            Value::TimeTicks(v) => ber::tlv(tag::TIMETICKS, &ber::uint_content(u64::from(*v))),
            Value::Opaque(v) => ber::tlv(tag::OPAQUE, v),
            Value::Counter64(v) => ber::tlv(tag::COUNTER64, &ber::uint_content(*v)),
            Value::NoSuchObject => vec![tag::NO_SUCH_OBJECT, 0],
            Value::NoSuchInstance => vec![tag::NO_SUCH_INSTANCE, 0],
            Value::EndOfMibView => vec![tag::END_OF_MIB_VIEW, 0],
        }
    }

    /// Decodes one value TLV.
    ///
    /// # Errors
    /// Unknown tag or malformed content.
    pub fn decode(t: &Tlv<'_>) -> Result<Value, BerError> {
        let u32_of = |c: &[u8]| -> Result<u32, BerError> {
            // Keep the low 32 bits of over-long encodings (some agents).
            #[allow(clippy::cast_possible_truncation)]
            Ok(ber::decode_uint(c, t.offset)? as u32)
        };
        Ok(match t.tag {
            tag::INTEGER => Value::Integer(ber::decode_int(t.content, t.offset)?),
            tag::OCTET_STRING | tag::NSAP => Value::OctetString(t.content.to_vec()),
            tag::NULL => Value::Null,
            tag::OID => Value::Oid(ber::decode_oid(t.content, t.offset)?),
            tag::IP_ADDRESS => {
                let a: [u8; 4] = t.content.try_into().map_err(|_| {
                    BerError::Invalid(format!("IpAddress of {} octets", t.content.len()))
                })?;
                Value::IpAddress(a)
            }
            tag::COUNTER32 => Value::Counter32(u32_of(t.content)?),
            tag::GAUGE32 | tag::UINTEGER32 => Value::Gauge32(u32_of(t.content)?),
            tag::TIMETICKS => Value::TimeTicks(u32_of(t.content)?),
            tag::OPAQUE => Value::Opaque(t.content.to_vec()),
            tag::COUNTER64 => Value::Counter64(ber::decode_uint(t.content, t.offset)?),
            tag::NO_SUCH_OBJECT => Value::NoSuchObject,
            tag::NO_SUCH_INSTANCE => Value::NoSuchInstance,
            tag::END_OF_MIB_VIEW => Value::EndOfMibView,
            other => {
                return Err(BerError::Invalid(format!(
                    "unknown value tag 0x{other:02x} at offset {}",
                    t.offset
                )))
            }
        })
    }

    /// Parses `text` as a value of type `ty`. Types follow net-snmp's one-letter
    /// codes or the SMI names (case-insensitive):
    ///
    /// | code | name | text |
    /// |---|---|---|
    /// | `i` | `integer` | decimal (`-5`) |
    /// | `u` | `unsigned`, `gauge32` | decimal |
    /// | `c` | `counter32` | decimal |
    /// | `C` | `counter64` | decimal |
    /// | `t` | `timeticks` | hundredths of a second |
    /// | `a` | `ipaddress` | `192.0.2.1` |
    /// | `o` | `oid` | dotted decimal |
    /// | `s` | `string` | text (UTF-8) |
    /// | `x` | `hex` | hex octets (`00 1a:2b`, `0x001a2b`) |
    /// | `d` | `decimal` | decimal octets (`1.2.3`) |
    /// | `n` | `null` | ignored |
    /// | `opaque` | | hex octets |
    ///
    /// Names (enum labels, OID names) are resolved by the MIB layer before this.
    ///
    /// # Errors
    /// Unknown type or text that does not fit it.
    pub fn parse(ty: &str, text: &str) -> Result<Value, ValueParseError> {
        let err = || ValueParseError {
            ty: ty.to_owned(),
            text: text.to_owned(),
        };
        let t = text.trim();
        let lower = ty.trim().to_ascii_lowercase();
        // `C` (Counter64) vs `c` (Counter32) is the one case-sensitive code.
        let key = if ty.trim() == "C" {
            "counter64"
        } else {
            lower.as_str()
        };
        Ok(match key {
            "i" | "int" | "integer" | "integer32" => Value::Integer(t.parse().map_err(|_| err())?),
            "u" | "unsigned" | "unsigned32" | "gauge" | "gauge32" => {
                Value::Gauge32(t.parse().map_err(|_| err())?)
            }
            "c" | "counter" | "counter32" => Value::Counter32(t.parse().map_err(|_| err())?),
            "counter64" => Value::Counter64(t.parse().map_err(|_| err())?),
            "t" | "timeticks" => Value::TimeTicks(t.parse().map_err(|_| err())?),
            "a" | "ip" | "ipaddress" => {
                Value::IpAddress(t.parse::<Ipv4Addr>().map_err(|_| err())?.octets())
            }
            "o" | "oid" | "object identifier" | "objectidentifier" => {
                Value::Oid(t.parse().map_err(|_| err())?)
            }
            "s" | "string" | "octet string" | "octetstring" => {
                Value::OctetString(text.as_bytes().to_vec())
            }
            "x" | "hex" => Value::OctetString(parse_hex(t).ok_or_else(err)?),
            "opaque" => Value::Opaque(parse_hex(t).ok_or_else(err)?),
            "d" | "decimal" => Value::OctetString(
                t.split(['.', ' '])
                    .filter(|p| !p.is_empty())
                    .map(str::parse::<u8>)
                    .collect::<Result<_, _>>()
                    .map_err(|_| err())?,
            ),
            "n" | "null" => Value::Null,
            _ => return Err(err()),
        })
    }
}

/// Hex octets: pairs optionally separated by spaces, `:` or `-`, an optional
/// `0x` prefix.
#[must_use]
pub fn parse_hex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    let mut out = Vec::new();
    for group in s.split([' ', ':', '-']).filter(|g| !g.is_empty()) {
        if group.len() % 2 == 1 {
            // `0:1a:2` style single nibbles are a single octet each.
            if group.len() == 1 {
                out.push(u8::from_str_radix(group, 16).ok()?);
                continue;
            }
            return None;
        }
        for i in (0..group.len()).step_by(2) {
            out.push(u8::from_str_radix(group.get(i..i + 2)?, 16).ok()?);
        }
    }
    Some(out)
}

/// Lower-case hex with single spaces (`00 1a 2b`).
#[must_use]
pub fn hex_spaced(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 3);
    for (i, x) in b.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(&format!("{x:02x}"));
    }
    s
}

impl fmt::Display for Value {
    /// Plain rendering without MIB knowledge (see `mib::format` for the
    /// readable one).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Integer(v) => write!(f, "{v}"),
            Value::OctetString(b) => match std::str::from_utf8(b) {
                Ok(s)
                    if s.chars()
                        .all(|c| !c.is_control() || c == '\n' || c == '\r' || c == '\t') =>
                {
                    write!(f, "{s}")
                }
                _ => f.write_str(&hex_spaced(b)),
            },
            Value::Null => f.write_str("NULL"),
            Value::Oid(o) => write!(f, "{o}"),
            Value::IpAddress(a) => write!(f, "{}", Ipv4Addr::from(*a)),
            Value::Counter32(v) | Value::Gauge32(v) | Value::TimeTicks(v) => write!(f, "{v}"),
            Value::Counter64(v) => write!(f, "{v}"),
            Value::Opaque(b) => f.write_str(&hex_spaced(b)),
            Value::NoSuchObject | Value::NoSuchInstance | Value::EndOfMibView => {
                f.write_str(self.type_name())
            }
        }
    }
}

/// Encodes a `VarBindList`.
#[must_use]
pub fn encode_varbinds(vbs: &[VarBind]) -> Vec<u8> {
    let items: Vec<Vec<u8>> = vbs
        .iter()
        .map(|vb| ber::seq(tag::SEQUENCE, &[&ber::oid(&vb.oid), &vb.value.encode()]))
        .collect();
    let refs: Vec<&[u8]> = items.iter().map(Vec::as_slice).collect();
    ber::seq(tag::SEQUENCE, &refs)
}

/// Decodes a `VarBindList`.
///
/// # Errors
/// Malformed BER.
pub fn decode_varbinds(r: &mut Reader<'_>) -> Result<Vec<VarBind>, BerError> {
    let list = r.expect(tag::SEQUENCE)?;
    let mut lr = Reader::inner(&list);
    let mut out = Vec::new();
    while !lr.is_empty() {
        let item = lr.expect(tag::SEQUENCE)?;
        let mut ir = Reader::inner(&item);
        let oid = ir.oid()?;
        let v = ir.read()?;
        out.push(VarBind {
            oid,
            value: Value::decode(&v)?,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_round_trip() {
        for v in [
            Value::Integer(-42),
            Value::OctetString(b"eth0".to_vec()),
            Value::Null,
            Value::Oid("1.3.6.1.4.1.8072".parse().unwrap()),
            Value::IpAddress([192, 0, 2, 1]),
            Value::Counter32(u32::MAX),
            Value::Gauge32(1_000_000_000),
            Value::TimeTicks(12_345_678),
            Value::Opaque(vec![0x9f, 0x78, 0x04, 0x3f, 0x80, 0, 0]),
            Value::Counter64(u64::MAX),
            Value::NoSuchObject,
            Value::NoSuchInstance,
            Value::EndOfMibView,
        ] {
            let enc = v.encode();
            let t = Reader::new(&enc).read().unwrap();
            assert_eq!(Value::decode(&t).unwrap(), v);
        }
    }

    #[test]
    fn parse_values() {
        assert_eq!(Value::parse("i", "-3").unwrap(), Value::Integer(-3));
        assert_eq!(Value::parse("C", "5").unwrap(), Value::Counter64(5));
        assert_eq!(Value::parse("c", "5").unwrap(), Value::Counter32(5));
        assert_eq!(
            Value::parse("a", "10.0.0.1").unwrap(),
            Value::IpAddress([10, 0, 0, 1])
        );
        assert_eq!(
            Value::parse("x", "00:1a:2B").unwrap(),
            Value::OctetString(vec![0, 0x1a, 0x2b])
        );
        assert_eq!(
            Value::parse("hex", "0x0102 03").unwrap(),
            Value::OctetString(vec![1, 2, 3])
        );
        assert_eq!(
            Value::parse("d", "1.2.255").unwrap(),
            Value::OctetString(vec![1, 2, 255])
        );
        assert_eq!(
            Value::parse("s", "hello").unwrap(),
            Value::OctetString(b"hello".to_vec())
        );
        assert!(Value::parse("i", "x").is_err());
        assert!(Value::parse("zz", "1").is_err());
        assert!(Value::parse("a", "1.2.3").is_err());
    }

    #[test]
    fn varbind_list() {
        let vbs = vec![
            VarBind::null("1.3.6.1.2.1.1.1.0".parse().unwrap()),
            VarBind::new("1.3.6.1.2.1.1.3.0".parse().unwrap(), Value::TimeTicks(5)),
        ];
        let enc = encode_varbinds(&vbs);
        assert_eq!(decode_varbinds(&mut Reader::new(&enc)).unwrap(), vbs);
    }
}
