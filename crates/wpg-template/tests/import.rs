//! Import round trips: render sample templates to pcapng, import them back and
//! compare the re-rendered bytes.

use std::path::{Path, PathBuf};

use wpg_protocols::catalog;
use wpg_template::render::{render_pcapng, render_with};
use wpg_template::{
    compile, import_capture, import_frames, import_hex, lint, ImportMode, ImportOptions, Template,
    VarOverrides,
};

fn sample(name: &str) -> Template {
    let p: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../templates/examples")
        .join(name);
    Template::from_path(p).unwrap()
}

fn render(t: &Template, count: Option<u64>) -> Vec<Vec<u8>> {
    let c = compile(t, &VarOverrides::new(), catalog()).unwrap();
    let mut out = Vec::new();
    render_with(&c, catalog(), count, &mut |_, b| {
        out.push(b.to_vec());
        Ok(())
    })
    .unwrap();
    out
}

fn pcapng(t: &Template, count: Option<u64>) -> Vec<u8> {
    let c = compile(t, &VarOverrides::new(), catalog()).unwrap();
    let mut buf = Vec::new();
    render_pcapng(&c, catalog(), count, &mut buf).unwrap();
    buf
}

fn no_lint_issues(t: &Template) {
    let issues = lint(t, catalog());
    assert!(issues.is_empty(), "{issues:#?}\n{}", t.to_yaml().unwrap());
}

fn round_trip(
    name: &str,
    count: Option<u64>,
    detect: bool,
) -> (Template, Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let original = sample(name);
    let file = pcapng(&original, count);
    let opts = ImportOptions {
        detect_generators: detect,
        ..ImportOptions::default()
    };
    let r = import_capture(&file, "x.pcapng", &opts, catalog()).unwrap();
    assert_eq!(r.templates.len(), 1);
    let imported = r.templates[0].clone();
    // survives a YAML round trip and lints clean
    let again = Template::from_yaml_str(&imported.to_yaml().unwrap()).unwrap();
    assert_eq!(again, imported);
    no_lint_issues(&imported);
    (
        imported.clone(),
        render(&original, count),
        render(&imported, None),
    )
}

#[test]
fn packet_zero_is_exact_for_every_sample() {
    for name in [
        "arp-sweep.wpg.yaml",
        "eth-ipv4-udp-range.wpg.yaml",
        "icmp-echo-pattern-payload.wpg.yaml",
        "ipv6-ndp-ns.wpg.yaml",
        "vlan-tcp-syn.wpg.yaml",
    ] {
        let (t, before, after) = round_trip(name, Some(5), false);
        assert_eq!(before[0], after[0], "{name}\n{}", t.to_yaml().unwrap());
        assert_eq!(after.len(), 5, "{name}: stream.count");
    }
}

#[test]
fn udp_range_stream_is_reproduced() {
    let (t, before, after) = round_trip("eth-ipv4-udp-range.wpg.yaml", None, true);
    assert_eq!(before.len(), 1278);
    assert_eq!(before, after, "{}", t.to_yaml().unwrap());
    let yaml = t.to_yaml().unwrap();
    assert!(yaml.contains("kind: range"), "{yaml}");
    assert!(yaml.contains("192.168.5.254"), "{yaml}");
    // checksums / lengths / ethertype are left to the builder
    assert!(yaml.contains("checksum: auto"), "{yaml}");
    assert!(yaml.contains("ethertype: auto"), "{yaml}");
}

#[test]
fn arp_sweep_stream_is_reproduced() {
    let (t, before, after) = round_trip("arp-sweep.wpg.yaml", None, true);
    assert_eq!(before.len(), 254);
    assert_eq!(before, after, "{}", t.to_yaml().unwrap());
}

#[test]
fn counter_like_and_list_generators() {
    let (t, before, after) = round_trip("icmp-echo-pattern-payload.wpg.yaml", None, true);
    assert_eq!(before.len(), 10);
    assert_eq!(before, after, "{}", t.to_yaml().unwrap());
}

