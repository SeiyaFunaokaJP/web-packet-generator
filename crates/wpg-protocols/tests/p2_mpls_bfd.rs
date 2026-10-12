//! LDP, RSVP-TE and BFD (M7): golden byte vectors, Auto lengths / checksum /
//! flags, value-discriminated TLVs, RSVP objects with the length before the type,
//! parse round-trips and proptests.
//!
//! Expected bytes were derived field-by-field (RFC 5036, RFC 2205 / RFC 3209,
//! RFC 5880) and cross-checked with tshark 4.2 (messages, TLVs, objects,
//! subobjects and authentication sections decoded, RSVP checksum `[correct]`, no
//! expert warning). The RSVP checksum is re-verified with a naive one's
//! complement sum. An optional `tshark` cross-check runs when `WPG_TSHARK=1`.

use std::collections::BTreeMap;
use std::net::Ipv4Addr;

use proptest::prelude::*;
use wpg_core::diag::keys;
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

fn s(fields: &[(&str, FieldValue)]) -> FieldValue {
    FieldValue::Struct(
        fields
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect::<BTreeMap<_, _>>(),
    )
}

/// RFC 1071 one's complement sum of `data` (zero when the checksum verifies).
fn ones_complement(data: &[u8]) -> u16 {
    let mut sum: u32 = data
        .chunks(2)
        .map(|c| u32::from(c[0]) << 8 | u32::from(*c.get(1).unwrap_or(&0)))
        .sum();
    while sum > 0xffff {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !u16::try_from(sum).unwrap()
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

fn udp(src: u16, dst: u16) -> Layer {
    Layer::new("udp")
        .with("src_port", src)
        .with("dst_port", dst)
}

fn tcp(src: u16, dst: u16) -> Layer {
    Layer::new("tcp")
        .with("src_port", src)
        .with("dst_port", dst)
        .with("flags", 0x18u8)
}

fn ipv4_addrs(list: &[[u8; 4]]) -> FieldValue {
    FieldValue::List(list.iter().map(|a| FieldValue::Ipv4((*a).into())).collect())
}

// ---------------------------------------------------------------------------
// LDP
// ---------------------------------------------------------------------------

#[test]
fn ldp_hello_golden() {
    let bytes = build(vec![
        Layer::new("ethernet2"),
        Layer::new("ipv4").with("dst", Ipv4Addr::new(224, 0, 0, 2)),
        udp(646, 646),
        Layer::new("ldp"),
    ]);
    // Version 1, PDU length 22, LDP ID 192.0.2.1:0, Hello (ID 1) with Common Hello
    // Parameters (hold time 15).
    assert_golden(
        &bytes[42..],
        "0001 0016 c0000201 0000 0100 000c 00000001 0400 0004 000f 0000",
    );
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "ipv4", "udp", "ldp"]);
    assert_eq!(names(&items(&p, 3, "messages")), ["hello"]);
    assert_dissects(&bytes, None, "ldp");
}

#[test]
fn ldp_targeted_hello_with_transport_address() {
    let hello = TlvItem::named("hello").with(
        "tlvs",
        vec![
            TlvItem::named("common_hello_parameters")
                .with("hold_time", 45u16)
                .with("t", 1u8)
                .with("r", 1u8),
            TlvItem::named("ipv4_transport_address").with("address", Ipv4Addr::new(192, 0, 2, 1)),
            TlvItem::named("configuration_sequence_number").with("sequence_number", 3u32),
        ],
    );
    let bytes = build(vec![
        Layer::new("ethernet2"),
        Layer::new("ipv4"),
        udp(646, 646),
        Layer::new("ldp").with("messages", vec![hello]),
    ]);
    assert_golden(
        &bytes[42..],
        "0001 0026 c0000201 0000 0100 001c 00000001 \
         0400 0004 002d c000 0401 0004 c0000201 0402 0004 00000003",
    );
    let p = roundtrip(&bytes);
    let tlvs = match &items(&p, 3, "messages")[0].fields["tlvs"] {
        FieldValue::Tlv(t) => t.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        names(&tlvs),
        [
            "common_hello_parameters",
            "ipv4_transport_address",
            "configuration_sequence_number"
        ]
    );
    assert_dissects(&bytes, None, "ldp");
}

