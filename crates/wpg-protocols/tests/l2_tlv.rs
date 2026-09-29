//! M1 phase B: typed TCP / IPv4 options (TLV engine), 802.1ad QinQ, 802.3 + LLC +
//! SNAP, minimum-frame padding, FCS and the payload generator.
//!
//! Expected bytes were produced independently (Python `struct` + textbook RFC 1071
//! checksums); the FCS is re-verified with a bitwise CRC-32 written here.

use std::net::Ipv4Addr;

use wpg_core::diag::keys;
use wpg_core::{
    parse, parse_with_fcs, DiagLevel, FcsMode, FieldLayout, FieldValue, Layer, Packet, TlvItem,
    TrailerKind,
};
use wpg_protocols::catalog;

fn hex(s: &str) -> Vec<u8> {
    let s: String = s.split_whitespace().collect();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// Bitwise (table-free) IEEE CRC-32.
fn naive_crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for b in data {
        crc ^= u32::from(*b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn protos(p: &Packet) -> Vec<&str> {
    p.layers.iter().map(|l| l.protocol.as_str()).collect()
}

fn eth() -> Layer {
    Layer::new("ethernet2")
        .with("dst", FieldValue::Mac([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]))
        .with("src", FieldValue::Mac([0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb]))
}

fn ipv4(id: u16, src: [u8; 4], dst: [u8; 4]) -> Layer {
    Layer::new("ipv4")
        .with("identification", id)
        .with("src", Ipv4Addr::from(src))
        .with("dst", Ipv4Addr::from(dst))
}

fn linux_syn_options() -> Vec<TlvItem> {
    vec![
        TlvItem::named("mss").with("value", 1460u16),
        TlvItem::named("sack_permitted"),
        TlvItem::named("timestamps")
            .with("tsval", 1u32)
            .with("tsecr", 0u32),
        TlvItem::named("nop"),
        TlvItem::named("ws").with("shift", 7u8),
    ]
}

fn syn() -> Packet {
    Packet::from_layers(vec![
        eth(),
        ipv4(0xbeef, [172, 16, 0, 1], [172, 16, 0, 2]),
        Layer::new("tcp")
            .with("src_port", 51000u16)
            .with("dst_port", 80u16)
            .with("seq", 0xdead_beefu32)
            .with("window", 64240u16)
            .with("options", linux_syn_options()),
    ])
}

const SYN: &str = "00112233445566778899aabb0800
    4500003cbeef0000400663a9ac100001ac100002
    c7380050deadbeef00000000a002faf08fc40000
    020405b40402080a000000010000000001030307";

#[test]
fn tcp_syn_options_golden() {
    let built = syn().build(catalog()).unwrap();
    assert_eq!(built.bytes, hex(SYN));
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    assert_eq!(
        &built.bytes[54..],
        hex("02 04 05 b4 04 02 08 0a 00 00 00 01 00 00 00 00 01 03 03 07")
    );

    // Layout: one child per option, each with type / length / value children.
    let tcp = &built.layout[2];
    let opts = tcp.fields.iter().find(|f| f.name == "options").unwrap();
    assert_eq!((opts.bit_offset, opts.bit_len), (54 * 8, 20 * 8));
    let names: Vec<_> = opts.children.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["mss", "sack_permitted", "timestamps", "nop", "ws"]);
    let ts = &opts.children[2];
    assert_eq!((ts.bit_offset, ts.bit_len), ((54 + 6) * 8, 80));
    let child = |f: &FieldLayout, n: &str| f.children.iter().find(|c| c.name == n).cloned();
    let len = child(ts, "length").unwrap();
    assert!(len.auto);
    assert_eq!(len.value, FieldValue::UInt(10));
    assert_eq!(len.bit_offset, (54 + 7) * 8);
    let tsval = child(ts, "tsval").unwrap();
    assert_eq!(
        (tsval.bit_offset, tsval.value),
        ((54 + 8) * 8, FieldValue::UInt(1))
    );
    assert_eq!(opts.children[3].children.len(), 1, "nop: type only");

    // Parse back to typed items.
    let parsed = parse(&built.bytes, "ethernet2", catalog()).unwrap();
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let FieldValue::Tlv(items) = &parsed.packet.layers[2].fields["options"] else {
        panic!("untyped options");
    };
    let expected: Vec<_> = linux_syn_options()
        .into_iter()
        .zip([2u64, 4, 8, 1, 3])
        .map(|(i, ty)| TlvItem { ty: Some(ty), ..i })
        .collect();
    assert_eq!(items, &expected);
    assert_eq!(
        parsed.packet.layers[2].fields["data_offset"],
        FieldValue::UInt(10)
    );
    assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, built.bytes);
}

#[test]
fn tcp_options_are_padded_and_sack_blocks_decode() {
    let block = |l: u32, r: u32| {
        FieldValue::Struct(
            [
                ("left".to_owned(), FieldValue::UInt(l.into())),
                ("right".to_owned(), FieldValue::UInt(r.into())),
            ]
            .into(),
        )
    };
    let options = vec![
        TlvItem::named("nop"),
        TlvItem::named("sack").with("blocks", vec![block(1, 2), block(3, 4)]),
        TlvItem::with_type(99, vec![0xaa]),
    ];
    let p = Packet::from_layers(vec![
        Layer::new("ipv4"),
        Layer::new("tcp").with("options", options.clone()),
    ]);
    let built = p.build(catalog()).unwrap();
    // 1 + 18 + 3 = 22 option octets → 2 zero (EOL) octets of padding, offset 11.
    assert_eq!(built.bytes[20 + 12] >> 4, 11);
    assert_eq!(
        &built.bytes[40..64],
        &hex("01 0512 00000001 00000002 00000003 00000004 6303aa 0000")[..]
    );
    let parsed = parse(&built.bytes, "ipv4", catalog()).unwrap();
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let FieldValue::Tlv(items) = &parsed.packet.layers[1].fields["options"] else {
        panic!("untyped options");
    };
    assert_eq!(items.len(), 5, "3 options + 2 EOL");
    assert_eq!(items[1].fields, options[1].fields);
    assert_eq!(items[2].data.as_deref(), Some(&[0xaa][..]));
    assert_eq!(items[2].name, None);
    assert_eq!(items[4].name.as_deref(), Some("eol"));
    assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, built.bytes);
}

