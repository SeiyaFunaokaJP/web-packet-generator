//! ICMPv4 / ICMPv6 / NDP golden bytes and round trips.

use std::net::{Ipv4Addr, Ipv6Addr};

use wpg_core::{parse, DiagLevel, Layer, Packet};
use wpg_protocols::catalog;

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn naive_sum(data: &[u8]) -> u32 {
    let mut sum: u32 = 0;
    for c in data.chunks(2) {
        sum += (u32::from(c[0]) << 8) | u32::from(*c.get(1).unwrap_or(&0));
        while sum > 0xffff {
            sum = (sum & 0xffff) + (sum >> 16);
        }
    }
    sum
}

fn a(s: &str) -> Ipv6Addr {
    s.parse().unwrap()
}

fn assert_roundtrip(bytes: &[u8], first: &str, protos: &[&str]) {
    let parsed = parse(bytes, first, catalog()).unwrap();
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let got: Vec<_> = parsed
        .packet
        .layers
        .iter()
        .map(|l| l.protocol.as_str())
        .collect();
    assert_eq!(got, protos);
    assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
}

// Echo request, id 0x1234, seq 1, data "abcdefgh" (checksum from Python RFC 1071).
const ICMP4_ECHO: &str = "08005435123400016162636465666768";

fn echo4() -> Packet {
    Packet::from_layers(vec![
        Layer::new("icmpv4").with("rest", vec![0x12u8, 0x34, 0x00, 0x01]),
        Layer::new("raw").with("data", b"abcdefgh".to_vec()),
    ])
}

#[test]
fn golden_icmpv4_echo_request() {
    let built = echo4().build(catalog()).unwrap();
    assert_eq!(built.bytes, hex(ICMP4_ECHO));
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    assert_eq!(naive_sum(&built.bytes), 0xffff);
}

#[test]
fn icmpv4_in_ipv4_roundtrip() {
    let mut p = echo4();
    p.layers.insert(
        0,
        Layer::new("ipv4")
            .with("src", Ipv4Addr::new(10, 0, 0, 1))
            .with("dst", Ipv4Addr::new(10, 0, 0, 2)),
    );
    p.layers.insert(0, Layer::new("ethernet2"));
    let built = p.build(catalog()).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    assert_eq!(built.bytes[23], 1, "IPv4 protocol = ICMP");
    assert_eq!(naive_sum(&built.bytes[34..]), 0xffff);
    assert_roundtrip(
        &built.bytes,
        "ethernet2",
        &["ethernet2", "ipv4", "icmpv4", "raw"],
    );
}

#[test]
fn icmpv4_unknown_type_warns() {
    let p = Packet::from_layers(vec![Layer::new("icmpv4").with("type", 200u8)]);
    let b = p.build(catalog()).unwrap();
    assert!(b.diagnostics.iter().any(|d| d.level == DiagLevel::Warning));
}

const IPV6_NS: &str = "6000000000203aff20010db8000000000000000000000001ff0200000000000000000001ff0000018700b39500000000fe800000000000000000000000000001010166778899aabb";

fn ns_packet() -> Packet {
    Packet::from_layers(vec![
        Layer::new("ipv6")
            .with("hop_limit", 255u8)
            .with("src", a("2001:db8::1"))
            .with("dst", a("ff02::1:ff00:1")),
        Layer::new("icmpv6"),
        Layer::new("ndp_ns")
            .with("target", a("fe80::1"))
            .with("options", vec![1u8, 1, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb]),
    ])
}

#[test]
fn golden_icmpv6_neighbor_solicitation() {
    let built = ns_packet().build(catalog()).unwrap();
    assert_eq!(built.bytes, hex(IPV6_NS));
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);

    let b = &built.bytes;
    assert_eq!(b[6], 58);
    assert_eq!(b[40], 135, "type derived from the ndp_ns binding");
    let mut ph = b[8..40].to_vec();
    ph.extend_from_slice(&32u32.to_be_bytes());
    ph.extend_from_slice(&[0, 0, 0, 58]);
    ph.extend_from_slice(&b[40..]);
    assert_eq!(naive_sum(&ph), 0xffff);
    assert_roundtrip(b, "ipv6", &["ipv6", "icmpv6", "ndp_ns"]);
}

#[test]
fn icmpv6_echo_and_other_ndp_messages() {
    let echo = Packet::from_layers(vec![
        Layer::new("ipv6"),
        Layer::new("icmpv6"),
        Layer::new("icmpv6_echo")
            .with("identifier", 7u16)
            .with("sequence", 9u16),
        Layer::new("raw").with("data", b"ping".to_vec()),
    ]);
    let b = echo.build(catalog()).unwrap();
    assert!(b.diagnostics.is_empty(), "{:?}", b.diagnostics);
    assert_eq!(b.bytes[40], 128);
    assert_roundtrip(&b.bytes, "ipv6", &["ipv6", "icmpv6", "icmpv6_echo", "raw"]);

    for (body, ty, len) in [
        ("ndp_rs", 133u8, 4usize),
        ("ndp_ra", 134, 12),
        ("ndp_na", 136, 20),
        ("ndp_redirect", 137, 36),
    ] {
        let p = Packet::from_layers(vec![
            Layer::new("ipv6"),
            Layer::new("icmpv6"),
            Layer::new(body),
        ]);
        let b = p.build(catalog()).unwrap();
        assert!(b.diagnostics.is_empty(), "{body}: {:?}", b.diagnostics);
        assert_eq!(b.bytes[40], ty, "{body}");
        assert_eq!(b.bytes.len() - 40, 4 + len, "{body}");
        assert_roundtrip(&b.bytes, "ipv6", &["ipv6", "icmpv6", body]);
    }
}

#[test]
fn ndp_na_flags_pack_into_first_word() {
    let p = Packet::from_layers(vec![
        Layer::new("ipv6"),
        Layer::new("icmpv6"),
        Layer::new("ndp_na")
            .with("r", 1u8)
            .with("s", 0u8)
            .with("o", 1u8),
    ]);
    let b = p.build(catalog()).unwrap();
    assert_eq!(b.bytes[44], 0b1010_0000);
}
