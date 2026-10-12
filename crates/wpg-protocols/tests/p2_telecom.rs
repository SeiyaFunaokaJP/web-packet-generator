//! `MACsec`, GTPv2-C, Diameter, SIP and RTP / RTCP (M7): golden byte vectors, Auto
//! flags / counts / lengths / padding, value-discriminated IEs and AVPs, text
//! lines, compound RTCP packets, parse round-trips and proptests.
//!
//! Expected bytes were derived field-by-field (IEEE 802.1AE, 3GPP TS 29.274,
//! RFC 6733 / RFC 4006, RFC 3261, RFC 3550) and cross-checked with tshark 4.2
//! (`SecTAG` / ICV, IEs including the grouped Bearer Context, AVPs including grouped
//! and vendor-specific ones, SIP headers and SDP body, RTP CSRCs and header
//! extension, every RTCP packet of a compound one with `length_check` OK, no
//! expert warning). An optional `tshark` cross-check runs when `WPG_TSHARK=1`.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, Ipv6Addr};

use proptest::prelude::*;
use wpg_core::diag::keys;
use wpg_core::{parse, FieldValue, Layer, Packet, TlvItem};
use wpg_protocols::catalog;

mod tshark_util;
use tshark_util::{dissect_with, find};

/// tshark arguments decoding the IANA RTP / RTCP ports.
const RTP_PORTS: &[&str] = &["-d", "udp.port==5004,rtp", "-d", "udp.port==5005,rtcp"];

/// tshark decodes the `protocol_key` layer of `frame` (with extra arguments) and
/// flags nothing as malformed. No-op without `WPG_TSHARK=1`.
fn assert_dissects_with(frame: &[u8], args: &[&str], protocol_key: &str) {
    let Some(layers) = dissect_with(frame, None, args) else {
        return;
    };
    assert!(
        find(&layers, protocol_key).is_some(),
        "tshark did not decode a `{protocol_key}` layer"
    );
    assert!(
        find(&layers, "_ws.malformed").is_none(),
        "malformed: {layers}"
    );
}

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

fn text(s: &str) -> FieldValue {
    FieldValue::Bytes(s.as_bytes().to_vec())
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

fn ipv4() -> Layer {
    Layer::new("ipv4")
        .with("src", Ipv4Addr::new(192, 0, 2, 1))
        .with("dst", Ipv4Addr::new(192, 0, 2, 2))
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

/// Offset of the UDP / TCP payload after Ethernet + IPv4 + UDP (8) / TCP (20).
const UDP_PAYLOAD: usize = 42;
const TCP_PAYLOAD: usize = 54;

// ---------------------------------------------------------------------------
// MACsec
// ---------------------------------------------------------------------------

#[test]
fn macsec_golden() {
    let bytes = build(vec![Layer::new("ethernet2"), Layer::new("macsec")]);
    assert_eq!(&bytes[12..14], [0x88, 0xe5]);
    let data: String = (0u8..48).map(|b| format!("{b:02x}")).collect();
    // TCI 0x2c (SC, E, C) / AN 0, SL 0, PN 1, SCI 02:00:00:00:00:01 port 1, the
    // secure data and the ICV.
    assert_golden(
        &bytes[14..],
        &format!(
            "2c 00 00000001 020000000001 0001 {data} {}",
            "00".repeat(16)
        ),
    );
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "macsec"]);
    assert_eq!(p.layers[1].fields["icv"], FieldValue::Bytes(vec![0; 16]));
    assert_dissects_with(&bytes, &[], "macsec");
}

#[test]
fn macsec_without_sci_clears_the_sc_bit() {
    let bytes = build(vec![
        Layer::new("ethernet2"),
        Layer::new("dot1q").with("vid", 10u16),
        Layer::new("macsec")
            .with("es", 1u8)
            .with("an", 3u8)
            .with("sl", 20u8)
            .with("pn", 0xffff_fffeu32)
            .with("sci", FieldValue::List(Vec::new()))
            .with("secure_data", vec![0xeeu8; 20])
            .with("icv", vec![0x11u8; 16]),
    ]);
    // 802.1Q carries MACsec; TCI 0x4f (ES, E, C, AN 3), an 8-octet SecTAG with
    // the EtherType.
    assert_eq!(&bytes[16..18], [0x88, 0xe5]);
    assert_golden(&bytes[18..24], "4f 14 fffffffe");
    assert_eq!(bytes.len(), 24 + 20 + 16);
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "dot1q", "macsec"]);
    assert_eq!(p.layers[2].fields["sl"], FieldValue::UInt(20));
    assert_dissects_with(&bytes, &[], "macsec");
}

