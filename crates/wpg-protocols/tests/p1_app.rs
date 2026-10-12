//! Golden byte vectors, parse round-trips and proptests for the P1 application
//! protocols (`DHCPv4` / `DHCPv6` incl. relay, DNS, NTP, SNMP v1 / v2c, Syslog,
//! RADIUS, TFTP).
//!
//! The application-layer octets are re-derived here field-by-field (BER lengths
//! and DNS labels counted by hand); the IPv4 header and UDP checksums (with the
//! pseudo header) are re-verified with a naive RFC 1071 sum. An optional `tshark`
//! cross-check runs when `WPG_TSHARK=1`.

use std::net::{Ipv4Addr, Ipv6Addr};

use proptest::prelude::*;
use wpg_core::{parse, FieldValue, Layer, Packet, TlvItem};
use wpg_protocols::app::{ber_oid, ber_uint, dns_name, dns_question, dns_rr};
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

/// A TLV item of the named variant with verbatim value octets.
fn named_data(name: &str, data: impl Into<Vec<u8>>) -> TlvItem {
    let mut item = TlvItem::named(name);
    item.data = Some(data.into());
    item
}

const APP: usize = 28; // IPv4 (20) + UDP (8)

fn ipv4_udp(sport: u16, dport: u16) -> Vec<Layer> {
    vec![
        Layer::new("ipv4")
            .with("src", Ipv4Addr::new(192, 0, 2, 1))
            .with("dst", Ipv4Addr::new(192, 0, 2, 2)),
        Layer::new("udp")
            .with("src_port", sport)
            .with("dst_port", dport),
    ]
}

/// Builds `ipv4 / udp / app...`, checks there are no diagnostics, the IPv4 and
/// UDP checksums verify and the UDP length covers the datagram.
fn build_v4(sport: u16, dport: u16, app: Vec<Layer>) -> Vec<u8> {
    let mut layers = ipv4_udp(sport, dport);
    layers.extend(app);
    let built = Packet::from_layers(layers)
        .unpadded()
        .build(catalog())
        .unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    let b = built.bytes;
    assert!(verifies(&b[0..20]), "IPv4 header checksum");
    let udp_len = u16::try_from(b.len() - 20).unwrap();
    assert_eq!(u16::from_be_bytes([b[24], b[25]]), udp_len, "UDP length");
    let mut ph = b[12..20].to_vec();
    ph.extend_from_slice(&[0, 17]);
    ph.extend_from_slice(&udp_len.to_be_bytes());
    ph.extend_from_slice(&b[20..]);
    assert!(verifies(&ph), "UDP checksum with pseudo header");
    b
}

/// Round-trips a frame (parse → rebuild reproduces it, no errors) and returns the
/// parsed layer names.
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

#[test]
fn udp_port_bindings() {
    let udp = catalog().get("udp").unwrap();
    let b = udp.next.as_ref().unwrap();
    for (port, id) in [
        (53, "dns"),
        (67, "dhcpv4"),
        (68, "dhcpv4"),
        (69, "tftp"),
        (123, "ntp"),
        (161, "snmp"),
        (162, "snmp"),
        (514, "syslog"),
        (546, "dhcpv6"),
        (547, "dhcpv6"),
        (1812, "radius"),
        (1813, "radius"),
    ] {
        assert_eq!(b.protocol_for(port), Some(id), "UDP port {port}");
    }
}

// --------------------------------------------------------------------------
// DHCPv4
// --------------------------------------------------------------------------

/// BOOTP fixed part up to and including `giaddr`, then chaddr (MAC + 10 octets
/// padding), sname (64), file (128) and the magic cookie.
fn bootp(head: &str, mac: &str) -> Vec<u8> {
    let mut v = hex(head);
    v.extend(hex(mac));
    v.extend(vec![0u8; 10 + 64 + 128]);
    v.extend(hex("63825363"));
    v
}

