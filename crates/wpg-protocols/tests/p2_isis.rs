//! IS-IS (M7): golden byte vectors, Fletcher checksum verification, Hello padding,
//! parse round-trips with typed TLVs / sub-TLVs and proptests.
//!
//! Expected bytes were derived field-by-field (ISO 10589 / RFC 5305 / RFC 5308);
//! the LSP checksum is re-verified with a naive ISO 8473 implementation over the
//! PDU from the LSP ID on. An optional `tshark` cross-check runs when
//! `WPG_TSHARK=1`.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, Ipv6Addr};

use proptest::prelude::*;
use wpg_core::diag::keys;
use wpg_core::{parse, FieldValue, Layer, Packet, TlvItem};
use wpg_protocols::catalog;

mod tshark_util;
use tshark_util::{assert_dissects, dissect, find};

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

/// ISO 8473 verification: both running sums over the LSP from the LSP ID
/// (offset 12) to the end are zero.
fn lsp_checksum_verifies(pdu: &[u8]) -> bool {
    let (mut c0, mut c1) = (0u32, 0u32);
    for b in &pdu[12..] {
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
            .collect(),
    )
}

/// 802.3 + LLC to an IS-IS multicast group (`last`: 0x14 = all L1 ISs, 0x15 =
/// all L2 ISs).
fn lan(last: u8, pdu: Layer) -> Vec<Layer> {
    vec![
        Layer::new("dot3")
            .with("dst", FieldValue::Mac([0x01, 0x80, 0xc2, 0, 0, last]))
            .with("src", FieldValue::Mac([0x02, 0, 0, 0, 0, 1])),
        Layer::new("llc"),
        pdu,
    ]
}

fn build(layers: Vec<Layer>) -> Vec<u8> {
    let built = Packet::from_layers(layers)
        .unpadded()
        .build(catalog())
        .unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    built.bytes
}

fn roundtrip(bytes: &[u8], first: &str) -> Packet {
    let parsed = parse(bytes, first, catalog()).unwrap();
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    parsed.packet
}

fn protos(p: &Packet) -> Vec<&str> {
    p.layers.iter().map(|l| l.protocol.as_str()).collect()
}

/// tshark's verdict on the LSP checksum (`isis.lsp.checksum.status`, 1 = good),
/// or `None` without `WPG_TSHARK`.
fn tshark_lsp_checksum_good(frame: &[u8]) -> Option<bool> {
    let layers = dissect(frame, None)?;
    let status = find(&layers, "isis.lsp.checksum.status")?;
    Some(status.as_str() == Some("1"))
}

// --------------------------------------------------------------------------
// Hellos
// --------------------------------------------------------------------------

const LAN_HELLO: &str = "831b0100 0f010000 03 000000000001 001e 002a 40 00000000000101 \
    8101cc 010403490001 8404c0000201";

#[test]
fn l1_lan_hello_golden() {
    let bytes = build(lan(0x14, Layer::new("isis_l1_lan_hello")));
    // 802.3 length = LLC (3) + PDU (42); DSAP / SSAP 0xfe announce the OSI PDU.
    assert_eq!(&bytes[12..17], &[0, 45, 0xfe, 0xfe, 0x03]);
    assert_golden(&bytes[17..], LAN_HELLO);
    let p = roundtrip(&bytes, "ethernet2");
    assert_eq!(protos(&p), ["dot3", "llc", "isis_l1_lan_hello"]);
    assert!(matches!(
        p.layers[2].fields["tlvs"],
        FieldValue::Tlv(ref items) if items.len() == 3 && items.iter().all(|i| i.data.is_none())
    ));
    assert_dissects(&bytes, None, "isis");
}

#[test]
fn hellos_are_padded_to_a_manual_pdu_length() {
    for (id, last) in [
        ("isis_l1_lan_hello", 0x14),
        ("isis_l2_lan_hello", 0x15),
        ("isis_p2p_hello", 0x14),
    ] {
        // Ethernet MTU 1500 minus the 3-octet LLC header.
        let bytes = build(lan(last, Layer::new(id).with("pdu_length", 1497u16)));
        assert_eq!(bytes.len(), 14 + 3 + 1497, "{id}");
        let pdu = &bytes[17..];
        // Walk the TLVs: the fixed header, the default TLVs, then Padding (8).
        let mut pos = usize::from(pdu[1]);
        let mut padding = 0;
        while pos < pdu.len() {
            if pdu[pos] == 8 {
                padding += 2 + usize::from(pdu[pos + 1]);
                assert!(pdu[pos + 2..pos + 2 + usize::from(pdu[pos + 1])]
                    .iter()
                    .all(|b| *b == 0));
            }
            pos += 2 + usize::from(pdu[pos + 1]);
        }
        assert_eq!(pos, 1497, "{id}: TLVs end at the PDU end");
        assert!(padding > 1400, "{id}");
        let p = roundtrip(&bytes, "ethernet2");
        assert_eq!(protos(&p)[2], id);
        assert_dissects(&bytes, None, "isis");
    }
}

