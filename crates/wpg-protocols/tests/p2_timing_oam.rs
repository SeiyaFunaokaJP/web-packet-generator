//! PTP, ESMC, Ethernet CFM / Y.1731 (with ERPS R-APS) and 802.3ah OAM (M7):
//! golden byte vectors, Auto lengths, the Slow Protocols subtype demux, generic
//! layers re-decoded by message type / opcode / code, End TLVs followed by
//! Ethernet padding, parse round-trips and proptests.
//!
//! Expected bytes were derived field-by-field (IEEE 1588-2008, ITU-T G.8264,
//! IEEE 802.1Q clause 21 / ITU-T Y.1731 / G.8032, IEEE 802.3 clause 57) and
//! cross-checked with tshark 4.2 (every message, PDU, TLV and flag decoded, no
//! malformed packet). An optional `tshark` cross-check runs when `WPG_TSHARK=1`.

use std::net::{Ipv4Addr, Ipv6Addr};

use proptest::prelude::*;
use wpg_core::{parse, FieldValue, Layer, Packet, TlvItem};
use wpg_protocols::{catalog, cfm, efm_oam, ptp};

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

fn eth(dst: [u8; 6]) -> Layer {
    Layer::new("ethernet2").with("dst", FieldValue::Mac(dst))
}

/// Builds `layers` (padded to the Ethernet minimum) without diagnostics.
fn build(layers: Vec<Layer>) -> Vec<u8> {
    let built = Packet::from_layers(layers).build(catalog()).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    built.bytes
}

/// Parses `bytes` without diagnostics and checks that rebuilding gives them back.
fn roundtrip(bytes: &[u8]) -> Packet {
    let parsed = parse(bytes, "ethernet2", catalog()).unwrap();
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

fn names(items: &[TlvItem]) -> Vec<&str> {
    items
        .iter()
        .map(|i| i.name.as_deref().unwrap_or("?"))
        .collect()
}

const PTP_MCAST: [u8; 6] = [0x01, 0x1b, 0x19, 0x00, 0x00, 0x00];
const SLOW_MCAST: [u8; 6] = [0x01, 0x80, 0xc2, 0x00, 0x00, 0x02];
const CCM_MCAST: [u8; 6] = [0x01, 0x80, 0xc2, 0x00, 0x00, 0x35];
const RAPS_MCAST: [u8; 6] = [0x01, 0x19, 0xa7, 0x00, 0x00, 0x01];

// ---------------------------------------------------------------------------
// PTP
// ---------------------------------------------------------------------------

#[test]
fn ptp_sync_over_ethernet_golden() {
    let bytes = build(vec![
        eth(PTP_MCAST),
        Layer::new("ptp_sync")
            .with("flags", 0x0200u16) // two-step
            .with("sequence_id", 7u16)
            .with("log_message_interval", -3i8)
            .with("origin_timestamp_seconds", 1_700_000_000u64)
            .with("origin_timestamp_nanoseconds", 500_000_000u32),
    ]);
    assert_eq!(&bytes[12..14], [0x88, 0xf7]);
    // Sync, version 2, length 44 (Auto), two-step, clock 02:00:00:ff:fe:00:00:01
    // port 1, sequence 7, control 0, interval -3, timestamp.
    assert_golden(
        &bytes[14..58],
        "00 02 002c 00 00 0200 0000000000000000 00000000 020000fffe000001 0001 0007 00 fd
         00006553f100 1dcd6500",
    );
    assert_eq!(bytes.len(), 60, "padded to the Ethernet minimum");
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "ptp_sync"]);
    assert!(p.pad_to_min);
    assert_eq!(
        p.layers[1].fields["log_message_interval"],
        FieldValue::Int(-3)
    );
    assert_dissects(&bytes, None, "ptp");
}

