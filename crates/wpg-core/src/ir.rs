//! Declarative protocol definition IR (TODO §2.1 "プロトコル定義エンジン").

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::packet::FieldValue;

/// Length of a `bytes` field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Length {
    /// Fixed number of octets.
    Fixed {
        /// Octet count.
        octets: u32,
    },
    /// Length given by another (length) field of the same protocol, in octets.
    FromField {
        /// Name of the field holding the length.
        field: String,
    },
    /// Consumes the rest of the enclosing PDU.
    ToEnd,
}

/// Symbolic value of an `enum` field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumVariant {
    /// Numeric value on the wire.
    pub value: u64,
    /// Short machine name (e.g. `ipv4`).
    pub name: String,
    /// Optional human description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// One named bit inside a `flags` field (bit 0 = least significant).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlagBit {
    /// Bit position, LSB = 0.
    pub bit: u8,
    /// Short machine name (e.g. `df`).
    pub name: String,
}

/// Wire type of a field.
///
/// Serialised internally tagged, e.g. `{"kind":"uint","bits":16}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FieldType {
    /// Unsigned integer of `bits` (1..=64) bits, big endian.
    #[serde(rename = "uint")]
    UInt {
        /// Width in bits.
        bits: u8,
    },
    /// Two's complement signed integer of `bits` (2..=64) bits.
    Int {
        /// Width in bits.
        bits: u8,
    },
    /// Opaque octets.
    Bytes {
        /// How the length is determined.
        len: Length,
    },
    /// 48-bit IEEE MAC address.
    Mac,
    /// IPv4 address.
    Ipv4,
    /// IPv6 address.
    Ipv6,
    /// Variable-length OSI NSAP address (1..=20 octets).
    Nsap,
    /// Unsigned integer with named values.
    Enum {
        /// Width in bits.
        bits: u8,
        /// Known values.
        variants: Vec<EnumVariant>,
    },
    /// Bit set with named bits.
    Flags {
        /// Width in bits.
        bits: u8,
        /// Named bits.
        flags: Vec<FlagBit>,
    },
    /// Type-Length-Value container.
    Tlv {
        /// Width of the type field in bits.
        type_bits: u8,
        /// Width of the length field in bits.
        length_bits: u8,
    },
    /// Repeated items of one type.
    List {
        /// Element type.
        item: Box<FieldType>,
    },
    /// Padding up to an alignment (in octets) — `align: 0` means "fill to minimum size".
    Padding {
        /// Alignment in octets.
        align: u32,
    },
    /// Raw, uninterpreted remainder.
    Raw,
}

impl FieldType {
    /// Fixed wire width in bits, or `None` for variable-length types.
    #[must_use]
    pub fn fixed_bits(&self) -> Option<u32> {
        match self {
            FieldType::UInt { bits }
            | FieldType::Int { bits }
            | FieldType::Enum { bits, .. }
            | FieldType::Flags { bits, .. } => Some(u32::from(*bits)),
            FieldType::Mac => Some(48),
            FieldType::Ipv4 => Some(32),
            FieldType::Ipv6 => Some(128),
            FieldType::Bytes {
                len: Length::Fixed { octets },
            } => Some(octets * 8),
            FieldType::Bytes { .. }
            | FieldType::Nsap
            | FieldType::Tlv { .. }
            | FieldType::List { .. }
            | FieldType::Padding { .. }
            | FieldType::Raw => None,
        }
    }

    /// Validates internal consistency (bit widths etc.).
    ///
    /// # Errors
    /// Returns [`CoreError::InvalidWidth`] when a width is out of range.
    pub fn validate(&self, field: &str) -> Result<()> {
        let check = |bits: u8, min: u16, max: u16| {
            let b = u16::from(bits);
            if (min..=max).contains(&b) {
                Ok(())
            } else {
                Err(CoreError::InvalidWidth {
                    field: field.to_owned(),
                    bits: b,
                    min,
                    max,
                })
            }
        };
        match self {
            FieldType::UInt { bits }
            | FieldType::Enum { bits, .. }
            | FieldType::Flags { bits, .. } => check(*bits, 1, 64),
            FieldType::Int { bits } => check(*bits, 2, 64),
            FieldType::Tlv {
                type_bits,
                length_bits,
            } => check(*type_bits, 1, 32).and(check(*length_bits, 1, 32)),
            FieldType::List { item } => item.validate(field),
            _ => Ok(()),
        }
    }
}

/// Checksum algorithms that can be auto-computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChecksumKind {
    /// RFC 1071 Internet checksum over the header only (IPv4).
    InternetHeader,
    /// RFC 1071 Internet checksum including a pseudo header (TCP/UDP/ICMPv6).
    InternetPseudoHeader,
    /// ISO 8473 / IS-IS Fletcher checksum.
    Fletcher,
    /// CRC-32C (SCTP).
    Crc32c,
}