#[test]
fn random_ports_become_a_random_generator() {
    let (t, before, after) = round_trip("vlan-tcp-syn.wpg.yaml", Some(200), true);
    let yaml = t.to_yaml().unwrap();
    assert!(yaml.contains("kind: random"), "{yaml}");
    assert_eq!(before.len(), after.len());
    // everything but the random port (and the checksum that follows it) is stable
    for (a, b) in before.iter().zip(&after) {
        assert_eq!(a.len(), b.len());
        assert_eq!(a[..38], b[..38]);
    }
}

#[test]
fn per_packet_mode_and_max_packets() {
    let file = pcapng(&sample("arp-sweep.wpg.yaml"), Some(6));
    let opts = ImportOptions {
        mode: ImportMode::PerPacket,
        max_packets: Some(4),
        name_prefix: "arp".into(),
        ..ImportOptions::default()
    };
    let r = import_capture(&file, "arp.pcapng", &opts, catalog()).unwrap();
    assert_eq!(r.packets, 4);
    assert_eq!(r.templates.len(), 4);
    assert_eq!(r.templates[2].meta.name, "arp-0003");
    let orig = render(&sample("arp-sweep.wpg.yaml"), Some(4));
    for (t, want) in r.templates.iter().zip(&orig) {
        no_lint_issues(t);
        assert_eq!(&render(t, Some(1))[0], want);
        assert!(t.stream.count.is_none());
    }
}

#[test]
fn ipv6_extension_headers_and_tcp_options() {
    let yaml = r"
schema: 1
meta: {name: v6}
layers:
  - protocol: ethernet2
    fields: {dst: '02:00:00:00:00:01', src: '02:00:00:00:00:02'}
  - protocol: ipv6
    fields: {src: '2001:db8::1', dst: '2001:db8::2'}
  - protocol: ipv6_hopopts
  - protocol: ipv6_dstopts
  - protocol: tcp
    fields:
      src_port: 40000
      dst_port: 80
      flags: syn|ack
      options:
        - mss: 1440
        - sack_permitted
        - timestamps: {tsval: 7, tsecr: 9}
        - nop
        - ws: 8
  - protocol: payload
    fields: {mode: pattern, data: '68 65 6c 6c 6f', length: 5}
";
    let t = Template::from_yaml_str(yaml).unwrap();
    let frame = render(&t, Some(1)).remove(0);
    let r = import_frames(
        std::slice::from_ref(&frame),
        "v6",
        &ImportOptions::default(),
        catalog(),
    )
    .unwrap();
    let imported = &r.templates[0];
    no_lint_issues(imported);
    assert_eq!(render(imported, Some(1))[0], frame);
    let y = imported.to_yaml().unwrap();
    assert!(y.contains("mss: 1440"), "{y}");
    assert!(
        y.contains("protocol: ipv6_hopopts") && y.contains("protocol: ipv6_dstopts"),
        "{y}"
    );
    assert!(y.contains("flags: ack|syn"), "{y}");
    assert!(y.contains("checksum: auto"), "{y}");
    assert!(
        r.notes.iter().all(|n| n.key != "import-not-reproducible"),
        "{:?}",
        r.notes
    );
}

#[test]
fn wrong_checksum_stays_manual() {
    let yaml = r"
schema: 1
meta: {name: bad}
layers:
  - protocol: ethernet2
    fields: {dst: '02:00:00:00:00:01', src: '02:00:00:00:00:02'}
  - protocol: ipv4
    fields: {src: 10.0.0.1, dst: 10.0.0.2, header_checksum: 0x1234}
  - protocol: udp
    fields: {src_port: 1, dst_port: 2}
";
    let t = Template::from_yaml_str(yaml).unwrap();
    let frame = render(&t, Some(1)).remove(0);
    let r = import_frames(
        std::slice::from_ref(&frame),
        "bad",
        &ImportOptions::default(),
        catalog(),
    )
    .unwrap();
    let y = r.templates[0].to_yaml().unwrap();
    assert!(y.contains("header_checksum: 4660"), "{y}");
    assert_eq!(render(&r.templates[0], Some(1))[0], frame);
}

