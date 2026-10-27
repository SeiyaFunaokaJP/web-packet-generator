//! Cross-check of the golden pcaps against Wireshark's dissector.
//!
//! Ignored by default; runs with `WPG_TSHARK=1 cargo test -p wpg-golden-tests --test
//! tshark -- --ignored` (or `just tshark`). Requires `tshark` on `PATH`.

use std::path::Path;
use std::process::Command;

use serde_json::Value;
use wpg_testutil::golden::golden_dir;

/// Recursively finds the first value stored under `key`.
fn find<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    match v {
        Value::Object(m) => m.get(key).or_else(|| m.values().find_map(|c| find(c, key))),
        Value::Array(a) => a.iter().find_map(|c| find(c, key)),
        _ => None,
    }
}

/// Collects every `(key, value)` whose key satisfies `pred`.
fn collect<'a>(v: &'a Value, pred: &dyn Fn(&str) -> bool, out: &mut Vec<(&'a str, &'a Value)>) {
    match v {
        Value::Object(m) => {
            for (k, c) in m {
                if pred(k) {
                    out.push((k, c));
                }
                collect(c, pred, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|c| collect(c, pred, out)),
        _ => {}
    }
}

/// Cases whose repeated JSON keys are merged into arrays (`find` then sees the
/// first occurrence instead of only the last).
const MERGE_KEYS: &[&str] = &["eth_ipv4_tcp_diameter_ccr", "eth_ipv4_udp_rtcp_compound"];

fn dissect(pcap: &Path, merge_keys: bool) -> Value {
    let out = Command::new("tshark")
        .arg("-r")
        .arg(pcap)
        // Checksum validation is off by default in Wireshark.
        .args(["-o", "ip.check_checksum:TRUE"])
        .args(["-o", "udp.check_checksum:TRUE"])
        .args(["-o", "tcp.check_checksum:TRUE"])
        .args(["-o", "sctp.checksum:CRC-32C"])
        // RTP / RTCP have no well-known port in Wireshark: decode the IANA
        // `rtp-avp` / `rtcp-avp` ports as they are.
        .args(["-d", "udp.port==5004,rtp", "-d", "udp.port==5005,rtcp"])
        .args(["-T", "json"])
        // Repeated keys (one per AVP) otherwise keep only the last one.
        .args(merge_keys.then_some("--no-duplicate-keys"))
        .output()
        .expect("failed to run tshark (is it installed?)");
    assert!(
        out.status.success(),
        "tshark failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("tshark JSON")
}

fn s(v: Option<&Value>) -> &str {
    v.and_then(Value::as_str).unwrap_or("<missing>")
}

/// Per-case expectations: `(file stem, [(tshark field, expected string)])`.
/// One entry per golden case (required); every pcap also gets the generic checks.
/// `find` sees the last of repeated JSON keys, so for tunnels the inner header wins
/// (`udp.dstport` of a VXLAN frame is the inner one).
fn expectations() -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
    vec![
        (
            "eth_ipv4_udp",
            vec![
                ("eth.type", "0x0800"),
                ("ip.src", "10.0.0.1"),
                ("ip.dst", "10.0.0.2"),
                ("ip.id", "0x1234"),
                ("udp.srcport", "1234"),
                ("udp.dstport", "5678"),
                ("udp.length", "13"),
                ("data.data", "68:65:6c:6c:6f"),
            ],
        ),
        (
            "eth_dot1q_ipv4_tcp",
            vec![
                ("eth.type", "0x8100"),
                ("vlan.id", "100"),
                ("vlan.priority", "5"),
                ("vlan.etype", "0x0800"),
                ("ip.ttl", "128"),
                ("tcp.srcport", "40000"),
                ("tcp.dstport", "443"),
                ("tcp.hdr_len", "24"),
            ],
        ),
        (
            "eth_ipv4_tcp_syn",
            vec![
                ("eth.type", "0x0800"),
                ("tcp.flags.syn", "1"),
                ("tcp.flags.ack", "0"),
                ("tcp.dstport", "80"),
                ("tcp.hdr_len", "40"),
                ("tcp.options.mss_val", "1460"),
                ("tcp.options.wscale.shift", "7"),
                ("tcp.options.timestamp.tsval", "1"),
                ("tcp.options.timestamp.tsecr", "0"),
            ],
        ),
        (
            "eth_qinq_ipv4_udp",
            vec![
                ("eth.type", "0x88a8"),
                ("ieee8021ad.id", "100"),
                ("ieee8021ad.priority", "3"),
                ("vlan.id", "200"),
                ("udp.dstport", "4001"),
                ("ip.dst", "10.1.0.2"),
            ],
        ),
        (
            "dot3_llc_snap_ipv4_udp",
            vec![
                ("eth.len", "40"),
                ("llc.dsap", "0xaa"),
                ("llc.ssap", "0xaa"),
                ("llc.control", "0x0003"),
                ("llc.type", "0x0800"),
                ("ip.dst", "10.2.0.2"),
                ("udp.dstport", "33001"),
            ],
        ),
        (
            "eth_ipv4_ra_udp",
            vec![
                ("ip.hdr_len", "24"),
                ("ip.opt.type", "148"),
                ("ip.opt.ra", "0"),
                ("udp.dstport", "6001"),
            ],
        ),
        (
            "eth_ipv4_udp_counter",
            vec![
                ("udp.length", "24"),
                (
                    "data.data",
                    "77:70:67:2d:00:00:00:00:01:02:03:04:00:00:00:00",
                ),
            ],
        ),
        (
            "dot3_llc_isis_lsp",
            vec![
                ("llc.dsap", "0xfe"),
                ("isis.type", "20"),
                ("isis.lsp.pdu_length", "57"),
                ("isis.lsp.sequence_number", "0x00000010"),
                ("isis.lsp.checksum.status", "1"),
                ("isis.lsp.hostname", "r1"),
                ("isis.lsp.ext_ip_reachability.prefix_length", "32"),
            ],
        ),
        (
            "dot3_llc_clnp_er",
            vec![
                ("llc.dsap", "0xfe"),
                ("clnp.len", "35"),
                ("clnp.type", "1"),
                ("clnp.pdu.len", "42"),
                ("clnp.checksum.status", "1"),
                ("clnp.dsap", "49:00:01:00:00:00:00:00:02:00"),
            ],
        ),
        (
            "dot3_llc_esis_ish",
            vec![
                ("esis.length", "24"),
                ("esis.type", "4"),
                ("esis.chksum.status", "1"),
                ("esis.net", "49000100.000000000100"),
            ],
        ),
        (
            "eth_ipv4_sctp_init",
            vec![
                ("ip.proto", "132"),
                ("sctp.srcport", "36412"),
                ("sctp.verification_tag", "0x00000000"),
                ("sctp.chunk_type", "1"),
                ("sctp.chunk_length", "38"),
                ("sctp.init_initiate_tag", "0x01020304"),
                ("sctp.parameter_ipv4_address", "192.0.2.1"),
                ("sctp.parameter_supported_address_type", "5"),
            ],
        ),
        (
            "eth_ipv4_sctp_data_sack",
            vec![
                ("sctp.verification_tag", "0x01020304"),
                ("sctp.sack_number_of_gap_blocks", "0"),
                ("sctp.data_tsn_raw", "7"),
                ("sctp.data_sid", "0x0001"),
                ("sctp.data_payload_proto_id", "0"),
                ("data.data", "68:65:6c:6c:6f"),
            ],
        ),
        (
            "eth_ipv4_ospfv2_opaque_ri",
            vec![
                ("ospf.lsa", "10"),
                ("ospf.lsid_opaque_type", "4"),
                ("ospf.lsa.length", "68"),
                ("ospf.tlv.range_size", "8000"),
                ("ospf.tlv.sid_label", "16000"),
            ],
        ),
        (
            "eth_ipv4_ospfv2_opaque_ext_prefix",
            vec![
                ("ospf.lsid_opaque_type", "7"),
                ("ospf.lsid.opaque_id", "1"),
                ("ospf.tlv.extpfx.routetype", "1"),
                ("ospf.tlv.extpfx.flags.n", "1"),
                ("ospf.v3.address_prefix.ipv4", "192.0.2.1"),
                ("ospf.tlv.extpfx.subtlv_type", "2"),
                ("ospf.tlv.sid_label", "1"),
            ],
        ),
        (
            "eth_ipv4_tcp_bgp_vpnv4",
            vec![
                ("bgp.update.path_attribute.mp_reach_nlri.safi", "128"),
                (
                    "bgp.update.path_attribute.mp_reach_nlri.next_hop.ipv4",
                    "192.0.2.1",
                ),
                ("bgp.prefix_length", "112"),
                ("bgp.label_stack", "16 (bottom)"),
                ("bgp.rd", "65001:100"),
                ("bgp.mp_reach_nlri_ipv4_prefix", "198.51.100.0"),
            ],
        ),
        (
            "eth_ipv4_tcp_bgp_evpn",
            vec![
                ("bgp.update.path_attribute.mp_reach_nlri.afi", "25"),
                ("bgp.update.path_attribute.mp_reach_nlri.safi", "70"),
                // Repeated JSON keys: only the last route (Inclusive Multicast)
                // is visible here.
                ("bgp.evpn.nlri.rt", "3"),
                ("bgp.evpn.nlri.len", "17"),
                ("bgp.evpn.nlri.ip.addr", "192.0.2.1"),
            ],
        ),
        (
            "eth_ipv4_udp_ldp_hello",
            vec![
                ("udp.dstport", "646"),
                ("ldp.hdr.pdu_len", "30"),
                ("ldp.hdr.ldpid.lsr", "192.0.2.1"),
                ("ldp.msg.type", "0x0100"),
                ("ldp.msg.len", "20"),
                ("ldp.msg.tlv.hello.hold", "15"),
                ("ldp.msg.tlv.ipv4.taddr", "192.0.2.1"),
            ],
        ),
        (
            "eth_ipv4_tcp_ldp_label_mapping",
            vec![
                ("ldp.hdr.pdu_len", "33"),
                ("ldp.msg.type", "0x0400"),
                ("ldp.msg.len", "23"),
                ("ldp.msg.tlv.fec.type", "2"),
                ("ldp.msg.tlv.fec.len", "24"),
                ("ldp.msg.tlv.fec.pfval", "198.51.100.0"),
                ("ldp.msg.tlv.generic.label", "299776"),
            ],
        ),
        (
            "eth_ipv4_rsvp_path",
            vec![
                ("ip.proto", "46"),
                ("ip.opt.ra", "0"),
                ("rsvp.msg", "1"),
                ("rsvp.message_length", "128"),
                ("rsvp.session.tunnel_id", "1"),
                ("rsvp.ero_rro_subobjects.ipv4_hop", "192.0.2.2"),
                ("rsvp.refresh_interval", "30000"),
            ],
        ),
        (
            "eth_ipv4_udp_bfd",
            vec![
                ("udp.dstport", "3784"),
                ("bfd.sta", "0x03"),
                ("bfd.flags.a", "1"),
                ("bfd.message_length", "33"),
                ("bfd.my_discriminator", "0x00000101"),
                ("bfd.desired_min_tx_interval", "300000"),
                ("bfd.auth.password", "secret"),
            ],
        ),
        (
            "eth_ptp_sync",
            vec![
                ("eth.type", "0x88f7"),
                ("ptp.v2.messagetype", "0x00"),
                ("ptp.v2.messagelength", "44"),
                ("ptp.v2.flags.twostep", "1"),
                ("ptp.v2.clockidentity", "0x020000fffe000001"),
                ("ptp.v2.sequenceid", "7"),
                ("ptp.v2.logmessageperiod", "-3"),
                ("ptp.v2.sdr.origintimestamp.seconds", "1700000000"),
            ],
        ),
        (
            "eth_ipv4_udp_ptp_announce",
            vec![
                ("udp.dstport", "320"),
                ("ptp.v2.messagetype", "0x0b"),
                ("ptp.v2.messagelength", "64"),
                ("ptp.v2.flags.utcreasonable", "1"),
                ("ptp.v2.controlfield", "5"),
                ("ptp.v2.an.origincurrentutcoffset", "37"),
                ("ptp.v2.an.priority1", "100"),
                ("ptp.v2.an.grandmasterclockclass", "6"),
                ("ptp.v2.an.grandmasterclockaccuracy", "0x21"),
                ("ptp.v2.an.grandmasterclockvariance", "20061"),
            ],
        ),
        (
            "eth_esmc",
            vec![
                ("slow.subtype", "0x0a"),
                ("ossp.itu.subtype", "0x0001"),
                ("ossp.esmc.version", "0x01"),
                ("ossp.esmc.ql", "0xff02"), // enhanced SSM 0xff, QL-PRC
                ("ossp.esmc.tlv_ext_ql_essm", "0xff"),
                ("ossp.esmc.tlv_ext_ql_clockid", "0x020000fffe000001"),
                ("ossp.esmc.tlv_ext_ql_eeec", "1"),
            ],
        ),
        (
            "eth_dot1q_cfm_ccm",
            vec![
                ("vlan.id", "100"),
                ("cfm.md_level", "5"),
                ("cfm.opcode", "1"),
                ("cfm.ccm.flags.interval", "4"),
                ("cfm.first_tlv_offset", "70"),
                ("cfm.mep_id", "10"),
                ("cfm.maid.ma_name.icc", "WPG001MEG0001"),
                ("cfm.tlv.port_status.value", "2"),
                ("cfm.tlv.intf_status.value", "1"),
            ],
        ),
        (
            "eth_cfm_raps",
            vec![
                ("cfm.md_level", "7"),
                ("cfm.version", "1"),
                ("cfm.opcode", "40"),
                ("cfm.first_tlv_offset", "32"),
                ("cfm.raps.req_st", "0x0b"),
                ("cfm.raps.status.bpr", "1"),
                ("cfm.raps.node_id", "02:00:00:00:00:05"),
            ],
        ),
        (
            "eth_cfm_dmm",
            vec![
                ("cfm.opcode", "47"),
                ("cfm.first_tlv_offset", "32"),
                ("cfm.odm.dmm.dmr.txtimestampf", "1700000000.250000000"),
                ("cfm.tlv.type", "0"),
            ],
        ),
        (
            "eth_efm_oam_information",
            vec![
                ("slow.subtype", "0x03"),
                ("oampdu.flags", "0x0050"),
                ("oampdu.code", "0x00"),
                // Duplicate JSON keys: the last (Remote Information) TLV wins.
                ("oampdu.info.type", "0x02"),
                ("oampdu.info.length", "16"),
                ("oampdu.info.oamConfig", "0x1d"),
                ("oampdu.info.oampduConfig", "1518"),
            ],
        ),
        (
            "eth_ipv6_srh_udp",
            vec![
                ("ipv6.routing.type", "4"),
                ("ipv6.routing.segleft", "2"),
                ("ipv6.routing.srh.last_entry", "2"),
                ("ipv6.routing.srh.tag", "00:10"),
                ("udp.dstport", "40001"),
            ],
        ),
        (
            "eth_ipv6_srh_hmac_ipv4",
            vec![
                ("ipv6.routing.segleft", "1"),
                ("ipv6.routing.srh.last_entry", "1"),
                ("ipv6.nxt", "43"),
                ("ip.dst", "10.2.0.1"),
                ("udp.dstport", "4000"),
            ],
        ),
        (
            "eth_mpls_sr_eli_ipv4_udp",
            vec![
                ("mpls.label", "344865"), // the last (entropy) label wins in the JSON
                ("ip.dst", "10.0.0.2"),
            ],
        ),
        (
            "eth_macsec",
            vec![
                ("eth.type", "0x88e5"),
                ("macsec.TCI.SC", "1"),
                ("macsec.TCI.E", "1"),
                ("macsec.TCI.C", "1"),
                ("macsec.AN", "0x01"),
                ("macsec.SL", "0"),
                ("macsec.PN", "16909060"),
                ("macsec.SCI.system_identifier", "02:00:00:00:00:01"),
                ("macsec.SCI.port_identifier", "1"),
                (
                    "macsec.ICV",
                    "a5:a5:a5:a5:a5:a5:a5:a5:a5:a5:a5:a5:a5:a5:a5:a5",
                ),
            ],
        ),
        (
            "eth_ipv4_udp_gtpv2_create_session",
            vec![
                ("udp.dstport", "2123"),
                ("gtpv2.flags", "0x48"),
                ("gtpv2.message_type", "32"),
                ("gtpv2.msg_length", "129"),
                ("gtpv2.teid", "0x00000000"),
                ("gtpv2.seq", "0x001234"),
                ("e212.imsi", "001010123456789"),
                ("gtpv2.rat_type", "6"),
                ("gtpv2.f_teid_interface_type", "10"),
                ("gtpv2.f_teid_gre_key", "0x00001001"),
                ("gtpv2.f_teid_ipv4", "192.0.2.1"),
                ("gtpv2.apn", "internet"),
                ("gtpv2.pdn_addr_and_prefix.ipv4", "0.0.0.0"),
                ("gtpv2.ambr_up", "50000"),
                ("gtpv2.ambr_down", "100000"),
                ("gtpv2.ebi", "5"),
                ("gtpv2.bearer_qos_pl", "9"),
                ("gtpv2.bearer_qos_label_qci", "9"),
            ],
        ),
        (
            "eth_ipv4_tcp_diameter_ccr",
            vec![
                ("tcp.dstport", "3868"),
                ("diameter.length", "236"),
                ("diameter.flags", "0xc0"),
                ("diameter.cmd.code", "272"),
                ("diameter.applicationId", "4"),
                ("diameter.hopbyhopid", "0x22334455"),
                ("diameter.endtoendid", "0x66778899"),
                ("diameter.Session-Id", "client.example.com;1;1"),
                ("diameter.Origin-Host", "client.example.com"),
                ("diameter.Destination-Realm", "example.net"),
                ("diameter.Service-Context-Id", "32251@3gpp.org"),
                ("diameter.CC-Request-Type", "1"),
                ("diameter.Subscription-Id-Data", "819012345678"),
                ("diameter.avp.vendorId", "10415"),
                ("diameter.RAT-Type", "1004"),
            ],
        ),
        (
            "eth_ipv4_udp_sip_invite",
            vec![
                ("udp.dstport", "5060"),
                ("sip.Method", "INVITE"),
                ("sip.r-uri.user", "bob"),
                ("sip.Via.branch", "z9hG4bK74bf9"),
                ("sip.CSeq.seq", "1"),
                ("sip.Content-Type", "application/sdp"),
                ("sip.Content-Length", "132"),
                ("sdp.owner.username", "alice"),
                ("sdp.media.port", "49170"),
                ("sdp.media.proto", "RTP/AVP"),
            ],
        ),
        (
            "eth_ipv4_udp_rtp",
            vec![
                ("udp.dstport", "5004"),
                ("rtp.version", "2"),
                ("rtp.ext", "1"),
                ("rtp.cc", "2"),
                ("rtp.marker", "1"),
                ("rtp.p_type", "0"),
                ("rtp.seq", "1000"),
                ("rtp.timestamp", "160000"),
                ("rtp.ssrc", "0x12345678"),
                // Repeated key: the last CSRC.
                ("rtp.csrc.item", "0x22222222"),
                ("rtp.ext.profile", "0xbede"),
                ("rtp.ext.len", "1"),
                ("rtp.ext.rfc5285.id", "1"),
                ("rtp.ext.rfc5285.data", "ab"),
            ],
        ),
        (
            "eth_ipv4_udp_rtcp_compound",
            vec![
                ("udp.dstport", "5005"),
                // The first packet of the compound: the Receiver Report.
                ("rtcp.pt", "201"),
                ("rtcp.rc", "1"),
                ("rtcp.length", "7"),
                ("rtcp.senderssrc", "0x12345678"),
                ("rtcp.ssrc.fraction", "10"),
                ("rtcp.ssrc.cum_nr", "25"),
                ("rtcp.ssrc.jitter", "42"),
                ("rtcp.ssrc.dlsr", "65536"),
                ("rtcp.sdes.text", "user@192.0.2.1"),
                ("rtcp.sc", "1"),
                ("rtcp.length_check", "1"),
            ],
        ),
        // P0 / P1 protocols (cases built from the bundled per-protocol samples).
        (
            "dot3_llc",
            vec![
                ("eth.len", "46"),
                ("llc.dsap", "0x0e"),
                ("llc.ssap", "0x0e"),
                ("llc.control", "0x0003"),
                ("data.len", "43"),
            ],
        ),
        (
            "dot3_llc_stp",
            vec![
                ("llc.dsap", "0x42"),
                ("stp.version", "0"),
                ("stp.type", "0x00"),
                ("stp.root.hw", "02:00:00:00:00:01"),
                ("stp.port", "0x8001"),
                ("stp.max_age", "20"),
                ("stp.forward", "15"),
            ],
        ),
        (
            "dot3_llc_rstp",
            vec![
                ("stp.version", "2"),
                ("stp.type", "0x02"),
                ("stp.flags", "0x7e"),
                ("stp.flags.proposal", "1"),
                ("stp.flags.port_role", "3"),
                ("stp.flags.agreement", "1"),
                ("stp.version_1_length", "0"),
            ],
        ),
        (
            "dot3_llc_mstp",
            vec![
                ("stp.version", "3"),
                ("stp.type", "0x02"),
                ("stp.version_1_length", "0"),
                ("mstp.version_3_length", "64"),
                ("mstp.config_name", "region1"),
                ("mstp.config_revision_level", "1"),
                ("mstp.cist_bridge.hw", "02:00:00:00:00:01"),
                ("mstp.cist_remaining_hops", "20"),
            ],
        ),
        (
            "dot3_llc_snap_cdp",
            vec![
                ("llc.oui", "12"),
                ("llc.cisco_pid", "0x2000"),
                ("cdp.version", "2"),
                ("cdp.ttl", "180"),
                ("cdp.deviceid", "sw1"),
                ("cdp.portid", "Gi0/1"),
                ("cdp.capabilities.switch", "1"),
            ],
        ),
        (
            "eth_arp_request",
            vec![
                ("eth.type", "0x0806"),
                ("arp.opcode", "1"),
                ("arp.src.hw_mac", "02:00:00:00:00:01"),
                ("arp.src.proto_ipv4", "192.0.2.1"),
                ("arp.dst.proto_ipv4", "192.0.2.2"),
            ],
        ),
        (
            "eth_eapol",
            vec![
                ("eth.type", "0x888e"),
                ("eapol.version", "2"),
                ("eapol.type", "0"),
                ("eapol.len", "10"),
                ("eap.code", "2"),
                ("eap.type", "1"),
                ("eap.identity", "user1"),
            ],
        ),
        (
            "eth_lacp",
            vec![
                ("slow.subtype", "0x01"),
                ("lacp.version", "0x01"),
                ("lacp.actor.sysid", "02:00:00:00:00:01"),
                ("lacp.actor.key", "1"),
                ("lacp.actor.state", "0x45"),
                ("lacp.partner.sysid", "02:00:00:00:00:02"),
                ("lacp.collector.max_delay", "0"),
            ],
        ),
        (
            "eth_lldp",
            vec![
                ("eth.type", "0x88cc"),
                ("lldp.chassis.id.mac", "02:00:00:00:00:01"),
                ("lldp.port.subtype", "3"),
                ("lldp.time_to_live", "120"),
                ("lldp.tlv.system.name", "sw1"),
            ],
        ),
        (
            "eth_pppoe_discovery",
            vec![
                ("eth.type", "0x8863"),
                ("pppoe.version", "1"),
                ("pppoe.code", "0x09"),
                ("pppoe.session_id", "0x0000"),
                ("pppoe.payload_length", "12"),
                ("pppoed.tags.host_uniq", "de:ad:be:ef"),
            ],
        ),
        (
            "eth_pppoe_ppp_lcp",
            vec![
                ("eth.type", "0x8864"),
                ("pppoe.session_id", "0x0001"),
                ("pppoe.payload_length", "16"),
                ("ppp.protocol", "0xc021"),
                ("ppp.code", "1"),
                ("ppp.length", "14"),
                ("lcp.opt.mru", "1492"),
                ("lcp.opt.magic_number", "0x12345678"),
            ],
        ),
        (
            "eth_ipv4_icmpv4_echo",
            vec![
                ("ip.proto", "1"),
                ("icmp.type", "8"),
                ("icmp.code", "0"),
                ("icmp.checksum.status", "1"),
                ("icmp.ident", "4660"),
                ("icmp.seq", "0"),
                ("data.len", "32"),
            ],
        ),
        (
            "eth_ipv6_ext_chain",
            vec![
                ("eth.type", "0x86dd"),
                ("ipv6.nxt", "0"),
                ("ipv6.plen", "88"),
                ("ipv6.hopopts.nxt", "60"),
                ("ipv6.dstopts.nxt", "43"),
                ("ipv6.routing.nxt", "44"),
                ("ipv6.routing.type", "2"),
                ("ipv6.fraghdr.nxt", "17"),
                ("ipv6.fraghdr.ident", "0x00000001"),
                ("udp.dstport", "40001"),
                ("udp.length", "40"),
            ],
        ),
        (
            "eth_ipv6_udp",
            vec![
                ("ipv6.nxt", "17"),
                ("ipv6.plen", "40"),
                ("ipv6.src", "2001:db8::1"),
                ("ipv6.dst", "2001:db8::2"),
                ("udp.dstport", "40001"),
                ("udp.length", "40"),
                ("data.len", "32"),
            ],
        ),
        (
            "eth_ipv6_icmpv6_echo",
            vec![
                ("ipv6.nxt", "58"),
                ("icmpv6.type", "128"),
                ("icmpv6.checksum.status", "1"),
                ("icmpv6.echo.identifier", "0x1234"),
                ("icmpv6.echo.sequence_number", "1"),
                ("data.len", "32"),
            ],
        ),
        (
            "eth_ipv6_ndp_ns",
            vec![
                ("icmpv6.type", "135"),
                ("icmpv6.checksum.status", "1"),
                ("icmpv6.nd.ns.target_address", "2001:db8::2"),
                ("icmpv6.opt.type", "1"),
                ("icmpv6.opt.src_linkaddr", "02:00:00:00:00:01"),
            ],
        ),
        (
            "eth_ipv6_ndp_ra",
            vec![
                ("icmpv6.type", "134"),
                ("icmpv6.checksum.status", "1"),
                ("icmpv6.nd.ra.cur_hop_limit", "64"),
                ("icmpv6.nd.ra.router_lifetime", "1800"),
                ("icmpv6.opt.prefix.length", "64"),
                ("icmpv6.opt.prefix.flag", "0xc0"),
                ("icmpv6.opt.prefix.valid_lifetime", "2592000"),
                ("icmpv6.opt.prefix", "2001:db8:1::"),
            ],
        ),
        (
            "eth_ipv4_udp_vxlan",
            vec![
                ("vxlan.flags", "0x0800"),
                ("vxlan.flag_i", "1"),
                ("vxlan.vni", "1193046"),
                ("data.len", "16"),
            ],
        ),
        (
            "eth_ipv4_gre_ipv4",
            vec![
                ("gre.flags_and_version", "0x0000"),
                ("gre.proto", "0x0800"),
                ("icmp.type", "8"),
                ("icmp.checksum.status", "1"),
                ("icmp.seq", "1"),
            ],
        ),
        (
            "eth_ipv4_nvgre",
            vec![
                ("gre.flags_and_version", "0x2000"),
                ("gre.flags.key", "1"),
                ("gre.proto", "0x6558"),
                ("gre.key", "0x12345600"),
                ("data.len", "16"),
            ],
        ),
        (
            "eth_mpls_ipv4",
            vec![
                ("eth.type", "0x8847"),
                ("mpls.label", "200"),
                ("mpls.bottom", "1"),
                ("mpls.ttl", "64"),
                ("data.len", "16"),
            ],
        ),
        (
            "eth_ipv4_udp_mpls_in_udp",
            vec![
                ("mpls.label", "100"),
                ("mpls.bottom", "1"),
                ("mpls.ttl", "64"),
                ("data.len", "16"),
            ],
        ),
        (
            "eth_ipv4_udp_l2tpv2",
            vec![
                ("udp.dstport", "1701"),
                ("l2tp.flags", "0x4002"),
                ("l2tp.version", "2"),
                ("l2tp.length", "20"),
                ("l2tp.tunnel", "1"),
                ("l2tp.session", "2"),
                ("ppp.protocol", "0xc021"),
                ("ppp.code", "9"),
            ],
        ),
        (
            "eth_ipv4_l2tpv3",
            vec![
                ("ip.proto", "115"),
                ("l2tp.sid", "0x11223344"),
                ("data.len", "16"),
            ],
        ),
        (
            "eth_ipv4_udp_gtpu",
            vec![
                ("gtp.flags.version", "1"),
                ("gtp.message", "0xff"),
                ("gtp.length", "44"),
                ("gtp.teid", "0x00000001"),
                ("data.len", "16"),
            ],
        ),
        (
            "eth_ipv4_udp_geneve",
            vec![
                ("geneve.version", "0"),
                ("geneve.option.length", "0"),
                ("geneve.vni", "0x123456"),
                ("data.len", "16"),
            ],
        ),
        (
            "eth_ipv4_ipip",
            vec![
                ("ip.proto", "1"),
                ("icmp.type", "8"),
                ("icmp.checksum.status", "1"),
                ("icmp.ident", "1"),
                ("data.len", "8"),
            ],
        ),
        (
            "eth_ipv4_ospfv2_hello",
            vec![
                ("ip.proto", "89"),
                ("ospf.version", "2"),
                ("ospf.msg", "1"),
                ("ospf.packet_length", "48"),
                ("ospf.srcrouter", "192.0.2.1"),
                ("ospf.hello.network_mask", "255.255.255.0"),
                ("ospf.hello.router_dead_interval", "40"),
                ("ospf.hello.backup_designated_router", "192.0.2.2"),
            ],
        ),
        (
            "eth_ipv6_ospfv3_hello",
            vec![
                ("ipv6.nxt", "89"),
                ("ospf.version", "3"),
                ("ospf.msg", "1"),
                ("ospf.packet_length", "40"),
                ("ospf.hello.interface_id", "5"),
                ("ospf.v3.options", "0x000013"),
                ("ospf.hello.active_neighbor", "192.0.2.2"),
            ],
        ),
        (
            "eth_ipv4_tcp_bgp_open",
            vec![
                ("tcp.dstport", "179"),
                ("bgp.type", "1"),
                ("bgp.length", "45"),
                ("bgp.open.version", "4"),
                ("bgp.open.myas", "65001"),
                ("bgp.open.holdtime", "180"),
                ("bgp.open.identifier", "192.0.2.1"),
                ("bgp.cap.4as", "65001"),
            ],
        ),
        (
            "eth_ipv4_tcp_bgp_update",
            vec![
                ("bgp.type", "2"),
                ("bgp.length", "61"),
                ("bgp.update.path_attributes.length", "34"),
                ("bgp.update.path_attribute.community_as", "65001"),
                ("bgp.update.path_attribute.community_value", "100"),
                ("bgp.prefix_length", "24"),
                ("bgp.nlri_prefix", "198.51.100.0"),
            ],
        ),
        (
            "eth_ipv4_udp_ripv2",
            vec![
                ("udp.dstport", "520"),
                ("rip.command", "2"),
                ("rip.version", "2"),
            ],
        ),
        (
            "eth_ipv6_udp_ripng",
            vec![
                ("udp.dstport", "521"),
                ("ripng.cmd", "2"),
                ("ripng.version", "1"),
                ("ripng.rte.ipv6_prefix", "2001:db8:1::"),
                ("ripng.rte.prefix_length", "64"),
                ("ripng.rte.metric", "1"),
            ],
        ),
        (
            "eth_ipv4_eigrp_hello",
            vec![
                ("ip.proto", "88"),
                ("eigrp.version", "2"),
                ("eigrp.opcode", "5"),
                ("eigrp.checksum.status", "1"),
                ("eigrp.as", "100"),
                ("eigrp.par.k1", "1"),
                ("eigrp.par.k3", "1"),
                ("eigrp.par.holdtime", "15"),
            ],
        ),
        (
            "eth_ipv4_vrrpv2",
            vec![
                ("ip.proto", "112"),
                ("vrrp.version", "2"),
                ("vrrp.virt_rtr_id", "10"),
                ("vrrp.prio", "200"),
                ("vrrp.addr_count", "2"),
                ("vrrp.checksum.status", "1"),
            ],
        ),
        (
            "eth_ipv4_vrrpv3",
            vec![
                ("vrrp.version", "3"),
                ("vrrp.virt_rtr_id", "10"),
                ("vrrp.prio", "200"),
                ("vrrp.short_adver_int", "100"),
                ("vrrp.checksum.status", "1"),
                ("vrrp.ip_addr", "192.0.2.254"),
            ],
        ),
        (
            "eth_ipv4_udp_hsrpv1",
            vec![
                ("udp.dstport", "1985"),
                ("hsrp.version", "0"),
                ("hsrp.state", "16"),
                ("hsrp.priority", "110"),
                ("hsrp.group", "1"),
                ("hsrp.auth_data", "cisco"),
                ("hsrp.virt_ip", "192.0.2.254"),
            ],
        ),
        (
            "eth_ipv4_udp_hsrpv2",
            vec![
                ("hsrp2.version", "2"),
                ("hsrp2.state", "6"),
                ("hsrp2.group", "1"),
                ("hsrp2.priority", "110"),
                ("hsrp2.hellotime", "3000"),
                ("hsrp2.virt_ip", "192.0.2.254"),
            ],
        ),
        (
            "eth_ipv4_udp_glbp",
            vec![
                ("udp.dstport", "3222"),
                ("glbp.version", "1"),
                ("glbp.group", "1"),
                ("glbp.ownerid", "02:00:00:00:00:02"),
                ("glbp.hello.priority", "110"),
                ("glbp.hello.virtualipv4", "192.0.2.254"),
            ],
        ),
        (
            "eth_ipv4_igmpv2_query",
            vec![
                ("ip.proto", "2"),
                ("igmp.version", "2"),
                ("igmp.type", "0x11"),
                ("igmp.max_resp", "100"),
                ("igmp.checksum.status", "1"),
                ("igmp.maddr", "0.0.0.0"),
            ],
        ),
        (
            "eth_ipv4_igmpv3_report",
            vec![
                ("igmp.version", "3"),
                ("igmp.type", "0x22"),
                ("igmp.checksum.status", "1"),
                ("igmp.num_grp_recs", "2"),
            ],
        ),
        (
            "eth_ipv6_mldv1_query",
            vec![
                ("ipv6.hopopts.nxt", "58"),
                ("icmpv6.type", "130"),
                ("icmpv6.checksum.status", "1"),
                ("icmpv6.mld.maximum_response_delay", "10000"),
                ("icmpv6.mld.multicast_address", "::"),
            ],
        ),
        (
            "eth_ipv6_mldv2_report",
            vec![
                ("icmpv6.type", "143"),
                ("icmpv6.checksum.status", "1"),
                ("icmpv6.mldr.nb_mcast_records", "1"),
                ("icmpv6.mldr.mar.record_type", "4"),
                ("icmpv6.mldr.mar.multicast_address", "ff05::1"),
            ],
        ),
        (
            "eth_ipv4_pim_hello",
            vec![
                ("ip.proto", "103"),
                ("pim.version", "2"),
                ("pim.type", "0"),
                ("pim.cksum.status", "1"),
                ("pim.holdtime", "105"),
                ("pim.dr_priority", "1"),
                ("pim.generation_id", "305419896"),
            ],
        ),
        (
            "eth_ipv4_udp_dhcpv4_discover",
            vec![
                ("udp.dstport", "67"),
                ("dhcp.type", "1"),
                ("dhcp.id", "0x3903f326"),
                ("dhcp.flags.bc", "1"),
                ("dhcp.hw.mac_addr", "02:00:00:00:00:01"),
                ("dhcp.cookie", "99.130.83.99"),
                ("dhcp.option.end", "255"),
            ],
        ),
        (
            "eth_ipv6_udp_dhcpv6_solicit",
            vec![
                ("udp.dstport", "547"),
                ("dhcpv6.msgtype", "1"),
                ("dhcpv6.xid", "0x5a1b2c"),
                ("dhcpv6.requested_option_code", "24"),
            ],
        ),
        (
            "eth_ipv4_udp_dns_query",
            vec![
                ("udp.dstport", "53"),
                ("dns.id", "0x1234"),
                ("dns.flags", "0x0100"),
                ("dns.count.queries", "1"),
                ("dns.qry.name", "example.com"),
                ("dns.qry.type", "1"),
                ("dns.qry.class", "0x0001"),
            ],
        ),
        (
            "eth_ipv4_udp_dns_response",
            vec![
                ("dns.id", "0x1234"),
                ("dns.flags", "0x8580"),
                ("dns.count.answers", "1"),
                ("dns.resp.name", "example.com"),
                ("dns.resp.ttl", "3600"),
                ("dns.a", "192.0.2.10"),
            ],
        ),
        (
            "eth_ipv4_udp_ntp",
            vec![
                ("udp.dstport", "123"),
                ("ntp.flags.vn", "4"),
                ("ntp.flags.mode", "3"),
                ("ntp.stratum", "0"),
                ("ntp.ppoll", "6"),
                ("ntp.precision", "-20"),
                ("ntp.xmt", "Dec 16, 2024 03:21:16.500000000 UTC"),
            ],
        ),
        (
            "eth_ipv4_udp_snmp_get",
            vec![
                ("udp.dstport", "161"),
                ("snmp.version", "1"),
                ("snmp.community", "public"),
                ("snmp.request_id", "42"),
                ("snmp.variable_bindings", "2"),
            ],
        ),
        (
            "eth_ipv4_udp_syslog",
            vec![
                ("udp.dstport", "514"),
                ("syslog.facility", "4"),
                ("syslog.level", "2"),
                ("syslog.version", "1"),
                ("syslog.hostname", "host1"),
                ("syslog.appname", "su"),
            ],
        ),
        (
            "eth_ipv4_udp_radius",
            vec![
                ("udp.dstport", "1812"),
                ("radius.code", "1"),
                ("radius.id", "1"),
                ("radius.length", "63"),
                (
                    "radius.authenticator",
                    "0f:40:3f:94:73:97:80:57:bd:83:d5:cb:98:f4:22:7a",
                ),
                ("radius.NAS_Port_Type", "15"),
            ],
        ),
        (
            "eth_ipv4_udp_tftp_rrq",
            vec![
                ("udp.dstport", "69"),
                ("tftp.opcode", "1"),
                ("tftp.source_file", "pxelinux.0"),
                ("tftp.type", "octet"),
                ("tftp.option.name", "blksize"),
                ("tftp.option.value", "1428"),
            ],
        ),
    ]
}

#[test]
#[ignore = "needs tshark; run with WPG_TSHARK=1 -- --ignored"]
fn tshark_agrees_with_golden_pcaps() {
    if std::env::var("WPG_TSHARK").is_ok_and(|v| v != "1") {
        panic!("WPG_TSHARK must be 1 (or unset) to run this test");
    }
    let dir = golden_dir();
    let mut pcaps: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "pcap"))
        .collect();
    pcaps.sort();
    assert!(!pcaps.is_empty(), "no golden pcaps in {}", dir.display());
    let expectations = expectations();

    for pcap in pcaps {
        let stem = pcap.file_stem().unwrap().to_str().unwrap().to_owned();
        let doc = dissect(&pcap, MERGE_KEYS.contains(&stem.as_str()));
        let packets = doc.as_array().expect("array of packets");
        assert_eq!(packets.len(), 1, "{stem}: one packet expected");
        let pkt = &packets[0];

        // Whole name tokens (`rtcp` does not mean TCP).
        let has_token = |t: &str| stem.split('_').any(|x| x == t);

        // Generic checks: nothing malformed / expert-flagged, every checksum good.
        assert!(find(pkt, "_ws.malformed").is_none(), "{stem}: malformed");
        let mut status = Vec::new();
        collect(pkt, &|k| k.ends_with(".checksum.status"), &mut status);
        for (k, v) in &status {
            // tshark prints the enum as its numeric value: 1 == Good.
            assert!(v.as_str() == Some("1"), "{stem}: {k} = {v}");
        }
        let mut bad = Vec::new();
        collect(
            pkt,
            &|k| k.ends_with(".checksum_bad") || k.ends_with(".checksum.bad"),
            &mut bad,
        );
        assert!(
            bad.iter().all(|(_, v)| v.as_str() == Some("0")),
            "{stem}: {bad:?}"
        );
        if has_token("ipv4") {
            assert!(
                status.iter().any(|(k, _)| *k == "ip.checksum.status"),
                "{stem}: no ip.checksum.status reported"
            );
        }
        if has_token("udp") {
            assert!(
                status.iter().any(|(k, _)| *k == "udp.checksum.status"),
                "{stem}"
            );
        }
        if has_token("sctp") {
            assert!(
                status.iter().any(|(k, _)| *k == "sctp.checksum.status"),
                "{stem}"
            );
        }
        if has_token("tcp") {
            assert!(
                status.iter().any(|(k, _)| *k == "tcp.checksum.status"),
                "{stem}"
            );
        }

        // Every golden case needs decoded-field expectations (the M12 gate).
        let (_, want) = expectations
            .iter()
            .find(|(n, _)| *n == stem)
            .unwrap_or_else(|| panic!("{stem}: no entry in expectations()"));
        assert!(!want.is_empty(), "{stem}: empty expectations");
        for (field, expected) in want {
            assert_eq!(s(find(pkt, field)), *expected, "{stem}: {field}");
        }
    }
}
