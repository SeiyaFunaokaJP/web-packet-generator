//! Packet builder: [`Packet`] + protocol definitions → wire bytes (TODO §2.1).
//!
//! Two passes:
//!
//! 1. every layer header is serialised on its own, with placeholders (zero) for
//!    Auto length / checksum fields and the default for Auto next-protocol fields;
//! 2. the headers are concatenated and the Auto fields are filled in: lengths
//!    (innermost first), next-protocol selectors from the [`Binding`] of each layer,
//!    then checksums innermost first so that outer checksums (e.g. an outer IP
//!    header covering a tunnelled packet) see the final inner bytes.
//!
//! A field present in [`Layer::fields`] is *Manual* and used verbatim, even if the
//! result is inconsistent; the builder then emits a warning diagnostic instead of
//! "fixing" it, which is how malformed packets are produced on purpose.
//!
//! [`Binding`]: crate::ir::Binding

use std::net::{Ipv4Addr, Ipv6Addr};

use serde::{Deserialize, Serialize};

use crate::bits::{mask, put_bits, BitWriter};
use crate::checksum::{crc32, crc32c, fletcher_checksum, InternetSum};
use crate::constraint::{evaluate, span_name, Spans};
use crate::diag::{keys, DiagLevel, Diagnostic};
use crate::error::{CoreError, Result};
use crate::ir::{
    raw_protocol, AutoCompute, ChecksumCoverage, ChecksumKind, FieldDef, FieldType, ItemDef,
    Length, ProtocolDef, Resolve, Synthesizer, TlvDef, RAW_PROTOCOL_ID,
};
use crate::packet::{FcsMode, FieldValue, Layer, Packet, TlvItem};

mod prebuilt;
pub use prebuilt::{PatchTarget, Prebuilt};

/// Position and final value of one field in the built bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldLayout {
    /// Field name.
    pub name: String,
    /// Offset in bits from the start of the packet.
    pub bit_offset: usize,
    /// Width in bits.
    pub bit_len: usize,
    /// Final value (after Auto computation).
    pub value: FieldValue,
    /// Whether the value was computed by the builder (Auto) rather than taken from
    /// the layer (Manual) or the definition default.
    pub auto: bool,
    /// Sub-fields of composite values (TLV items with their `type` / `length` /
    /// value fields, list elements), with absolute offsets like the parent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<FieldLayout>,
}

impl FieldLayout {
    fn leaf(name: impl Into<String>, bit_offset: usize, bit_len: usize, value: FieldValue) -> Self {
        Self {
            name: name.into(),
            bit_offset,
            bit_len,
            value,
            auto: false,
            children: Vec::new(),
        }
    }
}

/// Moves layouts (and their children) by `by` bits.
fn shift(mut v: Vec<FieldLayout>, by: usize) -> Vec<FieldLayout> {
    for f in &mut v {
        f.bit_offset += by;
        f.children = shift(std::mem::take(&mut f.children), by);
    }
    v
}

/// What a frame trailer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrailerKind {
    /// Zero padding up to the minimum frame length ([`Packet::pad_to_min`]).
    Padding,
    /// Frame check sequence ([`Packet::fcs`]).
    Fcs,
}

/// Octets appended after the last layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrailerLayout {
    /// Kind.
    pub kind: TrailerKind,
    /// Offset in octets from the start of the packet.
    pub offset: usize,
    /// Length in octets.
    pub len: usize,
}

/// Position of one layer in the built bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerLayout {
    /// Protocol id.
    pub protocol: String,
    /// Offset in octets from the start of the packet.
    pub offset: usize,
    /// Header length in octets.
    pub header_len: usize,
    /// Header plus payload length in octets (as present in the output bytes).
    pub len: usize,
    /// Whether the output was cut inside this layer (see [`Layer::truncate`]); fields
    /// that do not fit completely are omitted from [`LayerLayout::fields`].
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
    /// Fields in wire order.
    pub fields: Vec<FieldLayout>,
}

/// Output of [`Packet::build`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Built {
    /// Wire bytes (without FCS).
    pub bytes: Vec<u8>,
    /// Layer / field positions for hex-dump highlighting and the field tree.
    pub layout: Vec<LayerLayout>,
    /// Warnings and errors (constraints, inconsistent Manual values, ...).
    pub diagnostics: Vec<Diagnostic>,
    /// Padding / FCS appended after the layers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trailers: Vec<TrailerLayout>,
}

impl Built {
    /// Whether any diagnostic has [`DiagLevel::Error`].
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.level == DiagLevel::Error)
    }
}

impl Packet {
    /// Serialises the packet to wire bytes, computing Auto fields.
    ///
    /// # Errors
    /// [`CoreError::UnknownProtocol`] / [`CoreError::UnknownField`] for references
    /// that do not resolve, [`CoreError::InvalidValue`] when a Manual or default
    /// value does not match / fit its field type.
    /// Inconsistent-but-representable packets are *not* errors; see
    /// [`Built::diagnostics`].
    pub fn build(&self, catalog: &dyn Resolve) -> Result<Built> {
        build(self, catalog)
    }
}

/// Resolves a protocol id, falling back to the built-in `raw` protocol.
pub(crate) fn resolve<'a>(
    catalog: &'a dyn Resolve,
    raw: &'a ProtocolDef,
    id: &str,
) -> Option<&'a ProtocolDef> {
    catalog
        .protocol(id)
        .or_else(|| (id == RAW_PROTOCOL_ID).then_some(raw))
}

/// One encoded field.
#[derive(Debug)]
struct Slot {
    /// Index into `ProtocolDef::fields`.
    def: usize,
    /// Bit offset within the layer header.
    bit_off: usize,
    bits: usize,
    value: FieldValue,
    manual: bool,
    /// Auto rule to apply in pass 2 (`None` when Manual or not auto).
    pending: Option<AutoCompute>,
    computed: bool,
    /// Sub-field layouts (bit offsets relative to the header start).
    children: Vec<FieldLayout>,
}

/// One encoded layer header.
#[derive(Debug)]
struct Enc<'a> {
    def: &'a ProtocolDef,
    header: Vec<u8>,
    slots: Vec<Slot>,
    offset: usize,
}

impl Enc<'_> {
    fn value(&self, name: &str) -> Option<&FieldValue> {
        self.slots
            .iter()
            .find(|s| self.def.fields[s.def].name == name)
            .map(|s| &s.value)
    }

    fn header_len(&self) -> usize {
        self.header.len()
    }
}

