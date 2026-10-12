//! `SRv6` Segment Routing Header (RFC 8754) and SR-MPLS helpers (M7): golden byte
//! vectors, Auto `hdr_ext_len` / `last_entry` / padding, the routing-type peek that
//! picks `ipv6_srh` over the generic Routing header, the final destination used by
//! upper-layer pseudo headers, `SRv6` encapsulation (IPv6 / IPv4 / Ethernet inside),
//! named reserved MPLS labels, parse round-trips and proptests.
//!
//! Expected bytes were derived field-by-field from RFC 8754 §2 and RFC 8200; the
//! UDP checksum is re-verified with a naive one's complement sum over a pseudo
//! header whose destination is the last segment. An optional `tshark` cross-check
//! runs when `WPG_TSHARK=1`.

use std::net::{Ipv4Addr, Ipv6Addr};

use proptest::prelude::*;
use wpg_core::stream::FieldRef;
use wpg_core::{parse, FieldValue, Layer, Packet, Prebuilt, TlvItem};
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

fn v6(s: &str) -> Ipv6Addr {
    s.parse().unwrap()
}

fn segs(list: &[&str]) -> FieldValue {
    FieldValue::List(list.iter().map(|a| FieldValue::Ipv6(v6(a))).collect())
}

fn eth() -> Layer {
    Layer::new("ethernet2")
        .with("dst", FieldValue::Mac([2, 0, 0, 0, 0, 2]))
        .with("src", FieldValue::Mac([2, 0, 0, 0, 0, 1]))
}

fn ipv6(src: &str, dst: &str) -> Layer {
    Layer::new("ipv6").with("src", v6(src)).with("dst", v6(dst))
}

const ETH_IPV6: &str = "020000000002 020000000001 86dd";

// ---------------------------------------------------------------------------
// SRH
// ---------------------------------------------------------------------------

/// Two segments, one left, carrying UDP: the checksum uses the last segment.
#[test]
fn srh_udp_golden() {
    let bytes = build(vec![
        eth(),
        ipv6("2001:db8::1", "2001:db8:0:2::1"),
        Layer::new("ipv6_srh").with(
            "segment_list",
            segs(&["2001:db8:0:3::1", "2001:db8:0:2::1"]),
        ),
        Layer::new("udp")
            .with("src_port", 40000u16)
            .with("dst_port", 5000u16),
        Layer::new("raw").with("data", b"srv6".to_vec()),
    ]);
    assert_golden(
        &bytes,
        &format!(
            "{ETH_IPV6}
             60000000 0034 2b 40
             20010db8000000000000000000000001
             20010db8000000020000000000000001
             11 04 04 01 01 00 0000
             20010db8000000030000000000000001
             20010db8000000020000000000000001
             9c40 1388 000c 0aee
             73727636"
        ),
    );

    // UDP checksum over the pseudo header with the final destination.
    let mut ph = Vec::new();
    ph.extend_from_slice(&v6("2001:db8::1").octets());
    ph.extend_from_slice(&v6("2001:db8:0:3::1").octets());
    ph.extend_from_slice(&[0, 0, 0, 12, 0, 0, 0, 17]);
    ph.extend_from_slice(&bytes[bytes.len() - 12..]);
    assert_eq!(ones_complement(&ph), 0);

    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "ipv6", "ipv6_srh", "udp", "raw"]);
    assert_eq!(
        p.layers[2].fields["segment_list"],
        segs(&["2001:db8:0:3::1", "2001:db8:0:2::1"])
    );
    assert_dissects(&bytes, None, "ipv6.routing");
}

/// With no segment left the pseudo header uses the IPv6 destination again.
#[test]
fn srh_without_segments_left_uses_ipv6_destination() {
    let bytes = build(vec![
        eth(),
        ipv6("2001:db8::1", "2001:db8:0:3::1"),
        Layer::new("ipv6_srh").with("segments_left", 0u8).with(
            "segment_list",
            segs(&["2001:db8:0:9::1", "2001:db8:0:2::1"]),
        ),
        Layer::new("udp")
            .with("src_port", 40000u16)
            .with("dst_port", 5000u16),
        Layer::new("raw").with("data", b"srv6".to_vec()),
    ]);
    let mut ph = Vec::new();
    ph.extend_from_slice(&v6("2001:db8::1").octets());
    ph.extend_from_slice(&v6("2001:db8:0:3::1").octets());
    ph.extend_from_slice(&[0, 0, 0, 12, 0, 0, 0, 17]);
    ph.extend_from_slice(&bytes[bytes.len() - 12..]);
    assert_eq!(ones_complement(&ph), 0);
    roundtrip(&bytes);
}

