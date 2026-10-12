//! CLNP and ES-IS (M7): golden byte vectors, Fletcher checksum verification,
//! options up to the length indicator, the segmentation part, parse round-trips
//! and proptests.
//!
//! Expected bytes were derived field-by-field (ISO 8473-1 / ISO 9542) and
//! cross-checked with tshark 4.2 (checksums reported correct, options and the
//! segmentation part decoded); the checksums are re-verified with a naive ISO 8473
//! implementation. An optional `tshark` cross-check runs when `WPG_TSHARK=1`.

use std::collections::BTreeMap;

use proptest::prelude::*;
use wpg_core::diag::keys;
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

fn assert_golden(bytes: &[u8], want: &str) {
    assert_eq!(to_hex(bytes), to_hex(&hex(want)));
}

/// ISO 8473 verification: both running sums over `header` are zero.
fn checksum_verifies(header: &[u8]) -> bool {
    let (mut c0, mut c1) = (0u32, 0u32);
    for b in header {
        c0 = (c0 + u32::from(*b)) % 255;
        c1 = (c1 + c0) % 255;
    }
    c0 == 0 && c1 == 0
}

/// 802.3 + LLC to `09:00:2b:00:00:<last>` (0x04 all ESs, 0x05 all ISs), the PDU
/// and the optional data layer.
fn lan(last: u8, pdu: Layer, data: Option<&[u8]>) -> Vec<Layer> {
    let mut v = vec![
        Layer::new("dot3")
            .with("dst", FieldValue::Mac([0x09, 0x00, 0x2b, 0, 0, last]))
            .with("src", FieldValue::Mac([0x02, 0, 0, 0, 0, 1])),
        Layer::new("llc"),
        pdu,
    ];
    if let Some(d) = data {
        v.push(Layer::new("raw").with("data", d.to_vec()));
    }
    v
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

const DATA: [u8; 4] = [0xde, 0xad, 0xbe, 0xef];

// --------------------------------------------------------------------------
// CLNP
// --------------------------------------------------------------------------

/// DT: LI 31, version 1, lifetime 32, E/R + DT, segment length 35, checksum,
/// destination / source NSAPs.
const CLNP_DT: &str = "81 1f 01 20 3c 0023 46ec \
    0a 49000100000000000200 0a 49000100000000000100";

#[test]
fn clnp_dt_golden() {
    let bytes = build(lan(0x05, Layer::new("clnp"), Some(&DATA)));
    // 802.3 length = LLC (3) + header (31) + data (4).
    assert_eq!(&bytes[12..17], &[0, 38, 0xfe, 0xfe, 0x03]);
    let pdu = &bytes[17..];
    assert_golden(&pdu[..31], CLNP_DT);
    assert_eq!(&pdu[31..], DATA);
    assert!(checksum_verifies(&pdu[..31]));
    let p = roundtrip(&bytes, "ethernet2");
    assert_eq!(protos(&p), ["dot3", "llc", "clnp", "raw"]);
    assert_eq!(p.layers[2].fields["options"], FieldValue::Tlv(Vec::new()));
    assert_dissects(&bytes, None, "clnp");
}

fn segment(duid: u16, offset: u16, total: u16) -> FieldValue {
    FieldValue::Struct(BTreeMap::from([
        (
            "data_unit_identifier".to_owned(),
            FieldValue::UInt(duid.into()),
        ),
        ("segment_offset".to_owned(), FieldValue::UInt(offset.into())),
        ("total_length".to_owned(), FieldValue::UInt(total.into())),
    ]))
}

#[test]
fn clnp_segmentation_part_and_options() {
    let layer = Layer::new("clnp")
        .with("segmentation", vec![segment(7, 0, 48)])
        .with(
            "options",
            vec![
                TlvItem::named("priority").with("value", 5u8),
                TlvItem::named("padding").with("data", vec![0u8; 2]),
            ],
        );
    let bytes = build(lan(0x05, layer, Some(&DATA)));
    let pdu = &bytes[17..];
    // SP set (Auto), LI 44 = 31 + segmentation (6) + options (3 + 4).
    assert_golden(
        &pdu[..44],
        "81 2c 01 20 bc 0030 a21c \
         0a 49000100000000000200 0a 49000100000000000100 \
         0007 0000 0030 cd0105 cc020000",
    );
    assert!(checksum_verifies(&pdu[..44]));
    let p = roundtrip(&bytes, "ethernet2");
    assert_eq!(protos(&p), ["dot3", "llc", "clnp", "raw"]);
    let FieldValue::List(seg) = &p.layers[2].fields["segmentation"] else {
        panic!("segmentation did not decode: {:?}", p.layers[2]);
    };
    assert_eq!(seg.len(), 1);
    assert!(matches!(&p.layers[2].fields["options"], FieldValue::Tlv(t) if t.len() == 2));
    assert_dissects(&bytes, None, "clnp");
}

#[test]
fn clnp_error_report_with_reason_for_discard() {
    let layer = Layer::new("clnp")
        .with("pdu_type", 0x01u8)
        .with("er", 0u8)
        .with(
            "options",
            vec![TlvItem::named("reason_for_discard")
                .with("reason", 0x81u8)
                .with("pointer", 9u8)],
        );
    let bytes = build(lan(0x05, layer, Some(&DATA)));
    assert_golden(
        &bytes[17..17 + 35],
        "81 23 01 20 01 0027 38df \
         0a 49000100000000000200 0a 49000100000000000100 c1028109",
    );
    let p = roundtrip(&bytes, "ethernet2");
    let FieldValue::Tlv(opts) = &p.layers[2].fields["options"] else {
        panic!("options did not decode");
    };
    assert_eq!(opts[0].name.as_deref(), Some("reason_for_discard"));
    assert_eq!(opts[0].fields["reason"], FieldValue::UInt(0x81));
    assert_dissects(&bytes, None, "clnp");
}

#[test]
fn clnp_source_routing_addresses_decode() {
    let route = TlvItem::named("source_routing")
        .with("route_type", 0xffu8)
        .with("next_address", 3u8)
        .with(
            "addresses",
            vec![FieldValue::Struct(BTreeMap::from([(
                "address".to_owned(),
                FieldValue::Nsap(vec![0x49, 0, 2, 0, 0, 0, 0, 0, 3, 0]),
            )]))],
        );
    let bytes = build(lan(
        0x05,
        Layer::new("clnp").with("options", vec![route]),
        None,
    ));
    let p = roundtrip(&bytes, "ethernet2");
    let FieldValue::Tlv(opts) = &p.layers[2].fields["options"] else {
        panic!("options did not decode");
    };
    assert!(matches!(&opts[0].fields["addresses"], FieldValue::List(a) if a.len() == 1));
    assert!(checksum_verifies(&bytes[17..17 + usize::from(bytes[18])]));
}

#[test]
fn clnp_manual_checksums_are_sent_as_is() {
    // 0 = "checksum not used"; like any Manual value that differs from Auto it is
    // reported, and sent unchanged.
    for v in [0u16, 0x1234] {
        let built = Packet::from_layers(lan(0x05, Layer::new("clnp").with("checksum", v), None))
            .build(catalog())
            .unwrap();
        assert_eq!(&built.bytes[17 + 7..17 + 9], &v.to_be_bytes());
        assert!(built
            .diagnostics
            .iter()
            .any(|d| d.key == keys::CHECKSUM_MISMATCH));
    }
}

#[test]
fn clnp_over_ppp() {
    let bytes = build(vec![Layer::new("ppp"), Layer::new("clnp")]);
    assert_eq!(&bytes[..3], &[0x00, 0x23, 0x81]);
    assert_eq!(protos(&roundtrip(&bytes, "ppp")), ["ppp", "clnp"]);
}

// --------------------------------------------------------------------------
// ES-IS
// --------------------------------------------------------------------------

const ESH: &str = "82 15 01 00 02 001e 7275 01 0a 49000100000000000a00";
const ISH: &str = "82 14 01 00 04 001e b739 0a 49000100000000000100";
const RD: &str = "82 26 01 00 06 001e 88f2 0a 49000100000000000200 \
    06 020000000002 0a 49000100000000000200";

#[test]
fn esis_golden() {
    for (id, last, want) in [
        ("esis_esh", 0x05, ESH),
        ("esis_ish", 0x04, ISH),
        ("esis_rd", 0x05, RD),
    ] {
        let bytes = build(lan(last, Layer::new(id), None));
        let pdu = &bytes[17..];
        assert_golden(pdu, want);
        assert_eq!(usize::from(pdu[1]), pdu.len(), "{id}: LI covers the PDU");
        assert!(checksum_verifies(pdu), "{id}");
        assert_eq!(protos(&roundtrip(&bytes, "ethernet2")), ["dot3", "llc", id]);
        assert_dissects(&bytes, None, "esis");
    }
}

#[test]
fn esis_padded_frames_parse_back() {
    // On the wire the frame is padded to 60 octets; the 802.3 length bounds the PDU.
    for (id, _) in wpg_protocols::clnp::ESIS_PDUS
        .iter()
        .map(|(t, id)| (*id, t))
    {
        let built = Packet::from_layers(lan(0x04, Layer::new(id), None))
            .build(catalog())
            .unwrap();
        assert!(built.bytes.len() >= 60);
        assert_eq!(protos(&roundtrip(&built.bytes, "ethernet2"))[2], id);
    }
}

#[test]
fn esh_counts_its_source_addresses() {
    let addrs: Vec<FieldValue> = (1..=3u8)
        .map(|i| {
            FieldValue::Struct(BTreeMap::from([(
                "address".to_owned(),
                FieldValue::Nsap(vec![0x49, 0, 1, 0, 0, 0, 0, 0, i, 0]),
            )]))
        })
        .collect();
    let bytes = build(lan(
        0x05,
        Layer::new("esis_esh").with("source_addresses", addrs),
        None,
    ));
    let pdu = &bytes[17..];
    assert_eq!(pdu[9], 3, "number of source addresses (Auto)");
    assert_eq!(usize::from(pdu[1]), 9 + 1 + 3 * 11);
    let p = roundtrip(&bytes, "ethernet2");
    assert!(matches!(&p.layers[2].fields["source_addresses"], FieldValue::List(a) if a.len() == 3));
}

#[test]
fn ish_options_end_at_the_length_indicator() {
    let layer = Layer::new("esis_ish").with(
        "options",
        vec![TlvItem::named("es_configuration_timer").with("seconds", 600u16)],
    );
    let bytes = build(lan(0x04, layer, None));
    assert_golden(
        &bytes[17..],
        "82 18 01 00 04 001e 5079 0a 49000100000000000100 c6020258",
    );
    let p = roundtrip(&bytes, "ethernet2");
    let FieldValue::Tlv(opts) = &p.layers[2].fields["options"] else {
        panic!("options did not decode");
    };
    assert_eq!(opts[0].fields["seconds"], FieldValue::UInt(600));
    assert_dissects(&bytes, None, "esis");
}

#[test]
fn redirect_to_an_end_system_has_an_empty_net() {
    let bytes = build(lan(
        0x05,
        Layer::new("esis_rd").with("net", FieldValue::Nsap(Vec::new())),
        None,
    ));
    let pdu = &bytes[17..];
    assert_eq!(*pdu.last().unwrap(), 0, "NETL 0");
    assert_eq!(usize::from(pdu[1]), pdu.len());
    assert_eq!(protos(&roundtrip(&bytes, "ethernet2"))[2], "esis_rd");
}

#[test]
fn unknown_esis_types_stay_generic_and_short_pdus_fall_back() {
    let bytes = build(lan(
        0x05,
        Layer::new("esis")
            .with("pdu_type", 9u8)
            .with("data", vec![1u8, 2]),
        None,
    ));
    assert_eq!(
        protos(&roundtrip(&bytes, "ethernet2")),
        ["dot3", "llc", "esis"]
    );

    for (nlpid, len) in [(0x81u8, 10usize), (0x82, 8)] {
        let mut b = vec![0xfe, 0xfe, 0x03, nlpid];
        b.resize(3 + len, 1);
        let parsed = parse(&b, "llc", catalog()).unwrap();
        assert_eq!(protos(&parsed.packet), ["llc", "osi"], "nlpid {nlpid:#x}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn clnp_roundtrips_and_checksum_verifies(
        dst in proptest::collection::vec(any::<u8>(), 1..=20),
        src in proptest::collection::vec(any::<u8>(), 0..=20),
        lifetime in any::<u8>(),
        segmented in any::<bool>(),
        priority in proptest::option::of(any::<u8>()),
        data in proptest::collection::vec(any::<u8>(), 0..32),
    ) {
        let mut layer = Layer::new("clnp")
            .with("destination", FieldValue::Nsap(dst))
            .with("source", FieldValue::Nsap(src))
            .with("lifetime", lifetime);
        if segmented {
            layer = layer.with("segmentation", vec![segment(1, 0, 100)]);
        }
        let opts: Vec<TlvItem> = priority
            .map(|p| TlvItem::named("priority").with("value", p))
            .into_iter()
            .collect();
        layer = layer.with("options", opts);
        let mut layers = vec![Layer::new("llc"), layer];
        if !data.is_empty() {
            layers.push(Layer::new("raw").with("data", data));
        }
        let bytes = build(layers);
        let hl = usize::from(bytes[4]);
        prop_assert!(checksum_verifies(&bytes[3..3 + hl]));
        let parsed = parse(&bytes, "llc", catalog()).unwrap();
        prop_assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        prop_assert_eq!(&parsed.packet.layers[1].protocol, "clnp");
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    }

    #[test]
    fn esh_roundtrips_and_checksum_verifies(
        addrs in proptest::collection::vec(proptest::collection::vec(any::<u8>(), 1..=20), 0..6),
        holding in any::<u16>(),
    ) {
        let list: Vec<FieldValue> = addrs
            .into_iter()
            .map(|a| FieldValue::Struct(BTreeMap::from([("address".to_owned(), FieldValue::Nsap(a))])))
            .collect();
        let layer = Layer::new("esis_esh")
            .with("holding_time", holding)
            .with("source_addresses", list);
        let bytes = build(vec![Layer::new("llc"), layer]);
        prop_assert!(checksum_verifies(&bytes[3..]));
        let parsed = parse(&bytes, "llc", catalog()).unwrap();
        prop_assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        prop_assert_eq!(&parsed.packet.layers[1].protocol, "esis_esh");
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    }
}
