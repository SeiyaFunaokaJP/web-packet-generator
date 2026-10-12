//! Golden byte vectors, checksum verification, parse round-trips and proptests for
//! the P1 routing protocols (OSPFv2, OSPFv3, BGP, RIP, `RIPng`, EIGRP).
//!
//! Expected bytes were derived field-by-field; every OSPF / EIGRP Internet
//! checksum (OSPFv3 with the IPv6 pseudo header) and every LSA Fletcher checksum
//! is re-verified here with naive independent implementations (the LSA one per
//! RFC 2328 §12.1.7, i.e. *excluding* the LS age). An optional `tshark`
//! cross-check runs when `WPG_TSHARK=1`.

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

fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Asserts `bytes` equal the spaced hex `want` (compared as hex text so a
/// mismatch prints both sides readably).
fn assert_golden(bytes: &[u8], want: &str) {
    assert_eq!(to_hex(bytes), to_hex(&hex(want)));
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

/// OSPF packet checksum (RFC 2328 D.4.3): the sum over the packet with the 8-octet
/// authentication field (offset 16..24) left out.
fn ospfv2_verifies(pkt: &[u8]) -> bool {
    let mut v = pkt[..16].to_vec();
    v.extend_from_slice(&pkt[24..]);
    verifies(&v)
}

/// IPv6 upper-layer checksum (RFC 8200 §8.1) of `upper` from the IPv6 header
/// `ip6` (40 octets) with next header `nh`.
fn ipv6_pseudo_verifies(ip6: &[u8], nh: u8, upper: &[u8]) -> bool {
    let mut v = ip6[8..40].to_vec();
    v.extend_from_slice(&u32::try_from(upper.len()).unwrap().to_be_bytes());
    v.extend_from_slice(&[0, 0, 0, nh]);
    v.extend_from_slice(upper);
    verifies(&v)
}

/// LSA Fletcher verification (RFC 2328 §12.1.7 / RFC 905 annex B): over the LSA
/// from offset 2 (LS age excluded), both running sums are zero.
fn lsa_fletcher_verifies(lsa: &[u8]) -> bool {
    let (mut c0, mut c1) = (0u32, 0u32);
    for b in &lsa[2..] {
        c0 = (c0 + u32::from(*b)) % 255;
        c1 = (c1 + c0) % 255;
    }
    c0 == 0 && c1 == 0
}

fn ipv4(src: [u8; 4], dst: [u8; 4], id: u16) -> Layer {
    Layer::new("ipv4")
        .with("src", Ipv4Addr::from(src))
        .with("dst", Ipv4Addr::from(dst))
        .with("identification", id)
        .with("ttl", 1u8)
}

fn ipv6(src: Ipv6Addr, dst: Ipv6Addr) -> Layer {
    Layer::new("ipv6")
        .with("src", src)
        .with("dst", dst)
        .with("hop_limit", 1u8)
}

fn ll(last: u16) -> Ipv6Addr {
    Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, last)
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

fn build(layers: Vec<Layer>) -> Vec<u8> {
    let built = Packet::from_layers(layers)
        .unpadded()
        .build(catalog())
        .unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    built.bytes
}

fn v4(o: [u8; 4]) -> FieldValue {
    FieldValue::Ipv4(Ipv4Addr::from(o))
}

fn st(pairs: &[(&str, FieldValue)]) -> FieldValue {
    FieldValue::Struct(
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
    )
}

// --------------------------------------------------------------------------
// OSPFv2 Hello: ip / ospfv2 / ospfv2_hello
// --------------------------------------------------------------------------

const OSPFV2_HELLO: &str = "45c0 0044 0001 0000 01 59 169a c0000201 e0000005 \
    02 01 0030 c0000201 00000000 b694 0000 0000000000000000 \
    ffffff00 000a 02 01 00000028 c0000201 00000000 \
    c0000202";

#[test]
fn ospfv2_hello_golden() {
    let bytes = build(vec![
        ipv4([192, 0, 2, 1], [224, 0, 0, 5], 1).with("dscp", 48u8),
        Layer::new("ospfv2").with("router_id", Ipv4Addr::new(192, 0, 2, 1)),
        Layer::new("ospfv2_hello")
            .with("designated_router", Ipv4Addr::new(192, 0, 2, 1))
            .with("neighbors", vec![v4([192, 0, 2, 2])]),
    ]);
    assert_golden(&bytes, OSPFV2_HELLO);
    assert_eq!(bytes[9], 89, "IP protocol 89 (OSPF)");
    assert!(verifies(&bytes[..20]), "IPv4 header checksum");
    let ospf = &bytes[20..];
    assert_eq!(ospf[1], 1, "type Hello");
    assert_eq!(
        usize::from(u16::from_be_bytes([ospf[2], ospf[3]])),
        ospf.len()
    );
    assert!(ospfv2_verifies(ospf), "OSPF checksum (auth excluded)");
    assert_eq!(
        roundtrip(&bytes, "ipv4"),
        ["ipv4", "ospfv2", "ospfv2_hello"]
    );
    assert_dissects(&bytes, Some(0x0800), "ospf");
}

#[test]
fn ospfv2_dbd_lsr_lsack_roundtrip() {
    let header = st(&[
        ("ls_age", FieldValue::UInt(1)),
        ("options", FieldValue::UInt(0x02)),
        ("ls_type", FieldValue::UInt(1)),
        ("link_state_id", v4([192, 0, 2, 1])),
        ("advertising_router", v4([192, 0, 2, 1])),
        ("ls_sequence", FieldValue::UInt(0x8000_0001)),
        ("ls_checksum", FieldValue::UInt(0x1234)),
        ("length", FieldValue::UInt(36)),
    ]);
    let request = st(&[
        ("ls_type", FieldValue::UInt(1)),
        ("link_state_id", v4([192, 0, 2, 1])),
        ("advertising_router", v4([192, 0, 2, 1])),
    ]);
    let bodies = [
        (
            "ospfv2_dbd",
            Layer::new("ospfv2_dbd")
                .with("flags", 0x01u8)
                .with("dd_sequence", 0x1000u32)
                .with("lsa_headers", vec![header.clone()]),
            2u8,
            8 + 20,
        ),
        (
            "ospfv2_lsr",
            Layer::new("ospfv2_lsr").with("requests", vec![request.clone(), request]),
            3,
            24,
        ),
        (
            "ospfv2_lsack",
            Layer::new("ospfv2_lsack").with("lsa_headers", vec![header]),
            5,
            20,
        ),
    ];
    for (id, body, ty, body_len) in bodies {
        let bytes = build(vec![
            ipv4([192, 0, 2, 1], [192, 0, 2, 2], 2),
            Layer::new("ospfv2"),
            body,
        ]);
        let ospf = &bytes[20..];
        assert_eq!(ospf[1], ty, "{id}: type");
        assert_eq!(ospf.len(), 24 + body_len, "{id}: length");
        assert_eq!(
            usize::from(u16::from_be_bytes([ospf[2], ospf[3]])),
            ospf.len()
        );
        assert!(ospfv2_verifies(ospf), "{id}: OSPF checksum");
        assert_eq!(roundtrip(&bytes, "ipv4"), ["ipv4", "ospfv2", id]);
        assert_dissects(&bytes, Some(0x0800), "ospf");
    }
}

// --------------------------------------------------------------------------
// OSPFv2 LSU with a Router-LSA: ip / ospfv2 / ospfv2_lsu / ospfv2_lsa / body
// --------------------------------------------------------------------------

const OSPFV2_LSU_ROUTER: &str = "45c0 0060 0003 0000 01 59 167c c0000201 e0000005 \
    02 04 004c c0000201 00000000 38b0 0000 0000000000000000 \
    00000001 \
    0000 02 01 c0000201 c0000201 80000001 b1b3 0030 \
    02 00 0002 \
    c0000200 ffffff00 03 00 000a \
    c0000202 c0000201 01 00 0001";

#[test]
fn ospfv2_lsu_router_lsa_golden() {
    let link = |id: [u8; 4], data: [u8; 4], ty: u64, metric: u64| {
        st(&[
            ("link_id", v4(id)),
            ("link_data", v4(data)),
            ("type", FieldValue::UInt(ty)),
            ("num_tos", FieldValue::UInt(0)),
            ("metric", FieldValue::UInt(metric)),
        ])
    };
    let bytes = build(vec![
        ipv4([192, 0, 2, 1], [224, 0, 0, 5], 3).with("dscp", 48u8),
        Layer::new("ospfv2"),
        Layer::new("ospfv2_lsu"),
        Layer::new("ospfv2_lsa"),
        Layer::new("ospfv2_router_lsa")
            .with("flags", 0x02u8)
            .with("num_links", 2u16)
            .with(
                "links",
                vec![
                    link([192, 0, 2, 0], [255, 255, 255, 0], 3, 10),
                    link([192, 0, 2, 2], [192, 0, 2, 1], 1, 1),
                ],
            ),
    ]);
    assert_golden(&bytes, OSPFV2_LSU_ROUTER);
    let ospf = &bytes[20..];
    assert!(ospfv2_verifies(ospf), "OSPF checksum");
    let lsa = &ospf[28..];
    assert_eq!(
        usize::from(u16::from_be_bytes([lsa[18], lsa[19]])),
        lsa.len()
    );
    assert!(lsa_fletcher_verifies(lsa), "LSA Fletcher checksum");
    assert_eq!(
        roundtrip(&bytes, "ipv4"),
        [
            "ipv4",
            "ospfv2",
            "ospfv2_lsu",
            "ospfv2_lsa",
            "ospfv2_router_lsa"
        ]
    );
    assert_dissects(&bytes, Some(0x0800), "ospf");
}

#[test]
fn ospfv2_lsa_bodies_roundtrip() {
    let bodies = [
        (
            Layer::new("ospfv2_network_lsa").with(
                "attached_routers",
                vec![v4([192, 0, 2, 1]), v4([192, 0, 2, 2])],
            ),
            2u8,
            12usize,
        ),
        (Layer::new("ospfv2_summary_lsa"), 3, 8),
        (
            Layer::new("ospfv2_external_lsa").with("external_route_tag", 7u32),
            5,
            16,
        ),
    ];
    for (body, ls_type, len) in bodies {
        let id = body.protocol.clone();
        let bytes = build(vec![
            ipv4([192, 0, 2, 1], [224, 0, 0, 5], 4),
            Layer::new("ospfv2").with("area_id", Ipv4Addr::new(0, 0, 0, 1)),
            Layer::new("ospfv2_lsu"),
            Layer::new("ospfv2_lsa").with("link_state_id", Ipv4Addr::new(198, 51, 100, 0)),
            body,
        ]);
        let ospf = &bytes[20..];
        assert!(ospfv2_verifies(ospf), "{id}: OSPF checksum");
        let lsa = &ospf[28..];
        assert_eq!(lsa[3], ls_type, "{id}: LS type");
        assert_eq!(lsa.len(), 20 + len, "{id}: LSA length");
        assert!(lsa_fletcher_verifies(lsa), "{id}: LSA Fletcher checksum");
        assert_eq!(
            roundtrip(&bytes, "ipv4"),
            ["ipv4", "ospfv2", "ospfv2_lsu", "ospfv2_lsa", id.as_str()]
        );
    }
}

// --------------------------------------------------------------------------
// OSPFv3 Hello: ipv6 / ospfv3 / ospfv3_hello
// --------------------------------------------------------------------------

const OSPFV3_HELLO: &str = "6e000000 0028 59 01 \
    fe800000000000000000000000000001 ff020000000000000000000000000005 \
    03 01 0028 c0000201 00000000 b77b 00 00 \
    00000005 01 000013 000a 0028 c0000201 00000000 \
    c0000202";

#[test]
fn ospfv3_hello_golden() {
    let bytes = build(vec![
        ipv6(ll(1), Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 5)).with("dscp", 56u8),
        Layer::new("ospfv3"),
        Layer::new("ospfv3_hello")
            .with("interface_id", 5u32)
            .with("designated_router", Ipv4Addr::new(192, 0, 2, 1))
            .with("neighbors", vec![v4([192, 0, 2, 2])]),
    ]);
    assert_golden(&bytes, OSPFV3_HELLO);
    assert_eq!(bytes[6], 89, "next header 89 (OSPF)");
    assert!(
        ipv6_pseudo_verifies(&bytes[..40], 89, &bytes[40..]),
        "OSPFv3 checksum with IPv6 pseudo header"
    );
    assert_eq!(
        roundtrip(&bytes, "ipv6"),
        ["ipv6", "ospfv3", "ospfv3_hello"]
    );
    assert_dissects(&bytes, Some(0x86dd), "ospf");
}

#[test]
fn ospfv3_messages_and_lsas_roundtrip() {
    let header = st(&[
        ("ls_age", FieldValue::UInt(1)),
        ("ls_type", FieldValue::UInt(0x2001)),
        ("link_state_id", FieldValue::UInt(0)),
        ("advertising_router", v4([192, 0, 2, 1])),
        ("ls_sequence", FieldValue::UInt(0x8000_0001)),
        ("ls_checksum", FieldValue::UInt(0xabcd)),
        ("length", FieldValue::UInt(40)),
    ]);
    let request = st(&[
        ("reserved", FieldValue::UInt(0)),
        ("ls_type", FieldValue::UInt(0x2001)),
        ("link_state_id", FieldValue::UInt(0)),
        ("advertising_router", v4([192, 0, 2, 1])),
    ]);
    let simple = [
        Layer::new("ospfv3_dbd").with("lsa_headers", vec![header.clone()]),
        Layer::new("ospfv3_lsr").with("requests", vec![request]),
        Layer::new("ospfv3_lsack").with("lsa_headers", vec![header]),
    ];
    for body in simple {
        let id = body.protocol.clone();
        let bytes = build(vec![ipv6(ll(1), ll(2)), Layer::new("ospfv3"), body]);
        assert!(
            ipv6_pseudo_verifies(&bytes[..40], 89, &bytes[40..]),
            "{id}: checksum"
        );
        assert_eq!(roundtrip(&bytes, "ipv6"), ["ipv6", "ospfv3", id.as_str()]);
    }

    let lsas = [
        (Layer::new("ospfv3_router_lsa"), 0x2001u16),
        (
            Layer::new("ospfv3_network_lsa").with(
                "attached_routers",
                vec![v4([192, 0, 2, 1]), v4([192, 0, 2, 2])],
            ),
            0x2002,
        ),
        (Layer::new("ospfv3_inter_area_prefix_lsa"), 0x2003),
        (Layer::new("ospfv3_as_external_lsa"), 0x4005),
        (
            Layer::new("ospfv3_link_lsa")
                .with("num_prefixes", 1u32)
                .with("prefixes", hex("40 00 0000 20010db8 00010000")),
            0x0008,
        ),
        (Layer::new("ospfv3_intra_area_prefix_lsa"), 0x2009),
    ];
    for (body, ls_type) in lsas {
        let id = body.protocol.clone();
        let bytes = build(vec![
            ipv6(ll(1), Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 5)),
            Layer::new("ospfv3"),
            Layer::new("ospfv3_lsu"),
            Layer::new("ospfv3_lsa").with("link_state_id", 7u32),
            body,
        ]);
        assert!(
            ipv6_pseudo_verifies(&bytes[..40], 89, &bytes[40..]),
            "{id}: checksum"
        );
        let lsa = &bytes[40 + 16 + 4..];
        assert_eq!(u16::from_be_bytes([lsa[2], lsa[3]]), ls_type, "{id}");
        assert_eq!(
            usize::from(u16::from_be_bytes([lsa[18], lsa[19]])),
            lsa.len()
        );
        assert!(lsa_fletcher_verifies(lsa), "{id}: LSA Fletcher checksum");
        assert_eq!(
            roundtrip(&bytes, "ipv6"),
            ["ipv6", "ospfv3", "ospfv3_lsu", "ospfv3_lsa", id.as_str()]
        );
        assert_dissects(&bytes, Some(0x86dd), "ospf");
    }
}

