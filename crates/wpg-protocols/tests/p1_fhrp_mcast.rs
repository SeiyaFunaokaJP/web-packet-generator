//! Golden byte vectors, checksum verification, parse round-trips and proptests for
//! the P1 redundancy (`VRRPv2` / v3, `HSRPv1` / v2, GLBP) and multicast (`IGMPv1`/2/3,
//! `MLDv1/v2`, PIM Hello / Join-Prune / Register) protocols, plus the MPLS-in-UDP
//! and IP-in-IP encapsulation bindings.
//!
//! Expected bytes are re-derived here field-by-field and every Internet checksum
//! (with the IPv4 / IPv6 pseudo header where the protocol uses one) is re-verified
//! with a naive RFC 1071 sum; an optional `tshark` cross-check runs when
//! `WPG_TSHARK=1`.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, Ipv6Addr};

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

/// Naive RFC 1071: a region verifies when the one's-complement sum is 0xffff.
fn verifies(data: &[u8]) -> bool {
    let mut sum: u32 = 0;
    for c in data.chunks(2) {
        let lo = u32::from(*c.get(1).unwrap_or(&0));
        sum += (u32::from(c[0]) << 8) | lo;
        while sum > 0xffff {
            sum = (sum & 0xffff) + (sum >> 16);
        }
    }
    sum == 0xffff
}

/// IPv4 pseudo header (src, dst, 0, protocol, length) followed by `upper`.
fn with_v4_pseudo(ip: &[u8], proto: u8, upper: &[u8]) -> Vec<u8> {
    let mut v = ip[12..20].to_vec();
    v.extend_from_slice(&[0, proto]);
    v.extend_from_slice(&u16::try_from(upper.len()).unwrap().to_be_bytes());
    v.extend_from_slice(upper);
    v
}

/// IPv6 pseudo header (src, dst, length, 0, next header) followed by `upper`.
fn with_v6_pseudo(ip: &[u8], next: u8, upper: &[u8]) -> Vec<u8> {
    let mut v = ip[8..40].to_vec();
    v.extend_from_slice(&u32::try_from(upper.len()).unwrap().to_be_bytes());
    v.extend_from_slice(&[0, 0, 0, next]);
    v.extend_from_slice(upper);
    v
}

fn ipv4(src: [u8; 4], dst: [u8; 4], id: u16, ttl: u8) -> Layer {
    Layer::new("ipv4")
        .with("src", Ipv4Addr::from(src))
        .with("dst", Ipv4Addr::from(dst))
        .with("identification", id)
        .with("ttl", ttl)
}

fn build(layers: Vec<Layer>) -> Vec<u8> {
    let built = Packet::from_layers(layers)
        .unpadded()
        .build(catalog())
        .unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    built.bytes
}

fn roundtrip(bytes: &[u8], first: &str) -> Vec<String> {
    let parsed = parse(bytes, first, catalog()).unwrap();
    assert!(
        !parsed.has_errors(),
        "parse errors: {:?}",
        parsed.diagnostics
    );
    assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    parsed
        .packet
        .layers
        .iter()
        .map(|l| l.protocol.clone())
        .collect()
}

fn addrs(list: &[Ipv4Addr]) -> FieldValue {
    FieldValue::List(list.iter().map(|a| FieldValue::Ipv4(*a)).collect())
}

// --------------------------------------------------------------------------
// VRRP
// --------------------------------------------------------------------------

const VRRPV2: &str = "4500002c 0001 0000 ff 70 194d c0000201 e0000012 \
    21 0a 64 02 00 01 f4f5 \
    c00002fe c00002fd \
    0000000000000000";

