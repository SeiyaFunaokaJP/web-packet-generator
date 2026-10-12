//! SCTP (M7): golden byte vectors, CRC-32C verification, chunk / parameter /
//! error cause padding, parse round-trips (including packets built by another
//! implementation) and proptests.
//!
//! Expected bytes were derived field-by-field (RFC 9260) and cross-checked with
//! tshark 4.2 (`sctp.checksum:CRC-32C`: checksum correct, chunks and parameters
//! decoded, no expert warning about parameter padding); the CRC-32C is re-verified
//! with a naive bitwise implementation. The "foreign" packets were built with
//! Scapy 2.6, which counts the padding of the last parameter in the chunk length.
//! An optional `tshark` cross-check runs when `WPG_TSHARK=1`.

use std::collections::BTreeMap;

use proptest::prelude::*;
use wpg_core::{parse, FieldValue, Layer, Packet, TlvItem};
use wpg_protocols::catalog;

mod tshark_util;
use tshark_util::assert_dissects;

fn hex(s: &str) -> Vec<u8> {
    let s: String = s.split_whitespace().collect();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn assert_golden(bytes: &[u8], want: &str) {
    assert_eq!(to_hex(bytes), to_hex(&hex(want)));
}

/// RFC 9260 appendix A: CRC-32C (reflected polynomial `0x82f63b78`) of the packet
/// with a zero checksum field, stored least significant octet first.
fn crc_verifies(sctp: &[u8]) -> bool {
    let mut data = sctp.to_vec();
    data[8..12].fill(0);
    let mut crc = !0u32;
    for b in &data {
        crc ^= u32::from(*b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0x82f6_3b78
            } else {
                crc >> 1
            };
        }
    }
    sctp[8..12] == (!crc).to_le_bytes()
}

fn s(fields: &[(&str, FieldValue)]) -> FieldValue {
    FieldValue::Struct(
        fields
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect::<BTreeMap<_, _>>(),
    )
}

fn ipv4_frame(sctp: Layer) -> Vec<Layer> {
    vec![Layer::new("ethernet2"), Layer::new("ipv4"), sctp]
}

fn build(layers: Vec<Layer>) -> Vec<u8> {
    let built = Packet::from_layers(layers)
        .unpadded()
        .build(catalog())
        .unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    built.bytes
}

fn roundtrip(bytes: &[u8]) -> Packet {
    let parsed = parse(bytes, "ethernet2", catalog()).unwrap();
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    parsed.packet
}

fn protos(p: &Packet) -> Vec<&str> {
    p.layers.iter().map(|l| l.protocol.as_str()).collect()
}

/// The decoded chunks of the SCTP layer (`layer`), as items.
fn items(p: &Packet, layer: usize, field: &str) -> Vec<TlvItem> {
    match &p.layers[layer].fields[field] {
        FieldValue::Tlv(t) => t.clone(),
        other => panic!("`{field}` not decoded as items: {other:?}"),
    }
}

fn names(items: &[TlvItem]) -> Vec<&str> {
    items
        .iter()
        .map(|i| i.name.as_deref().unwrap_or("?"))
        .collect()
}

/// INIT with the defaults: verification tag 0, initiate tag 1, `a_rwnd` 106496,
/// 10 / 65535 streams, initial TSN 1.
const INIT: &str = "c000 0009 00000000 322b6224 \
    01 00 0014 00000001 0001a000 000a ffff 00000001";

#[test]
fn init_golden() {
    let bytes = build(ipv4_frame(Layer::new("sctp")));
    assert_eq!(bytes[23], 132, "IPv4 protocol is Auto");
    let sctp = &bytes[34..];
    assert_golden(sctp, INIT);
    assert!(crc_verifies(sctp));
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "ipv4", "sctp"]);
    assert_eq!(names(&items(&p, 2, "chunks")), ["init"]);
    assert_dissects(&bytes, None, "sctp");
}

