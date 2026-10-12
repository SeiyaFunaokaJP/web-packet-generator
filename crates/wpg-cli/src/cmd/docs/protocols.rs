//! Protocol reference: `reference/protocols.md`, `reference/protocols/<id>.md`,
//! the `SUMMARY.md` entries and the links at the end of the hand-written
//! `protocols/*.md` pages, all from the catalog IR ([`wpg_protocols::catalog`]).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use wpg_core::constraint::ConstraintRule;
use wpg_core::notation::format_value;
use wpg_core::{
    AutoCompute, BindingEntry, ChecksumCoverage, ChecksumKind, DiagLevel, FieldDef, FieldType,
    FieldValue, ItemDef, Length, LengthOf, LengthUnit, Peek, ProtocolDef, ProtocolLayer, TlvDef,
};
use wpg_i18n::{try_t, Locale};
use wpg_protocols::catalog;
use wpg_template::Template;

use super::md::{code, finish, heading, num, para, plain, table, tx, txa};
use super::{read_text, src_path, without_region, Output, GENERATED};

/// Directory of the per-protocol pages, relative to `docs/<lang>/src/`.
pub(crate) const PAGE_DIR: &str = "reference/protocols";

/// Hand-written protocol notes, relative to `docs/<lang>/src/`.
const NOTES_DIR: &str = "protocols";

/// Region of the generated links in the hand-written notes pages.
const NOTES_REGION: &str = "reference-links";

/// Region of the protocol pages in `SUMMARY.md`.
const SUMMARY_REGION: &str = "protocol-pages";

/// Where sample templates are linked to (relative links would leave the book).
const REPO_BLOB: &str = "https://github.com/SeiyaFunaokaJP/web-packet-generator/blob/main";

/// At most this many templates are listed for a protocol without `protocol:<id>` samples.
const MAX_USES: usize = 10;

const LAYERS: [ProtocolLayer; 5] = [
    ProtocolLayer::L2,
    ProtocolLayer::L3,
    ProtocolLayer::L4,
    ProtocolLayer::L7,
    ProtocolLayer::Payload,
];

fn layer_key(l: ProtocolLayer) -> &'static str {
    match l {
        ProtocolLayer::L2 => "docgen-layer-l2",
        ProtocolLayer::L3 => "docgen-layer-l3",
        ProtocolLayer::L4 => "docgen-layer-l4",
        ProtocolLayer::L7 => "docgen-layer-l7",
        ProtocolLayer::Payload => "docgen-layer-payload",
    }
}

/// A bundled template: path relative to the root and its name.
#[derive(Debug, Clone)]
struct Sample {
    path: String,
    name: String,
    tagged: bool,
}

/// A hand-written notes page: file name, title, protocol ids it mentions.
#[derive(Debug)]
struct NotesPage {
    file: String,
    title: String,
    ids: BTreeSet<String>,
}

/// How a lower layer reaches a protocol.
#[derive(Debug)]
struct Inbound<'a> {
    from: &'a ProtocolDef,
    selector: String,
    value: String,
}

fn collect_templates(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        if p.is_dir() {
            collect_templates(&p, out);
        } else if [".wpg.yaml", ".wpg.yml", ".wpg.json"]
            .iter()
            .any(|s| name.ends_with(s))
        {
            out.push(p);
        }
    }
}

/// Protocol id → bundled templates using it.
fn samples(root: &Path) -> Result<BTreeMap<String, Vec<Sample>>, String> {
    let mut files = Vec::new();
    collect_templates(&root.join("templates"), &mut files);
    let mut map: BTreeMap<String, Vec<Sample>> = BTreeMap::new();
    for f in files {
        let tpl = Template::from_path(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        let rel = f
            .strip_prefix(root)
            .unwrap_or(&f)
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        let ids: BTreeSet<&str> = tpl.layers.iter().map(|l| l.protocol.as_str()).collect();
        for id in ids {
            let tagged = tpl.meta.tags.iter().any(|t| *t == format!("protocol:{id}"));
            map.entry(id.to_owned()).or_default().push(Sample {
                path: rel.clone(),
                name: tpl.meta.name.clone(),
                tagged,
            });
        }
    }
    Ok(map)
}

/// The hand-written notes pages of `locale` (the overview page excluded).
fn notes_pages(root: &Path, locale: Locale) -> Result<Vec<NotesPage>, String> {
    let dir = root.join(src_path(locale, NOTES_DIR));
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };
    let mut files: Vec<String> = rd
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".md") && n != "overview.md")
        .collect();
    files.sort();
    let known: BTreeSet<&str> = catalog().iter().map(|p| p.id.as_str()).collect();
    let mut pages = Vec::new();
    for file in files {
        let path = dir.join(&file);
        let text = read_text(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let text = without_region(&text, NOTES_REGION);
        let title = text
            .lines()
            .find_map(|l| l.strip_prefix("# "))
            .unwrap_or(&file)
            .trim()
            .to_owned();
        let ids = text
            .split('`')
            .skip(1)
            .step_by(2)
            .filter(|s| known.contains(s))
            .map(str::to_owned)
            .collect();
        pages.push(NotesPage { file, title, ids });
    }
    Ok(pages)
}