fn invalid(field: &FieldDef, reason: impl Into<String>) -> CoreError {
    CoreError::InvalidValue {
        field: field.name.clone(),
        reason: reason.into(),
    }
}

/// The value used for a field that has neither a Manual value nor a default.
pub(crate) fn zero_value(ty: &FieldType) -> FieldValue {
    match ty {
        FieldType::UInt { .. } | FieldType::Enum { .. } | FieldType::Flags { .. } => {
            FieldValue::UInt(0)
        }
        FieldType::Int { .. } => FieldValue::Int(0),
        FieldType::Bytes {
            len: Length::Fixed { octets },
        } => FieldValue::Bytes(vec![0; *octets as usize]),
        FieldType::Mac => FieldValue::Mac([0; 6]),
        FieldType::Ipv4 => FieldValue::Ipv4(Ipv4Addr::UNSPECIFIED),
        FieldType::Ipv6 => FieldValue::Ipv6(Ipv6Addr::UNSPECIFIED),
        FieldType::Nsap => FieldValue::Nsap(Vec::new()),
        FieldType::CString => FieldValue::Bytes(Vec::new()),
        FieldType::Tlv(_) => FieldValue::Tlv(Vec::new()),
        FieldType::List { .. } => FieldValue::List(Vec::new()),
        FieldType::Bytes { .. } | FieldType::Padding { .. } | FieldType::Raw => {
            FieldValue::Bytes(Vec::new())
        }
    }
}

/// Number of elements of `value` for an [`AutoCompute::Count`] rule; `None` when
/// the counted field holds verbatim octets that are not whole fixed-size elements.
pub(crate) fn element_count(ty: &FieldType, value: &FieldValue) -> Option<u64> {
    let n = match value {
        FieldValue::List(v) => v.len(),
        FieldValue::Tlv(v) => v.len(),
        FieldValue::Bytes(b) => match ty {
            FieldType::List {
                item: ItemDef::Field { ty },
                ..
            } => {
                let size = usize::try_from(ty.fixed_bits()? / 8).ok()?;
                if size == 0 || !b.len().is_multiple_of(size) {
                    return None;
                }
                b.len() / size
            }
            _ => return None,
        },
        _ => return None,
    };
    u64::try_from(n).ok()
}

/// Value of a [`AutoCompute::Count`] field of a group whose (Manual or default)
/// values `get` returns; `None` when the count cannot be known.
fn count_for<'v>(
    fields: &'v [FieldDef],
    target: &str,
    get: impl Fn(&'v FieldDef) -> Option<&'v FieldValue>,
) -> Option<u64> {
    let fd = fields.iter().find(|f| f.name == target)?;
    let zero;
    let value = match get(fd) {
        Some(v) => v,
        None => {
            zero = zero_value(&fd.ty);
            &zero
        }
    };
    element_count(&fd.ty, value)
}

/// Clamps a computed count to the field width (error diagnostic on overflow).
fn fit_count(fd: &FieldDef, count: u64, diags: &mut Vec<Diagnostic>) -> u64 {
    let bits = u32::from(fd.ty.int_bits().unwrap_or(64));
    if count > mask(bits) {
        diags.push(
            Diagnostic::new(
                DiagLevel::Error,
                keys::AUTO_OVERFLOW,
                format!(
                    "computed value {count} does not fit in `{}` ({bits} bits)",
                    fd.name
                ),
            )
            .on_field(fd.name.clone())
            .arg("value", count)
            .arg("bits", bits),
        );
    }
    count & mask(bits)
}

/// Warning for a Manual count that differs from the number of elements.
fn count_mismatch(name: &str, actual: u64, expected: u64) -> Diagnostic {
    Diagnostic::new(
        DiagLevel::Warning,
        keys::COUNT_MISMATCH,
        format!("`{name}` is {actual} but the list has {expected} elements"),
    )
    .on_field(name.to_owned())
    .arg("value", actual)
    .arg("expected", expected)
}

fn type_name(v: &FieldValue) -> &'static str {
    match v {
        FieldValue::UInt(_) => "uint",
        FieldValue::Int(_) => "int",
        FieldValue::Bytes(_) => "bytes",
        FieldValue::Mac(_) => "mac",
        FieldValue::Ipv4(_) => "ipv4",
        FieldValue::Ipv6(_) => "ipv6",
        FieldValue::Nsap(_) => "nsap",
        FieldValue::List(_) => "list",
        FieldValue::Struct(_) => "struct",
        FieldValue::Tlv(_) => "tlv",
    }
}