#[test]
fn dhcpv4_discover_golden() {
    let b = build_v4(
        68,
        67,
        vec![Layer::new("dhcpv4")
            .with("xid", 0xdead_beefu32)
            .with("flags", 0x8000u16)
            .with(
                "chaddr",
                FieldValue::Mac([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]),
            )
            .with(
                "options",
                vec![
                    TlvItem::named("message_type").with("type", 1u8),
                    TlvItem::named("parameter_request_list").with(
                        "codes",
                        FieldValue::List(vec![1u8.into(), 3u8.into(), 6u8.into()]),
                    ),
                    TlvItem::named("end"),
                ],
            )],
    );
    let mut want = bootp(
        "01 01 06 00 deadbeef 0000 8000 00000000 00000000 00000000 00000000",
        "001122334455",
    );
    want.extend(hex("35 01 01  37 03 01 03 06  ff"));
    assert_eq!(&b[APP..], want.as_slice());
    assert_eq!(b.len() - APP, 240 + 9);
    assert_eq!(roundtrip(&b, "ipv4"), ["ipv4", "udp", "dhcpv4"]);
    assert_dissects(&b, Some(0x0800), "dhcp");
}

#[test]
fn dhcpv4_relayed_request_golden() {
    // A relay agent forwards to the server (67 → 67) with hops 1, giaddr set and
    // Relay Agent Information (circuit-id "eth0", remote-id = MAC).
    let b = build_v4(
        67,
        67,
        vec![Layer::new("dhcpv4")
            .with("hops", 1u8)
            .with("xid", 1u32)
            .with("giaddr", Ipv4Addr::new(10, 0, 0, 1))
            .with(
                "chaddr",
                FieldValue::Mac([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]),
            )
            .with(
                "options",
                vec![
                    TlvItem::named("message_type").with("type", 3u8),
                    TlvItem::named("requested_ip").with("address", Ipv4Addr::new(10, 0, 0, 50)),
                    TlvItem::named("relay_agent_info").with(
                        "suboptions",
                        vec![
                            named_data("circuit_id", b"eth0"),
                            named_data("remote_id", [0x00, 0x11, 0x22, 0x33, 0x44, 0x55]),
                        ],
                    ),
                    TlvItem::named("end"),
                ],
            )],
    );
    let mut want = bootp(
        "01 01 06 01 00000001 0000 0000 00000000 00000000 00000000 0a000001",
        "001122334455",
    );
    want.extend(hex(
        "35 01 03  32 04 0a000032  52 0e 01 04 65746830 02 06 001122334455  ff",
    ));
    assert_eq!(&b[APP..], want.as_slice());
    assert_eq!(roundtrip(&b, "ipv4"), ["ipv4", "udp", "dhcpv4"]);
    assert_dissects(&b, Some(0x0800), "dhcp");
}

// --------------------------------------------------------------------------
// DHCPv6
// --------------------------------------------------------------------------

fn ipv6_udp(sport: u16, dport: u16) -> Vec<Layer> {
    vec![
        Layer::new("ipv6")
            .with("src", "fe80::1".parse::<Ipv6Addr>().unwrap())
            .with("dst", "ff02::1:2".parse::<Ipv6Addr>().unwrap()),
        Layer::new("udp")
            .with("src_port", sport)
            .with("dst_port", dport),
    ]
}

const DUID: [u8; 10] = [0x00, 0x03, 0x00, 0x01, 0x02, 0x00, 0x00, 0x00, 0x00, 0x01];

#[test]
fn dhcpv6_solicit_golden() {
    let mut layers = ipv6_udp(546, 547);
    layers.push(Layer::new("dhcpv6").with("transaction_id", 0x00ab_cdefu32));
    let built = Packet::from_layers(layers)
        .unpadded()
        .build(catalog())
        .unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    let b = built.bytes;
    // Default options: Client ID (DUID-LL), Elapsed Time 0, IA_NA (IAID 1).
    let want = hex("01 abcdef \
        0001 000a 0003 0001 020000000001 \
        0008 0002 0000 \
        0003 000c 00000001 00000000 00000000");
    assert_eq!(&b[48..], want.as_slice());
    assert_eq!(b[6], 17, "IPv6 next header UDP");
    assert_eq!(roundtrip(&b, "ipv6"), ["ipv6", "udp", "dhcpv6"]);
    assert_dissects(&b, Some(0x86dd), "dhcpv6");
}

