//! ARP golden bytes and round trip.

use std::net::Ipv4Addr;

use wpg_core::{parse, DiagLevel, FieldValue, Layer, Packet};
use wpg_protocols::catalog;

fn hex(s: &str) -> Vec<u8> {
    let s: String = s.split_whitespace().collect();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// Wireshark sample: who has 192.168.0.10? tell 192.168.0.1.
const ARP_REQUEST: &str = "ff ff ff ff ff ff 00 0b 82 01 fc 42 08 06 \
    00 01 08 00 06 04 00 01 00 0b 82 01 fc 42 c0 a8 00 01 \
    00 00 00 00 00 00 c0 a8 00 0a";

fn request() -> Packet {
    Packet::from_layers(vec![
            Layer::new("ethernet2")
                .with("dst", FieldValue::Mac([0xff; 6]))
                .with("src", FieldValue::Mac([0x00, 0x0b, 0x82, 0x01, 0xfc, 0x42])),
            Layer::new("arp")
                .with("sha", FieldValue::Mac([0x00, 0x0b, 0x82, 0x01, 0xfc, 0x42]))
                .with("spa", Ipv4Addr::new(192, 168, 0, 1))
                .with("tpa", Ipv4Addr::new(192, 168, 0, 10))
                .with("tha", FieldValue::Mac([0; 6])),
        ])
    // The sample was captured on the sending host, before minimum-frame padding.
    .unpadded()
}

#[test]
fn golden_arp_request() {
    let built = request().build(catalog()).unwrap();
    assert_eq!(built.bytes, hex(ARP_REQUEST));
    assert_eq!(built.bytes.len(), 42);
    let mut padded = request();
    padded.pad_to_min = true;
    let mut want = hex(ARP_REQUEST);
    want.resize(60, 0);
    assert_eq!(padded.build(catalog()).unwrap().bytes, want);
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    // EtherType came from the binding.
    let eth = &built.layout[0];
    let et = eth.fields.iter().find(|f| f.name == "ethertype").unwrap();
    assert!(et.auto);
    assert_eq!(et.value, FieldValue::UInt(0x0806));
}

#[test]
fn arp_reply_sets_oper() {
    let mut p = request();
    p.layers[1] = p.layers[1].clone().with("oper", 2u16);
    let b = p.build(catalog()).unwrap();
    assert_eq!(&b.bytes[20..22], &[0, 2]);
}

#[test]
fn arp_parse_rebuild() {
    let bytes = hex(ARP_REQUEST);
    let parsed = parse(&bytes, "ethernet2", catalog()).unwrap();
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let protos: Vec<_> = parsed
        .packet
        .layers
        .iter()
        .map(|l| l.protocol.as_str())
        .collect();
    assert_eq!(protos, ["ethernet2", "arp"]);
    let rebuilt = parsed.packet.build(catalog()).unwrap();
    assert_eq!(rebuilt.bytes, bytes);
}

#[test]
fn arp_nonstandard_sizes_warn() {
    let p = Packet::from_layers(vec![Layer::new("arp").with("hlen", 8u8)]);
    let b = p.build(catalog()).unwrap();
    assert!(b.diagnostics.iter().any(|d| d.level == DiagLevel::Warning));
}
