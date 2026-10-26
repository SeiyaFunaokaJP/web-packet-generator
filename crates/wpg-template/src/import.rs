//! Reverse generation (spec §2.1 "逆生成"): captures / hex dumps → templates.
//!
//! * [`import_capture`] reads pcap / pcapng ([`crate::pcapread`]), parses every frame
//!   with [`wpg_core::parse()`] and either merges them into **one** template
//!   ([`ImportMode::Merge`]) or yields one template per packet
//!   ([`ImportMode::PerPacket`]);
//! * [`import_hex`] does the same for a pasted hex string / hexdump.
//!
//! ## Semantics
//!
//! The parser records every decoded field as Manual. The importer then tries to put
//! fields with an Auto rule (lengths, checksums, next-protocol ids) back on `auto`:
//! a field becomes `auto` iff dropping it and rebuilding the packet reproduces the
//! captured octets exactly, so a wrong checksum / length stays explicit. All other
//! fields are written in human notation (`192.168.1.1`, `syn|ack`, `ipv4`, ...).
//!
//! In `Merge` mode the first packet is the base and, with
//! [`ImportOptions::detect_generators`], the other packets are used to infer
//! generators *best effort*: for every fixed-size scalar field (integers, enums,
//! flags, MAC / IPv4 / IPv6 addresses) that differs across packets, an arithmetic
//! progression becomes a `range` (with `step`), a periodic sequence of at most 16
//! values (or at most 16 distinct values) a `list`, anything else a `random`
//! between the observed minimum and maximum. Inference only runs when all packets
//! have the same layer stack; the result is only exact for the first two kinds.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, Ipv6Addr};

use indexmap::IndexMap;
use thiserror::Error;
use wpg_core::generator::{CompileOptions, Generator};
use wpg_core::notation::{format_bytes, format_value};
use wpg_core::stream::Expansion;
use wpg_core::{
    parse, raw_protocol, AutoCompute, CoreError, DiagLevel, FieldType, FieldValue, ItemDef, Packet,
    ProtocolDef, Resolve, TlvDef, TlvItem, RAW_PROTOCOL_ID,
};

use crate::compile::{apply_overrides, compile, VarOverrides};
use crate::pcapread::{read_capture, CaptureError};
use crate::{FieldInput, GeneratorSpec, Template, TemplateError, TemplateLayer};

/// Maximum number of values of a generated `list`.
const MAX_LIST: usize = 16;

/// How the packets of a capture become templates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ImportMode {
    /// One template per packet.
    PerPacket,
    /// One template from the first packet (+ inferred generators).
    #[default]
    Merge,
}

/// Options of [`import_capture`] / [`import_frames`] / [`import_hex`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportOptions {
    /// Per packet or merged.
    pub mode: ImportMode,
    /// Read at most this many packets.
    pub max_packets: Option<usize>,
    /// Protocol the frames start with.
    pub first_protocol: String,
    /// Template name (`Merge`) or name prefix (`PerPacket`: `<prefix>-0001`).
    pub name_prefix: String,
    /// Infer generators from the packets (`Merge` only).
    pub detect_generators: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            mode: ImportMode::Merge,
            max_packets: None,
            first_protocol: "ethernet2".to_owned(),
            name_prefix: "imported".to_owned(),
            detect_generators: true,
        }
    }
}

