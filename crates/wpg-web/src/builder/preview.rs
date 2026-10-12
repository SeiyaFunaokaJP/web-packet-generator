//! In-browser packet build (TODO §2.5 "ブラウザ WASM プレビュー").
//!
//! [`build_preview`] compiles the template and builds one packet with **the very same
//! crates the server uses** (`wpg-core` + `wpg-protocols` + `wpg-template`, compiled
//! to WASM), so the bytes match `POST /api/render` byte for byte — the builder shows a
//! parity badge and a native test guards it. This module also owns the byte ⇄ field
//! mapping the hex view needs and the reverse-application of a direct hex edit.

use std::net::{Ipv4Addr, Ipv6Addr};

use leptos::prelude::*;
use wpg_core::notation::{format_bytes, format_mac, format_nsap, format_value};
use wpg_core::stream::{Expansion, PlanConfig};
use wpg_core::{
    CompiledGenerator, Diagnostic, FieldLayout, FieldType, FieldValue, Generator, LayerLayout,
    TrailerLayout,
};
use wpg_template::{apply_overrides, compile, FieldInput, GeneratorSpec, Template, VarOverrides};

use super::proto::{catalog, field_type};
use crate::validate::RateKind;

/// A compile / build failure that produced no bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreviewError {
    /// `error` / `warning`.
    pub(crate) level: String,
    /// Location (`layers[0].fields.dst`, ...).
    pub(crate) path: String,
    /// Human message.
    pub(crate) message: String,
}

/// Outcome of an in-browser build.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Preview {
    /// Frame bytes (empty on failure).
    pub(crate) bytes: Vec<u8>,
    /// Layer / field positions.
    pub(crate) layout: Vec<LayerLayout>,
    /// Padding / FCS after the layers.
    pub(crate) trailers: Vec<TrailerLayout>,
    /// Builder findings.
    pub(crate) diagnostics: Vec<Diagnostic>,
    /// Packets in the whole stream (`None` = unbounded).
    pub(crate) cardinality: Option<u64>,
    /// `packets * frame_len` estimate (`None` = unbounded).
    pub(crate) estimated_bytes: Option<u128>,
    /// Compile / build errors (when `bytes` is empty).
    pub(crate) errors: Vec<PreviewError>,
}

impl Preview {
    /// Whether the packet was built.
    pub(crate) fn ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// Lower-case hex of the bytes (to compare with the server).
    pub(crate) fn hex(&self) -> String {
        let mut s = String::with_capacity(self.bytes.len() * 2);
        for b in &self.bytes {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }
}

/// The latest in-browser build plus the server-parity result (context).
#[derive(Clone, Copy)]
pub(crate) struct PreviewCtx {
    /// The most recent in-browser build.
    pub(crate) preview: RwSignal<Preview>,
    /// `Some(true/false)` once the server render was compared, `None` while pending.
    pub(crate) parity: RwSignal<Option<bool>>,
}

impl PreviewCtx {
    pub(crate) fn new() -> Self {
        Self {
            preview: RwSignal::new(Preview::empty()),
            parity: RwSignal::new(None),
        }
    }

    /// The [`PreviewCtx`] context.
    pub(crate) fn use_ctx() -> PreviewCtx {
        expect_context::<PreviewCtx>()
    }
}

impl Preview {
    /// An empty (not-yet-built) preview.
    pub(crate) fn empty() -> Self {
        Self {
            bytes: Vec::new(),
            layout: Vec::new(),
            trailers: Vec::new(),
            diagnostics: Vec::new(),
            cardinality: None,
            estimated_bytes: None,
            errors: Vec::new(),
        }
    }

