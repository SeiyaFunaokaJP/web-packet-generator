//! Golden packets: each case builds a packet with the current catalog and compares
//! the bytes with `tests/golden/<name>.hex`; the same bytes are also stored as
//! `tests/golden/<name>.pcap` (used by `tests/tshark.rs`).
//!
//! Regenerate with `WPG_UPDATE_GOLDEN=1 cargo test -p wpg-golden-tests --test golden`
//! (or `just golden-update`).
//!
//! To add a protocol: write one `fn case_xxx() -> Packet`, add one line to [`cases`],
//! regenerate, and (optionally) add expectations in `tests/tshark.rs`.

use std::net::Ipv4Addr;

use wpg_core::{FieldValue, Layer, Packet, TlvItem};
use wpg_protocols::{bgp_mp, catalog};
use wpg_testutil::{assert_golden, read_pcap};

fn eth() -> Layer {
    Layer::new("ethernet2")
        .with("dst", FieldValue::Mac([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]))
        .with("src", FieldValue::Mac([0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb]))
}

fn ipv4(id: u16, src: [u8; 4], dst: [u8; 4]) -> Layer {
    Layer::new("ipv4")
        .with("identification", id)
        .with("src", Ipv4Addr::from(src))
        .with("dst", Ipv4Addr::from(dst))
}

fn case_eth_ipv4_udp() -> Packet {
    Packet::from_layers(vec![
        eth(),
        ipv4(0x1234, [10, 0, 0, 1], [10, 0, 0, 2]).with("flags", 0b010u8),
        Layer::new("udp")
            .with("src_port", 1234u16)
            .with("dst_port", 5678u16),
        Layer::new("raw").with("data", b"hello".to_vec()),
    ])
}

fn case_eth_dot1q_ipv4_tcp() -> Packet {
    Packet::from_layers(vec![
        eth(),
        Layer::new("dot1q").with("pcp", 5u8).with("vid", 100u16),
        ipv4(1, [192, 168, 1, 10], [198, 51, 100, 7]).with("ttl", 128u8),
        Layer::new("tcp")
            .with("src_port", 40000u16)
            .with("dst_port", 443u16)
            .with("seq", 0x0102_0304u32)
            .with("window", 64240u16)
            .with(
                "options",
                vec![TlvItem::named("mss").with("value", 1460u16)],
            ),
    ])
}

fn case_eth_ipv4_tcp_syn() -> Packet {
    Packet::from_layers(vec![
        eth(),
        ipv4(0xbeef, [172, 16, 0, 1], [172, 16, 0, 2]),
        Layer::new("tcp")
            .with("src_port", 51000u16)
            .with("dst_port", 80u16)
            .with("seq", 0xdead_beefu32)
            .with("flags", 0x02u8)
            .with("window", 64240u16)
            .with("options", linux_syn_options()),
    ])
}

/// The option list of a Linux SYN: MSS 1460, SACK permitted, timestamps, NOP,
/// window scale 7.
fn linux_syn_options() -> Vec<TlvItem> {
    vec![
        TlvItem::named("mss").with("value", 1460u16),
        TlvItem::named("sack_permitted"),
        TlvItem::named("timestamps")
            .with("tsval", 1u32)
            .with("tsecr", 0u32),
        TlvItem::named("nop"),
        TlvItem::named("ws").with("shift", 7u8),
    ]
}

fn udp(src: u16, dst: u16, payload: &[u8]) -> [Layer; 2] {
    [
        Layer::new("udp")
            .with("src_port", src)
            .with("dst_port", dst),
        Layer::new("raw").with("data", payload.to_vec()),
    ]
}

fn case_eth_qinq_ipv4_udp() -> Packet {
    let mut layers = vec![
        eth(),
        Layer::new("dot1ad").with("vid", 100u16).with("pcp", 3u8),
        Layer::new("dot1q").with("vid", 200u16),
        ipv4(7, [10, 1, 0, 1], [10, 1, 0, 2]),
    ];
    layers.extend(udp(4000, 4001, b"qinq"));
    Packet::from_layers(layers)
}

fn case_dot3_llc_snap_ipv4_udp() -> Packet {
    let mut layers = vec![
        Layer::new("dot3")
            .with("dst", FieldValue::Mac([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]))
            .with("src", FieldValue::Mac([0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb])),
        Layer::new("llc"),
        Layer::new("snap"),
        ipv4(8, [10, 2, 0, 1], [10, 2, 0, 2]),
    ];
    layers.extend(udp(33000, 33001, b"snap"));
    Packet::from_layers(layers)
}

