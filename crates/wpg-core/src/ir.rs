//! Declarative protocol definition IR (TODO §2.1 "プロトコル定義エンジン").
//!
//! A [`ProtocolDef`] is a list of [`FieldDef`]s in wire order (bit-packed, MSB first)
//! plus the rules the builder / parser need:
//!
//! * per-field [`AutoCompute`] rules (lengths, checksums, next-protocol numbers),
//! * a [`Binding`] that maps a field value to the protocol of the next layer
//!   (EtherType, IP protocol, IPv6 Next Header, ...),
//! * an optional [`PseudoHeader`] declaration for upper-layer checksums,
//! * declarative [`Constraint`]s producing warning / error diagnostics.
//!
//! Definitions are written either with the [`protocol!`](crate::protocol) macro
//! (built-in protocols) or in YAML ([`ProtocolDef::from_yaml`], user extensions);
//! both yield the same IR.

use serde::{Deserialize, Serialize};

use crate::constraint::{Constraint, ConstraintRule};
use crate::error::{CoreError, Result};
use crate::packet::FieldValue;

const fn one_u32() -> u32 {
    1
}

fn is_one(v: &u32) -> bool {
    *v == 1
}

fn is_zero(v: &i64) -> bool {
    *v == 0
}

fn is_false(v: &bool) -> bool {
    !*v
}

/// Length of a `bytes` field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Length {
    /// Fixed number of octets.
    Fixed {
        /// Octet count.
        octets: u32,
    },
    /// Length derived from an earlier integer field of the same protocol:
    /// `octets = value * unit + adjust` (negative results clamp to 0).
    ///
    /// Example: IPv4 options are `ihl * 4 - 20` octets.
    FromField {
        /// Name of the (earlier) field holding the length.
        field: String,
        /// Octets per unit of the field value.
        #[serde(default = "one_u32", skip_serializing_if = "is_one")]
        unit: u32,
        /// Octets added after multiplying.
        #[serde(default, skip_serializing_if = "is_zero")]
        adjust: i64,
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
/// Integer-like fields (`uint`, `int`, `enum`, `flags`) may start at any bit offset;
/// all other types must be octet aligned.
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
    /// Variable-length OSI NSAP address (1..=20 octets) in its usual OSI encoding:
    /// a 1-octet length indicator followed by the address octets (CLNP source /
    /// destination, ES-IS NET / NSAP, IS-IS area addresses). The value is the
    /// address alone; the builder writes the length indicator, the parser reads it.
    Nsap,
    /// NUL-terminated octet string (TFTP file names, C strings). The value is the
    /// octets *without* the terminator, which the builder appends; the parser reads
    /// up to and including the first zero octet.
    #[serde(rename = "cstring")]
    CString,
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
    /// Sequence of Type-Length-Value items (TCP / IPv4 options, IS-IS TLVs, ...).
    Tlv(TlvDef),
    /// Repeated items of one shape until the region given by `length` is used up.
    List {
        /// Element shape.
        item: ItemDef,
        /// Size of the whole list region.
        length: Length,
    },
    /// Zero padding so that the header (so far) is a multiple of `align` octets
    /// (`align: 0` = no padding).
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
            | FieldType::CString
            | FieldType::Tlv { .. }
            | FieldType::List { .. }
            | FieldType::Padding { .. }
            | FieldType::Raw => None,
        }
    }

    /// Width in bits of integer-like types (`uint`, `int`, `enum`, `flags`).
    #[must_use]
    pub fn int_bits(&self) -> Option<u8> {
        match self {
            FieldType::UInt { bits }
            | FieldType::Int { bits }
            | FieldType::Enum { bits, .. }
            | FieldType::Flags { bits, .. } => Some(*bits),
            _ => None,
        }
    }

    /// Whether this is an unsigned integer-like type (`uint`, `enum`, `flags`).
    #[must_use]
    pub fn is_unsigned_int(&self) -> bool {
        matches!(
            self,
            FieldType::UInt { .. } | FieldType::Enum { .. } | FieldType::Flags { .. }
        )
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
            FieldType::Tlv(t) => {
                let octets = |bits: u8, what: &str| {
                    if bits.is_multiple_of(8) && (8..=32).contains(&bits) {
                        Ok(())
                    } else {
                        Err(CoreError::InvalidDefinition {
                            protocol: field.to_owned(),
                            reason: format!("tlv {what} width must be 8, 16, 24 or 32 bits"),
                        })
                    }
                };
                octets(t.type_bits, "type")?;
                octets(t.len_bits, "length")
            }
            FieldType::List {
                item: ItemDef::Field { ty },
                ..
            } => ty.validate(field),
            _ => Ok(()),
        }
    }
}