fn binding_bits(def: &ProtocolDef, field: &str) -> Option<u8> {
    def.field(field).and_then(|f| f.ty.int_bits())
}

fn entry_value(e: &BindingEntry, bits: Option<u8>) -> String {
    match e.max {
        Some(max) => format!("{} – {}", num(e.value, bits), num(max, bits)),
        None => num(e.value, bits),
    }
}

fn entry_notes(locale: Locale, e: &BindingEntry) -> String {
    let mut notes = Vec::new();
    if e.reparse {
        notes.push(tx(locale, "docgen-bind-reparse"));
    }
    if let Some(n) = e.min_len {
        notes.push(txa(
            locale,
            "docgen-bind-min-len",
            &[("octets", n.to_string())],
        ));
    }
    notes.join("; ")
}

/// "First 4 bits of the payload when `s` = 1" (after `skip` bits when set).
fn peek_text(locale: Locale, p: &Peek, field: &str, bits: Option<u8>) -> String {
    let mut args = vec![
        ("bits", p.bits.to_string()),
        ("field", code(field)),
        ("value", num(p.on, bits)),
    ];
    if p.skip == 0 {
        txa(locale, "docgen-bind-peek", &args)
    } else {
        args.push(("skip", p.skip.to_string()));
        txa(locale, "docgen-bind-peek-skip", &args)
    }
}

/// Every (lower layer → protocol) edge of the catalog, keyed by the upper id.
fn inbound(locale: Locale) -> BTreeMap<String, Vec<Inbound<'static>>> {
    let mut map: BTreeMap<String, Vec<Inbound<'static>>> = BTreeMap::new();
    for def in catalog().iter() {
        let Some(b) = &def.next else { continue };
        let bits = binding_bits(def, &b.field);
        let selector = match &b.alt_field {
            Some(alt) => format!("{} / {}", code(&b.field), code(alt)),
            None => code(&b.field),
        };
        for e in &b.table {
            let mut value = entry_value(e, bits);
            let notes = entry_notes(locale, e);
            if !notes.is_empty() {
                let _ = write!(value, " ({notes})");
            }
            map.entry(e.protocol.clone()).or_default().push(Inbound {
                from: def,
                selector: selector.clone(),
                value,
            });
        }
        if let Some(d) = &b.default {
            map.entry(d.clone()).or_default().push(Inbound {
                from: def,
                selector: code(&b.field),
                value: tx(locale, "docgen-bind-default"),
            });
        }
        if let Some(p) = &b.peek {
            let selector = peek_text(locale, p, &b.field, bits);
            for e in &p.table {
                map.entry(e.protocol.clone()).or_default().push(Inbound {
                    from: def,
                    selector: selector.clone(),
                    value: entry_value(e, Some(p.bits)),
                });
            }
            if let Some(d) = &p.default {
                map.entry(d.clone()).or_default().push(Inbound {
                    from: def,
                    selector,
                    value: tx(locale, "docgen-bind-default"),
                });
            }
        }
    }
    map
}

fn proto_link(id: &str) -> String {
    format!("[{}]({id}.md)", code(id))
}

