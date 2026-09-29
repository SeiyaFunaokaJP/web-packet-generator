//! Golden byte vectors, Manual overrides and constraint diagnostics for the
//! reference protocols. Expected bytes were produced independently (Python
//! `struct` + a textbook RFC 1071 implementation) and checksums are re-verified
//! here with a naive implementation.

use std::net::Ipv4Addr;

use wpg_core::diag::keys;
use wpg_core::{parse, DiagLevel, FieldValue, Layer, Packet};
use wpg_protocols::catalog;

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// Naive RFC 1071: sum 16-bit big-endian words with end-around carry.
fn naive_sum(data: &[u8]) -> u32 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i < data.len() {
        let hi = u32::from(data[i]);
        let lo = u32::from(*data.get(i + 1).unwrap_or(&0));
        sum += (hi << 8) | lo;
        while sum > 0xffff {
            sum = (sum & 0xffff) + (sum >> 16);
        }
        i += 2;
    }
    sum
}

fn verifies(data: &[u8]) -> bool {
    naive_sum(data) == 0xffff
}

fn eth() -> Layer {
    Layer::new("ethernet2")
        .with("dst", FieldValue::Mac([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]))
        .with("src", FieldValue::Mac([0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb]))
}

fn eth_ipv4_udp() -> Packet {
    Packet {
        layers: vec![
            eth(),
            Layer::new("ipv4")
                .with("identification", 0x1234u16)
                .with("flags", 0b010u8)
                .with("src", Ipv4Addr::new(10, 0, 0, 1))
                .with("dst", Ipv4Addr::new(10, 0, 0, 2)),
            Layer::new("udp")
                .with("src_port", 1234u16)
                .with("dst_port", 5678u16),
            Layer::new("raw").with("data", b"hello".to_vec()),
        ],
    }
}

const ETH_IPV4_UDP: &str = "00112233445566778899aabb0800\
    4500002112344000401114960a0000010a000002\
    04d2162e000d8cff\
    68656c6c6f";

fn eth_vlan_ipv4_tcp() -> Packet {
    Packet {
        layers: vec![
            eth(),
            Layer::new("dot1q").with("pcp", 5u8).with("vid", 100u16),
            Layer::new("ipv4")
                .with("identification", 1u16)
                .with("ttl", 128u8)
                .with("src", Ipv4Addr::new(192, 168, 1, 10))
                .with("dst", Ipv4Addr::new(198, 51, 100, 7)),
            Layer::new("tcp")
                .with("src_port", 40000u16)
                .with("dst_port", 443u16)
                .with("seq", 0x0102_0304u32)
                .with("window", 64240u16)
                .with("options", vec![0x02, 0x04, 0x05, 0xb4]),
        ],
    }
}

const ETH_VLAN_IPV4_TCP: &str = "00112233445566778899aabb8100\
    a0640800\
    4500002c0001000080064edec0a8010ac6336407\
    9c4001bb01020304000000006002faf00f470000020405b4";

#[test]
fn golden_ethernet_ipv4_udp() {
    let built = eth_ipv4_udp().build(catalog()).unwrap();
    assert_eq!(built.bytes, hex(ETH_IPV4_UDP));
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);

    let b = &built.bytes;
    assert!(verifies(&b[14..34]), "IPv4 header checksum");
    let mut ph = b[26..34].to_vec(); // src + dst
    ph.extend_from_slice(&[0, 17, 0, 13]);
    ph.extend_from_slice(&b[34..]);
    assert!(verifies(&ph), "UDP checksum with pseudo header");

    // Layout: auto flags and bit positions of packed fields.
    let ip = &built.layout[1];
    assert_eq!((ip.offset, ip.header_len, ip.len), (14, 20, 33));
    let f = |n: &str| ip.fields.iter().find(|f| f.name == n).unwrap();
    assert_eq!((f("ihl").bit_offset, f("ihl").bit_len), (14 * 8 + 4, 4));
    assert_eq!(
        (f("frag_offset").bit_offset, f("frag_offset").bit_len),
        (14 * 8 + 51, 13)
    );
    assert!(f("ihl").auto && f("total_length").auto && f("header_checksum").auto);
    assert!(f("protocol").auto);
    assert!(!f("ttl").auto);
    assert_eq!(f("protocol").value, FieldValue::UInt(17));
}

