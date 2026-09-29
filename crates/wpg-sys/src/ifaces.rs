//! Network interface enumeration.
//!
//! * Linux: `getifaddrs(3)` for addresses / flags, `/sys/class/net/<if>/…` for MTU, speed,
//!   carrier and driver.
//! * macOS: `getifaddrs(3)` (`AF_LINK` for the MAC) and `ioctl(SIOCGIFMTU)`.
//! * Windows: `GetAdaptersAddresses`.

use std::fmt::Write as _;
use std::io;
use std::net::{Ipv4Addr, Ipv6Addr};

use serde::{Serialize, Serializer};

/// One network interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Iface {
    /// Name usable with the transmit backends (`eth0`, `en0`; on Windows the adapter GUID
    /// such as `{1234…}`, which the pcap backend maps to `\Device\NPF_{…}`).
    pub name: String,
    /// OS interface index.
    pub index: u32,
    /// Hardware address, if the interface has one (all-zero addresses are reported as `None`).
    #[serde(serialize_with = "serialize_mac")]
    pub mac: Option<[u8; 6]>,
    /// IPv4 addresses.
    pub ipv4: Vec<Ipv4Addr>,
    /// IPv6 addresses.
    pub ipv6: Vec<Ipv6Addr>,
    /// MTU in bytes.
    pub mtu: Option<u32>,
    /// Administratively up (`IFF_UP`; on Windows: operational status is Up).
    pub up: bool,
    /// Link has carrier / resources allocated (`IFF_RUNNING`).
    pub running: bool,
    /// Loopback interface.
    pub loopback: bool,
    /// Negotiated link speed in Mbit/s.
    pub speed_mbps: Option<u64>,
    /// Kernel driver name (Linux only).
    pub driver: Option<String>,
    /// Human readable name (Windows friendly name / description).
    pub description: Option<String>,
}

impl Iface {
    fn new(name: String) -> Self {
        Self {
            name,
            index: 0,
            mac: None,
            ipv4: Vec::new(),
            ipv6: Vec::new(),
            mtu: None,
            up: false,
            running: false,
            loopback: false,
            speed_mbps: None,
            driver: None,
            description: None,
        }
    }

    /// Whether the interface is a plausible transmit target: up, running and not loopback.
    #[must_use]
    pub fn is_usable(&self) -> bool {
        self.up && self.running && !self.loopback
    }
}

/// Formats a MAC as `aa:bb:cc:dd:ee:ff`.
#[must_use]
pub fn format_mac(mac: &[u8; 6]) -> String {
    let mut s = String::with_capacity(17);
    for (i, b) in mac.iter().enumerate() {
        if i > 0 {
            s.push(':');
        }
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Parses `aa:bb:cc:dd:ee:ff` (also `-` separated). All-zero addresses yield `None`.
#[must_use]
pub fn parse_mac(text: &str) -> Option<[u8; 6]> {
    let mut out = [0u8; 6];
    let mut n = 0;
    for part in text.trim().split([':', '-']) {
        if n >= 6 || part.len() != 2 {
            return None;
        }
        out[n] = u8::from_str_radix(part, 16).ok()?;
        n += 1;
    }
    (n == 6 && out != [0; 6]).then_some(out)
}

fn serialize_mac<S: Serializer>(mac: &Option<[u8; 6]>, s: S) -> Result<S::Ok, S::Error> {
    match mac {
        Some(m) => s.serialize_some(&format_mac(m)),
        None => s.serialize_none(),
    }
}

/// Lists all network interfaces, sorted by index.
///
/// # Errors
/// The OS call (`getifaddrs` / `GetAdaptersAddresses`) failed.
pub fn list() -> io::Result<Vec<Iface>> {
    let mut v = list_os()?;
    v.sort_by(|a, b| a.index.cmp(&b.index).then_with(|| a.name.cmp(&b.name)));
    Ok(v)
}

#[cfg(unix)]
fn list_os() -> io::Result<Vec<Iface>> {
    unix::list()
}

#[cfg(windows)]
fn list_os() -> io::Result<Vec<Iface>> {
    windows::list()
}

#[cfg(not(any(unix, windows)))]
fn list_os() -> io::Result<Vec<Iface>> {
    Ok(Vec::new())
}

// ---------------------------------------------------------------------------------------
// Linux sysfs (pure over a base directory so tests can use fixtures)
// ---------------------------------------------------------------------------------------

/// Attributes read from `/sys/class/net/<if>/`.
#[cfg(any(target_os = "linux", test))]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SysfsInfo {
    pub(crate) mtu: Option<u32>,
    pub(crate) operstate: Option<String>,
    pub(crate) speed_mbps: Option<u64>,
    pub(crate) carrier: Option<bool>,
    pub(crate) mac: Option<[u8; 6]>,
    pub(crate) driver: Option<String>,
}

#[cfg(any(target_os = "linux", test))]
/// Parses the `speed` attribute: `-1` (unknown / down) and `0` map to `None`.
pub(crate) fn parse_speed(text: &str) -> Option<u64> {
    let v: i64 = text.trim().parse().ok()?;
    u64::try_from(v).ok().filter(|&s| s > 0)
}

#[cfg(any(target_os = "linux", test))]
/// Reads sysfs attributes of `name` below `class_net` (normally `/sys/class/net`).
pub(crate) fn read_sysfs(class_net: &std::path::Path, name: &str) -> SysfsInfo {
    let dir = class_net.join(name);
    let read = |file: &str| {
        std::fs::read_to_string(dir.join(file))
            .ok()
            .map(|s| s.trim().to_owned())
    };
    SysfsInfo {
        mtu: read("mtu").and_then(|s| s.parse().ok()),
        operstate: read("operstate"),
        speed_mbps: read("speed").and_then(|s| parse_speed(&s)),
        carrier: read("carrier").map(|s| s != "0"),
        mac: read("address").and_then(|s| parse_mac(&s)),
        driver: std::fs::read_link(dir.join("device/driver"))
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())),
    }
}

