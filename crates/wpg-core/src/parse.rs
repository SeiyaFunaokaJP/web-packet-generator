//! Packet parser: wire bytes → [`Packet`] (TODO §2.1 "逆生成 (import)").
//!
//! The parser decodes one header at a time, follows each protocol's [`Binding`] to
//! pick the next protocol and bounds inner PDUs by their total / payload length
//! fields. Anything it cannot decode becomes a trailing `raw` layer. It never panics
//! on arbitrary input: problems are reported as [`Diagnostic`]s.
//!
//! Every decoded field is recorded as *Manual* so that building the parsed packet
//! reproduces the input byte for byte (including wrong checksums / lengths, which
//! then show up as warnings). Auto padding that is all zeros is left on Auto, and
//! so are TLV item lengths that match their content.
//!
//! TLV and list fields are decoded into typed items where possible; every decoded
//! composite value is verified by re-encoding it, and falls back to raw items /
//! verbatim octets when that would not reproduce the input.
//!
//! Zero padding up to the minimum frame length of the outermost protocol (Ethernet:
//! 60 octets) is recognised and mapped to [`Packet::pad_to_min`]. The FCS is not
//! expected (capture backends usually strip it); use [`parse_with_fcs`] for frames
//! that carry one.
//!
//! [`Binding`]: crate::ir::Binding

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, Ipv6Addr};

use serde::{Deserialize, Serialize};

use crate::bits::{get_bits, BitWriter};
use crate::build::{encode_tlv_item, encode_value, resolve, LayerLayout, TrailerLayout};
use crate::checksum::crc32;
use crate::diag::{keys, DiagLevel, Diagnostic};
use crate::error::{CoreError, Result};
use crate::ir::{
    raw_protocol, AutoCompute, FieldDef, FieldType, ItemDef, Length, LengthOf, ProtocolDef,
    Resolve, TlvDef, RAW_PROTOCOL_ID,
};
use crate::packet::{FcsMode, FieldValue, Layer, Packet, TlvItem};

/// Output of [`parse`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parsed {
    /// Decoded layers; every field is Manual with its decoded value.
    pub packet: Packet,
    /// Layer / field positions (from rebuilding [`Parsed::packet`]).
    pub layout: Vec<LayerLayout>,
    /// Parse problems followed by the builder's diagnostics for the packet.
    pub diagnostics: Vec<Diagnostic>,
    /// Padding / FCS after the layers (from rebuilding [`Parsed::packet`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trailers: Vec<TrailerLayout>,
}

impl Parsed {
    /// Whether any diagnostic has [`DiagLevel::Error`].
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.level == DiagLevel::Error)
    }
}

struct Header {
    fields: BTreeMap<String, FieldValue>,
    len: usize,
}

/// The protocol a field group belongs to (for diagnostics).
#[derive(Clone, Copy)]
struct Who<'a> {
    name: &'a str,
    id: &'a str,
    layer: usize,
}

fn short(who: Who<'_>, offset: usize) -> Diagnostic {
    Diagnostic::new(
        DiagLevel::Error,
        keys::SHORT_HEADER,
        format!(
            "input ends inside the {} header at offset {offset}",
            who.name
        ),
    )
    .at_layer(who.layer)
    .arg("protocol", who.id)
    .arg("offset", offset)
}

fn take(data: &[u8], bit: usize, n: usize) -> Option<&[u8]> {
    if !bit.is_multiple_of(8) {
        return None;
    }
    let start = bit / 8;
    data.get(start..start.checked_add(n)?)
}

/// Octets a `Length` announces, clamped to `remaining` (with a diagnostic).
/// `offset` is where the region starts in its field group.
#[allow(clippy::too_many_arguments)]
fn region_len(
    len: &Length,
    fd: &FieldDef,
    fields: &BTreeMap<String, FieldValue>,
    remaining: usize,
    who: Who<'_>,
    at: usize,
    offset: usize,
    diags: &mut Vec<Diagnostic>,
) -> Result<usize, Diagnostic> {
    match len {
        Length::Fixed { octets } => {
            let n = *octets as usize;
            if n > remaining {
                Err(short(who, at))
            } else {
                Ok(n)
            }
        }
        // TLV regions closed by an item are measured in `decode_fields`.
        Length::ToEnd | Length::UntilTlv { .. } => Ok(remaining),
        // Count lists are bounded by their elements (see `decode_counted`).
        Length::Count { .. } => Ok(remaining),
        Length::FromField { field, .. } | Length::UntilOffset { field, .. } => {
            let v = fields.get(field).and_then(FieldValue::as_u64).unwrap_or(0);
            let want = match len {
                Length::FromField {
                    unit, adjust, div, ..
                } => {
                    (i128::from(v) * i128::from(*unit) + i128::from(*adjust))
                        / i128::from((*div).max(1))
                }
                Length::UntilOffset { unit, adjust, .. } => {
                    i128::from(v) * i128::from(*unit) + i128::from(*adjust) - offset as i128
                }
                _ => 0,
            };
            let want = usize::try_from(want.max(0)).unwrap_or(usize::MAX);
            if want > remaining {
                diags.push(
                    Diagnostic::new(
                        DiagLevel::Error,
                        keys::LENGTH_EXCEEDS_DATA,
                        format!(
                            "`{field}` announces {want} octets of `{}` but only {remaining} are present",
                            fd.name
                        ),
                    )
                    .at_layer(who.layer)
                    .on_field(field.clone())
                    .arg("len", want)
                    .arg("available", remaining),
                );
                Ok(remaining)
            } else {
                Ok(want)
            }
        }
    }
}

