//! Fast re-rendering of one packet with per-packet field overrides (on-the-fly TX).
//!
//! [`Prebuilt::new`] builds the packet once and records where every header field
//! and every Auto checksum lives. [`Prebuilt::patch`] then produces the bytes of the
//! packet with some fields set to other (Manual) values — exactly what
//! [`Packet::build`] would produce for the packet with those fields set — by
//!
//! 1. copying the base bytes,
//! 2. writing each override in place at its recorded bit offset,
//! 3. recomputing only the Auto checksums whose input changed: the coverage of the
//!    checksum contains a patched bit range (or a checksum recomputed before it,
//!    innermost first, like the builder), or its pseudo header reads a patched
//!    address / protocol field;
//! 4. re-applying truncation, minimum-frame padding and the FCS.
//!
//! Lengths never change because only fixed-size fields are patched in place
//! (`uint` / `int` / `enum` / `flags` / `mac` / `ipv4` / `ipv6` / fixed-length
//! `bytes`). Everything else — variable-length values, TLV / list fields, generator
//! (payload) layers, a value that does not fit its field, and overrides that would
//! turn an Auto field (length / checksum / next protocol) into a Manual one — falls
//! back to a full [`Packet::build`] of the base packet with the overrides applied.
//! The result is identical either way; only the speed differs (see
//! [`PatchTarget::is_fast`], [`Prebuilt::fallbacks`]).
//!
//! Tip for streams: build the `Prebuilt` from the base packet *with the first
//! packet's overrides applied*. Every generated field is then Manual in the base
//! and later overrides never change a field's Auto/Manual status.

use crate::bits::{get_bits, mask, put_bits};
use crate::checksum::{crc32, crc32c, fletcher_checksum, InternetSum};
use crate::error::{CoreError, Result};
use crate::ir::{
    raw_protocol, AutoCompute, ChecksumCoverage, ChecksumKind, FieldType, Length, ProtocolDef,
    Resolve,
};
use crate::packet::{FcsMode, FieldValue, Packet};
use crate::stream::FieldRef;

use super::{build, resolve, zero_bits, Built, LayerLayout};

/// How a fixed-size field is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// `uint` / `enum` / `flags` (bits <= 64).
    UInt,
    /// Two's complement `int`.
    Int,
    Mac,
    Ipv4,
    Ipv6,
    /// Fixed-length `bytes` of this many octets.
    Bytes(usize),
}

/// In-place write location of a fast target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fast {
    /// Absolute bit offset in the (untruncated) packet.
    bit: usize,
    bits: usize,
    kind: Kind,
}

/// A resolved override target (see [`Prebuilt::target`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchTarget {
    layer: usize,
    field: String,
    fast: Option<Fast>,
}

impl PatchTarget {
    /// Layer index in the packet.
    #[must_use]
    pub fn layer(&self) -> usize {
        self.layer
    }

    /// Field name.
    #[must_use]
    pub fn field(&self) -> &str {
        &self.field
    }

    /// Whether overrides of this field are patched in place (otherwise every patch
    /// touching it rebuilds the whole packet).
    #[must_use]
    pub fn is_fast(&self) -> bool {
        self.fast.is_some()
    }
}

/// Where the protocol number of a pseudo header comes from.
#[derive(Debug, Clone, Copy)]
enum Proto {
    Static(u64),
    /// The lower layer's selector field (read from the current bytes).
    Field {
        bit: usize,
        bits: usize,
    },
}

/// IPv4 / IPv6 pseudo header of an upper-layer checksum.
#[derive(Debug, Clone)]
struct Pseudo {
    v6: bool,
    src_bit: usize,
    dst_bit: usize,
    proto: Proto,
    upper_len: u64,
    /// Bit ranges the pseudo header reads.
    deps: Vec<(usize, usize)>,
}

/// One Auto checksum, in the builder's fill order.
#[derive(Debug, Clone)]
struct Csum {
    kind: ChecksumKind,
    nonzero: bool,
    /// Coverage in octets `[start, end)`.
    start: usize,
    end: usize,
    /// Field position relative to `start`, in bits.
    rel_bit: usize,
    bits: usize,
    /// Later Auto checksums of the same layer and excluded fields (relative bit,
    /// bits): read as zero when the builder computes this one.
    zero_also: Vec<(usize, usize)>,
    /// `None` with [`ChecksumKind::InternetPseudoHeader`]: computed without.
    pseudo: Option<Pseudo>,
}