#[test]
fn vrrpv2_golden() {
    let bytes = build(vec![
        ipv4([192, 0, 2, 1], [224, 0, 0, 18], 1, 255),
        Layer::new("vrrpv2").with("vrid", 10u8).with(
            "ip_addresses",
            addrs(&[Ipv4Addr::new(192, 0, 2, 254), Ipv4Addr::new(192, 0, 2, 253)]),
        ),
    ]);
    assert_eq!(bytes, hex(VRRPV2));
    assert_eq!(bytes[9], 112, "IP protocol 112 (VRRP)");
    // version 2 / type 1, VRID 10, priority 100, count 2 (Auto), no auth, 1 s.
    assert_eq!(&bytes[20..26], &[0x21, 10, 100, 2, 0, 1]);
    assert!(verifies(&bytes[0..20]), "IPv4");
    assert!(verifies(&bytes[20..]), "VRRPv2 checksum (no pseudo header)");
    // VRRPv2 shares protocol 112 with VRRPv3 and decodes as `vrrpv3`; the
    // authentication data is left over as `raw`. The bytes still round-trip.
    assert_eq!(roundtrip(&bytes, "ipv4"), ["ipv4", "vrrpv3", "raw"]);
    assert_eq!(roundtrip(&bytes[20..], "vrrpv2"), ["vrrpv2"]);

    assert_dissects(&bytes, Some(0x0800), "vrrp");
}

const VRRPV3: &str = "45000020 0002 0000 ff 70 1958 c0000201 e0000012 \
    31 0a 64 01 0064 0501 \
    c00002fe";

#[test]
fn vrrpv3_ipv4_golden() {
    let bytes = build(vec![
        ipv4([192, 0, 2, 1], [224, 0, 0, 18], 2, 255),
        Layer::new("vrrpv3").with("vrid", 10u8),
    ]);
    assert_eq!(bytes, hex(VRRPV3));
    // version 3 / type 1, count 1 (Auto), max advertisement interval 100 cs.
    assert_eq!(&bytes[20..26], &[0x31, 10, 100, 1, 0x00, 0x64]);
    assert!(verifies(&bytes[0..20]), "IPv4");
    assert!(
        verifies(&with_v4_pseudo(&bytes, 112, &bytes[20..])),
        "VRRPv3 checksum with the IPv4 pseudo header"
    );
    assert_eq!(roundtrip(&bytes, "ipv4"), ["ipv4", "vrrpv3"]);

    assert_dissects(&bytes, Some(0x0800), "vrrp");
}

const VRRPV3_IPV6: &str = "60000000 0018 70 ff \
    fe800000000000000000000000000001 ff020000000000000000000000000012 \
    31 0a 64 01 0064 6def \
    fe800000000000000000000000000001";

#[test]
fn vrrpv3_ipv6_golden() {
    let bytes = build(vec![
        Layer::new("ipv6")
            .with("hop_limit", 255u8)
            .with("src", Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1))
            .with("dst", Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 0x12)),
        Layer::new("vrrpv3_ipv6").with("vrid", 10u8),
    ]);
    assert_eq!(bytes, hex(VRRPV3_IPV6));
    assert_eq!(bytes[6], 112, "next header 112 (VRRP)");
    assert!(
        verifies(&with_v6_pseudo(&bytes, 112, &bytes[40..])),
        "VRRPv3 checksum with the IPv6 pseudo header"
    );
    assert_eq!(roundtrip(&bytes, "ipv6"), ["ipv6", "vrrpv3_ipv6"]);

    assert_dissects(&bytes, Some(0x86dd), "vrrp");
}

// --------------------------------------------------------------------------
// HSRP / GLBP
// --------------------------------------------------------------------------

const HSRP: &str = "45000030 0003 0000 01 11 17b7 c0000201 e0000002 \
    07c1 07c1 001c 29fd \
    00 00 10 03 0a 64 01 00 636973636f000000 c00002fe";