#[test]
fn init_parameters_leave_the_last_padding_to_the_chunk() {
    let one = |v: u64| vec![FieldValue::UInt(v)];
    let init = TlvItem::named("init").with(
        "parameters",
        vec![
            TlvItem::named("ipv4_address").with("address", FieldValue::Ipv4([192, 0, 2, 1].into())),
            TlvItem::named("supported_address_types").with("types", one(5)),
            TlvItem::named("ecn_capable"),
            TlvItem::named("forward_tsn_supported"),
            TlvItem::named("supported_extensions").with("chunk_types", one(0xc0)),
        ],
    );
    let bytes = build(ipv4_frame(Layer::new("sctp").with("chunks", vec![init])));
    let sctp = &bytes[34..];
    // Chunk length 49: the 3 octets after the last parameter are the chunk padding.
    assert_golden(
        sctp,
        "c000 0009 00000000 47a32737 01 00 0031 00000001 0001a000 000a ffff 00000001 \
         0005 0008 c0000201  000c 0006 0005 0000  8000 0004  c000 0004 \
         8008 0005 c0 000000",
    );
    assert!(crc_verifies(sctp));
    let p = roundtrip(&bytes);
    let params = match &items(&p, 2, "chunks")[0].fields["parameters"] {
        FieldValue::Tlv(t) => t.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        names(&params),
        [
            "ipv4_address",
            "supported_address_types",
            "ecn_capable",
            "forward_tsn_supported",
            "supported_extensions"
        ]
    );
    assert_dissects(&bytes, None, "sctp");
}

#[test]
fn sack_and_data_bundle() {
    let layer = Layer::new("sctp")
        .with("verification_tag", 0x1122_3344u32)
        .with(
            "chunks",
            vec![
                TlvItem::named("sack").with(
                    "gap_ack_blocks",
                    vec![s(&[
                        ("start", FieldValue::UInt(2)),
                        ("end", FieldValue::UInt(3)),
                    ])],
                ),
                TlvItem::named("data")
                    .with("tsn", 5u32)
                    .with("payload_protocol_identifier", 46u32)
                    .with("user_data", vec![1u8, 2, 3, 4, 5]),
            ],
        );
    let bytes = build(ipv4_frame(layer));
    let sctp = &bytes[34..];
    // SACK: one gap block (count Auto); DATA: flags B|E, length 21 + 3 padding.
    assert_golden(
        sctp,
        "c000 0009 11223344 af73eb1a \
         03 00 0014 00000001 0001a000 0001 0000 0002 0003 \
         00 03 0015 00000005 0000 0000 0000002e 0102030405 000000",
    );
    assert!(crc_verifies(sctp));
    let p = roundtrip(&bytes);
    let chunks = items(&p, 2, "chunks");
    assert_eq!(names(&chunks), ["sack", "data"]);
    assert_eq!(chunks[1].fields["flags"], FieldValue::UInt(3));
    assert_eq!(
        chunks[1].fields["user_data"],
        FieldValue::Bytes(vec![1, 2, 3, 4, 5])
    );
    assert_dissects(&bytes, None, "data");
}

#[test]
fn abort_with_error_causes() {
    let abort = TlvItem::named("abort").with("flags", 1u8).with(
        "causes",
        vec![
            TlvItem::named("user_initiated_abort").with("reason", b"bye".to_vec()),
            TlvItem::named("missing_mandatory_parameter")
                .with("missing_parameters", vec![FieldValue::UInt(7)]),
        ],
    );
    let bytes = build(ipv4_frame(Layer::new("sctp").with("chunks", vec![abort])));
    let sctp = &bytes[34..];
    // T bit; the number of missing parameters is Auto; chunk length 22.
    assert_golden(
        sctp,
        "c000 0009 00000000 ea72afb5 06 01 0016 \
         000c 0007 627965 00  0002 000a 00000001 0007 0000",
    );
    assert!(crc_verifies(sctp));
    roundtrip(&bytes);
    assert_dissects(&bytes, None, "sctp");
}

