//! # wpg-dhcp
//!
//! A small DHCP server for test networks (ADR-0012): unlike the `dhcp` /
//! `dhcpv6` protocols of `wpg-protocols`, which only *craft* packets, this
//! crate answers clients on one interface and keeps their leases.
//!
//! * [`v4`], [`v6`]: message codecs (RFC 2131 / 2132, RFC 8415);
//! * [`config`]: pools and options ([`Settings`]) with validation;
//! * [`lease`]: the lease table shared by both families;
//! * [`server4`], [`server6`]: the protocol logic as pure functions (request
//!   + leases + time → reply), unit tested without sockets;
//! * [`netlink`]: interface addresses set from the screen (Linux);
//! * [`ra`]: router advertisements with the M / O flags so that hosts use
//!   DHCPv6;
//! * [`server`]: the sockets (Linux: bound to the interface with
//!   `SO_BINDTODEVICE`) and the running [`Server`].

pub mod config;
pub mod lease;
pub mod netlink;
pub mod ra;
pub mod server;
pub mod server4;
pub mod server6;
pub mod v4;
pub mod v6;

pub use config::{Reservation, Settings, V4Config, V6Config};
pub use lease::{Lease, LeaseKind, LeaseState, Leases};
pub use server::{Activity, Family, Iface, Server, Stats, StatsSnapshot};

/// Seconds since the Unix epoch.
#[must_use]
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// `aa:bb:cc:dd:ee:ff`.
#[must_use]
pub fn fmt_mac(mac: &[u8]) -> String {
    mac.iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// Lower-case hex without separators.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Parses `aa:bb:cc:dd:ee:ff`, `aa-bb-...` or `aabb.ccdd.eeff`.
#[must_use]
pub fn parse_mac(s: &str) -> Option<[u8; 6]> {
    let digits: String = s.chars().filter(char::is_ascii_hexdigit).collect();
    let rest: String = s
        .chars()
        .filter(|c| !c.is_ascii_hexdigit() && !matches!(c, ':' | '-' | '.'))
        .collect();
    if digits.len() != 12 || !rest.trim().is_empty() {
        return None;
    }
    let mut mac = [0u8; 6];
    for (i, b) in mac.iter_mut().enumerate() {
        *b = u8::from_str_radix(&digits[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(mac)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macs() {
        let m = [0x00, 0x11, 0x22, 0xaa, 0xbb, 0xcc];
        assert_eq!(fmt_mac(&m), "00:11:22:aa:bb:cc");
        assert_eq!(parse_mac("00:11:22:AA:bb:cc"), Some(m));
        assert_eq!(parse_mac("0011.22aa.bbcc"), Some(m));
        assert_eq!(parse_mac("00-11-22-aa-bb-cc"), Some(m));
        assert_eq!(parse_mac("00:11:22:aa:bb"), None);
        assert_eq!(parse_mac("00:11:22:aa:bb:cg"), None);
        assert_eq!(hex(&[1, 0xab]), "01ab");
    }
}