/// Shape of one element of a [`FieldType::List`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ItemDef {
    /// Every element is a single value of this type (e.g. a list of IPv4 addresses).
    Field {
        /// Element type.
        #[serde(rename = "type")]
        ty: Box<FieldType>,
    },
    /// Every element is a group of fields, valued as [`FieldValue::Struct`]
    /// (e.g. SACK blocks `{left, right}`).
    Struct {
        /// Fields of one element, in wire order.
        fields: Vec<FieldDef>,
    },
}

/// One known item type of a [`TlvDef`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TlvVariant {
    /// Type value on the wire.
    pub type_value: u64,
    /// Short machine name (e.g. `mss`).
    pub name: String,
    /// Fields of the value part, in wire order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<FieldDef>,
    /// The item is the type octet(s) alone, without length or value
    /// (TCP / IPv4 EOL and NOP, IPv6 Pad1).
    #[serde(default, skip_serializing_if = "is_false")]
    pub type_only: bool,
    /// Fluent message id for the description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_key: Option<String>,
}

impl TlvVariant {
    /// A variant with value fields.
    #[must_use]
    pub fn new(type_value: u64, name: impl Into<String>, fields: Vec<FieldDef>) -> Self {
        Self {
            type_value,
            name: name.into(),
            fields,
            type_only: false,
            description_key: None,
        }
    }

    /// A type-only variant (no length, no value).
    #[must_use]
    pub fn type_only(type_value: u64, name: impl Into<String>) -> Self {
        Self {
            type_only: true,
            ..Self::new(type_value, name, Vec::new())
        }
    }
}

/// Definition of a Type-Length-Value sequence ([`FieldType::Tlv`]).
///
/// Each item is `type` (`type_bits`), `length` (`len_bits`, in `len_unit`s, counting
/// the type and length octets when `len_includes_header`) and the value. Known types
/// are decoded into their variant's fields; unknown ones are kept as raw value bytes
/// (`unknown_as_raw`) or end the typed decoding (the rest becomes one raw item).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TlvDef {
    /// Width of the type field in bits (8, 16, 24 or 32).
    pub type_bits: u8,
    /// Width of the length field in bits (8, 16, 24 or 32).
    pub len_bits: u8,
    /// Whether the length counts the type and length fields too (TCP options: yes,
    /// IS-IS TLVs: no).
    #[serde(default, skip_serializing_if = "is_false")]
    pub len_includes_header: bool,
    /// Unit of the length value (NDP options: 8 octets).
    #[serde(default, skip_serializing_if = "LengthUnit::is_bytes")]
    pub len_unit: LengthUnit,
    /// Known item types.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<TlvVariant>,
    /// Keep items of unknown type as raw value bytes (otherwise the rest of the
    /// region is decoded as one raw item).
    #[serde(default = "true_fn", skip_serializing_if = "is_true")]
    pub unknown_as_raw: bool,
    /// Size of the whole TLV region.
    pub length: Length,
}

impl TlvDef {
    /// Variant by wire type.
    #[must_use]
    pub fn variant_by_type(&self, ty: u64) -> Option<&TlvVariant> {
        self.variants.iter().find(|v| v.type_value == ty)
    }

    /// Variant by name.
    #[must_use]
    pub fn variant_by_name(&self, name: &str) -> Option<&TlvVariant> {
        self.variants.iter().find(|v| v.name == name)
    }
}

const fn true_fn() -> bool {
    true
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde skip_serializing_if signature
fn is_true(v: &bool) -> bool {
    *v
}

/// Checksum algorithms that can be auto-computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChecksumKind {
    /// RFC 1071 Internet checksum (IPv4 header, ICMPv4, IGMP, ...).
    Internet,
    /// RFC 1071 Internet checksum prefixed with the IPv4 or IPv6 pseudo header of the
    /// nearest lower layer that declares a [`PseudoHeader`] (TCP, UDP, ICMPv6).
    InternetPseudoHeader,
    /// ISO 8473 / IS-IS Fletcher checksum (not implemented yet).
    Fletcher,
    /// CRC-32C, SCTP (not implemented yet).
    Crc32c,
    /// CRC-32, IEEE 802.3 (not implemented yet).
    Crc32,
}