    /// The built value of a top-level field of `layer` (for seeding a Manual edit).
    #[must_use]
    pub(crate) fn field_value(&self, layer: usize, field: &str) -> Option<&FieldValue> {
        self.layout
            .get(layer)?
            .fields
            .iter()
            .find(|f| f.name == field)
            .map(|f| &f.value)
    }
}

fn err(level: &str, path: &str, message: String) -> PreviewError {
    PreviewError {
        level: level.to_owned(),
        path: path.to_owned(),
        message,
    }
}

/// Compiles `template` and builds packet `index` in the browser.
#[must_use]
pub(crate) fn build_preview(template: &Template, index: u64) -> Preview {
    let cat = catalog();
    let empty = Preview {
        bytes: Vec::new(),
        layout: Vec::new(),
        trailers: Vec::new(),
        diagnostics: Vec::new(),
        cardinality: None,
        estimated_bytes: None,
        errors: Vec::new(),
    };
    let vars = VarOverrides::new();
    let compiled = match compile(template, &vars, cat) {
        Ok(c) => c,
        Err(wpg_template::TemplateError::Lint(issues)) => {
            return Preview {
                errors: issues
                    .iter()
                    .map(|i| err(&i.level.to_string(), &i.path, i.message.clone()))
                    .collect(),
                ..empty
            };
        }
        Err(e) => {
            return Preview {
                errors: vec![err("error", "", e.to_string())],
                ..empty
            };
        }
    };
    let exp = match Expansion::new(&compiled.stream, None) {
        Ok(e) => e,
        Err(e) => {
            return Preview {
                errors: vec![err("error", "stream", e.to_string())],
                ..empty
            };
        }
    };
    let cardinality = exp.len();
    let idx = cardinality.map_or(index, |n| if n == 0 { 0 } else { index.min(n - 1) });
    let mut packet = compiled.packet.clone();
    if !exp.is_empty() {
        match exp.nth(idx) {
            Ok(overrides) => {
                if let Err(e) = apply_overrides(&mut packet, &overrides) {
                    return Preview {
                        errors: vec![err("error", "stream", e.to_string())],
                        cardinality,
                        ..empty
                    };
                }
            }
            Err(e) => {
                return Preview {
                    errors: vec![err("error", "stream", e.to_string())],
                    cardinality,
                    ..empty
                };
            }
        }
    }
    match packet.build(cat) {
        Ok(built) => {
            let plan = exp.plan(built.bytes.len(), &PlanConfig::default());
            Preview {
                bytes: built.bytes,
                layout: built.layout,
                trailers: built.trailers,
                diagnostics: built.diagnostics,
                cardinality,
                estimated_bytes: plan.estimated_bytes,
                errors: Vec::new(),
            }
        }
        Err(e) => Preview {
            errors: vec![err("error", "build", e.to_string())],
            cardinality,
            ..empty
        },
    }
}

// ---------------------------------------------------------------------------
// byte ⇄ field mapping
// ---------------------------------------------------------------------------

/// The deepest field of `fields` covering `bit` (recurses into TLV / list children).
fn deepest_at(fields: &[FieldLayout], bit: usize) -> Option<&FieldLayout> {
    for f in fields {
        if (f.bit_offset..f.bit_offset + f.bit_len).contains(&bit) {
            return deepest_at(&f.children, bit).or(Some(f));
        }
    }
    None
}

/// The `(bit_offset, bit_len)` of the deepest field covering octet `byte`.
#[must_use]
pub(crate) fn range_at_byte(layout: &[LayerLayout], byte: usize) -> Option<(usize, usize)> {
    let bit = byte * 8;
    for layer in layout {
        if (layer.offset..layer.offset + layer.len).contains(&byte) {
            if let Some(f) = deepest_at(&layer.fields, bit) {
                return Some((f.bit_offset, f.bit_len));
            }
        }
    }
    None
}

/// The layer index and top-level field covering octet `byte`. Outer layers span
/// their payload, so the innermost field that actually covers the byte wins.
fn top_field_at_byte(layout: &[LayerLayout], byte: usize) -> Option<(usize, &FieldLayout)> {
    let bit = byte * 8;
    for (li, layer) in layout.iter().enumerate() {
        if !(layer.offset..layer.offset + layer.len).contains(&byte) {
            continue;
        }
        if let Some(f) = layer
            .fields
            .iter()
            .find(|f| (f.bit_offset..f.bit_offset + f.bit_len).contains(&bit))
        {
            return Some((li, f));
        }
    }
    None
}

/// The layer, field name and `(bit_offset, bit_len)` of the top-level field covering
/// octet `byte` (for click-to-select in the hex view).
#[must_use]
pub(crate) fn field_at_byte(
    layout: &[LayerLayout],
    byte: usize,
) -> Option<(usize, String, usize, usize)> {
    top_field_at_byte(layout, byte).map(|(li, f)| (li, f.name.clone(), f.bit_offset, f.bit_len))
}

/// A hex edit reverse-applied to a field.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ReverseEdit {
    /// Layer index.
    pub(crate) layer: usize,
    /// Field name.
    pub(crate) field: String,
    /// New Manual value (human notation).
    pub(crate) input: FieldInput,
}