// --------------------------------------------------------------------------
// BGP: ip / tcp(179) / bgp [/ body]
// --------------------------------------------------------------------------

fn bgp_stack(msg: Vec<Layer>) -> Vec<u8> {
    let mut layers = vec![
        ipv4([192, 0, 2, 1], [192, 0, 2, 2], 5).with("ttl", 64u8),
        Layer::new("tcp")
            .with("src_port", 49152u16)
            .with("dst_port", 179u16)
            .with("seq", 1u32)
            .with("ack", 1u32)
            .with("flags", 0x18u8),
    ];
    layers.extend(msg);
    build(layers)
}

/// Verifies the IPv4 header and the TCP checksum, and returns the TCP payload.
fn bgp_payload(bytes: &[u8]) -> &[u8] {
    assert!(verifies(&bytes[..20]), "IPv4 header");
    let mut ph = bytes[12..20].to_vec();
    ph.extend_from_slice(&[0, 6]);
    ph.extend_from_slice(&u16::try_from(bytes.len() - 20).unwrap().to_be_bytes());
    ph.extend_from_slice(&bytes[20..]);
    assert!(verifies(&ph), "TCP checksum with pseudo header");
    &bytes[40..]
}

const BGP_OPEN: &str = "ffffffffffffffffffffffffffffffff 002d 01 \
    04 fde9 00b4 c0000201 10 \
    02 0e 01 04 0001 00 01 02 00 41 04 0000fde9";