/// Which bytes a checksum covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChecksumCoverage {
    /// Only this layer's header.
    Header,
    /// This layer's header and everything after it.
    HeaderAndPayload,
}

/// Which span an auto length field measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LengthOf {
    /// This layer's header (e.g. IPv4 IHL, TCP data offset).
    Header,
    /// Everything after this layer's header (e.g. IPv6 payload length).
    Payload,
    /// Header plus payload (e.g. IPv4 total length, UDP length).
    Total,
}

/// Unit of an auto length value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LengthUnit {
    /// Octets.
    #[default]
    Bytes,
    /// 16-bit words.
    Words16,
    /// 32-bit words (IPv4 IHL, TCP data offset).
    Words32,
    /// 64-bit words (IPv6 extension header length).
    Words64,
}

impl LengthUnit {
    /// Octets per unit.
    #[must_use]
    pub const fn octets(self) -> u64 {
        match self {
            LengthUnit::Bytes => 1,
            LengthUnit::Words16 => 2,
            LengthUnit::Words32 => 4,
            LengthUnit::Words64 => 8,
        }
    }

    fn is_bytes(&self) -> bool {
        *self == LengthUnit::Bytes
    }
}

/// How an "Auto" field is computed when the user leaves it on Auto (TODO §2.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AutoCompute {
    /// `value = octets(of) / unit + adjust`.
    Length {
        /// Span measured.
        of: LengthOf,
        /// Unit of the value.
        #[serde(default, skip_serializing_if = "LengthUnit::is_bytes")]
        unit: LengthUnit,
        /// Added after the division (e.g. `-1` for IPv6 extension headers).
        #[serde(default, skip_serializing_if = "is_zero")]
        adjust: i64,
    },
    /// Checksum; the field itself is zero while computing.
    Checksum {
        /// Algorithm.
        algorithm: ChecksumKind,
        /// Covered bytes.
        covers: ChecksumCoverage,
        /// Transmit a computed `0` as all-ones (UDP, RFC 768).
        #[serde(default, skip_serializing_if = "is_false")]
        nonzero: bool,
        /// Header fields of this layer left out of the sum (read as zero), e.g. the
        /// OSPFv2 authentication field.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        exclude: Vec<String>,
    },
    /// Identifier of the next (upper) layer, taken from this protocol's [`Binding`].
    NextProtocol,
    /// Number of elements of a `list` / `tlv` field of the same field group
    /// (`num_sources`, `count_ip_addrs`, ...). Unlike the other rules it is also
    /// allowed inside list elements and TLV values. When the counted field is given
    /// as verbatim octets the count is unknown and the default value is used.
    Count {
        /// The counted `list` / `tlv` field.
        field: String,
    },
    /// Per-stream counter (sequence numbers etc.). Not computed by the builder yet:
    /// the default value is used.
    Counter,
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

    /// Builder: sets the Fluent description key.
    #[must_use]
    pub fn with_description(mut self, key: impl Into<String>) -> Self {
        self.description_key = Some(key.into());
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

/// One entry of a [`Binding`] table: a single value or an inclusive value range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingEntry {
    /// Field value on the wire (lower bound of a range).
    pub value: u64,
    /// Inclusive upper bound when the entry covers a range (`value..=max`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<u64>,
    /// Protocol id of the next layer.
    pub protocol: String,
    /// The selected protocol re-decodes *this* layer from its start instead of
    /// following it (Ethernet II with a type/length value < 0x600 is really an
    /// IEEE 802.3 header, `dot3`). Never used by the builder.
    #[serde(default, skip_serializing_if = "is_false")]
    pub reparse: bool,
    /// Parse only: the entry applies only when at least this many octets follow
    /// the header of this layer (an `IGMPv3` query is a longer type 0x11 message
    /// than an `IGMPv2` one). Entries without it match any length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_len: Option<u32>,
}

