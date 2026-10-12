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

fn dissect(pcap: &Path) -> Value {
    let out = Command::new("tshark")
        .arg("-r")
        .arg(pcap)
        // Checksum validation is off by default in Wireshark.
        .args(["-o", "ip.check_checksum:TRUE"])
        .args(["-o", "udp.check_checksum:TRUE"])
        .args(["-o", "tcp.check_checksum:TRUE"])
        .args(["-o", "sctp.checksum:CRC-32C"])
        .args(["-T", "json"])
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
/// One entry per golden case; cases without an entry still get the generic checks.
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
        let doc = dissect(&pcap);
        let packets = doc.as_array().expect("array of packets");
        assert_eq!(packets.len(), 1, "{stem}: one packet expected");
        let pkt = &packets[0];

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
        if stem.contains("ipv4") {
            assert!(
                status.iter().any(|(k, _)| *k == "ip.checksum.status"),
                "{stem}: no ip.checksum.status reported"
            );
        }
        if stem.contains("udp") {
            assert!(
                status.iter().any(|(k, _)| *k == "udp.checksum.status"),
                "{stem}"
            );
        }
        if stem.contains("sctp") {
            assert!(
                status.iter().any(|(k, _)| *k == "sctp.checksum.status"),
                "{stem}"
            );
        }
        if stem.contains("tcp") {
            assert!(
                status.iter().any(|(k, _)| *k == "tcp.checksum.status"),
                "{stem}"
            );
        }

        if let Some((_, want)) = expectations.iter().find(|(n, _)| *n == stem) {
            for (field, expected) in want {
                assert_eq!(s(find(pkt, field)), *expected, "{stem}: {field}");
            }
        }
    }
}