const BGP_UPDATE: &str = "ffffffffffffffffffffffffffffffff 002f 02 \
    0000 \
    0014 40 01 01 00 40 02 06 02 01 0000fde9 40 03 04 c0000201 \
    18 c63364";

#[test]
fn bgp_golden() {
    // OPEN with the default capabilities.
    let bytes = bgp_stack(vec![Layer::new("bgp"), Layer::new("bgp_open")]);
    assert_golden(bgp_payload(&bytes), BGP_OPEN);
    assert_eq!(
        roundtrip(&bytes, "ipv4"),
        ["ipv4", "tcp", "bgp", "bgp_open"]
    );
    assert_dissects(&bytes, Some(0x0800), "bgp");

    // UPDATE with the default attributes and NLRI.
    let bytes = bgp_stack(vec![Layer::new("bgp"), Layer::new("bgp_update")]);
    assert_golden(bgp_payload(&bytes), BGP_UPDATE);
    assert_eq!(
        roundtrip(&bytes, "ipv4"),
        ["ipv4", "tcp", "bgp", "bgp_update"]
    );
    assert_dissects(&bytes, Some(0x0800), "bgp");

    // KEEPALIVE: the bare 19-octet header (type 4).
    let bytes = bgp_stack(vec![Layer::new("bgp")]);
    assert_golden(
        bgp_payload(&bytes),
        "ffffffffffffffffffffffffffffffff 0013 04",
    );
    assert_eq!(roundtrip(&bytes, "ipv4"), ["ipv4", "tcp", "bgp"]);
    assert_dissects(&bytes, Some(0x0800), "bgp");

    // NOTIFICATION Cease / Administrative Shutdown, and ROUTE-REFRESH IPv4 unicast.
    let bytes = bgp_stack(vec![Layer::new("bgp"), Layer::new("bgp_notification")]);
    assert_golden(
        bgp_payload(&bytes),
        "ffffffffffffffffffffffffffffffff 0015 03 06 02",
    );
    assert_eq!(
        roundtrip(&bytes, "ipv4"),
        ["ipv4", "tcp", "bgp", "bgp_notification"]
    );
    let bytes = bgp_stack(vec![Layer::new("bgp"), Layer::new("bgp_route_refresh")]);
    assert_golden(
        bgp_payload(&bytes),
        "ffffffffffffffffffffffffffffffff 0017 05 0001 00 01",
    );
    assert_eq!(
        roundtrip(&bytes, "ipv4"),
        ["ipv4", "tcp", "bgp", "bgp_route_refresh"]
    );
}