/// The fast patch path follows the final destination and `segments_left`.
#[test]
fn srh_prebuilt_patch_matches_full_build() {
    let base = Packet::from_layers(vec![
        eth(),
        ipv6("2001:db8::1", "2001:db8:0:2::1"),
        Layer::new("ipv6_srh").with(
            "segment_list",
            segs(&["2001:db8:0:3::1", "2001:db8:0:2::1"]),
        ),
        Layer::new("udp")
            .with("src_port", 40000u16)
            .with("dst_port", 5000u16),
        Layer::new("raw").with("data", b"srv6".to_vec()),
    ]);
    let mut pre = Prebuilt::new(&base, catalog()).unwrap();
    for left in [0u8, 1, 2] {
        let mut want = base.clone();
        want.layers[2]
            .fields
            .insert("segments_left".into(), FieldValue::UInt(left.into()));
        let got = pre
            .patch(&[(
                FieldRef::new("ipv6_srh", "segments_left"),
                FieldValue::UInt(left.into()),
            )])
            .unwrap()
            .to_vec();
        assert_eq!(
            got,
            want.build(catalog()).unwrap().bytes,
            "segments_left {left}"
        );
    }
    assert_eq!(pre.fallbacks(), 0);
}

/// HMAC TLV (RFC 8754 §2.1.2) followed by Pad1 octets up to a multiple of 8.
#[test]
fn srh_hmac_tlv_and_padding() {
    let bytes = build(vec![
        eth(),
        ipv6("2001:db8::1", "2001:db8:0:2::1"),
        Layer::new("ipv6_srh")
            .with("segment_list", segs(&["2001:db8:0:2::1"]))
            .with("segments_left", 0u8)
            .with(
                "tlvs",
                vec![TlvItem::named("hmac")
                    .with("hmac_key_id", 7u32)
                    .with("hmac", vec![0xaa; 8])],
            ),
        Layer::new("ipv6").with("next_header", 59u8),
    ]);
    // 8 + 16 + (2 + 14) = 40 octets: already a multiple of 8.
    assert_golden(
        &bytes[14 + 40..14 + 40 + 40],
        "29 04 04 00 00 00 0000
         20010db8000000020000000000000001
         05 0e 0000 00000007 aaaaaaaaaaaaaaaa",
    );
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "ipv6", "ipv6_srh", "ipv6"]);

    // A PadN TLV of 1 octet value: 8 + 16 + 3 = 27 → five Pad1 octets.
    let bytes = build(vec![
        eth(),
        ipv6("2001:db8::1", "2001:db8:0:2::1"),
        Layer::new("ipv6_srh")
            .with("segment_list", segs(&["2001:db8:0:2::1"]))
            .with("segments_left", 0u8)
            .with(
                "tlvs",
                vec![TlvItem::named("padn").with("padding", vec![0u8])],
            ),
        Layer::new("ipv6").with("next_header", 59u8),
    ]);
    assert_golden(
        &bytes[14 + 40..14 + 40 + 32],
        "29 03 04 00 00 00 0000
         20010db8000000020000000000000001
         04 01 00 0000000000",
    );
    roundtrip(&bytes);
}

/// Routing header types other than 4 still decode as the generic header.
#[test]
fn routing_type_peek() {
    let bytes = build(vec![
        eth(),
        ipv6("2001:db8::1", "2001:db8::2"),
        Layer::new("ipv6_route").with("segments_left", 0u8),
        Layer::new("udp")
            .with("src_port", 1u16)
            .with("dst_port", 2u16),
    ]);
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "ipv6", "ipv6_route", "udp"]);
}