#[test]
fn malformed_tlvs_roundtrip_verbatim() {
    // Manual wrong length: kept and warned.
    let p = Packet::from_layers(vec![
        Layer::new("ipv4"),
        Layer::new("tcp").with(
            "options",
            vec![
                TlvItem::named("mss").with("value", 1460u16).with_length(3),
                TlvItem::raw(vec![0x08, 0x0a, 0x00]),
            ],
        ),
    ]);
    let built = p.build(catalog()).unwrap();
    assert_eq!(&built.bytes[40..48], &hex("020305b4 080a0000")[..]);
    let d = built
        .diagnostics
        .iter()
        .find(|d| d.key == keys::LENGTH_MISMATCH)
        .expect("length mismatch warning");
    assert_eq!(
        (d.layer, d.field.as_deref()),
        (Some(1), Some("options[0].length"))
    );

    let parsed = parse(&built.bytes, "ipv4", catalog()).unwrap();
    assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, built.bytes);
    assert!(parsed
        .diagnostics
        .iter()
        .any(|d| d.key == keys::LENGTH_EXCEEDS_DATA && d.level == DiagLevel::Warning));
    let FieldValue::Tlv(items) = &parsed.packet.layers[1].fields["options"] else {
        panic!("untyped options");
    };
    // Length 3 frames `02 03 05` (too short for an MSS value: kept as data, the
    // length then matches), then kind `b4` claims 8 octets → raw rest.
    assert_eq!(items[0].name.as_deref(), Some("mss"));
    assert_eq!(items[0].length, None);
    assert_eq!(items[0].data.as_deref(), Some(&[0x05][..]));
    assert!(items.last().unwrap().raw.is_some());
}

#[test]
fn ipv4_router_alert_option() {
    let p = Packet::from_layers(vec![ipv4(9, [10, 3, 0, 1], [224, 0, 0, 22]).with(
        "options",
        vec![TlvItem::named("router_alert").with("value", 0u16)],
    )]);
    let built = p.build(catalog()).unwrap();
    assert_eq!(built.bytes[0], 0x46);
    assert_eq!(&built.bytes[20..24], &[0x94, 0x04, 0, 0]);
    let parsed = parse(&built.bytes, "ipv4", catalog()).unwrap();
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert_eq!(
        parsed.packet.layers[0].fields["options"],
        FieldValue::Tlv(vec![TlvItem {
            ty: Some(148),
            ..TlvItem::named("router_alert").with("value", 0u16)
        }])
    );
}