/// Re-encodes `value` for `fd`; `true` when that reproduces `bytes` exactly.
fn reproduces(fd: &FieldDef, value: &FieldValue, bytes: &[u8]) -> bool {
    let mut w = BitWriter::default();
    encode_value(&mut w, fd, value, &mut Vec::new()).is_ok() && w.buf == bytes
}

/// Decodes a group of fields from `data` (which starts at packet offset `base`).
fn decode_fields(
    who: Who<'_>,
    defs: &[FieldDef],
    data: &[u8],
    base: usize,
    diags: &mut Vec<Diagnostic>,
) -> Result<Header, Diagnostic> {
    let mut fields = BTreeMap::new();
    let mut bit = 0usize;
    for fd in defs {
        let at = base + bit / 8;
        let remaining = data.len().saturating_sub(bit.div_ceil(8));
        let value = match &fd.ty {
            FieldType::UInt { bits }
            | FieldType::Enum { bits, .. }
            | FieldType::Flags { bits, .. } => {
                let v = get_bits(data, bit, u32::from(*bits)).ok_or_else(|| short(who, at))?;
                bit += usize::from(*bits);
                FieldValue::UInt(v)
            }
            FieldType::Int { bits } => {
                let w = u32::from(*bits);
                let v = get_bits(data, bit, w).ok_or_else(|| short(who, at))?;
                bit += usize::from(*bits);
                // Sign-extend from `w` bits.
                let shift = 64 - w;
                #[allow(clippy::cast_possible_wrap)]
                let signed = ((v << shift) as i64) >> shift;
                FieldValue::Int(signed)
            }
            FieldType::Mac
            | FieldType::Ipv4
            | FieldType::Ipv6
            | FieldType::Bytes {
                len: Length::Fixed { .. },
            } => {
                let n = fd.ty.fixed_bits().unwrap_or(0) as usize / 8;
                let b = take(data, bit, n).ok_or_else(|| short(who, at))?;
                bit += n * 8;
                match &fd.ty {
                    FieldType::Mac => {
                        let mut m = [0u8; 6];
                        m.copy_from_slice(b);
                        FieldValue::Mac(m)
                    }
                    FieldType::Ipv4 => {
                        let mut a = [0u8; 4];
                        a.copy_from_slice(b);
                        FieldValue::Ipv4(Ipv4Addr::from(a))
                    }
                    FieldType::Ipv6 => {
                        let mut a = [0u8; 16];
                        a.copy_from_slice(b);
                        FieldValue::Ipv6(Ipv6Addr::from(a))
                    }
                    _ => FieldValue::Bytes(b.to_vec()),
                }
            }
            FieldType::Bytes { len } => {
                let n = region_len(len, fd, &fields, remaining, who, at, bit / 8, diags)?;
                let b = take(data, bit, n).ok_or_else(|| short(who, at))?;
                bit += n * 8;
                FieldValue::Bytes(b.to_vec())
            }
            FieldType::CString => {
                let rest = take(data, bit, remaining).ok_or_else(|| short(who, at))?;
                let n = rest
                    .iter()
                    .position(|b| *b == 0)
                    .ok_or_else(|| short(who, at))?;
                bit += (n + 1) * 8;
                FieldValue::Bytes(rest[..n].to_vec())
            }
            FieldType::Raw => {
                let b = take(data, bit, remaining).ok_or_else(|| short(who, at))?;
                bit += remaining * 8;
                FieldValue::Bytes(b.to_vec())
            }
            FieldType::Padding { align } => {
                let align = *align as usize;
                let pad = if align == 0 {
                    0
                } else {
                    (align - (bit / 8) % align) % align
                };
                let b = take(data, bit, pad.min(remaining)).ok_or_else(|| short(who, at))?;
                bit += b.len() * 8;
                if b.len() == pad && b.iter().all(|x| *x == 0) {
                    continue; // identical to Auto
                }
                FieldValue::Bytes(b.to_vec())
            }
            FieldType::Tlv(t) => {
                let n = match &t.length {
                    Length::UntilTlv { type_value } => {
                        let rest = take(data, bit, remaining).ok_or_else(|| short(who, at))?;
                        tlv_span_until(t, rest, *type_value)
                    }
                    len => region_len(len, fd, &fields, remaining, who, at, bit / 8, diags)?,
                };
                let region = take(data, bit, n).ok_or_else(|| short(who, at))?;
                bit += n * 8;
                let items = decode_tlv(fd, t, region, Some((who, at, &mut *diags)));
                let v = FieldValue::Tlv(items);
                if reproduces(fd, &v, region) {
                    v
                } else {
                    FieldValue::Bytes(region.to_vec())
                }
            }
            FieldType::List {
                item,
                length: Length::Count { field },
            } => {
                let want = fields.get(field).and_then(FieldValue::as_u64).unwrap_or(0);
                let rest = take(data, bit, remaining).ok_or_else(|| short(who, at))?;
                let (elems, n) =
                    decode_counted(who, fd, item, rest, at, want).ok_or_else(|| short(who, at))?;
                bit += n * 8;
                let v = FieldValue::List(elems);
                if reproduces(fd, &v, &rest[..n]) {
                    v
                } else {
                    FieldValue::Bytes(rest[..n].to_vec())
                }
            }
            FieldType::List { item, length } => {
                let n = region_len(length, fd, &fields, remaining, who, at, bit / 8, diags)?;
                let region = take(data, bit, n).ok_or_else(|| short(who, at))?;
                bit += n * 8;
                match decode_list(who, fd, item, region, at).map(FieldValue::List) {
                    Some(v) if reproduces(fd, &v, region) => v,
                    _ => FieldValue::Bytes(region.to_vec()),
                }
            }
            FieldType::Nsap => {
                let rest = take(data, bit, remaining).ok_or_else(|| short(who, at))?;
                let n = usize::from(*rest.first().ok_or_else(|| short(who, at))?);
                let addr = rest.get(1..=n).ok_or_else(|| short(who, at))?;
                bit += (n + 1) * 8;
                FieldValue::Nsap(addr.to_vec())
            }
        };
        fields.insert(fd.name.clone(), value);
    }
    if !bit.is_multiple_of(8) {
        return Err(short(who, base + bit / 8));
    }
    Ok(Header {
        fields,
        len: bit / 8,
    })
}