#[test]
fn macsec_manual_sc_bit_is_warned_and_short_frames_are_rejected() {
    let built = Packet::from_layers(vec![
        Layer::new("ethernet2"),
        Layer::new("macsec").with("sc", 0u8),
    ])
    .build(catalog())
    .unwrap();
    assert!(built
        .diagnostics
        .iter()
        .any(|d| d.key == keys::COUNT_MISMATCH));
    // Fewer octets than the ICV after the SecTAG: the layer does not decode.
    let mut frame = build(vec![Layer::new("ethernet2"), Layer::new("macsec")]);
    frame.truncate(14 + 16 + 8);
    let parsed = parse(&frame, "ethernet2", catalog()).unwrap();
    assert!(parsed
        .diagnostics
        .iter()
        .any(|d| d.key == keys::SHORT_HEADER));
    assert_eq!(protos(&parsed.packet), ["ethernet2", "raw"]);
}

// ---------------------------------------------------------------------------
// GTPv2-C
// ---------------------------------------------------------------------------

#[test]
fn gtpv2_create_session_request_golden() {
    let bytes = build(vec![
        Layer::new("ethernet2"),
        ipv4(),
        udp(2123, 2123),
        Layer::new("gtpv2"),
    ]);
    assert_golden(
        &bytes[UDP_PAYLOAD..],
        "48 20 005b 00000000 000001 00 \
         01 0008 00 0001012143658 7f9 \
         52 0001 00 06 \
         57 0009 00 8a 00000001 c0000201 \
         47 0009 00 08696e7465726e6574 \
         63 0001 00 01 \
         5d 001f 00 \
           49 0001 00 05 \
           50 0016 00 3c 09 0000000000 0000000000 0000000000 0000000000",
    );
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "ipv4", "udp", "gtpv2"]);
    let ies = items(&p, 3, "ies");
    assert_eq!(
        names(&ies),
        [
            "imsi",
            "rat_type",
            "fteid",
            "apn",
            "pdn_type",
            "bearer_context"
        ]
    );
    let FieldValue::Tlv(inner) = &ies[5].fields["ies"] else {
        panic!("bearer context not decoded");
    };
    assert_eq!(names(inner), ["ebi", "bearer_qos"]);
    assert_dissects_with(&bytes, &[], "gtpv2");
}

#[test]
fn gtpv2_echo_request_has_no_teid() {
    let bytes = build(vec![
        Layer::new("ethernet2"),
        ipv4(),
        udp(2123, 2123),
        Layer::new("gtpv2")
            .with("message_type", 1u8)
            .with("teid", FieldValue::List(Vec::new()))
            .with("sequence_number", 0x00_abcdu32)
            .with(
                "ies",
                vec![TlvItem::named("recovery").with("restart_counter", 7u8)],
            ),
    ]);
    // T = 0: version 2 only, length 9 (sequence, spare, Recovery).
    assert_golden(&bytes[UDP_PAYLOAD..], "40 01 0009 00abcd 00 03 0001 00 07");
    let p = roundtrip(&bytes);
    assert_eq!(p.layers[3].fields["teid"], FieldValue::List(Vec::new()));
    assert_dissects_with(&bytes, &[], "gtpv2");
}