#[test]
fn golden_ethernet_vlan_ipv4_tcp() {
    let built = eth_vlan_ipv4_tcp().build(catalog()).unwrap();
    assert_eq!(built.bytes, hex(ETH_VLAN_IPV4_TCP));
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);

    let b = &built.bytes;
    assert!(verifies(&b[18..38]), "IPv4 header checksum");
    let mut ph = b[30..38].to_vec();
    ph.extend_from_slice(&[0, 6, 0, 24]);
    ph.extend_from_slice(&b[38..]);
    assert!(verifies(&ph), "TCP checksum with pseudo header");
}

#[test]
fn parse_golden_vectors_roundtrip() {
    for (bytes, protos) in [
        (hex(ETH_IPV4_UDP), &["ethernet2", "ipv4", "udp", "raw"][..]),
        (
            hex(ETH_VLAN_IPV4_TCP),
            &["ethernet2", "dot1q", "ipv4", "tcp"][..],
        ),
    ] {
        let parsed = parse(&bytes, "ethernet2", catalog()).unwrap();
        let got: Vec<_> = parsed
            .packet
            .layers
            .iter()
            .map(|l| l.protocol.as_str())
            .collect();
        assert_eq!(got, protos);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let rebuilt = parsed.packet.build(catalog()).unwrap();
        assert_eq!(rebuilt.bytes, bytes);
        assert_eq!(rebuilt.layout, parsed.layout);
    }
    let parsed = parse(&hex(ETH_VLAN_IPV4_TCP), "ethernet2", catalog()).unwrap();
    let tcp = &parsed.packet.layers[3].fields;
    assert_eq!(tcp["options"], FieldValue::Bytes(vec![2, 4, 5, 0xb4]));
    assert_eq!(tcp["data_offset"], FieldValue::UInt(6));
}

#[test]
fn ipv4_options_are_padded_and_ihl_follows() {
    let mut p = eth_ipv4_udp();
    p.layers[1] = p.layers[1].clone().with("options", vec![0x01, 0x01, 0x01]); // 3 x NOP
    let built = p.build(catalog()).unwrap();
    assert_eq!(built.bytes[14], 0x46, "ihl = 6 words");
    assert_eq!(
        &built.bytes[34..38],
        &[1, 1, 1, 0],
        "zero padding to 4 octets"
    );
    assert!(verifies(&built.bytes[14..38]));
    let parsed = parse(&built.bytes, "ethernet2", catalog()).unwrap();
    assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, built.bytes);
}

#[test]
fn manual_wrong_checksum_is_kept_and_warned() {
    let mut p = eth_ipv4_udp();
    p.layers[2] = p.layers[2].clone().with("checksum", 0xbeefu16);
    p.layers[1] = p.layers[1].clone().with("protocol", 6u8);
    let built = p.build(catalog()).unwrap();
    let b = &built.bytes;
    assert_eq!(&b[40..42], &[0xbe, 0xef], "Manual checksum used verbatim");
    assert_eq!(b[23], 6, "Manual protocol used verbatim");
    assert!(verifies(&b[14..34]), "IPv4 checksum still auto and correct");
    let keys: Vec<_> = built
        .diagnostics
        .iter()
        .map(|d| (d.level, d.key.as_str(), d.layer))
        .collect();
    assert!(
        keys.contains(&(DiagLevel::Warning, keys::CHECKSUM_MISMATCH, Some(2))),
        "{keys:?}"
    );
    assert!(
        keys.contains(&(DiagLevel::Warning, keys::NEXT_PROTOCOL_MISMATCH, Some(1))),
        "{keys:?}"
    );

    // A wrong checksum survives parse → build, and parse reports it.
    let parsed = parse(b, "ethernet2", catalog()).unwrap();
    assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, *b);
}

