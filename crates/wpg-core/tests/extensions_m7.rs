//! IR extensions for the IS-IS TLVs (M7): rounded-up prefix lengths
//! (`Length::FromField { div }`), count-bounded lists of variable-size elements
//! (`Length::Count`), octet-size Auto fields (`AutoCompute::Size`) and TLV filler
//! items up to a requested PDU size (`TlvFill`); for CLNP / ES-IS, regions that
//! end at a header length indicator (`Length::UntilOffset`); for SCTP, TLV value
//! octets between the type and the length (`TlvDef::head_octets`) and items padded
//! to a multiple of 4 octets (`TlvDef::align`); for BGP, variants told apart by
//! their value (`TlvVariant::value_prefix` / `value_len`) and a type flag that
//! widens the length (`TlvDef::long_length`); for RSVP, items whose length comes
//! before the type (`TlvDef::len_first`).

use std::collections::BTreeMap;

use wpg_core::diag::keys;
use wpg_core::stream::FieldRef;
use wpg_core::{
    parse, protocol, AutoCompute, FieldDef, FieldType, FieldValue, ItemDef, Layer, Length,
    LengthUnit, Packet, Prebuilt, ProtocolDef, TlvDef, TlvFill, TlvItem, TlvLongLength, TlvVariant,
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
        head_octets: 0,
        align: 0,
        unpadded_last: false,
        long_length: None,
        len_first: false,
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

/// A CLNP-like header: `li` (Auto header length), a variable-size NSAP, then
/// options up to `li`; anything after belongs to the next layer.
fn until_def() -> ProtocolDef {
    let d = protocol! {
        id: "until",
        name: "Until",
        layer: L3,
        fields: {
            li: uint(8), auto = length(header);
            address: nsap = vec![0x49, 0, 1];
            options: ty(FieldType::Tlv(tlv(
                vec![TlvVariant::new(0xcd, "priority", vec![uint("value", 8)])],
                Length::UntilOffset { field: "li".into(), unit: 1 },
            ))) = Vec::<TlvItem>::new();
        },
    };
    d.validate().unwrap();
    d
}

#[test]
fn regions_end_at_the_header_length() {
    let defs = vec![until_def(), wpg_core::raw_protocol()];
    let p = Packet::from_layers(vec![
        Layer::new("until").with(
            "options",
            vec![TlvItem::named("priority").with("value", 3u8)],
        ),
        Layer::new("raw").with("data", vec![0xee; 4]),
    ]);
    let built = p.build(&defs).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    assert_eq!(
        built.bytes,
        [8, 3, 0x49, 0, 1, 0xcd, 1, 3, 0xee, 0xee, 0xee, 0xee]
    );

    let parsed = parse(&built.bytes, "until", &defs).unwrap();
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let l = &parsed.packet.layers;
    assert_eq!(l.len(), 2);
    assert!(matches!(&l[0].fields["options"], FieldValue::Tlv(t) if t.len() == 1));
    assert_eq!(l[1].fields["data"], FieldValue::Bytes(vec![0xee; 4]));
    assert_eq!(parsed.packet.build(&defs).unwrap().bytes, built.bytes);

    // No options: the region is empty, everything after the header is payload.
    let parsed = parse(&[5, 3, 0x49, 0, 1, 0xcd, 1, 3], "until", &defs).unwrap();
    assert_eq!(
        parsed.packet.layers[0].fields["options"],
        FieldValue::Tlv(Vec::new())
    );
    assert_eq!(
        parsed.packet.layers[1].fields["data"],
        FieldValue::Bytes(vec![0xcd, 1, 3])
    );
    // A length pointing inside the address: empty region, not an underflow.
    let parsed = parse(&[1, 3, 0x49, 0, 1], "until", &defs).unwrap();
    assert_eq!(
        parsed.packet.layers[0].fields["options"],
        FieldValue::Tlv(Vec::new())
    );
    // A length beyond the data: clamped, reported.
    let parsed = parse(&[20, 3, 0x49, 0, 1, 0xcd, 1, 3], "until", &defs).unwrap();
    assert!(parsed
        .diagnostics
        .iter()
        .any(|d| d.key == keys::LENGTH_EXCEEDS_DATA));

    let mut d = until_def();
    if let FieldType::Tlv(t) = &mut d.fields[2].ty {
        t.length = Length::UntilOffset {
            field: "nope".into(),
            unit: 1,
        };
    }
    assert!(d.validate().is_err(), "until an unknown field");
}

/// Chunks shaped like SCTP: 8-bit type, 8-bit flags (head), 16-bit length counting
/// the header but not the padding to 4 octets.
fn chunk_def() -> ProtocolDef {
    let mut chunks = tlv(
        vec![
            TlvVariant::new(
                0,
                "data",
                vec![
                    uint("flags", 8).with_default(FieldValue::UInt(3)),
                    uint("tsn", 32),
                    FieldDef::new("user_data", FieldType::Bytes { len: Length::ToEnd }),
                ],
            ),
            TlvVariant::new(11, "cookie_ack", vec![uint("flags", 8)]),
        ],
        Length::ToEnd,
    );
    chunks.len_bits = 16;
    chunks.len_includes_header = true;
    chunks.head_octets = 1;
    chunks.align = 4;
    let mut d = ProtocolDef::new("chunks", "Chunks", wpg_core::ProtocolLayer::L4);
    d.fields = vec![FieldDef::new("chunks", FieldType::Tlv(chunks))];
    d
}

#[test]
fn head_octets_and_aligned_items() {
    let defs = vec![chunk_def()];
    let p = Packet::from_layers(vec![Layer::new("chunks").with(
        "chunks",
        vec![
            TlvItem::named("data")
                .with("tsn", 1u32)
                .with("user_data", vec![0xaa, 0xbb, 0xcc]),
            TlvItem::named("cookie_ack"),
            TlvItem::with_type(0xc0, vec![0x01, 0x02]),
        ],
    )]);
    let built = p.build(&defs).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    assert_eq!(
        built.bytes,
        [
            0, 3, 0, 11, 0, 0, 0, 1, 0xaa, 0xbb, 0xcc, 0, // data + 1 octet of padding
            11, 0, 0, 4, // cookie ack
            0xc0, 0x01, 0, 5, 0x02, 0, 0, 0, // unknown: flags 0x01, value 0x02
        ]
    );
    // Layout: flags right after the type, then the length, then the value.
    let data = &built.layout[0].fields[0].children[0];
    let names: Vec<_> = data
        .children
        .iter()
        .map(|c| (c.name.as_str(), c.bit_offset / 8, c.bit_len / 8))
        .collect();
    assert_eq!(
        names,
        [
            ("type", 0, 1),
            ("flags", 1, 1),
            ("length", 2, 2),
            ("tsn", 4, 4),
            ("user_data", 8, 3),
            ("padding", 11, 1)
        ]
    );
    assert_eq!(data.bit_len, 12 * 8);

    let parsed = parse(&built.bytes, "chunks", &defs).unwrap();
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let FieldValue::Tlv(items) = &parsed.packet.layers[0].fields["chunks"] else {
        panic!("not decoded as items");
    };
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].fields["flags"], FieldValue::UInt(3));
    assert_eq!(
        items[0].fields["user_data"],
        FieldValue::Bytes(vec![0xaa, 0xbb, 0xcc])
    );
    assert_eq!(items[2].data.as_deref(), Some(&[0x01, 0x02][..]));
    assert_eq!(parsed.packet.build(&defs).unwrap().bytes, built.bytes);

    // Non-zero padding cannot be reproduced: the region stays opaque.
    let mut odd = built.bytes.clone();
    odd[11] = 0xff;
    let parsed = parse(&odd, "chunks", &defs).unwrap();
    assert_eq!(parsed.packet.build(&defs).unwrap().bytes, odd);
    // The last item may lack its padding (it is added back when building).
    let short = &built.bytes[..11];
    let parsed = parse(short, "chunks", &defs).unwrap();
    assert_eq!(parsed.packet.build(&defs).unwrap().bytes, short);
}

