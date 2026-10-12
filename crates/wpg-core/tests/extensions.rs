//! IR extensions that let P1 protocols be modelled without approximations:
//! payload-peek / remaining-length / alternate-field demultiplexing, element-count
//! Auto fields, NUL-terminated strings and checksum field exclusion.

use std::collections::BTreeMap;
use std::net::Ipv4Addr;

use wpg_core::diag::keys;
use wpg_core::{
    parse, protocol, Binding, BindingEntry, FieldType, FieldValue, ItemDef, Layer, Length, Packet,
    Peek, ProtocolDef,
};

fn names(p: &Packet) -> Vec<&str> {
    p.layers.iter().map(|l| l.protocol.as_str()).collect()
}

fn ipv4_list(count: &str) -> FieldType {
    FieldType::List {
        item: ItemDef::Field {
            ty: Box::new(FieldType::Ipv4),
        },
        length: Length::FromField {
            field: count.into(),
            unit: 4,
            adjust: 0,
        },
    }
}

/// `label` (MPLS-like): `s = 0` → another label, `s = 1` → peek the first nibble.
/// `v4` / `v6`: one-octet "headers" whose first nibble is the version.
fn peek_defs() -> Vec<ProtocolDef> {
    let mut label = protocol! {
        id: "label",
        name: "Label",
        layer: L2,
        fields: {
            value: uint(7) = 0;
            s: uint(1) = 1, auto = next_protocol;
        },
        next: s { 0 => "label" },
    };
    let mut peek = Peek::new(
        1,
        4,
        vec![BindingEntry::new(4, "v4"), BindingEntry::new(6, "v6")],
    );
    peek.default = Some("other".into());
    label.next.as_mut().unwrap().peek = Some(peek);
    let v = |id: &'static str, version: u64| {
        let mut d = protocol! {
            id: "x",
            name: "X",
            layer: L3,
            fields: {
                version: uint(4) = 0;
                rest: uint(4) = 0;
            },
        };
        d.id = id.into();
        d.fields[0].default = Some(FieldValue::UInt(version));
        d
    };
    let other = protocol! {
        id: "other",
        name: "Other",
        layer: L2,
        fields: { data: raw; },
    };
    let defs = vec![label, v("v4", 4), v("v6", 6), other];
    for d in &defs {
        d.validate().unwrap();
    }
    defs
}

#[test]
fn peek_selects_on_payload_and_builds_selector() {
    let defs = peek_defs();
    let pkt = Packet::from_layers(vec![
        Layer::new("label"),
        Layer::new("label"),
        Layer::new("v6"),
    ]);
    let built = pkt.build(&defs).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    // s = 0 (next is a label), then s = 1 (next is peeked), then version 6.
    assert_eq!(built.bytes, [0x00, 0x01, 0x60]);

    let p = parse(&built.bytes, "label", &defs).unwrap();
    assert_eq!(names(&p.packet), ["label", "label", "v6"]);
    let p = parse(&[0x01, 0x45], "label", &defs).unwrap();
    assert_eq!(names(&p.packet), ["label", "v4"]);
    let p = parse(&[0x01, 0x95, 0x01], "label", &defs).unwrap();
    assert_eq!(names(&p.packet), ["label", "other"], "peek default");
    let p = parse(&[0x01], "label", &defs).unwrap();
    assert_eq!(names(&p.packet), ["label"], "nothing to peek at");
}

#[test]
fn min_len_and_alt_field_select() {
    let mut b = Binding::new(
        "dst",
        vec![
            BindingEntry::new(1, "long").with_min_len(3),
            BindingEntry::new(1, "short"),
            BindingEntry::new(53, "short"),
        ],
    );
    b.alt_field = Some("src".into());
    let get = |src: u64, dst: u64| {
        move |f: &str| match f {
            "src" => Some(src),
            "dst" => Some(dst),
            _ => None,
        }
    };
    assert_eq!(b.select(get(0, 1), &[0; 3]), Some(("long", false)));
    assert_eq!(b.select(get(0, 1), &[0; 2]), Some(("short", false)));
    assert_eq!(b.select(get(53, 40000), &[]), Some(("short", false)), "alt");
    assert_eq!(
        b.select(get(1, 53), &[0; 9]),
        Some(("short", false)),
        "primary first"
    );
    assert_eq!(b.select(get(7, 8), &[]), None);
    // Building only ever announces through the primary field.
    assert_eq!(b.value_for("long"), Some(1));
    assert_eq!(b.value_for("short"), Some(1));
}