/// How an "Auto" field is computed when the user leaves it on Auto (TODO §2.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AutoCompute {
    /// Length of this PDU (or of a sub-range), with a fixed adjustment.
    Length {
        /// Unit in octets (e.g. 4 for IPv4 IHL).
        unit: u32,
        /// Value added after division by `unit`.
        #[serde(default)]
        adjust: i32,
    },
    /// Checksum.
    Checksum {
        /// Algorithm.
        algorithm: ChecksumKind,
    },
    /// Identifier of the next (upper) layer, e.g. EtherType / IP protocol number.
    NextProtocol,
    /// Per-stream counter (sequence numbers etc.).
    StreamCounter,
}

/// One field of a protocol header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldDef {
    /// Machine name, unique within the protocol (e.g. `ethertype`).
    pub name: String,
    /// Wire type.
    #[serde(rename = "type")]
    pub ty: FieldType,
    /// Default value used when the template does not set the field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<FieldValue>,
    /// Auto computation rule, if the field supports "Auto".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto: Option<AutoCompute>,
    /// Fluent message id for the field description (resolved by the UI).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_key: Option<String>,
}

impl FieldDef {
    /// Creates a field with no default, no auto rule and no description.
    #[must_use]
    pub fn new(name: impl Into<String>, ty: FieldType) -> Self {
        Self {
            name: name.into(),
            ty,
            default: None,
            auto: None,
            description_key: None,
        }
    }

    /// Builder: sets the default value.
    #[must_use]
    pub fn with_default(mut self, value: FieldValue) -> Self {
        self.default = Some(value);
        self
    }

    /// Builder: sets the auto computation rule.
    #[must_use]
    pub fn with_auto(mut self, auto: AutoCompute) -> Self {
        self.auto = Some(auto);
        self
    }
}

/// Coarse layer classification used by the GUI palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolLayer {
    /// Data link (Ethernet, 802.1Q, LLC ...).
    L2,
    /// Network (IPv4, IPv6, CLNP ...).
    L3,
    /// Transport (TCP, UDP, SCTP ...).
    L4,
    /// Application / control protocols.
    L7,
    /// Payload generators.
    Payload,
}

/// A declarative protocol definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolDef {
    /// Stable machine id (e.g. `ethernet2`).
    pub id: String,
    /// Display name (not translated; protocol names are the same in ja/en).
    pub name: String,
    /// Palette classification.
    pub layer: ProtocolLayer,
    /// Header fields in wire order.
    pub fields: Vec<FieldDef>,
}

impl ProtocolDef {
    /// Looks up a field by name.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&FieldDef> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Total header width in bits if every field is fixed-size.
    #[must_use]
    pub fn fixed_header_bits(&self) -> Option<u32> {
        self.fields.iter().map(|f| f.ty.fixed_bits()).sum()
    }

    /// Checks field widths and name uniqueness.
    ///
    /// # Errors
    /// Returns the first inconsistency found.
    pub fn validate(&self) -> Result<()> {
        for (i, field) in self.fields.iter().enumerate() {
            field.ty.validate(&field.name)?;
            if self.fields[..i].iter().any(|f| f.name == field.name) {
                return Err(CoreError::DuplicateField {
                    protocol: self.id.clone(),
                    field: field.name.clone(),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ProtocolDef {
        ProtocolDef {
            id: "test".into(),
            name: "Test".into(),
            layer: ProtocolLayer::L2,
            fields: vec![
                FieldDef::new("dst", FieldType::Mac),
                FieldDef::new("kind", FieldType::UInt { bits: 16 })
                    .with_auto(AutoCompute::NextProtocol),
            ],
        }
    }

    #[test]
    fn fixed_width_is_summed() {
        assert_eq!(sample().fixed_header_bits(), Some(64));
        let mut p = sample();
        p.fields.push(FieldDef::new("rest", FieldType::Raw));
        assert_eq!(p.fixed_header_bits(), None);
    }

    #[test]
    fn validation_catches_bad_width_and_duplicates() {
        assert!(sample().validate().is_ok());

        let mut p = sample();
        p.fields
            .push(FieldDef::new("x", FieldType::UInt { bits: 65 }));
        assert!(matches!(
            p.validate(),
            Err(CoreError::InvalidWidth { bits: 65, .. })
        ));

        let mut p = sample();
        p.fields.push(FieldDef::new("dst", FieldType::Mac));
        assert!(matches!(
            p.validate(),
            Err(CoreError::DuplicateField { .. })
        ));
    }

    #[test]
    fn field_type_serde_shape() {
        let json = serde_json::to_string(&FieldType::UInt { bits: 16 }).unwrap();
        assert_eq!(json, r#"{"kind":"uint","bits":16}"#);
        let back: FieldType = serde_json::from_str(r#"{"kind":"mac"}"#).unwrap();
        assert_eq!(back, FieldType::Mac);
        let def = sample();
        let round: ProtocolDef =
            serde_json::from_str(&serde_json::to_string(&def).unwrap()).unwrap();
        assert_eq!(round, def);
    }
}