fn case_eth_ipv4_ra_udp() -> Packet {
    let mut layers = vec![
        eth(),
        ipv4(9, [10, 3, 0, 1], [224, 0, 0, 22]).with(
            "options",
            vec![TlvItem::named("router_alert").with("value", 0u16)],
        ),
    ];
    layers.extend(udp(6000, 6001, b"ra"));
    Packet::from_layers(layers)
}

fn case_eth_ipv4_udp_pattern() -> Packet {
    Packet::from_layers(vec![
        eth(),
        ipv4(10, [10, 4, 0, 1], [10, 4, 0, 2]),
        Layer::new("udp")
            .with("src_port", 7000u16)
            .with("dst_port", 7001u16),
        Layer::new("payload")
            .with("mode", 3u8)
            .with("data", b"wpg-".to_vec())
            .with("length", 16u32)
            .with("counter_offset", 8u32)
            .with("counter_value", 0x0102_0304u32),
    ])
}

/// IS-IS L2 LSP over 802.3 + LLC (SAP 0xfe): Fletcher checksum, a wide-metric
/// neighbor and a prefix with a Prefix-SID sub-TLV.
fn case_dot3_llc_isis_lsp() -> Packet {
    let prefix = FieldValue::Struct(
        [
            ("metric".to_owned(), FieldValue::UInt(10)),
            ("prefix_length".to_owned(), FieldValue::UInt(32)),
            ("prefix".to_owned(), FieldValue::Bytes(vec![192, 0, 2, 1])),
            (
                "sub_tlv_block".to_owned(),
                FieldValue::List(vec![FieldValue::Struct(
                    [(
                        "sub_tlvs".to_owned(),
                        FieldValue::Tlv(vec![TlvItem::named("prefix_sid")
                            .with("flags", 0x40u8)
                            .with("sid", vec![0, 0, 0, 1])]),
                    )]
                    .into(),
                )]),
            ),
        ]
        .into(),
    );
    Packet::from_layers(vec![
        Layer::new("dot3")
            .with("dst", FieldValue::Mac([0x01, 0x80, 0xc2, 0, 0, 0x15]))
            .with("src", FieldValue::Mac([0x02, 0, 0, 0, 0, 1])),
        Layer::new("llc"),
        Layer::new("isis_l2_lsp")
            .with("sequence_number", 0x10u32)
            .with(
                "tlvs",
                vec![
                    TlvItem::named("area_addresses").with(
                        "addresses",
                        vec![FieldValue::Struct(
                            [("address".to_owned(), FieldValue::Nsap(vec![0x49, 0, 1]))].into(),
                        )],
                    ),
                    TlvItem::named("dynamic_hostname").with("hostname", b"r1".to_vec()),
                    TlvItem::named("extended_ip_reachability").with("prefixes", vec![prefix]),
                ],
            ),
    ])
}

/// CLNP error report over 802.3 + LLC: header Fletcher checksum, a reason for
/// discard option ending at the length indicator and the discarded header as data.
fn case_dot3_llc_clnp_er() -> Packet {
    Packet::from_layers(vec![
        Layer::new("dot3")
            .with("dst", FieldValue::Mac([0x02, 0, 0, 0, 0, 1]))
            .with("src", FieldValue::Mac([0x02, 0, 0, 0, 0, 3])),
        Layer::new("llc"),
        Layer::new("clnp")
            .with("er", 0u8)
            .with("pdu_type", 0x01u8)
            .with(
                "options",
                vec![TlvItem::named("reason_for_discard")
                    .with("reason", 0x81u8)
                    .with("pointer", 9u8)],
            ),
        Layer::new("raw").with("data", vec![0x81, 0x1f, 0x01, 0x20, 0x3c, 0x00, 0x23]),
    ])
}

/// ES-IS ISH over 802.3 + LLC with a suggested ES configuration timer option.
fn case_dot3_llc_esis_ish() -> Packet {
    Packet::from_layers(vec![
        Layer::new("dot3")
            .with("dst", FieldValue::Mac([0x09, 0x00, 0x2b, 0, 0, 0x04]))
            .with("src", FieldValue::Mac([0x02, 0, 0, 0, 0, 1])),
        Layer::new("llc"),
        Layer::new("esis_ish").with(
            "options",
            vec![TlvItem::named("es_configuration_timer").with("seconds", 600u16)],
        ),
    ])
}