#[test]
fn gtpv2_discriminated_ies_cause_and_unknown_ies() {
    let ies = vec![
        TlvItem::named("cause").with("cause", 16u8),
        TlvItem::named("fteid_ipv6")
            .with("interface_type", 11u8)
            .with("teid", 0x0a0b_0c0du32),
        TlvItem::named("fteid_ipv4v6").with("interface_type", 7u8),
        TlvItem::named("ip_address").with("address", Ipv4Addr::new(198, 51, 100, 1)),
        TlvItem::named("ip_address_ipv6")
            .with("address", "2001:db8::2".parse::<Ipv6Addr>().unwrap()),
        TlvItem::named("paa_ipv4v6").with("ipv4", Ipv4Addr::new(10, 0, 0, 1)),
        TlvItem::named("ebi").with("instance", 1u8).with("ebi", 6u8),
        TlvItem {
            ty: Some(200),
            data: Some(vec![0x00, 0xde, 0xad]),
            ..TlvItem::default()
        },
    ];
    let bytes = build(vec![
        Layer::new("ethernet2"),
        ipv4(),
        udp(2123, 2123),
        Layer::new("gtpv2")
            .with("message_type", 33u8)
            .with("teid", FieldValue::List(vec![FieldValue::UInt(0x1001)]))
            .with("ies", ies),
    ]);
    let b = &bytes[UDP_PAYLOAD + 12..];
    // Cause: request accepted, no flags (length 2).
    assert_golden(&b[..6], "02 0002 00 10 00");
    // F-TEID with only an IPv6 address: V6, interface 11, length 21.
    assert_golden(&b[6..10], "57 0015 00");
    assert_eq!(b[10], 0x4b);
    let p = roundtrip(&bytes);
    assert_eq!(
        names(&items(&p, 3, "ies")),
        [
            "cause",
            "fteid_ipv6",
            "fteid_ipv4v6",
            "ip_address",
            "ip_address_ipv6",
            "paa_ipv4v6",
            "ebi",
            "?"
        ]
    );
    assert_eq!(
        items(&p, 3, "ies")[6].fields["instance"],
        FieldValue::UInt(1)
    );
    assert_dissects_with(&bytes, &[], "gtpv2");
}

#[test]
fn gtpv2_piggybacked_message_is_the_next_layer() {
    let bytes = build(vec![
        Layer::new("ethernet2"),
        ipv4(),
        udp(2123, 2123),
        Layer::new("gtpv2")
            .with("p", 1u8)
            .with("message_type", 33u8)
            .with("ies", vec![TlvItem::named("cause")]),
        Layer::new("gtpv2").with("message_type", 95u8).with(
            "ies",
            vec![TlvItem::named("ebi"), TlvItem::named("bearer_context")],
        ),
    ]);
    // The first message ends at its length (12 + 6).
    assert_golden(&bytes[UDP_PAYLOAD..UDP_PAYLOAD + 4], "58 21 000e");
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "ipv4", "udp", "gtpv2", "gtpv2"]);
    assert_eq!(names(&items(&p, 4, "ies")), ["ebi", "bearer_context"]);
    assert_dissects_with(&bytes, &[], "gtpv2");
}

// ---------------------------------------------------------------------------
// Diameter
// ---------------------------------------------------------------------------

#[test]
fn diameter_cer_golden() {
    let bytes = build(vec![
        Layer::new("ethernet2"),
        ipv4(),
        tcp(40000, 3868),
        Layer::new("diameter"),
    ]);
    assert_golden(
        &bytes[TCP_PAYLOAD..],
        "01 00006c 80 000101 00000000 00000001 00000001 \
         00000108 40 00001a 636c69656e742e6578616d706c652e636f6d 0000 \
         00000128 40 000013 6578616d706c652e636f6d 00 \
         00000101 40 00000e 0001 c0000201 0000 \
         0000010a 40 00000c 00000000 \
         0000010d 00 00000b 777067 00",
    );
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "ipv4", "tcp", "diameter"]);
    assert_eq!(
        names(&items(&p, 3, "avps")),
        [
            "origin_host",
            "origin_realm",
            "host_ip_address",
            "vendor_id",
            "product_name"
        ]
    );
    assert_dissects_with(&bytes, &[], "diameter");
}