/// Errors of the importers.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ImportError {
    /// The capture file could not be read.
    #[error("{0}")]
    Capture(#[from] CaptureError),
    /// The hex text is not valid.
    #[error("invalid hex input: {0}")]
    Hex(String),
    /// The input holds no packet.
    #[error("the input contains no packets")]
    Empty,
    /// Parsing failed (unknown first protocol, ...).
    #[error("{0}")]
    Core(#[from] CoreError),
    /// Writing a template failed.
    #[error("{0}")]
    Template(#[from] TemplateError),
}

/// A finding while importing (localisable through its Fluent `key`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportNote {
    /// 0-based packet index the note refers to, if any.
    pub packet: Option<usize>,
    /// Fluent key (`import-*` or a wpg-core `diag-*` key).
    pub key: String,
    /// Named Fluent arguments.
    pub args: BTreeMap<String, String>,
    /// English text.
    pub message: String,
}

impl ImportNote {
    fn new(packet: Option<usize>, key: &str, message: String) -> Self {
        Self {
            packet,
            key: key.to_owned(),
            args: BTreeMap::new(),
            message,
        }
    }

    fn arg(mut self, name: &str, value: impl std::fmt::Display) -> Self {
        self.args.insert(name.to_owned(), value.to_string());
        self
    }
}

/// Result of an import.
#[derive(Debug, Clone)]
pub struct ImportResult {
    /// The templates (exactly one in `Merge` mode).
    pub templates: Vec<Template>,
    /// Number of packets read.
    pub packets: usize,
    /// Problems and approximations.
    pub notes: Vec<ImportNote>,
}

/// Imports a pcap / pcapng file (`source` is used in the description only).
///
/// # Errors
/// Unreadable captures, a non-Ethernet link type, an empty capture or an unknown
/// `first_protocol`.
pub fn import_capture(
    bytes: &[u8],
    source: &str,
    opts: &ImportOptions,
    catalog: &dyn Resolve,
) -> Result<ImportResult, ImportError> {
    let frames: Vec<Vec<u8>> = read_capture(bytes)?.into_iter().map(|f| f.data).collect();
    import_frames(&frames, source, opts, catalog)
}

/// Imports already extracted frames.
///
/// # Errors
/// An empty input or an unknown `first_protocol`.
pub fn import_frames(
    frames: &[Vec<u8>],
    source: &str,
    opts: &ImportOptions,
    catalog: &dyn Resolve,
) -> Result<ImportResult, ImportError> {
    let take = opts.max_packets.unwrap_or(usize::MAX).min(frames.len());
    let frames = &frames[..take];
    if frames.is_empty() {
        return Err(ImportError::Empty);
    }
    let mut notes = Vec::new();
    let mut parsed: Vec<Option<Packet>> = Vec::with_capacity(frames.len());
    for (i, f) in frames.iter().enumerate() {
        let p = parse(f, &opts.first_protocol, catalog)?;
        let first_only = opts.mode == ImportMode::Merge && i > 0;
        let mut errors = 0usize;
        for d in &p.diagnostics {
            let level_ok = matches!(d.level, DiagLevel::Error | DiagLevel::Warning);
            if !level_ok {
                continue;
            }
            errors += 1;
            if !first_only {
                notes.push(ImportNote {
                    packet: Some(i),
                    key: d.key.clone(),
                    args: d.args.clone(),
                    message: d.message.clone(),
                });
            }
        }
        if first_only && errors > 0 {
            notes.push(
                ImportNote::new(
                    Some(i),
                    "import-packet-problems",
                    format!("packet {} has {errors} parse problem(s)", i + 1),
                )
                .arg("index", i + 1)
                .arg("count", errors),
            );
        }
        if p.packet.layers.is_empty() {
            notes.push(
                ImportNote::new(
                    Some(i),
                    "import-packet-skipped",
                    format!("packet {} could not be decoded and was skipped", i + 1),
                )
                .arg("index", i + 1),
            );
            parsed.push(None);
        } else {
            parsed.push(Some(p.packet));
        }
    }
    let n = frames.len();
    let templates = match opts.mode {
        ImportMode::PerPacket => {
            let width = n.to_string().len().max(4);
            let mut out = Vec::new();
            for (i, (p, raw)) in parsed.iter().zip(frames).enumerate() {
                let Some(p) = p else { continue };
                let name = format!("{}-{:0width$}", opts.name_prefix, i + 1);
                let mut t = base_template(&name, p, raw, Some(i), catalog, &mut notes);
                t.meta.description = Some(format!("Imported from {source}, packet {}", i + 1));
                t.meta.descriptions.insert(
                    "en".into(),
                    format!("Imported from {source}, packet {}", i + 1),
                );
                t.meta.descriptions.insert(
                    "ja".into(),
                    format!("{source} のパケット {} からインポート", i + 1),
                );
                out.push(t);
            }
            out
        }
        ImportMode::Merge => {
            let Some(Some(first)) = parsed.first() else {
                return Err(ImportError::Empty);
            };
            let mut t = base_template(
                &opts.name_prefix,
                first,
                &frames[0],
                Some(0),
                catalog,
                &mut notes,
            );
            let desc = format!("Imported from {source} ({n} packets)");
            t.meta.description = Some(desc.clone());
            t.meta.descriptions.insert("en".into(), desc);
            t.meta.descriptions.insert(
                "ja".into(),
                format!("{source} からインポート ({n} パケット)"),
            );
            t.stream.count = Some(n as u64);
            if opts.detect_generators && n > 1 {
                infer_generators(&mut t, &parsed, catalog, &mut notes);
            }
            vec![t]
        }
    };
    Ok(ImportResult {
        templates,
        packets: n,
        notes,
    })
}

/// Imports a hex string / dump as a one-packet template (see [`parse_hex`]).
///
/// # Errors
/// Invalid hex, an empty input or an unknown `first_protocol`.
pub fn import_hex(
    text: &str,
    first_protocol: &str,
    catalog: &dyn Resolve,
) -> Result<ImportResult, ImportError> {
    let bytes = parse_hex(text)?;
    let opts = ImportOptions {
        first_protocol: first_protocol.to_owned(),
        name_prefix: "hex-import".to_owned(),
        detect_generators: false,
        ..ImportOptions::default()
    };
    let mut r = import_frames(&[bytes], "pasted hex", &opts, catalog)?;
    for t in &mut r.templates {
        t.stream.count = None;
        t.meta.description = Some("Imported from pasted hex".to_owned());
        t.meta
            .descriptions
            .insert("en".into(), "Imported from pasted hex".into());
        t.meta
            .descriptions
            .insert("ja".into(), "貼り付けた Hex からインポート".into());
    }
    Ok(r)
}

// --------------------------------------------------------------------------------
// hex parsing
// --------------------------------------------------------------------------------

fn is_hex(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn decode_hex(s: &str) -> Result<Vec<u8>, ImportError> {
    if !s.len().is_multiple_of(2) {
        return Err(ImportError::Hex(format!(
            "odd number of hex digits in `{s}`"
        )));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|_| ImportError::Hex(format!("`{}` is not hex", &s[i..i + 2])))
        })
        .collect()
}