#[test]
fn dhcpv6_relay_forward_golden() {
    let mut layers = ipv6_udp(547, 547);
    layers.push(
        Layer::new("dhcpv6_relay")
            .with("hop_count", 0u8)
            .with("link_address", "2001:db8::1".parse::<Ipv6Addr>().unwrap())
            .with("peer_address", "fe80::2".parse::<Ipv6Addr>().unwrap())
            .with(
                "options",
                vec![
                    named_data("interface_id", b"eth0"),
                    TlvItem::named("relay_message")
                        .with("msg_type", 1u8)
                        .with("transaction_id", 0x00_1234u32)
                        .with("options", vec![TlvItem::with_type(1, DUID)]),
                ],
            ),
    );
    let built = Packet::from_layers(layers)
        .unpadded()
        .build(catalog())
        .unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    let b = built.bytes;
    let want = hex("0c 00 \
        20010db8000000000000000000000001 \
        fe800000000000000000000000000002 \
        0012 0004 65746830 \
        0009 0012 01 001234 0001 000a 0003 0001 020000000001");
    assert_eq!(&b[48..], want.as_slice());
    // Port 547 announces `dhcpv6`, which re-decodes as the relay layout.
    assert_eq!(roundtrip(&b, "ipv6"), ["ipv6", "udp", "dhcpv6_relay"]);
    assert_dissects(&b, Some(0x86dd), "dhcpv6");
}

// --------------------------------------------------------------------------
// DNS
// --------------------------------------------------------------------------

#[test]
fn dns_query_golden() {
    let b = build_v4(53000, 53, vec![Layer::new("dns").with("id", 0xbeefu16)]);
    let want = hex("beef 0100 0001 0000 0000 0000 \
        07 6578616d706c65 03 636f6d 00 0001 0001");
    assert_eq!(&b[APP..], want.as_slice());
    assert_eq!(roundtrip(&b, "ipv4"), ["ipv4", "udp", "dns"]);
    assert_dissects(&b, Some(0x0800), "dns");
}

#[test]
fn dns_response_golden() {
    let mut sections = dns_question("www.example.org", 28, 1);
    sections.extend(dns_rr(
        "www.example.org",
        28,
        1,
        3600,
        &"2001:db8::80".parse::<Ipv6Addr>().unwrap().octets(),
    ));
    // From the server's port 53 to the client's ephemeral port: UDP decodes it
    // through the source port.
    let b = build_v4(
        53,
        53000,
        vec![Layer::new("dns")
            .with("id", 7u16)
            .with("qr", 1u8)
            .with("aa", 1u8)
            .with("ra", 1u8)
            .with("ancount", 1u16)
            .with("sections", sections)],
    );
    let name = "03 777777 07 6578616d706c65 03 6f7267 00";
    let want = hex(&format!(
        "0007 8580 0001 0001 0000 0000 \
         {name} 001c 0001 \
         {name} 001c 0001 00000e10 0010 20010db8000000000000000000000080"
    ));
    assert_eq!(&b[APP..], want.as_slice());
    assert_eq!(roundtrip(&b, "ipv4"), ["ipv4", "udp", "dns"]);
    assert_dissects(&b, Some(0x0800), "dns");
}

// --------------------------------------------------------------------------
// NTP
// --------------------------------------------------------------------------

#[test]
fn ntp_client_golden() {
    let b = build_v4(
        123,
        123,
        vec![Layer::new("ntp").with("transmit_ts", 0xe8b0_c0de_8000_0000u64)],
    );
    let want = hex("23 00 06 ec 00000000 00000000 00000000 \
        0000000000000000 0000000000000000 0000000000000000 e8b0c0de80000000");
    assert_eq!(&b[APP..], want.as_slice());
    assert_eq!(roundtrip(&b, "ipv4"), ["ipv4", "udp", "ntp"]);
    assert_dissects(&b, Some(0x0800), "ntp");
}

