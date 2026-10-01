//! Golden byte vectors, checksum verification, parse round-trips and a label-stack
//! proptest for the P1 encapsulation protocols (VXLAN, GRE, NVGRE, MPLS, GTP-U,
//! Geneve, IP-in-IP).
//!
//! Expected bytes are re-derived here field-by-field and every IPv4 / UDP / ICMP
//! checksum is re-verified with a naive RFC 1071 sum; an optional `tshark`
//! cross-check runs when `WPG_TSHARK=1`.

use std::net::Ipv4Addr;

use proptest::prelude::*;
use wpg_core::{parse, FieldValue, Layer, Packet};
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

fn ipv4(src: [u8; 4], dst: [u8; 4], id: u16) -> Layer {
    Layer::new("ipv4")
        .with("src", Ipv4Addr::from(src))
        .with("dst", Ipv4Addr::from(dst))
        .with("identification", id)
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

// --------------------------------------------------------------------------
// GRE: ip / gre / ip
// --------------------------------------------------------------------------

const GRE: &str = "4500002f 0001 0000 40 2f 669d 0a000001 0a000002 \
    0000 0800 \
    4500 0017 0002 0000 40 fd f694 c0a80101 c0a80102 \
    677265";

#[test]
fn gre_golden() {
    let p = Packet::from_layers(vec![
        ipv4([10, 0, 0, 1], [10, 0, 0, 2], 1),
        Layer::new("gre").with("protocol_type", 0x0800u16),
        ipv4([192, 168, 1, 1], [192, 168, 1, 2], 2),
        Layer::new("raw").with("data", b"gre".to_vec()),
    ])
    .unpadded();
    let built = p.build(catalog()).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    assert_eq!(built.bytes, hex(GRE));
    // Outer IP protocol 47 (GRE), both IPv4 headers verify.
    assert_eq!(built.bytes[9], 47);
    assert!(verifies(&built.bytes[0..20]), "outer IPv4 checksum");
    assert!(verifies(&built.bytes[24..44]), "inner IPv4 checksum");
    // GRE base header: flags 0, protocol type 0x0800.
    assert_eq!(&built.bytes[20..24], &[0x00, 0x00, 0x08, 0x00]);
    assert_eq!(
        roundtrip(&built.bytes, "ipv4"),
        ["ipv4", "gre", "ipv4", "raw"]
    );

    assert_dissects(&built.bytes, Some(0x0800), "gre");
}

#[test]
fn nvgre_encapsulates_ethernet() {
    let p = Packet::from_layers(vec![
        ipv4([10, 0, 0, 1], [10, 0, 0, 2], 1),
        Layer::new("nvgre")
            .with("vsid", 0x00abcdu32)
            .with("flow_id", 0x11u8),
        Layer::new("ethernet2")
            .with("dst", FieldValue::Mac([0xaa; 6]))
            .with("src", FieldValue::Mac([0xbb; 6]))
            .with("ethertype", 0x0800u16),
        ipv4([192, 0, 2, 1], [192, 0, 2, 2], 2),
    ])
    .unpadded();
    let built = p.build(catalog()).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    // GRE header with Key bit (0x2000), protocol 0x6558, VSID 0x00abcd, `FlowID` 0x11.
    assert_eq!(
        &built.bytes[20..28],
        &[0x20, 0x00, 0x65, 0x58, 0x00, 0xab, 0xcd, 0x11]
    );
    // NVGRE is not auto-reachable on parse (IP proto 47 → gre); build only.
}

// --------------------------------------------------------------------------
// MPLS: eth / mpls / mpls / ip / udp
// --------------------------------------------------------------------------

const MPLS: &str = "00112233445566778899aabb 8847 \
    00064040 \
    000c8140 \
    45000020 0003 0000 40 11 66c8 0a000001 0a000002 \
    0457 08ae 000c 04eb \
    6d706c73";

#[test]
fn mpls_two_labels_golden() {
    let p = Packet::from_layers(vec![
        Layer::new("ethernet2")
            .with("dst", FieldValue::Mac([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]))
            .with("src", FieldValue::Mac([0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb]))
            .with("ethertype", 0x8847u16),
        Layer::new("mpls")
            .with("label", 100u32)
            .with("s", 0u8)
            .with("ttl", 64u8),
        Layer::new("mpls")
            .with("label", 200u32)
            .with("s", 1u8)
            .with("ttl", 64u8),
        ipv4([10, 0, 0, 1], [10, 0, 0, 2], 3),
        Layer::new("udp")
            .with("src_port", 1111u16)
            .with("dst_port", 2222u16),
        Layer::new("raw").with("data", b"mpls".to_vec()),
    ])
    .unpadded();
    let built = p.build(catalog()).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    assert_eq!(built.bytes, hex(MPLS));

    // Decode the two 32-bit label entries independently.
    let entry = |o: usize| {
        let w = u32::from_be_bytes([
            built.bytes[o],
            built.bytes[o + 1],
            built.bytes[o + 2],
            built.bytes[o + 3],
        ]);
        (w >> 12, (w >> 9) & 0x7, (w >> 8) & 1, w & 0xff) // label, tc, s, ttl
    };
    assert_eq!(entry(14), (100, 0, 0, 64));
    assert_eq!(entry(18), (200, 0, 1, 64));
    assert!(verifies(&built.bytes[22..42]), "inner IPv4 checksum");

    // The bottom-of-stack payload has no first-nibble binding, so it parses as raw.
    assert_eq!(
        roundtrip(&built.bytes, "ethernet2"),
        ["ethernet2", "mpls", "mpls", "raw"]
    );

    assert_dissects(&built.bytes, None, "mpls");
}

// --------------------------------------------------------------------------
// VXLAN: eth / ip / udp / vxlan / eth / ip / icmp
// --------------------------------------------------------------------------

const VXLAN: &str = "00112233445566778899aabb 0800 \
    45000050 0006 0000 40 11 6695 0a000001 0a000002 \
    ffff 12b5 003c 2d57 \
    08000000 12345600 \
    aaaaaaaaaaaa bbbbbbbbbbbb 0800 \
    4500001e 0007 0000 40 01 f984 c0a80001 c0a80002 \
    0800 8185 0001 0001 \
    7678";

#[test]
fn vxlan_golden() {
    let p = Packet::from_layers(vec![
        Layer::new("ethernet2")
            .with("dst", FieldValue::Mac([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]))
            .with("src", FieldValue::Mac([0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb])),
        ipv4([10, 0, 0, 1], [10, 0, 0, 2], 6),
        Layer::new("udp")
            .with("src_port", 0xffffu16)
            .with("dst_port", 4789u16),
        Layer::new("vxlan").with("vni", 0x123456u32),
        Layer::new("ethernet2")
            .with("dst", FieldValue::Mac([0xaa; 6]))
            .with("src", FieldValue::Mac([0xbb; 6])),
        ipv4([192, 168, 0, 1], [192, 168, 0, 2], 7),
        Layer::new("icmpv4")
            .with("type", 8u8)
            .with("rest", vec![0, 1, 0, 1]),
        Layer::new("raw").with("data", b"vx".to_vec()),
    ])
    .unpadded();
    let built = p.build(catalog()).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    assert_eq!(built.bytes, hex(VXLAN));

    // VXLAN header: I flag set (0x08), VNI 0x123456.
    assert_eq!(
        &built.bytes[42..50],
        &[0x08, 0, 0, 0, 0x12, 0x34, 0x56, 0x00]
    );
    // Outer IPv4, outer UDP (pseudo header), inner IPv4 and inner ICMP all verify.
    assert!(verifies(&built.bytes[14..34]), "outer IPv4");
    let mut ph = built.bytes[26..34].to_vec(); // outer src+dst
    ph.extend_from_slice(&[0, 17]);
    let udp_len = u16::try_from(built.bytes.len() - 34).unwrap();
    ph.extend_from_slice(&udp_len.to_be_bytes());
    ph.extend_from_slice(&built.bytes[34..]);
    assert!(verifies(&ph), "outer UDP checksum with pseudo header");
    assert!(verifies(&built.bytes[64..84]), "inner IPv4");
    assert!(verifies(&built.bytes[84..]), "inner ICMP");

    assert_eq!(
        roundtrip(&built.bytes, "ethernet2"),
        [
            "ethernet2",
            "ipv4",
            "udp",
            "vxlan",
            "ethernet2",
            "ipv4",
            "icmpv4",
            "raw"
        ]
    );

    assert_dissects(&built.bytes, None, "vxlan");
}

// --------------------------------------------------------------------------
// GTP-U: ip / udp / gtpu / ip
// --------------------------------------------------------------------------

const GTPU: &str = "4500003b 0004 0000 40 11 66ac 0a000001 0a000002 \
    0868 0868 0027 c00e \
    30 ff 0017 00001234 \
    45000017 0005 0000 40 fd f5e1 c0000201 c0000202 \
    677470";

#[test]
fn gtpu_golden() {
    let p = Packet::from_layers(vec![
        ipv4([10, 0, 0, 1], [10, 0, 0, 2], 4),
        Layer::new("udp")
            .with("src_port", 2152u16)
            .with("dst_port", 2152u16),
        Layer::new("gtpu")
            .with("teid", 0x1234u32)
            .with("message_type", 255u8),
        ipv4([192, 0, 2, 1], [192, 0, 2, 2], 5),
        Layer::new("raw").with("data", b"gtp".to_vec()),
    ])
    .unpadded();
    let built = p.build(catalog()).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    assert_eq!(built.bytes, hex(GTPU));

    // GTP-U header: flags 0x30 (v1, PT), message type 0xff (G-PDU), length 23, TEID.
    assert_eq!(built.bytes[28], 0x30);
    assert_eq!(built.bytes[29], 0xff);
    assert_eq!(u16::from_be_bytes([built.bytes[30], built.bytes[31]]), 23);
    assert_eq!(&built.bytes[32..36], &[0, 0, 0x12, 0x34]);
    assert!(verifies(&built.bytes[0..20]), "outer IPv4");
    assert!(verifies(&built.bytes[36..56]), "inner IPv4");

    assert_eq!(
        roundtrip(&built.bytes, "ipv4"),
        ["ipv4", "udp", "gtpu", "ipv4", "raw"]
    );

    assert_dissects(&built.bytes, Some(0x0800), "gtp");
}

// --------------------------------------------------------------------------
// Geneve: ip / udp / geneve (one option) / ip
// --------------------------------------------------------------------------

const GENEVE: &str = "4500003e 0008 0000 40 11 66a5 0a000001 0a000002 \
    ffff 17c1 002a c790 \
    01 00 0800 abcdef 00 \
    00010100 \
    45000016 0009 0000 40 fd f5de c0000201 c0000202 \
    6776";

#[test]
fn geneve_golden() {
    let p = Packet::from_layers(vec![
        ipv4([10, 0, 0, 1], [10, 0, 0, 2], 8),
        Layer::new("udp")
            .with("src_port", 0xffffu16)
            .with("dst_port", 6081u16),
        Layer::new("geneve")
            .with("protocol_type", 0x0800u16)
            .with("opt_len", 1u8)
            .with("options", vec![0x00, 0x01, 0x01, 0x00])
            .with("vni", 0xabcdefu32),
        ipv4([192, 0, 2, 1], [192, 0, 2, 2], 9),
        Layer::new("raw").with("data", b"gv".to_vec()),
    ])
    .unpadded();
    let built = p.build(catalog()).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    assert_eq!(built.bytes, hex(GENEVE));

    // Geneve base header: ver 0 / opt_len 1 (4 octets), protocol 0x0800, VNI.
    assert_eq!(built.bytes[28] & 0x3f, 1, "opt_len = 1 word");
    assert_eq!(&built.bytes[30..32], &[0x08, 0x00], "protocol type IPv4");
    assert_eq!(&built.bytes[32..35], &[0xab, 0xcd, 0xef], "VNI");
    assert!(verifies(&built.bytes[0..20]), "outer IPv4");
    assert!(verifies(&built.bytes[40..60]), "inner IPv4");

    assert_eq!(
        roundtrip(&built.bytes, "ipv4"),
        ["ipv4", "udp", "geneve", "ipv4", "raw"]
    );

    assert_dissects(&built.bytes, Some(0x0800), "geneve");
}

// --------------------------------------------------------------------------
// IP-in-IP (already bound: IPv4 protocol 4 → ipv4, 41 → ipv6)
// --------------------------------------------------------------------------

#[test]
fn ip_in_ip_encapsulation() {
    // IPv4-in-IPv4 (protocol 4).
    let p = Packet::from_layers(vec![
        ipv4([203, 0, 113, 1], [203, 0, 113, 2], 1),
        ipv4([10, 0, 0, 1], [10, 0, 0, 2], 2),
        Layer::new("icmpv4").with("type", 8u8),
        Layer::new("raw").with("data", b"ping".to_vec()),
    ])
    .unpadded();
    let built = p.build(catalog()).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    assert_eq!(built.bytes[9], 4, "outer IP protocol 4 (IP-in-IP)");
    assert!(verifies(&built.bytes[0..20]), "outer IPv4");
    assert!(verifies(&built.bytes[20..40]), "inner IPv4");
    assert_eq!(
        roundtrip(&built.bytes, "ipv4"),
        ["ipv4", "ipv4", "icmpv4", "raw"]
    );

    // IPv6-in-IPv4 (protocol 41).
    let p = Packet::from_layers(vec![
        ipv4([203, 0, 113, 1], [203, 0, 113, 2], 3),
        Layer::new("ipv6"),
        Layer::new("raw").with("data", b"6in4".to_vec()),
    ])
    .unpadded();
    let built = p.build(catalog()).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    assert_eq!(built.bytes[9], 41, "outer IP protocol 41 (IPv6-in-IPv4)");
    assert_eq!(roundtrip(&built.bytes, "ipv4"), ["ipv4", "ipv6", "raw"]);
}

// --------------------------------------------------------------------------
// Proptest: random MPLS label stacks round-trip
// --------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn mpls_label_stacks_roundtrip(
        labels in proptest::collection::vec(0u32..=0xfffff, 1..6),
        ttl in any::<u8>(),
    ) {
        let mut layers = vec![Layer::new("ethernet2")
            .with("src", FieldValue::Mac([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]))
            .with("ethertype", 0x8847u16)];
        let last = labels.len() - 1;
        for (i, label) in labels.iter().enumerate() {
            layers.push(
                Layer::new("mpls")
                    .with("label", *label)
                    .with("s", u8::from(i == last))
                    .with("ttl", ttl),
            );
        }
        layers.push(Layer::new("raw").with("data", vec![0xde, 0xad, 0xbe, 0xef]));
        let bytes = Packet::from_layers(layers).unpadded().build(catalog()).unwrap().bytes;
        let parsed = parse(&bytes, "ethernet2", catalog()).unwrap();
        prop_assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    }
}