#[test]
fn mixed_stacks_skip_inference() {
    let a = render(&sample("arp-sweep.wpg.yaml"), Some(1)).remove(0);
    let b = render(&sample("ipv6-ndp-ns.wpg.yaml"), Some(1)).remove(0);
    let r = import_frames(&[a.clone(), b], "mix", &ImportOptions::default(), catalog()).unwrap();
    assert!(r.notes.iter().any(|n| n.key == "import-mixed-stacks"));
    assert_eq!(render(&r.templates[0], Some(1))[0], a);
}

#[test]
fn hex_forms_import_identically() {
    let frame = render(&sample("ipv6-ndp-ns.wpg.yaml"), Some(1)).remove(0);
    let plain: String = frame.iter().map(|b| format!("{b:02x}")).collect();
    let spaced: Vec<String> = frame.iter().map(|b| format!("{b:02x}")).collect();
    let prefixed: Vec<String> = frame.iter().map(|b| format!("0x{b:02x}")).collect();
    let mut dump = String::new();
    let mut xxd = String::new();
    for (i, chunk) in frame.chunks(16).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
        let ascii: String = chunk
            .iter()
            .map(|&b| {
                if (0x20..0x7f).contains(&b) {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        // Wireshark style: offset, hex with a wider middle gap, ASCII column
        let mut cols = String::new();
        for (j, h) in hex.iter().enumerate() {
            if j == 8 {
                cols.push(' ');
            }
            cols.push_str(h);
            cols.push(' ');
        }
        let pad = " ".repeat((16 - chunk.len()) * 3 + usize::from(chunk.len() <= 8));
        dump.push_str(&format!("{:04x}   {cols}{pad}  {ascii}\n", i * 16));
        // xxd style
        let groups: Vec<String> = hex.chunks(2).map(<[String]>::concat).collect();
        xxd.push_str(&format!(
            "{:08x}: {:<39}  {ascii}\n",
            i * 16,
            groups.join(" ")
        ));
        // hexdump -C is exercised below
    }
    let mut canon = String::new();
    for (i, chunk) in frame.chunks(16).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
        canon.push_str(&format!(
            "{:08x}  {:<48} |{}|\n",
            i * 16,
            hex.join(" "),
            "x".repeat(chunk.len())
        ));
    }
    for (label, text) in [
        ("plain", plain),
        ("spaced", spaced.join(" ")),
        ("prefixed", prefixed.join(", ")),
        ("wireshark dump", dump),
        ("xxd", xxd),
        ("hexdump -C", canon),
    ] {
        let r =
            import_hex(&text, "ethernet2", catalog()).unwrap_or_else(|e| panic!("{label}: {e}"));
        let t = &r.templates[0];
        assert!(t.stream.count.is_none(), "{label}");
        assert_eq!(render(t, Some(1))[0], frame, "{label}");
    }
    assert!(import_hex("zz", "ethernet2", catalog()).is_err());
    assert!(import_hex("abc", "ethernet2", catalog()).is_err());
    assert!(import_hex("  \n", "ethernet2", catalog()).is_err());
}

#[test]
fn errors() {
    let opts = ImportOptions::default();
    assert!(import_capture(b"nope", "x", &opts, catalog()).is_err());
    let empty = wpg_template::PcapngWriter::new(Vec::new())
        .unwrap()
        .finish()
        .unwrap();
    assert!(matches!(
        import_capture(&empty, "x", &opts, catalog()),
        Err(wpg_template::ImportError::Empty)
    ));
    let bad = ImportOptions {
        first_protocol: "nope".into(),
        ..ImportOptions::default()
    };
    let frame = render(&sample("arp-sweep.wpg.yaml"), Some(1));
    assert!(import_frames(&frame, "x", &bad, catalog()).is_err());
}