fn count_def() -> ProtocolDef {
    let inner = FieldType::List {
        item: ItemDef::Struct {
            fields: vec![
                wpg_core::FieldDef::new("n", FieldType::UInt { bits: 8 }).with_auto(
                    wpg_core::AutoCompute::Count {
                        field: "addrs".into(),
                    },
                ),
                wpg_core::FieldDef::new("addrs", ipv4_list("n"))
                    .with_default(FieldValue::List(Vec::new())),
            ],
        },
        length: Length::ToEnd,
    };
    let d = protocol! {
        id: "counted",
        name: "Counted",
        layer: L7,
        fields: {
            num: uint(8), auto = count(groups);
            ck: uint(16), auto = checksum(internet, header, exclude = [secret]);
            secret: bytes(2) = [0xab, 0xcd];
            groups: ty(inner) = Vec::<FieldValue>::new();
        },
    };
    d.validate().unwrap();
    d
}

fn group(addrs: &[[u8; 4]]) -> FieldValue {
    FieldValue::Struct(BTreeMap::from([(
        "addrs".to_owned(),
        FieldValue::List(
            addrs
                .iter()
                .map(|a| FieldValue::Ipv4(Ipv4Addr::from(*a)))
                .collect(),
        ),
    )]))
}

#[test]
fn counts_are_auto_at_header_and_element_level() {
    let defs = vec![count_def()];
    let groups = FieldValue::List(vec![group(&[[1, 1, 1, 1], [2, 2, 2, 2]]), group(&[])]);
    let pkt = Packet::from_layers(vec![Layer::new("counted").with("groups", groups.clone())]);
    let built = pkt.build(&defs).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    // The checksum covers the header with `secret` read as zero: 0x0200 + 0x0002 +
    // 0x0101 + 0x0101 + 0x0202 + 0x0202 + 0x0000 (odd tail) = 0x0808 → 0xf7f7.
    #[rustfmt::skip]
    let want = [
        2,                    // num (auto): two groups
        0xf7, 0xf7,           // ck
        0xab, 0xcd,           // secret
        2, 1, 1, 1, 1, 2, 2, 2, 2, // n (auto) + two addresses
        0,                    // n (auto) of the empty group
    ];
    assert_eq!(built.bytes, want);
    assert!(built.layout[0].fields[0].auto);

    // Manual counts that disagree are warned about, at both levels.
    let mut bad = group(&[[3, 3, 3, 3]]);
    if let FieldValue::Struct(m) = &mut bad {
        m.insert("n".into(), FieldValue::UInt(2));
    }
    let pkt = Packet::from_layers(vec![Layer::new("counted")
        .with("num", 3u8)
        .with("groups", FieldValue::List(vec![bad]))]);
    let built = pkt.build(&defs).unwrap();
    let mismatches = built
        .diagnostics
        .iter()
        .filter(|d| d.key == keys::COUNT_MISMATCH)
        .count();
    assert_eq!(mismatches, 2, "{:?}", built.diagnostics);

    // Parsing reproduces the octets.
    let bytes = Packet::from_layers(vec![Layer::new("counted").with("groups", groups)])
        .build(&defs)
        .unwrap()
        .bytes;
    let p = parse(&bytes, "counted", &defs).unwrap();
    assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
    assert_eq!(p.packet.build(&defs).unwrap().bytes, bytes);
}