/// Label Mapping (two prefix FEC elements, generic label), Address, Label
/// Withdraw with the wildcard FEC, `KeepAlive` and Notification in one PDU.
fn session_pdu() -> Layer {
    let prefix = |plen: u8, p: &[u8]| {
        s(&[
            ("prefix_length", FieldValue::UInt(plen.into())),
            ("prefix", FieldValue::Bytes(p.to_vec())),
        ])
    };
    Layer::new("ldp").with(
        "messages",
        vec![
            TlvItem::named("label_mapping")
                .with("message_id", 7u32)
                .with(
                    "tlvs",
                    vec![
                        TlvItem::named("fec").with(
                            "elements",
                            FieldValue::List(vec![
                                prefix(24, &[198, 51, 100]),
                                prefix(32, &[192, 0, 2, 1]),
                            ]),
                        ),
                        TlvItem::named("generic_label").with("label", 299_776u32),
                    ],
                ),
            TlvItem::named("address").with("message_id", 8u32).with(
                "tlvs",
                vec![TlvItem::named("address_list")
                    .with("addresses", ipv4_addrs(&[[192, 0, 2, 1], [10, 0, 0, 1]]))],
            ),
            TlvItem::named("label_withdraw")
                .with("message_id", 9u32)
                .with("tlvs", vec![TlvItem::named("fec_wildcard")]),
            TlvItem::named("keepalive").with("message_id", 10u32),
            TlvItem::named("notification")
                .with("message_id", 11u32)
                .with(
                    "tlvs",
                    vec![TlvItem::named("status")
                        .with("e", 1u8)
                        .with("status_code", 0x0au32)],
                ),
        ],
    )
}

#[test]
fn ldp_session_messages_golden() {
    let bytes = build(vec![
        Layer::new("ethernet2"),
        Layer::new("ipv4"),
        tcp(646, 49152),
        session_pdu(),
    ]);
    assert_golden(
        &bytes[54..],
        "0001 006a c0000201 0000 \
         0400 001f 00000007 0100 000f 02 0001 18 c63364 02 0001 20 c0000201 \
                            0200 0004 00049300 \
         0300 0012 00000008 0101 000a 0001 c0000201 0a000001 \
         0402 0009 00000009 0100 0001 01 \
         0201 0004 0000000a \
         0001 0012 0000000b 0300 000a 8000000a 00000000 0000",
    );
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "ipv4", "tcp", "ldp"]);
    let messages = items(&p, 3, "messages");
    assert_eq!(
        names(&messages),
        [
            "label_mapping",
            "address",
            "label_withdraw",
            "keepalive",
            "notification"
        ]
    );
    let tlv_names = |i: usize| match &messages[i].fields["tlvs"] {
        FieldValue::Tlv(t) => names(t).into_iter().map(str::to_owned).collect::<Vec<_>>(),
        other => panic!("{other:?}"),
    };
    assert_eq!(tlv_names(0), ["fec", "generic_label"]);
    assert_eq!(tlv_names(1), ["address_list"]);
    // The wildcard element is told apart from prefix elements by its first octet.
    assert_eq!(tlv_names(2), ["fec_wildcard"]);
    assert_eq!(tlv_names(4), ["status"]);
    assert_dissects(&bytes, None, "ldp");
}

#[test]
fn ldp_ipv6_address_list_and_transport_address() {
    let v6 = |last: u16| FieldValue::Ipv6([0x2001, 0xdb8, 0, 0, 0, 0, 0, last].into());
    let ldp = Layer::new("ldp").with(
        "messages",
        vec![
            TlvItem::named("address").with(
                "tlvs",
                vec![TlvItem::named("address_list_ipv6")
                    .with("addresses", FieldValue::List(vec![v6(1)]))],
            ),
            TlvItem::named("hello").with(
                "tlvs",
                vec![
                    TlvItem::named("common_hello_parameters"),
                    TlvItem::named("ipv6_transport_address").with("address", v6(1)),
                ],
            ),
        ],
    );
    let bytes = build(vec![
        Layer::new("ethernet2"),
        Layer::new("ipv6"),
        tcp(49152, 646),
        ldp,
    ]);
    let ldp = &bytes[74..];
    // Address List: family 2 (IPv6), one address.
    assert_eq!(ldp[10..24], hex("0300 001a 00000001 0101 0012 0002")[..]);
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "ipv6", "tcp", "ldp"]);
    let messages = items(&p, 3, "messages");
    let FieldValue::Tlv(tlvs) = &messages[0].fields["tlvs"] else {
        panic!()
    };
    assert_eq!(names(tlvs), ["address_list_ipv6"]);
    assert_dissects(&bytes, None, "ldp");
}