#[test]
fn heartbeat_over_ipv6() {
    let layer = Layer::new("sctp").with(
        "chunks",
        vec![
            TlvItem::named("heartbeat").with(
                "parameters",
                vec![TlvItem::named("heartbeat_info").with("info", vec![9u8; 6])],
            ),
            TlvItem::named("shutdown_complete").with("flags", 1u8),
        ],
    );
    let bytes = build(vec![Layer::new("ethernet2"), Layer::new("ipv6"), layer]);
    assert_eq!(bytes[20], 132, "IPv6 next header is Auto");
    let sctp = &bytes[54..];
    assert_golden(
        sctp,
        "c000 0009 00000000 b505986a 04 00 000e 0001 000a 090909090909 0000 0e 01 0004",
    );
    assert!(crc_verifies(sctp));
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "ipv6", "sctp"]);
    assert_dissects(&bytes, None, "sctp");
}

#[test]
fn sctp_over_udp_and_extension_chunks() {
    let layer = Layer::new("sctp").with(
        "chunks",
        vec![
            TlvItem::named("cookie_echo").with("cookie", vec![7u8; 5]),
            TlvItem::named("i_data").with("user_data", vec![1u8]),
            TlvItem::named("forward_tsn").with("new_cumulative_tsn", 9u32),
            TlvItem::named("asconf").with("serial_number", 1u32).with(
                "parameters",
                vec![
                    TlvItem::named("ipv4_address")
                        .with("address", FieldValue::Ipv4([192, 0, 2, 1].into())),
                    TlvItem::named("add_ip_address")
                        .with("correlation_id", 7u32)
                        .with(
                            "address",
                            vec![TlvItem::named("ipv4_address")
                                .with("address", FieldValue::Ipv4([192, 0, 2, 9].into()))],
                        ),
                ],
            ),
            TlvItem::named("re_config").with(
                "parameters",
                vec![TlvItem::named("outgoing_ssn_reset_request")
                    .with("request_sequence", 1u32)
                    .with("streams", vec![FieldValue::UInt(1)])],
            ),
        ],
    );
    let bytes = build(vec![
        Layer::new("ethernet2"),
        Layer::new("ipv4"),
        Layer::new("udp").with("dst_port", 9899u16),
        layer,
    ]);
    let sctp = &bytes[42..];
    assert_golden(
        sctp,
        "c000 0009 00000000 90889784 \
         0a 00 0009 0707070707 000000 \
         40 03 0015 00000001 0000 0000 00000000 00000000 01 000000 \
         c0 00 0008 00000009 \
         c1 00 0020 00000001 0005 0008 c0000201 c001 0010 00000007 0005 0008 c0000209 \
         82 00 0016 000d 0012 00000001 00000000 00000000 0001 0000",
    );
    assert!(crc_verifies(sctp));
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "ipv4", "udp", "sctp"]);
    assert_eq!(
        names(&items(&p, 3, "chunks")),
        [
            "cookie_echo",
            "i_data",
            "forward_tsn",
            "asconf",
            "re_config"
        ]
    );
    assert_dissects(&bytes, None, "sctp");
}

/// Packets built by Scapy 2.6 (Ethernet + IPv4 / IPv6 + SCTP).
const FOREIGN: [&str; 4] = [
    // INIT with IPv4 address, supported address types (IPv4, IPv6), ECN, FORWARD-TSN.
    "ffffffffffff02fc0000000108004500004c000100004084f629c0000201c0000202\
     8e3c8e3c00000000982874c50100002c000012340001a000000affff00000063\
     00050008c0000201000c00080005000680000004c0000004",
    // DATA "hello" + SACK with a gap block and a duplicate TSN.
    "ffffffffffff0000000000000800450000500001000040847c277f0000017f000001\
     00000000000000054f712e6c0000001500000003000100020000001268656c6c6f000000\
     0300001800000002000003e8000100010002000300000007",
    // HEARTBEAT (7-octet info, its padding counted in the chunk length) + COOKIE ACK.
    "ffffffffffff00000000000086dd600000000020844000000000000000000000000000000001\
     00000000000000000000000000000001000000000000000582cd0391\
     040000100001000b61626364656667000b000004",
    // ABORT with a user initiated abort cause (padding counted) + SHUTDOWN COMPLETE.
    "ffffffffffff0000000000000800450000300001000040847c477f0000017f000001\
     0000000000000005374d21cf0600000c000c0007627965000e000004",
];