/// Adds the outputs of `locale`.
pub(crate) fn outputs(root: &Path, locale: Locale, out: &mut Vec<Output>) -> Result<(), String> {
    let samples = samples(root)?;
    let notes = notes_pages(root, locale)?;
    let inbound = inbound(locale);
    let cat = catalog();

    // Index page.
    let mut idx = format!("{GENERATED}\n\n");
    heading(&mut idx, 1, &tx(locale, "docgen-proto-title"));
    para(
        &mut idx,
        &txa(
            locale,
            "docgen-proto-intro",
            &[("count", cat.len().to_string())],
        ),
    );
    for layer in LAYERS {
        let defs: Vec<&ProtocolDef> = cat.iter().filter(|p| p.layer == layer).collect();
        if defs.is_empty() {
            continue;
        }
        heading(&mut idx, 2, &tx(locale, layer_key(layer)));
        let rows: Vec<Vec<String>> = defs
            .iter()
            .map(|p| {
                vec![
                    format!("[{}](protocols/{}.md)", code(&p.id), p.id),
                    p.name.clone(),
                    p.fields.len().to_string(),
                    p.next.as_ref().map(|b| code(&b.field)).unwrap_or_default(),
                    notes
                        .iter()
                        .filter(|n| n.ids.contains(&p.id))
                        .map(|n| format!("[{}](../{NOTES_DIR}/{})", n.title, n.file))
                        .collect::<Vec<_>>()
                        .join(", "),
                ]
            })
            .collect();
        table(
            &mut idx,
            &[
                tx(locale, "docgen-col-id"),
                tx(locale, "docgen-col-name"),
                tx(locale, "docgen-col-fields"),
                tx(locale, "docgen-col-next-by"),
                tx(locale, "docgen-col-notes"),
            ],
            &rows,
        );
    }
    out.push(Output::whole(
        src_path(locale, "reference/protocols.md"),
        finish(idx),
    ));

    // One page per protocol.
    for def in cat.iter() {
        let page = protocol_page(
            locale,
            def,
            inbound.get(&def.id).map_or(&[][..], Vec::as_slice),
            samples.get(&def.id).map_or(&[][..], Vec::as_slice),
            &notes,
        );
        out.push(Output::whole(
            src_path(locale, &format!("{PAGE_DIR}/{}.md", def.id)),
            page,
        ));
    }

    // SUMMARY.md entries, grouped by layer (draft chapters as group titles).
    let mut summary = String::new();
    for layer in LAYERS {
        let defs: Vec<&ProtocolDef> = cat.iter().filter(|p| p.layer == layer).collect();
        if defs.is_empty() {
            continue;
        }
        let _ = writeln!(summary, "  - [{}]()", tx(locale, layer_key(layer)));
        for p in defs {
            let _ = writeln!(summary, "    - [{}]({PAGE_DIR}/{}.md)", p.id, p.id);
        }
    }
    out.push(Output::region(
        src_path(locale, "SUMMARY.md"),
        SUMMARY_REGION,
        summary,
        false,
    ));

    // Links from the hand-written notes pages.
    for n in &notes {
        let mut body = String::new();
        heading(&mut body, 2, &tx(locale, "docgen-notes-heading"));
        para(&mut body, &tx(locale, "docgen-notes-intro"));
        let links: Vec<String> = cat
            .iter()
            .filter(|p| n.ids.contains(&p.id))
            .map(|p| format!("[{}](../{PAGE_DIR}/{}.md)", code(&p.id), p.id))
            .collect();
        let _ = writeln!(body, "{}", links.join(" · "));
        out.push(Output::region(
            src_path(locale, &format!("{NOTES_DIR}/{}", n.file)),
            NOTES_REGION,
            body,
            true,
        ));
    }
    Ok(())
}