/// Decodes exactly `count` list elements from the start of `data`; returns them and
/// the octets they span, `None` if one does not decode.
fn decode_counted(
    who: Who<'_>,
    fd: &FieldDef,
    item: &ItemDef,
    data: &[u8],
    base: usize,
    count: u64,
) -> Option<(Vec<FieldValue>, usize)> {
    let mut out = Vec::new();
    let mut pos = 0;
    for _ in 0..count {
        let (elem, n) = decode_element(who, fd, item, data.get(pos..)?, base + pos)?;
        out.push(elem);
        pos += n;
    }
    Some((out, pos))
}

/// Decodes one list element from the start of `data`: the value and its size.
fn decode_element(
    who: Who<'_>,
    fd: &FieldDef,
    item: &ItemDef,
    data: &[u8],
    base: usize,
) -> Option<(FieldValue, usize)> {
    let scratch = &mut Vec::new();
    match item {
        ItemDef::Field { ty } => {
            let efd = [FieldDef::new(fd.name.clone(), (**ty).clone())];
            let n = ty.fixed_bits()? as usize / 8;
            let h = decode_fields(who, &efd, data.get(..n)?, base, scratch).ok()?;
            Some((h.fields.into_values().next()?, h.len))
        }
        ItemDef::Struct { fields } => {
            let h = decode_fields(who, fields, data, base, scratch).ok()?;
            Some((FieldValue::Struct(h.fields), h.len))
        }
    }
}

/// Decodes list elements until `region` is used up; `None` if an element does not
/// decode completely.
fn decode_list(
    who: Who<'_>,
    fd: &FieldDef,
    item: &ItemDef,
    region: &[u8],
    base: usize,
) -> Option<Vec<FieldValue>> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < region.len() {
        let (elem, n) = decode_element(who, fd, item, &region[pos..], base + pos)?;
        if n == 0 {
            return None;
        }
        out.push(elem);
        pos += n;
    }
    Some(out)
}

/// Reads a big-endian unsigned value of `octets` octets.
fn be(data: &[u8]) -> u64 {
    data.iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b))
}

/// Octets of `data` up to and including the first item of type `end` (with its
/// padding), or all of `data` when no such item is framed.
fn tlv_span_until(t: &TlvDef, data: &[u8], end: u64) -> usize {
    let tb = usize::from(t.type_bits / 8);
    let ty_at = if t.len_first {
        usize::from(t.len_bits / 8)
    } else {
        0
    };
    let unit = usize::try_from(t.len_unit.octets()).unwrap_or(1);
    let align = t.align.max(1) as usize;
    let padded = |n: usize| n + (align - n % align) % align;
    let mut pos = 0;
    while let Some(rest) = data.get(pos..).filter(|r| r.len() >= ty_at + tb) {
        let ty = be(&rest[ty_at..ty_at + tb]);
        let total = if t.variant_by_type(ty).is_some_and(|v| v.type_only) {
            Some(tb)
        } else {
            let hl = tb + usize::from(t.head_octets) + usize::from(t.len_bits_for(ty) / 8);
            let len = if t.len_first {
                rest.get(..ty_at).map(be)
            } else {
                rest.get(tb + usize::from(t.head_octets)..hl).map(be)
            };
            len.and_then(|l| usize::try_from(l).ok()?.checked_mul(unit))
                .and_then(|l| {
                    if t.len_includes_header {
                        (l >= hl).then_some(l)
                    } else {
                        l.checked_add(hl)
                    }
                })
        };
        let Some(total) = total.filter(|n| *n <= rest.len()) else {
            break;
        };
        pos += padded(total).min(rest.len());
        if t.same_type(ty, end) {
            return pos;
        }
    }
    data.len()
}