/// SCTP INIT over IPv4: CRC-32C, chunk flags between type and length, an
/// unpadded last parameter covered by the chunk padding.
fn case_eth_ipv4_sctp_init() -> Packet {
    let types = vec![FieldValue::UInt(5)];
    Packet::from_layers(vec![
        eth(),
        ipv4(0x0007, [192, 0, 2, 1], [192, 0, 2, 2]),
        Layer::new("sctp")
            .with("src_port", 36412u16)
            .with("dst_port", 36412u16)
            .with(
                "chunks",
                vec![TlvItem::named("init")
                    .with("initiate_tag", 0x0102_0304u32)
                    .with(
                        "parameters",
                        vec![
                            TlvItem::named("ipv4_address")
                                .with("address", Ipv4Addr::new(192, 0, 2, 1)),
                            TlvItem::named("ecn_capable"),
                            TlvItem::named("supported_address_types").with("types", types),
                        ],
                    )],
            ),
    ])
}

/// SCTP DATA (5 octets of user data, padded) bundled with a SACK.
fn case_eth_ipv4_sctp_data_sack() -> Packet {
    Packet::from_layers(vec![
        eth(),
        ipv4(0x0008, [192, 0, 2, 2], [192, 0, 2, 1]),
        Layer::new("sctp")
            .with("src_port", 5002u16)
            .with("dst_port", 5001u16)
            .with("verification_tag", 0x0102_0304u32)
            .with(
                "chunks",
                vec![
                    TlvItem::named("sack").with("cumulative_tsn_ack", 41u32),
                    TlvItem::named("data")
                        .with("tsn", 7u32)
                        .with("stream_identifier", 1u16)
                        .with("user_data", b"hello".to_vec()),
                ],
            ),
    ])
}

/// LDP link Hello to 224.0.0.2 (UDP 646) with an IPv4 transport address.
fn case_eth_ipv4_udp_ldp_hello() -> Packet {
    Packet::from_layers(vec![
        eth(),
        ipv4(0x000d, [10, 0, 12, 1], [224, 0, 0, 2]).with("ttl", 1u8),
        Layer::new("udp")
            .with("src_port", 646u16)
            .with("dst_port", 646u16),
        Layer::new("ldp").with(
            "messages",
            vec![TlvItem::named("hello").with(
                "tlvs",
                vec![
                    TlvItem::named("common_hello_parameters"),
                    TlvItem::named("ipv4_transport_address")
                        .with("address", Ipv4Addr::new(192, 0, 2, 1)),
                ],
            )],
        ),
    ])
}

/// LDP Label Mapping (prefix FEC 198.51.100.0/24, label 299776) on the TCP session.
fn case_eth_ipv4_tcp_ldp_label_mapping() -> Packet {
    let element = FieldValue::Struct(
        [
            ("prefix_length".to_owned(), FieldValue::UInt(24)),
            ("prefix".to_owned(), FieldValue::Bytes(vec![198, 51, 100])),
        ]
        .into_iter()
        .collect(),
    );
    Packet::from_layers(vec![
        eth(),
        ipv4(0x000e, [192, 0, 2, 1], [192, 0, 2, 2]),
        Layer::new("tcp")
            .with("src_port", 646u16)
            .with("dst_port", 49152u16)
            .with("seq", 1u32)
            .with("ack", 1u32)
            .with("flags", 0x18u8),
        Layer::new("ldp").with(
            "messages",
            vec![TlvItem::named("label_mapping").with(
                "tlvs",
                vec![
                    TlvItem::named("fec").with("elements", FieldValue::List(vec![element])),
                    TlvItem::named("generic_label").with("label", 299_776u32),
                ],
            )],
        ),
    ])
}

/// RSVP-TE Path (the default objects plus an explicit route) with Router Alert.
fn case_eth_ipv4_rsvp_path() -> Packet {
    Packet::from_layers(vec![
        eth(),
        ipv4(0x000f, [192, 0, 2, 1], [192, 0, 2, 2]).with(
            "options",
            vec![TlvItem::named("router_alert").with("value", 0u16)],
        ),
        Layer::new("rsvp").with(
            "objects",
            vec![
                TlvItem::named("session_lsp_tunnel_ipv4"),
                TlvItem::named("rsvp_hop_ipv4"),
                TlvItem::named("time_values"),
                TlvItem::named("explicit_route").with(
                    "subobjects",
                    vec![TlvItem::named("ipv4_prefix")
                        .with("address", Ipv4Addr::new(192, 0, 2, 2))],
                ),
                TlvItem::named("label_request"),
                TlvItem::named("session_attribute"),
                TlvItem::named("sender_template_lsp_tunnel_ipv4"),
                TlvItem::named("sender_tspec"),
            ],
        ),
    ])
}