fn protocol_page(
    locale: Locale,
    def: &ProtocolDef,
    inbound: &[Inbound<'_>],
    samples: &[Sample],
    notes: &[NotesPage],
) -> String {
    let mut s = format!("{GENERATED}\n\n");
    heading(&mut s, 1, &format!("{} ({})", def.name, code(&def.id)));

    // Summary list.
    let mut facts = vec![format!(
        "**{}**: {}",
        tx(locale, "docgen-proto-layer"),
        tx(locale, layer_key(def.layer))
    )];
    let header = match def.fixed_header_bits() {
        Some(bits) if def.synth.is_none() => txa(
            locale,
            "docgen-proto-header-fixed",
            &[("octets", (bits / 8).to_string())],
        ),
        _ => tx(locale, "docgen-proto-header-variable"),
    };
    facts.push(format!(
        "**{}**: {header}",
        tx(locale, "docgen-proto-header")
    ));
    if let Some(n) = def.min_frame_len {
        facts.push(format!(
            "**{}**: {}",
            tx(locale, "docgen-proto-min-frame"),
            txa(locale, "docgen-octets", &[("octets", n.to_string())])
        ));
    }
    if def.synth.is_some() {
        facts.push(format!(
            "**{}**: {}",
            tx(locale, "docgen-proto-synth"),
            tx(locale, "docgen-proto-synth-payload")
        ));
    }
    if let Some(ph) = &def.pseudo_header {
        facts.push(format!(
            "**{}**: {}",
            tx(locale, "docgen-proto-pseudo"),
            txa(
                locale,
                "docgen-proto-pseudo-fields",
                &[("src", code(&ph.src)), ("dst", code(&ph.dst))]
            )
        ));
    }
    if let Some(fd) = &def.final_destination {
        facts.push(format!(
            "**{}**: {}",
            tx(locale, "docgen-proto-final-dst"),
            txa(
                locale,
                "docgen-proto-final-dst-text",
                &[("when", code(&fd.when)), ("list", code(&fd.list))]
            )
        ));
    }
    let pages: Vec<String> = notes
        .iter()
        .filter(|n| n.ids.contains(&def.id))
        .map(|n| format!("[{}](../../{NOTES_DIR}/{})", n.title, n.file))
        .collect();
    if !pages.is_empty() {
        facts.push(format!(
            "**{}**: {}",
            tx(locale, "docgen-proto-notes"),
            pages.join(", ")
        ));
    }
    for f in facts {
        let _ = writeln!(s, "- {f}");
    }
    s.push('\n');

    // Reached from.
    heading(&mut s, 2, &tx(locale, "docgen-proto-inbound"));
    if inbound.is_empty() {
        para(&mut s, &tx(locale, "docgen-proto-inbound-none"));
    } else {
        let rows: Vec<Vec<String>> = inbound
            .iter()
            .map(|i| vec![proto_link(&i.from.id), i.selector.clone(), i.value.clone()])
            .collect();
        table(
            &mut s,
            &[
                tx(locale, "docgen-col-lower"),
                tx(locale, "docgen-col-selector"),
                tx(locale, "docgen-col-value"),
            ],
            &rows,
        );
    }

    // Fields.
    heading(
        &mut s,
        2,
        &tx(
            locale,
            if def.synth.is_some() {
                "docgen-proto-params"
            } else {
                "docgen-proto-fields"
            },
        ),
    );
    if def.fields.is_empty() {
        para(&mut s, &tx(locale, "docgen-proto-no-fields"));
    } else {
        fields_section(&mut s, locale, &def.fields, &[], 3);
    }

    // Next layer.
    if let Some(b) = &def.next {
        heading(&mut s, 2, &tx(locale, "docgen-proto-next"));
        let mut intro = txa(
            locale,
            "docgen-proto-next-intro",
            &[("field", code(&b.field))],
        );
        if let Some(alt) = &b.alt_field {
            intro.push(' ');
            intro.push_str(&txa(
                locale,
                "docgen-proto-next-alt",
                &[("field", code(alt))],
            ));
        }
        para(&mut s, &intro);
        let bits = binding_bits(def, &b.field);
        let mut rows: Vec<Vec<String>> = b
            .table
            .iter()
            .map(|e| {
                vec![
                    entry_value(e, bits),
                    proto_link(&e.protocol),
                    entry_notes(locale, e),
                ]
            })
            .collect();
        if let Some(d) = &b.default {
            rows.push(vec![
                tx(locale, "docgen-bind-default"),
                proto_link(d),
                String::new(),
            ]);
        }
        if !rows.is_empty() {
            table(
                &mut s,
                &[
                    tx(locale, "docgen-col-value"),
                    tx(locale, "docgen-col-protocol"),
                    tx(locale, "docgen-col-note"),
                ],
                &rows,
            );
        }
        if let Some(p) = &b.peek {
            para(&mut s, &peek_text(locale, p, &b.field, bits));
            let mut rows: Vec<Vec<String>> = p
                .table
                .iter()
                .map(|e| vec![entry_value(e, Some(p.bits)), proto_link(&e.protocol)])
                .collect();
            if let Some(d) = &p.default {
                rows.push(vec![tx(locale, "docgen-bind-default"), proto_link(d)]);
            }
            table(
                &mut s,
                &[
                    tx(locale, "docgen-col-value"),
                    tx(locale, "docgen-col-protocol"),
                ],
                &rows,
            );
        }
    }

    // Constraints.
    if !def.constraints.is_empty() {
        heading(&mut s, 2, &tx(locale, "docgen-proto-constraints"));
        let rows: Vec<Vec<String>> = def
            .constraints
            .iter()
            .map(|c| {
                vec![
                    tx(locale, level_key(c.level)),
                    constraint_text(locale, &c.rule),
                ]
            })
            .collect();
        table(
            &mut s,
            &[
                tx(locale, "docgen-col-level"),
                tx(locale, "docgen-col-rule"),
            ],
            &rows,
        );
    }

    // Samples.
    heading(&mut s, 2, &tx(locale, "docgen-proto-samples"));
    let tagged: Vec<&Sample> = samples.iter().filter(|x| x.tagged).collect();
    let (list, more) = if tagged.is_empty() {
        let n = samples.len();
        (
            samples.iter().take(MAX_USES).collect::<Vec<_>>(),
            n.saturating_sub(MAX_USES),
        )
    } else {
        (tagged, 0)
    };
    if list.is_empty() {
        para(&mut s, &tx(locale, "docgen-proto-samples-none"));
    } else {
        for x in list {
            let _ = writeln!(
                s,
                "- [{}]({REPO_BLOB}/{}) — {}",
                x.name.replace(['[', ']'], ""),
                x.path,
                code(&x.path)
            );
        }
        if more > 0 {
            let _ = writeln!(
                s,
                "- {}",
                txa(
                    locale,
                    "docgen-proto-samples-more",
                    &[("count", more.to_string())]
                )
            );
        }
        s.push('\n');
    }
    finish(s)
}

fn level_key(l: DiagLevel) -> &'static str {
    match l {
        DiagLevel::Info => "docgen-level-info",
        DiagLevel::Warning => "docgen-level-warning",
        DiagLevel::Error => "docgen-level-error",
    }
}