/// Decodes a TLV region into items. Never fails: whatever cannot be framed becomes
/// one trailing raw item (with a warning when `top` is given).
fn decode_tlv(
    fd: &FieldDef,
    t: &TlvDef,
    region: &[u8],
    mut top: Option<(Who<'_>, usize, &mut Vec<Diagnostic>)>,
) -> Vec<TlvItem> {
    let tb = usize::from(t.type_bits / 8);
    let head = usize::from(t.head_octets);
    let unit = usize::try_from(t.len_unit.octets()).unwrap_or(1);
    let align = t.align.max(1) as usize;
    // Item size with its padding (the last item may lack it).
    let padded = |n: usize, left: usize| (n + (align - n % align) % align).min(left);
    let mut items = Vec::new();
    let mut pos = 0;
    // `want`: octets the item needs, or `None` when its length value is invalid.
    let mut malformed = |pos: usize, want: Option<usize>, items: &mut Vec<TlvItem>| {
        if let Some((who, at, diags)) = top.as_mut() {
            let left = region.len() - pos;
            let msg = match want {
                Some(n) => format!(
                    "item at offset {} of `{}` needs {n} octets but only {left} are left",
                    *at + pos,
                    fd.name
                ),
                None => format!(
                    "item at offset {} of `{}` has an invalid length",
                    *at + pos,
                    fd.name
                ),
            };
            diags.push(
                Diagnostic::new(DiagLevel::Warning, keys::LENGTH_EXCEEDS_DATA, msg)
                    .at_layer(who.layer)
                    .on_field(fd.name.clone())
                    .arg("len", want.unwrap_or(0))
                    .arg("available", left),
            );
        }
        items.push(TlvItem::raw(&region[pos..]));
    };
    while pos < region.len() {
        let rest = &region[pos..];
        if rest.len() < tb {
            malformed(pos, Some(tb), &mut items);
            break;
        }
        // With the length first, the type follows it.
        let ty_at = if t.len_first {
            usize::from(t.len_bits / 8)
        } else {
            0
        };
        if rest.len() < ty_at + tb {
            malformed(pos, Some(ty_at + tb), &mut items);
            break;
        }
        let ty = be(&rest[ty_at..ty_at + tb]);
        let variant = t.variant_by_type(ty);
        if variant.is_some_and(|v| v.type_only) {
            items.push(TlvItem {
                ty: Some(ty),
                name: variant.map(|v| v.name.clone()),
                ..TlvItem::default()
            });
            pos += padded(tb, rest.len());
            continue;
        }
        if variant.is_none() && !t.unknown_as_raw {
            items.push(TlvItem::raw(rest));
            break;
        }
        let hl = tb + head + usize::from(t.len_bits_for(ty) / 8);
        if rest.len() < hl {
            malformed(pos, Some(hl), &mut items);
            break;
        }
        let len = if t.len_first {
            be(&rest[..ty_at])
        } else {
            be(&rest[tb + head..hl])
        };
        let total = usize::try_from(len)
            .ok()
            .and_then(|l| l.checked_mul(unit))
            .and_then(|l| {
                if t.len_includes_header {
                    (l >= hl).then_some(l)
                } else {
                    l.checked_add(hl)
                }
            });
        let Some(total) = total.filter(|n| *n <= rest.len()) else {
            malformed(pos, total, &mut items);
            break;
        };
        let total_padded = padded(total, rest.len());
        // Verified with its full padding: the last item may lack it.
        let mut bytes = rest[..total_padded].to_vec();
        bytes.resize(padded(total, usize::MAX), 0);
        // Variants sharing a type are told apart by their value.
        let value = [&rest[tb..tb + head], &rest[hl..total]].concat();
        let variant = t.variant_for(ty, &value);
        items.push(decode_item(fd, t, ty, len, variant, &bytes, total, hl));
        pos += total_padded;
        if t.unpadded_last && pos == region.len() && total_padded > total {
            // The last item is padded after all: an empty item after it keeps that.
            items.push(TlvItem::raw([]));
        }
    }
    items
}

/// Typed decoding of one framed item (`bytes`: the item and its padding, `total`
/// octets without it), verified by re-encoding it.
#[allow(clippy::too_many_arguments)]
fn decode_item(
    fd: &FieldDef,
    t: &TlvDef,
    ty: u64,
    len: u64,
    variant: Option<&crate::ir::TlvVariant>,
    bytes: &[u8],
    total: usize,
    hl: usize,
) -> TlvItem {
    // The value octets: the head (between type and length) and what follows the length.
    let tb = usize::from(t.type_bits / 8);
    let head = &bytes[tb..tb + usize::from(t.head_octets)];
    let value = &[head, &bytes[hl..total]].concat()[..];
    let same = |item: &TlvItem| {
        encode_tlv_item(fd, t, 0, item, true, &mut Vec::new()).is_ok_and(|(b, _)| b == bytes)
    };
    let with_length = |mut item: TlvItem| {
        if same(&item) {
            return Some(item);
        }
        item.length = Some(len);
        same(&item).then_some(item)
    };
    if let Some(v) = variant {
        let who = Who {
            name: &v.name,
            id: &v.name,
            layer: 0,
        };
        if let Ok(h) = decode_fields(who, &v.fields, value, 0, &mut Vec::new()) {
            let item = TlvItem {
                ty: Some(ty),
                name: Some(v.name.clone()),
                fields: h.fields,
                ..TlvItem::default()
            };
            if let Some(item) = with_length(item) {
                return item;
            }
        }
    }
    let item = TlvItem {
        ty: Some(ty),
        name: variant.map(|v| v.name.clone()),
        data: Some(value.to_vec()),
        ..TlvItem::default()
    };
    with_length(item.clone()).unwrap_or(TlvItem {
        length: Some(len),
        ..item
    })
}

/// Decodes one header from `data` (which starts at packet offset `base`).
fn decode_header(
    def: &ProtocolDef,
    data: &[u8],
    base: usize,
    layer: usize,
    diags: &mut Vec<Diagnostic>,
) -> Result<Header, Diagnostic> {
    let who = Who {
        name: &def.name,
        id: &def.id,
        layer,
    };
    if def.synth.is_some() {
        // Generator protocols: the octets map to `fixed` mode with `data`.
        let mut fields = BTreeMap::new();
        if def.field("data").is_some() {
            fields.insert("data".to_owned(), FieldValue::Bytes(data.to_vec()));
        }
        return Ok(Header {
            fields,
            len: data.len(),
        });
    }
    decode_fields(who, &def.fields, data, base, diags)
}

/// End of the PDU announced by a total / payload length field, if the protocol has
/// one. `start` is the layer offset, `payload_start` the end of its header.
fn announced_end(
    def: &ProtocolDef,
    hdr: &Header,
    start: usize,
    payload_start: usize,
) -> Option<(String, i128)> {
    def.fields.iter().find_map(|fd| {
        let AutoCompute::Length { of, unit, adjust } = fd.auto.as_ref()? else {
            return None;
        };
        let base = match of {
            LengthOf::Header => return None,
            LengthOf::Payload => payload_start,
            LengthOf::Total => start,
        };
        let v = hdr.fields.get(&fd.name)?.as_u64()?;
        let len = (i128::from(v) - i128::from(*adjust)) * i128::from(unit.octets());
        Some((fd.name.clone(), base as i128 + len))
    })
}

/// The re-parse protocol selected by the fields up to `def`'s binding selector,
/// for a header that does not decode completely.
fn reparse_target<'a>(def: &'a ProtocolDef, data: &[u8]) -> Option<&'a str> {
    let b = def.next.as_ref()?;
    let k = def.field_index(&b.field)?;
    let who = Who {
        name: &def.name,
        id: &def.id,
        layer: 0,
    };
    let h = decode_fields(who, &def.fields[..=k], data, 0, &mut Vec::new()).ok()?;
    let rest = data.get(h.len..).unwrap_or_default();
    match b.select(|f| h.fields.get(f).and_then(FieldValue::as_u64), rest)? {
        (id, true) => Some(id),
        _ => None,
    }
}

