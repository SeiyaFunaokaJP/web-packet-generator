//! Backend registry: open a backend by name and list what this build supports.

use serde::{Deserialize, Serialize};

use crate::{NullBackend, PcapFileBackend, TxBackend, TxError, TxOptions};

/// Every backend name, in the order shown to users.
pub const NAMES: &[&str] = &["null", "pcapfile", "afpacket", "pcap", "afxdp", "dpdk"];

/// Whether a backend can be used in this build / on this OS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendInfo {
    /// Backend name (`null`, `pcapfile`, `afpacket`, `pcap`, `afxdp`, `dpdk`).
    pub name: String,
    /// Compiled in and supported on this OS. Privileges are *not* checked here
    /// (see `wpg doctor`).
    pub available: bool,
    /// Why it is unavailable, or a note on requirements.
    pub reason: Option<String>,
}

fn info(name: &str, available: bool, reason: Option<&str>) -> BackendInfo {
    BackendInfo {
        name: name.to_owned(),
        available,
        reason: reason.map(str::to_owned),
    }
}

/// Availability of every backend in [`NAMES`].
#[must_use]
pub fn available() -> Vec<BackendInfo> {
    let mut v = vec![
        info("null", true, Some("discards frames (benchmarks, tests)")),
        info(
            "pcapfile",
            true,
            Some("writes pcapng (.pcap: classic pcap); the interface is the output path"),
        ),
    ];
    v.push(afpacket_info());
    v.push(pcap_info());
    v.push(info(
        "afxdp",
        false,
        Some("not implemented yet (planned for M8)"),
    ));
    v.push(info(
        "dpdk",
        false,
        Some("not implemented yet (planned for M8, build with --features dpdk)"),
    ));
    v
}

#[cfg(all(feature = "afpacket", target_os = "linux"))]
fn afpacket_info() -> BackendInfo {
    info(
        "afpacket",
        true,
        Some("needs CAP_NET_RAW (setcap cap_net_raw,cap_net_admin+ep)"),
    )
}

#[cfg(not(all(feature = "afpacket", target_os = "linux")))]
fn afpacket_info() -> BackendInfo {
    if cfg!(target_os = "linux") {
        info(
            "afpacket",
            false,
            Some("not compiled in (build with --features afpacket)"),
        )
    } else {
        info("afpacket", false, Some("Linux only"))
    }
}

// pcap: added by pcap module — the `pcap` backend lives in `crate::pcap` (feature
// `pcap`) and must provide
// `pub fn open(iface: &str, opts: &TxOptions) -> Result<Box<dyn TxBackend>, TxError>`.
#[cfg(feature = "pcap")]
fn pcap_info() -> BackendInfo {
    info(
        "pcap",
        true,
        Some("libpcap / Npcap (Windows: Npcap in WinPcap-compatible mode)"),
    )
}

#[cfg(not(feature = "pcap"))]
fn pcap_info() -> BackendInfo {
    info(
        "pcap",
        false,
        Some("not compiled in (build with --features pcap)"),
    )
}

/// Creates and opens the backend `name` on `iface`.
///
/// # Errors
/// [`TxError::Unsupported`] for an unknown name or a backend that is not compiled
/// in / not available on this OS; otherwise whatever the backend's `open` returns.
pub fn open_by_name(
    name: &str,
    iface: &str,
    opts: &TxOptions,
) -> Result<Box<dyn TxBackend>, TxError> {
    match name {
        "null" => {
            let mut b = NullBackend::new();
            b.open(iface, opts)?;
            Ok(Box::new(b))
        }
        "pcapfile" => {
            let mut b = PcapFileBackend::new();
            b.open(iface, opts)?;
            Ok(Box::new(b))
        }
        "afpacket" => open_afpacket(iface, opts),
        // pcap: added by pcap module
        #[cfg(feature = "pcap")]
        "pcap" => crate::pcap::open(iface, opts),
        _ => {
            let why = available()
                .into_iter()
                .find(|b| b.name == name)
                .and_then(|b| b.reason)
                .unwrap_or_else(|| format!("known backends: {}", NAMES.join(", ")));
            Err(TxError::Unsupported(format!("backend `{name}`: {why}")))
        }
    }
}