fn span_key(of: LengthOf) -> &'static str {
    match of {
        LengthOf::Header => "docgen-span-header",
        LengthOf::Payload => "docgen-span-payload",
        LengthOf::Total => "docgen-span-total",
    }
}

fn constraint_text(locale: Locale, rule: &ConstraintRule) -> String {
    match rule {
        ConstraintRule::FieldInRange { field, min, max } => txa(
            locale,
            "docgen-c-range",
            &[
                ("field", code(field)),
                ("min", min.to_string()),
                ("max", max.to_string()),
            ],
        ),
        ConstraintRule::FieldEquals { field, value } => txa(
            locale,
            "docgen-c-equals",
            &[("field", code(field)), ("value", value.to_string())],
        ),
        ConstraintRule::FieldNotEquals { field, value } => txa(
            locale,
            "docgen-c-not-equals",
            &[("field", code(field)), ("value", value.to_string())],
        ),
        ConstraintRule::MinLength { of, octets } => txa(
            locale,
            "docgen-c-min-length",
            &[
                ("span", tx(locale, span_key(*of))),
                ("octets", octets.to_string()),
            ],
        ),
        ConstraintRule::MaxLength { of, octets } => txa(
            locale,
            "docgen-c-max-length",
            &[
                ("span", tx(locale, span_key(*of))),
                ("octets", octets.to_string()),
            ],
        ),
    }
}

/// `ihl * 4 - 20`, `(prefix_length + 7) / 8`.
fn length_expr(field: &str, unit: u32, adjust: i64, div: u32) -> String {
    let mut e = field.to_owned();
    if unit != 1 {
        let _ = write!(e, " * {unit}");
    }
    if adjust > 0 {
        let _ = write!(e, " + {adjust}");
    } else if adjust < 0 {
        let _ = write!(e, " - {}", adjust.unsigned_abs());
    }
    if div != 1 {
        if unit != 1 || adjust != 0 {
            e = format!("({e})");
        }
        let _ = write!(e, " / {div}");
    }
    e
}

fn length_text(locale: Locale, len: &Length) -> String {
    match len {
        Length::Fixed { octets } => txa(locale, "docgen-octets", &[("octets", octets.to_string())]),
        Length::FromField {
            field,
            unit,
            adjust,
            div,
        } => txa(
            locale,
            "docgen-len-from-field",
            &[("expr", code(&length_expr(field, *unit, *adjust, *div)))],
        ),
        Length::Count { field } => txa(locale, "docgen-len-count", &[("field", code(field))]),
        Length::UntilOffset {
            field,
            unit,
            adjust,
        } => txa(
            locale,
            "docgen-len-until",
            &[("expr", code(&length_expr(field, *unit, *adjust, 1)))],
        ),
        Length::ToEnd => tx(locale, "docgen-len-to-end"),
        Length::ToEndExcept { octets } => txa(
            locale,
            "docgen-len-to-end-except",
            &[("octets", octets.to_string())],
        ),
        Length::Until { terminator } => txa(
            locale,
            "docgen-len-until-terminator",
            &[(
                "octets",
                code(&wpg_core::notation::format_bytes(terminator)),
            )],
        ),
        Length::UntilTlv { type_value } => txa(
            locale,
            "docgen-len-until-tlv",
            &[("type", type_value.to_string())],
        ),
    }
}