#[test]
fn ldp_messages_end_at_the_pdu_length() {
    // A second PDU in the same segment is not read as messages of the first.
    let second = hex("0001 000e c0000201 0000 0201 0004 00000002");
    let bytes = build(vec![
        Layer::new("ethernet2"),
        Layer::new("ipv4"),
        tcp(646, 49152),
        Layer::new("ldp").with("messages", vec![TlvItem::named("keepalive")]),
        Layer::new("raw").with("data", second.clone()),
    ]);
    assert_eq!(bytes[54..58], hex("0001 000e")[..]);
    let p = roundtrip(&bytes);
    assert_eq!(names(&items(&p, 3, "messages")), ["keepalive"]);
    assert_eq!(protos(&p), ["ethernet2", "ipv4", "tcp", "ldp", "raw"]);
}

#[test]
fn ldp_unknown_tlvs_keep_their_octets() {
    // A vendor-private TLV with the U bit (0xbe00) inside an Initialization.
    let init = hex("0001 0028 c0000201 0000 0200 001e 00000001 \
         0500 000e 0001 00b4 0000 0000 c0000202 0000 \
         be00 0004 deadbeef");
    let frame = build(vec![
        Layer::new("ethernet2"),
        Layer::new("ipv4"),
        tcp(646, 49152),
        Layer::new("raw").with("data", init.clone()),
    ]);
    let parsed = parse(&frame, "ethernet2", catalog()).unwrap();
    let p = &parsed.packet;
    assert_eq!(protos(p), ["ethernet2", "ipv4", "tcp", "ldp"]);
    let FieldValue::Tlv(tlvs) = &items(p, 3, "messages")[0].fields["tlvs"] else {
        panic!()
    };
    assert_eq!(tlvs[0].name.as_deref(), Some("common_session_parameters"));
    assert_eq!(tlvs[1].ty, Some(0xbe00));
    assert_eq!(tlvs[1].data.as_deref(), Some(&hex("deadbeef")[..]));
    assert_eq!(&p.build(catalog()).unwrap().bytes[54..], &init[..]);
}

// ---------------------------------------------------------------------------
// RSVP-TE
// ---------------------------------------------------------------------------

/// The default Path: SESSION (LSP tunnel), `RSVP_HOP`, `TIME_VALUES`, `LABEL_REQUEST`,
/// `SESSION_ATTRIBUTE` ("tunnel1", padded), `SENDER_TEMPLATE`, `SENDER_TSPEC`.
const PATH: &str = "1001 ed8c 40 00 0074 \
    0010 0107 c0000202 0000 0001 c0000201 \
    000c 0301 c0000201 00000000 \
    0008 0501 00007530 \
    0008 1301 0000 0800 \
    0010 cf07 07 07 04 07 74756e6e656c31 00 \
    000c 0b07 c0000201 0000 0001 \
    0024 0c02 0000 0007 01 00 0006 7f 00 0005 47f42400 447a0000 7f800000 00000000 000005dc";

#[test]
fn rsvp_path_golden() {
    let bytes = build(vec![
        Layer::new("ethernet2"),
        Layer::new("ipv4"),
        Layer::new("rsvp"),
    ]);
    assert_eq!(bytes[23], 46, "IPv4 protocol is Auto");
    let rsvp = &bytes[34..];
    assert_golden(rsvp, PATH);
    assert_eq!(ones_complement(rsvp), 0, "checksum verifies");
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "ipv4", "rsvp"]);
    assert_eq!(
        names(&items(&p, 2, "objects")),
        [
            "session_lsp_tunnel_ipv4",
            "rsvp_hop_ipv4",
            "time_values",
            "label_request",
            "session_attribute",
            "sender_template_lsp_tunnel_ipv4",
            "sender_tspec"
        ]
    );
    assert_dissects(&bytes, None, "rsvp");
}