#[test]
fn foreign_packets_decode_and_roundtrip() {
    let want: [&[&str]; 4] = [
        &["init"],
        &["data", "sack"],
        &["heartbeat", "cookie_ack"],
        &["abort", "shutdown_complete"],
    ];
    for (h, want) in FOREIGN.iter().zip(want) {
        let bytes = hex(h);
        let l3 = if bytes[12] == 0x86 { 54 } else { 34 };
        assert!(crc_verifies(&bytes[l3..]));
        let p = roundtrip(&bytes);
        let chunks = items(&p, 2, "chunks");
        assert_eq!(names(&chunks), want);
        // Every chunk is typed (parameters / causes included), not opaque octets.
        for c in &chunks {
            assert!(c.data.is_none() && c.raw.is_none(), "{c:?}");
            for v in c.fields.values() {
                assert!(!matches!(v, FieldValue::Bytes(b) if b.len() > 8), "{c:?}");
            }
        }
    }
    // The heartbeat info keeps its padding inside the chunk: an empty trailing
    // item records it.
    let p = roundtrip(&hex(FOREIGN[2]));
    let FieldValue::Tlv(params) = &items(&p, 2, "chunks")[0].fields["parameters"] else {
        panic!("parameters not decoded");
    };
    assert_eq!(params.len(), 2);
    assert_eq!(
        params[0].fields["info"],
        FieldValue::Bytes(b"abcdefg".to_vec())
    );
    assert_eq!(params[1].raw.as_deref(), Some(&[][..]));
}

#[test]
fn manual_checksums_and_unknown_chunks_are_kept() {
    let layer = Layer::new("sctp")
        .with("checksum", 0xdead_beefu32)
        .with("chunks", vec![TlvItem::with_type(0x99, vec![0x80, 1, 2])]);
    let built = Packet::from_layers(ipv4_frame(layer))
        .unpadded()
        .build(catalog())
        .unwrap();
    let sctp = &built.bytes[34..];
    // Unknown chunk: flags 0x80 from the first data octet, length 6, padding.
    assert_golden(sctp, "c000 0009 00000000 deadbeef 99 80 0006 0102 0000");
    assert!(!crc_verifies(sctp));
    let p = parse(&built.bytes, "ethernet2", catalog()).unwrap().packet;
    assert_eq!(p.build(catalog()).unwrap().bytes, built.bytes);
    assert_eq!(
        p.layers[2].fields["checksum"],
        FieldValue::UInt(0xdead_beef)
    );
    let chunks = items(&p, 2, "chunks");
    assert_eq!(chunks[0].data.as_deref(), Some(&[0x80, 1, 2][..]));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn data_bundles_roundtrip_and_crc_verifies(
        tag in any::<u32>(),
        data in proptest::collection::vec(
            (any::<u32>(), any::<u16>(), proptest::collection::vec(any::<u8>(), 0..40)),
            1..5,
        ),
        sack in proptest::option::of(proptest::collection::vec((any::<u16>(), any::<u16>()), 0..4)),
    ) {
        let mut chunks: Vec<TlvItem> = data
            .into_iter()
            .map(|(tsn, sid, user)| {
                TlvItem::named("data")
                    .with("tsn", tsn)
                    .with("stream_identifier", sid)
                    .with("user_data", user)
            })
            .collect();
        if let Some(gaps) = sack {
            let gaps: Vec<FieldValue> = gaps
                .into_iter()
                .map(|(a, b)| s(&[("start", FieldValue::UInt(a.into())), ("end", FieldValue::UInt(b.into()))]))
                .collect();
            chunks.push(TlvItem::named("sack").with("gap_ack_blocks", gaps));
        }
        let n = chunks.len();
        let layer = Layer::new("sctp").with("verification_tag", tag).with("chunks", chunks);
        let bytes = build(vec![Layer::new("ipv4"), layer]);
        prop_assert!(crc_verifies(&bytes[20..]));
        prop_assert_eq!((bytes.len() - 20) % 4, 0);
        let parsed = parse(&bytes, "ipv4", catalog()).unwrap();
        prop_assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        prop_assert_eq!(items(&parsed.packet, 1, "chunks").len(), n);
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    }
}