impl BindingEntry {
    /// A single-value entry.
    #[must_use]
    pub fn new(value: u64, protocol: impl Into<String>) -> Self {
        Self {
            value,
            max: None,
            protocol: protocol.into(),
            reparse: false,
            min_len: None,
        }
    }

    /// A range entry `lo..=hi`.
    #[must_use]
    pub fn range(lo: u64, hi: u64, protocol: impl Into<String>) -> Self {
        Self {
            max: Some(hi),
            ..Self::new(lo, protocol)
        }
    }

    /// Builder: marks the entry as re-decoding the current layer (see
    /// [`BindingEntry::reparse`]).
    #[must_use]
    pub fn reparsing(mut self) -> Self {
        self.reparse = true;
        self
    }

    /// Builder: the entry needs at least `octets` after this layer's header (see
    /// [`BindingEntry::min_len`]).
    #[must_use]
    pub fn with_min_len(mut self, octets: u32) -> Self {
        self.min_len = Some(octets);
        self
    }

    /// Whether `v` selects this entry.
    #[must_use]
    pub fn matches(&self, v: u64) -> bool {
        match self.max {
            None => v == self.value,
            Some(hi) => (self.value..=hi).contains(&v),
        }
    }

    fn fits(&self, remaining: usize) -> bool {
        self.min_len
            .is_none_or(|n| remaining >= usize::try_from(n).unwrap_or(usize::MAX))
    }
}

/// Selects the next protocol from the leading bits of the payload instead of a
/// header field (MPLS bottom of stack and GTP-U G-PDU: first nibble 4 → IPv4,
/// 6 → IPv6). See [`Binding::peek`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Peek {
    /// Selector value of [`Binding::field`] under which the payload is peeked; the
    /// builder writes it when the next layer is one of the peeked protocols.
    pub on: u64,
    /// Number of leading payload bits read (1..=64, MSB first).
    pub bits: u8,
    /// Peeked value → protocol (exact entries first, then ranges).
    pub table: Vec<BindingEntry>,
    /// Protocol decoded when no entry matches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

impl Peek {
    /// Peek of the first `bits` payload bits when the selector is `on`.
    #[must_use]
    pub fn new(on: u64, bits: u8, table: Vec<BindingEntry>) -> Self {
        Self {
            on,
            bits,
            table,
            default: None,
        }
    }
}

/// Exact entries matching `v` (and the remaining length) first, then ranges.
fn pick(table: &[BindingEntry], v: u64, remaining: usize) -> Option<&BindingEntry> {
    table
        .iter()
        .find(|e| e.max.is_none() && e.value == v && e.fits(remaining))
        .or_else(|| table.iter().find(|e| e.matches(v) && e.fits(remaining)))
}

/// "Which protocol comes next": a selector field plus a value → protocol table.
///
/// The builder writes the selector (when it is Auto) from the protocol of the next
/// layer; the parser reads it to decide how to decode the rest. Exact entries win
/// over ranges; `default` applies when nothing matches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    /// Selector field (e.g. `ethertype`).
    pub field: String,
    /// Known values. The first entry for a protocol is used when building.
    pub table: Vec<BindingEntry>,
    /// Protocol decoded when no entry matches (e.g. 802.3 → LLC).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    /// Parse only: a second selector field looked up in `table` when `field`
    /// matches no entry (UDP / TCP: the source port, so that replies from a
    /// well-known port decode too). The builder only ever writes `field`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt_field: Option<String>,
    /// Payload peek used when `field` has the value [`Peek::on`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peek: Option<Peek>,
}

impl Binding {
    /// A binding without default.
    #[must_use]
    pub fn new(field: impl Into<String>, table: Vec<BindingEntry>) -> Self {
        Self {
            field: field.into(),
            table,
            default: None,
            alt_field: None,
            peek: None,
        }
    }

    /// The entry selected by `value` (exact entries first, then ranges).
    #[must_use]
    pub fn entry_for(&self, value: u64) -> Option<&BindingEntry> {
        self.table
            .iter()
            .find(|e| e.max.is_none() && e.value == value)
            .or_else(|| self.table.iter().find(|e| e.matches(value)))
    }