/// BGP-like attributes: 16-bit type (flags << 8 | code), 8-bit length, 16-bit with
/// the `0x1000` flag; code 14 has one variant per value prefix (AFI / SAFI) and a
/// fallback, code 5 one variant per value length.
fn attr_def() -> ProtocolDef {
    let rest = || FieldDef::new("rest", FieldType::Bytes { len: Length::ToEnd });
    let mut attrs = tlv(
        vec![
            TlvVariant::new(
                0x800e,
                "mp_a",
                vec![uint("afi", 16), uint("safi", 8), rest()],
            )
            .when_value_starts(&[0, 1, 1]),
            TlvVariant::new(
                0x800e,
                "mp_b",
                vec![uint("afi", 16), uint("safi", 8), rest()],
            )
            .when_value_starts(&[0, 2, 1]),
            TlvVariant::new(
                0x800e,
                "mp",
                vec![FieldDef::new(
                    "data",
                    FieldType::Bytes { len: Length::ToEnd },
                )],
            ),
            TlvVariant::new(5, "short", vec![uint("v", 16)]).when_value_len(2),
            TlvVariant::new(5, "long", vec![uint("v", 32)]).when_value_len(4),
        ],
        Length::ToEnd,
    );
    attrs.type_bits = 16;
    attrs.long_length = Some(TlvLongLength {
        flag: 0x1000,
        len_bits: 16,
    });
    let mut d = ProtocolDef::new("attrs", "Attrs", wpg_core::ProtocolLayer::L7);
    d.fields = vec![FieldDef::new("attrs", FieldType::Tlv(attrs))];
    d
}