#[test]
fn ntp_server_reply_golden() {
    let b = build_v4(
        123,
        50000,
        vec![Layer::new("ntp")
            .with("mode", 4u8)
            .with("stratum", 1u8)
            .with("precision", FieldValue::Int(-23))
            .with("reference_id", b"GPS\0".to_vec())
            .with("root_dispersion", 0x0000_0010u32)],
    );
    assert_eq!(
        &b[APP..APP + 16],
        hex("24 01 06 e9 00000000 00000010 47505300").as_slice()
    );
    assert_eq!(b.len() - APP, 48);
}

// --------------------------------------------------------------------------
// SNMP
// --------------------------------------------------------------------------

const SNMP_GET: &str = "30 26 02 01 01 04 06 7075626c6963 \
    a0 19 02 01 01 02 01 00 02 01 00 \
    30 0e 30 0c 06 08 2b06010201010100 05 00";

#[test]
fn snmp_v2c_get_request_golden() {
    // All defaults: v2c, "public", GetRequest request-id 1 for sysDescr.0.
    let b = build_v4(50000, 161, vec![Layer::new("snmp"), Layer::new("snmp_pdu")]);
    assert_eq!(&b[APP..], hex(SNMP_GET).as_slice());
    // Both BER lengths are Auto (short form).
    assert_eq!(usize::from(b[APP + 1]), b.len() - APP - 2);
    assert_eq!(usize::from(b[APP + 14]), b.len() - APP - 15);
    assert_eq!(roundtrip(&b, "ipv4"), ["ipv4", "udp", "snmp", "snmp_pdu"]);
    assert_dissects(&b, Some(0x0800), "snmp");
}

#[test]
fn snmp_get_bulk_with_explicit_body() {
    let varbind = |oid: &[u64]| {
        TlvItem::named("sequence").with(
            "items",
            vec![
                named_data("object_identifier", ber_oid(oid)),
                TlvItem::named("null"),
            ],
        )
    };
    let b = build_v4(
        50000,
        161,
        vec![
            Layer::new("snmp")
                .with("community", b"pub".to_vec())
                .with("community_length", 3u8),
            Layer::new("snmp_pdu").with("pdu_type", 0xa5u8).with(
                "body",
                vec![
                    TlvItem::with_type(0x02, ber_uint(0x1234)),
                    TlvItem::with_type(0x02, ber_uint(0)), // non-repeaters
                    TlvItem::with_type(0x02, ber_uint(10)), // max-repetitions
                    TlvItem::named("sequence")
                        .with("items", vec![varbind(&[1, 3, 6, 1, 2, 1, 2, 2, 1, 2])]),
                ],
            ),
        ],
    );
    let want = hex("30 25 02 01 01 04 03 707562 \
        a5 1b 02 02 1234 02 01 00 02 01 0a \
        30 0f 30 0d 06 09 2b06010201020201 02 05 00");
    assert_eq!(&b[APP..], want.as_slice());
    assert_eq!(roundtrip(&b, "ipv4"), ["ipv4", "udp", "snmp", "snmp_pdu"]);
}

#[test]
fn snmp_v1_trap_golden() {
    let b = build_v4(
        50000,
        162,
        vec![
            Layer::new("snmp").with("version", 0u8),
            Layer::new("snmp_trap").with(
                "body",
                vec![
                    TlvItem::with_type(0x06, ber_oid(&[1, 3, 6, 1, 4, 1, 8072])),
                    TlvItem::named("ip_address").with("address", Ipv4Addr::new(192, 0, 2, 1)),
                    TlvItem::with_type(0x02, ber_uint(6)), // enterpriseSpecific
                    TlvItem::with_type(0x02, ber_uint(1)),
                    named_data("timeticks", ber_uint(1000)),
                    TlvItem::named("sequence").with("items", Vec::<TlvItem>::new()),
                ],
            ),
        ],
    );
    let want = hex("30 28 02 01 00 04 06 7075626c6963 \
        a4 1b 06 07 2b060104 01bf08 40 04 c0000201 02 01 06 02 01 01 43 02 03e8 30 00");
    assert_eq!(&b[APP..], want.as_slice());
    // `snmp` defaults to `snmp_pdu`, whose 0xa4 entry re-decodes as `snmp_trap`.
    assert_eq!(roundtrip(&b, "ipv4"), ["ipv4", "udp", "snmp", "snmp_trap"]);
    assert_dissects(&b, Some(0x0800), "snmp");
}