/// A packet built once, re-rendered cheaply with field overrides. See the module
/// docs.
#[derive(Debug, Clone)]
pub struct Prebuilt {
    /// The packet as given (overrides are applied to it for fallbacks).
    base: Packet,
    /// Definitions of the packet's protocols (fallback builds).
    defs: Vec<ProtocolDef>,
    /// Output of `base.build()`.
    built: Built,
    /// Untruncated, unpadded bytes without FCS; checksums are computed over these.
    core: Vec<u8>,
    /// Layout of `core` (field positions, also of fields cut by truncation).
    core_layout: Vec<LayerLayout>,
    /// Truncation point in `core`.
    cut: usize,
    /// Pad to this length (after truncation).
    min_frame: Option<usize>,
    fcs: Option<FcsMode>,
    csums: Vec<Csum>,
    /// Output buffer.
    out: Vec<u8>,
    scratch: Vec<u8>,
    dirty: Vec<(usize, usize)>,
    fallbacks: u64,
}

fn overlaps(a: (usize, usize), b: (usize, usize)) -> bool {
    a.0 < b.1 && b.0 < a.1
}

fn kind_of(ty: &FieldType) -> Option<(Kind, usize)> {
    Some(match ty {
        FieldType::UInt { bits } | FieldType::Enum { bits, .. } | FieldType::Flags { bits, .. } => {
            (Kind::UInt, usize::from(*bits))
        }
        FieldType::Int { bits } => (Kind::Int, usize::from(*bits)),
        FieldType::Mac => (Kind::Mac, 48),
        FieldType::Ipv4 => (Kind::Ipv4, 32),
        FieldType::Ipv6 => (Kind::Ipv6, 128),
        FieldType::Bytes {
            len: Length::Fixed { octets },
        } => (Kind::Bytes(*octets as usize), *octets as usize * 8),
        _ => return None,
    })
}

fn put_octets(buf: &mut [u8], bit: usize, bytes: &[u8]) {
    if bit.is_multiple_of(8) {
        let at = bit / 8;
        buf[at..at + bytes.len()].copy_from_slice(bytes);
    } else {
        for (k, b) in bytes.iter().enumerate() {
            put_bits(buf, bit + k * 8, 8, u64::from(*b));
        }
    }
}

fn get_octets<const N: usize>(buf: &[u8], bit: usize) -> [u8; N] {
    let mut out = [0u8; N];
    if bit.is_multiple_of(8) {
        out.copy_from_slice(&buf[bit / 8..bit / 8 + N]);
    } else {
        for (k, o) in out.iter_mut().enumerate() {
            #[allow(clippy::cast_possible_truncation)] // 8 bits
            let b = get_bits(buf, bit + k * 8, 8).unwrap_or(0) as u8;
            *o = b;
        }
    }
    out
}

/// Writes `v` at `f`; `false` when the value does not fit / match (the builder
/// would reject it, so the caller falls back to a full build for the error).
fn write_fast(buf: &mut [u8], f: Fast, v: &FieldValue) -> bool {
    let width = u32::try_from(f.bits).unwrap_or(u32::MAX);
    match (f.kind, v) {
        (Kind::UInt, FieldValue::UInt(x)) => *x <= mask(width) && put_bits(buf, f.bit, width, *x),
        (Kind::Int, FieldValue::Int(x)) => {
            if width == 0 || width > 64 {
                return false;
            }
            let half = 1i128 << (width - 1);
            if !(-half..half).contains(&i128::from(*x)) {
                return false;
            }
            #[allow(clippy::cast_sign_loss)] // two's complement reinterpretation
            let raw = *x as u64;
            put_bits(buf, f.bit, width, raw)
        }
        (Kind::Mac, FieldValue::Mac(m)) => {
            put_octets(buf, f.bit, m);
            true
        }
        (Kind::Ipv4, FieldValue::Ipv4(a)) => {
            put_octets(buf, f.bit, &a.octets());
            true
        }
        (Kind::Ipv6, FieldValue::Ipv6(a)) => {
            put_octets(buf, f.bit, &a.octets());
            true
        }
        (Kind::Bytes(n), FieldValue::Bytes(b)) if b.len() == n => {
            put_octets(buf, f.bit, b);
            true
        }
        _ => false,
    }
}