#[test]
fn variants_told_apart_by_value_and_long_lengths() {
    let defs = vec![attr_def()];
    defs[0].validate().unwrap();
    let p = Packet::from_layers(vec![Layer::new("attrs").with(
        "attrs",
        vec![
            TlvItem::named("mp_b")
                .with("afi", 2u16)
                .with("safi", 1u8)
                .with("rest", vec![0xaa]),
            TlvItem::named("long").with("v", 7u32),
            TlvItem::named("short").with("v", 7u16),
            // Too long for an 8-bit length: the Extended Length flag is set.
            TlvItem::named("mp").with("data", vec![0x55; 300]),
            // An explicit type keeps its form.
            TlvItem {
                ty: Some(0x900e),
                ..TlvItem::named("mp_a").with("afi", 1u16).with("safi", 1u8)
            },
            TlvItem::named("mp").with("data", vec![0, 3, 1]),
        ],
    )]);
    let built = p.build(&defs).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    let b = &built.bytes;
    assert_eq!(b[..7], [0x80, 0x0e, 4, 0, 2, 1, 0xaa]);
    assert_eq!(b[7..14], [0, 5, 4, 0, 0, 0, 7]);
    assert_eq!(b[14..19], [0, 5, 2, 0, 7]);
    assert_eq!(b[19..23], [0x90, 0x0e, 0x01, 0x2c]);
    assert_eq!(b[323..330], [0x90, 0x0e, 0, 3, 0, 1, 1]);
    assert_eq!(b[330..], [0x80, 0x0e, 3, 0, 3, 1]);

    let parsed = parse(b, "attrs", &defs).unwrap();
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let FieldValue::Tlv(items) = &parsed.packet.layers[0].fields["attrs"] else {
        panic!("not decoded as items");
    };
    let names: Vec<_> = items.iter().map(|i| i.name.as_deref().unwrap()).collect();
    assert_eq!(names, ["mp_b", "long", "short", "mp", "mp_a", "mp"]);
    assert_eq!(items[3].ty, Some(0x900e));
    assert_eq!(items[4].fields["afi"], FieldValue::UInt(1));
    assert_eq!(parsed.packet.build(&defs).unwrap().bytes, *b);

    // A value length no variant accepts and no fallback: kept as data.
    let odd = [0, 5, 3, 1, 2, 3];
    let parsed = parse(&odd, "attrs", &defs).unwrap();
    let FieldValue::Tlv(items) = &parsed.packet.layers[0].fields["attrs"] else {
        panic!("not decoded as items");
    };
    assert_eq!(items[0].name, None);
    assert_eq!(items[0].data.as_deref(), Some(&[1, 2, 3][..]));
    assert_eq!(parsed.packet.build(&defs).unwrap().bytes, odd);
}