    /// Protocol id selected by `value` (falling back to [`Binding::default`]).
    #[must_use]
    pub fn protocol_for(&self, value: u64) -> Option<&str> {
        self.entry_for(value)
            .map(|e| e.protocol.as_str())
            .or(self.default.as_deref())
    }

    /// Selector value announcing `protocol` (re-parse entries are ignored; a
    /// protocol reached through the [`Peek`] yields [`Peek::on`]).
    #[must_use]
    pub fn value_for(&self, protocol: &str) -> Option<u64> {
        self.table
            .iter()
            .find(|e| e.protocol == protocol && !e.reparse)
            .map(|e| e.value)
            .or_else(|| {
                let p = self.peek.as_ref()?;
                let known = p.table.iter().any(|e| e.protocol == protocol && !e.reparse)
                    || p.default.as_deref() == Some(protocol);
                known.then_some(p.on)
            })
    }

    /// The protocol a parser decodes next and whether it re-decodes the current
    /// layer. `field` reads a header field of the current layer; `payload` is
    /// what follows the header (bounded by the enclosing PDU).
    ///
    /// Order: the [`Peek`] when the selector equals [`Peek::on`]; the table entry
    /// for the selector; the table entry for [`Binding::alt_field`]; the default.
    #[must_use]
    pub fn select<'a>(
        &'a self,
        field: impl Fn(&str) -> Option<u64>,
        payload: &[u8],
    ) -> Option<(&'a str, bool)> {
        let v = field(&self.field)?;
        let n = payload.len();
        if let Some(p) = self.peek.as_ref().filter(|p| p.on == v) {
            let bits = u32::from(p.bits);
            return match crate::bits::get_bits(payload, 0, bits) {
                Some(x) => pick(&p.table, x, n)
                    .map(|e| (e.protocol.as_str(), e.reparse))
                    .or_else(|| p.default.as_deref().map(|d| (d, false))),
                None => None,
            };
        }
        pick(&self.table, v, n)
            .or_else(|| {
                let alt = field(self.alt_field.as_deref()?)?;
                pick(&self.table, alt, n)
            })
            .map(|e| (e.protocol.as_str(), e.reparse))
            .or_else(|| self.default.as_deref().map(|d| (d, false)))
    }
}

/// Declares that this (network) layer supplies a pseudo header for upper-layer
/// [`ChecksumKind::InternetPseudoHeader`] checksums. The address type (IPv4 / IPv6
/// pseudo header) follows the field type; the protocol number comes from the
/// [`Binding`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PseudoHeader {
    /// Source address field.
    pub src: String,
    /// Destination address field.
    pub dst: String,
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
    /// Next-layer selector.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<Binding>,
    /// Pseudo header supplied to upper-layer checksums.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pseudo_header: Option<PseudoHeader>,
    /// Declarative checks run after building.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<Constraint>,
    /// Minimum frame length in octets (without FCS) when this protocol is the
    /// outermost layer; [`Packet::pad_to_min`](crate::Packet::pad_to_min) pads short
    /// frames up to it (Ethernet: 60).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_frame_len: Option<u32>,
    /// When set, `fields` are generator parameters rather than wire fields and the
    /// builder produces the bytes with this built-in generator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synth: Option<Synthesizer>,
}

/// Built-in content generators for [`ProtocolDef::synth`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Synthesizer {
    /// Payload generator (see [`crate::synth::payload`]): parameters `mode`, `data`,
    /// `length`, `seed`, `counter_offset`, `counter_width`, `counter_value`.
    Payload,
}

/// Protocol id of the built-in raw payload protocol, used by the parser for
/// undecodable remainders.
pub const RAW_PROTOCOL_ID: &str = "raw";

/// The built-in `raw` payload protocol: a single `data` field consuming the rest.
#[must_use]
pub fn raw_protocol() -> ProtocolDef {
    let mut def = ProtocolDef::new(RAW_PROTOCOL_ID, "Raw payload", ProtocolLayer::Payload);
    def.fields.push(
        FieldDef::new("data", FieldType::Bytes { len: Length::ToEnd })
            .with_default(FieldValue::Bytes(Vec::new())),
    );
    def
}

fn def_err(protocol: &str, reason: impl Into<String>) -> CoreError {
    CoreError::InvalidDefinition {
        protocol: protocol.to_owned(),
        reason: reason.into(),
    }
}