#[test]
fn ptp_follow_up_correction_and_delay_resp() {
    let bytes = build(vec![
        eth(PTP_MCAST),
        Layer::new("ptp_follow_up")
            .with("correction_field", -65_536i64) // -1 ns
            .with("precise_origin_timestamp_seconds", 1u64),
    ]);
    assert_golden(&bytes[14..18], "08 02 002c");
    assert_golden(&bytes[22..30], "ffffffffffff0000");
    assert_eq!(bytes[46], 2, "control field: Follow_Up");
    roundtrip(&bytes);

    let bytes = build(vec![eth(PTP_MCAST), Layer::new("ptp_delay_resp")]);
    // `Delay_Resp`: 54 octets, control 3, requesting port identity after the
    // receive timestamp.
    assert_golden(&bytes[14..18], "09 02 0036");
    assert_eq!(bytes[46], 3);
    assert_golden(&bytes[58..68], "020000fffe000002 0001");
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "ptp_delay_resp"]);
    assert_dissects(&bytes, None, "ptp");
}

#[test]
fn ptp_over_udp_event_and_general_ports() {
    let udp = |port: u16| {
        Layer::new("udp")
            .with("src_port", port)
            .with("dst_port", port)
    };
    let v4 = |dst: [u8; 4]| Layer::new("ipv4").with("dst", Ipv4Addr::from(dst));
    let cases = [
        (vec![v4([224, 0, 1, 129]), udp(319)], "ptp_delay_req"),
        (vec![v4([224, 0, 1, 129]), udp(320)], "ptp_announce"),
        (vec![v4([224, 0, 0, 107]), udp(319)], "ptp_pdelay_resp"),
        (
            vec![
                Layer::new("ipv6").with("dst", "ff0e::181".parse::<Ipv6Addr>().unwrap()),
                udp(320),
            ],
            "ptp_follow_up",
        ),
    ];
    for (lower, msg) in cases {
        let mut layers = vec![Layer::new("ethernet2")];
        layers.extend(lower);
        layers.push(Layer::new(msg));
        let n = layers.len();
        let bytes = build(layers);
        let p = roundtrip(&bytes);
        assert_eq!(protos(&p)[n - 1], msg);
        assert_dissects(&bytes, None, "ptp");
    }
    // The binding announces event messages on 319 and general ones on 320.
    let udp = catalog().get("udp").unwrap().next.clone().unwrap();
    assert_eq!(udp.value_for("ptp_sync"), Some(319));
    assert_eq!(udp.value_for("ptp_announce"), Some(320));
    assert_eq!(udp.protocol_for(319), Some(ptp::PTP));
}

#[test]
fn ptp_announce_golden_with_path_trace() {
    let bytes = build(vec![
        eth(PTP_MCAST),
        Layer::new("ptp_announce")
            .with("grandmaster_clock_class", 6u8)
            .with("grandmaster_clock_accuracy", 0x21u8)
            .with("grandmaster_offset_scaled_log_variance", 0x4e5du16)
            .with("time_source", 0x20u8)
            .with(
                "tlvs",
                vec![TlvItem::named("path_trace").with(
                    "path_sequence",
                    FieldValue::List(vec![FieldValue::UInt(0x0200_00ff_fe00_0001)]),
                )],
            ),
    ]);
    // Length 64 + 4 + 8 = 76 (Auto), Announce body and the PATH_TRACE TLV.
    assert_golden(&bytes[14..18], "0b 02 004c");
    assert_golden(
        &bytes[48..90],
        "0000000000000000 0000 0025 00 80 06 21 4e5d 80 020000fffe000001 0000 20
         0008 0008 020000fffe000001",
    );
    let p = roundtrip(&bytes);
    assert_eq!(names(&items(&p, 1, "tlvs")), ["path_trace"]);
    assert_dissects(&bytes, None, "ptp");
}