#[test]
fn padding_counts_what_follows_and_warns_when_inexact() {
    // 43 octets cannot be reached from 42: no TLV is one octet long.
    let built = Packet::from_layers(lan(
        0x14,
        Layer::new("isis_l1_lan_hello").with("pdu_length", 43u16),
    ))
    .build(catalog())
    .unwrap();
    assert!(built
        .diagnostics
        .iter()
        .any(|d| d.key == keys::FILL_INEXACT));
    // 44: one empty Padding TLV.
    let bytes = build(lan(
        0x14,
        Layer::new("isis_l1_lan_hello").with("pdu_length", 44u16),
    ));
    assert_eq!(&bytes[17 + 42..], &[8, 0]);
}

const P2P_HELLO: &str = "83140100 11010000 03 000000000001 001e 0034 01 \
    8101cc 010403490001 8404c0000201 f00f02 00000001 000000000000 00000000";

#[test]
fn p2p_hello_golden_over_ppp() {
    let bytes = build(vec![Layer::new("ppp"), Layer::new("isis_p2p_hello")]);
    assert_eq!(&bytes[..2], &[0x00, 0x23], "PPP OSI network layer");
    assert_golden(&bytes[2..], P2P_HELLO);
    let p = roundtrip(&bytes, "ppp");
    assert_eq!(protos(&p), ["ppp", "isis_p2p_hello"]);
}

// --------------------------------------------------------------------------
// LSPs
// --------------------------------------------------------------------------

const L2_LSP: &str = "831b0100 14010000 0045 04af 0000000000010000 00000001 ff3c 03 \
    010403490001 8101cc 89027231 8404c0000201 \
    160b 00000000000200 00000a 00 \
    8708 0000000a 18 c00002";

#[test]
fn l2_lsp_golden_and_checksum() {
    let bytes = build(lan(0x15, Layer::new("isis_l2_lsp")));
    let pdu = &bytes[17..];
    assert_golden(pdu, L2_LSP);
    assert!(lsp_checksum_verifies(pdu));
    let p = roundtrip(&bytes, "ethernet2");
    assert_eq!(protos(&p), ["dot3", "llc", "isis_l2_lsp"]);
    assert_dissects(&bytes, None, "isis");
    if let Some(good) = tshark_lsp_checksum_good(&bytes) {
        assert!(good, "tshark rejects the LSP checksum");
    }
}

#[test]
fn lsp_checksum_ignores_lifetime_and_follows_the_content() {
    let lsp = |l: Layer| build(lan(0x14, l));
    let a = lsp(Layer::new("isis_l1_lsp"));
    let b = lsp(Layer::new("isis_l1_lsp").with("remaining_lifetime", 0u16));
    let c = lsp(Layer::new("isis_l1_lsp").with("sequence_number", 2u32));
    assert_eq!(
        a[17 + 24..17 + 26],
        b[17 + 24..17 + 26],
        "lifetime excluded"
    );
    assert_ne!(a[17 + 24..17 + 26], c[17 + 24..17 + 26]);
    for x in [&a, &b, &c] {
        assert!(lsp_checksum_verifies(&x[17..]));
    }
}

fn prefix(metric: u32, len: u8, prefix: &[u8], subs: Vec<TlvItem>) -> FieldValue {
    let mut m = BTreeMap::from([
        ("metric".to_owned(), FieldValue::UInt(metric.into())),
        ("prefix_length".to_owned(), FieldValue::UInt(len.into())),
        ("prefix".to_owned(), FieldValue::Bytes(prefix.to_vec())),
    ]);
    if !subs.is_empty() {
        m.insert(
            "sub_tlv_block".to_owned(),
            FieldValue::List(vec![st(&[("sub_tlvs", FieldValue::Tlv(subs))])]),
        );
    }
    FieldValue::Struct(m)
}