#[test]
fn hsrpv1_golden() {
    let bytes = build(vec![
        ipv4([192, 0, 2, 1], [224, 0, 0, 2], 3, 1),
        Layer::new("udp")
            .with("src_port", 1985u16)
            .with("dst_port", 1985u16),
        Layer::new("hsrp").with("group", 1u8),
    ]);
    assert_eq!(bytes, hex(HSRP));
    // version 0, Hello, Active, hello 3 s / hold 10 s, priority 100, group 1.
    assert_eq!(&bytes[28..36], &[0, 0, 16, 3, 10, 100, 1, 0]);
    assert_eq!(&bytes[36..44], b"cisco\0\0\0");
    assert!(verifies(&bytes[0..20]), "IPv4");
    assert!(
        verifies(&with_v4_pseudo(&bytes, 17, &bytes[20..])),
        "UDP checksum"
    );
    assert_eq!(roundtrip(&bytes, "ipv4"), ["ipv4", "udp", "hsrp"]);

    assert_dissects(&bytes, Some(0x0800), "hsrp");
}

const HSRPV2_IPV6: &str = "60000000 003c 11 40 \
    20010db8000000000000000000000001 20010db8000000000000000000000002 \
    07ed 07ed 003c 49f8 \
    01 28 02 00 06 06 0000 020000000001 00000064 00000bb8 00002710 \
    c00002fe000000000000000000000000 \
    03 08 636973636f000000";

#[test]
fn hsrpv2_golden() {
    // Over IPv6 (UDP 2029) HSRPv2 has a port of its own and decodes back.
    let bytes = build(vec![
        Layer::new("ipv6"),
        Layer::new("udp")
            .with("src_port", 2029u16)
            .with("dst_port", 2029u16),
        Layer::new("hsrpv2").with(
            "tlvs",
            vec![
                TlvItem::named("group_state").with("ip_version", 6u8),
                TlvItem::named("text_auth"),
            ],
        ),
    ]);
    assert_eq!(bytes, hex(HSRPV2_IPV6));
    // Group State TLV: type 1, length 40 (value only), version 2, Active (6).
    assert_eq!(&bytes[48..52], &[1, 40, 2, 0]);
    assert_eq!(bytes[52], 6);
    // Text Authentication TLV: type 3, length 8.
    assert_eq!(&bytes[90..92], &[3, 8]);
    assert!(
        verifies(&with_v6_pseudo(&bytes, 17, &bytes[40..])),
        "UDP checksum"
    );
    assert_eq!(roundtrip(&bytes, "ipv6"), ["ipv6", "udp", "hsrpv2"]);

    // Over IPv4 HSRPv2 shares UDP 1985 with HSRPv1, which wins on parse.
    let bytes = build(vec![
        ipv4([192, 0, 2, 1], [224, 0, 0, 102], 4, 1),
        Layer::new("udp")
            .with("src_port", 1985u16)
            .with("dst_port", 1985u16),
        Layer::new("hsrpv2"),
    ]);
    assert_eq!(&bytes[28..32], &[1, 40, 2, 0]);
    assert_eq!(roundtrip(&bytes, "ipv4"), ["ipv4", "udp", "hsrp", "raw"]);

    // tshark files HSRPv2 under the `hsrp` protocol; `hsrp2.*` fields mark v2.
    assert_dissects(&bytes, Some(0x0800), "hsrp2.version");
}

const GLBP: &str = "45000044 0005 0000 01 11 173d c0000201 e0000066 \
    0c96 0c96 0030 0df5 \
    01 00 0001 0000 020000000001 \
    01 1c 00 20 00 64 0000 00000bb8 00002710 0258 3840 0000 01 04 c00002fe";