impl Prebuilt {
    /// Builds `packet` once and prepares in-place patching.
    ///
    /// # Errors
    /// Whatever [`Packet::build`] returns for `packet`.
    pub fn new(packet: &Packet, catalog: &dyn Resolve) -> Result<Prebuilt> {
        let raw = raw_protocol();
        let mut defs: Vec<ProtocolDef> = Vec::new();
        for l in &packet.layers {
            let d = resolve(catalog, &raw, &l.protocol)
                .ok_or_else(|| CoreError::UnknownProtocol(l.protocol.clone()))?;
            if !defs.iter().any(|x| x.id == d.id) {
                defs.push(d.clone());
            }
        }
        let built = build(packet, &defs)?;
        let mut core_pkt = packet.clone();
        for l in &mut core_pkt.layers {
            l.truncate = None;
        }
        core_pkt.pad_to_min = false;
        core_pkt.fcs = None;
        let core = build(&core_pkt, &defs)?;
        let total = core.bytes.len();

        let mut cut = total;
        for (i, l) in packet.layers.iter().enumerate() {
            if let Some(n) = l.truncate {
                cut = cut.min(core.layout[i].offset.saturating_add(n));
            }
        }
        let min_frame = if packet.pad_to_min {
            packet
                .layers
                .first()
                .and_then(|l| defs.protocol(&l.protocol))
                .and_then(|d| d.min_frame_len)
                .map(|m| m as usize)
        } else {
            None
        };

        let def_of = |i: usize| -> &ProtocolDef {
            defs.protocol(&packet.layers[i].protocol)
                .expect("resolved above")
        };
        let field_bit = |i: usize, name: &str| -> Option<(usize, usize)> {
            core.layout[i]
                .fields
                .iter()
                .find(|f| f.name == name)
                .map(|f| (f.bit_offset, f.bit_len))
        };

        // Auto checksums in fill order: innermost layer first, field order within.
        let mut csums = Vec::new();
        for i in (0..packet.layers.len()).rev() {
            let def = def_of(i);
            if def.synth.is_some() {
                continue;
            }
            let lay = &core.layout[i];
            let pending: Vec<(usize, usize, &AutoCompute)> = def
                .fields
                .iter()
                .filter(|fd| !packet.layers[i].fields.contains_key(&fd.name))
                .filter_map(|fd| match &fd.auto {
                    Some(a @ AutoCompute::Checksum { .. }) => {
                        field_bit(i, &fd.name).map(|(b, n)| (b - lay.offset * 8, n, a))
                    }
                    _ => None,
                })
                .collect();
            for (k, &(rel_bit, bits, auto)) in pending.iter().enumerate() {
                let AutoCompute::Checksum {
                    algorithm,
                    covers,
                    nonzero,
                    exclude,
                } = auto
                else {
                    continue;
                };
                // Later checksums of the layer are still zero, excluded fields
                // always read as zero.
                let mut zero_also: Vec<(usize, usize)> =
                    pending[k + 1..].iter().map(|p| (p.0, p.1)).collect();
                zero_also.extend(
                    exclude
                        .iter()
                        .filter_map(|n| field_bit(i, n))
                        .map(|(b, n)| (b - lay.offset * 8, n)),
                );
                let end = match covers {
                    ChecksumCoverage::Header => lay.offset + lay.header_len,
                    ChecksumCoverage::HeaderAndPayload => total,
                };
                let pseudo = if *algorithm == ChecksumKind::InternetPseudoHeader {
                    Self::pseudo(packet, &defs, &core, i, total)
                } else {
                    None
                };
                csums.push(Csum {
                    kind: *algorithm,
                    nonzero: *nonzero,
                    start: lay.offset,
                    end,
                    rel_bit,
                    bits,
                    zero_also,
                    pseudo,
                });
            }
        }
        let cap = core.bytes.len().max(min_frame.unwrap_or(0)) + 4;
        Ok(Prebuilt {
            base: packet.clone(),
            defs,
            built,
            core: core.bytes,
            core_layout: core.layout,
            cut,
            min_frame,
            fcs: packet.fcs,
            csums,
            out: Vec::with_capacity(cap),
            scratch: Vec::with_capacity(cap),
            dirty: Vec::new(),
            fallbacks: 0,
        })
    }