/// Splits a line into tokens (whitespace, `,`, `:` and `-` separated hex groups).
fn hex_tokens(part: &str) -> Result<Vec<u8>, ImportError> {
    let mut out = Vec::new();
    for tok in part.split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | ':' | '-')) {
        if tok.is_empty() {
            continue;
        }
        let digits = tok
            .strip_prefix("0x")
            .or_else(|| tok.strip_prefix("0X"))
            .unwrap_or(tok);
        if !is_hex(digits) {
            return Err(ImportError::Hex(format!("`{tok}` is not hex")));
        }
        out.extend(decode_hex(digits)?);
    }
    Ok(out)
}

/// Offset column of a dump line: 4-8 hex digits, optionally followed by `:`.
fn offset_of(line: &str) -> Option<(usize, &str, bool)> {
    let t = line.trim_start();
    let end = t.find(char::is_whitespace).unwrap_or(t.len());
    let (tok, rest) = t.split_at(end);
    let (digits, colon) = match tok.strip_suffix(':') {
        Some(d) => (d, true),
        None => (tok, false),
    };
    if (4..=8).contains(&digits.len()) && is_hex(digits) {
        Some((usize::from_str_radix(digits, 16).ok()?, rest, colon))
    } else {
        None
    }
}

/// Hex part of a dump line: cut at `|` or a run of three spaces (ASCII column); tokens
/// are read until the first non-hex token or 16 octets.
fn dump_hex_part(rest: &str) -> Vec<u8> {
    let rest = rest.split('|').next().unwrap_or("").trim_start();
    let cut = rest.find("   ").unwrap_or(rest.len());
    let mut bytes = Vec::new();
    for tok in rest[..cut].split(|c: char| c.is_whitespace() || c == ',') {
        if tok.is_empty() {
            continue;
        }
        let digits = tok.strip_prefix("0x").unwrap_or(tok);
        if !is_hex(digits) || !digits.len().is_multiple_of(2) || bytes.len() + digits.len() / 2 > 16
        {
            break;
        }
        if let Ok(b) = decode_hex(digits) {
            bytes.extend(b);
        }
    }
    bytes
}

