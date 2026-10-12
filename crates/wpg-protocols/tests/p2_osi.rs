//! OSI encapsulation (M7): 802.3 + LLC `0xfe` and PPP `0x0023` carry OSI PDUs
//! demultiplexed by their NLPID octet; NLPIDs without a dedicated definition
//! decode as the generic `osi` PDU (IS-IS has one, see `p2_isis.rs`).

use wpg_core::{parse, FieldValue, Layer, Packet};
use wpg_protocols::catalog;

fn protos(p: &Packet) -> Vec<&str> {
    p.layers.iter().map(|l| l.protocol.as_str()).collect()
}

/// An ES-IS-looking body (NLPID `0x82`) as an opaque generic OSI PDU.
fn osi_layer() -> Layer {
    Layer::new("osi")
        .with("nlpid", 0x82u8)
        .with("data", vec![0x09, 0x01, 0x00, 0x02, 0x00, 0x1e, 0x00])
}

#[test]
fn llc_announces_osi_and_parses_it_back() {
    let p = Packet::from_layers(vec![
        Layer::new("dot3")
            .with("dst", FieldValue::Mac([0x01, 0x80, 0xc2, 0, 0, 0x15]))
            .with("src", FieldValue::Mac([0x02, 0, 0, 0, 0, 1])),
        Layer::new("llc"),
        osi_layer(),
    ]);
    let built = p.build(catalog()).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    // length = LLC (3) + PDU (8); DSAP / SSAP Auto = 0xfe; then the NLPID.
    assert_eq!(&built.bytes[12..18], &[0, 11, 0xfe, 0xfe, 0x03, 0x82]);
    assert_eq!(built.bytes.len(), 60, "padded to the Ethernet minimum");

    let parsed = parse(&built.bytes, "ethernet2", catalog()).unwrap();
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert_eq!(protos(&parsed.packet), ["dot3", "llc", "osi"]);
    assert_eq!(
        parsed.packet.layers[2].fields["nlpid"],
        FieldValue::UInt(0x82)
    );
    assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, built.bytes);
}

#[test]
fn ppp_carries_osi() {
    let p = Packet::from_layers(vec![Layer::new("ppp"), osi_layer()]);
    let built = p.build(catalog()).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    assert_eq!(&built.bytes[..3], &[0x00, 0x23, 0x82]);

    let parsed = parse(&built.bytes, "ppp", catalog()).unwrap();
    assert_eq!(protos(&parsed.packet), ["ppp", "osi"]);
    assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, built.bytes);
}

#[test]
fn other_nlpids_fall_back_to_the_generic_pdu() {
    for nlpid in [0x00u8, 0x81, 0x82, 0x8e, 0xcc, 0x42] {
        let bytes = [0xfe, 0xfe, 0x03, nlpid, 0x01, 0x02];
        let parsed = parse(&bytes, "llc", catalog()).unwrap();
        assert_eq!(protos(&parsed.packet), ["llc", "osi"], "nlpid {nlpid:#x}");
        assert_eq!(
            parsed.packet.layers[1].fields["data"],
            FieldValue::Bytes(vec![1, 2])
        );
    }
}