/// Writes one value into `w`; returns the layouts of its sub-fields (offsets in
/// bits from the start of `w`).
pub(crate) fn encode_value(
    w: &mut BitWriter,
    fd: &FieldDef,
    value: &FieldValue,
    diags: &mut Vec<Diagnostic>,
) -> Result<Vec<FieldLayout>> {
    let mismatch = || {
        invalid(
            fd,
            format!(
                "a `{}` value does not fit this field type",
                type_name(value)
            ),
        )
    };
    match (&fd.ty, value) {
        (
            FieldType::UInt { bits } | FieldType::Enum { bits, .. } | FieldType::Flags { bits, .. },
            FieldValue::UInt(v),
        ) => {
            if *v > mask(u32::from(*bits)) {
                return Err(invalid(fd, format!("{v} does not fit in {bits} bits")));
            }
            w.push_bits(u32::from(*bits), *v);
        }
        (FieldType::Int { bits }, FieldValue::Int(v)) => {
            let half = 1i128 << (bits - 1);
            if !(-half..half).contains(&i128::from(*v)) {
                return Err(invalid(
                    fd,
                    format!("{v} does not fit in {bits} signed bits"),
                ));
            }
            #[allow(clippy::cast_sign_loss)] // two's complement reinterpretation
            let raw = *v as u64;
            w.push_bits(u32::from(*bits), raw);
        }
        (FieldType::Bytes { len }, FieldValue::Bytes(b)) => {
            if let Length::Fixed { octets } = len {
                if b.len() != *octets as usize {
                    return Err(invalid(
                        fd,
                        format!("expected {octets} octets, got {}", b.len()),
                    ));
                }
            }
            w.push_bytes(b);
        }
        // Composite fields accept verbatim octets too (malformed content on purpose).
        (
            FieldType::Raw | FieldType::Padding { .. } | FieldType::Tlv(_) | FieldType::List { .. },
            FieldValue::Bytes(b),
        )
        | (FieldType::Nsap, FieldValue::Nsap(b) | FieldValue::Bytes(b)) => w.push_bytes(b),
        (FieldType::CString, FieldValue::Bytes(b)) => {
            if b.contains(&0) {
                return Err(invalid(
                    fd,
                    "a NUL-terminated string cannot contain a zero octet",
                ));
            }
            w.push_bytes(b);
            w.push_bytes(&[0]);
        }
        (FieldType::Mac, FieldValue::Mac(m)) => w.push_bytes(m),
        (FieldType::Ipv4, FieldValue::Ipv4(a)) => w.push_bytes(&a.octets()),
        (FieldType::Ipv6, FieldValue::Ipv6(a)) => w.push_bytes(&a.octets()),
        (FieldType::Tlv(t), FieldValue::Tlv(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for (k, item) in items.iter().enumerate() {
                let (bytes, mut lay) = encode_tlv_item(fd, t, k, item, diags)?;
                let at = w.bits;
                lay.bit_offset += at;
                lay.children = shift(lay.children, at);
                w.push_bytes(&bytes);
                out.push(lay);
            }
            return Ok(out);
        }
        (FieldType::List { item, .. }, FieldValue::List(elems)) => {
            let mut out = Vec::with_capacity(elems.len());
            for (k, e) in elems.iter().enumerate() {
                let before = w.bits;
                let children = match (item, e) {
                    (ItemDef::Field { ty }, v) => {
                        let efd = FieldDef::new(format!("{}[{k}]", fd.name), (**ty).clone());
                        encode_value(w, &efd, v, diags)?
                    }
                    (ItemDef::Struct { fields }, FieldValue::Struct(m)) => {
                        encode_group(w, &fd.name, fields, m, diags)?
                    }
                    _ => return Err(mismatch()),
                };
                out.push(FieldLayout {
                    children,
                    ..FieldLayout::leaf(format!("[{k}]"), before, w.bits - before, e.clone())
                });
            }
            return Ok(out);
        }
        _ => return Err(mismatch()),
    }
    Ok(Vec::new())
}

/// Encodes a group of fields (TLV value, list element) without auto rules; unset
/// fields use their default. Layout offsets are relative to the start of `w`.
fn encode_group(
    w: &mut BitWriter,
    owner: &str,
    fields: &[FieldDef],
    values: &std::collections::BTreeMap<String, FieldValue>,
    diags: &mut Vec<Diagnostic>,
) -> Result<Vec<FieldLayout>> {
    if let Some(k) = values
        .keys()
        .find(|k| !fields.iter().any(|f| &f.name == *k))
    {
        return Err(CoreError::UnknownField {
            protocol: owner.to_owned(),
            field: k.clone(),
        });
    }
    let start = w.bits;
    let mut out = Vec::with_capacity(fields.len());
    for fd in fields {
        let count = match &fd.auto {
            Some(AutoCompute::Count { field }) => count_for(fields, field, |f| {
                values.get(&f.name).or(f.default.as_ref())
            }),
            _ => None,
        };
        if let (Some(v), Some(n)) = (values.get(&fd.name).and_then(FieldValue::as_u64), count) {
            if v != n {
                diags.push(count_mismatch(&format!("{owner}.{}", fd.name), v, n));
            }
        }
        let (value, computed) = match (values.get(&fd.name), &fd.ty) {
            (Some(v), _) => (v.clone(), false),
            (None, _) if count.is_some() => (
                FieldValue::UInt(fit_count(fd, count.unwrap_or(0), diags)),
                true,
            ),
            (None, FieldType::Padding { align }) => {
                let align = *align as usize;
                let used = (w.bits - start) / 8;
                let pad = if align == 0 {
                    0
                } else {
                    (align - used % align) % align
                };
                (FieldValue::Bytes(vec![0; pad]), true)
            }
            (None, ty) => (fd.default.clone().unwrap_or_else(|| zero_value(ty)), false),
        };
        let before = w.bits;
        let children = encode_value(w, fd, &value, diags)?;
        out.push(FieldLayout {
            auto: computed,
            children,
            ..FieldLayout::leaf(fd.name.clone(), before, w.bits - before, value)
        });
    }
    Ok(out)
}

/// Encodes one TLV item; returns its octets and its layout (offsets relative to
/// the item start).
pub(crate) fn encode_tlv_item(
    fd: &FieldDef,
    t: &TlvDef,
    index: usize,
    item: &TlvItem,
    diags: &mut Vec<Diagnostic>,
) -> Result<(Vec<u8>, FieldLayout)> {
    if let Some(raw) = &item.raw {
        let lay = FieldLayout::leaf("raw", 0, raw.len() * 8, FieldValue::Bytes(raw.clone()));
        return Ok((raw.clone(), lay));
    }
    let variant = match &item.name {
        Some(n) => Some(
            t.variant_by_name(n)
                .ok_or_else(|| invalid(fd, format!("unknown tlv item `{n}`")))?,
        ),
        None => item.ty.and_then(|ty| t.variant_by_type(ty)),
    };
    let ty = item
        .ty
        .or(variant.map(|v| v.type_value))
        .ok_or_else(|| invalid(fd, "a tlv item needs a `type` or a `name`"))?;
    let type_bits = u32::from(t.type_bits);
    let len_bits = u32::from(t.len_bits);
    if ty > mask(type_bits) {
        return Err(invalid(
            fd,
            format!("tlv type {ty} does not fit in {type_bits} bits"),
        ));
    }
    let label = variant.map_or_else(|| format!("type_{ty}"), |v| v.name.clone());
    let mut w = BitWriter::default();
    w.push_bits(type_bits, ty);
    let mut children = vec![FieldLayout::leaf(
        "type",
        0,
        type_bits as usize,
        FieldValue::UInt(ty),
    )];
    let finish = |bytes: Vec<u8>, children: Vec<FieldLayout>| {
        let lay = FieldLayout {
            children,
            ..FieldLayout::leaf(
                label.clone(),
                0,
                bytes.len() * 8,
                FieldValue::Bytes(bytes.clone()),
            )
        };
        (bytes, lay)
    };
    if variant.is_some_and(|v| v.type_only) && item.length.is_none() && item.data.is_none() {
        if !item.fields.is_empty() {
            return Err(invalid(
                fd,
                format!("tlv item `{label}` has no value fields"),
            ));
        }
        return Ok(finish(w.buf, children));
    }
    let hdr = ((type_bits + len_bits) / 8) as usize;
    let mut vw = BitWriter::default();
    let vchildren = match (&item.data, variant) {
        (Some(d), _) => {
            if !item.fields.is_empty() {
                return Err(invalid(
                    fd,
                    format!("tlv item `{label}` has both `data` and `fields`"),
                ));
            }
            vw.push_bytes(d);
            vec![FieldLayout::leaf(
                "data",
                0,
                d.len() * 8,
                FieldValue::Bytes(d.clone()),
            )]
        }
        (None, Some(v)) => encode_group(
            &mut vw,
            &format!("{}.{label}", fd.name),
            &v.fields,
            &item.fields,
            diags,
        )?,
        (None, None) if item.fields.is_empty() => Vec::new(),
        (None, None) => {
            return Err(invalid(
                fd,
                format!("tlv type {ty} is unknown; give its value as `data`"),
            ))
        }
    };
    let mut value = vw.buf;
    let unit = usize::try_from(t.len_unit.octets()).unwrap_or(1);
    let counted = if t.len_includes_header { hdr } else { 0 } + value.len();
    let expected = counted.div_ceil(unit);
    let field_name = format!("{}[{index}].length", fd.name);
    let (len, auto) = match item.length {
        Some(l) => {
            if l > mask(len_bits) {
                return Err(invalid(
                    fd,
                    format!("tlv length {l} does not fit in {len_bits} bits"),
                ));
            }
            if l != expected as u64 {
                diags.push(
                    Diagnostic::new(
                        DiagLevel::Warning,
                        keys::LENGTH_MISMATCH,
                        format!("`{field_name}` is {l:#x} but the correct length is {expected:#x}"),
                    )
                    .on_field(field_name.clone())
                    .arg("value", l)
                    .arg("expected", expected),
                );
            }
            (l, false)
        }
        None => {
            value.resize(value.len() + expected * unit - counted, 0);
            let l = expected as u64;
            if l > mask(len_bits) {
                diags.push(
                    Diagnostic::new(
                        DiagLevel::Error,
                        keys::AUTO_OVERFLOW,
                        format!(
                            "computed value {l} does not fit in `{field_name}` ({len_bits} bits)"
                        ),
                    )
                    .on_field(field_name.clone())
                    .arg("value", l)
                    .arg("bits", len_bits),
                );
            }
            (l & mask(len_bits), true)
        }
    };
    w.push_bits(len_bits, len);
    children.push(FieldLayout {
        auto,
        ..FieldLayout::leaf(
            "length",
            type_bits as usize,
            len_bits as usize,
            FieldValue::UInt(len),
        )
    });
    children.extend(shift(vchildren, hdr * 8));
    w.push_bytes(&value);
    Ok(finish(w.buf, children))
}

/// Pass 1: serialise one layer header.
fn encode_layer<'a>(
    def: &'a ProtocolDef,
    layer: &Layer,
    diags: &mut Vec<Diagnostic>,
) -> Result<Enc<'a>> {
    if let Some(name) = layer.fields.keys().find(|k| def.field(k).is_none()) {
        return Err(CoreError::UnknownField {
            protocol: def.id.clone(),
            field: name.clone(),
        });
    }
    if let Some(Synthesizer::Payload) = def.synth {
        return Ok(encode_synth(def, layer));
    }
    let mut w = BitWriter::default();
    let mut slots = Vec::with_capacity(def.fields.len());
    for (k, fd) in def.fields.iter().enumerate() {
        let manual = layer.fields.get(&fd.name);
        let pending = match (manual, &fd.auto) {
            (None, Some(a)) if !matches!(a, AutoCompute::Counter) => Some(a.clone()),
            _ => None,
        };
        let mut computed = false;
        let count = match &pending {
            Some(AutoCompute::Count { field }) => count_for(&def.fields, field, |f| {
                layer.fields.get(&f.name).or(f.default.as_ref())
            }),
            _ => None,
        };
        let value = match (manual, &pending, &fd.ty) {
            (Some(v), _, _) => v.clone(),
            (None, Some(AutoCompute::Count { .. }), _) if count.is_some() => {
                computed = true;
                FieldValue::UInt(fit_count(fd, count.unwrap_or(0), diags))
            }
            (None, Some(AutoCompute::Length { .. } | AutoCompute::Checksum { .. }), _) => {
                FieldValue::UInt(0)
            }
            (None, _, FieldType::Padding { align }) => {
                let align = *align as usize;
                let pad = if align == 0 {
                    0
                } else {
                    (align - (w.bits / 8) % align) % align
                };
                computed = true;
                FieldValue::Bytes(vec![0; pad])
            }
            (None, _, ty) => fd.default.clone().unwrap_or_else(|| zero_value(ty)),
        };
        let before = w.bits;
        let children = encode_value(&mut w, fd, &value, diags)?;
        slots.push(Slot {
            def: k,
            bit_off: before,
            bits: w.bits - before,
            value,
            manual: manual.is_some(),
            pending,
            computed,
            children,
        });
    }
    if !w.bits.is_multiple_of(8) {
        return Err(CoreError::InvalidDefinition {
            protocol: def.id.clone(),
            reason: "header is not a whole number of octets".into(),
        });
    }
    Ok(Enc {
        def,
        header: w.buf,
        slots,
        offset: 0,
    })
}