#[test]
fn ptp_signaling_and_management_tlvs() {
    let bytes = build(vec![
        eth(PTP_MCAST),
        Layer::new("ptp_signaling").with(
            "tlvs",
            vec![
                TlvItem::named("grant_unicast_transmission")
                    .with("message_type", 0x0u8)
                    .with("log_inter_message_period", -4i8)
                    .with("renewal_invited", 1u8),
                TlvItem::named("cancel_unicast_transmission").with("message_type", 0x9u8),
            ],
        ),
    ]);
    // 44 + (4 + 8) + (4 + 2) = 62.
    assert_golden(&bytes[14..18], "0c 02 003e");
    assert_golden(
        &bytes[58..76],
        "0005 0008 00 fc 0000012c 00 01 0006 0002 90 00",
    );
    let p = roundtrip(&bytes);
    assert_eq!(
        names(&items(&p, 1, "tlvs")),
        ["grant_unicast_transmission", "cancel_unicast_transmission"]
    );
    assert_dissects(&bytes, None, "ptp");

    let bytes = build(vec![
        eth(PTP_MCAST),
        Layer::new("ptp_management").with("action", 1u8).with(
            "tlvs",
            vec![TlvItem::named("management")
                .with("management_id", 0x2005u16)
                .with("data_field", vec![0x80, 0x00])],
        ),
    ]);
    // 48 + 4 + 4 = 56; SET PRIORITY1 = 128.
    assert_golden(&bytes[14..18], "0d 02 0038");
    assert_golden(&bytes[58..70], "01 01 01 00 0001 0004 2005 8000");
    roundtrip(&bytes);
    assert_dissects(&bytes, None, "ptp");
}

#[test]
fn ptp_generic_redecodes_every_message() {
    for (ty, id, _) in ptp::MESSAGES {
        let bytes = build(vec![eth(PTP_MCAST), Layer::new(id)]);
        assert_eq!(bytes[14] & 0x0f, u8::try_from(ty).unwrap());
        let p = roundtrip(&bytes);
        assert_eq!(protos(&p), ["ethernet2", id]);
        // Announced through the generic message.
        let b = catalog().get("ethernet2").unwrap().next.clone().unwrap();
        assert_eq!(b.value_for(id), Some(ptp::PTP_ETHERTYPE));
    }
    // An unknown message type stays generic.
    let bytes = build(vec![
        eth(PTP_MCAST),
        Layer::new("ptp").with("message_type", 0x5u8),
    ]);
    assert_eq!(protos(&roundtrip(&bytes)), ["ethernet2", "ptp"]);
}

// ---------------------------------------------------------------------------
// Slow Protocols: ESMC and the subtype demux
// ---------------------------------------------------------------------------

#[test]
fn esmc_golden() {
    let bytes = build(vec![eth(SLOW_MCAST), Layer::new("esmc")]);
    assert_eq!(&bytes[12..14], [0x88, 0x09]);
    assert_golden(
        &bytes[14..48],
        "0a 0019a7 0001 10 000000 01 0004 02
         02 0014 ff 020000fffe000001 00 01 00 0000000000",
    );
    assert_eq!(bytes.len(), 60);
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "esmc"]);
    // The padding after the TLVs is kept as one raw item.
    let tlvs = items(&p, 1, "tlvs");
    assert_eq!(names(&tlvs)[..2], ["ql", "extended_ql"]);
    assert_eq!(tlvs[2].raw.as_deref(), Some(&[0u8; 12][..]));
    assert_dissects(&bytes, None, "ossp");

    // Event PDU with QL-DNU only.
    let bytes = build(vec![
        eth(SLOW_MCAST),
        Layer::new("esmc")
            .with("event_flag", 1u8)
            .with("tlvs", vec![TlvItem::named("ql").with("ssm_code", 0xfu8)]),
    ]);
    assert_golden(&bytes[14..28], "0a 0019a7 0001 18 000000 01 0004 0f");
    roundtrip(&bytes);
}