#[test]
fn glbp_golden() {
    let bytes = build(vec![
        ipv4([192, 0, 2, 1], [224, 0, 0, 102], 5, 1),
        Layer::new("udp")
            .with("src_port", 3222u16)
            .with("dst_port", 3222u16),
        Layer::new("glbp"),
    ]);
    assert_eq!(bytes, hex(GLBP));
    // Hello TLV: type 1, length 28 including the 2-octet header.
    assert_eq!(&bytes[40..42], &[1, 28]);
    assert_eq!(bytes.len() - 40, 28);
    assert!(
        verifies(&with_v4_pseudo(&bytes, 17, &bytes[20..])),
        "UDP checksum"
    );
    assert_eq!(roundtrip(&bytes, "ipv4"), ["ipv4", "udp", "glbp"]);

    assert_dissects(&bytes, Some(0x0800), "glbp");
}

// --------------------------------------------------------------------------
// IGMP
// --------------------------------------------------------------------------

const IGMPV2_QUERY: &str = "4500001c 0006 0000 01 02 17d8 c0000201 e0000001 \
    11 64 ee9b 00000000";

#[test]
fn igmpv2_query_golden() {
    let bytes = build(vec![
        ipv4([192, 0, 2, 1], [224, 0, 0, 1], 6, 1),
        Layer::new("igmp"),
        Layer::new("igmpv2"),
    ]);
    assert_eq!(bytes, hex(IGMPV2_QUERY));
    assert_eq!(bytes[9], 2, "IP protocol 2 (IGMP)");
    assert_eq!(bytes[20], 0x11, "type Auto: membership query");
    assert!(verifies(&bytes[20..]), "IGMP checksum");
    assert_eq!(roundtrip(&bytes, "ipv4"), ["ipv4", "igmp", "igmpv2"]);

    assert_dissects(&bytes, Some(0x0800), "igmp");
}

const IGMPV3_QUERY: &str = "45000024 0007 0000 01 02 17cf c0000201 e0000001 \
    11 64 3a10 ef010101 02 7d 0001 c000020a";

#[test]
fn igmpv3_query_golden() {
    let bytes = build(vec![
        ipv4([192, 0, 2, 1], [224, 0, 0, 1], 7, 1),
        Layer::new("igmp"),
        Layer::new("igmpv3_query")
            .with("group", Ipv4Addr::new(239, 1, 1, 1))
            .with("sources", addrs(&[Ipv4Addr::new(192, 0, 2, 10)])),
    ]);
    assert_eq!(bytes, hex(IGMPV3_QUERY));
    // QRV 2, QQIC 125, one source (Auto).
    assert_eq!(&bytes[28..32], &[0x02, 125, 0, 1]);
    assert!(verifies(&bytes[20..]), "IGMP checksum");
    // A v3 query decodes as the v2 body plus the extension as `raw`.
    assert_eq!(roundtrip(&bytes, "ipv4"), ["ipv4", "igmp", "igmpv2", "raw"]);
    assert_eq!(
        roundtrip(&bytes[24..], "igmpv3_query"),
        ["igmpv3_query"],
        "the v3 body decodes on its own"
    );
}

const IGMPV3_REPORT: &str = "45000028 0008 0000 01 02 17b5 c0000201 e0000016 \
    22 00 2af0 0000 0001 \
    01 00 0001 ef010101 c000020a";

#[test]
fn igmpv3_report_golden() {
    let record = FieldValue::Struct(BTreeMap::from([
        ("record_type".to_owned(), FieldValue::UInt(1)),
        ("num_sources".to_owned(), FieldValue::UInt(1)),
        (
            "multicast_address".to_owned(),
            FieldValue::Ipv4(Ipv4Addr::new(239, 1, 1, 1)),
        ),
        ("sources".to_owned(), addrs(&[Ipv4Addr::new(192, 0, 2, 10)])),
    ]));
    let bytes = build(vec![
        ipv4([192, 0, 2, 1], [224, 0, 0, 22], 8, 1),
        Layer::new("igmp").with("max_resp_code", 0u8),
        Layer::new("igmpv3_report").with("group_records", FieldValue::List(vec![record])),
    ]);
    assert_eq!(bytes, hex(IGMPV3_REPORT));
    assert_eq!(bytes[20], 0x22, "type Auto: v3 membership report");
    assert!(verifies(&bytes[20..]), "IGMP checksum");
    assert_eq!(roundtrip(&bytes, "ipv4"), ["ipv4", "igmp", "igmpv3_report"]);

    assert_dissects(&bytes, Some(0x0800), "igmp");
}

