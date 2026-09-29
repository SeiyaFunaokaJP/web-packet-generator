//! Packet instances: a stack of [`Layer`]s whose fields hold [`FieldValue`]s.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, Ipv6Addr};

use serde::{Deserialize, Serialize};

/// A concrete field value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldValue {
    /// Unsigned integer (also used for `enum` and `flags`). Serialised as `uint`,
    /// matching the field type name.
    #[serde(rename = "uint")]
    UInt(u64),
    /// Signed integer.
    Int(i64),
    /// Raw octets.
    Bytes(Vec<u8>),
    /// MAC address.
    Mac([u8; 6]),
    /// IPv4 address.
    Ipv4(Ipv4Addr),
    /// IPv6 address.
    Ipv6(Ipv6Addr),
    /// NSAP address octets.
    Nsap(Vec<u8>),
    /// Elements of a `list` field.
    List(Vec<FieldValue>),
    /// One element of a `list` of field groups: field name → value.
    Struct(BTreeMap<String, FieldValue>),
    /// Items of a `tlv` field.
    Tlv(Vec<TlvItem>),
}

/// One item of a `tlv` field value.
///
/// * `raw` set: the octets are emitted verbatim, without type / length framing
///   (garbage or truncated items; the parser produces these for undecodable rests).
/// * otherwise the item is `type` (or the type of the variant called `name`), the
///   `length` (Auto when `None`) and the value: `data` verbatim when set, else the
///   variant's `fields` (unset fields use their defaults).
///
/// YAML: `{ name: mss, fields: { value: { uint: 1460 } } }`, `{ type: 99, data: [1, 2] }`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TlvItem {
    /// Wire type; defaults to the type of the variant named `name`.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub ty: Option<u64>,
    /// Variant name (selects the value fields).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Manual length field value (`None` = Auto).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<u64>,
    /// Value fields of the variant.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, FieldValue>,
    /// Value octets used verbatim instead of `fields` (unknown types).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<u8>>,
    /// Whole item octets, emitted verbatim (see the type docs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<Vec<u8>>,
}

impl TlvItem {
    /// An item of the named variant.
    #[must_use]
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..Self::default()
        }
    }

    /// An item of a (possibly unknown) type with verbatim value octets.
    #[must_use]
    pub fn with_type(ty: u64, data: impl Into<Vec<u8>>) -> Self {
        Self {
            ty: Some(ty),
            data: Some(data.into()),
            ..Self::default()
        }
    }

    /// Verbatim octets without framing.
    #[must_use]
    pub fn raw(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            raw: Some(bytes.into()),
            ..Self::default()
        }
    }

    /// Builder: sets a value field.
    #[must_use]
    pub fn with(mut self, field: impl Into<String>, value: impl Into<FieldValue>) -> Self {
        self.fields.insert(field.into(), value.into());
        self
    }

    /// Builder: sets a Manual length.
    #[must_use]
    pub fn with_length(mut self, length: u64) -> Self {
        self.length = Some(length);
        self
    }
}

impl From<Vec<TlvItem>> for FieldValue {
    fn from(v: Vec<TlvItem>) -> Self {
        FieldValue::Tlv(v)
    }
}

impl From<Vec<FieldValue>> for FieldValue {
    fn from(v: Vec<FieldValue>) -> Self {
        FieldValue::List(v)
    }
}

macro_rules! from_uint {
    ($($t:ty),*) => {$(
        impl From<$t> for FieldValue {
            fn from(v: $t) -> Self {
                FieldValue::UInt(u64::from(v))
            }
        }
    )*};
}
from_uint!(u8, u16, u32, u64, bool);

macro_rules! from_int {
    ($($t:ty),*) => {$(
        impl From<$t> for FieldValue {
            fn from(v: $t) -> Self {
                FieldValue::Int(i64::from(v))
            }
        }
    )*};
}
from_int!(i8, i16, i32, i64);

impl From<Ipv4Addr> for FieldValue {
    fn from(v: Ipv4Addr) -> Self {
        FieldValue::Ipv4(v)
    }
}

impl From<Ipv6Addr> for FieldValue {
    fn from(v: Ipv6Addr) -> Self {
        FieldValue::Ipv6(v)
    }
}

impl From<Vec<u8>> for FieldValue {
    fn from(v: Vec<u8>) -> Self {
        FieldValue::Bytes(v)
    }
}

impl From<&[u8]> for FieldValue {
    fn from(v: &[u8]) -> Self {
        FieldValue::Bytes(v.to_vec())
    }
}

impl<const N: usize> From<[u8; N]> for FieldValue {
    fn from(v: [u8; N]) -> Self {
        FieldValue::Bytes(v.to_vec())
    }
}

impl FieldValue {
    /// The value of an unsigned integer field.
    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            FieldValue::UInt(v) => Some(*v),
            _ => None,
        }
    }

    /// The octets of a `bytes` / `raw` / `nsap` value.
    #[must_use]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            FieldValue::Bytes(v) | FieldValue::Nsap(v) => Some(v),
            _ => None,
        }
    }
}

