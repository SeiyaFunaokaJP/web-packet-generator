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
                ("tcp.hdr_len", "20"),
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