// --------------------------------------------------------------------------
// MLD
// --------------------------------------------------------------------------

const MLD_QUERY: &str = "60000000 0020 00 01 \
    fe800000000000000000000000000001 ff020000000000000000000000000001 \
    3a 00 0502 0000 0100 \
    82 00 5917 2710 0000 00000000000000000000000000000000";

#[test]
fn mldv1_query_golden() {
    let bytes = build(vec![
        Layer::new("ipv6")
            .with("src", Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1))
            .with("dst", Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 1))
            .with("hop_limit", 1u8),
        // Router Alert (MLD) + PadN.
        Layer::new("ipv6_hopopts").with("options", vec![5u8, 2, 0, 0, 1, 0]),
        Layer::new("icmpv6"),
        Layer::new("mld"),
    ]);
    assert_eq!(bytes, hex(MLD_QUERY));
    assert_eq!(bytes[48], 130, "ICMPv6 type Auto: MLD query");
    assert!(
        verifies(&with_v6_pseudo(&bytes, 58, &bytes[48..])),
        "ICMPv6 checksum with the IPv6 pseudo header"
    );
    assert_eq!(
        roundtrip(&bytes, "ipv6"),
        ["ipv6", "ipv6_hopopts", "icmpv6", "mld"]
    );

    assert_dissects(&bytes, Some(0x86dd), "icmpv6");
}

const MLDV2_QUERY: &str = "60000000 002c 3a 01 \
    fe800000000000000000000000000001 ff020000000000000000000000000001 \
    82 00 29b5 2710 0000 ff050000000000000000000000000001 02 7d 0001 \
    20010db8000000000000000000000010";

#[test]
fn mldv2_query_golden() {
    let bytes = build(vec![
        Layer::new("ipv6")
            .with("src", Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1))
            .with("dst", Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 1))
            .with("hop_limit", 1u8),
        Layer::new("icmpv6"),
        Layer::new("mldv2_query")
            .with(
                "multicast_address",
                Ipv6Addr::new(0xff05, 0, 0, 0, 0, 0, 0, 1),
            )
            .with("num_sources", 1u16)
            .with(
                "sources",
                FieldValue::List(vec![FieldValue::Ipv6(Ipv6Addr::new(
                    0x2001, 0xdb8, 0, 0, 0, 0, 0, 0x10,
                ))]),
            ),
    ]);
    assert_eq!(bytes, hex(MLDV2_QUERY));
    assert!(
        verifies(&with_v6_pseudo(&bytes, 58, &bytes[40..])),
        "ICMPv6 checksum"
    );
    // A v2 query decodes as the v1 body plus the extension as `raw`.
    assert_eq!(roundtrip(&bytes, "ipv6"), ["ipv6", "icmpv6", "mld", "raw"]);
    assert_eq!(roundtrip(&bytes[44..], "mldv2_query"), ["mldv2_query"]);
}

const MLDV2_REPORT: &str = "60000000 001c 3a 01 \
    fe800000000000000000000000000001 ff020000000000000000000000000016 \
    8f 00 7006 0000 0001 \
    04 00 0000 ff050000000000000000000000000001";

#[test]
fn mldv2_report_golden() {
    let bytes = build(vec![
        Layer::new("ipv6")
            .with("src", Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1))
            .with("dst", Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 0x16))
            .with("hop_limit", 1u8),
        Layer::new("icmpv6"),
        Layer::new("mldv2_report"),
    ]);
    assert_eq!(bytes, hex(MLDV2_REPORT));
    assert_eq!(bytes[40], 143, "ICMPv6 type Auto: MLDv2 report");
    assert!(
        verifies(&with_v6_pseudo(&bytes, 58, &bytes[40..])),
        "ICMPv6 checksum"
    );
    assert_eq!(
        roundtrip(&bytes, "ipv6"),
        ["ipv6", "icmpv6", "mldv2_report"]
    );

    assert_dissects(&bytes, Some(0x86dd), "icmpv6");
}

