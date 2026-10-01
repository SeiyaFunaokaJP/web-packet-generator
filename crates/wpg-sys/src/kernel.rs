//! Kernel version and the packet I/O features it gates (Linux).

use std::cmp::Ordering;
use std::fmt;

use serde::Serialize;

/// `major.minor.patch` kernel version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct KernelVersion {
    /// Major.
    pub major: u32,
    /// Minor.
    pub minor: u32,
    /// Patch (0 if absent).
    pub patch: u32,
}

impl KernelVersion {
    /// Constructs a version.
    #[must_use]
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Whether `self >= other`.
    #[must_use]
    pub fn at_least(self, other: Self) -> bool {
        self.cmp(&other) != Ordering::Less
    }
}

impl Ord for KernelVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch))
    }
}

impl PartialOrd for KernelVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for KernelVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Minimum kernel for AF_XDP zero-copy usable from `wpg` (TODO §3).
pub const AF_XDP_MIN: KernelVersion = KernelVersion::new(5, 10, 0);
/// Minimum kernel for `TPACKET_V3` (3.2).
pub const TPACKET_V3_MIN: KernelVersion = KernelVersion::new(3, 2, 0);

/// Parses a `uname -r` string such as `5.15.0-91-generic`, `6.18.44-fc-v49`, `4.19+`, `5.10`.
#[must_use]
pub fn parse_release(release: &str) -> Option<KernelVersion> {
    let mut nums = [0u32; 3];
    let mut rest = release.trim();
    let mut parsed = 0;
    for (n, slot) in nums.iter_mut().enumerate() {
        let end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        if end == 0 {
            if n < 2 {
                return None;
            }
            break;
        }
        *slot = rest[..end].parse().ok()?;
        parsed += 1;
        rest = &rest[end..];
        match rest.strip_prefix('.') {
            Some(r) => rest = r,
            None => break,
        }
    }
    (parsed >= 2).then(|| KernelVersion::new(nums[0], nums[1], nums[2]))
}

/// Raw `uname -r` (Unix only).
#[must_use]
pub fn release() -> Option<String> {
    release_os()
}

#[cfg(unix)]
fn release_os() -> Option<String> {
    // SAFETY: utsname is plain old data; uname fills it on success.
    unsafe {
        let mut u: libc::utsname = std::mem::zeroed();
        if libc::uname(&mut u) != 0 {
            return None;
        }
        Some(
            std::ffi::CStr::from_ptr(u.release.as_ptr())
                .to_string_lossy()
                .into_owned(),
        )
    }
}

#[cfg(not(unix))]
fn release_os() -> Option<String> {
    None
}

/// Running Linux kernel version; `None` on other operating systems.
#[must_use]
pub fn version() -> Option<KernelVersion> {
    if cfg!(target_os = "linux") {
        release().and_then(|r| parse_release(&r))
    } else {
        None
    }
}

/// Whether the running kernel is Linux >= 5.10 (AF_XDP).
#[must_use]
pub fn supports_af_xdp() -> bool {
    version().is_some_and(|v| v.at_least(AF_XDP_MIN))
}

/// Whether the running kernel is Linux >= 3.2 (`TPACKET_V3`).
#[must_use]
pub fn supports_tpacket_v3() -> bool {
    version().is_some_and(|v| v.at_least(TPACKET_V3_MIN))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_uname_strings() {
        assert_eq!(
            parse_release("5.15.0-91-generic"),
            Some(KernelVersion::new(5, 15, 0))
        );
        assert_eq!(
            parse_release("6.18.44-fc-v49\n"),
            Some(KernelVersion::new(6, 18, 44))
        );
        assert_eq!(parse_release("4.19+"), Some(KernelVersion::new(4, 19, 0)));
        assert_eq!(parse_release("5.10"), Some(KernelVersion::new(5, 10, 0)));
        assert_eq!(
            parse_release("6.1.0-rc3"),
            Some(KernelVersion::new(6, 1, 0))
        );
        assert_eq!(parse_release("garbage"), None);
        assert_eq!(parse_release(""), None);
        assert_eq!(parse_release("5"), None);
    }

    #[test]
    fn comparisons() {
        assert!(KernelVersion::new(5, 10, 0).at_least(AF_XDP_MIN));
        assert!(KernelVersion::new(6, 0, 0).at_least(AF_XDP_MIN));
        assert!(!KernelVersion::new(5, 9, 255).at_least(AF_XDP_MIN));
        assert!(KernelVersion::new(3, 2, 0).at_least(TPACKET_V3_MIN));
        assert_eq!(KernelVersion::new(5, 4, 1).to_string(), "5.4.1");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn running_kernel_is_parsed() {
        assert!(version().is_some());
    }
}
