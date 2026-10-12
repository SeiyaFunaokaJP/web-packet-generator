//! IR extensions for the IS-IS TLVs (M7): rounded-up prefix lengths
//! (`Length::FromField { div }`), count-bounded lists of variable-size elements
//! (`Length::Count`), octet-size Auto fields (`AutoCompute::Size`) and TLV filler
//! items up to a requested PDU size (`TlvFill`).

use std::collections::BTreeMap;

use wpg_core::diag::keys;
use wpg_core::stream::FieldRef;
use wpg_core::{
    parse, protocol, AutoCompute, FieldDef, FieldType, FieldValue, ItemDef, Layer, Length,
    LengthUnit, Packet, Prebuilt, ProtocolDef, TlvDef, TlvFill, TlvItem, TlvVariant,
};

fn uint(name: &str, bits: u8) -> FieldDef {
    FieldDef::new(name, FieldType::UInt { bits })
}

fn tlv(variants: Vec<TlvVariant>, length: Length) -> TlvDef {
    TlvDef {
        type_bits: 8,
        len_bits: 8,
        len_includes_header: false,
        len_unit: LengthUnit::Bytes,
        variants,
        unknown_as_raw: true,
        length,
        fill: None,
    }
}

/// Prefix entries shaped like IS-IS TLV 135: `metric`, `s` (sub-TLVs present,
/// Auto = number of sub-TLV blocks), `plen` (bits), `prefix` (`ceil(plen / 8)`
/// octets) and an optional block `{len (Auto size), subs}`.
fn prefix_def() -> ProtocolDef {
    let block = ItemDef::Struct {
        fields: vec![
            uint("len", 8).with_auto(AutoCompute::Size {
                field: "subs".into(),
            }),
            FieldDef::new(
                "subs",
                FieldType::Tlv(tlv(
                    vec![TlvVariant::new(1, "tag", vec![uint("value", 32)])],
                    Length::FromField {
                        field: "len".into(),
                        unit: 1,
                        adjust: 0,
                        div: 1,
                    },
                )),
            ),
        ],
    };
    let entry = ItemDef::Struct {
        fields: vec![
            uint("metric", 8),
            uint("s", 1).with_auto(AutoCompute::Count {
                field: "block".into(),
            }),
            uint("plen", 7),
            FieldDef::new(
                "prefix",
                FieldType::Bytes {
                    len: Length::FromField {
                        field: "plen".into(),
                        unit: 1,
                        adjust: 7,
                        div: 8,
                    },
                },
            ),
            FieldDef::new(
                "block",
                FieldType::List {
                    item: block,
                    length: Length::Count { field: "s".into() },
                },
            ),
        ],
    };
    let d = protocol! {
        id: "prefixes",
        name: "Prefixes",
        layer: L3,
        fields: {
            size: uint(8), auto = size(entries);
            entries: ty(FieldType::List {
                item: entry,
                length: Length::FromField { field: "size".into(), unit: 1, adjust: 0, div: 1 },
            }) = Vec::<FieldValue>::new();
        },
    };
    d.validate().unwrap();
    d
}

fn entry(metric: u8, plen: u8, prefix: &[u8], tags: &[u32]) -> FieldValue {
    let mut m = BTreeMap::new();
    m.insert("metric".to_owned(), FieldValue::UInt(metric.into()));
    m.insert("plen".to_owned(), FieldValue::UInt(plen.into()));
    m.insert("prefix".to_owned(), FieldValue::Bytes(prefix.to_vec()));
    let blocks = if tags.is_empty() {
        Vec::new()
    } else {
        let subs = tags
            .iter()
            .map(|t| TlvItem::named("tag").with("value", *t))
            .collect::<Vec<_>>();
        let mut b = BTreeMap::new();
        b.insert("subs".to_owned(), FieldValue::Tlv(subs));
        vec![FieldValue::Struct(b)]
    };
    m.insert("block".to_owned(), FieldValue::List(blocks));
    FieldValue::Struct(m)
}

#[test]
fn rounded_prefixes_counted_blocks_and_sizes() {
    let defs = vec![prefix_def()];
    let p = Packet::from_layers(vec![Layer::new("prefixes").with(
        "entries",
        vec![
            entry(10, 24, &[192, 0, 2], &[]),
            entry(20, 17, &[198, 51, 0x80], &[7]),
            entry(30, 0, &[], &[]),
        ],
    )]);
    let built = p.build(&defs).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    #[rustfmt::skip]
    let want = [
        19, // size of the entries
        10, 24, 192, 0, 2,
        20, 0x80 | 17, 198, 51, 0x80, 6, 1, 4, 0, 0, 0, 7,
        30, 0,
    ];
    assert_eq!(built.bytes, want);

    let parsed = parse(&built.bytes, "prefixes", &defs).unwrap();
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let FieldValue::List(entries) = &parsed.packet.layers[0].fields["entries"] else {
        panic!("entries did not decode: {:?}", parsed.packet.layers[0]);
    };
    assert_eq!(entries.len(), 3);
    assert_eq!(parsed.packet.build(&defs).unwrap().bytes, built.bytes);
}

#[test]
fn manual_sizes_are_kept_and_warned() {
    let defs = vec![prefix_def()];
    let p = Packet::from_layers(vec![Layer::new("prefixes")
        .with("size", 9u8)
        .with("entries", vec![entry(10, 24, &[192, 0, 2], &[])])]);
    let built = p.build(&defs).unwrap();
    assert_eq!(built.bytes[0], 9);
    assert!(built
        .diagnostics
        .iter()
        .any(|d| d.key == keys::LENGTH_MISMATCH && d.field.as_deref() == Some("size")));
}