const QINQ: &str = "00112233445566778899aabb88a8 6064 8100 00c8 0800
    4500002000070000401166c20a0100010a010002
    0fa00fa1000cecb5 71696e71";

#[test]
fn qinq_golden() {
    let p = Packet::from_layers(vec![
        eth(),
        Layer::new("dot1ad").with("vid", 100u16).with("pcp", 3u8),
        Layer::new("dot1q").with("vid", 200u16),
        ipv4(7, [10, 1, 0, 1], [10, 1, 0, 2]),
        Layer::new("udp")
            .with("src_port", 4000u16)
            .with("dst_port", 4001u16),
        Layer::new("raw").with("data", b"qinq".to_vec()),
    ]);
    let built = p.build(catalog()).unwrap();
    let mut want = hex(QINQ);
    assert_eq!(want.len(), 54);
    want.resize(60, 0);
    assert_eq!(built.bytes, want);
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    let parsed = parse(&built.bytes, "ethernet2", catalog()).unwrap();
    assert_eq!(
        protos(&parsed.packet),
        ["ethernet2", "dot1ad", "dot1q", "ipv4", "udp", "raw"]
    );
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, built.bytes);
}

const DOT3: &str = "00112233445566778899aabb 0028 aaaa03 000000 0800
    4500002000080000401166bf0a0200010a020002
    13881389000cefdf 736e6170";

#[test]
fn dot3_llc_snap_golden_and_reparse() {
    let p = Packet::from_layers(vec![
        Layer::new("dot3")
            .with("dst", FieldValue::Mac([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]))
            .with("src", FieldValue::Mac([0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb])),
        Layer::new("llc"),
        Layer::new("snap"),
        ipv4(8, [10, 2, 0, 1], [10, 2, 0, 2]),
        Layer::new("udp")
            .with("src_port", 5000u16)
            .with("dst_port", 5001u16),
        Layer::new("raw").with("data", b"snap".to_vec()),
    ]);
    let built = p.build(catalog()).unwrap();
    let mut want = hex(DOT3);
    want.resize(60, 0);
    assert_eq!(built.bytes, want, "802.3 length excludes the padding");
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    let llc = &built.layout[1];
    assert!(llc.fields.iter().all(|f| f.name == "control" || f.auto));

    // Captures start as "Ethernet"; a length value re-decodes the frame as 802.3.
    let parsed = parse(&built.bytes, "ethernet2", catalog()).unwrap();
    assert_eq!(
        protos(&parsed.packet),
        ["dot3", "llc", "snap", "ipv4", "udp", "raw"]
    );
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert!(parsed.packet.pad_to_min);
    assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, built.bytes);

    // LLC for OSI: DSAP/SSAP 0xfe announce the (planned) CLNP layer.
    let p = Packet::from_layers(vec![
        Layer::new("dot3"),
        Layer::new("llc").with("dsap", 0xfeu8).with("ssap", 0xfeu8),
        Layer::new("raw").with("data", vec![0x81, 0x01]),
    ]);
    let bytes = p.build(catalog()).unwrap().bytes;
    assert_eq!(&bytes[12..17], &[0, 5, 0xfe, 0xfe, 0x03]);
    let parsed = parse(&bytes, "ethernet2", catalog()).unwrap();
    assert_eq!(protos(&parsed.packet), ["dot3", "llc", "raw"]);
    assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
}

fn small() -> Packet {
    Packet::from_layers(vec![
        eth(),
        ipv4(1, [10, 0, 0, 1], [10, 0, 0, 2]),
        Layer::new("udp"),
        Layer::new("raw").with("data", b"hi".to_vec()),
    ])
}

