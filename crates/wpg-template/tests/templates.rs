//! End-to-end tests: sample templates, compile, lint, stream expansion, render and
//! the JSON Schema golden.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use wpg_core::stream::Expansion;
use wpg_core::FieldValue;
use wpg_protocols::catalog;
use wpg_template::render::{packet_count, render_pcapng, render_with};
use wpg_template::{
    apply_overrides, compile, lint, LintLevel, Template, TemplateError, VarOverrides,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sample_files() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                walk(&p, out);
            } else if p.to_string_lossy().ends_with(".wpg.yaml") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(&root().join("templates"), &mut out);
    out
}

fn yaml(src: &str) -> Template {
    Template::from_yaml_str(src).unwrap()
}

fn no_vars() -> VarOverrides {
    VarOverrides::new()
}

#[test]
fn every_sample_compiles_lints_clean_and_renders() {
    let files = sample_files();
    assert!(files.len() >= 5, "found {files:?}");
    for f in files {
        let t = Template::from_path(&f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let issues = lint(&t, catalog());
        // `templates/malformed/` is deliberately broken (checked by tests/samples.rs)
        if !f.components().any(|c| c.as_os_str() == "malformed") {
            assert!(issues.is_empty(), "{}: {issues:#?}", f.display());
        }
        let c = compile(&t, &no_vars(), catalog()).unwrap();
        let mut n = 0;
        render_with(&c, catalog(), Some(3), &mut |_, bytes| {
            assert!(bytes.len() >= 42, "{}: {} bytes", f.display(), bytes.len());
            n += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(n, 3);
        // the canonical YAML form round-trips
        assert_eq!(
            Template::from_yaml_str(&t.to_yaml_string().unwrap()).unwrap(),
            t
        );
    }
}

#[test]
fn udp_range_sample_yields_distinct_source_ips() {
    let t =
        Template::from_path(root().join("templates/examples/eth-ipv4-udp-range.wpg.yaml")).unwrap();
    let c = compile(&t, &no_vars(), catalog()).unwrap();
    let exp = Expansion::new(&c.stream, None).unwrap();
    assert_eq!(exp.len(), Some(1278));
    let mut srcs = BTreeSet::new();
    for i in 0..exp.len().unwrap() {
        let mut p = c.packet.clone();
        apply_overrides(&mut p, &exp.nth(i).unwrap()).unwrap();
        let FieldValue::Ipv4(ip) = p.layers[1].fields["src"] else {
            panic!("src is not an IPv4 value");
        };
        srcs.insert(ip);
    }
    assert_eq!(srcs.len(), 1278);
    assert_eq!(srcs.first().unwrap().to_string(), "192.168.1.1");
    assert_eq!(srcs.last().unwrap().to_string(), "192.168.5.254");
}

const VARS: &str = r#"
schema: 1
meta: { name: vars }
variables:
  - name: ip
    default: 10.0.0.1
  - name: nodefault
layers:
  - protocol: ethernet2
  - protocol: ipv4
    fields:
      dst: ${ip}
      ttl: ${nodefault}
"#;

#[test]
fn variables_default_override_and_missing() {
    let t = yaml(VARS);
    // no value for `nodefault`
    let err = compile(&t, &no_vars(), catalog()).unwrap_err();
    let TemplateError::Lint(issues) = &err else {
        panic!("{err}")
    };
    assert_eq!(issues[0].key, "lint-missing-variable");
    assert_eq!(issues[0].path, "layers[1].fields.ttl");

    let mut vars = VarOverrides::new();
    vars.insert("nodefault".into(), "7".into());
    let c = compile(&t, &vars, catalog()).unwrap();
    assert_eq!(c.packet.layers[1].fields["dst"].to_string_ip(), "10.0.0.1");
    assert_eq!(c.packet.layers[1].fields["ttl"], FieldValue::UInt(7));

    vars.insert("ip".into(), "172.16.0.9".into());
    let c = compile(&t, &vars, catalog()).unwrap();
    assert_eq!(
        c.packet.layers[1].fields["dst"].to_string_ip(),
        "172.16.0.9"
    );

    // overriding an undeclared variable is an error
    vars.insert("nope".into(), "1".into());
    assert!(compile(&t, &vars, catalog()).is_err());
}

trait IpText {
    fn to_string_ip(&self) -> String;
}

impl IpText for FieldValue {
    fn to_string_ip(&self) -> String {
        match self {
            FieldValue::Ipv4(a) => a.to_string(),
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn enum_and_flag_names() {
    let t = yaml(
        r#"
schema: 1
meta: { name: names }
layers:
  - protocol: ethernet2
    fields: { ethertype: ARP }
  - protocol: ipv4
    fields: { flags: "df|mf", protocol: udp }
  - protocol: tcp
    fields: { flags: "syn | ack" }
"#,
    );
    let c = compile(&t, &no_vars(), catalog()).unwrap();
    assert_eq!(
        c.packet.layers[0].fields["ethertype"],
        FieldValue::UInt(0x0806)
    );
    assert_eq!(c.packet.layers[1].fields["flags"], FieldValue::UInt(0b011));
    assert_eq!(c.packet.layers[1].fields["protocol"], FieldValue::UInt(17));
    assert_eq!(c.packet.layers[2].fields["flags"], FieldValue::UInt(0x12));

    let bad = yaml(
        "schema: 1\nmeta: {name: x}\nlayers:\n  - protocol: tcp\n    fields: { flags: 'syn|bogus' }\n",
    );
    let issues = lint(&bad, catalog());
    assert!(issues[0].message.contains("bogus"), "{issues:?}");
}

#[test]
fn tlv_options_and_forms() {
    let t = yaml(
        r#"
schema: 1
meta: { name: opts }
layers:
  - protocol: tcp
    fields:
      options:
        - mss: 1460
        - nop
        - { ws: 7 }
        - timestamps: { tsval: 1, tsecr: 2 }
        - { type: 99, data: "01 02" }
        - name: mss
          fields: { value: 536 }
"#,
    );
    let c = compile(&t, &no_vars(), catalog()).unwrap();
    let FieldValue::Tlv(items) = &c.packet.layers[0].fields["options"] else {
        panic!()
    };
    assert_eq!(items.len(), 6);
    assert_eq!(items[0].name.as_deref(), Some("mss"));
    assert_eq!(items[0].fields["value"], FieldValue::UInt(1460));
    assert_eq!(items[4].ty, Some(99));
    let built = c.packet.build(catalog()).unwrap();
    // MSS option 02 04 05 b4 right after the fixed TCP header
    assert_eq!(&built.bytes[20..24], &[2, 4, 0x05, 0xb4]);

    let bad = yaml(
        "schema: 1\nmeta: {name: x}\nlayers:\n  - protocol: tcp\n    fields:\n      options: [bogus]\n",
    );
    assert!(lint(&bad, catalog())
        .iter()
        .any(|i| i.path == "layers[0].fields.options" && i.level == LintLevel::Error));
}

#[test]
fn generators_assign_distinct_source_ips() {
    let t = yaml(
        r#"
schema: 1
meta: { name: gen }
layers:
  - protocol: ethernet2
  - protocol: ipv4
    generators:
      src: 10.0.0.0/28
      ttl: { kind: list, values: ["1", "2", "3"] }
  - protocol: udp
    generators:
      src_port: { kind: counter, start: 1000 }
stream:
  mode: product
  order: [ttl-is-not-a-layer.x]
"#,
    );
    // bad order reference
    assert!(compile(&t, &no_vars(), catalog()).is_err());

    let mut t = t;
    t.stream.order.clear();
    t.stream.count = None;
    // an unbounded counter cannot sit in an inner product position
    let err = compile(&t, &no_vars(), catalog()).unwrap_err();
    assert!(err.to_string().contains("unbounded"), "{err}");
    t.layers[2].generators.clear();
    let c = compile(&t, &no_vars(), catalog()).unwrap();
    let exp = Expansion::new(&c.stream, None).unwrap();
    assert_eq!(exp.len(), Some(42));
    assert_eq!(exp.total_cardinality(), Some(14 * 3));
    let mut srcs = BTreeSet::new();
    for i in 0..42 {
        let mut p = c.packet.clone();
        apply_overrides(&mut p, &exp.nth(i).unwrap()).unwrap();
        srcs.insert(p.layers[1].fields["src"].to_string_ip());
    }
    assert_eq!(srcs.len(), 14, "{srcs:?}");
}

fn issue_keys(src: &str) -> Vec<(LintLevel, String, String)> {
    lint(&yaml(src), catalog())
        .into_iter()
        .map(|i| (i.level, i.path, i.key))
        .collect()
}

#[test]
fn lint_error_cases() {
    use LintLevel::{Error, Warning};
    let head = "schema: 1\nmeta: { name: t }\n";
    let has = |body: &str, level, path: &str, key: &str| {
        let got = issue_keys(&format!("{head}{body}"));
        assert!(
            got.contains(&(level, path.to_owned(), key.to_owned())),
            "expected {key} at {path} in {got:#?}"
        );
    };
    has(
        "layers:\n  - protocol: nope\n",
        Error,
        "layers[0].protocol",
        "lint-unknown-protocol",
    );
    has(
        "layers:\n  - protocol: ipv4\n    fields: { bogus: 1 }\n",
        Error,
        "layers[0].fields.bogus",
        "lint-unknown-field",
    );
    has(
        "layers:\n  - protocol: ipv4\n    fields: { ttl: 999 }\n",
        Error,
        "layers[0].fields.ttl",
        "lint-invalid-value",
    );
    has(
        "layers:\n  - protocol: ipv4\n    fields: { src: not-an-ip }\n",
        Error,
        "layers[0].fields.src",
        "lint-invalid-value",
    );
    has(
        "layers:\n  - protocol: ipv4\n    generators: { src: { kind: range, start: 1.1.1.1, end: 300.0.0.1 } }\n",
        Error,
        "layers[0].generators.src",
        "lint-invalid-generator",
    );
    has(
        "layers:\n  - protocol: ipv4\n    generators: { ttl: { kind: random_mac } }\n",
        Error,
        "layers[0].generators.ttl",
        "lint-invalid-generator",
    );
    has(
        "layers:\n  - protocol: ipv4\n    fields: { dst: '${nope}' }\n",
        Error,
        "layers[0].fields.dst",
        "lint-unknown-variable",
    );
    has(
        "variables: [{ name: unused }]\nlayers:\n  - protocol: ipv4\n",
        Warning,
        "variables[0]",
        "lint-unused-variable",
    );
    has(
        "layers:\n  - protocol: ethernet2\n  - protocol: tcp\n",
        Warning,
        "layers[1].protocol",
        "lint-next-protocol-manual",
    );
    has(
        "layers:\n  - protocol: ipv4\n    fields: { ttl: 1 }\n    generators: { ttl: 1-3 }\n",
        Warning,
        "layers[0].generators.ttl",
        "lint-field-and-generator",
    );
    has("layers: []\n", Error, "layers", "lint-no-layers");
    // missing meta.name
    let got = lint(
        &yaml("schema: 1\nmeta: { name: '' }\nlayers:\n  - protocol: ipv4\n"),
        catalog(),
    );
    assert!(got.iter().any(|i| i.path == "meta.name"), "{got:?}");
    // schema (programmatic)
    let mut t = yaml(&format!("{head}layers:\n  - protocol: ipv4\n"));
    t.schema = 7;
    assert!(lint(&t, catalog()).iter().any(|i| i.path == "schema"));
    // build diagnostics are mapped: a wrong manual header checksum
    has(
        "layers:\n  - protocol: ipv4\n    fields: { header_checksum: 1 }\n",
        Warning,
        "layers[0].fields.header_checksum",
        "diag-checksum-mismatch",
    );
    // a healthy template is clean
    assert!(issue_keys(&format!(
        "{head}layers:\n  - protocol: ethernet2\n  - protocol: ipv4\n"
    ))
    .is_empty());
}

#[test]
fn compile_error_lists_all_findings() {
    let t = yaml("schema: 1\nmeta: {name: t}\nlayers:\n  - protocol: nope\n  - protocol: ipv4\n    fields: { x: 1 }\n");
    let err = compile(&t, &no_vars(), catalog()).unwrap_err();
    let TemplateError::Lint(v) = &err else {
        panic!()
    };
    assert_eq!(v.len(), 2);
    assert!(err.to_string().contains("more error"), "{err}");
}

#[test]
fn packet_options_apply() {
    let t = yaml(
        "schema: 1\nmeta: {name: t}\npacket: { pad_to_min: false, fcs: auto }\nlayers:\n  - protocol: ethernet2\n",
    );
    let c = compile(&t, &no_vars(), catalog()).unwrap();
    assert!(!c.packet.pad_to_min);
    let built = c.packet.build(catalog()).unwrap();
    assert_eq!(built.bytes.len(), 14 + 4);
}

#[test]
fn render_counts() {
    let t = Template::from_path(root().join("templates/examples/arp-sweep.wpg.yaml")).unwrap();
    let c = compile(&t, &no_vars(), catalog()).unwrap();
    assert_eq!(packet_count(&c, None).unwrap(), 254);
    assert_eq!(packet_count(&c, Some(10)).unwrap(), 10);

    let unbounded = yaml(
        "schema: 1\nmeta: {name: t}\nlayers:\n  - protocol: ethernet2\n  - protocol: ipv4\n    generators: { identification: { kind: counter } }\n",
    );
    let c = compile(&unbounded, &no_vars(), catalog()).unwrap();
    assert!(matches!(
        packet_count(&c, None),
        Err(TemplateError::Unbounded)
    ));
    assert_eq!(packet_count(&c, Some(5)).unwrap(), 5);
}

#[test]
fn render_round_trip_through_pcapng() {
    let t =
        Template::from_path(root().join("templates/examples/eth-ipv4-udp-range.wpg.yaml")).unwrap();
    let mut vars = VarOverrides::new();
    vars.insert("dst_ip".into(), "203.0.113.5".into());
    let c = compile(&t, &vars, catalog()).unwrap();
    let mut buf = Vec::new();
    let n = render_pcapng(&c, catalog(), Some(10), &mut buf).unwrap();
    assert_eq!(n, 10);
    let frames = wpg_testutil::pcap::parse_pcapng(&buf).unwrap();
    assert_eq!(frames.len(), 10);
    let mut srcs = BTreeSet::new();
    for (i, f) in frames.iter().enumerate() {
        // eth(14) + ipv4(20) + udp(8) + payload(64)
        assert_eq!(f.len(), 14 + 20 + 8 + 64);
        assert_eq!(&f[30..34], &[203, 0, 113, 5], "dst ip of frame {i}");
        srcs.insert(f[26..30].to_vec());
        assert_eq!(&f[26..30], &[192, 168, 1, u8::try_from(i + 1).unwrap()]);
        assert_eq!(&f[42..46], &[0, 1, 2, 3], "pattern payload");
    }
    assert_eq!(srcs.len(), 10);
}

#[test]
#[ignore = "requires tshark"]
fn tshark_reads_rendered_pcapng() {
    for f in sample_files() {
        let t = Template::from_path(&f).unwrap();
        let c = compile(&t, &no_vars(), catalog()).unwrap();
        let path = std::env::temp_dir().join(format!(
            "wpg-template-{}-{}.pcapng",
            std::process::id(),
            f.file_stem().unwrap().to_string_lossy()
        ));
        let file = std::fs::File::create(&path).unwrap();
        render_pcapng(&c, catalog(), Some(4), file).unwrap();
        let out = std::process::Command::new("tshark")
            .args(["-r", path.to_str().unwrap(), "-T", "json"])
            .output()
            .expect("tshark");
        assert!(
            out.status.success(),
            "{}: {}",
            f.display(),
            String::from_utf8_lossy(&out.stderr)
        );
        let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(json.as_array().unwrap().len(), 4, "{}", f.display());
        for pkt in json.as_array().unwrap() {
            assert!(
                pkt["_source"]["layers"].get("_ws.malformed").is_none(),
                "{}: malformed packet",
                f.display()
            );
        }
        std::fs::remove_file(path).ok();
    }
}

#[test]
fn json_schema_matches_golden() {
    let mut text = serde_json::to_string_pretty(&Template::json_schema()).unwrap();
    text.push('\n');
    let path = root().join("docs/schema/template.schema.json");
    if std::env::var_os("WPG_UPDATE_GOLDEN").is_some_and(|v| v == "1") {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &text).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "cannot read {} ({e}); run with WPG_UPDATE_GOLDEN=1",
            path.display()
        )
    });
    // A Windows checkout may convert the golden file to CRLF.
    let want = want.replace("\r\n", "\n");
    assert!(
        want == text,
        "docs/schema/template.schema.json is stale; run WPG_UPDATE_GOLDEN=1 cargo test -p wpg-template --test templates"
    );
}

#[test]
fn json_schema_accepts_samples_shape() {
    let s = Template::json_schema();
    assert_eq!(s["title"], "Template");
    assert!(s["properties"]["layers"].is_object());
    assert!(s["properties"]["stream"].is_object());
}

/// A list or TLV field also takes verbatim octets as a hex string (what the import
/// writes for content it cannot decode, and the representation BGP `nlri` had
/// before it became a prefix list).
#[test]
fn composite_fields_accept_hex_strings() {
    let render_one = |nlri: &str| {
        let t = yaml(&format!(
            "schema: 1\nmeta: {{ name: x }}\nlayers:\n  - protocol: ipv4\n  - protocol: tcp\n    fields: {{ dst_port: 179 }}\n  - protocol: bgp\n  - protocol: bgp_update\n    fields:\n      nlri: {nlri}\n"
        ));
        let c = compile(&t, &VarOverrides::new(), catalog()).unwrap();
        let mut out = Vec::new();
        render_with(&c, catalog(), Some(1), &mut |_, b| {
            out.push(b.to_vec());
            Ok(())
        })
        .unwrap();
        out.remove(0)
    };
    let typed =
        render_one("[{ length: 24, prefix: \"c6 33 64\" }, { length: 16, prefix: \"c612\" }]");
    assert_eq!(render_one("\"18 c63364 10 c612\""), typed);
    assert_eq!(&typed[typed.len() - 7..], [24, 198, 51, 100, 16, 198, 18]);
}

#[test]
fn expr_references_accept_protocol_ids() {
    let src = |r: &str| {
        format!(
            r#"
schema: 1
meta: {{ name: expr }}
layers:
  - protocol: ethernet2
  - protocol: ipv4
    generators:
      ttl: {{ kind: list, values: ["7", "9"] }}
      identification: {{ kind: expr, expr: "{r} + 1" }}
"#
        )
    };
    for r in ["$1.ttl", "$ipv4.ttl"] {
        let c = compile(&yaml(&src(r)), &no_vars(), catalog()).unwrap();
        let exp = Expansion::new(&c.stream, None).unwrap();
        let ids: Vec<_> = (0..2)
            .map(|i| {
                let mut p = c.packet.clone();
                apply_overrides(&mut p, &exp.nth(i).unwrap()).unwrap();
                p.layers[1].fields["identification"].clone()
            })
            .collect();
        assert_eq!(ids, [FieldValue::UInt(8), FieldValue::UInt(10)], "{r}");
    }
    for r in ["$udp.ttl", "$5.ttl"] {
        let keys = issue_keys(&src(r));
        assert!(
            keys.iter().any(|(l, p, k)| *l == LintLevel::Error
                && p == "layers[1].generators.identification"
                && k == "lint-invalid-generator"),
            "{r}: {keys:#?}"
        );
    }
}

#[test]
fn layer_references_pick_the_nth_occurrence() {
    let src = |inner: &str, stream: &str| {
        format!(
            r#"
schema: 1
meta: {{ name: nth }}
layers:
  - protocol: ethernet2
  - protocol: ipv4
    generators:
      ttl: {{ kind: list, values: ["7", "9"] }}
  - protocol: ipv4
    generators:
{inner}
stream: {stream}
"#
        )
    };
    let expr = |r: &str| {
        src(
            &format!(
                "      ttl: {{ kind: list, values: [\"1\", \"2\"] }}\n      identification: {{ kind: expr, expr: \"{r} + 1\" }}"
            ),
            "{ mode: zip }",
        )
    };
    let c = compile(&yaml(&expr("$ipv4[0].ttl")), &no_vars(), catalog()).unwrap();
    let exp = Expansion::new(&c.stream, None).unwrap();
    let mut p = c.packet.clone();
    apply_overrides(&mut p, &exp.nth(1).unwrap()).unwrap();
    assert_eq!(p.layers[2].fields["identification"], FieldValue::UInt(10));
    for r in ["$ipv4.ttl", "$ipv4[2].ttl", "$ipv4[x].ttl"] {
        let keys = issue_keys(&expr(r));
        assert!(
            keys.iter().any(|(l, p, k)| *l == LintLevel::Error
                && p == "layers[2].generators.identification"
                && k == "lint-invalid-generator"),
            "{r}: {keys:#?}"
        );
    }

    // `stream.order` takes the same form: the inner ipv4 runs slowest here
    let inner = "      ttl: { kind: list, values: [\"1\", \"2\", \"3\"] }";
    let c = compile(
        &yaml(&src(
            inner,
            r#"{ mode: nested, order: ["ipv4[1].ttl", "ipv4[0].ttl"] }"#,
        )),
        &no_vars(),
        catalog(),
    )
    .unwrap();
    let exp = Expansion::new(&c.stream, None).unwrap();
    let mut p = c.packet.clone();
    apply_overrides(&mut p, &exp.nth(1).unwrap()).unwrap();
    assert_eq!(p.layers[1].fields["ttl"], FieldValue::UInt(9));
    assert_eq!(p.layers[2].fields["ttl"], FieldValue::UInt(1));
    let keys = issue_keys(&src(inner, r#"{ mode: nested, order: ["ipv4[2].ttl"] }"#));
    assert!(
        keys.iter()
            .any(|(l, p, _)| *l == LintLevel::Error && p == "stream.order[0]"),
        "{keys:#?}"
    );
}