/// One protocol layer of a packet (an instance of a `ProtocolDef`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layer {
    /// Protocol id (see `ProtocolDef::id`).
    pub protocol: String,
    /// Explicitly set ("Manual") field values; unset fields use default / Auto.
    /// Manual values are used verbatim, even when inconsistent (malformed packets).
    #[serde(default)]
    pub fields: BTreeMap<String, FieldValue>,
    /// Cut this layer (header + everything after it) to at most this many octets
    /// after all lengths / checksums were computed (TODO §2.1 "末尾切詰め").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncate: Option<usize>,
}

impl Layer {
    /// Creates an empty layer of the given protocol.
    #[must_use]
    pub fn new(protocol: impl Into<String>) -> Self {
        Self {
            protocol: protocol.into(),
            fields: BTreeMap::new(),
            truncate: None,
        }
    }

    /// Builder: sets a field value (switching the field to "Manual").
    #[must_use]
    pub fn with(mut self, field: impl Into<String>, value: impl Into<FieldValue>) -> Self {
        self.fields.insert(field.into(), value.into());
        self
    }

    /// Builder: truncates the layer to `octets` (see [`Layer::truncate`]).
    #[must_use]
    pub fn truncated(mut self, octets: usize) -> Self {
        self.truncate = Some(octets);
        self
    }
}

/// Frame check sequence handling (IEEE 802.3 CRC-32 appended after the frame).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FcsMode {
    /// Append the correct CRC-32.
    Auto,
    /// Append a deliberately wrong CRC-32 (the complement of the correct one).
    Bad,
    /// Append this value (as the four FCS octets, first octet = low byte, like
    /// `Auto`); produced by `parse_with_fcs` for arbitrary wrong FCS values.
    Manual(u32),
    /// Do not append an FCS (same as leaving [`Packet::fcs`] unset).
    None,
}

const fn true_fn() -> bool {
    true
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde skip_serializing_if signature
fn is_true(v: &bool) -> bool {
    *v
}

/// A packet: layers from outermost (L2) to innermost, plus frame-level options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Packet {
    /// Layer stack.
    pub layers: Vec<Layer>,
    /// Zero-pad the frame to the minimum frame length of its outermost protocol
    /// (`ProtocolDef::min_frame_len`, Ethernet: 60 octets without FCS). Default `true`.
    #[serde(default = "true_fn", skip_serializing_if = "is_true")]
    pub pad_to_min: bool,
    /// Frame check sequence to append (`None` = no FCS, as capture / TX backends
    /// usually expect).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fcs: Option<FcsMode>,
}

impl Default for Packet {
    fn default() -> Self {
        Self {
            layers: Vec::new(),
            pad_to_min: true,
            fcs: None,
        }
    }
}

impl From<Vec<Layer>> for Packet {
    fn from(layers: Vec<Layer>) -> Self {
        Self::from_layers(layers)
    }
}

impl Packet {
    /// Creates an empty packet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A packet with these layers and default frame options (padding on, no FCS).
    #[must_use]
    pub fn from_layers(layers: Vec<Layer>) -> Self {
        Self {
            layers,
            ..Self::default()
        }
    }

    /// Builder: disables minimum-frame padding.
    #[must_use]
    pub fn unpadded(mut self) -> Self {
        self.pad_to_min = false;
        self
    }

    /// Builder: sets the FCS mode.
    #[must_use]
    pub fn with_fcs(mut self, fcs: FcsMode) -> Self {
        self.fcs = Some(fcs);
        self
    }

    /// Appends a layer on top of the stack.
    pub fn push(&mut self, layer: Layer) -> &mut Self {
        self.layers.push(layer);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_stack_and_serde() {
        let mut pkt = Packet::new();
        pkt.push(Layer::new("ethernet2").with("dst", FieldValue::Mac([0xff; 6])))
            .push(Layer::new("ipv4").with("src", FieldValue::Ipv4(Ipv4Addr::new(192, 0, 2, 1))));
        assert_eq!(pkt.layers.len(), 2);

        let json = serde_json::to_string(&pkt).unwrap();
        let back: Packet = serde_json::from_str(&json).unwrap();
        assert_eq!(back, pkt);
        assert_eq!(
            serde_json::to_string(&FieldValue::UInt(1)).unwrap(),
            r#"{"uint":1}"#
        );
    }

    #[test]
    fn conversions() {
        assert_eq!(FieldValue::from(7u8), FieldValue::UInt(7));
        assert_eq!(FieldValue::from(-1i32), FieldValue::Int(-1));
        assert_eq!(FieldValue::from([1u8, 2]).as_bytes(), Some(&[1u8, 2][..]));
        assert_eq!(FieldValue::UInt(3).as_u64(), Some(3));
        assert_eq!(FieldValue::Int(3).as_u64(), None);
    }
}