/// BFD control packet, state Up, with simple password authentication.
fn case_eth_ipv4_udp_bfd() -> Packet {
    Packet::from_layers(vec![
        eth(),
        ipv4(0x0010, [10, 0, 12, 1], [10, 0, 12, 2]).with("ttl", 255u8),
        Layer::new("udp")
            .with("src_port", 49152u16)
            .with("dst_port", 3784u16),
        Layer::new("bfd")
            .with("state", 3u8)
            .with("my_discriminator", 0x101u32)
            .with("your_discriminator", 0x201u32)
            .with("desired_min_tx_interval", 300_000u32)
            .with("required_min_rx_interval", 300_000u32)
            .with(
                "authentication",
                vec![TlvItem::named("simple_password").with("password", b"secret".to_vec())],
            ),
    ])
}

/// OSPFv2 LSU with an area-scope Router Information opaque LSA: SR algorithms,
/// SRGB and SRLB (3-octet SID/Label sub-TLVs padded to 4 octets).
fn case_eth_ipv4_ospfv2_opaque_ri() -> Packet {
    Packet::from_layers(vec![
        eth(),
        ipv4(0x0009, [192, 0, 2, 1], [224, 0, 0, 5]).with("ttl", 1u8),
        Layer::new("ospfv2").with("router_id", Ipv4Addr::new(192, 0, 2, 1)),
        Layer::new("ospfv2_lsu"),
        Layer::new("ospfv2_opaque_lsa"),
        Layer::new("ospfv2_ri_lsa"),
    ])
}

/// OSPFv2 LSU with an Extended Prefix opaque LSA: 192.0.2.1/32 with a node
/// Prefix SID (index 1).
fn case_eth_ipv4_ospfv2_opaque_ext_prefix() -> Packet {
    Packet::from_layers(vec![
        eth(),
        ipv4(0x000a, [192, 0, 2, 1], [224, 0, 0, 5]).with("ttl", 1u8),
        Layer::new("ospfv2").with("router_id", Ipv4Addr::new(192, 0, 2, 1)),
        Layer::new("ospfv2_lsu"),
        Layer::new("ospfv2_opaque_lsa").with("opaque_id", 1u32),
        Layer::new("ospfv2_ext_prefix_lsa"),
    ])
}

/// A BGP UPDATE over TCP 179 with ORIGIN, an empty `AS_PATH` and `mp`.
fn bgp_mp_update(id: u16, mp: Vec<TlvItem>) -> Packet {
    let mut attrs = vec![
        TlvItem::named("origin").with("origin", 0u8),
        TlvItem::named("as_path").with("segments", Vec::<FieldValue>::new()),
    ];
    attrs.extend(mp);
    Packet::from_layers(vec![
        eth(),
        ipv4(id, [192, 0, 2, 1], [192, 0, 2, 2]),
        Layer::new("tcp")
            .with("src_port", 49152u16)
            .with("dst_port", 179u16)
            .with("seq", 1u32)
            .with("ack", 1u32)
            .with("flags", 0x18u8),
        Layer::new("bgp"),
        Layer::new("bgp_update")
            .with("path_attributes", attrs)
            .with("nlri", bgp_mp::prefixes(&[])),
    ])
}

/// BGP UPDATE announcing the VPN-IPv4 route 65001:100:198.51.100.0/24, label 16.
fn case_eth_ipv4_tcp_bgp_vpnv4() -> Packet {
    let next_hop = FieldValue::Struct(
        [
            ("rd".to_owned(), FieldValue::Bytes(vec![0; 8])),
            (
                "address".to_owned(),
                FieldValue::Ipv4(Ipv4Addr::new(192, 0, 2, 1)),
            ),
        ]
        .into_iter()
        .collect(),
    );
    bgp_mp_update(
        0x000b,
        vec![TlvItem::named("mp_reach_vpnv4")
            .with("next_hop", vec![next_hop])
            .with("nlri", vec![bgp_mp::vpn_prefix(24, 16, &[198, 51, 100])])],
    )
}