/// A generator protocol ([`ProtocolDef::synth`]): parameters (Manual or default)
/// → octets, reported as one `data` field.
fn encode_synth<'a>(def: &'a ProtocolDef, layer: &Layer) -> Enc<'a> {
    let params: std::collections::BTreeMap<String, FieldValue> = def
        .fields
        .iter()
        .filter_map(|f| {
            layer
                .fields
                .get(&f.name)
                .or(f.default.as_ref())
                .map(|v| (f.name.clone(), v.clone()))
        })
        .collect();
    let bytes = crate::synth::payload(&params);
    let generated = params
        .get("mode")
        .and_then(FieldValue::as_u64)
        .is_some_and(|m| m != crate::synth::payload_mode::FIXED);
    let slots = def
        .field_index("data")
        .map(|k| Slot {
            def: k,
            bit_off: 0,
            bits: bytes.len() * 8,
            value: FieldValue::Bytes(bytes.clone()),
            manual: layer.fields.contains_key("data"),
            pending: None,
            computed: generated,
            children: Vec::new(),
        })
        .into_iter()
        .collect();
    Enc {
        def,
        header: bytes,
        slots,
        offset: 0,
    }
}

struct Ctx<'a> {
    packet: &'a Packet,
    encs: Vec<Enc<'a>>,
    bytes: Vec<u8>,
    diags: Vec<Diagnostic>,
}

