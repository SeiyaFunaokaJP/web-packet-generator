//! CPU pinning and NUMA / queue topology for multi-core transmission.
//!
//! * [`pin_current_thread`]: pins the calling thread to one core. Linux uses
//!   `sched_setaffinity(0, ...)`; on other operating systems pinning is a
//!   documented no-op (macOS has no hard affinity API and Windows support,
//!   `SetThreadAffinityMask`, is not wired up yet).
//! * [`iface_numa_node`], [`node_cpus`], [`allowed_cpus`], [`auto_cores`]: picks
//!   worker cores on the NUMA node of the NIC (`/sys/class/net/<if>/device/numa_node`)
//!   so frame buffers, rings and the NIC's DMA stay on one memory controller.
//! * [`tx_queue_count`] / [`rx_queue_count`]: the interface's queues (AF_XDP binds
//!   one socket per queue).
//! * [`prefer_node`]: `mbind(MPOL_PREFERRED)` for a mapping that is about to be
//!   touched (the AF_XDP UMEM), so its pages are allocated on the NIC's node.
//!
//! Frame buffers of a TX worker are allocated by the worker thread itself after it
//! has pinned itself, so Linux's first-touch policy already places them on the
//! core's node.

use std::path::Path;

/// Pins the calling thread to `core`. Returns `Ok(true)` when pinned, `Ok(false)` when
/// pinning is not supported on this OS.
///
/// # Errors
/// The OS error (e.g. `EINVAL` for a core that does not exist or is not allowed).
#[cfg(target_os = "linux")]
pub fn pin_current_thread(core: usize) -> std::io::Result<bool> {
    let max = 8 * size_of::<libc::cpu_set_t>();
    if core >= max {
        return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
    }
    // SAFETY: `cpu_set_t` is plain data; zeroed is the empty set. `CPU_SET` only
    // writes inside the set (index checked above), and `sched_setaffinity` reads
    // exactly `size_of::<cpu_set_t>()` bytes from a valid pointer.
    let rc = unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_SET(core, &mut set);
        libc::sched_setaffinity(0, size_of::<libc::cpu_set_t>(), &raw const set)
    };
    if rc == 0 {
        Ok(true)
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// See the Linux version; always `Ok(false)` here.
///
/// # Errors
/// None.
#[cfg(not(target_os = "linux"))]
#[allow(clippy::unnecessary_wraps)]
pub fn pin_current_thread(_core: usize) -> std::io::Result<bool> {
    Ok(false)
}

/// Parses a kernel CPU / node list such as `0-3,8,10-11` (sysfs `cpulist`).
/// Malformed parts are skipped; the result is sorted and deduplicated.
#[must_use]
pub fn parse_cpu_list(s: &str) -> Vec<usize> {
    let mut v = Vec::new();
    for part in s.trim().split(',').map(str::trim).filter(|p| !p.is_empty()) {
        match part.split_once('-') {
            Some((a, b)) => {
                if let (Ok(a), Ok(b)) = (a.trim().parse::<usize>(), b.trim().parse::<usize>()) {
                    if a <= b && b - a < 65_536 {
                        v.extend(a..=b);
                    }
                }
            }
            None => {
                if let Ok(c) = part.parse() {
                    v.push(c);
                }
            }
        }
    }
    v.sort_unstable();
    v.dedup();
    v
}

fn sys_net(iface: &str) -> Option<std::path::PathBuf> {
    // Interface names never contain '/', but do not let one escape sysfs.
    (!iface.is_empty() && !iface.contains('/') && iface != "." && iface != "..")
        .then(|| Path::new("/sys/class/net").join(iface))
}

/// NUMA node of the NIC behind `iface` (`None`: not Linux, virtual interface, or a
/// single-node machine reporting `-1`).
#[must_use]
pub fn iface_numa_node(iface: &str) -> Option<usize> {
    let s = std::fs::read_to_string(sys_net(iface)?.join("device/numa_node")).ok()?;
    s.trim()
        .parse::<i64>()
        .ok()
        .and_then(|n| usize::try_from(n).ok())
}

/// CPUs of NUMA node `node` (empty when unknown).
#[must_use]
pub fn node_cpus(node: usize) -> Vec<usize> {
    std::fs::read_to_string(format!("/sys/devices/system/node/node{node}/cpulist"))
        .map(|s| parse_cpu_list(&s))
        .unwrap_or_default()
}

/// NUMA node of `cpu` (`None` when unknown).
#[must_use]
pub fn cpu_node(cpu: usize) -> Option<usize> {
    let dir = std::fs::read_dir(format!("/sys/devices/system/cpu/cpu{cpu}")).ok()?;
    dir.filter_map(Result::ok)
        .find_map(|e| e.file_name().to_str()?.strip_prefix("node")?.parse().ok())
}

fn count_queues(iface: &str, prefix: &str) -> Option<usize> {
    let dir = std::fs::read_dir(sys_net(iface)?.join("queues")).ok()?;
    let n = dir
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.starts_with(prefix))
        })
        .count();
    (n > 0).then_some(n)
}