#[test]
fn slow_protocols_demux_by_subtype() {
    for (id, subtype) in [
        ("lacp", 1u8),
        ("marker", 2),
        ("efm_oam_information", 3),
        ("efm_oam_loopback_control", 3),
        ("esmc", 0x0a),
    ] {
        let bytes = build(vec![eth(SLOW_MCAST), Layer::new(id)]);
        assert_eq!(&bytes[12..14], [0x88, 0x09], "{id}");
        assert_eq!(bytes[14], subtype, "{id}");
        assert_eq!(protos(&roundtrip(&bytes)), ["ethernet2", id]);
    }
    // An unknown subtype is raw.
    let bytes = hex("0180c2000002 020000000001 8809 0b 00000000");
    let parsed = parse(&bytes, "ethernet2", catalog()).unwrap();
    assert_eq!(protos(&parsed.packet), ["ethernet2", "raw"]);
}

// ---------------------------------------------------------------------------
// CFM / Y.1731 / R-APS
// ---------------------------------------------------------------------------

#[test]
fn cfm_ccm_golden_behind_a_vlan_tag() {
    let bytes = build(vec![
        eth(CCM_MCAST),
        Layer::new("dot1q").with("vid", 100u16),
        Layer::new("cfm_ccm")
            .with("md_level", 5u8)
            .with("sequence_number", 1u32)
            .with("mep_id", 10u16)
            .with(
                "tlvs",
                vec![
                    TlvItem::named("port_status"),
                    TlvItem::named("interface_status"),
                    TlvItem::named("sender_id"),
                    TlvItem::named("end"),
                ],
            ),
    ]);
    assert_eq!(&bytes[16..18], [0x89, 0x02]);
    let mut want = String::from("a0 01 04 46 00000001 000a 01200d 575047303031 4d4547303030 31");
    want.push_str(&"00".repeat(32));
    want.push_str(" 00000000 00000000 00000000 00000000");
    want.push_str(" 02 0001 02 04 0001 01 01 0008 06 04 020000000001 00");
    assert_golden(&bytes[18..], &want);
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "dot1q", "cfm_ccm"]);
    assert_eq!(
        names(&items(&p, 2, "tlvs")),
        ["port_status", "interface_status", "sender_id", "end"]
    );
    assert_dissects(&bytes, None, "cfm");
}

#[test]
fn cfm_end_tlv_and_ethernet_padding() {
    let bytes = build(vec![eth(CCM_MCAST), Layer::new("cfm_lbm")]);
    // LBM: level 0, opcode 3, offset 4, transaction ID 1, End TLV, then padding.
    assert_golden(&bytes[14..23], "00 03 00 04 00000001 00");
    assert!(bytes[23..].iter().all(|b| *b == 0));
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "cfm_lbm"]);
    assert!(p.pad_to_min);
    assert_eq!(names(&items(&p, 1, "tlvs")), ["end"]);
}

#[test]
fn cfm_loopback_and_linktrace_tlvs() {
    let bytes = build(vec![
        eth([2, 0, 0, 0, 0, 2]),
        Layer::new("cfm_lbr")
            .with("transaction_id", 0x1234u32)
            .with(
                "tlvs",
                vec![
                    TlvItem::named("data_tlv").with("value", b"wpg".to_vec()),
                    TlvItem::named("end"),
                ],
            ),
    ]);
    assert_golden(&bytes[14..29], "00 02 00 04 00001234 03 0003 777067 00");
    roundtrip(&bytes);
    assert_dissects(&bytes, None, "cfm");

    let bytes = build(vec![
        eth([1, 0x80, 0xc2, 0, 0, 0x38]),
        Layer::new("cfm_ltm"),
    ]);
    assert_golden(
        &bytes[14..47],
        "00 05 80 11 00000001 40 020000000001 020000000002 07 0008 0000020000000001 00",
    );
    roundtrip(&bytes);

    let bytes = build(vec![eth([2, 0, 0, 0, 0, 1]), Layer::new("cfm_ltr")]);
    assert_golden(
        &bytes[14..54],
        "00 04 a0 06 00000001 3f 01 08 0010 0000020000000001 0000020000000002
         05 0007 01 020000000002 00",
    );
    let p = roundtrip(&bytes);
    assert_eq!(
        names(&items(&p, 1, "tlvs")),
        ["ltr_egress_identifier", "reply_ingress", "end"]
    );
    assert_dissects(&bytes, None, "cfm");
}