/// Short type notation (the IR / YAML names), with the length rule of sized types.
fn type_text(locale: Locale, ty: &FieldType) -> String {
    match ty {
        FieldType::UInt { bits } => code(&format!("uint({bits})")),
        FieldType::Int { bits } => code(&format!("int({bits})")),
        FieldType::Enum { bits, .. } => code(&format!("enum({bits})")),
        FieldType::Flags { bits, .. } => code(&format!("flags({bits})")),
        FieldType::Mac => code("mac"),
        FieldType::Ipv4 => code("ipv4"),
        FieldType::Ipv6 => code("ipv6"),
        FieldType::Nsap => code("nsap"),
        FieldType::CString => code("cstring"),
        FieldType::Raw => code("raw"),
        FieldType::Padding { align } => code(&format!("padding({align})")),
        FieldType::Bytes { len } => match len {
            Length::Fixed { octets } => code(&format!("bytes({octets})")),
            other => format!("{} — {}", code("bytes"), length_text(locale, other)),
        },
        FieldType::Tlv(t) => format!("{} — {}", code("tlv"), length_text(locale, &t.length)),
        FieldType::List { item, length } => {
            let inner = match item {
                ItemDef::Field { ty } => item_type_name(ty),
                ItemDef::Struct { .. } => "struct".to_owned(),
            };
            format!(
                "{} — {}",
                code(&format!("list<{inner}>")),
                length_text(locale, length)
            )
        }
    }
}

fn item_type_name(ty: &FieldType) -> String {
    match ty {
        FieldType::UInt { bits } => format!("uint({bits})"),
        FieldType::Int { bits } => format!("int({bits})"),
        FieldType::Enum { bits, .. } => format!("enum({bits})"),
        FieldType::Flags { bits, .. } => format!("flags({bits})"),
        FieldType::Mac => "mac".into(),
        FieldType::Ipv4 => "ipv4".into(),
        FieldType::Ipv6 => "ipv6".into(),
        FieldType::Nsap => "nsap".into(),
        FieldType::CString => "cstring".into(),
        FieldType::Bytes { .. } => "bytes".into(),
        FieldType::Tlv(_) => "tlv".into(),
        FieldType::List { .. } => "list".into(),
        FieldType::Padding { .. } => "padding".into(),
        FieldType::Raw => "raw".into(),
    }
}

fn unit_key(u: LengthUnit) -> &'static str {
    match u {
        LengthUnit::Bytes => "docgen-unit-bytes",
        LengthUnit::Words16 => "docgen-unit-words16",
        LengthUnit::Words32 => "docgen-unit-words32",
        LengthUnit::Words64 => "docgen-unit-words64",
    }
}

fn auto_text(locale: Locale, auto: &AutoCompute) -> String {
    match auto {
        AutoCompute::Length { of, unit, adjust } => {
            let mut s = txa(
                locale,
                "docgen-auto-length",
                &[
                    ("span", tx(locale, span_key(*of))),
                    ("unit", tx(locale, unit_key(*unit))),
                ],
            );
            if *adjust != 0 {
                let _ = write!(s, " ({})", code(&format!("{adjust:+}")));
            }
            s
        }
        AutoCompute::Checksum {
            algorithm,
            covers,
            nonzero,
            exclude,
        } => {
            let alg = match algorithm {
                ChecksumKind::Internet => "docgen-cksum-internet",
                ChecksumKind::InternetPseudoHeader => "docgen-cksum-internet-pseudo",
                ChecksumKind::Fletcher => "docgen-cksum-fletcher",
                ChecksumKind::Crc32c => "docgen-cksum-crc32c",
                ChecksumKind::Crc32 => "docgen-cksum-crc32",
            };
            let cov = match covers {
                ChecksumCoverage::Header => "docgen-cover-header",
                ChecksumCoverage::HeaderAndPayload => "docgen-cover-all",
            };
            let mut s = txa(
                locale,
                "docgen-auto-checksum",
                &[("algorithm", tx(locale, alg)), ("covers", tx(locale, cov))],
            );
            if *nonzero {
                s.push_str("; ");
                s.push_str(&tx(locale, "docgen-auto-nonzero"));
            }
            if !exclude.is_empty() {
                s.push_str("; ");
                s.push_str(&txa(
                    locale,
                    "docgen-auto-exclude",
                    &[(
                        "fields",
                        exclude
                            .iter()
                            .map(|f| code(f))
                            .collect::<Vec<_>>()
                            .join(", "),
                    )],
                ));
            }
            s
        }
        AutoCompute::NextProtocol => tx(locale, "docgen-auto-next"),
        AutoCompute::Count { field, adjust } => {
            let mut s = txa(locale, "docgen-auto-count", &[("field", code(field))]);
            if *adjust != 0 {
                let _ = write!(s, " ({})", code(&format!("{adjust:+}")));
            }
            s
        }
        AutoCompute::Size { field } => txa(locale, "docgen-auto-size", &[("field", code(field))]),
        AutoCompute::Counter => tx(locale, "docgen-auto-counter"),
    }
}