/// Upper bound on decoded layers (defends against self-referencing bindings).
const MAX_LAYERS: usize = 64;
/// Upper bound on re-parse bindings followed for one layer.
const MAX_REPARSE: usize = 4;

fn raw_layer(bytes: &[u8]) -> Layer {
    Layer::new(RAW_PROTOCOL_ID).with("data", bytes.to_vec())
}

/// Parses `bytes`, starting with protocol `first_protocol`.
///
/// # Errors
/// Only [`CoreError::UnknownProtocol`] when `first_protocol` is not in `catalog`;
/// malformed input is reported through [`Parsed::diagnostics`].
pub fn parse(bytes: &[u8], first_protocol: &str, catalog: &dyn Resolve) -> Result<Parsed> {
    let raw = raw_protocol();
    let first = resolve(catalog, &raw, first_protocol)
        .ok_or_else(|| CoreError::UnknownProtocol(first_protocol.to_owned()))?;
    let mut diags = Vec::new();
    let mut layers: Vec<Layer> = Vec::new();
    let mut pos = 0usize;
    let mut limit = bytes.len();
    let mut next = Some(first);
    let mut reparses = 0usize;
    let mut outer: Option<&ProtocolDef> = None;

    while let Some(def) = next.take() {
        if pos >= limit && !layers.is_empty() {
            // The layer below announced `def`, but nothing is left for it.
            diags.push(short(
                Who {
                    name: &def.name,
                    id: &def.id,
                    layer: layers.len(),
                },
                pos,
            ));
            break;
        }
        let idx = layers.len();
        let mut layer_diags = Vec::new();
        let hdr = match decode_header(def, &bytes[pos..limit], pos, idx, &mut layer_diags) {
            Ok(h) => h,
            Err(d) => {
                // The header does not fit this layout, but its selector may already
                // say that another one applies (TFTP DATA under the request layout).
                let again = (reparses < MAX_REPARSE)
                    .then(|| reparse_target(def, &bytes[pos..limit]))
                    .flatten()
                    .and_then(|id| resolve(catalog, &raw, id));
                if let Some(again) = again {
                    reparses += 1;
                    next = Some(again);
                    continue;
                }
                diags.extend(layer_diags);
                diags.push(d);
                if pos < limit {
                    layers.push(raw_layer(&bytes[pos..limit]));
                }
                pos = limit;
                break;
            }
        };
        let announced = announced_end(def, &hdr, pos, pos + hdr.len);
        let selected = def.next.as_ref().and_then(|b| {
            // The payload as the next layer will see it: bounded by this layer's
            // own length field when that is consistent.
            let from = pos + hdr.len;
            let to = match &announced {
                Some((_, end)) if *end >= from as i128 && *end <= limit as i128 => {
                    usize::try_from(*end).unwrap_or(limit)
                }
                _ => limit,
            };
            let payload = bytes.get(from..to).unwrap_or_default();
            let (proto, reparse) =
                b.select(|f| hdr.fields.get(f).and_then(FieldValue::as_u64), payload)?;
            Some((resolve(catalog, &raw, proto)?, reparse))
        });
        if let Some((again, true)) = selected {
            if reparses < MAX_REPARSE {
                // Re-decode this layer as `again` (e.g. Ethernet II → 802.3).
                reparses += 1;
                next = Some(again);
                continue;
            }
        }
        reparses = 0;
        diags.extend(layer_diags);
        if idx == 0 {
            outer = Some(def);
        }
        let start = pos;
        pos += hdr.len;
        if let Some((field, end)) = announced {
            if end < pos as i128 || end > limit as i128 {
                diags.push(
                    Diagnostic::new(
                        DiagLevel::Warning,
                        keys::LENGTH_EXCEEDS_DATA,
                        format!(
                            "`{field}` announces a PDU ending at offset {end}, but the data spans {pos}..{limit}"
                        ),
                    )
                    .at_layer(idx)
                    .on_field(field)
                    .arg("len", end - start as i128)
                    .arg("available", limit - start),
                );
            } else {
                limit = usize::try_from(end).unwrap_or(limit);
            }
        }
        next = selected.map(|(d, _)| d);
        if hdr.len == 0 || layers.len() + 1 >= MAX_LAYERS {
            next = None; // guard against definitions that would loop forever
        }
        layers.push(Layer {
            protocol: def.id.clone(),
            fields: hdr.fields,
            truncate: None,
        });
        if next.is_none() && pos < limit {
            // Zero padding up to the minimum frame is left for `pad_to_min`.
            let padding = limit == bytes.len()
                && outer
                    .and_then(|d| d.min_frame_len)
                    .is_some_and(|m| bytes.len() == m as usize)
                && bytes[pos..].iter().all(|b| *b == 0);
            if !padding {
                layers.push(raw_layer(&bytes[pos..limit]));
                pos = limit;
            }
        }
    }

    let min_frame = outer.and_then(|d| d.min_frame_len).map(|m| m as usize);
    let mut pad_to_min = min_frame.is_none_or(|m| bytes.len() >= m);
    if pos < bytes.len() {
        let is_padding =
            min_frame.is_some_and(|m| bytes.len() == m && bytes[pos..].iter().all(|b| *b == 0));
        if is_padding {
            pad_to_min = true;
        } else {
            let n = bytes.len() - pos;
            diags.push(
                Diagnostic::new(
                    DiagLevel::Info,
                    keys::TRAILING_BYTES,
                    format!("{n} octets after the end of the innermost PDU"),
                )
                .at_layer(layers.len())
                .arg("len", n),
            );
            layers.push(raw_layer(&bytes[pos..]));
        }
    }

    let mut packet = Packet {
        layers,
        pad_to_min,
        fcs: None,
    };
    let built = match packet.build(catalog) {
        Ok(b) if b.bytes == bytes => b,
        other => {
            // Should not happen (decoded values always re-encode); degrade to raw.
            let why = match other {
                Ok(_) => "different octets".to_owned(),
                Err(e) => e.to_string(),
            };
            diags.push(
                Diagnostic::new(
                    DiagLevel::Error,
                    keys::UNSUPPORTED_FIELD,
                    format!("could not rebuild the decoded packet: {why}"),
                )
                .on_field("*")
                .arg("protocol", first_protocol),
            );
            packet = Packet::from_layers(vec![raw_layer(bytes)]).unpadded();
            packet.build(catalog)?
        }
    };
    diags.extend(built.diagnostics);
    Ok(Parsed {
        packet,
        layout: built.layout,
        diagnostics: diags,
        trailers: built.trailers,
    })
}