#[test]
fn rsvp_path_with_router_alert_ero_and_rro() {
    let objects = vec![
        TlvItem::named("session_lsp_tunnel_ipv4"),
        TlvItem::named("rsvp_hop_ipv4"),
        TlvItem::named("time_values"),
        TlvItem::named("explicit_route").with(
            "subobjects",
            vec![
                TlvItem::named("ipv4_prefix").with("address", Ipv4Addr::new(10, 0, 12, 2)),
                TlvItem::named("ipv4_prefix_loose").with("address", Ipv4Addr::new(192, 0, 2, 2)),
                TlvItem::named("unnumbered_interface").with("interface_id", 5u32),
            ],
        ),
        TlvItem::named("label_request"),
        TlvItem::named("session_attribute_ra")
            .with("flags", 0x07u8)
            .with("name", b"to-pe2".to_vec()),
        TlvItem::named("sender_template_lsp_tunnel_ipv4"),
        TlvItem::named("sender_tspec"),
        TlvItem::named("record_route").with("subobjects", vec![TlvItem::named("ipv4_address")]),
    ];
    let ip = Layer::new("ipv4").with(
        "options",
        vec![TlvItem::named("router_alert").with("value", 0u16)],
    );
    let bytes = build(vec![
        Layer::new("ethernet2"),
        ip,
        Layer::new("rsvp").with("objects", objects),
    ]);
    assert_eq!(bytes[14], 0x46, "IHL 6 with the Router Alert option");
    let rsvp = &bytes[38..];
    assert_eq!(ones_complement(rsvp), 0);
    // ERO: strict IPv4, loose IPv4 (L bit), unnumbered interface.
    assert_eq!(
        to_hex(&rsvp[44..76]),
        "0020140101080a000c0220008108c00002022000040c0000c000020200000005"
    );
    // SESSION_ATTRIBUTE with affinities: name length 6, padded to 28 octets.
    assert_golden(
        &rsvp[84..112],
        "001c cf01 00000000 00000000 00000000 07 07 07 06 746f2d706532 0000",
    );
    let p = roundtrip(&bytes);
    let objects = items(&p, 2, "objects");
    assert_eq!(objects[5].name.as_deref(), Some("session_attribute_ra"));
    assert_eq!(
        objects[5].fields["name"],
        FieldValue::Bytes(b"to-pe2".to_vec())
    );
    let FieldValue::Tlv(ero) = &objects[3].fields["subobjects"] else {
        panic!()
    };
    assert_eq!(
        names(ero),
        ["ipv4_prefix", "ipv4_prefix_loose", "unnumbered_interface"]
    );
    assert_dissects(&bytes, None, "rsvp");
}

#[test]
fn rsvp_resv_golden() {
    let objects = vec![
        TlvItem::named("session_lsp_tunnel_ipv4"),
        TlvItem::named("rsvp_hop_ipv4").with("address", Ipv4Addr::new(192, 0, 2, 2)),
        TlvItem::named("time_values"),
        TlvItem::named("style"),
        TlvItem::named("flowspec"),
        TlvItem::named("filter_spec_lsp_tunnel_ipv4"),
        TlvItem::named("label").with("label", 299_776u32),
        TlvItem::named("record_route").with(
            "subobjects",
            vec![
                TlvItem::named("ipv4_address").with("address", Ipv4Addr::new(192, 0, 2, 2)),
                TlvItem::named("label").with("label", 299_776u32),
            ],
        ),
    ];
    let bytes = build(vec![
        Layer::new("ethernet2"),
        Layer::new("ipv4")
            .with("src", Ipv4Addr::new(192, 0, 2, 2))
            .with("dst", Ipv4Addr::new(192, 0, 2, 1)),
        Layer::new("rsvp")
            .with("msg_type", 2u8)
            .with("objects", objects),
    ]);
    let rsvp = &bytes[34..];
    assert_golden(
        rsvp,
        "1002 21a8 40 00 0080 \
         0010 0107 c0000202 0000 0001 c0000201 \
         000c 0301 c0000202 00000000 \
         0008 0501 00007530 \
         0008 0801 00 000012 \
         0024 0902 0000 0007 05 00 0006 7f 00 0005 47f42400 447a0000 7f800000 00000000 000005dc \
         000c 0a07 c0000201 0000 0001 \
         0008 1001 00049300 \
         0014 1501 01 08 c0000202 20 00 03 08 01 01 00049300",
    );
    assert_eq!(ones_complement(rsvp), 0);
    let p = roundtrip(&bytes);
    assert_eq!(items(&p, 2, "objects").len(), 8);
    assert_dissects(&bytes, None, "rsvp");
}