/// A hello padded with filler TLVs (type 8) up to its Manual `total`.
fn hello_def() -> ProtocolDef {
    let mut t = tlv(
        vec![
            TlvVariant::new(
                1,
                "name",
                vec![FieldDef::new(
                    "text",
                    FieldType::Bytes { len: Length::ToEnd },
                )],
            ),
            TlvVariant::new(
                8,
                "padding",
                vec![FieldDef::new(
                    "data",
                    FieldType::Bytes { len: Length::ToEnd },
                )],
            ),
        ],
        Length::ToEnd,
    );
    t.fill = Some(TlvFill {
        variant: "padding".into(),
        until: "total".into(),
    });
    let d = protocol! {
        id: "hello",
        name: "Hello",
        layer: L3,
        fields: {
            kind: uint(8) = 1;
            total: uint(16), auto = length(total);
            tlvs: ty(FieldType::Tlv(t)) = vec![TlvItem::named("name").with("text", b"r1".to_vec())];
        },
    };
    d.validate().unwrap();
    d
}

fn hello(total: Option<u16>) -> Packet {
    let mut l = Layer::new("hello");
    if let Some(t) = total {
        l = l.with("total", t);
    }
    Packet::from_layers(vec![l])
}

#[test]
fn filler_items_reach_the_requested_size() {
    let defs = vec![hello_def()];
    // Auto: no filler, 3 + (2 + 2) octets.
    let auto = hello(None).build(&defs).unwrap();
    assert_eq!(auto.bytes, [1, 0, 7, 1, 2, b'r', b'1']);

    for total in [9u16, 10, 264, 265, 1497] {
        let built = hello(Some(total)).build(&defs).unwrap();
        assert!(
            built.diagnostics.is_empty(),
            "{total}: {:?}",
            built.diagnostics
        );
        assert_eq!(built.bytes.len(), usize::from(total));
        assert_eq!(&built.bytes[1..3], &total.to_be_bytes());
        let tlvs = &built.layout[0].fields[2];
        assert!(tlvs.auto, "filled field is reported as computed");
        // Every filler item is a well-formed type 8 TLV.
        let mut pos = 7;
        while pos < built.bytes.len() {
            assert_eq!(built.bytes[pos], 8, "{total}: item at {pos}");
            pos += 2 + usize::from(built.bytes[pos + 1]);
        }
        assert_eq!(pos, built.bytes.len());

        let parsed = parse(&built.bytes, "hello", &defs).unwrap();
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        assert_eq!(parsed.packet.build(&defs).unwrap().bytes, built.bytes);
    }
}

#[test]
fn fill_counts_following_layers_and_reports_leftovers() {
    let mut defs = vec![hello_def()];
    defs.push(wpg_core::raw_protocol());
    let mut p = hello(Some(100));
    p.layers
        .push(Layer::new("raw").with("data", vec![0xee; 10]));
    let built = p.build(&defs).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    assert_eq!(built.bytes.len(), 100);
    assert_eq!(built.layout[0].header_len, 90);

    // One octet more than the unpadded hello: no TLV is that small.
    let built = hello(Some(8)).build(&defs).unwrap();
    assert_eq!(built.bytes.len(), 7);
    assert!(built
        .diagnostics
        .iter()
        .any(|d| d.key == keys::FILL_INEXACT));
    // Smaller than the content: nothing to fill, the mismatch is reported.
    let built = hello(Some(4)).build(&defs).unwrap();
    assert!(built
        .diagnostics
        .iter()
        .any(|d| d.key == keys::LENGTH_MISMATCH));
}

#[test]
fn patching_the_fill_target_rebuilds() {
    let defs = vec![hello_def()];
    let mut pre = Prebuilt::new(&hello(Some(64)), &defs).unwrap();
    let r = FieldRef::new("hello", "total");
    assert!(!pre.target(&r).unwrap().is_fast());
    let bytes = pre.patch(&[(r, FieldValue::UInt(300))]).unwrap().to_vec();
    assert_eq!(bytes, hello(Some(300)).build(&defs).unwrap().bytes);
}

#[test]
fn invalid_m7_extensions_are_rejected() {
    let mut d = hello_def();
    if let FieldType::Tlv(t) = &mut d.fields[2].ty {
        t.fill.as_mut().unwrap().until = "kind".into();
    }
    assert!(
        d.validate().is_err(),
        "fill until a field without length rule"
    );
    let mut d = hello_def();
    if let FieldType::Tlv(t) = &mut d.fields[2].ty {
        t.fill.as_mut().unwrap().variant = "nope".into();
    }
    assert!(d.validate().is_err(), "unknown filler variant");
    let mut d = hello_def();
    d.fields.push(uint("trailer", 8));
    assert!(d.validate().is_err(), "filled tlv not last");

    let mut d = prefix_def();
    d.fields[0].auto = Some(AutoCompute::Size {
        field: "nope".into(),
    });
    assert!(d.validate().is_err(), "size of an unknown field");
    let mut d = prefix_def();
    if let FieldType::List { length, .. } = &mut d.fields[1].ty {
        *length = Length::FromField {
            field: "size".into(),
            unit: 1,
            adjust: 0,
            div: 0,
        };
    }
    assert!(d.validate().is_err(), "division by zero");
    let mut d = prefix_def();
    d.fields[1].ty = FieldType::Bytes {
        len: Length::Count {
            field: "size".into(),
        },
    };
    assert!(d.validate().is_err(), "count length on bytes");
}