#[test]
fn clashing_variants_are_rejected() {
    let mut d = attr_def();
    let FieldType::Tlv(t) = &mut d.fields[0].ty else {
        unreachable!()
    };
    // Same type once the long-length flag is cleared, no discriminator.
    t.variants
        .push(TlvVariant::new(0x900e, "mp_again", Vec::new()));
    assert!(d.validate().is_err());
}

/// RSVP-shaped objects: 16-bit length (counting the header, padded to 4 octets),
/// then a 16-bit type (Class-Num, C-Type).
fn object_def() -> ProtocolDef {
    let mut objects = tlv(
        vec![
            TlvVariant::new(0x0501, "time", vec![uint("refresh", 32)]),
            TlvVariant::new(
                0x0701,
                "name",
                vec![FieldDef::new(
                    "name",
                    FieldType::Bytes { len: Length::ToEnd },
                )],
            ),
        ],
        Length::ToEnd,
    );
    objects.type_bits = 16;
    objects.len_bits = 16;
    objects.len_includes_header = true;
    objects.align = 4;
    objects.len_first = true;
    let mut d = ProtocolDef::new("objects", "Objects", wpg_core::ProtocolLayer::L4);
    d.fields = vec![FieldDef::new("objects", FieldType::Tlv(objects))];
    d
}

#[test]
fn length_before_type() {
    let defs = vec![object_def()];
    defs[0].validate().unwrap();
    let p = Packet::from_layers(vec![Layer::new("objects").with(
        "objects",
        vec![
            TlvItem::named("time").with("refresh", 30_000u32),
            TlvItem::named("name").with("name", b"ab".to_vec()),
            TlvItem {
                ty: Some(0x0901),
                data: Some(vec![1, 2, 3, 4]),
                ..TlvItem::default()
            },
        ],
    )]);
    let built = p.build(&defs).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    let b = &built.bytes;
    assert_eq!(b[..8], [0, 8, 5, 1, 0, 0, 0x75, 0x30]);
    // Length 6, padded to 8 octets.
    assert_eq!(b[8..16], [0, 6, 7, 1, b'a', b'b', 0, 0]);
    assert_eq!(b[16..], [0, 8, 9, 1, 1, 2, 3, 4]);
    // The layout shows the length first.
    let item = &built.layout[0].fields[0].children[0];
    let names: Vec<_> = item.children.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["length", "type", "refresh"]);
    assert_eq!(item.children[1].bit_offset, 16);
    assert_eq!(item.children[2].bit_offset, 32);

    let parsed = parse(b, "objects", &defs).unwrap();
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let FieldValue::Tlv(items) = &parsed.packet.layers[0].fields["objects"] else {
        panic!("not decoded as items");
    };
    let names: Vec<_> = items.iter().map(|i| i.name.as_deref()).collect();
    assert_eq!(names, [Some("time"), Some("name"), None]);
    assert_eq!(items[1].fields["name"], FieldValue::Bytes(b"ab".to_vec()));
    assert_eq!(items[2].ty, Some(0x0901));
    assert_eq!(parsed.packet.build(&defs).unwrap().bytes, *b);

    // A truncated item (length 12, 8 octets left) becomes raw with a warning.
    let short = [0, 12, 5, 1, 0, 0, 0, 1];
    let parsed = parse(&short, "objects", &defs).unwrap();
    assert!(parsed
        .diagnostics
        .iter()
        .any(|d| d.key == keys::LENGTH_EXCEEDS_DATA));

    let mut bad = object_def();
    let FieldType::Tlv(t) = &mut bad.fields[0].ty else {
        unreachable!()
    };
    t.head_octets = 1;
    assert!(bad.validate().is_err());
    let mut bad = object_def();
    let FieldType::Tlv(t) = &mut bad.fields[0].ty else {
        unreachable!()
    };
    t.variants.push(TlvVariant::type_only(0x0001, "pad"));
    assert!(bad.validate().is_err());
}