#[test]
fn diameter_grouped_and_vendor_specific_avps() {
    let avps = vec![
        TlvItem::named("session_id").with("value", b"s;1".to_vec()),
        TlvItem::named("result_code").with("value", 2001u32),
        TlvItem::named("experimental_result"),
        TlvItem::named("vendor_specific_application_id"),
        // A vendor AVP with a known code: the V flag follows the Vendor-ID.
        TlvItem::named("user_name")
            .with("vendor_id", FieldValue::List(vec![FieldValue::UInt(10415)]))
            .with("value", b"u".to_vec()),
        TlvItem::named("host_ip_address_ipv6"),
    ];
    let bytes = build(vec![
        Layer::new("ethernet2"),
        ipv4(),
        tcp(3868, 40000),
        Layer::new("diameter")
            .with("r", 0u8)
            .with("command_code", 316u32)
            .with("application_id", 16_777_251u32)
            .with("avps", avps),
    ]);
    let b = &bytes[TCP_PAYLOAD + 20..];
    // Session-Id: 8 + 3 octets, one octet of padding.
    assert_golden(&b[..12], "00000107 40 00000b 733b31 00");
    // Result-Code 2001.
    assert_golden(&b[12..24], "0000010c 40 00000c 000007d1");
    // Experimental-Result: Vendor-Id 10415 and Experimental-Result-Code 5001.
    assert_golden(
        &b[24..56],
        "00000129 40 000020 0000010a 40 00000c 000028af 0000012a 40 00000c 00001389",
    );
    let user = &b[56 + 32..];
    // User-Name with V and M set and the Vendor-ID, 13 octets + 3 of padding.
    assert_golden(&user[..16], "00000001 c0 00000d 000028af 75 000000");
    let p = roundtrip(&bytes);
    let avps = items(&p, 3, "avps");
    assert_eq!(
        names(&avps),
        [
            "session_id",
            "result_code",
            "experimental_result",
            "vendor_specific_application_id",
            "user_name",
            "host_ip_address_ipv6"
        ]
    );
    let FieldValue::Tlv(inner) = &avps[3].fields["avps"] else {
        panic!("grouped AVP not decoded");
    };
    assert_eq!(names(inner), ["vendor_id", "auth_application_id"]);
    assert_dissects_with(&bytes, &[], "diameter");
}

#[test]
fn diameter_messages_end_at_their_length() {
    let dwr = || {
        Layer::new("diameter").with("command_code", 280u32).with(
            "avps",
            vec![
                TlvItem::named("origin_host").with("value", b"a.example".to_vec()),
                TlvItem::named("origin_realm").with("value", b"example".to_vec()),
            ],
        )
    };
    let bytes = build(vec![
        Layer::new("ethernet2"),
        ipv4(),
        tcp(3868, 3868),
        dwr(),
        dwr(),
    ]);
    // Two Device-Watchdog-Requests in one segment: the first one ends at its
    // length, the rest is the next layer.
    let p = parse(&bytes, "ethernet2", catalog()).unwrap();
    assert_eq!(
        protos(&p.packet),
        ["ethernet2", "ipv4", "tcp", "diameter", "raw"]
    );
    assert_eq!(
        names(&items(&p.packet, 3, "avps")),
        ["origin_host", "origin_realm"]
    );
    assert_eq!(p.packet.build(catalog()).unwrap().bytes, bytes);
}

// ---------------------------------------------------------------------------
// SIP
// ---------------------------------------------------------------------------

#[test]
fn sip_options_golden() {
    let bytes = build(vec![
        Layer::new("ethernet2"),
        ipv4(),
        udp(5060, 5060),
        Layer::new("sip"),
    ]);
    let msg = String::from_utf8(bytes[UDP_PAYLOAD..].to_vec()).unwrap();
    assert!(msg.starts_with("OPTIONS sip:bob@example.com SIP/2.0\r\nVia: SIP/2.0/UDP"));
    assert!(msg.ends_with("CSeq: 63104 OPTIONS\r\nContact: <sip:alice@192.0.2.1>\r\nAccept: application/sdp\r\nContent-Length: 0\r\n\r\n"));
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "ipv4", "udp", "sip"]);
    assert_eq!(
        p.layers[3].fields["start_line"],
        text("OPTIONS sip:bob@example.com SIP/2.0")
    );
    let FieldValue::List(lines) = &p.layers[3].fields["headers"] else {
        panic!("headers not decoded");
    };
    assert_eq!(lines.len(), 9);
    assert_eq!(lines[8], text("Content-Length: 0"));
    assert_dissects_with(&bytes, &[], "sip");
}

