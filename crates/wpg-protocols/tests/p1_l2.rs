//! Golden byte vectors, parse round-trips and a TLV proptest for the P1 L2 /
//! control protocols (LLDP, CDP, STP, LACP, EAPOL, `PPPoE`, PPP).
//!
//! Expected bytes were derived independently (field-by-field, re-checked in the
//! assertions below); where a golden pcap is rendered, an optional `tshark`
//! cross-check runs when `WPG_TSHARK=1`.

use std::collections::BTreeMap;

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

fn protos(p: &Packet) -> Vec<&str> {
    p.layers.iter().map(|l| l.protocol.as_str()).collect()
}

fn mac(m: [u8; 6]) -> FieldValue {
    FieldValue::Mac(m)
}

fn lldp_tlv(ty: u64, value: Vec<u8>) -> FieldValue {
    FieldValue::Struct(BTreeMap::from([
        ("type".to_owned(), FieldValue::UInt(ty)),
        ("length".to_owned(), FieldValue::UInt(value.len() as u64)),
        ("value".to_owned(), FieldValue::Bytes(value)),
    ]))
}

/// Round-trips a frame: parse → rebuild reproduces the input byte for byte, with
/// no error diagnostics, and returns the parsed layer names.
fn roundtrip(bytes: &[u8]) -> Vec<String> {
    let parsed = parse(bytes, "ethernet2", catalog()).unwrap();
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
// LLDP
// --------------------------------------------------------------------------

const LLDP: &str = "0180c200000e 001122334455 88cc \
    0207 04 001122334455 \
    0407 03 001122334455 \
    0602 0078 \
    0000";

#[test]
fn lldp_golden() {
    let m = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
    let mut chassis = vec![0x04];
    chassis.extend_from_slice(&m);
    let mut port = vec![0x03];
    port.extend_from_slice(&m);
    let p = Packet::from_layers(vec![
        Layer::new("ethernet2")
            .with("dst", mac([0x01, 0x80, 0xc2, 0x00, 0x00, 0x0e]))
            .with("src", mac(m))
            .with("ethertype", 0x88ccu16),
        Layer::new("lldp").with(
            "tlvs",
            FieldValue::List(vec![
                lldp_tlv(1, chassis),
                lldp_tlv(2, port),
                lldp_tlv(3, vec![0x00, 0x78]),
                lldp_tlv(0, Vec::new()),
            ]),
        ),
    ])
    .unpadded();
    let built = p.build(catalog()).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    assert_eq!(built.bytes, hex(LLDP));

    // Independent decode of the 7-bit type / 9-bit length TLV headers.
    let tlvs = &built.bytes[14..];
    let mut pos = 0;
    let mut framed = Vec::new();
    while pos < tlvs.len() {
        let hdr = u16::from_be_bytes([tlvs[pos], tlvs[pos + 1]]);
        let ty = hdr >> 9;
        let len = (hdr & 0x1ff) as usize;
        framed.push((ty, len));
        pos += 2 + len;
    }
    assert_eq!(framed, [(1, 7), (2, 7), (3, 2), (0, 0)]);
    assert_eq!(pos, tlvs.len(), "TLVs tile the PDU exactly");

    assert_eq!(roundtrip(&built.bytes), ["ethernet2", "lldp"]);

    assert_dissects(&built.bytes, None, "lldp");
}

// --------------------------------------------------------------------------
// STP configuration BPDU
// --------------------------------------------------------------------------

const STP: &str = "0180c2000000 001122334455 0026 \
    424203 \
    0000 00 00 00 \
    8000 001122334455 00000000 \
    8000 001122334455 8001 \
    0000 1400 0200 0f00";

#[test]
fn stp_config_bpdu_golden() {
    let bridge = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
    let p = Packet::from_layers(vec![
        Layer::new("dot3")
            .with("dst", mac([0x01, 0x80, 0xc2, 0x00, 0x00, 0x00]))
            .with("src", mac(bridge)),
        Layer::new("llc")
            .with("dsap", 0x42u8)
            .with("ssap", 0x42u8)
            .with("control", 0x03u8),
        Layer::new("stp")
            .with("root_mac", mac(bridge))
            .with("bridge_mac", mac(bridge)),
    ])
    .unpadded();
    let built = p.build(catalog()).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    assert_eq!(built.bytes, hex(STP));

    // dot3 length announces LLC (3) + config BPDU (35) = 38 octets.
    assert_eq!(u16::from_be_bytes([built.bytes[12], built.bytes[13]]), 38);
    // The BPDU itself is exactly 35 octets (offset 17..52).
    assert_eq!(built.bytes.len(), 52);

    assert_eq!(roundtrip(&built.bytes), ["dot3", "llc", "stp"]);

    assert_dissects(&built.bytes, None, "stp");
}

// --------------------------------------------------------------------------
// LACP
// --------------------------------------------------------------------------

#[test]
fn lacp_golden() {
    let actor = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
    let p = Packet::from_layers(vec![
        Layer::new("ethernet2")
            .with("dst", mac([0x01, 0x80, 0xc2, 0x00, 0x00, 0x02]))
            .with("src", mac(actor))
            .with("ethertype", 0x8809u16),
        Layer::new("lacp")
            .with("actor_system", mac(actor))
            .with("actor_key", 5u16)
            .with("partner_system", mac([0; 6])),
    ])
    .unpadded();
    let built = p.build(catalog()).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    // 14 (Ethernet) + 110 (LACPDU) octets.
    assert_eq!(built.bytes.len(), 124);
    let l = &built.bytes[14..];
    assert_eq!((l[0], l[1]), (1, 1), "subtype 1, version 1");
    assert_eq!((l[2], l[3]), (1, 20), "actor TLV type 1, length 20");
    assert_eq!(&l[4..6], &[0x80, 0x00], "actor system priority");
    assert_eq!(&l[6..12], &actor, "actor system MAC");
    assert_eq!(u16::from_be_bytes([l[12], l[13]]), 5, "actor key");
    assert_eq!((l[22], l[23]), (2, 20), "partner TLV type 2, length 20");
    assert_eq!((l[42], l[43]), (3, 16), "collector TLV type 3, length 16");
    assert_eq!((l[58], l[59]), (0, 0), "terminator TLV type 0, length 0");
    assert!(l[60..].iter().all(|b| *b == 0), "50 reserved octets");

    assert_eq!(roundtrip(&built.bytes), ["ethernet2", "lacp"]);

    assert_dissects(&built.bytes, None, "slow");
}

// --------------------------------------------------------------------------
// `PPPoE` Discovery (PADI)
// --------------------------------------------------------------------------

const PADI: &str = "ffffffffffff 001122334455 8863 \
    11 09 0000 000c \
    0101 0000 \
    0103 0004 deadbeef";

#[test]
fn pppoe_padi_golden() {
    let p = Packet::from_layers(vec![
        Layer::new("ethernet2")
            .with("dst", mac([0xff; 6]))
            .with("src", mac([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]))
            .with("ethertype", 0x8863u16),
        Layer::new("pppoe_discovery")
            .with("code", 0x09u8)
            .with("session_id", 0u16)
            .with(
                "tags",
                FieldValue::Tlv(vec![
                    TlvItem::named("service_name"),
                    TlvItem {
                        data: Some(vec![0xde, 0xad, 0xbe, 0xef]),
                        ..TlvItem::named("host_uniq")
                    },
                ]),
            ),
    ])
    .unpadded();
    let built = p.build(catalog()).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    assert_eq!(built.bytes, hex(PADI));

    // ver/type nibbles 0x11, code 0x09 (PADI), length = tag octets (12).
    assert_eq!(built.bytes[14], 0x11);
    assert_eq!(built.bytes[15], 0x09);
    assert_eq!(u16::from_be_bytes([built.bytes[18], built.bytes[19]]), 12);

    assert_eq!(roundtrip(&built.bytes), ["ethernet2", "pppoe_discovery"]);

    assert_dissects(&built.bytes, None, "pppoed");
}

// --------------------------------------------------------------------------
// EAPOL, CDP and a `PPPoE`-session / PPP-LCP stack (structural + round-trip)
// --------------------------------------------------------------------------

#[test]
fn eapol_length_and_roundtrip() {
    let p = Packet::from_layers(vec![
        Layer::new("ethernet2")
            .with("dst", mac([0x01, 0x80, 0xc2, 0x00, 0x00, 0x03]))
            .with("src", mac([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]))
            .with("ethertype", 0x888eu16),
        Layer::new("eapol")
            .with("type", 0u8)
            .with("body", vec![0x02, 0x01, 0x00, 0x06, 0x01, 0x00]),
    ])
    .unpadded();
    let built = p.build(catalog()).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    // length auto = EAP body length (6).
    assert_eq!(u16::from_be_bytes([built.bytes[16], built.bytes[17]]), 6);
    assert_eq!(roundtrip(&built.bytes), ["ethernet2", "eapol"]);
}

#[test]
fn pppoe_session_ppp_lcp_roundtrip() {
    let p = Packet::from_layers(vec![
        Layer::new("ethernet2")
            .with("dst", mac([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]))
            .with("src", mac([0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb]))
            .with("ethertype", 0x8864u16),
        Layer::new("pppoe_session").with("session_id", 0x0102u16),
        Layer::new("ppp").with("protocol", 0xc021u16),
        Layer::new("ppp_lcp")
            .with("code", 1u8)
            .with("identifier", 1u8)
            .with(
                "options",
                FieldValue::Tlv(vec![
                    TlvItem::named("mru").with("mru", 1492u16),
                    TlvItem::named("magic_number").with("magic", 0x1234_5678u32),
                ]),
            ),
    ])
    .unpadded();
    let built = p.build(catalog()).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    // `PPPoE` session length = PPP protocol (2) + LCP packet (4 header + MRU 4 + magic 6 = 14) = 16.
    let plen = u16::from_be_bytes([built.bytes[18], built.bytes[19]]);
    assert_eq!(plen, 16);
    assert_eq!(
        roundtrip(&built.bytes),
        ["ethernet2", "pppoe_session", "ppp", "ppp_lcp"]
    );
}

#[test]
fn cdp_checksum_and_roundtrip() {
    // CDP over 802.3 + LLC/SNAP (OUI 00:00:0c, PID 0x2000), even total length.
    let p = Packet::from_layers(vec![
        Layer::new("dot3")
            .with("dst", mac([0x01, 0x00, 0x0c, 0xcc, 0xcc, 0xcc]))
            .with("src", mac([0x00, 0x11, 0x22, 0x33, 0x44, 0x55])),
        Layer::new("llc").with("dsap", 0xaau8).with("ssap", 0xaau8),
        Layer::new("snap")
            .with("oui", 0x00000cu32)
            .with("pid", 0x2000u16),
        Layer::new("cdp").with(
            "tlvs",
            FieldValue::Tlv(vec![
                TlvItem {
                    data: Some(b"Switch".to_vec()),
                    ..TlvItem::named("device_id")
                },
                TlvItem {
                    data: Some(b"Gig0/1".to_vec()),
                    ..TlvItem::named("port_id")
                },
            ]),
        ),
    ])
    .unpadded();
    let built = p.build(catalog()).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    // Independent RFC 1071 verification of the CDP checksum (even length).
    let cdp = &built.bytes[22..];
    assert_eq!(cdp.len() % 2, 0, "even length so the standard sum applies");
    let mut sum: u32 = 0;
    for c in cdp.chunks(2) {
        sum += u32::from(u16::from_be_bytes([c[0], c[1]]));
        while sum > 0xffff {
            sum = (sum & 0xffff) + (sum >> 16);
        }
    }
    assert_eq!(sum, 0xffff, "CDP internet checksum verifies");

    let parsed = parse(&built.bytes, "ethernet2", catalog()).unwrap();
    assert_eq!(protos(&parsed.packet), ["dot3", "llc", "snap", "cdp"]);
    assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, built.bytes);
}

// --------------------------------------------------------------------------
// Proptest: LLDP TLV lists round-trip
// --------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn lldp_tlv_lists_roundtrip(
        tlvs in proptest::collection::vec(
            (0u64..=127, proptest::collection::vec(any::<u8>(), 0..40)),
            0..8,
        )
    ) {
        let items: Vec<_> = tlvs
            .into_iter()
            .map(|(ty, v)| lldp_tlv(ty, v))
            .chain(std::iter::once(lldp_tlv(0, Vec::new())))
            .collect();
        let p = Packet::from_layers(vec![
            Layer::new("ethernet2")
                .with("src", mac([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]))
                .with("ethertype", 0x88ccu16),
            Layer::new("lldp").with("tlvs", FieldValue::List(items)),
        ])
        .unpadded();
        let bytes = p.build(catalog()).unwrap().bytes;
        let parsed = parse(&bytes, "ethernet2", catalog()).unwrap();
        prop_assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    }
}