// --------------------------------------------------------------------------
// PIM
// --------------------------------------------------------------------------

const PIM_HELLO: &str = "4500002e 0009 0000 01 67 1752 c0000201 e000000d \
    20 00 df63 \
    0001 0002 0069 \
    0013 0004 00000001 \
    0014 0004 00000000";

#[test]
fn pim_hello_golden() {
    let bytes = build(vec![
        ipv4([192, 0, 2, 1], [224, 0, 0, 13], 9, 1),
        Layer::new("pim"),
        Layer::new("pim_hello"),
    ]);
    assert_eq!(bytes, hex(PIM_HELLO));
    assert_eq!(bytes[9], 103, "IP protocol 103 (PIM)");
    assert_eq!(bytes[20], 0x20, "version 2, type Auto 0 (Hello)");
    assert!(verifies(&bytes[20..]), "PIM checksum");
    assert_eq!(roundtrip(&bytes, "ipv4"), ["ipv4", "pim", "pim_hello"]);

    assert_dissects(&bytes, Some(0x0800), "pim");
}

const PIM_JOIN_PRUNE: &str = "45000036 000a 0000 01 67 1749 c0000201 e000000d \
    23 00 60dc \
    01 00 c0000201 00 01 00d2 \
    01 00 00 20 ef010101 0001 0000 \
    01 00 04 20 c000020a";

#[test]
fn pim_join_prune_golden() {
    let bytes = build(vec![
        ipv4([192, 0, 2, 1], [224, 0, 0, 13], 10, 1),
        Layer::new("pim"),
        Layer::new("pim_join_prune"),
    ]);
    assert_eq!(bytes, hex(PIM_JOIN_PRUNE));
    assert_eq!(bytes[20], 0x23, "version 2, type Auto 3 (Join/Prune)");
    assert!(verifies(&bytes[20..]), "PIM checksum");
    assert_eq!(roundtrip(&bytes, "ipv4"), ["ipv4", "pim", "pim_join_prune"]);

    assert_dissects(&bytes, Some(0x0800), "pim");
}

const PIM_REGISTER: &str = "4500003a 000b 0000 01 67 cd1c c0000201 c6336401 \
    21 00 deff 00000000 \
    4500001e 000c 0000 01 11 07b7 c000020a ef010101 \
    1388 1389 000a b958 6d63";

#[test]
fn pim_register_golden() {
    let bytes = build(vec![
        ipv4([192, 0, 2, 1], [198, 51, 100, 1], 11, 1),
        Layer::new("pim_register"),
        ipv4([192, 0, 2, 10], [239, 1, 1, 1], 12, 1),
        Layer::new("udp")
            .with("src_port", 5000u16)
            .with("dst_port", 5001u16),
        Layer::new("raw").with("data", b"mc".to_vec()),
    ]);
    assert_eq!(bytes, hex(PIM_REGISTER));
    assert_eq!(bytes[9], 103, "IP protocol 103 (PIM)");
    // The Register checksum covers only the first 8 octets.
    assert!(verifies(&bytes[20..28]), "PIM Register checksum");
    assert!(!verifies(&bytes[20..]), "not the whole message");
    assert!(verifies(&bytes[28..48]), "inner IPv4");
    // Parsed from IPv4 it decodes as the common PIM header; as a standalone
    // Register header the encapsulated packet follows.
    let parsed = parse(&bytes, "ipv4", catalog()).unwrap();
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    assert_eq!(parsed.packet.layers[1].protocol, "pim");
    assert_eq!(
        roundtrip(&bytes[20..], "pim_register"),
        ["pim_register", "ipv4", "udp", "raw"]
    );

    assert_dissects(&bytes, Some(0x0800), "pim");
}

