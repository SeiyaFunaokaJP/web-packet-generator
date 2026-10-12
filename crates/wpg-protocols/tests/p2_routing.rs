//! OSPFv2 Opaque LSAs with the TE / Router Information / Extended Prefix /
//! Extended Link bodies and their Segment Routing TLVs, and the BGP multiprotocol
//! NLRI (IPv4 / IPv6 unicast, VPN-IPv4 / VPN-IPv6, EVPN) (M7): golden byte
//! vectors, checksum verification, parse round-trips and proptests.
//!
//! Expected bytes were derived field-by-field (RFC 5250 / 3630 / 7770 / 7684 /
//! 8665, RFC 4760 / 4364 / 4659 / 7432 / 9136) and cross-checked with tshark 4.2
//! (every TLV, sub-TLV, prefix, route distinguisher, label and EVPN route decoded
//! as built, no expert warning); the OSPF and LSA checksums are re-verified with
//! naive implementations. An optional `tshark` cross-check runs when
//! `WPG_TSHARK=1`.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, Ipv6Addr};

use proptest::prelude::*;
use wpg_core::{parse, FieldValue, Layer, Packet, TlvItem};
use wpg_protocols::{bgp_mp, catalog};

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

/// OSPF packet checksum without the authentication field (RFC 2328 D.4.3).
fn ospfv2_verifies(pkt: &[u8]) -> bool {
    let mut v = pkt[..16].to_vec();
    v.extend_from_slice(&pkt[24..]);
    verifies(&v)
}

/// LSA Fletcher checksum (RFC 2328 §12.1.7) without the LS age: both running sums
/// are zero.
fn lsa_fletcher_verifies(lsa: &[u8]) -> bool {
    let (mut c0, mut c1) = (0u32, 0u32);
    for b in &lsa[2..] {
        c0 = (c0 + u32::from(*b)) % 255;
        c1 = (c1 + c0) % 255;
    }
    c0 == 0 && c1 == 0
}

fn st(pairs: &[(&str, FieldValue)]) -> FieldValue {
    FieldValue::Struct(
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect::<BTreeMap<_, _>>(),
    )
}

fn v4(o: [u8; 4]) -> FieldValue {
    FieldValue::Ipv4(Ipv4Addr::from(o))
}

fn build(layers: Vec<Layer>) -> Vec<u8> {
    let built = Packet::from_layers(layers)
        .unpadded()
        .build(catalog())
        .unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    built.bytes
}