// --------------------------------------------------------------------------
// Syslog
// --------------------------------------------------------------------------

#[test]
fn syslog_golden() {
    let msg = b"<13>Oct 11 10:00:00 host app: test";
    let b = build_v4(
        514,
        514,
        vec![Layer::new("syslog").with("message", msg.to_vec())],
    );
    assert_eq!(&b[APP..], msg);
    assert_eq!(roundtrip(&b, "ipv4"), ["ipv4", "udp", "syslog"]);
    assert_dissects(&b, Some(0x0800), "syslog");
}

// --------------------------------------------------------------------------
// RADIUS
// --------------------------------------------------------------------------

#[test]
fn radius_access_request_golden() {
    let auth: Vec<u8> = (0u8..16).collect();
    let b = build_v4(
        50000,
        1812,
        vec![Layer::new("radius")
            .with("identifier", 7u8)
            .with("authenticator", auth)
            .with(
                "attributes",
                vec![
                    named_data("user_name", b"user"),
                    TlvItem::named("nas_ip_address").with("address", Ipv4Addr::new(192, 0, 2, 1)),
                    TlvItem::named("nas_port").with("value", 1u32),
                    TlvItem::named("vendor_specific")
                        .with("vendor_id", 9u32)
                        .with("value", b"test".to_vec()),
                ],
            )],
    );
    let want = hex("01 07 0030 000102030405060708090a0b0c0d0e0f \
        01 06 75736572 \
        04 06 c0000201 \
        05 06 00000001 \
        1a 0a 00000009 74657374");
    assert_eq!(&b[APP..], want.as_slice());
    assert_eq!(roundtrip(&b, "ipv4"), ["ipv4", "udp", "radius"]);
    assert_dissects(&b, Some(0x0800), "radius");
}

#[test]
fn radius_accounting_request_golden() {
    let b = build_v4(
        50000,
        1813,
        vec![Layer::new("radius").with("code", 4u8).with(
            "attributes",
            vec![
                TlvItem::named("acct_status_type").with("value", 1u32),
                named_data("acct_session_id", b"S1"),
            ],
        )],
    );
    let mut want = hex("04 00 001e");
    want.extend([0u8; 16]);
    want.extend(hex("28 06 00000001 2c 04 5331"));
    assert_eq!(&b[APP..], want.as_slice());
    assert_eq!(roundtrip(&b, "ipv4"), ["ipv4", "udp", "radius"]);
}

// --------------------------------------------------------------------------
// TFTP
// --------------------------------------------------------------------------

/// One TFTP option name / value pair.
fn tftp_option(name: &str, value: &str) -> FieldValue {
    FieldValue::Struct(
        [
            (
                "name".to_owned(),
                FieldValue::Bytes(name.as_bytes().to_vec()),
            ),
            (
                "value".to_owned(),
                FieldValue::Bytes(value.as_bytes().to_vec()),
            ),
        ]
        .into(),
    )
}

#[test]
fn tftp_golden() {
    let cases: [(Layer, &str, &str); 6] = [
        (
            Layer::new("tftp"),
            "0001 626f6f742e62696e00 6f6374657400",
            "tftp",
        ),
        (
            Layer::new("tftp_data")
                .with("block", 2u16)
                .with("data", b"hello".to_vec()),
            "0003 0002 68656c6c6f",
            "tftp_data",
        ),
        (
            Layer::new("tftp_ack").with("block", 2u16),
            "0004 0002",
            "tftp_ack",
        ),
        (
            Layer::new("tftp_error"),
            "0005 0001 46696c65206e6f7420666f756e6400",
            "tftp_error",
        ),
        (
            Layer::new("tftp")
                .with("filename", b"pxelinux.0".to_vec())
                .with(
                    "options",
                    vec![tftp_option("blksize", "1428"), tftp_option("tsize", "0")],
                ),
            "0001 7078656c696e75782e3000 6f6374657400 \
             626c6b73697a6500 3134323800 7473697a6500 3000",
            "tftp",
        ),
        (
            Layer::new("tftp_oack"),
            "0006 626c6b73697a6500 3134323800",
            "tftp_oack",
        ),
    ];
    for (layer, want, id) in cases {
        // Port 69 announces `tftp`; DATA / ACK / ERROR / OACK re-decode by opcode.
        let b = build_v4(50000, 69, vec![layer]);
        assert_eq!(&b[APP..], hex(want).as_slice(), "{id}");
        assert_eq!(roundtrip(&b, "ipv4"), ["ipv4", "udp", id]);
        assert_dissects(&b, Some(0x0800), "tftp");
    }
}