    /// Mirrors `Ctx::pseudo_header` of the builder, as positions.
    fn pseudo(
        packet: &Packet,
        defs: &[ProtocolDef],
        core: &Built,
        i: usize,
        total: usize,
    ) -> Option<Pseudo> {
        let def_of = |j: usize| defs.protocol(&packet.layers[j].protocol);
        let j = (0..i)
            .rev()
            .find(|&j| def_of(j).is_some_and(|d| d.pseudo_header.is_some()))?;
        let lower = def_of(j)?;
        let ph = lower.pseudo_header.as_ref()?;
        let binding = lower.next.as_ref()?;
        let field = |name: &str| {
            core.layout[j]
                .fields
                .iter()
                .find(|f| f.name == name)
                .map(|f| (f.bit_offset, f.bit_len))
        };
        let (src_bit, src_len) = field(&ph.src)?;
        let (dst_bit, dst_len) = field(&ph.dst)?;
        let v6 = match (&lower.field(&ph.src)?.ty, &lower.field(&ph.dst)?.ty) {
            (FieldType::Ipv4, FieldType::Ipv4) => false,
            (FieldType::Ipv6, FieldType::Ipv6) => true,
            _ => return None,
        };
        let mut deps = vec![(src_bit, src_bit + src_len), (dst_bit, dst_bit + dst_len)];
        let selector = match lower.field(&binding.field).map(|f| &f.ty) {
            Some(FieldType::UInt { .. } | FieldType::Enum { .. } | FieldType::Flags { .. }) => {
                field(&binding.field)
            }
            _ => None,
        };
        let from_binding = if j + 1 == i {
            None
        } else {
            binding.value_for(&packet.layers[i].protocol)
        };
        let proto = match (from_binding, selector) {
            (Some(v), _) => Proto::Static(v),
            (None, Some((bit, bits))) => {
                deps.push((bit, bit + bits));
                Proto::Field { bit, bits }
            }
            (None, None) => Proto::Static(0),
        };
        Some(Pseudo {
            v6,
            src_bit,
            dst_bit,
            proto,
            upper_len: (total - core.layout[i].offset) as u64,
            deps,
        })
    }

    /// The build of the base packet (layout, diagnostics).
    #[must_use]
    pub fn built(&self) -> &Built {
        &self.built
    }

    /// The base packet.
    #[must_use]
    pub fn packet(&self) -> &Packet {
        &self.base
    }

    /// Number of patches that needed a full rebuild so far.
    #[must_use]
    pub fn fallbacks(&self) -> u64 {
        self.fallbacks
    }

    /// Resolves an override target once (for repeated [`Self::patch_targets`]).
    ///
    /// # Errors
    /// [`CoreError::UnknownLayer`] if `r.layer` matches no layer (see
    /// [`Packet::layer_index`]). Unknown field names are reported by the patch
    /// itself, like the builder does.
    pub fn target(&self, r: &FieldRef) -> Result<PatchTarget> {
        let layer = self
            .base
            .layer_index(&r.layer)
            .ok_or_else(|| CoreError::UnknownLayer(r.layer.clone()))?;
        let def = self
            .defs
            .protocol(&self.base.layers[layer].protocol)
            .expect("resolved in new");
        let fast = (|| {
            if def.synth.is_some() {
                return None;
            }
            let fd = def.field(&r.field)?;
            // Turning an Auto field Manual changes more than its own bytes.
            let auto = matches!(
                fd.auto,
                Some(
                    AutoCompute::Length { .. }
                        | AutoCompute::Checksum { .. }
                        | AutoCompute::NextProtocol
                        | AutoCompute::Count { .. }
                        | AutoCompute::Size { .. }
                )
            );
            if auto && !self.base.layers[layer].fields.contains_key(&r.field) {
                return None;
            }
            // A TLV fill target resizes the filler items.
            let fills = def.fields.iter().any(|f| {
                matches!(&f.ty, FieldType::Tlv(t) if t.fill.as_ref().is_some_and(|x| x.until == r.field))
            });
            if fills {
                return None;
            }
            let (kind, bits) = kind_of(&fd.ty)?;
            let lay = self.core_field(layer, &r.field)?;
            (lay.1 == bits).then_some(Fast {
                bit: lay.0,
                bits,
                kind,
            })
        })();
        Ok(PatchTarget {
            layer,
            field: r.field.clone(),
            fast,
        })
    }

