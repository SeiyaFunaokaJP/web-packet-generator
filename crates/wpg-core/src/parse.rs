//! Packet parser: wire bytes → [`Packet`] (TODO §2.1 "逆生成 (import)").
//!
//! The parser decodes one header at a time, follows each protocol's [`Binding`] to
//! pick the next protocol and bounds inner PDUs by their total / payload length
//! fields. Anything it cannot decode becomes a trailing `raw` layer. It never panics
//! on arbitrary input: problems are reported as [`Diagnostic`]s.
//!
//! Every decoded field is recorded as *Manual* so that building the parsed packet
//! reproduces the input byte for byte (including wrong checksums / lengths, which
//! then show up as warnings). Auto padding that is all zeros is left on Auto.
//!
//! [`Binding`]: crate::ir::Binding

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, Ipv6Addr};

use serde::{Deserialize, Serialize};

use crate::bits::get_bits;
use crate::build::{resolve, LayerLayout};
use crate::diag::{keys, DiagLevel, Diagnostic};
use crate::error::{CoreError, Result};
use crate::ir::{
    raw_protocol, AutoCompute, FieldType, Length, LengthOf, ProtocolDef, Resolve, RAW_PROTOCOL_ID,
};
use crate::packet::{FieldValue, Layer, Packet};

/// Output of [`parse`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parsed {
    /// Decoded layers; every field is Manual with its decoded value.
    pub packet: Packet,
    /// Layer / field positions (from rebuilding [`Parsed::packet`]).
    pub layout: Vec<LayerLayout>,
    /// Parse problems followed by the builder's diagnostics for the packet.
    pub diagnostics: Vec<Diagnostic>,
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

fn short(def: &ProtocolDef, layer: usize, offset: usize) -> Diagnostic {
    Diagnostic::new(
        DiagLevel::Error,
        keys::SHORT_HEADER,
        format!(
            "input ends inside the {} header at offset {offset}",
            def.name
        ),
    )
    .at_layer(layer)
    .arg("protocol", &def.id)
    .arg("offset", offset)
}

fn take(data: &[u8], bit: usize, n: usize) -> Option<&[u8]> {
    if !bit.is_multiple_of(8) {
        return None;
    }
    let start = bit / 8;
    data.get(start..start.checked_add(n)?)
}