// --------------------------------------------------------------------------
// Encapsulation: MPLS-in-UDP (6635) and IP-in-IP / IPv6-in-IPv6
// --------------------------------------------------------------------------

const MPLS_IN_UDP: &str = "45000036 000d 0000 01 11 a5a8 0a000001 0a000002 \
    c000 19eb 0022 6300 \
    00064140 \
    45000016 000e 0000 01 fd 34da c0000201 c0000202 \
    6d75";

#[test]
fn mpls_in_udp_golden() {
    let bytes = build(vec![
        ipv4([10, 0, 0, 1], [10, 0, 0, 2], 13, 1),
        Layer::new("udp")
            .with("src_port", 49152u16)
            .with("dst_port", 6635u16),
        Layer::new("mpls").with("label", 100u32),
        ipv4([192, 0, 2, 1], [192, 0, 2, 2], 14, 1),
        Layer::new("raw").with("data", b"mu".to_vec()),
    ]);
    assert_eq!(bytes, hex(MPLS_IN_UDP));
    // Label 100, TC 0, bottom of stack, TTL 64.
    assert_eq!(&bytes[28..32], &[0x00, 0x06, 0x41, 0x40]);
    assert!(
        verifies(&with_v4_pseudo(&bytes, 17, &bytes[20..])),
        "UDP checksum"
    );
    assert!(verifies(&bytes[32..52]), "inner IPv4");
    // UDP 6635 → MPLS; the bottom-of-stack payload has no binding (raw).
    assert_eq!(roundtrip(&bytes, "ipv4"), ["ipv4", "udp", "mpls", "raw"]);

    assert_dissects(&bytes, Some(0x0800), "mpls");
}

#[test]
fn ip_in_ipv6_encapsulation() {
    // IPv4-in-IPv6 (next header 4).
    let bytes = build(vec![
        Layer::new("ipv6"),
        ipv4([10, 0, 0, 1], [10, 0, 0, 2], 15, 64),
        Layer::new("udp")
            .with("src_port", 1000u16)
            .with("dst_port", 1001u16),
    ]);
    assert_eq!(bytes[6], 4, "next header 4 (IPv4)");
    assert!(verifies(&bytes[40..60]), "inner IPv4");
    assert_eq!(roundtrip(&bytes, "ipv6"), ["ipv6", "ipv4", "udp"]);

    // IPv6-in-IPv6 (next header 41) carrying ICMPv6 echo.
    let bytes = build(vec![
        Layer::new("ipv6"),
        Layer::new("ipv6").with("src", Ipv6Addr::new(0x2001, 0xdb8, 1, 0, 0, 0, 0, 1)),
        Layer::new("icmpv6"),
        Layer::new("icmpv6_echo"),
    ]);
    assert_eq!(bytes[6], 41, "outer next header 41 (IPv6)");
    assert_eq!(bytes[46], 58, "inner next header 58 (ICMPv6)");
    assert!(
        verifies(&with_v6_pseudo(&bytes[40..], 58, &bytes[80..])),
        "ICMPv6 checksum uses the inner IPv6 pseudo header"
    );
    assert_eq!(
        roundtrip(&bytes, "ipv6"),
        ["ipv6", "ipv6", "icmpv6", "icmpv6_echo"]
    );
}

// --------------------------------------------------------------------------
// Proptests
// --------------------------------------------------------------------------