impl Ctx<'_> {
    fn spans(&self, i: usize) -> Spans {
        let e = &self.encs[i];
        Spans {
            header: e.header_len() as u64,
            payload: (self.bytes.len() - e.offset - e.header_len()) as u64,
        }
    }

    fn slot_abs_bit(&self, i: usize, s: usize) -> usize {
        self.encs[i].offset * 8 + self.encs[i].slots[s].bit_off
    }

    fn field_name(&self, i: usize, s: usize) -> &str {
        let e = &self.encs[i];
        &e.def.fields[e.slots[s].def].name
    }

    /// Writes a computed unsigned value into slot `s` of layer `i`, clamping with
    /// an error diagnostic when it does not fit.
    fn store(&mut self, i: usize, s: usize, value: i128) {
        let bits = self.encs[i].slots[s].bits;
        let width = u32::try_from(bits).unwrap_or(64);
        let m = mask(width);
        let fits = (0..=i128::from(m)).contains(&value);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let v = (value as u64) & m; // wrap on purpose; reported below
        if !fits {
            let name = self.field_name(i, s).to_owned();
            self.diags.push(
                Diagnostic::new(
                    DiagLevel::Error,
                    keys::AUTO_OVERFLOW,
                    format!("computed value {value} does not fit in `{name}` ({bits} bits)"),
                )
                .at_layer(i)
                .on_field(name)
                .arg("value", value)
                .arg("bits", bits),
            );
        }
        let abs = self.slot_abs_bit(i, s);
        put_bits(&mut self.bytes, abs, width, v);
        let slot = &mut self.encs[i].slots[s];
        slot.value = FieldValue::UInt(v);
        slot.computed = true;
    }

    /// Value an auto Length rule yields for layer `i` (plus an alignment warning).
    fn length_value(&self, i: usize, s: usize, auto: &AutoCompute) -> (i128, Option<Diagnostic>) {
        let AutoCompute::Length { of, unit, adjust } = auto else {
            return (0, None);
        };
        let len = self.spans(i).get(*of);
        let u = unit.octets();
        let diag = (!len.is_multiple_of(u)).then(|| {
            let name = self.field_name(i, s).to_owned();
            Diagnostic::new(
                DiagLevel::Warning,
                keys::LENGTH_NOT_ALIGNED,
                format!(
                    "{} length {len} is not a multiple of {u} octets (`{name}`)",
                    span_name(*of)
                ),
            )
            .at_layer(i)
            .on_field(name)
            .arg("len", len)
            .arg("unit", u)
        });
        (i128::from(len / u) + i128::from(*adjust), diag)
    }

    /// Selector value announcing the next layer, if layer `i` has one it knows.
    fn next_value(&self, i: usize) -> Option<u64> {
        let next = self.packet.layers.get(i + 1)?;
        self.encs[i].def.next.as_ref()?.value_for(&next.protocol)
    }

    /// Checksum for slot `s` of layer `i` over the current bytes (field zeroed),
    /// plus a warning when a pseudo header was wanted but not available.
    ///
    /// CRC values are returned byte-swapped so that the (big-endian) field carries
    /// them least significant octet first, as Ethernet FCS and SCTP do.
    fn checksum_value(&self, i: usize, s: usize, auto: &AutoCompute) -> (u64, Option<Diagnostic>) {
        let AutoCompute::Checksum {
            algorithm: kind,
            covers,
            nonzero,
            exclude,
        } = auto
        else {
            return (0, None);
        };
        let e = &self.encs[i];
        let end = match covers {
            ChecksumCoverage::Header => e.offset + e.header_len(),
            ChecksumCoverage::HeaderAndPayload => self.bytes.len(),
        };
        let mut data = self.bytes[e.offset..end].to_vec();
        let slot = &e.slots[s];
        let excluded = e
            .slots
            .iter()
            .filter(|x| exclude.contains(&e.def.fields[x.def].name));
        for z in std::iter::once(slot).chain(excluded) {
            zero_bits(&mut data, z.bit_off, z.bits);
        }
        match kind {
            ChecksumKind::Internet | ChecksumKind::InternetPseudoHeader => {}
            ChecksumKind::Fletcher => {
                return (u64::from(fletcher_checksum(&data, slot.bit_off / 8)), None)
            }
            ChecksumKind::Crc32 => return (u64::from(crc32(&data).swap_bytes()), None),
            ChecksumKind::Crc32c => return (u64::from(crc32c(&data).swap_bytes()), None),
        }
        let name = self.field_name(i, s).to_owned();
        let mut warning = None;
        let mut sum = InternetSum::new();
        if *kind == ChecksumKind::InternetPseudoHeader {
            match self.pseudo_header(i) {
                Some(ph) => {
                    sum.add(&ph);
                }
                None => {
                    warning = Some(
                        Diagnostic::new(
                            DiagLevel::Warning,
                            keys::NO_PSEUDO_HEADER,
                            format!(
                                "`{name}` needs an IPv4/IPv6 layer below for its pseudo header; computed without it"
                            ),
                        )
                        .at_layer(i)
                        .on_field(name),
                    );
                }
            }
        }
        sum.add(&data);
        let mut c = sum.finish();
        if *nonzero && c == 0 {
            c = 0xffff;
        }
        (u64::from(c), warning)
    }

    /// IPv4 / IPv6 pseudo header for layer `i` from the nearest lower layer that
    /// declares one.
    fn pseudo_header(&self, i: usize) -> Option<Vec<u8>> {
        let j = (0..i)
            .rev()
            .find(|&j| self.encs[j].def.pseudo_header.is_some())?;
        let lower = &self.encs[j];
        let ph = lower.def.pseudo_header.as_ref()?;
        let binding = lower.def.next.as_ref()?;
        let selector = lower.value(&binding.field).and_then(FieldValue::as_u64);
        let proto = if j + 1 == i {
            selector
        } else {
            binding
                .value_for(&self.packet.layers[i].protocol)
                .or(selector)
        }
        .unwrap_or(0);
        let upper_len = (self.bytes.len() - self.encs[i].offset) as u64;
        #[allow(clippy::cast_possible_truncation)] // wire widths, wrap on purpose
        match (lower.value(&ph.src)?, lower.value(&ph.dst)?) {
            (FieldValue::Ipv4(s), FieldValue::Ipv4(d)) => {
                let mut v = Vec::with_capacity(12);
                v.extend_from_slice(&s.octets());
                v.extend_from_slice(&d.octets());
                v.push(0);
                v.push(proto as u8);
                v.extend_from_slice(&(upper_len as u16).to_be_bytes());
                Some(v)
            }
            (FieldValue::Ipv6(s), FieldValue::Ipv6(d)) => {
                let mut v = Vec::with_capacity(40);
                v.extend_from_slice(&s.octets());
                v.extend_from_slice(&d.octets());
                v.extend_from_slice(&(upper_len as u32).to_be_bytes());
                v.extend_from_slice(&[0, 0, 0, proto as u8]);
                Some(v)
            }
            _ => None,
        }
    }

    fn slots_with<F: Fn(&AutoCompute) -> bool>(
        &self,
        i: usize,
        pred: F,
    ) -> Vec<(usize, AutoCompute)> {
        self.encs[i]
            .slots
            .iter()
            .enumerate()
            .filter_map(|(s, slot)| {
                slot.pending
                    .as_ref()
                    .filter(|a| pred(a))
                    .map(|a| (s, a.clone()))
            })
            .collect()
    }

    fn fill_lengths(&mut self) {
        for i in (0..self.encs.len()).rev() {
            for (s, auto) in self.slots_with(i, |a| matches!(a, AutoCompute::Length { .. })) {
                let (v, diag) = self.length_value(i, s, &auto);
                self.diags.extend(diag);
                self.store(i, s, v);
            }
        }
    }

    fn fill_next_protocols(&mut self) {
        for i in 0..self.encs.len() {
            let Some(v) = self.next_value(i) else {
                continue;
            };
            for (s, _) in self.slots_with(i, |a| matches!(a, AutoCompute::NextProtocol)) {
                self.store(i, s, i128::from(v));
            }
        }
    }

    fn fill_checksums(&mut self) {
        for i in (0..self.encs.len()).rev() {
            for (s, auto) in self.slots_with(i, |a| matches!(a, AutoCompute::Checksum { .. })) {
                let (v, warning) = self.checksum_value(i, s, &auto);
                self.diags.extend(warning);
                self.store(i, s, i128::from(v));
            }
        }
    }

    /// Warns about Manual values of auto-capable fields that differ from what Auto
    /// would have produced.
    fn check_manual_autos(&mut self) {
        for i in 0..self.encs.len() {
            for s in 0..self.encs[i].slots.len() {
                let slot = &self.encs[i].slots[s];
                let Some(auto) = self.encs[i].def.fields[slot.def].auto.clone() else {
                    continue;
                };
                if !slot.manual {
                    continue;
                }
                let Some(actual) = slot.value.as_u64() else {
                    continue;
                };
                let name = self.field_name(i, s).to_owned();
                let (key, expected, what) = match &auto {
                    AutoCompute::Length { .. } => {
                        let (v, _) = self.length_value(i, s, &auto);
                        (keys::LENGTH_MISMATCH, v, "length")
                    }
                    AutoCompute::Checksum { .. } => {
                        let (v, _) = self.checksum_value(i, s, &auto);
                        (keys::CHECKSUM_MISMATCH, i128::from(v), "checksum")
                    }
                    AutoCompute::NextProtocol => match self.next_value(i) {
                        Some(v) => (keys::NEXT_PROTOCOL_MISMATCH, i128::from(v), "next protocol"),
                        None => continue,
                    },
                    AutoCompute::Counter => continue,
                    AutoCompute::Count { field } => {
                        let e = &self.encs[i];
                        let target = e.def.fields.iter().position(|f| &f.name == field);
                        let n = target
                            .and_then(|k| e.slots.iter().find(|x| x.def == k))
                            .and_then(|x| element_count(&e.def.fields[x.def].ty, &x.value));
                        match n {
                            Some(n) => (keys::COUNT_MISMATCH, i128::from(n), "count"),
                            None => continue,
                        }
                    }
                };
                if i128::from(actual) == expected {
                    continue;
                }
                let mut d = Diagnostic::new(
                    DiagLevel::Warning,
                    key,
                    format!("`{name}` is {actual:#x} but the correct {what} is {expected:#x}"),
                )
                .at_layer(i)
                .on_field(name)
                .arg("value", actual)
                .arg("expected", expected);
                if matches!(auto, AutoCompute::NextProtocol) {
                    d = d.arg("next", &self.packet.layers[i + 1].protocol);
                }
                self.diags.push(d);
            }
        }
    }

    fn check_constraints(&mut self) {
        for i in 0..self.encs.len() {
            let spans = self.spans(i);
            let e = &self.encs[i];
            for c in &e.def.constraints {
                if let Some(d) = evaluate(c, i, |n| e.value(n), spans) {
                    self.diags.push(d);
                }
            }
        }
    }
}