/// Reverse-applies setting octet `byte` of the built `bytes` to `new_byte`: returns
/// the Manual override on the owning field (only octet-aligned, whole-octet fields;
/// sub-byte fields have no single owner). `None` when no field owns the byte cleanly.
#[must_use]
pub(crate) fn reverse_apply(
    bytes: &[u8],
    layout: &[LayerLayout],
    byte: usize,
    new_byte: u8,
) -> Option<ReverseEdit> {
    let (li, f) = top_field_at_byte(layout, byte)?;
    if !f.bit_offset.is_multiple_of(8) || !f.bit_len.is_multiple_of(8) || f.bit_len == 0 {
        return None;
    }
    let start = f.bit_offset / 8;
    let end = start + f.bit_len / 8;
    if end > bytes.len() || !(start..end).contains(&byte) {
        return None;
    }
    let mut field_bytes = bytes[start..end].to_vec();
    field_bytes[byte - start] = new_byte;
    let ty = field_type(&layout[li].protocol, &f.name)?;
    let input = bytes_to_input(&ty, &field_bytes)?;
    Some(ReverseEdit {
        layer: li,
        field: f.name.clone(),
        input,
    })
}

/// Converts whole-octet field bytes back to human notation for its type.
fn bytes_to_input(ty: &FieldType, b: &[u8]) -> Option<FieldInput> {
    let text = |v: &FieldValue| FieldInput::Str(format_value(v, ty));
    match ty {
        FieldType::Mac if b.len() == 6 => {
            let mut m = [0u8; 6];
            m.copy_from_slice(b);
            Some(FieldInput::Str(format_mac(&m)))
        }
        FieldType::Ipv4 if b.len() == 4 => {
            let a = Ipv4Addr::new(b[0], b[1], b[2], b[3]);
            Some(text(&FieldValue::Ipv4(a)))
        }
        FieldType::Ipv6 if b.len() == 16 => {
            let mut o = [0u8; 16];
            o.copy_from_slice(b);
            Some(text(&FieldValue::Ipv6(Ipv6Addr::from(o))))
        }
        FieldType::Nsap => Some(FieldInput::Str(format_nsap(b))),
        FieldType::Bytes { .. } | FieldType::Raw | FieldType::Padding { .. } => {
            Some(FieldInput::Str(format_bytes(b)))
        }
        FieldType::UInt { bits } | FieldType::Enum { bits, .. } | FieldType::Flags { bits, .. }
            if *bits <= 64 && b.len() * 8 == usize::from(*bits) =>
        {
            let v = b.iter().fold(0u64, |a, x| (a << 8) | u64::from(*x));
            Some(text(&FieldValue::UInt(v)))
        }
        FieldType::Int { bits } if *bits <= 64 && b.len() * 8 == usize::from(*bits) => {
            let raw = b.iter().fold(0u64, |a, x| (a << 8) | u64::from(*x));
            let bits = u32::from(*bits);
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            let signed = if bits < 64 && (raw >> (bits - 1)) & 1 == 1 {
                (i128::from(raw) - (1i128 << bits)) as i64
            } else {
                raw as i64
            };
            Some(text(&FieldValue::Int(signed)))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// estimates
// ---------------------------------------------------------------------------

/// Number of distinct values a generator produces (`None` = unbounded / can't tell).
#[must_use]
pub(crate) fn gen_cardinality(spec: &GeneratorSpec, ty: &FieldType) -> Option<u64> {
    let g: Generator = match spec {
        GeneratorSpec::Text(s) => wpg_core::notation::parse_generator(s, ty).ok()?,
        GeneratorSpec::Full(g) => g.clone(),
    };
    let compiled: CompiledGenerator = g
        .compile(ty, &wpg_core::generator::CompileOptions::default())
        .ok()?;
    compiled.cardinality()
}

/// Smallest frame on the wire without FCS (mirrors `wpg_tx::rate::MIN_FRAME_LEN`).
const MIN_FRAME_LEN: usize = 60;
/// Preamble + SFD + FCS + minimum IFG (mirrors `wpg_tx::rate::WIRE_OVERHEAD`).
const WIRE_OVERHEAD: usize = 24;

/// Seconds needed to send `packets` frames of `frame_len` octets at `rate`, capped
/// by the template's `duration_secs` limit. `packets: None` is an endless stream.
/// `None` when it can't be told: a percentage of an unknown link, an unlimited rate,
/// or an endless stream without a time limit.
#[must_use]
pub(crate) fn estimate_secs(
    packets: Option<u64>,
    frame_len: usize,
    rate: RateKind,
    limit_secs: Option<f64>,
) -> Option<f64> {
    #[allow(clippy::cast_precision_loss)] // estimate only
    let wire_bits = (frame_len.max(MIN_FRAME_LEN) + WIRE_OVERHEAD) as f64 * 8.0;
    let per_packet = match rate {
        RateKind::Pps(v) => Some(1.0 / v),
        RateKind::Bps(v) => Some(wire_bits / v),
        RateKind::Ifg(d) => Some(d),
        RateKind::Percent(_) | RateKind::Unlimited => None,
    };
    #[allow(clippy::cast_precision_loss)] // estimate only
    let raw = match (packets, per_packet) {
        (Some(n), Some(t)) => Some(n as f64 * t),
        (None, _) => Some(f64::INFINITY),
        (Some(_), None) => None,
    };
    let secs = match (raw, limit_secs) {
        (Some(r), Some(l)) => r.min(l),
        (Some(r), None) => r,
        (None, _) => return None,
    };
    secs.is_finite().then_some(secs)
}

/// `Some(n)` → grouped count; `None` → the infinity sign.
#[must_use]
pub(crate) fn format_count(n: Option<u64>) -> String {
    match n {
        Some(v) => crate::fmt::group_thousands(v),
        None => "∞".to_owned(),
    }
}

/// A byte estimate as an SI size (`None` → `?`).
#[must_use]
pub(crate) fn format_size(bytes: Option<u128>) -> String {
    match bytes {
        #[allow(clippy::cast_precision_loss)] // display only
        Some(b) => crate::fmt::si(b as f64, "B"),
        None => "?".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::state::edit;

    fn eth_ip_udp() -> Template {
        let mut t = Template::new("t");
        edit::add_layer(&mut t, "ethernet2", None);
        edit::add_layer(&mut t, "ipv4", None);
        edit::add_layer(&mut t, "udp", None);
        edit::add_layer(&mut t, "payload", None);
        edit::set_field(&mut t, 1, "src", FieldInput::Str("10.0.0.1".into()));
        edit::set_field(&mut t, 1, "dst", FieldInput::Str("10.0.0.2".into()));
        t
    }

    #[test]
    fn builds_and_maps_bytes() {
        let t = eth_ip_udp();
        let p = build_preview(&t, 0);
        assert!(p.ok(), "{:?}", p.errors);
        assert!(!p.bytes.is_empty());
        // Byte 0 is the Ethernet destination MAC.
        let (off, len) = range_at_byte(&p.layout, 0).unwrap();
        assert_eq!((off, len), (0, 48));
        // The IPv4 source address (offset 14 + 12 = 26).
        let (li, f) = top_field_at_byte(&p.layout, 26).unwrap();
        assert_eq!(li, 1);
        assert_eq!(f.name, "src");
    }

    #[test]
    fn reverse_apply_ipv4_and_mac() {
        let t = eth_ip_udp();
        let p = build_preview(&t, 0);
        // Edit the last octet of the IPv4 source (byte 29).
        let e = reverse_apply(&p.bytes, &p.layout, 29, 0x63).unwrap();
        assert_eq!(e.layer, 1);
        assert_eq!(e.field, "src");
        assert_eq!(e.input, FieldInput::Str("10.0.0.99".into()));
        // Edit the first octet of the Ethernet destination MAC (byte 0).
        let e = reverse_apply(&p.bytes, &p.layout, 0, 0x02).unwrap();
        assert_eq!(e.field, "dst");
        assert!(matches!(e.input, FieldInput::Str(ref s) if s.starts_with("02:")));
    }

    #[test]
    fn model_build_matches_yaml_render() {
        // Sample 1: a static Ethernet / IPv4 / UDP / payload frame, built two ways —
        // through the document model (the edit ops the GUI uses) and from the YAML a
        // `wpg render` would read. The bytes must be identical (same wpg-core path).
        let mut model = Template::new("sample1");
        edit::add_layer(&mut model, "ethernet2", None);
        edit::set_field(
            &mut model,
            0,
            "dst",
            FieldInput::Str("ff:ff:ff:ff:ff:ff".into()),
        );
        edit::set_field(
            &mut model,
            0,
            "src",
            FieldInput::Str("02:00:00:00:00:01".into()),
        );
        edit::add_layer(&mut model, "ipv4", None);
        edit::set_field(&mut model, 1, "src", FieldInput::Str("10.0.0.1".into()));
        edit::set_field(&mut model, 1, "dst", FieldInput::Str("10.0.0.2".into()));
        edit::add_layer(&mut model, "udp", None);
        edit::add_layer(&mut model, "payload", None);
        let yaml = "schema: 1\nmeta:\n  name: sample1\nlayers:\n  - protocol: ethernet2\n    fields:\n      dst: ff:ff:ff:ff:ff:ff\n      src: 02:00:00:00:00:01\n  - protocol: ipv4\n    fields:\n      src: 10.0.0.1\n      dst: 10.0.0.2\n  - protocol: udp\n  - protocol: payload\n";
        let from_yaml = Template::from_yaml(yaml).unwrap();
        let a = build_preview(&model, 0);
        let b = build_preview(&from_yaml, 0);
        assert!(a.ok() && b.ok(), "{:?} {:?}", a.errors, b.errors);
        assert!(!a.bytes.is_empty());
        assert_eq!(a.hex(), b.hex());

        // Sample 2: a stream with an IPv4 source-address range; a mid-stream index
        // must also match between the model and the YAML.
        let mut m2 = model.clone();
        edit::set_generator(
            &mut m2,
            1,
            "src",
            GeneratorSpec::Text("10.0.0.1-10.0.0.9".into()),
        );
        let yaml2 = "schema: 1\nmeta:\n  name: sample2\nlayers:\n  - protocol: ethernet2\n    fields:\n      dst: ff:ff:ff:ff:ff:ff\n      src: 02:00:00:00:00:01\n  - protocol: ipv4\n    fields:\n      dst: 10.0.0.2\n    generators:\n      src: 10.0.0.1-10.0.0.9\n  - protocol: udp\n  - protocol: payload\n";
        let y2 = Template::from_yaml(yaml2).unwrap();
        for idx in [0u64, 3, 8] {
            let a = build_preview(&m2, idx);
            let b = build_preview(&y2, idx);
            assert!(a.ok(), "{:?}", a.errors);
            assert_eq!(a.hex(), b.hex(), "index {idx}");
        }
        assert_eq!(build_preview(&m2, 0).cardinality, Some(9));
    }

    #[test]
    fn duration_estimate() {
        // 1000 packets at 100 pps → 10 s.
        assert_eq!(
            estimate_secs(Some(1000), 64, RateKind::Pps(100.0), None),
            Some(10.0)
        );
        // 60-byte frames are 84 octets on the wire: 1e6 of them at 672 Mbps → 1 s.
        let s = estimate_secs(Some(1_000_000), 60, RateKind::Bps(672e6), None).unwrap();
        assert!((s - 1.0).abs() < 1e-9, "{s}");
        // Runts are padded to the minimum frame on the wire.
        assert_eq!(
            estimate_secs(Some(1_000_000), 20, RateKind::Bps(672e6), None),
            estimate_secs(Some(1_000_000), 60, RateKind::Bps(672e6), None)
        );
        assert_eq!(
            estimate_secs(Some(4), 64, RateKind::Ifg(0.5), None),
            Some(2.0)
        );
        // The time limit caps the estimate, and bounds an endless stream.
        assert_eq!(
            estimate_secs(Some(1000), 64, RateKind::Pps(10.0), Some(30.0)),
            Some(30.0)
        );
        assert_eq!(
            estimate_secs(None, 64, RateKind::Pps(10.0), Some(30.0)),
            Some(30.0)
        );
        assert_eq!(estimate_secs(None, 64, RateKind::Pps(10.0), None), None);
        // Link-relative and unlimited rates depend on the NIC.
        assert_eq!(
            estimate_secs(Some(10), 64, RateKind::Percent(50.0), None),
            None
        );
        assert_eq!(
            estimate_secs(Some(10), 64, RateKind::Unlimited, Some(5.0)),
            None
        );
    }

    #[test]
    fn estimate_formatting() {
        assert_eq!(format_count(Some(1000)), "1,000");
        assert_eq!(format_count(None), "∞");
        assert_eq!(format_size(Some(1_500_000)), "1.50 MB");
        assert_eq!(format_size(None), "?");
        let card = gen_cardinality(
            &GeneratorSpec::Text("10.0.0.1-10.0.0.254".into()),
            &FieldType::Ipv4,
        );
        assert_eq!(card, Some(254));
        let rnd = gen_cardinality(
            &GeneratorSpec::Full(Generator::Random {
                min: None,
                max: None,
                seed: None,
            }),
            &FieldType::UInt { bits: 16 },
        );
        assert_eq!(rnd, None);
    }
}