#[test]
fn bgp_update_with_more_attributes_roundtrips() {
    let attrs = vec![
        TlvItem::named("origin").with("origin", 2u8),
        TlvItem::named("as_path").with(
            "segments",
            vec![st(&[
                ("segment_type", FieldValue::UInt(2)),
                ("segment_length", FieldValue::UInt(2)),
                (
                    "asns",
                    FieldValue::List(vec![
                        FieldValue::UInt(65001),
                        FieldValue::UInt(4_200_000_000),
                    ]),
                ),
            ])],
        ),
        TlvItem::named("next_hop").with("next_hop", Ipv4Addr::new(192, 0, 2, 1)),
        TlvItem::named("multi_exit_disc").with("med", 100u32),
        TlvItem::named("local_pref").with("local_pref", 200u32),
        TlvItem::named("atomic_aggregate"),
        TlvItem::named("aggregator")
            .with("asn", 65001u32)
            .with("address", Ipv4Addr::new(192, 0, 2, 1)),
        TlvItem::named("communities").with(
            "values",
            vec![FieldValue::UInt(0xfde9_0064), FieldValue::UInt(0xffff_ff01)],
        ),
        TlvItem::named("mp_reach_nlri")
            .with("afi", 2u16)
            .with("safi", 1u8)
            .with("next_hop_length", 16u8)
            .with(
                "next_hop",
                Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)
                    .octets()
                    .to_vec(),
            )
            .with("reserved", 0u8)
            .with("nlri", hex("40 20010db8 00010000")),
    ];
    // 4 + 13 + 7 + 7 + 7 + 3 + 11 + 11 + 33 = 96
    let bytes = bgp_stack(vec![
        Layer::new("bgp"),
        Layer::new("bgp_update")
            .with("withdrawn_routes_length", 4u16)
            .with("withdrawn_routes", hex("18 cb0071"))
            .with("total_path_attribute_length", 96u16)
            .with("path_attributes", attrs)
            .with("nlri", hex("18 c63364 10 c612")),
    ]);
    let bgp = bgp_payload(&bytes);
    assert_eq!(
        usize::from(u16::from_be_bytes([bgp[16], bgp[17]])),
        bgp.len()
    );
    assert_eq!(bgp.len(), 19 + 2 + 4 + 2 + 96 + 7);
    assert_eq!(
        roundtrip(&bytes, "ipv4"),
        ["ipv4", "tcp", "bgp", "bgp_update"]
    );
    assert_dissects(&bytes, Some(0x0800), "bgp");
}

