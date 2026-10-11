//! IPv6 golden bytes, extension header chains and round trips.

use std::net::Ipv6Addr;

use proptest::prelude::*;
use wpg_core::{parse, DiagLevel, FieldValue, Layer, Packet};
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

fn eth() -> Layer {
    Layer::new("ethernet2")
        .with("dst", FieldValue::Mac([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]))
        .with("src", FieldValue::Mac([0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb]))
}

fn udp() -> Layer {
    Layer::new("udp")
        .with("src_port", 1234u16)
        .with("dst_port", 5678u16)
}

fn payload() -> Layer {
    Layer::new("raw").with("data", b"hello".to_vec())
}

fn assert_clean(diags: &[wpg_core::Diagnostic]) {
    assert!(diags.is_empty(), "{diags:?}");
}

const ETH_IPV6_UDP: &str = "00112233445566778899aabb86dd\
    6b812345000d114020010db800000000000000000000000120010db800000000000000000000000204d2162e000d458d\
    68656c6c6f";

const ETH_IPV6_HBH_FRAG_UDP: &str = "00112233445566778899aabb86dd\
    60000000001d004020010db800000000000000000000000120010db80000000000000000000000022c0001040000000011000001deadbeef\
    04d2162e000d458d68656c6c6f";

fn ip6(extra_fields: impl FnOnce(Layer) -> Layer) -> Layer {
    extra_fields(
        Layer::new("ipv6")
            .with("src", a("2001:db8::1"))
            .with("dst", a("2001:db8::2")),
    )
}

fn udp_packet() -> Packet {
    Packet::from_layers(vec![
        eth(),
        ip6(|l| {
            l.with("dscp", 0x2eu8)
                .with("ecn", 0u8)
                .with("flow_label", 0x12345u32)
        }),
        udp(),
        payload(),
    ])
}

fn chain_packet() -> Packet {
    Packet::from_layers(vec![
        eth(),
        ip6(|l| l),
        Layer::new("ipv6_hopopts"),
        Layer::new("ipv6_frag")
            .with("m", 1u8)
            .with("identification", 0xdead_beefu32),
        udp(),
        payload(),
    ])
}

#[test]
fn golden_ethernet_ipv6_udp() {
    let built = udp_packet().build(catalog()).unwrap();
    assert_eq!(built.bytes, hex(ETH_IPV6_UDP));
    assert_clean(&built.diagnostics);

    // Independent pseudo-header verification (IPv6, not IPv4).
    let b = &built.bytes;
    let mut ph = b[22..54].to_vec(); // src + dst
    ph.extend_from_slice(&13u32.to_be_bytes());
    ph.extend_from_slice(&[0, 0, 0, 17]);
    ph.extend_from_slice(&b[54..]);
    assert_eq!(naive_sum(&ph), 0xffff);

    let ip = built.layout.iter().find(|l| l.protocol == "ipv6").unwrap();
    assert_eq!((ip.offset, ip.header_len, ip.len), (14, 40, 53));
    let f = |n: &str| ip.fields.iter().find(|f| f.name == n).unwrap();
    assert!(f("payload_length").auto && f("next_header").auto);
    assert_eq!(f("payload_length").value, FieldValue::UInt(13));
    assert_eq!(f("next_header").value, FieldValue::UInt(17));
    assert_eq!(
        (f("flow_label").bit_offset, f("flow_label").bit_len),
        (14 * 8 + 12, 20)
    );
}

#[test]
fn golden_ipv6_hopopts_frag_udp() {
    let built = chain_packet().build(catalog()).unwrap();
    assert_eq!(built.bytes, hex(ETH_IPV6_HBH_FRAG_UDP));
    assert_clean(&built.diagnostics);

    let b = &built.bytes;
    assert_eq!(b[20], 0, "ipv6 next header = hop-by-hop");
    assert_eq!(u16::from_be_bytes([b[18], b[19]]), 29, "payload_length");
    assert_eq!(b[54], 44, "hbh next = fragment");
    assert_eq!(b[55], 0, "hdr_ext_len for 8 octets");
    assert_eq!(b[62], 17, "fragment next = udp");
    // UDP checksum uses the IPv6 pseudo header with protocol 17 (not 44 / 0).
    let mut ph = b[22..54].to_vec();
    ph.extend_from_slice(&13u32.to_be_bytes());
    ph.extend_from_slice(&[0, 0, 0, 17]);
    ph.extend_from_slice(&b[70..]);
    assert_eq!(naive_sum(&ph), 0xffff);
}

#[test]
fn hdr_ext_len_grows_with_options() {
    let p = Packet::from_layers(vec![
        ip6(|l| l),
        Layer::new("ipv6_dstopts").with("options", vec![1u8; 14]),
        udp(),
    ]);
    let b = p.build(catalog()).unwrap();
    assert_clean(&b.diagnostics);
    // 2 + 14 = 16 octets -> hdr_ext_len 1.
    assert_eq!(b.bytes[40], 17);
    assert_eq!(b.bytes[41], 1);
    assert_eq!(b.layout[1].header_len, 16);
    // Unaligned options are zero padded.
    let p = Packet::from_layers(vec![
        ip6(|l| l),
        Layer::new("ipv6_dstopts").with("options", vec![1u8, 1, 0]),
        udp(),
    ]);
    let b = p.build(catalog()).unwrap();
    assert_eq!(b.layout[1].header_len, 8);
    assert_eq!(b.bytes[41], 0);
}