/// Parses from IPv4, checks that the packet rebuilds identically and returns it.
fn roundtrip(bytes: &[u8]) -> Packet {
    let parsed = parse(bytes, "ipv4", catalog()).unwrap();
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

fn sub_items(item: &TlvItem, field: &str) -> Vec<TlvItem> {
    match &item.fields[field] {
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

// --------------------------------------------------------------------------
// OSPFv2 opaque LSAs: ipv4 / ospfv2 / ospfv2_lsu / ospfv2_opaque_lsa / body
// --------------------------------------------------------------------------

fn opaque_stack(lsa: Layer, body: Layer) -> Vec<u8> {
    build(vec![
        Layer::new("ipv4")
            .with("dst", Ipv4Addr::new(224, 0, 0, 5))
            .with("ttl", 1u8),
        Layer::new("ospfv2"),
        Layer::new("ospfv2_lsu"),
        lsa,
        body,
    ])
}

/// Checks the OSPF and LSA checksums and the lengths; returns the OSPF packet.
fn check_ospf(bytes: &[u8]) -> &[u8] {
    assert!(verifies(&bytes[..20]), "IPv4 header");
    let ospf = &bytes[20..];
    assert_eq!(
        usize::from(u16::from_be_bytes([ospf[2], ospf[3]])),
        ospf.len()
    );
    assert!(ospfv2_verifies(ospf), "OSPF checksum");
    let lsa = &ospf[28..];
    assert_eq!(
        usize::from(u16::from_be_bytes([lsa[18], lsa[19]])),
        lsa.len()
    );
    assert!(lsa_fletcher_verifies(lsa), "LSA checksum");
    ospf
}

/// Router Information: capabilities, SR algorithms 0 / 1, SRGB 16000 + 8000,
/// SRLB 15000 + 1000 (3-octet SID/Label sub-TLVs padded to 4 octets).
const RI: &str = "02 04 0060 c0000201 00000000 24b0 0000 0000000000000000 00000001 \
    0000 42 0a 04 000000 c0000201 80000001 4db4 0044 \
    0001 0004 00000000 \
    0008 0002 00 01 0000 \
    0009 000c 001f40 00 0001 0003 003e80 00 \
    000e 000c 0003e8 00 0001 0003 003a98 00";

/// Extended Prefix: 192.0.2.1/32 intra-area, N flag, Prefix SID index 1.
const EXT_PREFIX: &str = "02 04 0048 c0000201 00000000 7b71 0000 0000000000000000 00000001 \
    0000 42 0a 07 000000 c0000201 80000001 7184 002c \
    0001 0014 01 20 00 40 c0000201 \
    0002 0008 00 00 00 00 00000001";

/// Extended Link: point-to-point to 192.0.2.2, Adj-SID label 24000 (V and L).
const EXT_LINK: &str = "02 04 004c c0000201 00000000 5098 0000 0000000000000000 00000001 \
    0000 42 0a 08 000000 c0000201 80000001 511f 0030 \
    0001 0018 01 000000 c0000202 c6336401 \
    0002 0007 60 00 00 00 005dc0 00";

#[test]
fn opaque_lsa_bodies_golden() {
    for (body, want, opaque_type) in [
        ("ospfv2_ri_lsa", RI, 4),
        ("ospfv2_ext_prefix_lsa", EXT_PREFIX, 7),
        ("ospfv2_ext_link_lsa", EXT_LINK, 8),
    ] {
        let bytes = opaque_stack(Layer::new("ospfv2_opaque_lsa"), Layer::new(body));
        let ospf = check_ospf(&bytes);
        assert_golden(ospf, want);
        assert_eq!(ospf[28 + 3], 10, "{body}: area scope");
        assert_eq!(ospf[28 + 4], opaque_type, "{body}: opaque type is Auto");
        let p = roundtrip(&bytes);
        assert_eq!(
            protos(&p),
            ["ipv4", "ospfv2", "ospfv2_lsu", "ospfv2_opaque_lsa", body]
        );
        assert_dissects(&bytes, Some(0x0800), "ospf");
    }
}

#[test]
fn te_lsa_link_and_router_address() {
    let bytes = opaque_stack(
        Layer::new("ospfv2_opaque_lsa").with("opaque_id", 1u32),
        Layer::new("ospfv2_te_lsa"),
    );
    let ospf = check_ospf(&bytes);
    let lsa = &ospf[28..];
    assert_eq!(lsa[4..8], [1, 0, 0, 1], "opaque type 1, opaque ID 1");
    // Link TLV: 9 sub-TLVs, the 1-octet link type padded to 4 octets.
    assert_golden(
        &lsa[20..40],
        "0002 0064 0001 0001 01 000000 0002 0004 c0000202",
    );
    assert_golden(&lsa[64..72], "0006 0004 4b3ebc20");
    let p = roundtrip(&bytes);
    let tlvs = items(&p, 4, "tlvs");
    assert_eq!(
        names(&sub_items(&tlvs[0], "sub_tlvs")),
        [
            "link_type",
            "link_id",
            "local_address",
            "remote_address",
            "te_metric",
            "max_bandwidth",
            "max_reservable_bandwidth",
            "unreserved_bandwidth",
            "admin_group"
        ]
    );
    assert_dissects(&bytes, Some(0x0800), "ospf");

    // The Router Address TLV (TE LSA instance 0).
    let bytes = opaque_stack(
        Layer::new("ospfv2_opaque_lsa"),
        Layer::new("ospfv2_te_lsa").with(
            "tlvs",
            vec![TlvItem::named("router_address").with("address", v4([192, 0, 2, 1]))],
        ),
    );
    let ospf = check_ospf(&bytes);
    assert_golden(&ospf[48..], "0001 0004 c0000201");
    assert_eq!(protos(&roundtrip(&bytes))[4], "ospfv2_te_lsa");
}

#[test]
fn sid_forms_are_told_apart_by_length() {
    // Prefix SID as a label (L and V flags, 3 octets) and an Extended Prefix
    // Range; Adj-SID as an index and a LAN Adj-SID as a label.
    let prefix_sid_label = TlvItem::named("prefix_sid_label")
        .with("flags", 0x0cu8)
        .with("label", 16001u32);
    let range = TlvItem::named("extended_prefix_range")
        .with("prefix_length", 32u8)
        .with("range_size", 10u16)
        .with("prefix", Ipv4Addr::new(192, 0, 2, 100))
        .with(
            "sub_tlvs",
            vec![TlvItem::named("prefix_sid").with("index", 100u32)],
        );
    let ext = TlvItem::named("extended_prefix")
        .with("route_type", 3u8)
        .with("prefix_length", 24u8)
        .with("prefix", Ipv4Addr::new(198, 51, 100, 0))
        .with("sub_tlvs", vec![prefix_sid_label]);
    let bytes = opaque_stack(
        Layer::new("ospfv2_opaque_lsa").with("ls_type", 11u8),
        Layer::new("ospfv2_ext_prefix_lsa").with("tlvs", vec![ext, range]),
    );
    let ospf = check_ospf(&bytes);
    assert_golden(
        &ospf[48..],
        "0001 0014 03 18 00 00 c6336400 0002 0007 0c 00 00 00 003e81 00 \
         0002 0018 20 00 000a 00 000000 c0000264 0002 0008 00 00 00 00 00000064",
    );
    let p = roundtrip(&bytes);
    let tlvs = items(&p, 4, "tlvs");
    assert_eq!(names(&tlvs), ["extended_prefix", "extended_prefix_range"]);
    assert_eq!(
        names(&sub_items(&tlvs[0], "sub_tlvs")),
        ["prefix_sid_label"]
    );
    assert_eq!(names(&sub_items(&tlvs[1], "sub_tlvs")), ["prefix_sid"]);
    assert_dissects(&bytes, Some(0x0800), "ospf");

    let link = TlvItem::named("extended_link")
        .with("link_type", 2u8)
        .with("link_id", Ipv4Addr::new(198, 51, 100, 2))
        .with("link_data", Ipv4Addr::new(198, 51, 100, 1))
        .with(
            "sub_tlvs",
            vec![
                TlvItem::named("adj_sid_index")
                    .with("flags", 0x00u8)
                    .with("index", 7u32),
                TlvItem::named("lan_adj_sid")
                    .with("flags", 0x60u8)
                    .with("weight", 1u8)
                    .with("neighbor_id", Ipv4Addr::new(192, 0, 2, 3))
                    .with("label", 24001u32),
            ],
        );
    let bytes = opaque_stack(
        Layer::new("ospfv2_opaque_lsa"),
        Layer::new("ospfv2_ext_link_lsa").with("tlvs", vec![link]),
    );
    let ospf = check_ospf(&bytes);
    assert_golden(
        &ospf[48..],
        "0001 0028 02 000000 c6336402 c6336401 \
         0002 0008 00 00 00 00 00000007 \
         0003 000b 60 00 00 01 c0000203 005dc1 00",
    );
    let p = roundtrip(&bytes);
    let tlvs = items(&p, 4, "tlvs");
    assert_eq!(
        names(&sub_items(&tlvs[0], "sub_tlvs")),
        ["adj_sid_index", "lan_adj_sid"]
    );
    assert_dissects(&bytes, Some(0x0800), "ospf");
}

#[test]
fn ri_lsa_hostname_and_srms_preference() {
    let bytes = opaque_stack(
        Layer::new("ospfv2_opaque_lsa").with("ls_type", 9u8),
        Layer::new("ospfv2_ri_lsa").with(
            "tlvs",
            vec![
                TlvItem::named("informational_capabilities").with("capabilities", 0xc000_0000u32),
                TlvItem::named("hostname").with("hostname", b"r1".to_vec()),
                TlvItem::named("srms_preference").with("preference", 200u8),
            ],
        ),
    );
    let ospf = check_ospf(&bytes);
    assert_eq!(ospf[28 + 3], 9, "link-local scope");
    assert_golden(
        &ospf[48..],
        "0001 0004 c0000000 0007 0002 7231 0000 000f 0004 c8 000000",
    );
    let p = roundtrip(&bytes);
    assert_eq!(
        names(&items(&p, 4, "tlvs")),
        ["informational_capabilities", "hostname", "srms_preference"]
    );
}

#[test]
fn opaque_lsas_decode_from_the_plain_lsa_header() {
    // Any LS type 9-11 LSA is re-decoded as an opaque LSA; unknown opaque types
    // stay a bare opaque header followed by raw octets.
    let bytes = opaque_stack(
        Layer::new("ospfv2_opaque_lsa")
            .with("ls_type", 11u8)
            .with("opaque_type", 200u8),
        Layer::new("raw").with("data", vec![0xde, 0xad, 0xbe, 0xef]),
    );
    check_ospf(&bytes);
    let p = roundtrip(&bytes);
    assert_eq!(
        protos(&p),
        ["ipv4", "ospfv2", "ospfv2_lsu", "ospfv2_opaque_lsa", "raw"]
    );
    assert_eq!(p.layers[3].fields["opaque_type"], FieldValue::UInt(200));
}

// --------------------------------------------------------------------------
// BGP: ip / tcp(179) / bgp / bgp_update
// --------------------------------------------------------------------------

fn bgp_stack(update: Layer) -> Vec<u8> {
    build(vec![
        Layer::new("ipv4")
            .with("src", Ipv4Addr::new(192, 0, 2, 1))
            .with("dst", Ipv4Addr::new(192, 0, 2, 2)),
        Layer::new("tcp")
            .with("src_port", 49152u16)
            .with("dst_port", 179u16)
            .with("flags", 0x18u8),
        Layer::new("bgp"),
        update,
    ])
}

/// Verifies the TCP checksum and the BGP length; returns the BGP message.
fn bgp_payload(bytes: &[u8]) -> &[u8] {
    let mut ph = bytes[12..20].to_vec();
    ph.extend_from_slice(&[0, 6]);
    ph.extend_from_slice(&u16::try_from(bytes.len() - 20).unwrap().to_be_bytes());
    ph.extend_from_slice(&bytes[20..]);
    assert!(verifies(&ph), "TCP checksum");
    let bgp = &bytes[40..];
    assert_eq!(
        usize::from(u16::from_be_bytes([bgp[16], bgp[17]])),
        bgp.len()
    );
    bgp
}

/// An UPDATE with ORIGIN IGP, an empty `AS_PATH` (iBGP) and `mp`, no IPv4 NLRI.
fn mp_update(mp: Vec<TlvItem>) -> Layer {
    let mut attrs = vec![
        TlvItem::named("origin").with("origin", 0u8),
        TlvItem::named("as_path").with("segments", Vec::<FieldValue>::new()),
    ];
    attrs.extend(mp);
    Layer::new("bgp_update")
        .with("path_attributes", attrs)
        .with("nlri", bgp_mp::prefixes(&[]))
}

fn rd_next_hop(addr: FieldValue) -> FieldValue {
    FieldValue::List(vec![st(&[
        ("rd", FieldValue::Bytes(vec![0; 8])),
        ("address", addr),
    ])])
}

/// The attribute names of the parsed UPDATE (layer 3).
fn attr_names(p: &Packet) -> Vec<String> {
    items(p, 3, "path_attributes")
        .iter()
        .map(|i| i.name.clone().unwrap_or_default())
        .collect()
}

const MARKER: &str = "ffffffffffffffffffffffffffffffff";

#[test]
fn update_lengths_and_prefixes_are_typed() {
    // Withdrawn routes and NLRI as prefix lists, both lengths Auto.
    let bytes = bgp_stack(
        Layer::new("bgp_update")
            .with(
                "withdrawn_routes",
                bgp_mp::prefixes(&[(24, &[203, 0, 113]), (0, &[])]),
            )
            .with(
                "nlri",
                bgp_mp::prefixes(&[(24, &[198, 51, 100]), (31, &[198, 18, 0, 0])]),
            ),
    );
    assert_golden(
        bgp_payload(&bytes),
        &format!(
            "{MARKER} 0039 02 0005 18 cb0071 00 \
             0014 40 01 01 00 40 02 06 02 01 0000fde9 40 03 04 c0000201 \
             18 c63364 1f c6120000"
        ),
    );
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ipv4", "tcp", "bgp", "bgp_update"]);
    let FieldValue::List(nlri) = &p.layers[3].fields["nlri"] else {
        panic!("nlri not decoded as a list");
    };
    assert_eq!(nlri.len(), 2);
    assert_dissects(&bytes, Some(0x0800), "bgp");

    // Verbatim octets (the former representation) still build.
    let old = bgp_stack(
        Layer::new("bgp_update")
            .with("withdrawn_routes", hex("18 cb0071 00"))
            .with("nlri", hex("18 c63364 1f c6120000")),
    );
    assert_eq!(old, bytes);
}

#[test]
fn mp_reach_unicast_families() {
    let v4 = TlvItem::named("mp_reach_ipv4_unicast")
        .with("next_hop", vec![v4([192, 0, 2, 1])])
        .with(
            "nlri",
            bgp_mp::prefixes(&[(24, &[198, 51, 100]), (32, &[203, 0, 113, 1])]),
        );
    let bytes = bgp_stack(mp_update(vec![v4]));
    assert_golden(
        bgp_payload(&bytes),
        &format!(
            "{MARKER} 0033 02 0000 001c 40 01 01 00 40 02 00 \
             80 0e 12 0001 01 04 c0000201 00 18 c63364 20 cb007101"
        ),
    );
    let p = roundtrip(&bytes);
    assert_eq!(
        attr_names(&p),
        ["origin", "as_path", "mp_reach_ipv4_unicast"]
    );
    assert_dissects(&bytes, Some(0x0800), "bgp");

    // IPv6: global + link-local next hop (32 octets), and its withdrawal.
    let nh = vec![
        FieldValue::Ipv6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
        FieldValue::Ipv6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1)),
    ];
    let v6 = TlvItem::named("mp_reach_ipv6_unicast")
        .with("next_hop", nh)
        .with(
            "nlri",
            bgp_mp::prefixes(&[(48, &[0x20, 0x01, 0x0d, 0xb8, 0x00, 0x01])]),
        );
    let withdraw = TlvItem::named("mp_unreach_ipv6_unicast").with(
        "withdrawn",
        bgp_mp::prefixes(&[(32, &[0x20, 0x01, 0x0d, 0xb8])]),
    );
    let bytes = bgp_stack(mp_update(vec![v6, withdraw]));
    let bgp = bgp_payload(&bytes);
    assert_golden(
        &bgp[30..],
        "80 0e 2c 0002 01 20 20010db8000000000000000000000001 fe800000000000000000000000000001 \
         00 30 20010db80001 \
         80 0f 08 0002 01 20 20010db8",
    );
    let p = roundtrip(&bytes);
    assert_eq!(
        attr_names(&p),
        [
            "origin",
            "as_path",
            "mp_reach_ipv6_unicast",
            "mp_unreach_ipv6_unicast"
        ]
    );
    assert_dissects(&bytes, Some(0x0800), "bgp");
}

#[test]
fn mp_reach_vpn_families() {
    let vpnv4 = TlvItem::named("mp_reach_vpnv4")
        .with("next_hop", rd_next_hop(v4([192, 0, 2, 1])))
        .with(
            "nlri",
            FieldValue::List(vec![bgp_mp::vpn_prefix(24, 16, &[198, 51, 100])]),
        );
    let bytes = bgp_stack(mp_update(vec![vpnv4]));
    assert_golden(
        bgp_payload(&bytes),
        &format!(
            "{MARKER} 0041 02 0000 002a 40 01 01 00 40 02 00 \
             80 0e 20 0001 80 0c 0000000000000000 c0000201 00 \
             70 000101 0000fde900000064 c63364"
        ),
    );
    let p = roundtrip(&bytes);
    assert_eq!(attr_names(&p), ["origin", "as_path", "mp_reach_vpnv4"]);
    assert_dissects(&bytes, Some(0x0800), "bgp");

    // VPN-IPv6 and a VPN-IPv4 withdrawal with the 0x800000 compatibility label
    // (RFC 8277 §2.4).
    let vpnv6 = TlvItem::named("mp_reach_vpnv6")
        .with(
            "next_hop",
            rd_next_hop(FieldValue::Ipv6(Ipv6Addr::new(
                0x2001, 0xdb8, 0, 0, 0, 0, 0, 1,
            ))),
        )
        .with(
            "nlri",
            FieldValue::List(vec![bgp_mp::vpn_prefix(
                48,
                17,
                &[0x20, 0x01, 0x0d, 0xb8, 0x00, 0x01],
            )]),
        );
    let mut withdrawn = bgp_mp::vpn_prefix(24, 0x80000, &[198, 51, 100]);
    if let FieldValue::Struct(m) = &mut withdrawn {
        m.insert("bos".into(), FieldValue::UInt(0));
    }
    let unreach =
        TlvItem::named("mp_unreach_vpnv4").with("withdrawn", FieldValue::List(vec![withdrawn]));
    let bytes = bgp_stack(mp_update(vec![vpnv6, unreach]));
    let bgp = bgp_payload(&bytes);
    assert_golden(
        &bgp[30..],
        "80 0e 2f 0002 80 18 0000000000000000 20010db8000000000000000000000001 00 \
         88 000111 0000fde900000064 20010db80001 \
         80 0f 12 0001 80 70 800000 0000fde900000064 c63364",
    );
    let p = roundtrip(&bytes);
    assert_eq!(
        attr_names(&p),
        ["origin", "as_path", "mp_reach_vpnv6", "mp_unreach_vpnv4"]
    );
    assert_dissects(&bytes, Some(0x0800), "bgp");
}

/// EVPN: a MAC/IP route (VNI 10010) and an Inclusive Multicast route with a
/// route target and a PMSI Tunnel (ingress replication).
fn evpn_update() -> Layer {
    let imet = TlvItem::named("inclusive_multicast")
        .with("rd", bgp_mp::RD.to_vec())
        .with("ethernet_tag", 0u32)
        .with("ip_length", 32u8)
        .with("originating_ip", vec![192, 0, 2, 1]);
    let reach = TlvItem::named("mp_reach_evpn")
        .with("next_hop", vec![192, 0, 2, 1])
        .with(
            "nlri",
            vec![
                bgp_mp::evpn_mac_ip([0x02, 0, 0, 0, 0, 0x10], &[192, 0, 2, 10], 10010),
                imet,
            ],
        );
    let rt = TlvItem::named("extended_communities")
        .with("values", vec![FieldValue::UInt(0x0002_fde9_0000_0064)]);
    let pmsi = TlvItem::named("pmsi_tunnel")
        .with("flags", 0u8)
        .with("tunnel_type", 6u8)
        .with("mpls_label", 10010u32)
        .with("tunnel_id", vec![192, 0, 2, 1]);
    mp_update(vec![reach, rt, pmsi])
}

const EVPN_REACH: &str = "80 0e 43 0019 46 04 c0000201 00 \
    02 25 0000fde900000064 00000000000000000000 00000000 30 020000000010 20 c000020a 00271a \
    03 11 0000fde900000064 00000000 20 c0000201";

#[test]
fn mp_reach_evpn_routes() {
    let bytes = bgp_stack(evpn_update());
    let bgp = bgp_payload(&bytes);
    assert_golden(
        &bgp[30..],
        &format!(
            "{EVPN_REACH} c0 10 08 0002fde900000064 \
             c0 16 09 00 06 00271a c0000201"
        ),
    );
    let p = roundtrip(&bytes);
    assert_eq!(
        attr_names(&p),
        [
            "origin",
            "as_path",
            "mp_reach_evpn",
            "extended_communities",
            "pmsi_tunnel"
        ]
    );
    let reach = &items(&p, 3, "path_attributes")[2];
    assert_eq!(
        names(&sub_items(reach, "nlri")),
        ["mac_ip", "inclusive_multicast"]
    );
    assert_dissects(&bytes, Some(0x0800), "bgp");
}

#[test]
fn evpn_ethernet_segment_and_ip_prefix_routes() {
    let esi = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    let head = |name: &str| {
        TlvItem::named(name)
            .with("rd", bgp_mp::RD.to_vec())
            .with("esi", vec![0u8; 10])
            .with("ethernet_tag", 0u32)
    };
    let routes = vec![
        head("ethernet_ad")
            .with("esi", esi.clone())
            .with("ethernet_tag", 0xffff_ffffu32)
            .with("mpls_label", 0u32),
        TlvItem::named("ethernet_segment")
            .with("rd", bgp_mp::RD.to_vec())
            .with("esi", esi)
            .with("ip_length", 32u8)
            .with("originating_ip", vec![192, 0, 2, 1]),
        head("ip_prefix")
            .with("ip_prefix_length", 24u8)
            .with("ip_prefix", Ipv4Addr::new(198, 51, 100, 0))
            .with("gateway", Ipv4Addr::UNSPECIFIED)
            .with("mpls_label", 0x641u32), // label 100, bottom of stack
        head("ipv6_prefix")
            .with("ip_prefix_length", 64u8)
            .with("ip_prefix", Ipv6Addr::new(0x2001, 0xdb8, 1, 0, 0, 0, 0, 0))
            .with("gateway", Ipv6Addr::UNSPECIFIED)
            .with("mpls_label", 0x641u32),
    ];
    let reach = TlvItem::named("mp_reach_evpn")
        .with("next_hop", vec![192, 0, 2, 1])
        .with("nlri", routes);
    let bytes = bgp_stack(mp_update(vec![reach]));
    let bgp = bgp_payload(&bytes);
    // 9 + 27 + 25 + 36 + 60 octets of value.
    assert_golden(&bgp[30..33], "80 0e 9d");
    let p = roundtrip(&bytes);
    let reach = &items(&p, 3, "path_attributes")[2];
    assert_eq!(
        names(&sub_items(reach, "nlri")),
        [
            "ethernet_ad",
            "ethernet_segment",
            "ip_prefix",
            "ipv6_prefix"
        ]
    );
    assert_dissects(&bytes, Some(0x0800), "bgp");
}

#[test]
fn long_attributes_get_the_extended_length_flag() {
    // 70 /24 prefixes: 3 + 1 + 4 + 1 + 280 = 289 octets, too long for 8 bits.
    let list: Vec<(u8, Vec<u8>)> = (0..70u8).map(|i| (24, vec![10, 0, i])).collect();
    let refs: Vec<(u8, &[u8])> = list.iter().map(|(l, p)| (*l, p.as_slice())).collect();
    let reach = TlvItem::named("mp_reach_ipv4_unicast")
        .with("next_hop", vec![v4([192, 0, 2, 1])])
        .with("nlri", bgp_mp::prefixes(&refs));
    let bytes = bgp_stack(mp_update(vec![reach]));
    let bgp = bgp_payload(&bytes);
    assert_golden(&bgp[21..23], "012c"); // 4 + 3 + 4 + 289
    assert_golden(&bgp[30..38], "90 0e 0121 0001 01 04");
    let p = roundtrip(&bytes);
    let attrs = items(&p, 3, "path_attributes");
    assert_eq!(attrs[2].name.as_deref(), Some("mp_reach_ipv4_unicast"));
    assert_eq!(attrs[2].ty, Some(0x900e));
    assert_dissects(&bytes, Some(0x0800), "bgp");

    // An explicit Extended Length type on a short attribute is kept (FRR and
    // GoBGP send MP_REACH_NLRI that way).
    let short = TlvItem {
        ty: Some(0x900e),
        ..TlvItem::named("mp_reach_ipv4_unicast")
            .with("next_hop", vec![v4([192, 0, 2, 1])])
            .with("nlri", bgp_mp::prefixes(&[(24, &[198, 51, 100])]))
    };
    let bytes = bgp_stack(mp_update(vec![short]));
    let bgp = bgp_payload(&bytes);
    assert_golden(&bgp[30..], "90 0e 000d 0001 01 04 c0000201 00 18 c63364");
    let p = roundtrip(&bytes);
    assert_eq!(items(&p, 3, "path_attributes")[2].ty, Some(0x900e));
    assert_dissects(&bytes, Some(0x0800), "bgp");
}

#[test]
fn other_families_use_the_generic_mp_attributes() {
    // IPv4 multicast (SAFI 2) is not modelled: next hop and NLRI stay octets.
    let reach = TlvItem::named("mp_reach_nlri")
        .with("afi", 1u16)
        .with("safi", 2u8)
        .with("next_hop", vec![192, 0, 2, 1])
        .with("nlri", hex("18 e80001"));
    let bytes = bgp_stack(mp_update(vec![reach]));
    let p = roundtrip(&bytes);
    assert_eq!(attr_names(&p), ["origin", "as_path", "mp_reach_nlri"]);
}

// --------------------------------------------------------------------------
// Proptests
// --------------------------------------------------------------------------

fn prefix_strategy(max_bits: u8) -> impl Strategy<Value = (u8, Vec<u8>)> {
    (0..=max_bits).prop_flat_map(|len| {
        let n = usize::from(len).div_ceil(8);
        (Just(len), proptest::collection::vec(any::<u8>(), n))
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn unicast_and_vpn_prefixes_roundtrip(
        v4s in proptest::collection::vec(prefix_strategy(32), 0..10),
        vpns in proptest::collection::vec((prefix_strategy(32), 0u32..(1 << 20)), 0..6),
    ) {
        let refs: Vec<(u8, &[u8])> = v4s.iter().map(|(l, p)| (*l, p.as_slice())).collect();
        let vpn: Vec<FieldValue> = vpns
            .iter()
            .map(|((l, p), label)| bgp_mp::vpn_prefix(*l, *label, p))
            .collect();
        let reach = TlvItem::named("mp_reach_vpnv4")
            .with("next_hop", rd_next_hop(v4([192, 0, 2, 1])))
            .with("nlri", FieldValue::List(vpn));
        let bytes = bgp_stack(mp_update(vec![reach]).with("nlri", bgp_mp::prefixes(&refs)));
        let parsed = parse(&bytes, "ipv4", catalog()).unwrap();
        prop_assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
        let attrs = items(&parsed.packet, 3, "path_attributes");
        prop_assert_eq!(attrs[2].name.as_deref(), Some("mp_reach_vpnv4"));
    }

    #[test]
    fn prefix_sids_roundtrip(
        index in any::<u32>(),
        label in 0u32..(1 << 20),
        flags in any::<u8>(),
        algorithm in any::<u8>(),
    ) {
        let ext = TlvItem::named("extended_prefix")
            .with("prefix_length", 32u8)
            .with("prefix", Ipv4Addr::new(192, 0, 2, 1))
            .with("sub_tlvs", vec![
                TlvItem::named("prefix_sid")
                    .with("flags", flags)
                    .with("algorithm", algorithm)
                    .with("index", index),
                TlvItem::named("prefix_sid_label").with("label", label),
            ]);
        let bytes = opaque_stack(
            Layer::new("ospfv2_opaque_lsa"),
            Layer::new("ospfv2_ext_prefix_lsa").with("tlvs", vec![ext]),
        );
        prop_assert!(lsa_fletcher_verifies(&bytes[48..]));
        let parsed = parse(&bytes, "ipv4", catalog()).unwrap();
        prop_assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes.clone());
        let tlvs = items(&parsed.packet, 4, "tlvs");
        let subs = sub_items(&tlvs[0], "sub_tlvs");
        prop_assert_eq!(names(&subs), ["prefix_sid", "prefix_sid_label"]);
    }
}