// --------------------------------------------------------------------------
// RIP / RIPng: ip / udp(520|521) / rip|ripng
// --------------------------------------------------------------------------

const RIPV2: &str = "45000048 0006 0000 01 11 1795 c0000201 e0000009 \
    0208 0208 0034 ab20 \
    02 02 0000 \
    0002 0000 c0000200 ffffff00 00000000 00000001 \
    0002 0007 c6336400 ffffff00 c0000202 00000003";

#[test]
fn rip_golden() {
    let entry = |tag: u64, addr: [u8; 4], nh: [u8; 4], metric: u64| {
        st(&[
            ("afi", FieldValue::UInt(2)),
            ("route_tag", FieldValue::UInt(tag)),
            ("address", v4(addr)),
            ("subnet_mask", v4([255, 255, 255, 0])),
            ("next_hop", v4(nh)),
            ("metric", FieldValue::UInt(metric)),
        ])
    };
    let bytes = build(vec![
        ipv4([192, 0, 2, 1], [224, 0, 0, 9], 6),
        Layer::new("udp")
            .with("src_port", 520u16)
            .with("dst_port", 520u16),
        Layer::new("rip").with(
            "entries",
            vec![
                entry(0, [192, 0, 2, 0], [0, 0, 0, 0], 1),
                entry(7, [198, 51, 100, 0], [192, 0, 2, 2], 3),
            ],
        ),
    ]);
    assert_golden(&bytes, RIPV2);
    assert!(verifies(&bytes[..20]), "IPv4 header");
    assert_eq!(roundtrip(&bytes, "ipv4"), ["ipv4", "udp", "rip"]);
    assert_dissects(&bytes, Some(0x0800), "rip");

    // RIPv1 request for the whole table (AFI 0, metric 16).
    let bytes = build(vec![
        ipv4([192, 0, 2, 1], [255, 255, 255, 255], 7),
        Layer::new("udp")
            .with("src_port", 520u16)
            .with("dst_port", 520u16),
        Layer::new("rip")
            .with("command", 1u8)
            .with("version", 1u8)
            .with(
                "entries",
                vec![st(&[
                    ("afi", FieldValue::UInt(0)),
                    ("subnet_mask", v4([0, 0, 0, 0])),
                    ("metric", FieldValue::UInt(16)),
                ])],
            ),
    ]);
    assert_eq!(&bytes[28..32], &[1, 1, 0, 0]);
    assert_eq!(&bytes[48..52], &[0, 0, 0, 16]);
    assert_eq!(roundtrip(&bytes, "ipv4"), ["ipv4", "udp", "rip"]);
}