// ---------------------------------------------------------------------------------------
// Unix
// ---------------------------------------------------------------------------------------

#[cfg(unix)]
mod unix {
    use super::{Iface, Ipv4Addr, Ipv6Addr};
    use std::ffi::CStr;
    use std::io;
    use std::ptr;

    struct IfAddrs(*mut libc::ifaddrs);

    impl Drop for IfAddrs {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: pointer came from a successful getifaddrs and is freed once.
                unsafe { libc::freeifaddrs(self.0) };
            }
        }
    }

    pub(super) fn list() -> io::Result<Vec<Iface>> {
        let mut head: *mut libc::ifaddrs = ptr::null_mut();
        // SAFETY: `head` is a valid out-pointer.
        if unsafe { libc::getifaddrs(&mut head) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let guard = IfAddrs(head);
        let mut out: Vec<Iface> = Vec::new();
        let mut cur = guard.0;
        while !cur.is_null() {
            // SAFETY: `cur` is a live node of the list owned by `guard`.
            let ifa = unsafe { &*cur };
            cur = ifa.ifa_next;
            // SAFETY: ifa_name is a valid NUL-terminated string.
            let name = unsafe { CStr::from_ptr(ifa.ifa_name) }
                .to_string_lossy()
                .into_owned();
            let pos = if let Some(p) = out.iter().position(|i| i.name == name) {
                p
            } else {
                let mut i = Iface::new(name.clone());
                // SAFETY: `ifa_name` is valid, see above.
                i.index = unsafe { libc::if_nametoindex(ifa.ifa_name) };
                out.push(i);
                out.len() - 1
            };
            let entry = &mut out[pos];
            let flags = ifa.ifa_flags;
            let has = |f: libc::c_int| flags & (f as libc::c_uint) != 0;
            entry.up |= has(libc::IFF_UP);
            entry.running |= has(libc::IFF_RUNNING);
            entry.loopback |= has(libc::IFF_LOOPBACK);

            if ifa.ifa_addr.is_null() {
                continue;
            }
            // SAFETY: a non-null ifa_addr points to at least a `sockaddr`.
            let family = libc::c_int::from(unsafe { (*ifa.ifa_addr).sa_family });
            if family == libc::AF_INET {
                // SAFETY: AF_INET addresses are `sockaddr_in`.
                let sin = unsafe { &*ifa.ifa_addr.cast::<libc::sockaddr_in>() };
                let ip = Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr));
                entry.ipv4.push(ip);
            } else if family == libc::AF_INET6 {
                // SAFETY: AF_INET6 addresses are `sockaddr_in6`.
                let sin6 = unsafe { &*ifa.ifa_addr.cast::<libc::sockaddr_in6>() };
                entry.ipv6.push(Ipv6Addr::from(sin6.sin6_addr.s6_addr));
            } else {
                // SAFETY: family matches the cast inside.
                if let Some(mac) = unsafe { link_mac(ifa.ifa_addr, family) } {
                    entry.mac = Some(mac);
                }
            }
        }
        for i in &mut out {
            fill_extra(i);
        }
        Ok(out)
    }

    /// Extracts a 6-byte hardware address from a link-layer sockaddr.
    ///
    /// # Safety
    /// `addr` must be non-null and point to a sockaddr of the given `family`.
    #[cfg(target_os = "linux")]
    unsafe fn link_mac(addr: *const libc::sockaddr, family: libc::c_int) -> Option<[u8; 6]> {
        if family != libc::AF_PACKET {
            return None;
        }
        // SAFETY: AF_PACKET addresses are `sockaddr_ll` (caller contract).
        let ll = unsafe { &*addr.cast::<libc::sockaddr_ll>() };
        if ll.sll_halen != 6 {
            return None;
        }
        let mut mac = [0u8; 6];
        mac.copy_from_slice(&ll.sll_addr[..6]);
        (mac != [0; 6]).then_some(mac)
    }

    /// See the Linux variant.
    ///
    /// # Safety
    /// `addr` must be non-null and point to a sockaddr of the given `family`.
    #[cfg(target_vendor = "apple")]
    unsafe fn link_mac(addr: *const libc::sockaddr, family: libc::c_int) -> Option<[u8; 6]> {
        if family != libc::AF_LINK {
            return None;
        }
        // SAFETY: AF_LINK addresses are `sockaddr_dl` followed by name + address bytes
        // (caller contract); we only read `alen` bytes past `sdl_data + nlen`.
        unsafe {
            let dl = &*addr.cast::<libc::sockaddr_dl>();
            if dl.sdl_alen != 6 {
                return None;
            }
            let data = addr
                .cast::<u8>()
                .add(std::mem::offset_of!(libc::sockaddr_dl, sdl_data) + usize::from(dl.sdl_nlen));
            let mut mac = [0u8; 6];
            ptr::copy_nonoverlapping(data, mac.as_mut_ptr(), 6);
            (mac != [0; 6]).then_some(mac)
        }
    }

    #[cfg(not(any(target_os = "linux", target_vendor = "apple")))]
    unsafe fn link_mac(_addr: *const libc::sockaddr, _family: libc::c_int) -> Option<[u8; 6]> {
        None
    }

    #[cfg(target_os = "linux")]
    fn fill_extra(i: &mut Iface) {
        let info = super::read_sysfs(std::path::Path::new("/sys/class/net"), &i.name);
        i.mtu = info.mtu;
        i.speed_mbps = info.speed_mbps;
        i.driver = info.driver;
        if i.mac.is_none() {
            i.mac = info.mac;
        }
        if info.carrier == Some(false) {
            i.running = false;
        }
    }

    #[cfg(target_vendor = "apple")]
    fn fill_extra(i: &mut Iface) {
        i.mtu = mtu_ioctl(&i.name);
    }

    #[cfg(not(any(target_os = "linux", target_vendor = "apple")))]
    fn fill_extra(_i: &mut Iface) {}

    /// `SIOCGIFMTU` = `_IOWR('i', 51, struct ifreq)` on Darwin (`sizeof(ifreq)` = 32).
    #[cfg(target_vendor = "apple")]
    fn mtu_ioctl(name: &str) -> Option<u32> {
        const SIOCGIFMTU: libc::c_ulong = 0xc020_6933;
        #[repr(C)]
        struct IfReqMtu {
            name: [libc::c_char; libc::IF_NAMESIZE],
            mtu: libc::c_int,
            _pad: [u8; 12],
        }
        let mut req = IfReqMtu {
            name: [0; libc::IF_NAMESIZE],
            mtu: 0,
            _pad: [0; 12],
        };
        for (dst, src) in req
            .name
            .iter_mut()
            .zip(name.bytes().take(libc::IF_NAMESIZE - 1))
        {
            *dst = src as libc::c_char;
        }
        // SAFETY: plain socket/ioctl/close calls with a properly sized, initialised request.
        unsafe {
            let fd = libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0);
            if fd < 0 {
                return None;
            }
            let rc = libc::ioctl(fd, SIOCGIFMTU, &mut req as *mut IfReqMtu);
            libc::close(fd);
            (rc == 0).then(|| u32::try_from(req.mtu).ok()).flatten()
        }
    }
}