#[test]
fn routing_header_defaults() {
    let p = Packet::from_layers(vec![ip6(|l| l), Layer::new("ipv6_route"), udp()]);
    let b = p.build(catalog()).unwrap();
    assert_clean(&b.diagnostics);
    assert_eq!(b.bytes[6], 43);
    assert_eq!(b.layout[1].header_len, 24);
    assert_eq!(&b.bytes[40..44], &[17, 2, 2, 1]);
    let parsed = parse(&b.bytes, "ipv6", catalog()).unwrap();
    assert_clean(&parsed.diagnostics);
    assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, b.bytes);
}

#[test]
fn ipv6_in_ipv6_and_ipv4_binding() {
    let p = Packet::from_layers(vec![ip6(|l| l), Layer::new("ipv6"), udp()]);
    let b = p.build(catalog()).unwrap();
    assert_eq!(b.bytes[6], 41);
    let p = Packet::from_layers(vec![Layer::new("ipv4"), ip6(|l| l), udp()]);
    assert_eq!(p.build(catalog()).unwrap().bytes[9], 41);
}

#[test]
fn parse_rebuild_roundtrips() {
    for (bytes, protos) in [
        (hex(ETH_IPV6_UDP), vec!["ethernet2", "ipv6", "udp", "raw"]),
        (
            hex(ETH_IPV6_HBH_FRAG_UDP),
            vec![
                "ethernet2",
                "ipv6",
                "ipv6_hopopts",
                "ipv6_frag",
                "udp",
                "raw",
            ],
        ),
    ] {
        let parsed = parse(&bytes, "ethernet2", catalog()).unwrap();
        assert_clean(&parsed.diagnostics);
        let got: Vec<_> = parsed
            .packet
            .layers
            .iter()
            .map(|l| l.protocol.as_str())
            .collect();
        assert_eq!(got, protos);
        assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    }
}

fn v6_layer() -> impl Strategy<Value = Layer> {
    (
        (0u8..64, 0u8..4, 0u32..(1 << 20), any::<u8>()),
        (any::<[u8; 16]>(), any::<[u8; 16]>()),
    )
        .prop_map(|((dscp, ecn, fl, hl), (s, d))| {
            Layer::new("ipv6")
                .with("dscp", dscp)
                .with("ecn", ecn)
                .with("flow_label", fl)
                .with("hop_limit", hl)
                .with("src", Ipv6Addr::from(s))
                .with("dst", Ipv6Addr::from(d))
        })
}

fn ext_layers() -> impl Strategy<Value = Vec<Layer>> {
    (
        proptest::option::of(proptest::collection::vec(any::<u8>(), 0..30)),
        proptest::option::of((any::<u16>(), any::<u32>(), 0u8..2)),
        proptest::option::of(proptest::collection::vec(any::<u8>(), 0..30)),
    )
        .prop_map(|(hbh, frag, dst)| {
            let mut v = Vec::new();
            if let Some(o) = hbh {
                v.push(Layer::new("ipv6_hopopts").with("options", o));
            }
            if let Some((off, id, m)) = frag {
                v.push(
                    Layer::new("ipv6_frag")
                        .with("fragment_offset", off & 0x1fff)
                        .with("identification", id)
                        .with("m", m),
                );
            }
            if let Some(o) = dst {
                v.push(Layer::new("ipv6_dstopts").with("options", o));
            }
            v
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn ipv6_build_parse_build(
        ip in v6_layer(),
        ext in ext_layers(),
        is_tcp in any::<bool>(),
        data in proptest::collection::vec(any::<u8>(), 0..48),
        // Dynamic-range dst ports carry no `next` binding, so the payload stays `raw`.
        (sp, dp) in (any::<u16>(), 49152u16..=65535),
    ) {
        let mut layers = vec![Layer::new("ethernet2"), ip];
        layers.extend(ext);
        layers.push(if is_tcp {
            Layer::new("tcp").with("src_port", sp).with("dst_port", dp)
        } else {
            Layer::new("udp").with("src_port", sp).with("dst_port", dp)
        });
        if !data.is_empty() {
            layers.push(Layer::new("raw").with("data", data));
        }
        let p = Packet::from_layers(layers);
        let built = p.build(catalog()).unwrap();
        prop_assert!(!built.diagnostics.iter().any(|d| d.level == DiagLevel::Error), "{:?}", built.diagnostics);
        let parsed = parse(&built.bytes, "ethernet2", catalog()).unwrap();
        let want: Vec<_> = p.layers.iter().map(|l| l.protocol.clone()).collect();
        let got: Vec<_> = parsed.packet.layers.iter().map(|l| l.protocol.clone()).collect();
        prop_assert_eq!(got, want);
        let rebuilt = parsed.packet.build(catalog()).unwrap();
        prop_assert_eq!(&rebuilt.bytes, &built.bytes);
    }
}
