//! Small Linux system-call helpers shared by the `afpacket` and `afxdp` backends.

#![cfg_attr(not(any(feature = "afpacket", feature = "afxdp")), allow(dead_code))]

use std::ffi::CString;
use std::io;
use std::os::fd::RawFd;

use crate::TxError;

/// `size_of::<T>()` as `socklen_t` (socket address / option structs are tiny).
#[allow(clippy::cast_possible_truncation)]
pub(crate) const fn socklen<T>() -> libc::socklen_t {
    size_of::<T>() as libc::socklen_t
}

/// Turns a negative return code into the last OS error.
pub(crate) fn cvt(rc: libc::c_int) -> io::Result<libc::c_int> {
    if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(rc)
    }
}

/// `setsockopt` with a plain C struct / integer value.
pub(crate) fn setsockopt<T>(
    fd: RawFd,
    level: libc::c_int,
    opt: libc::c_int,
    v: &T,
) -> io::Result<()> {
    // SAFETY: passes a pointer to a live `T` with its exact size.
    cvt(unsafe { libc::setsockopt(fd, level, opt, std::ptr::from_ref(v).cast(), socklen::<T>()) })
        .map(drop)
}

/// `setsockopt` with a `c_int` value.
pub(crate) fn setsockopt_int(
    fd: RawFd,
    level: libc::c_int,
    opt: libc::c_int,
    v: libc::c_int,
) -> io::Result<()> {
    setsockopt(fd, level, opt, &v)
}

/// Interface index of `iface` (`if_nametoindex`).
///
/// # Errors
/// [`TxError::Interface`] if the interface does not exist.
pub fn ifindex(iface: &str) -> Result<i32, TxError> {
    let c = CString::new(iface).map_err(|_| TxError::Interface(iface.to_owned()))?;
    // SAFETY: `c` is a valid NUL-terminated string.
    let idx = unsafe { libc::if_nametoindex(c.as_ptr()) };
    if idx == 0 {
        return Err(TxError::Interface(iface.to_owned()));
    }
    i32::try_from(idx).map_err(|_| TxError::Interface(iface.to_owned()))
}

/// MTU of `iface` from sysfs.
pub(crate) fn iface_mtu(iface: &str) -> Option<usize> {
    std::fs::read_to_string(format!("/sys/class/net/{iface}/mtu"))
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// The system page size.
pub(crate) fn page_size() -> usize {
    // SAFETY: sysconf has no preconditions.
    let p = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    usize::try_from(p).ok().filter(|p| *p > 0).unwrap_or(4096)
}