#[test]
fn sip_response_with_body_over_tcp() {
    let body = "v=0\r\no=bob 1 1 IN IP4 192.0.2.2\r\ns=-\r\nc=IN IP4 192.0.2.2\r\nt=0 0\r\nm=audio 3456 RTP/AVP 0\r\n";
    let headers = FieldValue::List(vec![
        text("Via: SIP/2.0/TCP 192.0.2.1:5060;branch=z9hG4bK74bf9"),
        text("To: <sip:bob@example.com>;tag=8321234356"),
        text("From: <sip:alice@example.com>;tag=9fxced76sl"),
        text("Call-ID: 3848276298220188511@192.0.2.1"),
        text("CSeq: 1 INVITE"),
        text("Content-Type: application/sdp"),
        FieldValue::Bytes(format!("Content-Length: {}", body.len()).into_bytes()),
    ]);
    let bytes = build(vec![
        Layer::new("ethernet2"),
        ipv4(),
        tcp(5060, 49152),
        Layer::new("sip")
            .with("start_line", text("SIP/2.0 200 OK"))
            .with("headers", headers.clone())
            .with("body", text(body)),
    ]);
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "ipv4", "tcp", "sip"]);
    assert_eq!(p.layers[3].fields["headers"], headers);
    assert_eq!(p.layers[3].fields["body"], text(body));
    assert_dissects_with(&bytes, &[], "sdp");
}

#[test]
fn sip_without_the_empty_line_is_not_decoded() {
    let mut bytes = build(vec![
        Layer::new("ethernet2"),
        ipv4(),
        udp(5060, 5060),
        Layer::new("sip"),
    ]);
    bytes.truncate(bytes.len() - 2);
    let parsed = parse(&bytes, "ethernet2", catalog()).unwrap();
    assert!(parsed
        .diagnostics
        .iter()
        .any(|d| d.key == keys::SHORT_HEADER || d.key == keys::LENGTH_EXCEEDS_DATA));
    assert_ne!(protos(&parsed.packet).last(), Some(&"sip"));
}

// ---------------------------------------------------------------------------
// RTP / RTCP
// ---------------------------------------------------------------------------

#[test]
fn rtp_golden() {
    let bytes = build(vec![
        Layer::new("ethernet2"),
        ipv4(),
        udp(5004, 5004),
        Layer::new("rtp"),
        Layer::new("raw").with("data", vec![0xd5u8; 8]),
    ]);
    assert_golden(
        &bytes[UDP_PAYLOAD..],
        "80 00 0001 00000000 12345678 d5d5d5d5d5d5d5d5",
    );
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "ipv4", "udp", "rtp", "raw"]);
    assert_dissects_with(&bytes, RTP_PORTS, "rtp");
}

#[test]
fn rtp_csrcs_and_header_extension_are_counted() {
    let ext = s(&[(
        "data",
        FieldValue::List(vec![
            FieldValue::UInt(0x10ab_0000),
            FieldValue::UInt(0x2122_2300),
        ]),
    )]);
    let bytes = build(vec![
        Layer::new("ethernet2"),
        ipv4(),
        // Any port: the RTP layer is selected explicitly.
        udp(40000, 40002),
        Layer::new("rtp")
            .with("marker", 1u8)
            .with("payload_type", 96u8)
            .with(
                "csrc",
                FieldValue::List(vec![
                    FieldValue::UInt(1),
                    FieldValue::UInt(2),
                    FieldValue::UInt(3),
                ]),
            )
            .with("header_extension", FieldValue::List(vec![ext])),
    ]);
    // X = 1, CC = 3, M = 1 / PT 96; profile 0xbede, 2 words.
    assert_golden(
        &bytes[UDP_PAYLOAD..],
        "93 e0 0001 00000000 12345678 00000001 00000002 00000003 bede 0002 10ab0000 21222300",
    );
    // Not a well-known port: the payload stays raw when parsing.
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "ipv4", "udp", "raw"]);
    let parsed = parse(&bytes[UDP_PAYLOAD..], "rtp", catalog()).unwrap();
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert_eq!(parsed.packet.layers[0].fields["cc"], FieldValue::UInt(3));
    assert_dissects_with(&bytes, &["-d", "udp.port==40002,rtp"], "rtp");
}