fn build(packet: &Packet, catalog: &dyn Resolve) -> Result<Built> {
    let raw = raw_protocol();
    let mut encs = Vec::with_capacity(packet.layers.len());
    let mut bytes = Vec::new();
    let mut diags = Vec::new();
    for (i, layer) in packet.layers.iter().enumerate() {
        let def = resolve(catalog, &raw, &layer.protocol)
            .ok_or_else(|| CoreError::UnknownProtocol(layer.protocol.clone()))?;
        let mut layer_diags = Vec::new();
        let mut enc = encode_layer(def, layer, &mut layer_diags)?;
        diags.extend(layer_diags.into_iter().map(|d| d.at_layer(i)));
        enc.offset = bytes.len();
        bytes.extend_from_slice(&enc.header);
        encs.push(enc);
    }
    let mut ctx = Ctx {
        packet,
        encs,
        bytes,
        diags,
    };
    ctx.fill_lengths();
    ctx.fill_next_protocols();
    ctx.fill_checksums();
    ctx.check_manual_autos();
    ctx.check_constraints();

    // Deliberate truncation (after everything was computed on the full packet).
    let total = ctx.bytes.len();
    let mut cut = total;
    for (i, layer) in packet.layers.iter().enumerate() {
        if let Some(n) = layer.truncate {
            let end = ctx.encs[i].offset.saturating_add(n);
            if end < total {
                ctx.diags.push(
                    Diagnostic::new(
                        DiagLevel::Info,
                        keys::TRUNCATED,
                        format!("layer truncated to {n} octets"),
                    )
                    .at_layer(i)
                    .arg("len", n),
                );
            }
            cut = cut.min(end);
        }
    }
    let min_frame = ctx
        .encs
        .first()
        .and_then(|e| e.def.min_frame_len)
        .map(|m| m as usize);
    let Ctx {
        encs,
        mut bytes,
        diags,
        ..
    } = ctx;
    bytes.truncate(cut);

    let mut trailers = Vec::new();
    if let (true, Some(min)) = (packet.pad_to_min, min_frame) {
        if bytes.len() < min {
            trailers.push(TrailerLayout {
                kind: TrailerKind::Padding,
                offset: bytes.len(),
                len: min - bytes.len(),
            });
            bytes.resize(min, 0);
        }
    }
    let fcs = match packet.fcs {
        Some(FcsMode::Auto) => Some(crc32(&bytes)),
        Some(FcsMode::Bad) => Some(!crc32(&bytes)),
        Some(FcsMode::Manual(v)) => Some(v),
        Some(FcsMode::None) | None => None,
    };
    if let Some(v) = fcs {
        trailers.push(TrailerLayout {
            kind: TrailerKind::Fcs,
            offset: bytes.len(),
            len: 4,
        });
        bytes.extend_from_slice(&v.to_le_bytes());
    }

    let layout = encs
        .into_iter()
        .map(|e| {
            let offset = e.offset.min(cut);
            let len = total - e.offset;
            let present = cut.saturating_sub(e.offset).min(len);
            let base = e.offset * 8;
            LayerLayout {
                protocol: e.def.id.clone(),
                offset,
                header_len: e.header.len(),
                len: present,
                truncated: present < len,
                fields: e
                    .slots
                    .into_iter()
                    .map(|s| FieldLayout {
                        name: e.def.fields[s.def].name.clone(),
                        bit_offset: base + s.bit_off,
                        bit_len: s.bits,
                        value: s.value,
                        auto: s.computed,
                        children: clip(shift(s.children, base), cut * 8),
                    })
                    .filter(|f| f.bit_offset + f.bit_len <= cut * 8)
                    .collect(),
            }
        })
        .collect();
    Ok(Built {
        bytes,
        layout,
        diagnostics: diags,
        trailers,
    })
}