#[test]
fn raps_golden() {
    let bytes = build(vec![
        eth(RAPS_MCAST),
        Layer::new("cfm_raps")
            .with("md_level", 7u8)
            .with("request_state", 0b1011u8) // SF
            .with("rb", 0u8)
            .with("bpr", 1u8)
            .with("node_id", FieldValue::Mac([2, 0, 0, 0, 0, 5])),
    ]);
    assert_golden(
        &bytes[14..51],
        &format!("e1 28 00 20 b0 20 020000000005 {} 00", "00".repeat(24)),
    );
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "cfm_raps"]);
    assert_dissects(&bytes, None, "cfm");
}

#[test]
fn y1731_performance_pdus() {
    let bytes = build(vec![
        eth(CCM_MCAST),
        Layer::new("cfm_dmm")
            .with("md_level", 5u8)
            .with("tx_timestamp_f_seconds", 0x6553_f100u32)
            .with("tx_timestamp_f_nanoseconds", 1u32),
    ]);
    assert_golden(
        &bytes[14..51],
        &format!("a0 2f 00 20 6553f100 00000001 {} 00", "00".repeat(24)),
    );
    roundtrip(&bytes);

    let bytes = build(vec![
        eth(CCM_MCAST),
        Layer::new("cfm_lmm").with("tx_fcf", 1000u32),
    ]);
    assert_golden(&bytes[14..31], "00 2b 00 0c 000003e8 00000000 00000000 00");
    roundtrip(&bytes);

    let bytes = build(vec![
        eth(CCM_MCAST),
        Layer::new("cfm_slr")
            .with("responder_mep_id", 2u16)
            .with("tx_fcf", 5u32)
            .with("tx_fcb", 4u32),
    ]);
    assert_golden(
        &bytes[14..35],
        "00 36 00 10 0001 0002 00000001 00000005 00000004 00",
    );
    roundtrip(&bytes);

    let bytes = build(vec![
        eth(CCM_MCAST),
        Layer::new("cfm_csf").with("type", 2u8).with("period", 6u8),
    ]);
    assert_golden(&bytes[14..19], "00 34 16 00 00");
    roundtrip(&bytes);
    assert_dissects(&bytes, None, "cfm");
}

#[test]
fn cfm_generic_redecodes_every_opcode() {
    for (opcode, id) in cfm::PDUS {
        let bytes = build(vec![eth(CCM_MCAST), Layer::new(id)]);
        assert_eq!(u64::from(bytes[15]), opcode, "{id}");
        let p = roundtrip(&bytes);
        assert_eq!(protos(&p), ["ethernet2", id]);
        assert_dissects(&bytes, None, "cfm");
    }
    // opcodes without a dedicated definition stay generic (Linear APS).
    let bytes = build(vec![
        eth(CCM_MCAST),
        Layer::new("cfm")
            .with("opcode", 39u8)
            .with("data", vec![0xb0, 0x01, 0x01, 0x01, 0x00]),
    ]);
    assert_eq!(protos(&roundtrip(&bytes)), ["ethernet2", "cfm"]);
}

// ---------------------------------------------------------------------------
// 802.3ah OAM
// ---------------------------------------------------------------------------

#[test]
fn efm_oam_information_golden() {
    let info = |name: &str| {
        TlvItem::named(name)
            .with("revision", 1u16)
            .with("oam_configuration", 0x1du8)
    };
    let bytes = build(vec![
        eth(SLOW_MCAST),
        Layer::new("efm_oam_information").with(
            "tlvs",
            vec![
                info("local_information"),
                info("remote_information"),
                TlvItem::named("end"),
            ],
        ),
    ]);
    assert_golden(
        &bytes[14..51],
        "03 0050 00
         01 10 01 0001 00 1d 05ee 000000 00000000
         02 10 01 0001 00 1d 05ee 000000 00000000
         00",
    );
    let p = roundtrip(&bytes);
    assert_eq!(protos(&p), ["ethernet2", "efm_oam_information"]);
    assert!(p.pad_to_min);
    assert_eq!(
        names(&items(&p, 1, "tlvs")),
        ["local_information", "remote_information", "end"]
    );
    assert_dissects(&bytes, None, "oampdu");
}