/// An L2 LSP exercising the main TLVs and sub-TLVs.
fn rich_lsp() -> Layer {
    let neighbor = st(&[
        ("neighbor_id", FieldValue::Bytes(vec![0, 0, 0, 0, 0, 2, 0])),
        ("metric", FieldValue::UInt(10)),
        (
            "sub_tlvs",
            FieldValue::Tlv(vec![
                TlvItem::named("ipv4_interface_address")
                    .with("address", Ipv4Addr::new(10, 0, 0, 1)),
                TlvItem::named("ipv4_neighbor_address").with("address", Ipv4Addr::new(10, 0, 0, 2)),
                TlvItem::named("te_default_metric").with("value", 10u32),
                TlvItem::named("adj_sid")
                    .with("flags", 0x30u8)
                    .with("sid", vec![0, 0x3a, 0x98]),
            ]),
        ),
    ]);
    let tlvs = vec![
        TlvItem::named("authentication")
            .with("auth_type", 1u8)
            .with("value", b"secret".to_vec()),
        TlvItem::named("area_addresses").with(
            "addresses",
            vec![
                st(&[("address", FieldValue::Nsap(vec![0x49, 0, 1]))]),
                st(&[("address", FieldValue::Nsap(vec![0x49, 0, 2]))]),
            ],
        ),
        TlvItem::named("protocols_supported").with(
            "nlpids",
            vec![FieldValue::UInt(0xcc), FieldValue::UInt(0x8e)],
        ),
        TlvItem::named("dynamic_hostname").with("hostname", b"core-1".to_vec()),
        TlvItem::named("te_router_id").with("router_id", Ipv4Addr::new(192, 0, 2, 1)),
        TlvItem::named("router_capability")
            .with("router_id", Ipv4Addr::new(192, 0, 2, 1))
            .with(
                "sub_tlvs",
                vec![
                    TlvItem::named("sr_capabilities")
                        .with("flags", 0xc0u8)
                        .with("ranges", hex("001f40 0103 00003e80")),
                    TlvItem::named("sr_algorithms")
                        .with("algorithms", vec![FieldValue::UInt(0), FieldValue::UInt(1)]),
                ],
            ),
        TlvItem::named("is_reachability").with(
            "neighbors",
            vec![st(&[(
                "neighbor_id",
                FieldValue::Bytes(vec![0, 0, 0, 0, 0, 3, 0]),
            )])],
        ),
        TlvItem::named("extended_is_reachability").with("neighbors", vec![neighbor]),
        TlvItem::named("ip_internal_reachability").with(
            "prefixes",
            vec![st(&[
                ("ip_address", FieldValue::Ipv4(Ipv4Addr::new(10, 1, 0, 0))),
                (
                    "subnet_mask",
                    FieldValue::Ipv4(Ipv4Addr::new(255, 255, 0, 0)),
                ),
            ])],
        ),
        TlvItem::named("extended_ip_reachability").with(
            "prefixes",
            vec![
                prefix(
                    10,
                    32,
                    &[192, 0, 2, 1],
                    vec![TlvItem::named("prefix_sid")
                        .with("flags", 0x40u8)
                        .with("sid", vec![0, 0, 0, 1])],
                ),
                prefix(20, 17, &[198, 51, 0x80], Vec::new()),
                prefix(
                    30,
                    0,
                    &[],
                    vec![TlvItem::named("admin_tag").with("tags", vec![FieldValue::UInt(100)])],
                ),
            ],
        ),
        TlvItem::named("ipv6_interface_addresses").with(
            "addresses",
            vec![FieldValue::Ipv6(Ipv6Addr::new(
                0x2001, 0xdb8, 0, 0, 0, 0, 0, 1,
            ))],
        ),
        TlvItem::named("ipv6_reachability").with(
            "prefixes",
            vec![prefix(
                10,
                64,
                &[0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0],
                Vec::new(),
            )],
        ),
    ];
    Layer::new("isis_l2_lsp")
        .with("overload", 1u8)
        .with("tlvs", tlvs)
}