/// Validates a field group (a protocol header, a TLV value or a list element) and
/// returns the total fixed width in bits. `top` allows auto rules.
fn validate_fields(proto: &str, fields: &[FieldDef], top: bool) -> Result<u64> {
    let mut bit_pos: u64 = 0;
    for (i, field) in fields.iter().enumerate() {
        field.ty.validate(&field.name)?;
        if fields[..i].iter().any(|f| f.name == field.name) {
            return Err(CoreError::DuplicateField {
                protocol: proto.to_owned(),
                field: field.name.clone(),
            });
        }
        if field.ty.int_bits().is_none() && !bit_pos.is_multiple_of(8) {
            return Err(def_err(
                proto,
                format!("field `{}` must start on an octet boundary", field.name),
            ));
        }
        match &field.auto {
            Some(AutoCompute::Count { field: target }) => {
                let counted = fields.iter().any(|f| {
                    &f.name == target && matches!(f.ty, FieldType::List { .. } | FieldType::Tlv(_))
                });
                if !counted || !field.ty.is_unsigned_int() {
                    return Err(def_err(
                        proto,
                        format!(
                            "count field `{}` must be unsigned and count a list / tlv field `{target}` of the same group",
                            field.name
                        ),
                    ));
                }
            }
            Some(_) if !top => {
                return Err(def_err(
                    proto,
                    format!(
                        "field `{}`: auto rules other than count are only supported on header fields",
                        field.name
                    ),
                ));
            }
            _ => {}
        }
        let from_field = match &field.ty {
            FieldType::Bytes {
                len: Length::FromField { field: src, .. },
            }
            | FieldType::List {
                length: Length::FromField { field: src, .. },
                ..
            } => Some(src),
            FieldType::Tlv(t) => match &t.length {
                Length::FromField { field: src, .. } => Some(src),
                _ => None,
            },
            _ => None,
        };
        if let Some(src) = from_field {
            let ok = fields[..i]
                .iter()
                .any(|f| &f.name == src && f.ty.is_unsigned_int());
            if !ok {
                return Err(def_err(
                    proto,
                    format!(
                        "length of `{}` refers to `{src}`, which must be an earlier unsigned field",
                        field.name
                    ),
                ));
            }
        }
        match &field.ty {
            FieldType::Tlv(t) => {
                for (k, v) in t.variants.iter().enumerate() {
                    if t.variants[..k]
                        .iter()
                        .any(|o| o.name == v.name || o.type_value == v.type_value)
                    {
                        return Err(def_err(
                            proto,
                            format!("tlv `{}`: duplicate variant `{}`", field.name, v.name),
                        ));
                    }
                    if v.type_value > crate::bits::mask(u32::from(t.type_bits)) {
                        return Err(def_err(
                            proto,
                            format!("tlv variant `{}` does not fit the type width", v.name),
                        ));
                    }
                    if v.type_only && !v.fields.is_empty() {
                        return Err(def_err(
                            proto,
                            format!("type-only tlv variant `{}` cannot have fields", v.name),
                        ));
                    }
                    let bits = validate_fields(proto, &v.fields, false)?;
                    if !bits.is_multiple_of(8) {
                        return Err(def_err(
                            proto,
                            format!("tlv variant `{}` is not a whole number of octets", v.name),
                        ));
                    }
                }
            }
            FieldType::List {
                item: ItemDef::Struct { fields: inner },
                ..
            } => {
                let bits = validate_fields(proto, inner, false)?;
                if !bits.is_multiple_of(8) {
                    return Err(def_err(
                        proto,
                        format!(
                            "list `{}` element is not a whole number of octets",
                            field.name
                        ),
                    ));
                }
            }
            FieldType::List {
                item: ItemDef::Field { ty },
                ..
            } if ty.fixed_bits().is_none_or(|b| b % 8 != 0) => {
                return Err(def_err(
                    proto,
                    format!("list `{}` elements must be fixed whole octets", field.name),
                ));
            }
            _ => {}
        }
        bit_pos += u64::from(field.ty.fixed_bits().unwrap_or(0));
    }
    Ok(bit_pos)
}