#[test]
fn excluded_fields_do_not_change_the_checksum() {
    let defs = vec![count_def()];
    let ck = |secret: [u8; 2]| {
        Packet::from_layers(vec![Layer::new("counted").with("secret", secret.to_vec())])
            .build(&defs)
            .unwrap()
            .bytes[1..3]
            .to_vec()
    };
    assert_eq!(ck([0, 0]), ck([0xff, 0x12]));
}

fn cstr_def() -> ProtocolDef {
    let d = protocol! {
        id: "req",
        name: "Request",
        layer: L7,
        fields: {
            file: cstring = *b"boot.bin";
            mode: cstring = *b"octet";
            rest: bytes(to_end) = [];
        },
    };
    d.validate().unwrap();
    d
}

#[test]
fn cstrings_roundtrip() {
    let defs = vec![cstr_def()];
    let built = Packet::from_layers(vec![Layer::new("req").with("rest", vec![9u8])])
        .build(&defs)
        .unwrap();
    assert_eq!(built.bytes, b"boot.bin\0octet\0\x09");
    let p = parse(&built.bytes, "req", &defs).unwrap();
    assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
    assert_eq!(
        p.packet.layers[0].fields["mode"],
        FieldValue::Bytes(b"octet".to_vec())
    );
    assert_eq!(p.layout[0].fields[1].bit_len, 6 * 8);

    // A string containing NUL cannot be encoded; an unterminated one does not parse.
    assert!(
        Packet::from_layers(vec![Layer::new("req").with("file", vec![b'a', 0])])
            .build(&defs)
            .is_err()
    );
    let p = parse(b"boot.bin\0octet", "req", &defs).unwrap();
    assert!(p.has_errors());
    assert_eq!(names(&p.packet), ["raw"]);
}

#[test]
fn cstring_notation() {
    use wpg_core::notation::{format_value, parse_value};
    let v = parse_value("\"a \\\"b\\\"\"", &FieldType::CString).unwrap();
    assert_eq!(v, FieldValue::Bytes(b"a \"b\"".to_vec()));
    assert_eq!(format_value(&v, &FieldType::CString), "\"a \\\"b\\\"\"");
    assert!(parse_value("61 00", &FieldType::CString).is_err());
    assert_eq!(
        format_value(&FieldValue::Bytes(vec![1, 2]), &FieldType::CString),
        "01 02"
    );
}

#[test]
fn new_ir_shapes_serialise() {
    let def = count_def();
    let json = serde_json::to_string(&def).unwrap();
    assert!(
        json.contains(r#""kind":"count","field":"groups""#),
        "{json}"
    );
    assert!(json.contains(r#""exclude":["secret"]"#), "{json}");
    let back: ProtocolDef = serde_json::from_str(&json).unwrap();
    assert_eq!(back, def);
    let json = serde_json::to_string(&cstr_def()).unwrap();
    assert!(json.contains(r#"{"kind":"cstring"}"#), "{json}");
    let peek = &peek_defs()[0];
    let back: ProtocolDef = serde_json::from_str(&serde_json::to_string(peek).unwrap()).unwrap();
    assert_eq!(&back, peek);
}

#[test]
fn invalid_extensions_are_rejected() {
    let mut d = count_def();
    d.fields[0].auto = Some(wpg_core::AutoCompute::Count {
        field: "secret".into(),
    });
    assert!(d.validate().is_err(), "count of a non-list field");
    let mut d = count_def();
    if let Some(wpg_core::AutoCompute::Checksum { exclude, .. }) = &mut d.fields[1].auto {
        exclude.push("nope".into());
    }
    assert!(d.validate().is_err(), "unknown excluded field");
    let mut d = peek_defs().remove(0);
    d.next.as_mut().unwrap().alt_field = Some("nope".into());
    assert!(d.validate().is_err(), "unknown alt field");
    let mut d = peek_defs().remove(0);
    d.next.as_mut().unwrap().peek.as_mut().unwrap().bits = 0;
    assert!(d.validate().is_err(), "zero-width peek");
}