// --------------------------------------------------------------------------
// Proptests
// --------------------------------------------------------------------------

fn label() -> impl Strategy<Value = String> {
    "[a-z0-9]{1,20}"
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn dns_header_and_name_roundtrip(
        id in any::<u16>(),
        qr in 0u8..2, opcode in 0u8..16, rcode in 0u8..16,
        rd in 0u8..2, ad in 0u8..2,
        labels in proptest::collection::vec(label(), 1..5),
        qtype in any::<u16>(),
    ) {
        let name = labels.join(".");
        let sections = dns_question(&name, qtype, 1);
        let bytes = Packet::from_layers(vec![
            Layer::new("udp").with("dst_port", 53u16),
            Layer::new("dns")
                .with("id", id).with("qr", qr).with("opcode", opcode)
                .with("rcode", rcode).with("rd", rd).with("ad", ad)
                .with("sections", sections.clone()),
        ]).unpadded().build(catalog()).unwrap().bytes;
        let flags = u16::from_be_bytes([bytes[10], bytes[11]]);
        prop_assert_eq!(flags >> 15, u16::from(qr));
        prop_assert_eq!((flags >> 11) & 0xf, u16::from(opcode));
        prop_assert_eq!(flags & 0xf, u16::from(rcode));
        prop_assert_eq!(&bytes[20..], sections.as_slice());
        // The encoded name is the labels with one length octet each, plus root.
        prop_assert_eq!(dns_name(&name).len(), name.len() + 2);
        let parsed = parse(&bytes, "udp", catalog()).unwrap();
        prop_assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        prop_assert_eq!(&parsed.packet.layers[1].protocol, "dns");
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    }

    #[test]
    fn dhcpv4_options_roundtrip(
        xid in any::<u32>(),
        msg in 1u8..=8,
        lease in any::<u32>(),
        mask in any::<[u8; 4]>(),
        host in proptest::collection::vec(any::<u8>(), 1..32),
        codes in proptest::collection::vec(any::<u8>(), 0..12),
    ) {
        let mut layers = ipv4_udp(68, 67);
        layers.push(Layer::new("dhcpv4").with("xid", xid).with("options", vec![
            TlvItem::named("message_type").with("type", msg),
            TlvItem::named("lease_time").with("seconds", lease),
            TlvItem::named("subnet_mask").with("mask", Ipv4Addr::from(mask)),
            named_data("host_name", host.clone()),
            TlvItem::named("parameter_request_list").with(
                "codes",
                FieldValue::List(codes.iter().map(|c| FieldValue::from(*c)).collect()),
            ),
            TlvItem::named("end"),
        ]));
        let bytes = Packet::from_layers(layers).unpadded().build(catalog()).unwrap().bytes;
        // Options start after the 240-octet fixed part.
        let o = APP + 240;
        prop_assert_eq!(&bytes[o..o + 3], &[53, 1, msg]);
        prop_assert_eq!(*bytes.last().unwrap(), 0xff);
        prop_assert_eq!(bytes.len(), o + 3 + 6 + 6 + 2 + host.len() + 2 + codes.len() + 1);
        let parsed = parse(&bytes, "ipv4", catalog()).unwrap();
        prop_assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        prop_assert_eq!(&parsed.packet.layers[2].protocol, "dhcpv4");
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    }

    #[test]
    fn snmp_messages_roundtrip(
        v1 in any::<bool>(),
        community in "[a-z]{0,16}",
        request_id in 0u64..0x7fff_ffff,
        arcs in proptest::collection::vec(0u64..100_000, 0..6),
    ) {
        let mut oid = vec![1, 3, 6, 1];
        oid.extend(arcs);
        let bytes = Packet::from_layers(vec![
            Layer::new("udp").with("dst_port", 161u16),
            Layer::new("snmp")
                .with("version", u8::from(!v1))
                .with("community_length", u8::try_from(community.len()).unwrap())
                .with("community", community.as_bytes().to_vec()),
            Layer::new("snmp_pdu").with("body", vec![
                TlvItem::with_type(0x02, ber_uint(request_id)),
                TlvItem::with_type(0x02, ber_uint(0)),
                TlvItem::with_type(0x02, ber_uint(0)),
                TlvItem::named("sequence").with("items", vec![
                    TlvItem::named("sequence").with("items", vec![
                        TlvItem::with_type(0x06, ber_oid(&oid)),
                        TlvItem::named("null"),
                    ]),
                ]),
            ]),
        ]).unpadded().build(catalog()).unwrap().bytes;
        let app = &bytes[8..];
        prop_assert_eq!(app[0], 0x30);
        prop_assert_eq!(usize::from(app[1]), app.len() - 2);
        let pdu = 7 + community.len();
        prop_assert_eq!(app[pdu], 0xa0);
        prop_assert_eq!(usize::from(app[pdu + 1]), app.len() - pdu - 2);
        let parsed = parse(&bytes, "udp", catalog()).unwrap();
        prop_assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        let ids: Vec<&str> = parsed.packet.layers.iter().map(|l| l.protocol.as_str()).collect();
        prop_assert_eq!(ids, ["udp", "snmp", "snmp_pdu"]);
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    }

    #[test]
    fn radius_attributes_roundtrip(
        code in prop_oneof![Just(1u8), Just(2), Just(3), Just(4), Just(5), Just(11)],
        identifier in any::<u8>(),
        auth in any::<[u8; 16]>(),
        user in proptest::collection::vec(any::<u8>(), 1..40),
        port in any::<u32>(),
        unknown in proptest::collection::vec(any::<u8>(), 0..20),
    ) {
        let bytes = Packet::from_layers(vec![
            Layer::new("udp").with("dst_port", 1812u16),
            Layer::new("radius")
                .with("code", code).with("identifier", identifier)
                .with("authenticator", auth.to_vec())
                .with("attributes", vec![
                    named_data("user_name", user.clone()),
                    TlvItem::named("nas_port").with("value", port),
                    TlvItem::with_type(200, unknown.clone()),
                ]),
        ]).unpadded().build(catalog()).unwrap().bytes;
        let app = &bytes[8..];
        prop_assert_eq!(usize::from(u16::from_be_bytes([app[2], app[3]])), app.len());
        prop_assert_eq!(app.len(), 20 + 2 + user.len() + 6 + 2 + unknown.len());
        let parsed = parse(&bytes, "udp", catalog()).unwrap();
        prop_assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        prop_assert_eq!(&parsed.packet.layers[1].protocol, "radius");
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    }

    #[test]
    fn tftp_data_roundtrip(
        block in any::<u16>(),
        data in proptest::collection::vec(any::<u8>(), 0..=512),
    ) {
        let bytes = Packet::from_layers(vec![
            Layer::new("udp").with("dst_port", 69u16),
            Layer::new("tftp_data").with("block", block).with("data", data.clone()),
        ]).unpadded().build(catalog()).unwrap().bytes;
        prop_assert_eq!(&bytes[8..10], &[0, 3]);
        prop_assert_eq!(&bytes[12..], data.as_slice());
        let parsed = parse(&bytes, "udp", catalog()).unwrap();
        prop_assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        prop_assert_eq!(&parsed.packet.layers[1].protocol, "tftp_data");
        prop_assert_eq!(parsed.packet.build(catalog()).unwrap().bytes, bytes);
    }
}
