//! Pinning the current thread to one CPU core.
//!
//! Linux uses `sched_setaffinity(0, ...)` (the calling thread). On other operating
//! systems pinning is a documented no-op: macOS has no hard affinity API and Windows
//! support (`SetThreadAffinityMask`) is not wired up yet.

/// Pins the calling thread to `core`. Returns `Ok(true)` when pinned, `Ok(false)` when
/// pinning is not supported on this OS.
///
/// # Errors
/// The OS error (e.g. `EINVAL` for a core that does not exist or is not allowed).
#[cfg(target_os = "linux")]
pub(crate) fn pin_current_thread(core: usize) -> std::io::Result<bool> {
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
#[cfg(not(target_os = "linux"))]
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn pin_current_thread(_core: usize) -> std::io::Result<bool> {
    Ok(false)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    #[test]
    fn pin_to_core_zero_in_thread() {
        let r = std::thread::spawn(|| super::pin_current_thread(0))
            .join()
            .unwrap();
        // Core 0 may be outside the container's cpuset; either outcome is fine as
        // long as it does not panic, but an out-of-range core must fail.
        let _ = r;
        assert!(super::pin_current_thread(1 << 20).is_err());
    }
}