#[test]
fn rtcp_compound_golden() {
    let block = s(&[
        ("ssrc", FieldValue::UInt(0x8765_4321)),
        ("jitter", FieldValue::UInt(5)),
    ]);
    let bytes = build(vec![
        Layer::new("ethernet2"),
        ipv4(),
        udp(5005, 5005),
        Layer::new("rtcp_sr").with("report_blocks", FieldValue::List(vec![block])),
        Layer::new("rtcp_sdes"),
        Layer::new("rtcp_bye").with(
            "reason",
            FieldValue::List(vec![s(&[("text", text("bye"))])]),
        ),
    ]);
    assert_golden(
        &bytes[UDP_PAYLOAD..],
        "81 c8 000c 12345678 eb000000 00000000 00000000 00000000 00000000 \
           87654321 00 000000 00000000 00000005 00000000 00000000 \
         81 ca 0006 12345678 01 0e 7573657240 3139322e302e322e31 00 000000 \
         81 cb 0002 12345678 03 627965",
    );
    let p = roundtrip(&bytes);
    assert_eq!(
        protos(&p),
        [
            "ethernet2",
            "ipv4",
            "udp",
            "rtcp_sr",
            "rtcp_sdes",
            "rtcp_bye"
        ]
    );
    assert_dissects_with(&bytes, RTP_PORTS, "rtcp");
}

#[test]
fn rtcp_generic_packets_rr_and_app() {
    let bytes = build(vec![
        Layer::new("ethernet2"),
        ipv4(),
        udp(5005, 5005),
        Layer::new("rtcp_rr"),
        // A transport feedback packet (not typed): the generic layer.
        Layer::new("rtcp").with("count", 1u8),
        Layer::new("rtcp_app")
            .with("subtype", 3u8)
            .with("data", vec![1u8, 2, 3, 4]),
    ]);
    assert_golden(
        &bytes[UDP_PAYLOAD..],
        "80 c9 0001 12345678 \
         81 cd 0002 12345678 00000000 \
         83 cc 0003 12345678 57504730 01020304",
    );
    let p = roundtrip(&bytes);
    assert_eq!(
        protos(&p),
        ["ethernet2", "ipv4", "udp", "rtcp_rr", "rtcp", "rtcp_app"]
    );
    assert_eq!(
        p.layers[4].fields["body"],
        FieldValue::Bytes(hex("12345678 00000000"))
    );
    assert_dissects_with(&bytes, RTP_PORTS, "rtcp");
}

#[test]
fn rtcp_manual_counts_and_lengths_are_warned() {
    let built = Packet::from_layers(vec![
        Layer::new("rtcp_sdes").with("source_count", 2u8),
        Layer::new("rtcp_bye").with("length", 5u16),
    ])
    .unpadded()
    .build(catalog())
    .unwrap();
    let keys: Vec<_> = built.diagnostics.iter().map(|d| d.key.as_str()).collect();
    assert!(keys.contains(&keys::COUNT_MISMATCH), "{keys:?}");
    assert!(keys.contains(&keys::LENGTH_MISMATCH), "{keys:?}");
}

