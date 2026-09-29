//! Hugepage pools (Linux; required by DPDK).

use serde::Serialize;

/// Hugepage state from `/proc/meminfo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct MemInfoHuge {
    /// `HugePages_Total`.
    pub total: u64,
    /// `HugePages_Free`.
    pub free: u64,
    /// `Hugepagesize` in kB (0 if absent).
    pub size_kb: u64,
}

/// One pool from `/sys/kernel/mm/hugepages/hugepages-<size>kB/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HugepagePool {
    /// Page size in kB.
    pub size_kb: u64,
    /// `nr_hugepages`.
    pub total: u64,
    /// `free_hugepages`.
    pub free: u64,
}

/// Everything known about hugepages on this host.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct Hugepages {
    /// Default-size pool from `/proc/meminfo`.
    pub meminfo: MemInfoHuge,
    /// All pools from sysfs (2 MiB, 1 GiB, …).
    pub pools: Vec<HugepagePool>,
}

impl Hugepages {
    /// Total number of configured hugepages over all pools.
    #[must_use]
    pub fn total_pages(&self) -> u64 {
        if self.pools.is_empty() {
            self.meminfo.total
        } else {
            self.pools.iter().map(|p| p.total).sum()
        }
    }

    /// Total free hugepages over all pools.
    #[must_use]
    pub fn free_pages(&self) -> u64 {
        if self.pools.is_empty() {
            self.meminfo.free
        } else {
            self.pools.iter().map(|p| p.free).sum()
        }
    }
}

/// Parses `/proc/meminfo` text.
#[must_use]
pub fn parse_meminfo(text: &str) -> MemInfoHuge {
    let mut out = MemInfoHuge::default();
    for line in text.lines() {
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        let value = rest
            .split_whitespace()
            .next()
            .and_then(|v| v.parse::<u64>().ok());
        let Some(value) = value else { continue };
        match key.trim() {
            "HugePages_Total" => out.total = value,
            "HugePages_Free" => out.free = value,
            "Hugepagesize" => out.size_kb = value,
            _ => {}
        }
    }
    out
}

/// Parses a sysfs directory name like `hugepages-2048kB` into kB.
#[must_use]
pub fn parse_pool_dir(name: &str) -> Option<u64> {
    name.strip_prefix("hugepages-")?
        .strip_suffix("kB")?
        .parse()
        .ok()
}

/// Reads hugepage information; `None` when unavailable (non-Linux).
#[must_use]
pub fn read() -> Option<Hugepages> {
    read_os()
}

#[cfg(target_os = "linux")]
fn read_os() -> Option<Hugepages> {
    let meminfo = parse_meminfo(&std::fs::read_to_string("/proc/meminfo").ok()?);
    let mut pools = Vec::new();
    if let Ok(rd) = std::fs::read_dir("/sys/kernel/mm/hugepages") {
        for e in rd.flatten() {
            let Some(size_kb) = parse_pool_dir(&e.file_name().to_string_lossy()) else {
                continue;
            };
            let num = |f: &str| {
                std::fs::read_to_string(e.path().join(f))
                    .ok()
                    .and_then(|s| s.trim().parse::<u64>().ok())
                    .unwrap_or(0)
            };
            pools.push(HugepagePool {
                size_kb,
                total: num("nr_hugepages"),
                free: num("free_hugepages"),
            });
        }
        pools.sort_by_key(|p| p.size_kb);
    }
    Some(Hugepages { meminfo, pools })
}

#[cfg(not(target_os = "linux"))]
fn read_os() -> Option<Hugepages> {
    None
}

/// PCI address (`0000:03:00.0`) of a NIC, from `/sys/class/net/<iface>/device`.
#[must_use]
pub fn pci_address(iface: &str) -> Option<String> {
    let link = std::fs::read_link(format!("/sys/class/net/{iface}/device")).ok()?;
    let name = link.file_name()?.to_string_lossy().into_owned();
    (name.matches(':').count() == 2).then_some(name)
}

/// How to reserve hugepages and hand a NIC to DPDK.
#[must_use]
pub fn dpdk_devbind_hint(iface: Option<&str>) -> String {
    let bind = match iface.and_then(pci_address) {
        Some(pci) => format!("sudo dpdk-devbind.py --bind=vfio-pci {pci}"),
        None => "sudo dpdk-devbind.py --status (then: --bind=vfio-pci <PCI address>)".to_owned(),
    };
    format!("echo 1024 | sudo tee /sys/kernel/mm/hugepages/hugepages-2048kB/nr_hugepages ; {bind}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEMINFO: &str = "MemTotal:       16384000 kB\nHugePages_Total:    1024\nHugePages_Free:      512\nHugePages_Rsvd:        0\nHugePages_Surp:        0\nHugepagesize:       2048 kB\nHugetlb:         2097152 kB\n";

    #[test]
    fn meminfo() {
        assert_eq!(
            parse_meminfo(MEMINFO),
            MemInfoHuge {
                total: 1024,
                free: 512,
                size_kb: 2048
            }
        );
        assert_eq!(parse_meminfo("MemTotal: 1 kB\n"), MemInfoHuge::default());
        assert_eq!(parse_meminfo(""), MemInfoHuge::default());
    }

    #[test]
    fn pool_dirs() {
        assert_eq!(parse_pool_dir("hugepages-2048kB"), Some(2048));
        assert_eq!(parse_pool_dir("hugepages-1048576kB"), Some(1_048_576));
        assert_eq!(parse_pool_dir("hugepages-x"), None);
        assert_eq!(parse_pool_dir("other"), None);
    }

    #[test]
    fn totals_prefer_pools() {
        let mut h = Hugepages {
            meminfo: parse_meminfo(MEMINFO),
            pools: vec![],
        };
        assert_eq!((h.total_pages(), h.free_pages()), (1024, 512));
        h.pools.push(HugepagePool {
            size_kb: 1_048_576,
            total: 4,
            free: 3,
        });
        assert_eq!((h.total_pages(), h.free_pages()), (4, 3));
    }

    #[test]
    fn hint_mentions_devbind() {
        assert!(dpdk_devbind_hint(None).contains("dpdk-devbind.py"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn reads_this_host() {
        assert!(read().is_some());
    }
}