// ---------------------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------------------

#[cfg(windows)]
mod windows {
    use super::{Iface, Ipv4Addr, Ipv6Addr};
    use std::io;
    use std::ptr;
    use windows_sys::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, NO_ERROR};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetAdaptersAddresses, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER,
        GAA_FLAG_SKIP_MULTICAST, IF_TYPE_SOFTWARE_LOOPBACK, IP_ADAPTER_ADDRESSES_LH,
    };
    use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows_sys::Win32::Networking::WinSock::{
        AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR_IN, SOCKADDR_IN6,
    };

    /// Reads a NUL-terminated UTF-16 string.
    ///
    /// # Safety
    /// `p` must be null or point to a NUL-terminated UTF-16 string.
    unsafe fn wide(p: *const u16) -> Option<String> {
        if p.is_null() {
            return None;
        }
        let mut len = 0;
        // SAFETY: caller guarantees NUL termination.
        while unsafe { *p.add(len) } != 0 {
            len += 1;
        }
        // SAFETY: `len` elements were just read.
        let s = unsafe { std::slice::from_raw_parts(p, len) };
        Some(String::from_utf16_lossy(s))
    }

    /// Reads a NUL-terminated ANSI string.
    ///
    /// # Safety
    /// `p` must be null or point to a NUL-terminated string.
    unsafe fn ansi(p: *const u8) -> Option<String> {
        if p.is_null() {
            return None;
        }
        // SAFETY: caller guarantees NUL termination.
        let c = unsafe { std::ffi::CStr::from_ptr(p.cast()) };
        Some(c.to_string_lossy().into_owned())
    }

    pub(super) fn list() -> io::Result<Vec<Iface>> {
        let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
        let mut size: u32 = 16 * 1024;
        // u64 elements keep the buffer 8-byte aligned as IP_ADAPTER_ADDRESSES requires.
        let mut buf: Vec<u64> = Vec::new();
        let mut attempts = 0;
        loop {
            buf.clear();
            buf.resize((size as usize).div_ceil(8), 0);
            // SAFETY: `buf` is at least `size` bytes; `size` is updated on overflow.
            let rc = unsafe {
                GetAdaptersAddresses(
                    u32::from(AF_UNSPEC),
                    flags,
                    ptr::null(),
                    buf.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>(),
                    &mut size,
                )
            };
            attempts += 1;
            if rc == NO_ERROR {
                break;
            }
            if rc == ERROR_BUFFER_OVERFLOW && attempts < 4 {
                continue;
            }
            return Err(io::Error::from_raw_os_error(
                i32::try_from(rc).unwrap_or(-1),
            ));
        }

        let mut out = Vec::new();
        let mut cur = buf.as_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        while !cur.is_null() {
            // SAFETY: the list lives in `buf`, filled in by GetAdaptersAddresses.
            let a = unsafe { &*cur };
            cur = a.Next;
            // SAFETY: the string pointers are valid for the lifetime of `buf`.
            let (name, friendly, descr) = unsafe {
                (
                    ansi(a.AdapterName.cast_const()),
                    wide(a.FriendlyName.cast_const()),
                    wide(a.Description.cast_const()),
                )
            };
            let mut i = Iface::new(name.unwrap_or_default());
            // SAFETY: reading the `Anonymous.IfIndex` view of the union; both views are POD.
            i.index = unsafe { a.Anonymous1.Anonymous.IfIndex };
            if i.index == 0 {
                i.index = a.Ipv6IfIndex;
            }
            let alen = a.PhysicalAddressLength as usize;
            if alen == 6 {
                let mut mac = [0u8; 6];
                mac.copy_from_slice(&a.PhysicalAddress[..6]);
                i.mac = (mac != [0; 6]).then_some(mac);
            }
            i.mtu = (a.Mtu != 0 && a.Mtu != u32::MAX).then_some(a.Mtu);
            let oper_up = a.OperStatus == IfOperStatusUp;
            i.up = oper_up;
            i.running = oper_up;
            i.loopback = a.IfType == IF_TYPE_SOFTWARE_LOOPBACK;
            let bps = a.TransmitLinkSpeed;
            i.speed_mbps = (bps != 0 && bps != u64::MAX).then_some(bps / 1_000_000);
            i.description = match (friendly, descr) {
                (Some(f), Some(d)) if f != d => Some(format!("{f} ({d})")),
                (Some(f), _) => Some(f),
                (None, d) => d,
            };

            let mut ua = a.FirstUnicastAddress;
            while !ua.is_null() {
                // SAFETY: unicast list nodes live in `buf`.
                let u = unsafe { &*ua };
                ua = u.Next;
                let sa = u.Address.lpSockaddr;
                if sa.is_null() {
                    continue;
                }
                // SAFETY: lpSockaddr points to a sockaddr of the announced family.
                unsafe {
                    let family = (*sa).sa_family;
                    if family == AF_INET {
                        let sin = &*sa.cast::<SOCKADDR_IN>();
                        i.ipv4
                            .push(Ipv4Addr::from(u32::from_be(sin.sin_addr.S_un.S_addr)));
                    } else if family == AF_INET6 {
                        let sin6 = &*sa.cast::<SOCKADDR_IN6>();
                        i.ipv6.push(Ipv6Addr::from(sin6.sin6_addr.u.Byte));
                    }
                }
            }
            out.push(i);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_roundtrip() {
        let m = parse_mac("02:42:ac:11:00:02").unwrap();
        assert_eq!(m, [0x02, 0x42, 0xac, 0x11, 0x00, 0x02]);
        assert_eq!(format_mac(&m), "02:42:ac:11:00:02");
        assert_eq!(parse_mac("00-00-00-00-00-00"), None);
        assert_eq!(parse_mac("02:42:ac:11:00"), None);
        assert_eq!(parse_mac("zz:42:ac:11:00:02"), None);
        assert_eq!(parse_mac("02:42:ac:11:00:02:03"), None);
    }

    #[test]
    fn speed_parsing() {
        assert_eq!(parse_speed("1000\n"), Some(1000));
        assert_eq!(parse_speed("-1\n"), None);
        assert_eq!(parse_speed("0"), None);
        assert_eq!(parse_speed("garbage"), None);
    }

    #[test]
    fn sysfs_fixture() {
        let base = crate::testutil::tempdir("sysfs");
        let d = base.join("eth9");
        std::fs::create_dir_all(d.join("device")).unwrap();
        std::fs::write(d.join("mtu"), "9000\n").unwrap();
        std::fs::write(d.join("operstate"), "up\n").unwrap();
        std::fs::write(d.join("speed"), "25000\n").unwrap();
        std::fs::write(d.join("carrier"), "1\n").unwrap();
        std::fs::write(d.join("address"), "aa:bb:cc:00:11:22\n").unwrap();
        #[cfg(unix)]
        {
            std::fs::create_dir_all(base.join("drivers/mlx5_core")).unwrap();
            std::os::unix::fs::symlink(base.join("drivers/mlx5_core"), d.join("device/driver"))
                .unwrap();
        }
        let info = read_sysfs(&base, "eth9");
        assert_eq!(info.mtu, Some(9000));
        assert_eq!(info.operstate.as_deref(), Some("up"));
        assert_eq!(info.speed_mbps, Some(25000));
        assert_eq!(info.carrier, Some(true));
        assert_eq!(info.mac, Some([0xaa, 0xbb, 0xcc, 0, 0x11, 0x22]));
        #[cfg(unix)]
        assert_eq!(info.driver.as_deref(), Some("mlx5_core"));
        assert_eq!(read_sysfs(&base, "nope"), SysfsInfo::default());
    }

    #[test]
    fn json_shape() {
        let mut i = Iface::new("eth0".into());
        i.mac = Some([1, 2, 3, 4, 5, 6]);
        i.ipv4.push(Ipv4Addr::new(10, 0, 0, 1));
        let v = serde_json::to_value(&i).unwrap();
        assert_eq!(v["mac"], "01:02:03:04:05:06");
        assert_eq!(v["ipv4"][0], "10.0.0.1");
        assert_eq!(v["speed_mbps"], serde_json::Value::Null);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn lists_loopback() {
        let all = list().unwrap();
        let lo = all.iter().find(|i| i.name == "lo").expect("lo present");
        assert!(lo.loopback);
        assert!(lo.up);
        assert!(lo.index > 0);
        assert!(lo.ipv4.contains(&Ipv4Addr::LOCALHOST));
        assert!(lo.mtu.is_some());
        assert!(!lo.is_usable());
        assert!(all.windows(2).all(|w| w[0].index <= w[1].index));
    }
}