#[test]
fn rsvp_path_err_and_hello() {
    let err = build(vec![
        Layer::new("ethernet2"),
        Layer::new("ipv4"),
        Layer::new("rsvp").with("msg_type", 3u8).with(
            "objects",
            vec![
                TlvItem::named("session_lsp_tunnel_ipv4"),
                TlvItem::named("error_spec_ipv4")
                    .with("error_code", 24u8)
                    .with("error_value", 5u16),
                TlvItem::named("sender_template_lsp_tunnel_ipv4"),
            ],
        ),
    ]);
    assert_eq!(to_hex(&err[58..70]), "000c0601c000020200180005");
    assert_eq!(ones_complement(&err[34..]), 0);
    roundtrip(&err);
    assert_dissects(&err, None, "rsvp");

    let hello = build(vec![
        Layer::new("ethernet2"),
        Layer::new("ipv4").with("ttl", 1u8),
        Layer::new("rsvp")
            .with("msg_type", 20u8)
            .with("send_ttl", 1u8)
            .with(
                "objects",
                vec![TlvItem::named("hello_request").with("source_instance", 0x1234u32)],
            ),
    ]);
    assert_golden(
        &hello[34..],
        "1014 c696 01 00 0014 000c 1601 00001234 00000000",
    );
    roundtrip(&hello);
    assert_dissects(&hello, None, "rsvp");
}

#[test]
fn rsvp_over_ipv6() {
    let v6 = FieldValue::Ipv6([0x2001, 0xdb8, 0, 0, 0, 0, 0, 2].into());
    let bytes = build(vec![
        Layer::new("ethernet2"),
        Layer::new("ipv6"),
        Layer::new("rsvp").with(
            "objects",
            vec![
                TlvItem::named("session_lsp_tunnel_ipv6").with("tunnel_endpoint", v6.clone()),
                TlvItem::named("rsvp_hop_ipv6").with("address", v6),
                TlvItem::named("time_values"),
                TlvItem::named("label_request").with("l3pid", 0x86ddu16),
                TlvItem::named("sender_template_lsp_tunnel_ipv6"),
                TlvItem::named("sender_tspec"),
            ],
        ),
    ]);
    assert_eq!(bytes[20], 46, "next header is Auto");
    let rsvp = &bytes[54..];
    // SESSION: 4 + 16 + 4 + 16 octets.
    assert_eq!(rsvp[8..12], [0, 40, 1, 8]);
    assert_eq!(ones_complement(rsvp), 0);
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "ipv6", "rsvp"]);
    assert_dissects(&bytes, None, "rsvp");
}

#[test]
fn rsvp_unknown_objects_and_manual_lengths() {
    // An unknown object (class 0xc5, C-Type 1) keeps its value octets.
    let bytes = build(vec![
        Layer::new("ethernet2"),
        Layer::new("ipv4"),
        Layer::new("rsvp").with(
            "objects",
            vec![
                TlvItem::named("session_lsp_tunnel_ipv4"),
                TlvItem {
                    ty: Some(0xc501),
                    data: Some(vec![1, 2, 3, 4]),
                    ..TlvItem::default()
                },
            ],
        ),
    ]);
    assert_eq!(to_hex(&bytes[58..]), "0008c50101020304");
    let p = roundtrip(&bytes);
    assert_eq!(items(&p, 2, "objects")[1].ty, Some(0xc501));

    // A Manual object length is kept and warned about.
    let built = Packet::from_layers(vec![
        Layer::new("ipv4"),
        Layer::new("rsvp").with(
            "objects",
            vec![TlvItem {
                length: Some(12),
                ..TlvItem::named("time_values")
            }],
        ),
    ])
    .build(catalog())
    .unwrap();
    assert!(built
        .diagnostics
        .iter()
        .any(|d| d.key == keys::LENGTH_MISMATCH));
    assert_eq!(built.bytes[28..30], [0, 12]);
}