#[test]
fn rich_lsp_roundtrips_with_typed_tlvs() {
    let bytes = build(lan(0x15, rich_lsp()));
    let pdu = &bytes[17..];
    assert!(lsp_checksum_verifies(pdu));
    assert_eq!(usize::from(u16::from_be_bytes([pdu[8], pdu[9]])), pdu.len());
    assert_eq!(pdu[26], 0x07, "OL + IS type L1/L2");
    let p = roundtrip(&bytes, "ethernet2");
    let FieldValue::Tlv(items) = &p.layers[2].fields["tlvs"] else {
        panic!("tlvs not typed");
    };
    let names: Vec<_> = items.iter().filter_map(|i| i.name.as_deref()).collect();
    assert_eq!(names.len(), 12, "{names:?}");
    // Every TLV, sub-TLV block and prefix decoded into fields, not octets.
    assert!(
        items.iter().all(|i| i.data.is_none() && i.raw.is_none()),
        "{items:?}"
    );
    let ext_ip = items
        .iter()
        .find(|i| i.name.as_deref() == Some("extended_ip_reachability"))
        .unwrap();
    let FieldValue::List(prefixes) = &ext_ip.fields["prefixes"] else {
        panic!("prefixes not typed");
    };
    let FieldValue::Struct(first) = &prefixes[0] else {
        panic!();
    };
    assert_eq!(first["sub_tlvs_present"], FieldValue::UInt(1));
    assert_eq!(first["prefix"], FieldValue::Bytes(vec![192, 0, 2, 1]));
    assert_dissects(&bytes, None, "isis");
    if let Some(good) = tshark_lsp_checksum_good(&bytes) {
        assert!(good, "tshark rejects the LSP checksum");
    }
}

// --------------------------------------------------------------------------
// SNPs, generic PDU, constraints
// --------------------------------------------------------------------------

const L2_CSNP: &str = "83210100 19010000 0033 00000000000100 0000000000000000 ffffffffffffffff \
    0910 04af 0000000000010000 00000001 0000";
const L1_PSNP: &str = "83110100 1a010000 0023 00000000000100 \
    0910 04af 0000000000010000 00000001 0000";

#[test]
fn snp_golden() {
    let csnp = build(lan(0x15, Layer::new("isis_l2_csnp")));
    assert_golden(&csnp[17..], L2_CSNP);
    assert_eq!(protos(&roundtrip(&csnp, "ethernet2"))[2], "isis_l2_csnp");
    assert_dissects(&csnp, None, "isis");
    let psnp = build(lan(0x14, Layer::new("isis_l1_psnp")));
    assert_golden(&psnp[17..], L1_PSNP);
    assert_eq!(protos(&roundtrip(&psnp, "ethernet2"))[2], "isis_l1_psnp");
    assert_dissects(&psnp, None, "isis");
}

#[test]
fn every_pdu_builds_and_parses_back_as_itself() {
    for (ty, id) in wpg_protocols::isis::PDUS {
        let bytes = build(lan(0x14, Layer::new(id)));
        assert_eq!(u64::from(bytes[17 + 4]), ty, "{id}");
        assert_eq!(protos(&roundtrip(&bytes, "ethernet2"))[2], id);
        assert_dissects(&bytes, None, "isis");
    }
}

#[test]
fn unknown_pdu_types_stay_generic() {
    let bytes = build(lan(
        0x14,
        Layer::new("isis")
            .with("pdu_type", 9u8)
            .with("data", vec![1u8, 2, 3]),
    ));
    let p = roundtrip(&bytes, "ethernet2");
    assert_eq!(protos(&p), ["dot3", "llc", "isis"]);
}

#[test]
fn a_wrong_header_length_is_warned() {
    let built = Packet::from_layers(lan(
        0x14,
        Layer::new("isis_l1_psnp").with("header_length", 27u8),
    ))
    .build(catalog())
    .unwrap();
    assert!(!built.diagnostics.is_empty());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn lsp_prefixes_roundtrip_and_checksum_verifies(
        entries in proptest::collection::vec((any::<u32>(), 0u8..=32, any::<u32>(), any::<bool>()), 0..8),
        seq in any::<u32>(),
    ) {
        let prefixes: Vec<FieldValue> = entries
            .iter()
            .map(|(metric, len, addr, tag)| {
                let n = usize::from(len.div_ceil(8));
                let subs = if *tag {
                    vec![TlvItem::named("admin_tag").with("tags", vec![FieldValue::UInt(7)])]
                } else {
                    Vec::new()
                };
                prefix(*metric, *len, &addr.to_be_bytes()[..n], subs)
            })
            .collect();
        let layer = Layer::new("isis_l1_lsp")
            .with("sequence_number", seq)
            .with("tlvs", vec![TlvItem::named("extended_ip_reachability").with("prefixes", prefixes)]);
        let bytes = build(lan(0x14, layer));
        prop_assert!(lsp_checksum_verifies(&bytes[17..]));
        let parsed = parse(&bytes, "ethernet2", catalog()).unwrap();
        prop_assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    }
}