/// Decodes pasted hex into octets. Accepted forms:
///
/// * Wireshark "Copy as Hex Stream" (`ffffffffffff0011...`),
/// * spaced / comma separated bytes, with or without `0x` (`ff ff 00`, `0xff, 0x00`),
/// * dumps with an offset column (`hexdump -C`, `xxd`, Wireshark "Copy as Hex + ASCII"),
///   whose offsets must start at 0 and increase consistently; the ASCII column is
///   dropped.
///
/// Lines starting with `#` or `//` are ignored.
///
/// # Errors
/// [`ImportError::Hex`] for non-hex characters or an odd digit count;
/// [`ImportError::Empty`] when no octet was found.
pub fn parse_hex(text: &str) -> Result<Vec<u8>, ImportError> {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("//"))
        .collect();
    let dump = try_dump(&lines)?;
    let bytes = match dump {
        Some(b) => b,
        None => {
            let mut out = Vec::new();
            for l in &lines {
                out.extend(hex_tokens(l)?);
            }
            out
        }
    };
    if bytes.is_empty() {
        Err(ImportError::Empty)
    } else {
        Ok(bytes)
    }
}

/// Decodes `lines` as an offset dump, or `None` if they do not look like one.
fn try_dump(lines: &[&str]) -> Result<Option<Vec<u8>>, ImportError> {
    if lines.is_empty() {
        return Ok(None);
    }
    let mut out: Vec<u8> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some((off, rest, colon)) = offset_of(line) else {
            return Ok(None);
        };
        if i == 0 && lines.len() == 1 {
            // a lone line is only a dump if it is clearly marked as one
            let marked = colon || line.contains('|') || rest.starts_with("  ");
            if !marked {
                return Ok(None);
            }
        }
        if off != out.len() {
            return Ok(None);
        }
        out.extend(dump_hex_part(rest));
    }
    Ok(Some(out))
}

// --------------------------------------------------------------------------------
// packet → template
// --------------------------------------------------------------------------------

fn lookup<'a>(catalog: &'a dyn Resolve, raw: &'a ProtocolDef, id: &str) -> Option<&'a ProtocolDef> {
    catalog
        .protocol(id)
        .or_else(|| (id == RAW_PROTOCOL_ID).then_some(raw))
}

/// Auto-capable fields (by layer) that can be left on `auto` without changing the
/// octets of `packet`.
fn auto_fields(
    packet: &Packet,
    target: &[u8],
    catalog: &dyn Resolve,
    raw: &ProtocolDef,
) -> Vec<Vec<String>> {
    let mut work = packet.clone();
    let mut out = vec![Vec::new(); packet.layers.len()];
    for (li, layer) in packet.layers.iter().enumerate() {
        let Some(def) = lookup(catalog, raw, &layer.protocol) else {
            continue;
        };
        for name in layer.fields.keys() {
            let auto_rule = def.field(name).and_then(|f| f.auto.as_ref());
            if !matches!(
                auto_rule,
                Some(
                    AutoCompute::Length { .. }
                        | AutoCompute::Checksum { .. }
                        | AutoCompute::NextProtocol
                        | AutoCompute::Count { .. }
                        | AutoCompute::Size { .. }
                )
            ) {
                continue;
            }
            let Some(saved) = work.layers[li].fields.remove(name) else {
                continue;
            };
            match work.build(catalog) {
                Ok(b) if b.bytes == target => out[li].push(name.clone()),
                _ => {
                    work.layers[li].fields.insert(name.clone(), saved);
                }
            }
        }
    }
    out
}