// ---------------------------------------------------------------------------
// BFD
// ---------------------------------------------------------------------------

#[test]
fn bfd_golden() {
    let bytes = build(vec![
        Layer::new("ethernet2"),
        Layer::new("ipv4").with("ttl", 255u8),
        udp(49152, 3784),
        Layer::new("bfd"),
    ]);
    // Version 1, state Down, detect multiplier 3, length 24, 1 s intervals.
    assert_golden(
        &bytes[42..],
        "20 40 03 18 00000001 00000000 000f4240 000f4240 00000000",
    );
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "ipv4", "udp", "bfd"]);
    assert_dissects(&bytes, None, "bfd");
}

#[test]
fn bfd_authentication_sets_the_a_bit_and_length() {
    let bytes = build(vec![
        Layer::new("ethernet2"),
        Layer::new("ipv4").with("ttl", 255u8),
        udp(49152, 3784),
        Layer::new("bfd")
            .with("state", 3u8)
            .with("your_discriminator", 2u32)
            .with("desired_min_tx_interval", 300_000u32)
            .with("required_min_rx_interval", 300_000u32)
            .with(
                "authentication",
                vec![TlvItem::named("simple_password").with("password", b"secret".to_vec())],
            ),
    ]);
    assert_golden(
        &bytes[42..],
        "20 c4 03 21 00000001 00000002 000493e0 000493e0 00000000 01 09 01 736563726574",
    );
    let p = roundtrip(&bytes);
    let auth = items(&p, 3, "authentication");
    assert_eq!(names(&auth), ["simple_password"]);
    assert_dissects(&bytes, None, "bfd");

    // Keyed SHA1 over IPv6 multihop: 28-octet section, length 52.
    let sha1 = build(vec![
        Layer::new("ethernet2"),
        Layer::new("ipv6"),
        udp(49152, 4784),
        Layer::new("bfd").with(
            "authentication",
            vec![TlvItem::named("meticulous_keyed_sha1").with("sequence_number", 9u32)],
        ),
    ]);
    let bfd = &sha1[62..];
    assert_eq!(bfd[1] & 0x04, 0x04);
    assert_eq!(bfd[3], 52);
    assert_eq!(bfd[24..32], hex("05 1c 01 00 00000009")[..]);
    roundtrip(&sha1);
    assert_dissects(&sha1, None, "bfd");
}