/// `SRv6` encapsulations (RFC 8986): IPv4 (`End.DX4`), IPv6 (`End.DX6`) and Ethernet
/// (`End.DX2`, next header 143) inside the outer IPv6 + SRH.
#[test]
fn srv6_encapsulations() {
    let inner_v4 = vec![
        Layer::new("ipv4")
            .with("src", Ipv4Addr::new(10, 0, 0, 1))
            .with("dst", Ipv4Addr::new(10, 0, 0, 2)),
        Layer::new("udp")
            .with("src_port", 1u16)
            .with("dst_port", 2u16),
    ];
    let inner_v6 = vec![
        ipv6("2001:db8:a::1", "2001:db8:b::1"),
        Layer::new("udp")
            .with("src_port", 1u16)
            .with("dst_port", 2u16),
    ];
    let inner_eth = vec![eth(), Layer::new("arp")];
    for (inner, nh, want) in [
        (inner_v4, 4u8, vec!["ipv4", "udp"]),
        (inner_v6, 41, vec!["ipv6", "udp"]),
        (inner_eth, 143, vec!["ethernet2", "arp"]),
    ] {
        let mut layers = vec![
            eth(),
            ipv6("2001:db8::1", "fc00:0:2:e000::"),
            Layer::new("ipv6_srh").with(
                "segment_list",
                segs(&["fc00:0:3:e000::", "fc00:0:2:e000::"]),
            ),
        ];
        layers.extend(inner);
        let bytes = build(layers);
        assert_eq!(bytes[14 + 40], nh, "next header");
        let p = roundtrip(&bytes);
        assert_eq!(protos(&p)[3..], want[..]);
    }
}

/// `last_entry` is Auto from the number of segments.
#[test]
fn last_entry_is_auto() {
    let three = ["2001:db8::3", "2001:db8::2", "2001:db8::1"];
    let bytes = build(vec![
        eth(),
        ipv6("2001:db8::", "2001:db8::1"),
        Layer::new("ipv6_srh")
            .with("segments_left", 2u8)
            .with("segment_list", segs(&three)),
        Layer::new("ipv6").with("next_header", 59u8),
    ]);
    let srh = &bytes[14 + 40..];
    assert_eq!(srh[1], 6, "hdr_ext_len: 8 + 48 = 56 octets");
    assert_eq!(srh[4], 2, "last_entry");
    roundtrip(&bytes);
}

// ---------------------------------------------------------------------------
// SR-MPLS helpers
// ---------------------------------------------------------------------------

/// SR-MPLS label stack: prefix SID (SRGB 16000 + index 101), the entropy label
/// indicator and an entropy label (RFC 6790), then IPv4.
#[test]
fn sr_mpls_stack_with_entropy_label() {
    let bytes = build(vec![
        eth(),
        Layer::new("mpls").with("label", 16101u32),
        Layer::new("mpls").with("label", 7u32), // entropy_label_indicator
        Layer::new("mpls")
            .with("label", 0x5_4321u32)
            .with("ttl", 0u8),
        Layer::new("ipv4")
            .with("src", Ipv4Addr::new(10, 0, 0, 1))
            .with("dst", Ipv4Addr::new(10, 0, 0, 2)),
        Layer::new("udp")
            .with("src_port", 1u16)
            .with("dst_port", 2u16),
    ]);
    assert_golden(&bytes[12..26], "8847 03ee5040 00007040 54321100");
    let p = roundtrip(&bytes);
    assert_eq!(
        protos(&p),
        ["ethernet2", "mpls", "mpls", "mpls", "ipv4", "udp"]
    );
    assert_dissects(&bytes, None, "mpls");
}

// ---------------------------------------------------------------------------
// Proptests
// ---------------------------------------------------------------------------

fn addr() -> impl Strategy<Value = Ipv6Addr> {
    any::<u128>().prop_map(Ipv6Addr::from)
}

proptest! {
    #[test]
    fn srh_roundtrips(
        list in proptest::collection::vec(addr(), 1..6),
        left in 0u8..6,
        tag in any::<u16>(),
        pad in proptest::collection::vec(any::<u8>(), 0..12),
    ) {
        let left = left.min(u8::try_from(list.len() - 1).unwrap());
        let tlvs = if pad.is_empty() {
            Vec::new()
        } else {
            vec![TlvItem::named("padn").with("padding", pad)]
        };
        let bytes = build(vec![
            eth(),
            ipv6("2001:db8::1", "2001:db8::2"),
            Layer::new("ipv6_srh")
                .with("segments_left", left)
                .with("tag", tag)
                .with("segment_list", FieldValue::List(list.into_iter().map(FieldValue::Ipv6).collect()))
                .with("tlvs", tlvs),
            Layer::new("udp").with("src_port", 1u16).with("dst_port", 2u16),
        ]);
        prop_assert_eq!((bytes.len() - 14 - 40 - 8) % 8, 0);
        let p = roundtrip(&bytes);
        prop_assert_eq!(protos(&p), ["ethernet2", "ipv6", "ipv6_srh", "udp"]);
    }
}