fn default_text(locale: Locale, f: &FieldDef) -> String {
    let Some(v) = &f.default else {
        return String::new();
    };
    match v {
        FieldValue::List(items) => items_text(locale, items.len()),
        FieldValue::Tlv(items) => items_text(locale, items.len()),
        FieldValue::Struct(_) => String::new(),
        FieldValue::UInt(x) => {
            let shown = code(&format_value(v, &f.ty));
            match &f.ty {
                FieldType::Enum { variants, .. } => match variants.iter().find(|e| e.value == *x) {
                    Some(e) => format!("{shown} ({})", e.name),
                    None => shown,
                },
                _ => shown,
            }
        }
        FieldValue::Bytes(b) if b.is_empty() => code("\"\""),
        _ => code(&format_value(v, &f.ty)),
    }
}

fn items_text(locale: Locale, n: usize) -> String {
    if n == 0 {
        code("[]")
    } else {
        txa(locale, "docgen-items", &[("count", n.to_string())])
    }
}

fn desc(locale: Locale, key: Option<&String>) -> String {
    key.and_then(|k| try_t(locale, k))
        .map(|d| plain(&d))
        .unwrap_or_default()
}

/// Whether a field type has a detail section (values, bits, items).
fn has_details(ty: &FieldType) -> bool {
    match ty {
        FieldType::Enum { variants, .. } => !variants.is_empty(),
        FieldType::Flags { flags, .. } => !flags.is_empty(),
        FieldType::Tlv(_) => true,
        FieldType::List { item, .. } => match item {
            ItemDef::Struct { .. } => true,
            ItemDef::Field { ty } => has_details(ty),
        },
        _ => false,
    }
}

/// The field table of a field group, then the details of its composite fields.
fn fields_section(
    s: &mut String,
    locale: Locale,
    fields: &[FieldDef],
    path: &[&str],
    level: usize,
) {
    let rows: Vec<Vec<String>> = fields
        .iter()
        .map(|f| {
            vec![
                code(&f.name),
                type_text(locale, &f.ty),
                default_text(locale, f),
                f.auto
                    .as_ref()
                    .map(|a| auto_text(locale, a))
                    .unwrap_or_default(),
                desc(locale, f.description_key.as_ref()),
            ]
        })
        .collect();
    table(
        s,
        &[
            tx(locale, "docgen-col-field"),
            tx(locale, "docgen-col-type"),
            tx(locale, "docgen-col-default"),
            tx(locale, "docgen-col-auto"),
            tx(locale, "docgen-col-description"),
        ],
        &rows,
    );
    for f in fields.iter().filter(|f| has_details(&f.ty)) {
        let mut p = path.to_vec();
        p.push(&f.name);
        details(s, locale, &f.ty, &p, level);
    }
}

fn path_title(path: &[&str]) -> String {
    path.iter().map(|p| code(p)).collect::<Vec<_>>().join(" › ")
}

fn details(s: &mut String, locale: Locale, ty: &FieldType, path: &[&str], level: usize) {
    match ty {
        FieldType::Enum { bits, variants } => {
            heading(s, level, &path_title(path));
            let with_desc = variants.iter().any(|v| v.description.is_some());
            let rows: Vec<Vec<String>> = variants
                .iter()
                .map(|v| {
                    let mut r = vec![num(v.value, Some(*bits)), code(&v.name)];
                    if with_desc {
                        r.push(v.description.clone().unwrap_or_default());
                    }
                    r
                })
                .collect();
            let mut head = vec![
                tx(locale, "docgen-col-value"),
                tx(locale, "docgen-col-name"),
            ];
            if with_desc {
                head.push(tx(locale, "docgen-col-description"));
            }
            table(s, &head, &rows);
        }
        FieldType::Flags { bits, flags } => {
            heading(s, level, &path_title(path));
            let rows: Vec<Vec<String>> = flags
                .iter()
                .map(|f| {
                    vec![
                        f.bit.to_string(),
                        num(1u64 << f.bit, Some(*bits)),
                        code(&f.name),
                    ]
                })
                .collect();
            table(
                s,
                &[
                    tx(locale, "docgen-col-bit"),
                    tx(locale, "docgen-col-mask"),
                    tx(locale, "docgen-col-name"),
                ],
                &rows,
            );
        }
        FieldType::Tlv(t) => tlv_details(s, locale, t, path, level),
        FieldType::List { item, .. } => match item {
            ItemDef::Struct { fields } => {
                heading(s, level, &path_title(path));
                para(s, &tx(locale, "docgen-list-item"));
                fields_section(s, locale, fields, path, level + 1);
            }
            ItemDef::Field { ty } => details(s, locale, ty, path, level),
        },
        _ => {}
    }
}

