//! Privileges required for raw packet transmission.

use serde::Serialize;

/// `CAP_NET_ADMIN` bit number (linux/capability.h).
pub const CAP_NET_ADMIN: u32 = 12;
/// `CAP_NET_RAW` bit number (linux/capability.h).
pub const CAP_NET_RAW: u32 = 13;

/// What the current process may do with raw sockets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RawAccess {
    /// Effective uid is 0 (Linux / macOS) or the process is elevated (Windows).
    pub privileged_user: bool,
    /// Linux: `CAP_NET_RAW` is in the effective set. `None` if it could not be determined
    /// (non-Linux, or `/proc` unavailable).
    pub cap_net_raw: Option<bool>,
    /// Linux: `CAP_NET_ADMIN` is in the effective set.
    pub cap_net_admin: Option<bool>,
    /// macOS: at least one `/dev/bpf*` device is writable.
    pub bpf_writable: Option<bool>,
}

impl RawAccess {
    /// Whether raw sockets can be expected to work.
    #[must_use]
    pub fn allowed(&self) -> bool {
        match self.cap_net_raw {
            Some(cap) => cap,
            None => self.privileged_user || self.bpf_writable == Some(true),
        }
    }
}

/// Parses the hexadecimal `CapEff:` line of `/proc/<pid>/status`.
#[must_use]
pub fn parse_cap_eff(status: &str) -> Option<u64> {
    status
        .lines()
        .find_map(|l| l.strip_prefix("CapEff:"))
        .and_then(|v| u64::from_str_radix(v.trim(), 16).ok())
}

/// Whether capability bit `bit` is set in `mask`.
#[must_use]
pub fn cap_in_mask(mask: u64, bit: u32) -> bool {
    bit < 64 && mask & (1u64 << bit) != 0
}

/// Probes the current process.
#[must_use]
pub fn probe() -> RawAccess {
    probe_os()
}

/// Whether the process can open raw sockets (see [`RawAccess::allowed`]).
#[must_use]
pub fn has_raw_socket_capability() -> bool {
    probe().allowed()
}

#[cfg(target_os = "linux")]
fn probe_os() -> RawAccess {
    // SAFETY: geteuid has no preconditions.
    let root = unsafe { libc::geteuid() } == 0;
    let mask = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| parse_cap_eff(&s));
    RawAccess {
        privileged_user: root,
        cap_net_raw: mask.map(|m| cap_in_mask(m, CAP_NET_RAW)),
        cap_net_admin: mask.map(|m| cap_in_mask(m, CAP_NET_ADMIN)),
        bpf_writable: None,
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
fn probe_os() -> RawAccess {
    // SAFETY: geteuid has no preconditions.
    let root = unsafe { libc::geteuid() } == 0;
    let bpf = (0..256).any(|n| {
        std::ffi::CString::new(format!("/dev/bpf{n}"))
            // SAFETY: valid NUL-terminated path.
            .is_ok_and(|p| unsafe { libc::access(p.as_ptr(), libc::W_OK) } == 0)
    });
    RawAccess {
        privileged_user: root,
        cap_net_raw: None,
        cap_net_admin: None,
        bpf_writable: Some(bpf),
    }
}

#[cfg(windows)]
fn probe_os() -> RawAccess {
    // SAFETY: IsUserAnAdmin has no preconditions.
    let admin = unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() } != 0;
    RawAccess {
        privileged_user: admin,
        cap_net_raw: None,
        cap_net_admin: None,
        bpf_writable: None,
    }
}

#[cfg(not(any(unix, windows)))]
fn probe_os() -> RawAccess {
    RawAccess {
        privileged_user: false,
        cap_net_raw: None,
        cap_net_admin: None,
        bpf_writable: None,
    }
}

/// A shell command / instruction that grants the missing privilege to `exe_path`.
#[must_use]
pub fn setcap_hint(exe_path: &str) -> String {
    if cfg!(target_os = "linux") {
        format!("sudo setcap cap_net_raw,cap_net_admin+eip {exe_path}")
    } else if cfg!(target_os = "macos") {
        format!("sudo {exe_path} …   (or make /dev/bpf* accessible, e.g. via Wireshark's ChmodBPF)")
    } else if cfg!(windows) {
        "Run from an elevated (Administrator) terminal, or reinstall Npcap (https://npcap.com/#download) without \"Restrict Npcap driver's access to Administrators only\"".to_owned()
    } else {
        format!("run {exe_path} as root")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATUS: &str = "Name:\twpg\nCapInh:\t0000000000000000\nCapPrm:\t000001ffffffffff\nCapEff:\t0000000000002000\nCapBnd:\t000001ffffffffff\n";

    #[test]
    fn parses_cap_eff() {
        let m = parse_cap_eff(STATUS).unwrap();
        assert_eq!(m, 0x2000);
        assert!(cap_in_mask(m, CAP_NET_RAW));
        assert!(!cap_in_mask(m, CAP_NET_ADMIN));
        assert_eq!(parse_cap_eff("Name:\tx\n"), None);
        assert_eq!(parse_cap_eff("CapEff:\tzz\n"), None);
        assert!(!cap_in_mask(u64::MAX, 64));
    }

    #[test]
    fn full_root_mask() {
        let m = parse_cap_eff("CapEff:\t000001ffffffffff\n").unwrap();
        assert!(cap_in_mask(m, CAP_NET_RAW) && cap_in_mask(m, CAP_NET_ADMIN));
    }

    #[test]
    fn allowed_logic() {
        let base = RawAccess {
            privileged_user: true,
            cap_net_raw: Some(false),
            cap_net_admin: Some(false),
            bpf_writable: None,
        };
        // Root without the capability (dropped in a container) is not enough.
        assert!(!base.allowed());
        assert!(RawAccess {
            cap_net_raw: Some(true),
            privileged_user: false,
            ..base
        }
        .allowed());
        assert!(RawAccess {
            cap_net_raw: None,
            ..base
        }
        .allowed());
    }

    #[test]
    fn hint_mentions_path() {
        assert!(setcap_hint("/usr/bin/wpg").contains("/usr/bin/wpg") || cfg!(windows));
    }
}