const RIPNG: &str = "60000000 0020 11 ff \
    fe800000000000000000000000000001 ff020000000000000000000000000009 \
    0209 0209 0020 8e52 \
    02 01 0000 \
    20010db8000100000000000000000000 0000 40 01";

#[test]
fn ripng_golden() {
    let bytes = build(vec![
        ipv6(ll(1), Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 9)).with("hop_limit", 255u8),
        Layer::new("udp")
            .with("src_port", 521u16)
            .with("dst_port", 521u16),
        Layer::new("ripng"),
    ]);
    assert_golden(&bytes, RIPNG);
    assert!(
        ipv6_pseudo_verifies(&bytes[..40], 17, &bytes[40..]),
        "UDP checksum"
    );
    assert_eq!(roundtrip(&bytes, "ipv6"), ["ipv6", "udp", "ripng"]);
    assert_dissects(&bytes, Some(0x86dd), "ripng");
}

// --------------------------------------------------------------------------
// EIGRP: ip / eigrp
// --------------------------------------------------------------------------

const EIGRP_HELLO: &str = "45c0003c 0008 0000 01 58 1697 c0000201 e000000a \
    02 05 ee68 00000000 00000000 00000000 0000 0064 \
    0001 000c 01 00 01 00 00 00 000f \
    0004 0008 0c 04 01 02";

#[test]
fn eigrp_golden() {
    let bytes = build(vec![
        ipv4([192, 0, 2, 1], [224, 0, 0, 10], 8).with("dscp", 48u8),
        Layer::new("eigrp"),
    ]);
    assert_golden(&bytes, EIGRP_HELLO);
    assert_eq!(bytes[9], 88, "IP protocol 88 (EIGRP)");
    assert!(verifies(&bytes[..20]), "IPv4 header");
    assert!(verifies(&bytes[20..]), "EIGRP checksum");
    assert_eq!(roundtrip(&bytes, "ipv4"), ["ipv4", "eigrp"]);
    assert_dissects(&bytes, Some(0x0800), "eigrp");
}

#[test]
fn eigrp_update_routes_roundtrip() {
    let metric = |t: TlvItem| {
        t.with("delay", 2560u32)
            .with("bandwidth", 256u32)
            .with("mtu", 1500u32)
            .with("hop_count", 0u8)
            .with("reliability", 255u8)
            .with("load", 1u8)
            .with("internal_tag", 0u8)
            .with("flags", 0u8)
    };
    let tlvs = vec![
        metric(TlvItem::named("ipv4_internal").with("next_hop", Ipv4Addr::UNSPECIFIED))
            .with("prefix_length", 24u8)
            .with("destination", vec![198, 51, 100]),
        metric(
            TlvItem::named("ipv4_external")
                .with("next_hop", Ipv4Addr::UNSPECIFIED)
                .with("originating_router", Ipv4Addr::new(192, 0, 2, 9))
                .with("originating_as", 0u32)
                .with("arbitrary_tag", 0u32)
                .with("external_metric", 0u32)
                .with("reserved", 0u16)
                .with("external_protocol", 6u8)
                .with("external_flags", 0u8),
        )
        .with("prefix_length", 16u8)
        .with("destination", vec![203, 0]),
    ];
    let bytes = build(vec![
        ipv4([192, 0, 2, 1], [192, 0, 2, 2], 9),
        Layer::new("eigrp")
            .with("opcode", 1u8)
            .with("flags", 0x08u32)
            .with("sequence", 5u32)
            .with("tlvs", tlvs),
    ]);
    assert!(verifies(&bytes[20..]), "EIGRP checksum");
    // TLV 0x0102: 4 + 4 + 16 + 1 + 3 = 28 octets.
    assert_eq!(&bytes[40..44], &[0x01, 0x02, 0x00, 28]);
    assert_eq!(roundtrip(&bytes, "ipv4"), ["ipv4", "eigrp"]);
    assert_dissects(&bytes, Some(0x0800), "eigrp");

    // EIGRP for IPv6 shares IP protocol 88 (checksum without pseudo header).
    let bytes = build(vec![
        ipv6(ll(1), Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 0xa)),
        Layer::new("eigrp"),
    ]);
    assert_eq!(bytes[6], 88);
    assert!(verifies(&bytes[40..]), "EIGRP checksum");
    assert_eq!(roundtrip(&bytes, "ipv6"), ["ipv6", "eigrp"]);
}