/// Number of TX queues of `iface` (`/sys/class/net/<if>/queues/tx-*`).
#[must_use]
pub fn tx_queue_count(iface: &str) -> Option<usize> {
    count_queues(iface, "tx-")
}

/// Number of RX queues of `iface` (`/sys/class/net/<if>/queues/rx-*`).
#[must_use]
pub fn rx_queue_count(iface: &str) -> Option<usize> {
    count_queues(iface, "rx-")
}

/// CPUs this process may run on (`sched_getaffinity`; elsewhere `0..N` from
/// `available_parallelism`).
#[must_use]
pub fn allowed_cpus() -> Vec<usize> {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: zeroed cpu_set_t is the empty set; the kernel fills at most
        // `size_of::<cpu_set_t>()` bytes; CPU_ISSET reads inside the set.
        unsafe {
            let mut set: libc::cpu_set_t = std::mem::zeroed();
            if libc::sched_getaffinity(0, size_of::<libc::cpu_set_t>(), &raw mut set) == 0 {
                let max = 8 * size_of::<libc::cpu_set_t>();
                let v: Vec<usize> = (0..max).filter(|&c| libc::CPU_ISSET(c, &set)).collect();
                if !v.is_empty() {
                    return v;
                }
            }
        }
    }
    let n = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    (0..n).collect()
}

/// Chooses `n` cores for TX workers from `allowed` (see [`allowed_cpus`]):
/// cores on `node` (the NIC's NUMA node, see [`iface_numa_node`] / [`node_cpus`])
/// first, then the others; core 0, which usually carries housekeeping and
/// interrupts, last. With fewer cores than workers the list wraps around.
#[must_use]
pub fn choose_cores(allowed: &[usize], node_cpus: &[usize], n: usize) -> Vec<usize> {
    let mut order: Vec<usize> = Vec::with_capacity(allowed.len());
    let local = |c: &usize| node_cpus.contains(c);
    order.extend(allowed.iter().copied().filter(|c| *c != 0 && local(c)));
    order.extend(allowed.iter().copied().filter(|c| *c != 0 && !local(c)));
    order.extend(allowed.iter().copied().filter(|c| *c == 0));
    if order.is_empty() {
        return Vec::new();
    }
    (0..n).map(|i| order[i % order.len()]).collect()
}

/// [`choose_cores`] for the NIC behind `iface` on this machine.
#[must_use]
pub fn auto_cores(iface: &str, n: usize) -> Vec<usize> {
    let node = iface_numa_node(iface).map(node_cpus).unwrap_or_default();
    choose_cores(&allowed_cpus(), &node, n)
}