fn v4(a: u32) -> FieldValue {
    FieldValue::Ipv4(Ipv4Addr::from(a))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// `VRRPv2` / v3 with random address lists: the Auto count matches, the checksum
    /// verifies and the VRRP layer re-decodes to the same bytes.
    #[test]
    fn vrrp_address_lists_roundtrip(
        list in proptest::collection::vec(any::<u32>(), 1..8),
        vrid in 1u8..=255,
        priority in any::<u8>(),
        v3 in any::<bool>(),
    ) {
        let id = if v3 { "vrrpv3" } else { "vrrpv2" };
        let addrs = FieldValue::List(list.iter().map(|a| v4(*a)).collect());
        let bytes = build(vec![
            ipv4([192, 0, 2, 1], [224, 0, 0, 18], 1, 255),
            Layer::new(id)
                .with("vrid", vrid)
                .with("priority", priority)
                .with("ip_addresses", addrs),
        ]);
        prop_assert_eq!(usize::from(bytes[23]), list.len());
        if v3 {
            prop_assert!(verifies(&with_v4_pseudo(&bytes, 112, &bytes[20..])));
        } else {
            prop_assert!(verifies(&bytes[20..]));
        }
        let parsed = parse(&bytes[20..], id, catalog()).unwrap();
        prop_assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, &bytes[20..]);
    }

    /// `IGMPv3` reports with random group records and sources round-trip and keep a
    /// valid checksum.
    #[test]
    fn igmpv3_reports_roundtrip(
        records in proptest::collection::vec(
            (1u8..=6, any::<u32>(), proptest::collection::vec(any::<u32>(), 0..4)),
            1..5,
        ),
    ) {
        let n = u16::try_from(records.len()).unwrap();
        let recs: Vec<FieldValue> = records
            .iter()
            .map(|(ty, group, sources)| {
                FieldValue::Struct(BTreeMap::from([
                    ("record_type".to_owned(), FieldValue::UInt(u64::from(*ty))),
                    ("num_sources".to_owned(), FieldValue::UInt(sources.len() as u64)),
                    ("multicast_address".to_owned(), v4(*group)),
                    (
                        "sources".to_owned(),
                        FieldValue::List(sources.iter().map(|s| v4(*s)).collect()),
                    ),
                ]))
            })
            .collect();
        let bytes = build(vec![
            ipv4([192, 0, 2, 1], [224, 0, 0, 22], 1, 1),
            Layer::new("igmp").with("max_resp_code", 0u8),
            Layer::new("igmpv3_report")
                .with("num_group_records", n)
                .with("group_records", FieldValue::List(recs)),
        ]);
        prop_assert!(verifies(&bytes[20..]));
        let parsed = parse(&bytes, "ipv4", catalog()).unwrap();
        prop_assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        let ids: Vec<&str> = parsed.packet.layers.iter().map(|l| l.protocol.as_str()).collect();
        prop_assert_eq!(ids, ["ipv4", "igmp", "igmpv3_report"]);
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    }

    /// PIM Hello with random option values round-trips.
    #[test]
    fn pim_hello_options_roundtrip(
        holdtime in any::<u16>(),
        dr_priority in any::<u32>(),
        generation_id in any::<u32>(),
        prune_delay in proptest::option::of((0u16..0x8000, any::<u16>())),
    ) {
        let mut opts = vec![
            TlvItem::named("holdtime").with("holdtime", holdtime),
            TlvItem::named("dr_priority").with("priority", dr_priority),
            TlvItem::named("generation_id").with("generation_id", generation_id),
        ];
        if let Some((delay, interval)) = prune_delay {
            opts.push(
                TlvItem::named("lan_prune_delay")
                    .with("propagation_delay", delay)
                    .with("override_interval", interval),
            );
        }
        let bytes = build(vec![
            ipv4([192, 0, 2, 1], [224, 0, 0, 13], 1, 1),
            Layer::new("pim"),
            Layer::new("pim_hello").with("options", opts),
        ]);
        prop_assert!(verifies(&bytes[20..]));
        let parsed = parse(&bytes, "ipv4", catalog()).unwrap();
        prop_assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    }
}