#[test]
fn padding_and_fcs() {
    let unpadded = small().unpadded().build(catalog()).unwrap().bytes;
    assert_eq!(unpadded.len(), 44);

    let built = small().with_fcs(FcsMode::Auto).build(catalog()).unwrap();
    assert_eq!(built.bytes.len(), 64);
    assert_eq!(&built.bytes[..44], &unpadded[..]);
    assert!(built.bytes[44..60].iter().all(|b| *b == 0));
    let crc = naive_crc32(&built.bytes[..60]);
    assert_eq!(&built.bytes[60..], &crc.to_le_bytes());
    let kinds: Vec<_> = built
        .trailers
        .iter()
        .map(|t| (t.kind, t.offset, t.len))
        .collect();
    assert_eq!(
        kinds,
        [(TrailerKind::Padding, 44, 16), (TrailerKind::Fcs, 60, 4)]
    );

    let bad = small().with_fcs(FcsMode::Bad).build(catalog()).unwrap();
    assert_eq!(&bad.bytes[60..], &(!crc).to_le_bytes());
    let none = small().with_fcs(FcsMode::None).build(catalog()).unwrap();
    assert_eq!(none.bytes.len(), 60);

    // parse() does not expect an FCS: it is left as trailing octets.
    let p = parse(&built.bytes, "ethernet2", catalog()).unwrap();
    assert_eq!(p.packet.build(catalog()).unwrap().bytes, built.bytes);

    // parse_with_fcs() verifies and strips it.
    let p = parse_with_fcs(&built.bytes, "ethernet2", catalog()).unwrap();
    assert_eq!(p.packet.fcs, Some(FcsMode::Auto));
    assert!(p.packet.pad_to_min);
    assert_eq!(protos(&p.packet), ["ethernet2", "ipv4", "udp", "raw"]);
    assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
    assert_eq!(p.trailers.len(), 2);
    assert_eq!(p.packet.build(catalog()).unwrap().bytes, built.bytes);

    let p = parse_with_fcs(&bad.bytes, "ethernet2", catalog()).unwrap();
    assert_eq!(p.packet.fcs, Some(FcsMode::Bad));
    assert!(p
        .diagnostics
        .iter()
        .any(|d| d.key == keys::CHECKSUM_MISMATCH && d.field.as_deref() == Some("fcs")));
    assert_eq!(p.packet.build(catalog()).unwrap().bytes, bad.bytes);

    let mut odd = built.bytes.clone();
    odd[63] ^= 0x01;
    let p = parse_with_fcs(&odd, "ethernet2", catalog()).unwrap();
    assert!(matches!(p.packet.fcs, Some(FcsMode::Manual(_))));
    assert_eq!(p.packet.build(catalog()).unwrap().bytes, odd);
}

#[test]
fn payload_generator() {
    let p = Packet::from_layers(vec![
        Layer::new("udp"),
        Layer::new("payload")
            .with("mode", 3u8)
            .with("data", b"wpg-".to_vec())
            .with("length", 12u32)
            .with("counter_offset", 4u32)
            .with("counter_width", 8u8)
            .with("counter_value", 0x0102u16),
    ]);
    let built = p.build(catalog()).unwrap();
    assert_eq!(&built.bytes[8..], b"wpg-\0\0\0\0\0\0\x01\x02");
    assert_eq!(
        built.bytes[5], 20,
        "udp length covers the generated payload"
    );
    let data = &built.layout[1].fields[0];
    assert_eq!((data.name.as_str(), data.bit_len), ("data", 96));
    assert!(data.auto);

    let random = |seed: u64| {
        Packet::from_layers(vec![Layer::new("payload")
            .with("mode", 2u8)
            .with("seed", seed)
            .with("length", 32u32)])
        .build(catalog())
        .unwrap()
        .bytes
    };
    assert_eq!(random(7).len(), 32);
    assert_eq!(random(7), random(7));
    assert_ne!(random(7), random(8));

    // Parsing a payload layer yields fixed mode with the octets as data.
    let parsed = parse(&built.bytes[8..], "payload", catalog()).unwrap();
    assert_eq!(
        parsed.packet.layers[0].fields["data"],
        FieldValue::Bytes(built.bytes[8..].to_vec())
    );
    assert_eq!(
        parsed.packet.build(catalog()).unwrap().bytes,
        &built.bytes[8..]
    );
}

#[test]
fn tlv_items_serde_is_readable() {
    let items = linux_syn_options();
    let json = serde_json::to_string(&FieldValue::Tlv(items[..2].to_vec())).unwrap();
    assert_eq!(
        json,
        r#"{"tlv":[{"name":"mss","fields":{"value":{"uint":1460}}},{"name":"sack_permitted"}]}"#
    );
    let back: FieldValue = serde_json::from_str(&json).unwrap();
    assert_eq!(back, FieldValue::Tlv(items[..2].to_vec()));
    let pkt = serde_json::to_string(&Packet::default()).unwrap();
    assert_eq!(pkt, r#"{"layers":[]}"#, "pad_to_min defaults to true");
    let p: Packet = serde_json::from_str(r#"{"layers":[],"fcs":"auto"}"#).unwrap();
    assert!(p.pad_to_min);
    assert_eq!(p.fcs, Some(FcsMode::Auto));
}