fn base_template(
    name: &str,
    packet: &Packet,
    raw_frame: &[u8],
    index: Option<usize>,
    catalog: &dyn Resolve,
    notes: &mut Vec<ImportNote>,
) -> Template {
    let raw = raw_protocol();
    let autos = auto_fields(packet, raw_frame, catalog, &raw);
    let mut t = Template::new(name);
    t.meta.tags = vec!["imported".to_owned()];
    t.packet.pad_to_min = packet.pad_to_min;
    for (li, layer) in packet.layers.iter().enumerate() {
        let mut tl = TemplateLayer {
            protocol: layer.protocol.clone(),
            ..TemplateLayer::default()
        };
        if let Some(def) = lookup(catalog, &raw, &layer.protocol) {
            for fd in &def.fields {
                let Some(v) = layer.fields.get(&fd.name) else {
                    continue;
                };
                let is_empty = |v: &FieldValue| {
                    matches!(v, FieldValue::Tlv(i) if i.is_empty())
                        || matches!(v, FieldValue::List(i) if i.is_empty())
                };
                // Unset = the default, when that is empty too (BGP `nlri` is not).
                let empty = is_empty(v) && fd.default.as_ref().is_none_or(is_empty);
                if empty || matches!(fd.ty, FieldType::Padding { .. }) {
                    continue;
                }
                let input = if autos[li].contains(&fd.name) {
                    FieldInput::Str("auto".to_owned())
                } else {
                    value_to_input(v, &fd.ty)
                };
                tl.fields.insert(fd.name.clone(), input);
            }
        }
        t.layers.push(tl);
    }
    // verify: the template must rebuild the captured octets
    let rebuilt = compile(&t, &VarOverrides::new(), catalog)
        .ok()
        .and_then(|c| c.packet.build(catalog).ok())
        .map(|b| b.bytes);
    if rebuilt.as_deref() != Some(raw_frame) {
        notes.push(ImportNote::new(
            index,
            "import-not-reproducible",
            "the imported template does not rebuild the captured octets; \
                 the frame was stored as raw data"
                .to_owned(),
        ));
        t.layers = vec![TemplateLayer {
            protocol: RAW_PROTOCOL_ID.to_owned(),
            fields: IndexMap::from([("data".to_owned(), FieldInput::Str(format_bytes(raw_frame)))]),
            generators: IndexMap::new(),
        }];
        t.packet.pad_to_min = false;
    }
    t
}

fn text(s: String) -> FieldInput {
    FieldInput::Str(s)
}

/// A value in human notation for the template file.
fn value_to_input(v: &FieldValue, ty: &FieldType) -> FieldInput {
    match (v, ty) {
        (FieldValue::UInt(x), FieldType::Enum { variants, .. }) => variants
            .iter()
            .find(|e| e.value == *x && !e.name.eq_ignore_ascii_case("auto"))
            .map_or(FieldInput::Uint(*x), |e| text(e.name.clone())),
        (FieldValue::UInt(x), FieldType::Flags { flags, bits }) => {
            let named: u64 = flags.iter().fold(0, |a, f| a | 1u64 << f.bit);
            if *x == 0 || *x & !named != 0 || *bits > 64 {
                FieldInput::Uint(*x)
            } else {
                let names: Vec<&str> = flags
                    .iter()
                    .filter(|f| x >> f.bit & 1 == 1)
                    .map(|f| f.name.as_str())
                    .collect();
                text(names.join("|"))
            }
        }
        (FieldValue::UInt(x), _) => FieldInput::Uint(*x),
        (FieldValue::Int(x), _) => u64::try_from(*x).map_or(FieldInput::Int(*x), FieldInput::Uint),
        (FieldValue::Tlv(items), FieldType::Tlv(def)) => {
            FieldInput::List(items.iter().map(|i| tlv_input(i, def)).collect())
        }
        (FieldValue::List(items), FieldType::List { item, .. }) => FieldInput::List(
            items
                .iter()
                .map(|e| match (e, item) {
                    (FieldValue::Struct(m), ItemDef::Struct { fields }) => {
                        FieldInput::Map(group_input(m, fields))
                    }
                    (e, ItemDef::Field { ty }) => value_to_input(e, ty),
                    (e, _) => text(format_value(e, &FieldType::Raw)),
                })
                .collect(),
        ),
        (other, ty) => text(format_value(other, ty)),
    }
}

fn group_input(
    m: &BTreeMap<String, FieldValue>,
    fields: &[wpg_core::FieldDef],
) -> IndexMap<String, FieldInput> {
    let mut out = IndexMap::new();
    for fd in fields {
        if let Some(v) = m.get(&fd.name) {
            out.insert(fd.name.clone(), value_to_input(v, &fd.ty));
        }
    }
    out
}

const TLV_KEYS: [&str; 6] = ["name", "type", "length", "data", "raw", "fields"];