#[test]
fn bfd_manual_a_bit_is_warned_and_octets_after_the_length_are_payload() {
    let built = Packet::from_layers(vec![Layer::new("bfd").with("a", 0u8).with(
        "authentication",
        vec![TlvItem::named("keyed_md5").with("sequence_number", 1u32)],
    )])
    .build(catalog())
    .unwrap();
    assert!(!built.diagnostics.is_empty());
    assert_eq!(built.bytes[1] & 0x04, 0);
    assert_eq!(built.bytes[3], 48);

    // Micro-BFD (RFC 7130): UDP 6784 to 01-00-5e-90-00-01.
    let micro = vec![
        Layer::new("ethernet2").with("dst", FieldValue::Mac([0x01, 0, 0x5e, 0x90, 0, 1])),
        Layer::new("ipv4").with("ttl", 255u8),
        udp(49152, 6784),
        Layer::new("bfd"),
    ];
    let bytes = build(micro.clone());
    assert_dissects(&bytes, None, "bfd");
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "ipv4", "udp", "bfd"]);
    // Octets after the BFD length are not an authentication section.
    let mut layers = micro;
    layers.push(Layer::new("raw").with("data", vec![0xaa; 4]));
    let p = roundtrip(&build(layers));
    assert_eq!(items(&p, 3, "authentication").len(), 0);
    assert_eq!(protos(&p), ["ethernet2", "ipv4", "udp", "bfd", "raw"]);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn bfd_roundtrips(
        state in 0u8..4,
        diag in 0u8..10,
        flags in proptest::collection::vec(0u8..2, 5),
        mult in 1u8..,
        discs in (any::<u32>(), any::<u32>()),
        intervals in (any::<u32>(), any::<u32>(), any::<u32>()),
        password in proptest::option::of(proptest::collection::vec(any::<u8>(), 1..17)),
    ) {
        let mut bfd = Layer::new("bfd")
            .with("state", state)
            .with("diag", diag)
            .with("p", flags[0])
            .with("f", flags[1])
            .with("c", flags[2])
            .with("d", flags[3])
            .with("m", flags[4])
            .with("detect_mult", mult)
            .with("my_discriminator", discs.0.max(1))
            .with("your_discriminator", discs.1)
            .with("desired_min_tx_interval", intervals.0)
            .with("required_min_rx_interval", intervals.1)
            .with("required_min_echo_rx_interval", intervals.2);
        let auth = password.is_some();
        if let Some(pw) = password {
            bfd = bfd.with("authentication", vec![TlvItem::named("simple_password").with("password", pw)]);
        }
        let bytes = build(vec![Layer::new("ipv4"), udp(49152, 3784), bfd]);
        prop_assert_eq!(bytes[28 + 3] as usize, bytes.len() - 28);
        prop_assert_eq!(bytes[28 + 1] & 0x04 != 0, auth);
        let parsed = parse(&bytes, "ipv4", catalog()).unwrap();
        prop_assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        prop_assert_eq!(parsed.packet.layers[2].protocol.as_str(), "bfd");
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    }

    #[test]
    fn ldp_label_mappings_roundtrip(
        mappings in proptest::collection::vec((0u8..=32, any::<[u8; 4]>(), 16u32..0x10_0000), 1..6),
    ) {
        let messages: Vec<TlvItem> = mappings
            .iter()
            .enumerate()
            .map(|(i, (plen, addr, label))| {
                let n = usize::from(*plen).div_ceil(8);
                let element = s(&[
                    ("prefix_length", FieldValue::UInt((*plen).into())),
                    ("prefix", FieldValue::Bytes(addr[..n].to_vec())),
                ]);
                TlvItem::named("label_mapping")
                    .with("message_id", u32::try_from(i).unwrap())
                    .with("tlvs", vec![
                        TlvItem::named("fec").with("elements", FieldValue::List(vec![element])),
                        TlvItem::named("generic_label").with("label", *label),
                    ])
            })
            .collect();
        let bytes = build(vec![Layer::new("ipv4"), tcp(646, 49152), Layer::new("ldp").with("messages", messages)]);
        let ldp = &bytes[40..];
        prop_assert_eq!(usize::from(u16::from_be_bytes([ldp[2], ldp[3]])), ldp.len() - 4);
        let parsed = parse(&bytes, "ipv4", catalog()).unwrap();
        prop_assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        prop_assert_eq!(items(&parsed.packet, 2, "messages").len(), mappings.len());
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    }

    #[test]
    fn rsvp_checksum_verifies(
        refresh in any::<u32>(),
        tunnel in any::<u16>(),
        name in proptest::collection::vec(any::<u8>(), 0..20),
        ero in proptest::collection::vec(any::<[u8; 4]>(), 0..4),
    ) {
        let ero: Vec<TlvItem> = ero
            .iter()
            .map(|a| TlvItem::named("ipv4_prefix").with("address", Ipv4Addr::from(*a)))
            .collect();
        let objects = vec![
            TlvItem::named("session_lsp_tunnel_ipv4").with("tunnel_id", tunnel),
            TlvItem::named("rsvp_hop_ipv4"),
            TlvItem::named("time_values").with("refresh_period", refresh),
            TlvItem::named("explicit_route").with("subobjects", ero),
            TlvItem::named("session_attribute").with("name", name),
        ];
        let bytes = build(vec![Layer::new("ipv4"), Layer::new("rsvp").with("objects", objects)]);
        let rsvp = &bytes[20..];
        prop_assert_eq!(ones_complement(rsvp), 0);
        prop_assert_eq!(rsvp.len() % 4, 0);
        let parsed = parse(&bytes, "ipv4", catalog()).unwrap();
        prop_assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        prop_assert_eq!(items(&parsed.packet, 1, "objects").len(), 5);
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    }
}