    fn core_field(&self, layer: usize, name: &str) -> Option<(usize, usize)> {
        self.core_layout[layer]
            .fields
            .iter()
            .find(|f| f.name == name)
            .map(|f| (f.bit_offset, f.bit_len))
    }

    /// Bytes of the base packet with `overrides` applied as Manual values —
    /// identical to building the base packet with those fields set.
    ///
    /// # Errors
    /// [`CoreError::UnknownLayer`] for a layer reference that matches nothing;
    /// otherwise what [`Packet::build`] returns for the overridden packet.
    pub fn patch(&mut self, overrides: &[(FieldRef, FieldValue)]) -> Result<&[u8]> {
        let mut targets = Vec::with_capacity(overrides.len());
        for (r, _) in overrides {
            targets.push(self.target(r)?);
        }
        let values: Vec<FieldValue> = overrides.iter().map(|(_, v)| v.clone()).collect();
        self.patch_targets(&targets, &values)
    }

    /// [`Self::patch`] with pre-resolved targets (`values[i]` goes to
    /// `targets[i]`; extra entries of the longer slice are ignored).
    ///
    /// # Errors
    /// See [`Self::patch`].
    pub fn patch_targets(
        &mut self,
        targets: &[PatchTarget],
        values: &[FieldValue],
    ) -> Result<&[u8]> {
        if self.try_fast(targets, values) {
            return Ok(&self.out);
        }
        self.fallbacks += 1;
        self.rebuild(targets, values)
    }

    fn try_fast(&mut self, targets: &[PatchTarget], values: &[FieldValue]) -> bool {
        if targets.iter().any(|t| t.fast.is_none()) {
            return false;
        }
        self.out.clear();
        self.out.extend_from_slice(&self.core);
        self.dirty.clear();
        for (t, v) in targets.iter().zip(values) {
            let f = t.fast.expect("checked above");
            if !write_fast(&mut self.out, f, v) {
                return false;
            }
            self.dirty.push((f.bit, f.bit + f.bits));
        }
        if !self.dirty.is_empty() {
            for k in 0..self.csums.len() {
                let c = &self.csums[k];
                let cov = (c.start * 8, c.end * 8);
                let hit = self.dirty.iter().any(|d| overlaps(*d, cov))
                    || c.pseudo.as_ref().is_some_and(|p| {
                        p.deps
                            .iter()
                            .any(|dep| self.dirty.iter().any(|d| overlaps(*d, *dep)))
                    });
                if hit {
                    self.recompute(k);
                }
            }
        }
        self.finish();
        true
    }

    fn recompute(&mut self, k: usize) {
        let c = &self.csums[k];
        self.scratch.clear();
        self.scratch.extend_from_slice(&self.out[c.start..c.end]);
        let width = |b: usize| u32::try_from(b).unwrap_or(64);
        put_bits(&mut self.scratch, c.rel_bit, width(c.bits), 0);
        for &(b, n) in &c.zero_also {
            zero_bits(&mut self.scratch, b, n);
        }
        let v: u64 = match c.kind {
            ChecksumKind::Fletcher => u64::from(fletcher_checksum(&self.scratch, c.rel_bit / 8)),
            ChecksumKind::Crc32 => u64::from(crc32(&self.scratch).swap_bytes()),
            ChecksumKind::Crc32c => u64::from(crc32c(&self.scratch).swap_bytes()),
            ChecksumKind::Internet | ChecksumKind::InternetPseudoHeader => {
                let mut sum = InternetSum::new();
                if let Some(p) = &c.pseudo {
                    let proto = match p.proto {
                        Proto::Static(v) => v,
                        Proto::Field { bit, bits } => {
                            get_bits(&self.out, bit, width(bits)).unwrap_or(0)
                        }
                    };
                    #[allow(clippy::cast_possible_truncation)] // wire widths, wrap on purpose
                    if p.v6 {
                        sum.add(&get_octets::<16>(&self.out, p.src_bit));
                        sum.add(&get_octets::<16>(&self.out, p.dst_bit));
                        sum.add(&(p.upper_len as u32).to_be_bytes());
                        sum.add(&[0, 0, 0, proto as u8]);
                    } else {
                        sum.add(&get_octets::<4>(&self.out, p.src_bit));
                        sum.add(&get_octets::<4>(&self.out, p.dst_bit));
                        sum.add(&[0, proto as u8]);
                        sum.add(&(p.upper_len as u16).to_be_bytes());
                    }
                }
                sum.add(&self.scratch);
                let mut s = sum.finish();
                if c.nonzero && s == 0 {
                    s = 0xffff;
                }
                u64::from(s)
            }
        };
        let abs = c.start * 8 + c.rel_bit;
        let bits = c.bits;
        put_bits(&mut self.out, abs, width(bits), v & mask(width(bits)));
        self.dirty.push((abs, abs + bits));
    }