// --------------------------------------------------------------------------
// Proptests
// --------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn ospfv2_hello_neighbors_roundtrip(
        router_id in any::<[u8; 4]>(),
        neighbors in proptest::collection::vec(any::<[u8; 4]>(), 0..12),
        priority in any::<u8>(),
    ) {
        let bytes = Packet::from_layers(vec![
            ipv4([192, 0, 2, 1], [224, 0, 0, 5], 1),
            Layer::new("ospfv2").with("router_id", Ipv4Addr::from(router_id)),
            Layer::new("ospfv2_hello")
                .with("router_priority", priority)
                .with("neighbors", neighbors.iter().map(|n| v4(*n)).collect::<Vec<_>>()),
        ])
        .unpadded()
        .build(catalog())
        .unwrap()
        .bytes;
        prop_assert!(ospfv2_verifies(&bytes[20..]));
        let parsed = parse(&bytes, "ipv4", catalog()).unwrap();
        prop_assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        prop_assert_eq!(parsed.packet.layers.len(), 3);
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    }

    #[test]
    fn ospfv2_router_lsa_checksum_verifies(
        links in proptest::collection::vec((any::<[u8; 4]>(), any::<[u8; 4]>(), 1u64..=4, any::<u16>()), 0..8),
        seq in any::<u32>(),
    ) {
        let n = links.len();
        let links: Vec<FieldValue> = links
            .into_iter()
            .map(|(id, data, ty, metric)| st(&[
                ("link_id", v4(id)),
                ("link_data", v4(data)),
                ("type", FieldValue::UInt(ty)),
                ("num_tos", FieldValue::UInt(0)),
                ("metric", FieldValue::UInt(u64::from(metric))),
            ]))
            .collect();
        let bytes = Packet::from_layers(vec![
            ipv4([192, 0, 2, 1], [224, 0, 0, 5], 1),
            Layer::new("ospfv2"),
            Layer::new("ospfv2_lsu"),
            Layer::new("ospfv2_lsa").with("ls_sequence", seq),
            Layer::new("ospfv2_router_lsa")
                .with("num_links", u16::try_from(n).unwrap())
                .with("links", links),
        ])
        .unpadded()
        .build(catalog())
        .unwrap()
        .bytes;
        prop_assert!(ospfv2_verifies(&bytes[20..]));
        prop_assert!(lsa_fletcher_verifies(&bytes[48..]));
        let parsed = parse(&bytes, "ipv4", catalog()).unwrap();
        prop_assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    }

    #[test]
    fn rip_entries_roundtrip(
        entries in proptest::collection::vec(
            (any::<u16>(), any::<[u8; 4]>(), any::<[u8; 4]>(), 1u64..=16),
            0..10,
        ),
    ) {
        let entries: Vec<FieldValue> = entries
            .into_iter()
            .map(|(tag, addr, nh, metric)| st(&[
                ("afi", FieldValue::UInt(2)),
                ("route_tag", FieldValue::UInt(u64::from(tag))),
                ("address", v4(addr)),
                ("subnet_mask", v4([255, 255, 255, 0])),
                ("next_hop", v4(nh)),
                ("metric", FieldValue::UInt(metric)),
            ]))
            .collect();
        let bytes = Packet::from_layers(vec![
            ipv4([192, 0, 2, 1], [224, 0, 0, 9], 1),
            Layer::new("udp").with("src_port", 520u16).with("dst_port", 520u16),
            Layer::new("rip").with("entries", entries),
        ])
        .unpadded()
        .build(catalog())
        .unwrap()
        .bytes;
        let parsed = parse(&bytes, "ipv4", catalog()).unwrap();
        prop_assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        prop_assert_eq!(parsed.packet.layers[2].protocol.as_str(), "rip");
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    }

    #[test]
    fn bgp_communities_roundtrip(
        communities in proptest::collection::vec(any::<u32>(), 0..20),
        asns in proptest::collection::vec(any::<u32>(), 1..10),
    ) {
        let segment = st(&[
            ("segment_type", FieldValue::UInt(2)),
            ("segment_length", FieldValue::UInt(asns.len() as u64)),
            (
                "asns",
                FieldValue::List(asns.iter().map(|a| FieldValue::UInt(u64::from(*a))).collect()),
            ),
        ]);
        let attrs = vec![
            TlvItem::named("origin").with("origin", 0u8),
            TlvItem::named("as_path").with("segments", vec![segment]),
            TlvItem::named("next_hop").with("next_hop", Ipv4Addr::new(192, 0, 2, 1)),
            TlvItem::named("communities").with(
                "values",
                communities.iter().map(|c| FieldValue::UInt(u64::from(*c))).collect::<Vec<_>>(),
            ),
        ];
        let attr_len = 4 + (3 + 2 + 4 * asns.len()) + 7 + (3 + 4 * communities.len());
        let bytes = Packet::from_layers(vec![
            ipv4([192, 0, 2, 1], [192, 0, 2, 2], 1).with("ttl", 64u8),
            Layer::new("tcp").with("dst_port", 179u16),
            Layer::new("bgp"),
            Layer::new("bgp_update")
                .with("total_path_attribute_length", u16::try_from(attr_len).unwrap())
                .with("path_attributes", attrs),
        ])
        .unpadded()
        .build(catalog())
        .unwrap()
        .bytes;
        let parsed = parse(&bytes, "ipv4", catalog()).unwrap();
        prop_assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        prop_assert_eq!(parsed.packet.layers[3].protocol.as_str(), "bgp_update");
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    }
}