/// Zeroes `bits` bits at `bit_off` (any width; octets beyond `buf` are ignored).
pub(crate) fn zero_bits(buf: &mut [u8], mut bit_off: usize, mut bits: usize) {
    while bits > 0 {
        let w = bits.min(64);
        put_bits(buf, bit_off, u32::try_from(w).unwrap_or(64), 0);
        bit_off += w;
        bits -= w;
    }
}

/// Drops layouts that do not fit completely before bit `end`.
fn clip(v: Vec<FieldLayout>, end: usize) -> Vec<FieldLayout> {
    v.into_iter()
        .filter(|f| f.bit_offset + f.bit_len <= end)
        .map(|mut f| {
            f.children = clip(std::mem::take(&mut f.children), end);
            f
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checksum::internet_checksum;
    use crate::constraint::{Constraint, ConstraintRule};
    use crate::ir::{Binding, BindingEntry, LengthOf, LengthUnit, ProtocolLayer, PseudoHeader};

    /// A tiny two-level stack: `outer` (IPv4-like) → `inner` (UDP-like).
    pub(super) fn defs() -> Vec<ProtocolDef> {
        let mut outer = ProtocolDef::new("outer", "Outer", ProtocolLayer::L3);
        outer.fields = vec![
            FieldDef::new("ver", FieldType::UInt { bits: 4 }).with_default(FieldValue::UInt(4)),
            FieldDef::new("hl", FieldType::UInt { bits: 4 }).with_auto(AutoCompute::Length {
                of: LengthOf::Header,
                unit: LengthUnit::Words32,
                adjust: 0,
            }),
            FieldDef::new("proto", FieldType::UInt { bits: 8 })
                .with_default(FieldValue::UInt(0xfd))
                .with_auto(AutoCompute::NextProtocol),
            FieldDef::new("total", FieldType::UInt { bits: 16 }).with_auto(AutoCompute::Length {
                of: LengthOf::Total,
                unit: LengthUnit::Bytes,
                adjust: 0,
            }),
            FieldDef::new("id", FieldType::UInt { bits: 16 }),
            FieldDef::new("csum", FieldType::UInt { bits: 16 }).with_auto(AutoCompute::Checksum {
                algorithm: ChecksumKind::Internet,
                covers: ChecksumCoverage::Header,
                nonzero: false,
                exclude: Vec::new(),
            }),
            FieldDef::new("src", FieldType::Ipv4),
            FieldDef::new("dst", FieldType::Ipv4),
        ];
        outer.next = Some(Binding::new("proto", vec![BindingEntry::new(17, "inner")]));
        outer.pseudo_header = Some(PseudoHeader {
            src: "src".into(),
            dst: "dst".into(),
        });
        outer.constraints = vec![Constraint::new(
            DiagLevel::Error,
            ConstraintRule::FieldInRange {
                field: "hl".into(),
                min: 3,
                max: 15,
            },
        )];
        let mut inner = ProtocolDef::new("inner", "Inner", ProtocolLayer::L4);
        inner.fields = vec![
            FieldDef::new("len", FieldType::UInt { bits: 16 }).with_auto(AutoCompute::Length {
                of: LengthOf::Total,
                unit: LengthUnit::Bytes,
                adjust: 0,
            }),
            FieldDef::new("csum", FieldType::UInt { bits: 16 }).with_auto(AutoCompute::Checksum {
                algorithm: ChecksumKind::InternetPseudoHeader,
                covers: ChecksumCoverage::HeaderAndPayload,
                nonzero: true,
                exclude: Vec::new(),
            }),
        ];
        for d in [&outer, &inner] {
            d.validate().unwrap();
        }
        vec![outer, inner]
    }

    pub(super) fn packet() -> Packet {
        Packet::from_layers(vec![
            Layer::new("outer")
                .with("src", Ipv4Addr::new(10, 0, 0, 1))
                .with("dst", Ipv4Addr::new(10, 0, 0, 2)),
            Layer::new("inner"),
            Layer::new("raw").with("data", vec![1u8, 2, 3]),
        ])
    }

    #[test]
    fn auto_fields_are_filled() {
        let built = packet().build(&defs()).unwrap();
        let b = &built.bytes;
        assert_eq!(b.len(), 16 + 4 + 3);
        assert_eq!(b[0], 0x44, "ver 4, hl 4 words");
        assert_eq!(b[1], 17, "next protocol from binding");
        assert_eq!(u16::from_be_bytes([b[2], b[3]]), 23);
        let mut hdr = b[..16].to_vec();
        hdr[6] = 0;
        hdr[7] = 0;
        assert_eq!(u16::from_be_bytes([b[6], b[7]]), internet_checksum(&hdr));
        // Inner checksum with pseudo header.
        let mut ph = vec![10, 0, 0, 1, 10, 0, 0, 2, 0, 17, 0, 7];
        ph.extend_from_slice(&[0, 7, 0, 0, 1, 2, 3]);
        assert_eq!(u16::from_be_bytes([b[18], b[19]]), internet_checksum(&ph));
        assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
        assert_eq!(built.layout.len(), 3);
        assert_eq!(built.layout[1].offset, 16);
        assert_eq!(built.layout[1].header_len, 4);
        let hl = &built.layout[0].fields[1];
        assert_eq!((hl.name.as_str(), hl.bit_offset, hl.bit_len), ("hl", 4, 4));
        assert!(hl.auto);
        assert!(!built.layout[0].fields[0].auto);
    }

    #[test]
    fn manual_values_are_verbatim_and_warned() {
        let mut p = packet();
        p.layers[0]
            .fields
            .insert("csum".into(), FieldValue::UInt(0xdead));
        p.layers[0].fields.insert("hl".into(), FieldValue::UInt(2));
        let built = p.build(&defs()).unwrap();
        assert_eq!(&built.bytes[6..8], &[0xde, 0xad]);
        assert_eq!(built.bytes[0], 0x42);
        let keys: Vec<_> = built.diagnostics.iter().map(|d| d.key.as_str()).collect();
        assert!(keys.contains(&keys::CHECKSUM_MISMATCH), "{keys:?}");
        assert!(keys.contains(&keys::LENGTH_MISMATCH), "{keys:?}");
        assert!(keys.contains(&keys::FIELD_OUT_OF_RANGE), "{keys:?}");
        assert!(built.has_errors());
    }

    #[test]
    fn missing_pseudo_header_is_a_warning() {
        let p = Packet::from_layers(vec![Layer::new("inner")]);
        let built = p.build(&defs()).unwrap();
        assert_eq!(built.diagnostics[0].key, keys::NO_PSEUDO_HEADER);
        assert_eq!(built.bytes[..2], [0, 4]);
    }

    #[test]
    fn truncation_and_errors() {
        let mut p = packet();
        p.layers[1].truncate = Some(2);
        let built = p.build(&defs()).unwrap();
        assert_eq!(built.bytes.len(), 18);
        assert!(built.layout[1].truncated);
        assert_eq!(built.layout[1].fields.len(), 1);
        assert!(built.layout[2].truncated);
        assert_eq!(built.layout[2].len, 0);
        assert_eq!(built.diagnostics[0].key, keys::TRUNCATED);

        let p = Packet::from_layers(vec![Layer::new("nope")]);
        assert!(matches!(
            p.build(&defs()),
            Err(CoreError::UnknownProtocol(_))
        ));
        let p = Packet::from_layers(vec![Layer::new("inner").with("bogus", 1u8)]);
        assert!(matches!(
            p.build(&defs()),
            Err(CoreError::UnknownField { .. })
        ));
        let p = Packet::from_layers(vec![Layer::new("outer").with("ver", 16u8)]);
        assert!(matches!(
            p.build(&defs()),
            Err(CoreError::InvalidValue { .. })
        ));
        let p = Packet::from_layers(vec![Layer::new("outer").with("src", 1u8)]);
        assert!(matches!(
            p.build(&defs()),
            Err(CoreError::InvalidValue { .. })
        ));
    }

    #[test]
    fn auto_overflow_is_an_error_diagnostic() {
        let mut p = packet();
        p.layers[2] = Layer::new("raw").with("data", vec![0u8; 70_000]);
        let built = p.build(&defs()).unwrap();
        assert!(built
            .diagnostics
            .iter()
            .any(|d| d.key == keys::AUTO_OVERFLOW && d.level == DiagLevel::Error));
    }

    #[test]
    fn fletcher_and_crc_checksum_fields() {
        let mut p = ProtocolDef::new("osi", "OSI-like", ProtocolLayer::L3);
        p.fields = vec![
            FieldDef::new("a", FieldType::UInt { bits: 16 }).with_default(FieldValue::UInt(0x8101)),
            FieldDef::new("ck", FieldType::UInt { bits: 16 }).with_auto(AutoCompute::Checksum {
                algorithm: ChecksumKind::Fletcher,
                covers: ChecksumCoverage::Header,
                nonzero: false,
                exclude: Vec::new(),
            }),
            FieldDef::new("b", FieldType::UInt { bits: 32 })
                .with_default(FieldValue::UInt(0x1234_5678)),
            FieldDef::new("crc", FieldType::UInt { bits: 32 }).with_auto(AutoCompute::Checksum {
                algorithm: ChecksumKind::Crc32c,
                covers: ChecksumCoverage::HeaderAndPayload,
                nonzero: false,
                exclude: Vec::new(),
            }),
        ];
        p.validate().unwrap();
        let defs = vec![p];
        let built = Packet::from_layers(vec![
            Layer::new("osi"),
            Layer::new("raw").with("data", b"123456789".to_vec()),
        ])
        .build(&defs)
        .unwrap();
        let b = &built.bytes;
        // CRC-32C over everything with the field zeroed, least significant octet first.
        let mut z = b.clone();
        z[8..12].fill(0);
        assert_eq!(&b[8..12], &crc32c(&z).to_le_bytes());
        // Checksums of one layer are filled in field order: the Fletcher sum saw the
        // CRC field still zero, and verifies (C0 = C1 = 0) over those octets.
        let (mut c0, mut c1) = (0u32, 0u32);
        for x in &z[..12] {
            c0 = (c0 + u32::from(*x)) % 255;
            c1 = (c1 + c0) % 255;
        }
        assert_eq!((c0, c1), (0, 0));
        assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);

        // A 16-bit CRC field is rejected by validation.
        let mut bad = defs[0].clone();
        bad.fields[3].ty = FieldType::UInt { bits: 16 };
        assert!(bad.validate().is_err());
    }
}
