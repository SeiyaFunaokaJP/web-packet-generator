//! # wpg-sys
//!
//! Operating-system facts needed by the transmit engine and the `wpg ifaces` /
//! `wpg doctor` commands (spec §2.2, §2.7):
//!
//! * [`ifaces`] — enumerate network interfaces (Linux / macOS `getifaddrs`, Windows
//!   `GetAdaptersAddresses`) with MAC, addresses, MTU, link state, speed and driver.
//! * [`caps`] — do we have the privileges for raw sockets (`CAP_NET_RAW`, root, `/dev/bpf*`,
//!   Administrator)?
//! * [`kernel`] — Linux kernel version and the features it gates (AF_XDP, TPACKET_V3).
//! * [`hugepages`] — hugepage pools for DPDK.
//! * [`npcap`] / [`pcaplib`] — Npcap (Windows) and libpcap (Unix) presence.
//! * [`doctor`] — combines the above into a [`doctor::Report`].
//!
//! The crate deliberately does not depend on libpcap; it only looks for the library file.
//! All parsing is done by pure functions so it can be unit-tested with fixture strings on
//! any host. It is native-only and must never be pulled into the WASM crates.

pub mod caps;
pub mod doctor;
pub mod hugepages;
pub mod ifaces;
pub mod kernel;
pub mod npcap;
pub mod pcaplib;

#[cfg(test)]
pub(crate) mod testutil {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Creates a fresh empty directory under the system temp dir.
    pub(crate) fn tempdir(tag: &str) -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "wpg-sys-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