impl ProtocolDef {
    /// Creates a definition without fields, binding, pseudo header or constraints.
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>, layer: ProtocolLayer) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            layer,
            fields: Vec::new(),
            next: None,
            pseudo_header: None,
            constraints: Vec::new(),
            min_frame_len: None,
            synth: None,
        }
    }

    /// Looks up a field by name.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&FieldDef> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Index of a field by name.
    #[must_use]
    pub fn field_index(&self, name: &str) -> Option<usize> {
        self.fields.iter().position(|f| f.name == name)
    }

    /// Total header width in bits if every field is fixed-size.
    #[must_use]
    pub fn fixed_header_bits(&self) -> Option<u32> {
        self.fields.iter().map(|f| f.ty.fixed_bits()).sum()
    }

    /// Checks field widths, name uniqueness, octet alignment and that every
    /// cross-reference (lengths, binding, pseudo header, constraints) resolves.
    ///
    /// # Errors
    /// Returns the first inconsistency found.
    pub fn validate(&self) -> Result<()> {
        let mut bit_pos = validate_fields(&self.id, &self.fields, true)?;
        if self.synth.is_some() {
            bit_pos = 0; // parameters, not wire fields
        }
        for field in &self.fields {
            if let Some(auto) = &field.auto {
                self.validate_auto(field, auto)?;
            }
        }
        if !bit_pos.is_multiple_of(8) {
            return Err(def_err(&self.id, "header is not a whole number of octets"));
        }
        if let Some(b) = &self.next {
            for name in std::iter::once(&b.field).chain(&b.alt_field) {
                match self.field(name) {
                    Some(f) if f.ty.is_unsigned_int() => {}
                    _ => {
                        return Err(def_err(
                            &self.id,
                            format!("binding field `{name}` must be an unsigned field"),
                        ))
                    }
                }
            }
            if b.peek.as_ref().is_some_and(|p| !(1..=64).contains(&p.bits)) {
                return Err(def_err(&self.id, "binding peek width must be 1..=64 bits"));
            }
        }
        if let Some(ph) = &self.pseudo_header {
            let src = self.field(&ph.src).map(|f| &f.ty);
            let dst = self.field(&ph.dst).map(|f| &f.ty);
            let ok = matches!(
                (src, dst),
                (Some(FieldType::Ipv4), Some(FieldType::Ipv4))
                    | (Some(FieldType::Ipv6), Some(FieldType::Ipv6))
            );
            if !ok || self.next.is_none() {
                return Err(def_err(
                    &self.id,
                    "pseudo header needs two IPv4 or two IPv6 address fields and a binding",
                ));
            }
        }
        for c in &self.constraints {
            let field = match &c.rule {
                ConstraintRule::FieldInRange { field, .. }
                | ConstraintRule::FieldEquals { field, .. }
                | ConstraintRule::FieldNotEquals { field, .. } => Some(field),
                ConstraintRule::MinLength { .. } | ConstraintRule::MaxLength { .. } => None,
            };
            if let Some(field) = field {
                if self.field(field).and_then(|f| f.ty.int_bits()).is_none() {
                    return Err(def_err(
                        &self.id,
                        format!("constraint refers to `{field}`, which is not an integer field"),
                    ));
                }
            }
        }
        Ok(())
    }

    fn validate_auto(&self, field: &FieldDef, auto: &AutoCompute) -> Result<()> {
        let bits = match field.ty.int_bits() {
            Some(b) if field.ty.is_unsigned_int() => b,
            _ => {
                return Err(def_err(
                    &self.id,
                    format!("auto field `{}` must be an unsigned integer", field.name),
                ))
            }
        };
        match auto {
            AutoCompute::Checksum {
                algorithm: ChecksumKind::Internet | ChecksumKind::InternetPseudoHeader,
                ..
            } if bits != 16 => Err(def_err(
                &self.id,
                format!("Internet checksum field `{}` must be 16 bits", field.name),
            )),
            AutoCompute::NextProtocol
                if self
                    .next
                    .as_ref()
                    .and_then(|b| self.field(&b.field))
                    .and_then(|f| f.ty.int_bits())
                    != Some(bits) =>
            {
                Err(def_err(
                    &self.id,
                    format!(
                        "next-protocol field `{}` needs a binding whose selector has the same width",
                        field.name
                    ),
                ))
            }
            AutoCompute::Checksum { exclude, .. }
                if exclude.iter().any(|n| self.field(n).is_none()) =>
            {
                Err(def_err(
                    &self.id,
                    format!(
                        "checksum field `{}` excludes a field that does not exist",
                        field.name
                    ),
                ))
            }
            AutoCompute::Checksum {
                algorithm: ChecksumKind::Fletcher,
                ..
            } if bits != 16 => Err(def_err(
                &self.id,
                format!("Fletcher checksum field `{}` must be 16 bits", field.name),
            )),
            AutoCompute::Checksum {
                algorithm: ChecksumKind::Crc32 | ChecksumKind::Crc32c,
                ..
            } if bits != 32 => Err(def_err(
                &self.id,
                format!("CRC-32 field `{}` must be 32 bits", field.name),
            )),
            _ => Ok(()),
        }
    }

    /// Parses a definition from YAML and validates it.
    ///
    /// The shape is the same as the JSON served by `/api/protocols`; enum values
    /// such as defaults are single-key maps (`default: { uint: 0x0800 }`).
    ///
    /// # Errors
    /// [`CoreError::Yaml`] on syntax / shape errors, or the validation error.
    #[cfg(feature = "yaml")]
    pub fn from_yaml(src: &str) -> Result<Self> {
        let de = serde_yaml::Deserializer::from_str(src);
        let def: Self = serde_yaml::with::singleton_map_recursive::deserialize(de)
            .map_err(|e| CoreError::Yaml(e.to_string()))?;
        def.validate()?;
        Ok(def)
    }

    /// Serialises the definition to YAML.
    ///
    /// # Errors
    /// [`CoreError::Yaml`] if serialisation fails.
    #[cfg(feature = "yaml")]
    pub fn to_yaml(&self) -> Result<String> {
        let mut out = Vec::new();
        let mut ser = serde_yaml::Serializer::new(&mut out);
        serde_yaml::with::singleton_map_recursive::serialize(self, &mut ser)
            .map_err(|e| CoreError::Yaml(e.to_string()))?;
        String::from_utf8(out).map_err(|e| CoreError::Yaml(e.to_string()))
    }
}