// --------------------------------------------------------------------------
// Checksum exclusions, Auto counts and source-port demux
// --------------------------------------------------------------------------

#[test]
fn ospfv2_checksums_skip_authentication_and_ls_age() {
    // Simple-password authentication: the packet checksum leaves the
    // authentication field out (RFC 2328 D.4.3), so it changes nothing.
    let hello = |auth: &[u8; 8]| {
        build(vec![
            ipv4([192, 0, 2, 1], [224, 0, 0, 5], 1),
            Layer::new("ospfv2")
                .with("au_type", 1u16)
                .with("authentication", auth.to_vec()),
            Layer::new("ospfv2_hello"),
        ])
    };
    let a = hello(b"secret12");
    let b = hello(&[0; 8]);
    assert!(ospfv2_verifies(&a[20..]), "OSPF checksum with a password");
    assert_eq!(a[32..34], b[32..34], "same checksum for any password");

    // An aged LSA: the Fletcher checksum excludes LS age.
    let bytes = build(vec![
        ipv4([192, 0, 2, 1], [224, 0, 0, 5], 3),
        Layer::new("ospfv2"),
        Layer::new("ospfv2_lsu"),
        Layer::new("ospfv2_lsa").with("ls_age", 1234u16),
        Layer::new("ospfv2_router_lsa"),
    ]);
    let lsa = &bytes[20 + 28..];
    assert_eq!(u16::from_be_bytes([lsa[0], lsa[1]]), 1234);
    assert!(
        lsa_fletcher_verifies(lsa),
        "LSA checksum with a non-zero age"
    );
}

#[test]
fn ospfv2_router_lsa_link_count_is_auto() {
    let link = |id: [u8; 4], metric: u64| {
        st(&[
            ("link_id", v4(id)),
            ("link_data", v4([255, 255, 255, 0])),
            ("type", FieldValue::UInt(3)),
            ("num_tos", FieldValue::UInt(0)),
            ("metric", FieldValue::UInt(metric)),
        ])
    };
    let bytes = build(vec![
        ipv4([192, 0, 2, 1], [224, 0, 0, 5], 3),
        Layer::new("ospfv2"),
        Layer::new("ospfv2_lsu"),
        Layer::new("ospfv2_lsa"),
        Layer::new("ospfv2_router_lsa").with(
            "links",
            vec![
                link([192, 0, 2, 0], 10),
                link([198, 51, 100, 0], 20),
                link([203, 0, 113, 0], 30),
            ],
        ),
    ]);
    let body = &bytes[20 + 28 + 20..];
    assert_eq!(u16::from_be_bytes([body[2], body[3]]), 3, "num_links");
    assert_eq!(body.len(), 4 + 3 * 12);
}

#[test]
fn bgp_decodes_from_source_port_179() {
    // A reply from the speaker on port 179 to the peer's ephemeral port.
    let bytes = build(vec![
        ipv4([192, 0, 2, 2], [192, 0, 2, 1], 6).with("ttl", 64u8),
        Layer::new("tcp")
            .with("src_port", 179u16)
            .with("dst_port", 49152u16)
            .with("flags", 0x18u8),
        Layer::new("bgp"),
    ]);
    assert_eq!(roundtrip(&bytes, "ipv4"), ["ipv4", "tcp", "bgp"]);
}