/// BGP UPDATE with EVPN MAC/IP (VNI 10010) and Inclusive Multicast routes and a
/// PMSI Tunnel attribute.
fn case_eth_ipv4_tcp_bgp_evpn() -> Packet {
    let imet = TlvItem::named("inclusive_multicast")
        .with("rd", bgp_mp::RD.to_vec())
        .with("ethernet_tag", 0u32)
        .with("ip_length", 32u8)
        .with("originating_ip", vec![192, 0, 2, 1]);
    // MP_REACH_NLRI last: tshark's JSON repeats the `path_attribute` key and only
    // the last one survives in `serde_json`.
    bgp_mp_update(
        0x000c,
        vec![
            TlvItem::named("pmsi_tunnel")
                .with("tunnel_type", 6u8)
                .with("mpls_label", 10010u32)
                .with("tunnel_id", vec![192, 0, 2, 1]),
            TlvItem::named("mp_reach_evpn")
                .with("next_hop", vec![192, 0, 2, 1])
                .with(
                    "nlri",
                    vec![
                        bgp_mp::evpn_mac_ip([0x02, 0, 0, 0, 0, 0x10], &[192, 0, 2, 10], 10010),
                        imet,
                    ],
                ),
        ],
    )
}

/// The golden table: `(file stem, packet)`. One line per case.
fn cases() -> Vec<(&'static str, Packet)> {
    vec![
        ("eth_ipv4_udp", case_eth_ipv4_udp()),
        ("eth_dot1q_ipv4_tcp", case_eth_dot1q_ipv4_tcp()),
        ("eth_ipv4_tcp_syn", case_eth_ipv4_tcp_syn()),
        ("eth_qinq_ipv4_udp", case_eth_qinq_ipv4_udp()),
        ("dot3_llc_snap_ipv4_udp", case_dot3_llc_snap_ipv4_udp()),
        ("eth_ipv4_ra_udp", case_eth_ipv4_ra_udp()),
        ("eth_ipv4_udp_counter", case_eth_ipv4_udp_pattern()),
        ("dot3_llc_isis_lsp", case_dot3_llc_isis_lsp()),
        ("dot3_llc_clnp_er", case_dot3_llc_clnp_er()),
        ("dot3_llc_esis_ish", case_dot3_llc_esis_ish()),
        ("eth_ipv4_sctp_init", case_eth_ipv4_sctp_init()),
        ("eth_ipv4_sctp_data_sack", case_eth_ipv4_sctp_data_sack()),
        (
            "eth_ipv4_ospfv2_opaque_ri",
            case_eth_ipv4_ospfv2_opaque_ri(),
        ),
        (
            "eth_ipv4_ospfv2_opaque_ext_prefix",
            case_eth_ipv4_ospfv2_opaque_ext_prefix(),
        ),
        ("eth_ipv4_tcp_bgp_vpnv4", case_eth_ipv4_tcp_bgp_vpnv4()),
        ("eth_ipv4_tcp_bgp_evpn", case_eth_ipv4_tcp_bgp_evpn()),
        ("eth_ipv4_udp_ldp_hello", case_eth_ipv4_udp_ldp_hello()),
        (
            "eth_ipv4_tcp_ldp_label_mapping",
            case_eth_ipv4_tcp_ldp_label_mapping(),
        ),
        ("eth_ipv4_rsvp_path", case_eth_ipv4_rsvp_path()),
        ("eth_ipv4_udp_bfd", case_eth_ipv4_udp_bfd()),
    ]
}

#[test]
fn golden_packets_match() {
    for (name, packet) in cases() {
        let built = packet.build(catalog()).unwrap();
        assert!(
            built.diagnostics.is_empty(),
            "{name}: {:?}",
            built.diagnostics
        );
        assert_golden(name, &built.bytes);
    }
}

#[test]
fn golden_pcaps_contain_the_same_bytes() {
    if std::env::var("WPG_UPDATE_GOLDEN").is_ok_and(|v| v == "1") {
        return; // files are being rewritten by the other test
    }
    for (name, packet) in cases() {
        let built = packet.build(catalog()).unwrap();
        let path = wpg_testutil::golden::golden_dir().join(format!("{name}.pcap"));
        let pkts = read_pcap(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(pkts, vec![built.bytes], "{name}.pcap");
    }
}