/// Parses a frame that ends with a 4-octet IEEE 802.3 FCS: the FCS is verified,
/// stripped and recorded in [`Packet::fcs`] (`Auto` when correct, `Bad` when it is
/// the complement of the correct value, `Manual` otherwise, with a checksum
/// mismatch warning).
///
/// # Errors
/// As [`parse`].
pub fn parse_with_fcs(bytes: &[u8], first_protocol: &str, catalog: &dyn Resolve) -> Result<Parsed> {
    if bytes.len() < 4 {
        let mut p = parse(bytes, first_protocol, catalog)?;
        p.diagnostics.push(
            Diagnostic::new(
                DiagLevel::Error,
                keys::SHORT_HEADER,
                "frame too short for an FCS",
            )
            .arg("protocol", "fcs")
            .arg("offset", bytes.len()),
        );
        return Ok(p);
    }
    let (frame, tail) = bytes.split_at(bytes.len() - 4);
    let got = u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]);
    let want = crc32(frame);
    let mut p = parse(frame, first_protocol, catalog)?;
    p.packet.fcs = Some(if got == want {
        FcsMode::Auto
    } else if got == !want {
        FcsMode::Bad
    } else {
        FcsMode::Manual(got)
    });
    if got != want {
        p.diagnostics.push(
            Diagnostic::new(
                DiagLevel::Warning,
                keys::CHECKSUM_MISMATCH,
                format!("FCS is {got:#010x} but the correct CRC-32 is {want:#010x}"),
            )
            .on_field("fcs")
            .arg("value", format!("{got:#x}"))
            .arg("expected", format!("{want:#x}")),
        );
    }
    let rebuilt = p.packet.build(catalog)?;
    p.trailers = rebuilt.trailers;
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        Binding, BindingEntry, FieldDef, LengthUnit, ProtocolLayer, TlvDef, TlvVariant,
    };
    use proptest::prelude::*;

    fn defs() -> Vec<ProtocolDef> {
        let mut outer = ProtocolDef::new("outer", "Outer", ProtocolLayer::L3);
        outer.fields = vec![
            FieldDef::new("hl", FieldType::UInt { bits: 4 }).with_auto(AutoCompute::Length {
                of: LengthOf::Header,
                unit: LengthUnit::Words32,
                adjust: 0,
            }),
            FieldDef::new("sig", FieldType::Int { bits: 4 }),
            FieldDef::new("proto", FieldType::UInt { bits: 8 })
                .with_auto(AutoCompute::NextProtocol),
            FieldDef::new("total", FieldType::UInt { bits: 16 }).with_auto(AutoCompute::Length {
                of: LengthOf::Total,
                unit: LengthUnit::Bytes,
                adjust: 0,
            }),
            FieldDef::new(
                "opts",
                FieldType::Bytes {
                    len: Length::FromField {
                        field: "hl".into(),
                        unit: 4,
                        adjust: -4,
                        div: 1,
                    },
                },
            ),
            FieldDef::new("pad", FieldType::Padding { align: 4 }),
        ];
        outer.next = Some(Binding::new("proto", vec![BindingEntry::new(1, "outer")]));
        outer.validate().unwrap();
        vec![outer]
    }

    #[test]
    fn nested_with_trailer() {
        let pkt = Packet::from_layers(vec![
            Layer::new("outer")
                .with("opts", vec![9u8, 9, 9])
                .with("sig", -2i8),
            Layer::new("outer"),
            Layer::new("raw").with("data", vec![7u8; 5]),
        ]);
        let mut bytes = pkt.build(&defs()).unwrap().bytes;
        assert_eq!(bytes.len(), 8 + 4 + 5);
        bytes.extend_from_slice(&[0xee, 0xee]); // trailer beyond `total`
        let parsed = parse(&bytes, "outer", &defs()).unwrap();
        let protos: Vec<_> = parsed
            .packet
            .layers
            .iter()
            .map(|l| l.protocol.as_str())
            .collect();
        assert_eq!(protos, ["outer", "outer", "raw", "raw"]);
        assert_eq!(parsed.packet.layers[0].fields["sig"], FieldValue::Int(-2));
        assert_eq!(
            parsed.packet.layers[0].fields["opts"],
            FieldValue::Bytes(vec![9, 9, 9, 0])
        );
        assert_eq!(
            parsed.packet.layers[3].fields["data"],
            FieldValue::Bytes(vec![0xee, 0xee])
        );
        assert!(parsed
            .diagnostics
            .iter()
            .any(|d| d.key == keys::TRAILING_BYTES));
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        assert_eq!(parsed.packet.build(&defs()).unwrap().bytes, bytes);
    }

    #[test]
    fn short_and_inconsistent_input() {
        let p = parse(&[0x10], "outer", &defs()).unwrap();
        assert!(p.has_errors());
        assert_eq!(p.packet.layers.len(), 1);
        assert_eq!(p.packet.layers[0].protocol, "raw");

        // hl says 15 words, total says 0xffff: diagnostics, no panic.
        let p = parse(&[0xf0, 0x00, 0xff, 0xff, 1, 2], "outer", &defs()).unwrap();
        assert!(p.has_errors());
        assert_eq!(
            p.packet.build(&defs()).unwrap().bytes,
            [0xf0, 0x00, 0xff, 0xff, 1, 2]
        );

        assert!(parse(&[], "outer", &defs()).unwrap().has_errors());
        assert!(matches!(
            parse(&[1], "nope", &defs()),
            Err(CoreError::UnknownProtocol(_))
        ));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(512))]
        #[test]
        fn never_panics_and_roundtrips(bytes in proptest::collection::vec(any::<u8>(), 0..96)) {
            let p = parse(&bytes, "outer", &defs()).unwrap();
            prop_assert_eq!(p.packet.build(&defs()).unwrap().bytes, bytes);
        }
    }

    /// IS-IS-like container: 8-bit type / length (not counting the header), with a
    /// nested sub-TLV sequence (NDP-like: 8-octet units, length includes header)
    /// and a list of IPv4 addresses; preceded by a 16-bit count-free length.
    fn tlv_defs() -> Vec<ProtocolDef> {
        let sub = TlvDef {
            type_bits: 8,
            len_bits: 8,
            len_includes_header: true,
            len_unit: LengthUnit::Words64,
            variants: vec![TlvVariant::new(
                1,
                "mac",
                vec![FieldDef::new("addr", FieldType::Mac)],
            )],
            unknown_as_raw: true,
            length: Length::ToEnd,
            fill: None,
            head_octets: 0,
            align: 0,
            unpadded_last: false,
            long_length: None,
            len_first: false,
        };
        let outer = TlvDef {
            type_bits: 8,
            len_bits: 8,
            len_includes_header: false,
            len_unit: LengthUnit::Bytes,
            variants: vec![
                TlvVariant::type_only(0, "pad"),
                TlvVariant::new(
                    1,
                    "addrs",
                    vec![FieldDef::new(
                        "list",
                        FieldType::List {
                            item: ItemDef::Field {
                                ty: Box::new(FieldType::Ipv4),
                            },
                            length: Length::ToEnd,
                        },
                    )],
                ),
                TlvVariant::new(
                    2,
                    "nested",
                    vec![
                        FieldDef::new("flags", FieldType::UInt { bits: 8 }),
                        FieldDef::new("subs", FieldType::Tlv(sub)),
                    ],
                ),
            ],
            unknown_as_raw: true,
            length: Length::FromField {
                field: "len".into(),
                unit: 1,
                adjust: 0,
                div: 1,
            },
            fill: None,
            head_octets: 0,
            align: 0,
            unpadded_last: false,
            long_length: None,
            len_first: false,
        };
        let mut p = ProtocolDef::new("tlvs", "TLVs", ProtocolLayer::L7);
        p.fields = vec![
            FieldDef::new("len", FieldType::UInt { bits: 16 }).with_auto(AutoCompute::Length {
                of: LengthOf::Header,
                unit: LengthUnit::Bytes,
                adjust: -2,
            }),
            FieldDef::new("items", FieldType::Tlv(outer)),
        ];
        p.validate().unwrap();
        vec![p]
    }

    #[test]
    fn nested_tlvs_and_lists_roundtrip() {
        let items = vec![
            TlvItem::named("addrs").with(
                "list",
                vec![
                    FieldValue::Ipv4(Ipv4Addr::new(192, 0, 2, 1)),
                    FieldValue::Ipv4(Ipv4Addr::new(192, 0, 2, 2)),
                ],
            ),
            TlvItem::named("pad"),
            TlvItem::named("nested").with("flags", 0x80u8).with(
                "subs",
                vec![
                    TlvItem::named("mac").with("addr", FieldValue::Mac([1, 2, 3, 4, 5, 6])),
                    TlvItem::with_type(9, vec![0xee; 3]),
                ],
            ),
            TlvItem::with_type(7, vec![1, 2]),
        ];
        let pkt = Packet::from_layers(vec![Layer::new("tlvs").with("items", items.clone())]);
        let built = pkt.build(&tlv_defs()).unwrap();
        assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
        #[rustfmt::skip]
        let want = [
            0, 34,                                  // len (auto)
            1, 8, 192, 0, 2, 1, 192, 0, 2, 2,       // addrs, length excludes header
            0,                                      // pad (type only)
            2, 17, 0x80,                            // nested: 1 + 8 + 8
            1, 1, 1, 2, 3, 4, 5, 6,                 // mac sub-TLV: 1 unit of 8
            9, 1, 0xee, 0xee, 0xee, 0, 0, 0,        // unknown sub-TLV, zero padded
            7, 2, 1, 2,                             // unknown, raw value
        ];
        assert_eq!(built.bytes, want);
        // Layout children: nested → subs → mac → addr.
        let f = &built.layout[0].fields[1];
        let nested = &f.children[2];
        assert_eq!(nested.name, "nested");
        let subs = nested.children.iter().find(|c| c.name == "subs").unwrap();
        let mac = &subs.children[0];
        assert_eq!(mac.bit_offset, (2 + 10 + 1 + 3) * 8);
        let addr = mac.children.iter().find(|c| c.name == "addr").unwrap();
        assert_eq!((addr.bit_offset, addr.bit_len), ((16 + 2) * 8, 48));
        let list = &f.children[0].children[2];
        assert_eq!(list.name, "list");
        assert_eq!(list.children[1].name, "[1]");
        assert_eq!(list.children[1].bit_offset, (4 + 4) * 8);

        let parsed = parse(&built.bytes, "tlvs", &tlv_defs()).unwrap();
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let FieldValue::Tlv(got) = &parsed.packet.layers[0].fields["items"] else {
            panic!("untyped");
        };
        assert_eq!(got.len(), 4);
        assert_eq!(got[0].fields, items[0].fields);
        let FieldValue::Tlv(subs) = &got[2].fields["subs"] else {
            panic!("untyped subs");
        };
        assert_eq!(subs[0].fields["addr"], FieldValue::Mac([1, 2, 3, 4, 5, 6]));
        // The padded unknown sub-TLV keeps its padding in `data`.
        assert_eq!(
            subs[1].data.as_deref(),
            Some(&[0xee, 0xee, 0xee, 0, 0, 0][..])
        );
        assert_eq!(got[3].data.as_deref(), Some(&[1, 2][..]));
        let rebuilt = parsed.packet.build(&tlv_defs()).unwrap();
        assert_eq!(rebuilt.bytes, built.bytes);
        assert_eq!(rebuilt.layout, parsed.layout);
    }

    #[test]
    fn composite_values_are_checked() {
        let bad = |v: FieldValue| {
            Packet::from_layers(vec![Layer::new("tlvs").with("items", v)])
                .build(&tlv_defs())
                .is_err()
        };
        assert!(bad(FieldValue::Tlv(vec![TlvItem::named("nope")])));
        assert!(bad(FieldValue::Tlv(vec![TlvItem::default()])));
        assert!(bad(FieldValue::Tlv(vec![
            TlvItem::named("pad").with("x", 1u8)
        ])));
        assert!(bad(FieldValue::Tlv(vec![
            TlvItem::named("nested").with("bogus", 1u8)
        ])));
        assert!(bad(FieldValue::Tlv(vec![TlvItem::with_type(300, vec![])])));
        assert!(bad(FieldValue::Tlv(vec![
            TlvItem::named("addrs").with("list", vec![FieldValue::UInt(1)])
        ])));
        assert!(bad(FieldValue::UInt(1)));
        assert!(
            !bad(FieldValue::Bytes(vec![1, 2, 3])),
            "verbatim octets are allowed"
        );
    }

    #[test]
    fn reparse_and_default_bindings() {
        let mut a = ProtocolDef::new("a", "A", ProtocolLayer::L2);
        a.fields = vec![FieldDef::new("t", FieldType::UInt { bits: 8 })];
        let mut b = Binding::new(
            "t",
            vec![
                BindingEntry::new(5, "raw"),
                BindingEntry::range(0, 9, "b").reparsing(),
            ],
        );
        a.next = Some(b.clone());
        let mut bdef = ProtocolDef::new("b", "B", ProtocolLayer::L2);
        bdef.fields = vec![
            FieldDef::new("t", FieldType::UInt { bits: 8 }),
            FieldDef::new("u", FieldType::UInt { bits: 8 }),
        ];
        b.table.clear();
        b.default = Some("raw".into());
        bdef.next = Some(b);
        let defs = vec![a, bdef];
        let p = parse(&[3, 4, 5], "a", &defs).unwrap();
        let names: Vec<_> = p
            .packet
            .layers
            .iter()
            .map(|l| l.protocol.as_str())
            .collect();
        assert_eq!(names, ["b", "raw"], "range → re-parse as b, default → raw");
        assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
        let p = parse(&[5, 4], "a", &defs).unwrap();
        let names: Vec<_> = p
            .packet
            .layers
            .iter()
            .map(|l| l.protocol.as_str())
            .collect();
        assert_eq!(names, ["a", "raw"], "exact entries win over ranges");
        let p = parse(&[42, 4], "a", &defs).unwrap();
        assert_eq!(p.packet.layers[0].protocol, "a");
    }
}