    /// Truncation, padding and FCS (as in the builder).
    fn finish(&mut self) {
        self.out.truncate(self.cut);
        if let Some(min) = self.min_frame {
            if self.out.len() < min {
                self.out.resize(min, 0);
            }
        }
        let fcs = match self.fcs {
            Some(FcsMode::Auto) => Some(crc32(&self.out)),
            Some(FcsMode::Bad) => Some(!crc32(&self.out)),
            Some(FcsMode::Manual(v)) => Some(v),
            Some(FcsMode::None) | None => None,
        };
        if let Some(v) = fcs {
            self.out.extend_from_slice(&v.to_le_bytes());
        }
    }

    fn rebuild(&mut self, targets: &[PatchTarget], values: &[FieldValue]) -> Result<&[u8]> {
        let mut p = self.base.clone();
        for (t, v) in targets.iter().zip(values) {
            p.layers[t.layer].fields.insert(t.field.clone(), v.clone());
        }
        let built = build(&p, &self.defs)?;
        self.out = built.bytes;
        Ok(&self.out)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{defs, packet};
    use super::*;
    use crate::ir::{FieldDef, ProtocolLayer};
    use crate::packet::Layer;
    use proptest::prelude::*;
    use std::net::Ipv4Addr;

    fn rebuild(p: &Packet, ov: &[(FieldRef, FieldValue)], defs: &[ProtocolDef]) -> Result<Vec<u8>> {
        let mut p = p.clone();
        for (r, v) in ov {
            let i = p
                .layer_index(&r.layer)
                .ok_or_else(|| CoreError::UnknownLayer(r.layer.clone()))?;
            p.layers[i].fields.insert(r.field.clone(), v.clone());
        }
        p.build(&defs.to_vec()).map(|b| b.bytes)
    }

    #[test]
    fn patches_address_and_fixes_both_checksums() {
        let d = defs();
        let mut pre = Prebuilt::new(&packet(), &d).unwrap();
        assert_eq!(
            pre.patch(&[]).unwrap(),
            &packet().build(&d).unwrap().bytes[..]
        );
        let ov = vec![(
            FieldRef::new("outer", "src"),
            FieldValue::Ipv4(Ipv4Addr::new(192, 0, 2, 7)),
        )];
        let t = pre.target(&ov[0].0).unwrap();
        assert!(t.is_fast());
        let got = pre.patch(&ov).unwrap().to_vec();
        assert_eq!(got, rebuild(&packet(), &ov, &d).unwrap());
        assert_eq!(pre.fallbacks(), 0);
        // Auto -> Manual status change falls back but stays identical.
        let ov = vec![(FieldRef::new("inner", "csum"), FieldValue::UInt(0x1234))];
        assert!(!pre.target(&ov[0].0).unwrap().is_fast());
        assert_eq!(
            pre.patch(&ov).unwrap(),
            &rebuild(&packet(), &ov, &d).unwrap()[..]
        );
        assert_eq!(pre.fallbacks(), 1);
        // Errors match the builder.
        let bad = vec![(FieldRef::new("outer", "id"), FieldValue::UInt(1 << 20))];
        assert!(pre.patch(&bad).is_err());
        assert!(matches!(
            pre.patch(&[(FieldRef::new("nope", "x"), FieldValue::UInt(0))]),
            Err(CoreError::UnknownLayer(_))
        ));
        assert!(pre
            .patch(&[(FieldRef::new("outer", "bogus"), FieldValue::UInt(0))])
            .is_err());
    }

    #[test]
    fn truncation_padding_and_fcs() {
        let d = defs();
        let mut p = packet().with_fcs(FcsMode::Auto);
        p.layers[1].truncate = Some(2);
        let mut pre = Prebuilt::new(&p, &d).unwrap();
        let ov = vec![(
            FieldRef::new("0", "dst"),
            FieldValue::Ipv4(Ipv4Addr::new(1, 2, 3, 4)),
        )];
        assert_eq!(pre.patch(&ov).unwrap(), &rebuild(&p, &ov, &d).unwrap()[..]);
        assert_eq!(pre.fallbacks(), 0);
    }

    #[test]
    fn same_layer_checksums_see_later_ones_zeroed() {
        let mut p = ProtocolDef::new("osi", "OSI-like", ProtocolLayer::L3);
        p.fields = vec![
            FieldDef::new("a", FieldType::UInt { bits: 16 }),
            FieldDef::new("ck", FieldType::UInt { bits: 16 }).with_auto(AutoCompute::Checksum {
                algorithm: ChecksumKind::Fletcher,
                covers: ChecksumCoverage::Header,
                nonzero: false,
                exclude: Vec::new(),
            }),
            FieldDef::new("b", FieldType::Int { bits: 12 }),
            FieldDef::new("pad", FieldType::UInt { bits: 4 }),
            FieldDef::new("crc", FieldType::UInt { bits: 32 }).with_auto(AutoCompute::Checksum {
                algorithm: ChecksumKind::Crc32c,
                covers: ChecksumCoverage::HeaderAndPayload,
                nonzero: false,
                exclude: Vec::new(),
            }),
        ];
        let d = vec![p];
        let pkt = Packet::from_layers(vec![
            Layer::new("osi"),
            Layer::new("raw").with("data", b"123456789".to_vec()),
        ]);
        let mut pre = Prebuilt::new(&pkt, &d).unwrap();
        for (a, b) in [(1u64, -5i64), (0xffff, 2047), (7, -2048)] {
            let ov = vec![
                (FieldRef::new("osi", "a"), FieldValue::UInt(a)),
                (FieldRef::new("osi", "b"), FieldValue::Int(b)),
            ];
            assert_eq!(
                pre.patch(&ov).unwrap(),
                &rebuild(&pkt, &ov, &d).unwrap()[..]
            );
        }
        // Variable-length payload override: fallback, still identical.
        let ov = vec![(FieldRef::new("raw", "data"), FieldValue::Bytes(vec![9; 20]))];
        assert_eq!(
            pre.patch(&ov).unwrap(),
            &rebuild(&pkt, &ov, &d).unwrap()[..]
        );
        assert_eq!(pre.fallbacks(), 1);
    }

    proptest! {
        #[test]
        fn patch_equals_rebuild(
            src in any::<u32>(), dst in any::<u32>(), id in 0u64..0x2_0000,
            ver in 0u64..20, pick in 0u8..16, manual_csum in proptest::option::of(any::<u16>()),
        ) {
            let d = defs();
            let mut base = packet();
            if let Some(c) = manual_csum {
                base.layers[0].fields.insert("csum".into(), FieldValue::UInt(u64::from(c)));
            }
            let mut pre = Prebuilt::new(&base, &d).unwrap();
            let all = [
                (FieldRef::new("outer", "src"), FieldValue::Ipv4(Ipv4Addr::from(src))),
                (FieldRef::new("outer", "dst"), FieldValue::Ipv4(Ipv4Addr::from(dst))),
                (FieldRef::new("outer", "id"), FieldValue::UInt(id)),
                (FieldRef::new("outer", "ver"), FieldValue::UInt(ver)),
            ];
            let ov: Vec<_> = all.iter().enumerate()
                .filter(|(k, _)| pick & (1 << k) != 0)
                .map(|(_, x)| x.clone())
                .collect();
            let want = rebuild(&base, &ov, &d);
            let got = pre.patch(&ov).map(<[u8]>::to_vec);
            prop_assert_eq!(got.is_ok(), want.is_ok());
            if let (Ok(g), Ok(w)) = (got, want) {
                prop_assert_eq!(g, w);
            }
        }
    }
}
