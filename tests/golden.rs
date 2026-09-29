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

use wpg_core::{FieldValue, Layer, Packet};
use wpg_protocols::catalog;
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
    Packet {
        layers: vec![
            eth(),
            ipv4(0x1234, [10, 0, 0, 1], [10, 0, 0, 2]).with("flags", 0b010u8),
            Layer::new("udp")
                .with("src_port", 1234u16)
                .with("dst_port", 5678u16),
            Layer::new("raw").with("data", b"hello".to_vec()),
        ],
    }
}

fn case_eth_dot1q_ipv4_tcp() -> Packet {
    Packet {
        layers: vec![
            eth(),
            Layer::new("dot1q").with("pcp", 5u8).with("vid", 100u16),
            ipv4(1, [192, 168, 1, 10], [198, 51, 100, 7]).with("ttl", 128u8),
            Layer::new("tcp")
                .with("src_port", 40000u16)
                .with("dst_port", 443u16)
                .with("seq", 0x0102_0304u32)
                .with("window", 64240u16)
                .with("options", vec![0x02, 0x04, 0x05, 0xb4]),
        ],
    }
}

fn case_eth_ipv4_tcp_syn() -> Packet {
    Packet {
        layers: vec![
            eth(),
            ipv4(0xbeef, [172, 16, 0, 1], [172, 16, 0, 2]),
            Layer::new("tcp")
                .with("src_port", 51000u16)
                .with("dst_port", 80u16)
                .with("seq", 0xdead_beefu32)
                .with("flags", 0x02u8),
        ],
    }
}

/// The golden table: `(file stem, packet)`. One line per case.
fn cases() -> Vec<(&'static str, Packet)> {
    vec![
        ("eth_ipv4_udp", case_eth_ipv4_udp()),
        ("eth_dot1q_ipv4_tcp", case_eth_dot1q_ipv4_tcp()),
        ("eth_ipv4_tcp_syn", case_eth_ipv4_tcp_syn()),
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