/// Decodes one header from `data` (which starts at packet offset `base`).
fn decode_header(
    def: &ProtocolDef,
    data: &[u8],
    base: usize,
    layer: usize,
    diags: &mut Vec<Diagnostic>,
) -> Result<Header, Diagnostic> {
    let mut fields = BTreeMap::new();
    let mut bit = 0usize;
    for fd in &def.fields {
        let at = base + bit / 8;
        let remaining = data.len().saturating_sub(bit.div_ceil(8));
        let value = match &fd.ty {
            FieldType::UInt { bits }
            | FieldType::Enum { bits, .. }
            | FieldType::Flags { bits, .. } => {
                let v =
                    get_bits(data, bit, u32::from(*bits)).ok_or_else(|| short(def, layer, at))?;
                bit += usize::from(*bits);
                FieldValue::UInt(v)
            }
            FieldType::Int { bits } => {
                let w = u32::from(*bits);
                let v = get_bits(data, bit, w).ok_or_else(|| short(def, layer, at))?;
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
                let b = take(data, bit, n).ok_or_else(|| short(def, layer, at))?;
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
            FieldType::Bytes {
                len:
                    Length::FromField {
                        field,
                        unit,
                        adjust,
                    },
            } => {
                let v = fields.get(field).and_then(FieldValue::as_u64).unwrap_or(0);
                let want = i128::from(v) * i128::from(*unit) + i128::from(*adjust);
                let want = usize::try_from(want.max(0)).unwrap_or(usize::MAX);
                let n = if want > remaining {
                    diags.push(
                        Diagnostic::new(
                            DiagLevel::Error,
                            keys::LENGTH_EXCEEDS_DATA,
                            format!(
                                "`{field}` announces {want} octets of `{}` but only {remaining} are present",
                                fd.name
                            ),
                        )
                        .at_layer(layer)
                        .on_field(field.clone())
                        .arg("len", want)
                        .arg("available", remaining),
                    );
                    remaining
                } else {
                    want
                };
                let b = take(data, bit, n).ok_or_else(|| short(def, layer, at))?;
                bit += n * 8;
                FieldValue::Bytes(b.to_vec())
            }
            FieldType::Bytes { len: Length::ToEnd } | FieldType::Raw => {
                let b = take(data, bit, remaining).ok_or_else(|| short(def, layer, at))?;
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
                let b = take(data, bit, pad.min(remaining)).ok_or_else(|| short(def, layer, at))?;
                bit += b.len() * 8;
                if b.len() == pad && b.iter().all(|x| *x == 0) {
                    continue; // identical to Auto
                }
                FieldValue::Bytes(b.to_vec())
            }
            FieldType::Nsap | FieldType::Tlv { .. } | FieldType::List { .. } => {
                return Err(Diagnostic::new(
                    DiagLevel::Error,
                    keys::UNSUPPORTED_FIELD,
                    format!("cannot decode field `{}` of {} yet", fd.name, def.name),
                )
                .at_layer(layer)
                .on_field(fd.name.clone())
                .arg("protocol", &def.id));
            }
        };
        fields.insert(fd.name.clone(), value);
    }
    if !bit.is_multiple_of(8) {
        return Err(short(def, layer, base + bit / 8));
    }
    Ok(Header {
        fields,
        len: bit / 8,
    })
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

/// Upper bound on decoded layers (defends against self-referencing bindings).
const MAX_LAYERS: usize = 64;

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

    while let Some(def) = next.take() {
        if pos >= limit && !layers.is_empty() {
            // The layer below announced `def`, but nothing is left for it.
            diags.push(short(def, layers.len(), pos));
            break;
        }
        let idx = layers.len();
        let hdr = match decode_header(def, &bytes[pos..limit], pos, idx, &mut diags) {
            Ok(h) => h,
            Err(d) => {
                diags.push(d);
                if pos < limit {
                    layers.push(raw_layer(&bytes[pos..limit]));
                }
                pos = limit;
                break;
            }
        };
        let start = pos;
        pos += hdr.len;
        if let Some((field, end)) = announced_end(def, &hdr, start, pos) {
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
        next = def.next.as_ref().and_then(|b| {
            let v = hdr.fields.get(&b.field)?.as_u64()?;
            resolve(catalog, &raw, b.protocol_for(v)?)
        });
        if hdr.len == 0 || layers.len() + 1 >= MAX_LAYERS {
            next = None; // guard against definitions that would loop forever
        }
        layers.push(Layer {
            protocol: def.id.clone(),
            fields: hdr.fields,
            truncate: None,
        });
        if next.is_none() && pos < limit {
            layers.push(raw_layer(&bytes[pos..limit]));
            pos = limit;
        }
    }
    if pos < bytes.len() {
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

    let mut packet = Packet { layers };
    let built = match packet.build(catalog) {
        Ok(b) => b,
        Err(e) => {
            // Should not happen (decoded values always fit); degrade to raw bytes.
            diags.push(Diagnostic::new(
                DiagLevel::Error,
                keys::UNSUPPORTED_FIELD,
                format!("could not rebuild the decoded packet: {e}"),
            ));
            packet = Packet {
                layers: vec![raw_layer(bytes)],
            };
            packet.build(catalog)?
        }
    };
    diags.extend(built.diagnostics);
    Ok(Parsed {
        packet,
        layout: built.layout,
        diagnostics: diags,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Binding, BindingEntry, FieldDef, LengthUnit, ProtocolLayer};
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
                    },
                },
            ),
            FieldDef::new("pad", FieldType::Padding { align: 4 }),
        ];
        outer.next = Some(Binding {
            field: "proto".into(),
            table: vec![BindingEntry {
                value: 1,
                protocol: "outer".into(),
            }],
        });
        outer.validate().unwrap();
        vec![outer]
    }

    #[test]
    fn nested_with_trailer() {
        let pkt = Packet {
            layers: vec![
                Layer::new("outer")
                    .with("opts", vec![9u8, 9, 9])
                    .with("sig", -2i8),
                Layer::new("outer"),
                Layer::new("raw").with("data", vec![7u8; 5]),
            ],
        };
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
}