/// Asks the kernel to allocate the pages of `[addr, addr + len)` on NUMA node
/// `node` (`mbind(MPOL_PREFERRED)`); call it before the pages are first touched.
/// Returns whether the policy was applied (`false` on other OSes, single-node
/// kernels without NUMA support, or a node number above 1023).
#[cfg(target_os = "linux")]
// The pointer is only passed to mbind(2), which validates it; nothing is dereferenced.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn prefer_node(addr: *mut u8, len: usize, node: usize) -> bool {
    const MPOL_PREFERRED: libc::c_long = 1;
    const WORDS: usize = 16;
    if node >= WORDS * 64 {
        return false;
    }
    let mut mask = [0u64; WORDS];
    mask[node / 64] |= 1 << (node % 64);
    // SAFETY: mbind only changes the memory policy of our own mapping; `mask` is
    // live for the call and holds `WORDS * 64` bits.
    let rc = unsafe {
        libc::syscall(
            libc::SYS_mbind,
            addr,
            len,
            MPOL_PREFERRED,
            mask.as_ptr(),
            (WORDS * 64 + 1) as libc::c_ulong,
            0 as libc::c_uint,
        )
    };
    rc == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn pin_to_core_zero_in_thread() {
        let r = std::thread::spawn(|| pin_current_thread(0)).join().unwrap();
        // Core 0 may be outside the container's cpuset; either outcome is fine as
        // long as it does not panic, but an out-of-range core must fail.
        let _ = r;
        assert!(pin_current_thread(1 << 20).is_err());
    }

    #[test]
    fn cpu_lists() {
        assert_eq!(parse_cpu_list("0-3,8,10-11\n"), vec![0, 1, 2, 3, 8, 10, 11]);
        assert_eq!(parse_cpu_list("5"), vec![5]);
        assert_eq!(parse_cpu_list(""), Vec::<usize>::new());
        assert_eq!(parse_cpu_list("3-1,x,2"), vec![2]);
        assert_eq!(parse_cpu_list("1,1,0"), vec![0, 1]);
    }

    #[test]
    fn core_choice_prefers_the_nic_node() {
        // Two nodes: 0-3 and 4-7, NIC on node 1.
        let all: Vec<usize> = (0..8).collect();
        assert_eq!(choose_cores(&all, &[4, 5, 6, 7], 3), vec![4, 5, 6]);
        assert_eq!(choose_cores(&all, &[4, 5, 6, 7], 6), vec![4, 5, 6, 7, 1, 2]);
        // Unknown node: core 0 last.
        assert_eq!(choose_cores(&[0, 1, 2], &[], 3), vec![1, 2, 0]);
        // Wraps around with more workers than cores.
        assert_eq!(choose_cores(&[0, 1], &[], 3), vec![1, 0, 1]);
        assert!(choose_cores(&[], &[], 2).is_empty());
    }

    #[test]
    fn topology_queries_do_not_panic() {
        assert!(!allowed_cpus().is_empty());
        assert_eq!(iface_numa_node("../etc"), None);
        assert_eq!(tx_queue_count("nonexistent0"), None);
        #[cfg(target_os = "linux")]
        {
            assert!(tx_queue_count("lo").is_some_and(|n| n >= 1));
            assert!(rx_queue_count("lo").is_some_and(|n| n >= 1));
            assert_eq!(auto_cores("lo", 2).len(), 2);
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn mbind_on_anonymous_memory() {
        let len = 1 << 16;
        // SAFETY: anonymous private mapping, unmapped below.
        let p = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        assert_ne!(p, libc::MAP_FAILED);
        // Node 0 exists wherever NUMA is compiled in; the result may be false
        // without CONFIG_NUMA, but it must not crash.
        let _ = prefer_node(p.cast(), len, 0);
        assert!(!prefer_node(p.cast(), len, 1 << 20));
        // SAFETY: unmaps the mapping created above.
        unsafe { libc::munmap(p, len) };
    }
}