/// Resolves protocol ids to definitions (implemented by `wpg_protocols::Catalog`).
pub trait Resolve {
    /// Looks up a protocol definition by id.
    fn protocol(&self, id: &str) -> Option<&ProtocolDef>;
}

impl Resolve for [ProtocolDef] {
    fn protocol(&self, id: &str) -> Option<&ProtocolDef> {
        self.iter().find(|p| p.id == id)
    }
}

impl Resolve for Vec<ProtocolDef> {
    fn protocol(&self, id: &str) -> Option<&ProtocolDef> {
        self.as_slice().protocol(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ProtocolDef {
        let mut p = ProtocolDef::new("test", "Test", ProtocolLayer::L2);
        p.fields = vec![
            FieldDef::new("dst", FieldType::Mac),
            FieldDef::new("kind", FieldType::UInt { bits: 16 })
                .with_auto(AutoCompute::NextProtocol),
        ];
        p.next = Some(Binding::new("kind", vec![BindingEntry::new(1, "raw")]));
        p
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
    fn validation_catches_alignment_and_references() {
        let mut p = sample();
        p.fields
            .insert(0, FieldDef::new("nib", FieldType::UInt { bits: 4 }));
        assert!(matches!(
            p.validate(),
            Err(CoreError::InvalidDefinition { .. })
        ));

        let mut p = sample();
        p.fields.push(FieldDef::new(
            "opts",
            FieldType::Bytes {
                len: Length::FromField {
                    field: "nope".into(),
                    unit: 1,
                    adjust: 0,
                },
            },
        ));
        assert!(matches!(
            p.validate(),
            Err(CoreError::InvalidDefinition { .. })
        ));

        let mut p = sample();
        p.next = None;
        assert!(p.validate().is_err(), "next_protocol without binding");
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

    #[test]
    fn binding_lookup() {
        let b = sample().next.unwrap();
        assert_eq!(b.protocol_for(1), Some("raw"));
        assert_eq!(b.protocol_for(2), None);
        assert_eq!(b.value_for("raw"), Some(1));
    }

    #[test]
    fn raw_protocol_is_valid() {
        raw_protocol().validate().unwrap();
    }
}