fn tlv_input(item: &TlvItem, def: &TlvDef) -> FieldInput {
    if let Some(raw) = &item.raw {
        return FieldInput::Map(IndexMap::from([(
            "raw".to_owned(),
            text(format_bytes(raw)),
        )]));
    }
    let variant = match (&item.name, item.ty) {
        (Some(n), _) => def.variant_by_name(n),
        (None, Some(t)) => def.variant_by_type(t),
        (None, None) => None,
    };
    let fields = variant.map(|v| group_input(&item.fields, &v.fields));
    let simple = item.length.is_none()
        && item.data.is_none()
        && item
            .ty
            .is_none_or(|t| variant.is_some_and(|v| v.type_value == t));
    if let (Some(name), Some(v), true) = (&item.name, variant, simple) {
        let fields = fields.unwrap_or_default();
        if fields.is_empty() {
            return text(name.clone());
        }
        if !TLV_KEYS.contains(&name.as_str()) {
            let value = match (v.fields.as_slice(), fields.len()) {
                ([only], 1) if fields.contains_key(&only.name) => {
                    fields.into_iter().next().map(|(_, v)| v)
                }
                _ => Some(FieldInput::Map(fields)),
            };
            return FieldInput::Map(IndexMap::from([(
                name.clone(),
                value.unwrap_or(FieldInput::Null),
            )]));
        }
        return FieldInput::Map(IndexMap::from([
            ("name".to_owned(), text(name.clone())),
            ("fields".to_owned(), FieldInput::Map(fields)),
        ]));
    }
    // explicit form
    let mut m = IndexMap::new();
    match (&item.name, variant) {
        (Some(n), _) => {
            m.insert("name".to_owned(), text(n.clone()));
        }
        (None, Some(v)) => {
            m.insert("name".to_owned(), text(v.name.clone()));
        }
        (None, None) => {}
    }
    if let Some(t) = item.ty {
        m.insert("type".to_owned(), FieldInput::Uint(t));
    } else if item.name.is_none() && variant.is_none() {
        // cannot happen for parsed items; keep the octets rather than drop them
    }
    if let Some(l) = item.length {
        m.insert("length".to_owned(), FieldInput::Uint(l));
    }
    if let Some(d) = &item.data {
        m.insert("data".to_owned(), text(format_bytes(d)));
    }
    if let Some(f) = fields.filter(|f| !f.is_empty() && item.data.is_none()) {
        m.insert("fields".to_owned(), FieldInput::Map(f));
    }
    FieldInput::Map(m)
}

// --------------------------------------------------------------------------------
// generator inference
// --------------------------------------------------------------------------------

fn to_num(v: &FieldValue) -> Option<i128> {
    Some(match v {
        FieldValue::UInt(x) => i128::from(*x),
        FieldValue::Int(x) => i128::from(*x),
        FieldValue::Mac(m) => m.iter().fold(0i128, |a, b| a << 8 | i128::from(*b)),
        FieldValue::Ipv4(a) => i128::from(u32::from(*a)),
        FieldValue::Ipv6(a) => i128::try_from(u128::from(*a)).ok()?,
        _ => return None,
    })
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn num_text(n: i128, ty: &FieldType) -> String {
    match ty {
        FieldType::Mac => {
            let b = (n as u64).to_be_bytes();
            wpg_core::notation::format_mac(&[b[2], b[3], b[4], b[5], b[6], b[7]])
        }
        FieldType::Ipv4 => Ipv4Addr::from(n as u32).to_string(),
        FieldType::Ipv6 => Ipv6Addr::from(n as u128).to_string(),
        _ => n.to_string(),
    }
}

fn is_scalar(ty: &FieldType) -> bool {
    matches!(
        ty,
        FieldType::UInt { .. }
            | FieldType::Int { .. }
            | FieldType::Enum { .. }
            | FieldType::Flags { .. }
            | FieldType::Mac
            | FieldType::Ipv4
            | FieldType::Ipv6
    )
}

/// Smallest period `p <= MAX_LIST` with `v[k] == v[k % p]` for all `k`.
fn period(v: &[i128]) -> Option<usize> {
    (1..=MAX_LIST.min(v.len())).find(|&p| v.iter().enumerate().all(|(k, x)| *x == v[k % p]))
}

/// Which kind of generator [`infer`] chose (for the notes).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Inferred {
    Exact,
    Approximate,
}