/// Whether the backend `name` can run several TX workers on one interface (one
/// instance per worker). `pcapfile` writes one file and `pcap` is kept to a single
/// handle, so they are limited to one worker.
#[must_use]
pub fn supports_workers(name: &str) -> bool {
    !matches!(name, "pcapfile" | "pcap")
}

/// Opens `n` independent instances of the backend `name` on `iface`, one per TX
/// worker (`afpacket`: one socket and TX ring each). The core in `opts.pin_core`
/// is ignored here; workers pin themselves (see `JobConfig::pin_cores`).
///
/// # Errors
/// [`TxError::Config`] when `n > 1` for a backend that is limited to one worker
/// ([`supports_workers`]); otherwise the errors of [`open_by_name`]. Instances
/// opened before a failure are closed.
pub fn open_workers(
    name: &str,
    iface: &str,
    opts: &TxOptions,
    n: usize,
) -> Result<Vec<Box<dyn TxBackend>>, TxError> {
    let n = n.max(1);
    if n > 1 && !supports_workers(name) {
        return Err(TxError::Config(format!(
            "backend `{name}` supports a single worker (got {n})"
        )));
    }
    let mut out: Vec<Box<dyn TxBackend>> = Vec::with_capacity(n);
    for _ in 0..n {
        match open_by_name(name, iface, opts) {
            Ok(b) => out.push(b),
            Err(e) => {
                for b in &mut out {
                    let _ = b.close();
                }
                return Err(e);
            }
        }
    }
    Ok(out)
}

#[cfg(all(feature = "afpacket", target_os = "linux"))]
fn open_afpacket(iface: &str, opts: &TxOptions) -> Result<Box<dyn TxBackend>, TxError> {
    let mut b = crate::afpacket::AfPacketBackend::new();
    b.open(iface, opts)?;
    Ok(Box::new(b))
}

#[cfg(not(all(feature = "afpacket", target_os = "linux")))]
fn open_afpacket(_iface: &str, _opts: &TxOptions) -> Result<Box<dyn TxBackend>, TxError> {
    Err(TxError::Unsupported(format!(
        "backend `afpacket`: {}",
        afpacket_info().reason.unwrap_or_default()
    )))
}

/// The default backend for real interfaces on this build (`afpacket` on Linux when
/// compiled in, else `pcap` when compiled in, else `null`).
#[must_use]
pub fn default_name() -> &'static str {
    if cfg!(all(feature = "afpacket", target_os = "linux")) {
        "afpacket"
    } else if cfg!(feature = "pcap") {
        "pcap"
    } else {
        "null"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry() {
        let all = available();
        let names: Vec<&str> = all.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, NAMES);
        assert!(all[0].available);

        let b = open_by_name("null", "x", &TxOptions::default()).unwrap();
        assert_eq!(b.name(), "null");
        for n in ["afxdp", "dpdk", "bogus"] {
            match open_by_name(n, "x", &TxOptions::default()) {
                Err(TxError::Unsupported(m)) => assert!(m.contains(n), "{m}"),
                other => panic!("{n}: {:?}", other.map(|b| b.name())),
            }
        }
        assert_eq!(
            open_workers("null", "x", &TxOptions::default(), 3)
                .unwrap()
                .len(),
            3
        );
        assert!(matches!(
            open_workers("pcapfile", "/tmp/x.pcapng", &TxOptions::default(), 2),
            Err(TxError::Config(_))
        ));
        assert!(supports_workers("afpacket") && !supports_workers("pcap"));
        #[cfg(not(feature = "pcap"))]
        assert!(matches!(
            open_by_name("pcap", "eth0", &TxOptions::default()),
            Err(TxError::Unsupported(_))
        ));
    }
}