#[test]
fn constraint_diagnostics() {
    let mut p = eth_ipv4_udp();
    p.layers[1] = p.layers[1].clone().with("ihl", 4u8).with("ttl", 0u8);
    let built = p.build(catalog()).unwrap();
    let find = |key: &str, field: &str| {
        built
            .diagnostics
            .iter()
            .find(|d| d.key == key && d.field.as_deref() == Some(field))
            .map(|d| d.level)
    };
    assert_eq!(
        find(keys::FIELD_OUT_OF_RANGE, "ihl"),
        Some(DiagLevel::Error)
    );
    assert_eq!(
        find(keys::FIELD_FORBIDDEN_VALUE, "ttl"),
        Some(DiagLevel::Warning)
    );
    assert_eq!(find(keys::LENGTH_MISMATCH, "ihl"), Some(DiagLevel::Warning));
    assert!(built.has_errors());
    let d = built
        .diagnostics
        .iter()
        .find(|d| d.field.as_deref() == Some("ttl"))
        .unwrap();
    assert_eq!(d.layer, Some(1));
    assert_eq!(d.args["value"], "0");
    assert!(!d.message.is_empty());

    // VLAN 4095 is reserved.
    let mut p = eth_vlan_ipv4_tcp();
    p.layers[1] = p.layers[1].clone().with("vid", 4095u16);
    let built = p.build(catalog()).unwrap();
    assert!(built
        .diagnostics
        .iter()
        .any(|d| d.field.as_deref() == Some("vid")));
}

#[test]
fn next_protocol_defaults_and_raw_payload() {
    let p = Packet {
        layers: vec![eth(), Layer::new("raw").with("data", vec![0xde, 0xad])],
    };
    let built = p.build(catalog()).unwrap();
    assert_eq!(&built.bytes[12..14], &[0x88, 0xb5], "default EtherType");
    let parsed = parse(&built.bytes, "ethernet2", catalog()).unwrap();
    assert_eq!(parsed.packet, {
        let mut e = eth().with("ethertype", 0x88b5u16);
        e.truncate = None;
        Packet {
            layers: vec![e, Layer::new("raw").with("data", vec![0xde, 0xad])],
        }
    });
}

#[test]
fn truncated_frames_parse_with_errors() {
    let full = hex(ETH_VLAN_IPV4_TCP);
    for cut in 0..full.len() {
        let parsed = parse(&full[..cut], "ethernet2", catalog()).unwrap();
        assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, &full[..cut]);
        if cut < full.len() {
            assert!(
                parsed
                    .diagnostics
                    .iter()
                    .any(|d| d.level >= DiagLevel::Warning),
                "cut at {cut} should be diagnosed"
            );
        }
    }

    // Deliberate truncation through the builder.
    let mut p = eth_vlan_ipv4_tcp();
    p.layers[3].truncate = Some(10);
    let built = p.build(catalog()).unwrap();
    assert_eq!(built.bytes, &full[..38 + 10]);
    assert!(built.layout[3].truncated);
}

#[test]
fn parser_never_panics_on_noise() {
    // Deterministic xorshift noise, plus noise behind valid headers.
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let prefixes = [
        Vec::new(),
        hex(&ETH_IPV4_UDP[..28]),
        hex(&ETH_VLAN_IPV4_TCP[..76]),
    ];
    for round in 0..3000 {
        let len = usize::try_from(next() % 120).unwrap();
        let mut bytes = prefixes[round % prefixes.len()].clone();
        bytes.extend((0..len).map(|_| next().to_le_bytes()[0]));
        for first in ["ethernet2", "dot1q", "ipv4", "tcp", "udp", "raw"] {
            let parsed = parse(&bytes, first, catalog()).unwrap();
            let rebuilt = parsed.packet.build(catalog()).unwrap();
            assert_eq!(rebuilt.bytes, bytes, "first={first}");
        }
    }
}