fn infer(values: &[i128], ty: &FieldType) -> Option<(Generator, Inferred)> {
    let first = *values.first()?;
    if values.iter().all(|v| *v == first) {
        return None;
    }
    let t = |n: i128| num_text(n, ty);
    let d = values[1] - first;
    if d != 0 && values.windows(2).all(|w| w[1] - w[0] == d) {
        return Some((
            Generator::Range {
                start: t(first),
                end: t(*values.last()?),
                step: u64::try_from(d.unsigned_abs()).ok()?,
            },
            Inferred::Exact,
        ));
    }
    if let Some(p) = period(values) {
        return Some((
            Generator::List {
                values: values[..p].iter().map(|v| t(*v)).collect(),
            },
            Inferred::Exact,
        ));
    }
    let mut distinct: Vec<i128> = Vec::new();
    for v in values {
        if !distinct.contains(v) {
            distinct.push(*v);
            if distinct.len() > MAX_LIST {
                break;
            }
        }
    }
    if distinct.len() <= MAX_LIST {
        return Some((
            Generator::List {
                values: distinct.iter().map(|v| t(*v)).collect(),
            },
            Inferred::Approximate,
        ));
    }
    Some((
        Generator::Random {
            min: Some(t(*values.iter().min()?)),
            max: Some(t(*values.iter().max()?)),
            seed: None,
        },
        Inferred::Approximate,
    ))
}

fn infer_generators(
    t: &mut Template,
    parsed: &[Option<Packet>],
    catalog: &dyn Resolve,
    notes: &mut Vec<ImportNote>,
) {
    let packets: Vec<&Packet> = parsed.iter().flatten().collect();
    let Some(first) = packets.first() else { return };
    let stack =
        |p: &Packet| -> Vec<String> { p.layers.iter().map(|l| l.protocol.clone()).collect() };
    let want = stack(first);
    let different = packets.iter().filter(|p| stack(p) != want).count();
    if different > 0 || packets.len() != parsed.len() {
        notes.push(
            ImportNote::new(
                None,
                "import-mixed-stacks",
                format!(
                    "{} packet(s) have a different layer stack than the first one; \
                     no generators were inferred",
                    different + (parsed.len() - packets.len())
                ),
            )
            .arg("count", different + (parsed.len() - packets.len())),
        );
        return;
    }
    let raw = raw_protocol();
    let mut assigned: Vec<(usize, String, FieldType, Generator)> = Vec::new();
    for (li, layer) in first.layers.iter().enumerate() {
        let Some(def) = lookup(catalog, &raw, &layer.protocol) else {
            continue;
        };
        for fd in &def.fields {
            let manual = t.layers[li]
                .fields
                .get(&fd.name)
                .is_some_and(|f| !f.is_auto());
            if !manual || !is_scalar(&fd.ty) {
                continue;
            }
            let values: Option<Vec<i128>> = packets
                .iter()
                .map(|p| p.layers[li].fields.get(&fd.name).and_then(to_num))
                .collect();
            let Some((generator, kind)) = values.as_deref().and_then(|v| infer(v, &fd.ty)) else {
                continue;
            };
            let opts = CompileOptions::default();
            if generator.compile(&fd.ty, &opts).is_err() {
                continue;
            }
            if kind == Inferred::Approximate {
                let path = format!("layers[{li}].generators.{}", fd.name);
                notes.push(
                    ImportNote::new(
                        None,
                        "import-generator-approximate",
                        format!("`{path}` only approximates the captured values"),
                    )
                    .arg("value", path),
                );
            }
            assigned.push((li, fd.name.clone(), fd.ty.clone(), generator));
        }
    }
    for (li, name, _, generator) in &assigned {
        t.layers[*li].fields.shift_remove(name);
        t.layers[*li]
            .generators
            .insert(name.clone(), GeneratorSpec::Full(generator.clone()));
    }
    // A compile / expansion sanity check; drop the generators if the template broke.
    let ok = compile(t, &VarOverrides::new(), catalog)
        .ok()
        .is_some_and(|c| {
            let mut p = c.packet.clone();
            Expansion::new(&c.stream, None)
                .ok()
                .and_then(|e| e.nth(0).ok())
                .is_some_and(|ov| apply_overrides(&mut p, &ov).is_ok() && p.build(catalog).is_ok())
        });
    if !ok {
        for (li, name, _, _) in &assigned {
            t.layers[*li].generators.shift_remove(name);
            if let Some(v) = first.layers[*li].fields.get(name) {
                if let Some(fd) =
                    lookup(catalog, &raw, &first.layers[*li].protocol).and_then(|d| d.field(name))
                {
                    t.layers[*li]
                        .fields
                        .insert(name.clone(), value_to_input(v, &fd.ty));
                }
            }
        }
        notes.push(ImportNote::new(
            None,
            "import-generators-dropped",
            "the inferred generators produced an invalid stream and were dropped".to_owned(),
        ));
    }
}