// ---------------------------------------------------------------------------
// Proptests
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn gtpv2_roundtrips(
        teid in proptest::option::of(any::<u32>()),
        seq in 0u32..0x100_0000,
        digits in proptest::collection::vec(any::<u8>(), 1..9),
        ebi in 0u8..16,
        apn in proptest::collection::vec(any::<u8>(), 0..40),
    ) {
        let ies = vec![
            TlvItem::named("imsi").with("digits", digits),
            TlvItem::named("apn").with("apn", apn),
            TlvItem::named("bearer_context")
                .with("ies", vec![TlvItem::named("ebi").with("ebi", ebi), TlvItem::named("fteid_ipv6")]),
        ];
        let teid = FieldValue::List(teid.map(FieldValue::from).into_iter().collect());
        let bytes = build(vec![
            Layer::new("ethernet2"),
            ipv4(),
            udp(2123, 2123),
            Layer::new("gtpv2").with("teid", teid).with("sequence_number", seq).with("ies", ies),
        ]);
        let p = roundtrip(&bytes);
        prop_assert_eq!(protos(&p), ["ethernet2", "ipv4", "udp", "gtpv2"]);
        let ies = items(&p, 3, "ies");
        prop_assert_eq!(names(&ies), ["imsi", "apn", "bearer_context"]);
    }

    #[test]
    fn diameter_roundtrips(
        host in proptest::collection::vec(any::<u8>(), 0..30),
        vendor in proptest::option::of(any::<u32>()),
        code in any::<u32>(),
        flags in 0u8..8,
    ) {
        let vendor = FieldValue::List(vendor.map(FieldValue::from).into_iter().collect());
        let avps = vec![
            TlvItem::named("origin_host").with("value", host).with("vendor_id", vendor),
            TlvItem::named("proxy_info"),
            TlvItem::named("result_code").with("value", code),
        ];
        let bytes = build(vec![
            Layer::new("ethernet2"),
            ipv4(),
            tcp(3868, 3868),
            Layer::new("diameter")
                .with("r", flags & 1)
                .with("p", (flags >> 1) & 1)
                .with("e", flags >> 2)
                .with("avps", avps),
        ]);
        prop_assert_eq!((bytes.len() - TCP_PAYLOAD) % 4, 0);
        let p = roundtrip(&bytes);
        let avps = items(&p, 3, "avps");
        prop_assert_eq!(names(&avps), ["origin_host", "proxy_info", "result_code"]);
    }

    #[test]
    fn sip_roundtrips(
        lines in proptest::collection::vec("[ -~]{1,40}", 0..8),
        body in proptest::collection::vec(any::<u8>(), 0..64),
    ) {
        let headers = FieldValue::List(lines.iter().map(|l| text(l)).collect());
        let bytes = build(vec![
            Layer::new("ethernet2"),
            ipv4(),
            udp(5060, 5060),
            Layer::new("sip")
                .with("start_line", text("MESSAGE sip:x SIP/2.0"))
                .with("headers", headers.clone())
                .with("body", body.clone()),
        ]);
        let p = roundtrip(&bytes);
        prop_assert_eq!(&p.layers[3].fields["headers"], &headers);
        prop_assert_eq!(&p.layers[3].fields["body"], &FieldValue::Bytes(body));
    }

    #[test]
    fn rtp_and_rtcp_roundtrip(
        csrc in proptest::collection::vec(any::<u32>(), 0..16),
        ext in proptest::option::of(proptest::collection::vec(any::<u32>(), 0..4)),
        blocks in 0usize..4,
        cname in proptest::collection::vec(1u8..=255, 0..30),
        payload in proptest::collection::vec(any::<u8>(), 0..32),
    ) {
        let ext = FieldValue::List(
            ext.map(|w| s(&[("data", FieldValue::List(w.into_iter().map(FieldValue::from).collect()))]))
                .into_iter()
                .collect(),
        );
        let mut rtp = vec![
            Layer::new("ethernet2"),
            ipv4(),
            udp(5004, 5004),
            Layer::new("rtp")
                .with("csrc", FieldValue::List(csrc.into_iter().map(FieldValue::from).collect()))
                .with("header_extension", ext),
        ];
        if !payload.is_empty() {
            rtp.push(Layer::new("raw").with("data", payload));
        }
        roundtrip(&build(rtp));

        let block = s(&[("jitter", FieldValue::UInt(1))]);
        let item = s(&[("type", FieldValue::UInt(1)), ("text", FieldValue::Bytes(cname))]);
        let chunk = s(&[("items", FieldValue::List(vec![item]))]);
        let bytes = build(vec![
            Layer::new("ethernet2"),
            ipv4(),
            udp(5005, 5005),
            Layer::new("rtcp_rr").with("report_blocks", FieldValue::List(vec![block; blocks])),
            Layer::new("rtcp_sdes").with("chunks", FieldValue::List(vec![chunk.clone(), chunk])),
        ]);
        let p = roundtrip(&bytes);
        prop_assert_eq!(protos(&p), ["ethernet2", "ipv4", "udp", "rtcp_rr", "rtcp_sdes"]);
        prop_assert_eq!(&p.layers[4].fields["source_count"], &FieldValue::UInt(2));
    }
}
