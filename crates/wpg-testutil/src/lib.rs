//! Dependency-free helpers shared by the workspace tests: a minimal pcap / pcapng
//! writer and reader (Ethernet link type only), hex helpers and golden-file assertions.

pub mod golden;
pub mod hex;
pub mod pcap;

pub use golden::assert_golden;
pub use hex::{hex, to_hex};
pub use pcap::{read_pcap, read_pcapng, write_pcap, write_pcapng};