fn tlv_details(s: &mut String, locale: Locale, t: &TlvDef, path: &[&str], level: usize) {
    heading(s, level, &path_title(path));
    let mut facts = vec![txa(
        locale,
        "docgen-tlv-format",
        &[
            ("type", t.type_bits.to_string()),
            ("len", t.len_bits.to_string()),
        ],
    )];
    if t.len_first {
        facts.push(tx(locale, "docgen-tlv-len-first"));
    }
    facts.push(tx(
        locale,
        if t.len_includes_header {
            "docgen-tlv-len-incl"
        } else {
            "docgen-tlv-len-excl"
        },
    ));
    if t.len_unit != LengthUnit::Bytes {
        facts.push(txa(
            locale,
            "docgen-tlv-unit",
            &[("unit", tx(locale, unit_key(t.len_unit)))],
        ));
    }
    if t.uncounted_octets > 0 {
        facts.push(txa(
            locale,
            "docgen-tlv-uncounted",
            &[("octets", t.uncounted_octets.to_string())],
        ));
    }
    if t.head_octets > 0 {
        facts.push(txa(
            locale,
            "docgen-tlv-head",
            &[("octets", t.head_octets.to_string())],
        ));
    }
    if t.align > 1 {
        let mut a = txa(
            locale,
            "docgen-tlv-align",
            &[("align", t.align.to_string())],
        );
        if t.unpadded_last {
            a.push_str("; ");
            a.push_str(&tx(locale, "docgen-tlv-unpadded-last"));
        }
        facts.push(a);
    }
    if let Some(l) = &t.long_length {
        facts.push(txa(
            locale,
            "docgen-tlv-long",
            &[
                ("flag", format!("0x{:x}", l.flag)),
                ("len", l.len_bits.to_string()),
            ],
        ));
    }
    if let Some(f) = &t.fill {
        facts.push(txa(
            locale,
            "docgen-tlv-fill",
            &[("variant", code(&f.variant)), ("field", code(&f.until))],
        ));
    }
    facts.push(tx(
        locale,
        if t.unknown_as_raw {
            "docgen-tlv-unknown-raw"
        } else {
            "docgen-tlv-unknown-stop"
        },
    ));
    for f in facts {
        let _ = writeln!(s, "- {f}");
    }
    s.push('\n');
    if t.variants.is_empty() {
        return;
    }
    let rows: Vec<Vec<String>> = t
        .variants
        .iter()
        .map(|v| {
            let mut fields = if v.type_only {
                tx(locale, "docgen-tlv-type-only")
            } else {
                v.fields
                    .iter()
                    .map(|f| code(&f.name))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let mut cond = Vec::new();
            if !v.value_prefix.is_empty() {
                cond.push(txa(
                    locale,
                    "docgen-tlv-when-prefix",
                    &[(
                        "octets",
                        code(&wpg_core::notation::format_bytes(&v.value_prefix)),
                    )],
                ));
            }
            if let Some(n) = v.value_len {
                cond.push(txa(
                    locale,
                    "docgen-tlv-when-len",
                    &[("octets", n.to_string())],
                ));
            }
            if !cond.is_empty() {
                if !fields.is_empty() {
                    fields.push_str("; ");
                }
                fields.push_str(&cond.join(", "));
            }
            vec![
                num(v.type_value, Some(t.type_bits)),
                code(&v.name),
                fields,
                desc(locale, v.description_key.as_ref()),
            ]
        })
        .collect();
    table(
        s,
        &[
            tx(locale, "docgen-col-type-value"),
            tx(locale, "docgen-col-name"),
            tx(locale, "docgen-col-value-fields"),
            tx(locale, "docgen-col-description"),
        ],
        &rows,
    );
    for v in t.variants.iter().filter(|v| !v.fields.is_empty()) {
        let mut p = path.to_vec();
        p.push(&v.name);
        heading(s, level + 1, &path_title(&p));
        fields_section(s, locale, &v.fields, &p, level + 2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_expressions() {
        assert_eq!(length_expr("ihl", 4, -20, 1), "ihl * 4 - 20");
        assert_eq!(length_expr("plen", 1, 7, 8), "(plen + 7) / 8");
        assert_eq!(length_expr("n", 1, 0, 1), "n");
    }

    #[test]
    fn every_protocol_gets_a_page() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut out = Vec::new();
        outputs(&root, Locale::En, &mut out).unwrap();
        for p in catalog().iter() {
            let path = format!("docs/en/src/{PAGE_DIR}/{}.md", p.id);
            assert!(out.iter().any(|o| o.path == path), "{path}");
        }
    }
}
