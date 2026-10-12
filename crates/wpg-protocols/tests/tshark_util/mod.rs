//! Optional `tshark` cross-check helper shared by the P1 integration tests.
//!
//! The rendered frame is written to a one-packet pcap (via `wpg_testutil`) and
//! dissected with `tshark -T json`. It only runs when `WPG_TSHARK=1`; otherwise
//! [`dissect`] returns `None` and the callers skip their tshark assertions, so the
//! suite passes without Wireshark installed.
//!
//! The checks are deliberately coarse (the dissected protocol is present and
//! nothing is flagged malformed) so they do not couple to tshark's field naming,
//! which varies between versions.
#![allow(dead_code)]

use std::process::Command;

use serde_json::Value;
use wpg_testutil::pcap::write_pcap;

/// A synthetic Ethernet header (dst / src / ethertype) for frames that are not
/// themselves Ethernet (the pcap link type is `LINKTYPE_ETHERNET`).
fn eth_header(ethertype: u16) -> Vec<u8> {
    let mut h = vec![0x02, 0, 0, 0, 0, 2, 0x02, 0, 0, 0, 0, 1];
    h.extend_from_slice(&ethertype.to_be_bytes());
    h
}

/// Dissects a frame with tshark, or `None` when `WPG_TSHARK != 1`.
///
/// `ethertype` wraps a non-Ethernet frame (e.g. a bare IPv4 tunnel) in an Ethernet
/// header first; pass `None` for a frame that already starts with Ethernet.
#[must_use]
pub(crate) fn dissect(frame: &[u8], ethertype: Option<u16>) -> Option<Value> {
    dissect_with(frame, ethertype, &[])
}

/// [`dissect`] with extra tshark arguments (e.g. `-d udp.port==5004,rtp` for
/// protocols without a well-known port).
#[must_use]
pub(crate) fn dissect_with(frame: &[u8], ethertype: Option<u16>, args: &[&str]) -> Option<Value> {
    if std::env::var("WPG_TSHARK").ok().as_deref() != Some("1") {
        return None;
    }
    let bytes = match ethertype {
        Some(et) => {
            let mut b = eth_header(et);
            b.extend_from_slice(frame);
            b
        }
        None => frame.to_vec(),
    };
    let dir = std::env::temp_dir().join(format!(
        "wpg-p1-tshark-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("frame.pcap");
    write_pcap(&path, &[bytes]).expect("write pcap");
    let out = Command::new("tshark")
        .arg("-r")
        .arg(&path)
        .args(args)
        .args(["-T", "json"])
        .output()
        .expect("run tshark (WPG_TSHARK=1 requires tshark on PATH)");
    assert!(
        out.status.success(),
        "tshark failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc: Value = serde_json::from_slice(&out.stdout).expect("tshark JSON");
    let _ = std::fs::remove_dir_all(&dir);
    doc.as_array()
        .and_then(|a| a.first())
        .and_then(|p| find(p, "layers"))
        .cloned()
}

/// Asserts tshark dissected `frame` (optionally Ethernet-wrapped) with the named
/// protocol layer present. No-op without `WPG_TSHARK`.
///
/// Only the target protocol's presence is checked, not `_ws.malformed` anywhere in
/// the tree: tshark applies port-based heuristics to *inner* payloads (e.g. UDP
/// port 2222 → EtherNet/IP CIP) that can flag an unrelated, deeper dissector, which
/// says nothing about the layer under test.
pub(crate) fn assert_dissects(frame: &[u8], ethertype: Option<u16>, protocol_key: &str) {
    let Some(layers) = dissect(frame, ethertype) else {
        return;
    };
    assert!(
        find(&layers, protocol_key).is_some(),
        "tshark did not decode a `{protocol_key}` layer"
    );
}

/// Recursively finds the first value stored under `key`.
#[must_use]
pub(crate) fn find<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    match v {
        Value::Object(m) => m.get(key).or_else(|| m.values().find_map(|c| find(c, key))),
        Value::Array(a) => a.iter().find_map(|c| find(c, key)),
        _ => None,
    }
}