#[test]
fn efm_oam_event_notification_golden() {
    let bytes = build(vec![
        eth(SLOW_MCAST),
        Layer::new("efm_oam_event_notification")
            .with("sequence_number", 2u16)
            .with(
                "tlvs",
                vec![
                    TlvItem::named("errored_symbol_period")
                        .with("window", 125_000_000u64)
                        .with("errors", 3u64),
                    TlvItem::named("errored_frame_seconds_summary").with("window", 600u16),
                    TlvItem::named("end"),
                ],
            ),
    ]);
    assert_golden(
        &bytes[14..79],
        "03 0050 01 0002
         01 28 0000 0000000007735940 0000000000000001 0000000000000003 0000000000000001 00000001
         04 12 0000 0258 0001 0001 00000001 00000001
         00",
    );
    roundtrip(&bytes);
    assert_dissects(&bytes, None, "oampdu");
}

#[test]
fn efm_oam_generic_redecodes_every_code() {
    for (code, id) in efm_oam::PDUS {
        let bytes = build(vec![eth(SLOW_MCAST), Layer::new(id)]);
        assert_eq!(u64::from(bytes[17]), code, "{id}");
        let p = roundtrip(&bytes);
        assert_eq!(protos(&p), ["ethernet2", id]);
        assert_dissects(&bytes, None, "oampdu");
    }
    let bytes = build(vec![
        eth(SLOW_MCAST),
        Layer::new("efm_oam_loopback_control").with("remote_loopback_command", 2u8),
    ]);
    assert_golden(&bytes[14..19], "03 0050 04 02");
}

// ---------------------------------------------------------------------------
// Proptests
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn ptp_delay_req_roundtrips(
        seq in any::<u16>(),
        domain in any::<u8>(),
        correction in any::<i64>(),
        secs in 0u64..(1 << 48),
        ns in 0u32..1_000_000_000,
        interval in any::<i8>(),
    ) {
        let bytes = build(vec![
            eth(PTP_MCAST),
            Layer::new("ptp_delay_req")
                .with("sequence_id", seq)
                .with("domain_number", domain)
                .with("correction_field", correction)
                .with("log_message_interval", interval)
                .with("origin_timestamp_seconds", secs)
                .with("origin_timestamp_nanoseconds", ns),
        ]);
        let p = roundtrip(&bytes);
        prop_assert_eq!(protos(&p), ["ethernet2", "ptp_delay_req"]);
        prop_assert_eq!(&p.layers[1].fields["correction_field"], &FieldValue::Int(correction));
    }

    #[test]
    fn cfm_ccm_roundtrips(
        level in 0u8..8,
        seq in any::<u32>(),
        mep in 1u16..8192,
        rdi in 0u8..2,
        period in 1u8..8,
        data in proptest::collection::vec(any::<u8>(), 0..40),
    ) {
        let bytes = build(vec![
            eth(CCM_MCAST),
            Layer::new("cfm_ccm")
                .with("md_level", level)
                .with("sequence_number", seq)
                .with("mep_id", mep)
                .with("rdi", rdi)
                .with("period", period)
                .with("tlvs", vec![
                    TlvItem::named("data_tlv").with("value", data),
                    TlvItem::named("end"),
                ]),
        ]);
        let p = roundtrip(&bytes);
        prop_assert_eq!(protos(&p), ["ethernet2", "cfm_ccm"]);
        let tlvs = items(&p, 1, "tlvs");
        prop_assert_eq!(names(&tlvs), ["data_tlv", "end"]);
    }
}
