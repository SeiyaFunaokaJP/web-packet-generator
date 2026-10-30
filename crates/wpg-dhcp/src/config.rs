//! Pools and options of the server, as edited on the DHCP screen.

use std::net::{Ipv4Addr, Ipv6Addr};

use serde::{Deserialize, Serialize};

/// Shortest accepted lease / lifetime.
pub const MIN_LEASE_SECS: u32 = 30;

/// Everything the server needs besides the interface.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Addresses put on the interface while the server runs
    /// (`192.168.100.1/24`, `2001:db8:1::1/64`); removed when it stops.
    pub addresses: Vec<String>,
    /// DHCPv4.
    pub v4: V4Config,
    /// DHCPv6 and router advertisements.
    pub v6: V6Config,
}

/// A fixed address for a MAC address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reservation {
    /// `aa:bb:cc:dd:ee:ff`.
    pub mac: String,
    /// Address handed out to it (may be outside the range).
    pub ip: Ipv4Addr,
}

/// `[v4]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct V4Config {
    /// Answer DHCPv4.
    pub enabled: bool,
    /// First address of the pool.
    pub range_start: Ipv4Addr,
    /// Last address of the pool.
    pub range_end: Ipv4Addr,
    /// Option 1.
    pub subnet_mask: Ipv4Addr,
    /// Option 3.
    pub routers: Vec<Ipv4Addr>,
    /// Option 6.
    pub dns: Vec<Ipv4Addr>,
    /// Option 15.
    pub domain: String,
    /// Option 42.
    pub ntp: Vec<Ipv4Addr>,
    /// Option 51 (T1 / T2 are 1/2 and 7/8 of it).
    pub lease_secs: u32,
    /// Fixed addresses.
    pub reservations: Vec<Reservation>,
    /// Server identifier (option 54). `None`: the first IPv4 address of the
    /// interface.
    pub server_ip: Option<Ipv4Addr>,
}

impl Default for V4Config {
    fn default() -> Self {
        Self {
            enabled: true,
            range_start: Ipv4Addr::new(192, 168, 100, 100),
            range_end: Ipv4Addr::new(192, 168, 100, 199),
            subnet_mask: Ipv4Addr::new(255, 255, 255, 0),
            routers: Vec::new(),
            dns: Vec::new(),
            domain: String::new(),
            ntp: Vec::new(),
            lease_secs: 3600,
            reservations: Vec::new(),
            server_ip: None,
        }
    }
}

/// `[v6]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct V6Config {
    /// Answer DHCPv6.
    pub enabled: bool,
    /// Hand out addresses (`IA_NA`).
    pub na_enabled: bool,
    /// First address of the `IA_NA` pool.
    pub range_start: Ipv6Addr,
    /// Last address of the `IA_NA` pool.
    pub range_end: Ipv6Addr,
    /// Delegate prefixes (`IA_PD`).
    pub pd_enabled: bool,
    /// Pool the delegated prefixes are cut from.
    pub pd_prefix: Ipv6Addr,
    /// Length of [`V6Config::pd_prefix`].
    pub pd_prefix_len: u8,
    /// Length of each delegated prefix.
    pub pd_delegated_len: u8,
    /// Option 23.
    pub dns: Vec<Ipv6Addr>,
    /// Option 24.
    pub domains: Vec<String>,
    /// Preferred lifetime (T1 / T2 are 1/2 and 4/5 of it).
    pub preferred_secs: u32,
    /// Valid lifetime.
    pub valid_secs: u32,
    /// Answer a SOLICIT with Rapid Commit by a REPLY.
    pub rapid_commit: bool,
    /// Option 7 in ADVERTISE (0 = not sent).
    pub preference: u8,
    /// Send router advertisements (M flag when `IA_NA` is on, O flag always).
    pub ra_enabled: bool,
    /// Seconds between unsolicited advertisements.
    pub ra_interval_secs: u32,
    /// Router lifetime (0: not a default router).
    pub ra_router_lifetime_secs: u16,
    /// On-link prefix announced in the advertisements, e.g. `2001:db8:1::/64`
    /// (empty: none).
    pub ra_prefix: String,
    /// The A flag of [`V6Config::ra_prefix`] (SLAAC).
    pub ra_autonomous: bool,
}

impl Default for V6Config {
    fn default() -> Self {
        Self {
            enabled: false,
            na_enabled: true,
            range_start: Ipv6Addr::new(0x2001, 0xdb8, 0, 1, 0, 0, 0, 0x100),
            range_end: Ipv6Addr::new(0x2001, 0xdb8, 0, 1, 0, 0, 0, 0x1ff),
            pd_enabled: false,
            pd_prefix: Ipv6Addr::new(0x2001, 0xdb8, 0x100, 0, 0, 0, 0, 0),
            pd_prefix_len: 40,
            pd_delegated_len: 56,
            dns: Vec::new(),
            domains: Vec::new(),
            preferred_secs: 3600,
            valid_secs: 7200,
            rapid_commit: true,
            preference: 0,
            ra_enabled: false,
            ra_interval_secs: 30,
            ra_router_lifetime_secs: 0,
            ra_prefix: String::new(),
            ra_autonomous: false,
        }
    }
}

/// A setting that cannot work; `field` is the JSON path (`v4.range_end`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Problem {
    /// Field.
    pub field: String,
    /// Explanation (English).
    pub message: String,
}

fn problem(out: &mut Vec<Problem>, field: &str, message: impl Into<String>) {
    out.push(Problem {
        field: field.to_owned(),
        message: message.into(),
    });
}

/// Parses `2001:db8::/64`.
///
/// # Errors
/// A message when the text is not an IPv6 prefix.
pub fn parse_prefix6(s: &str) -> Result<(Ipv6Addr, u8), String> {
    let (a, l) = s
        .trim()
        .split_once('/')
        .ok_or_else(|| format!("{s}: expected address/length"))?;
    let addr: Ipv6Addr = a.parse().map_err(|e| format!("{a}: {e}"))?;
    let len: u8 = l.parse().map_err(|e| format!("{l}: {e}"))?;
    if len > 128 {
        return Err(format!("{len}: longer than 128"));
    }
    Ok((mask6(addr, len), len))
}

/// Parses an interface address `192.168.1.1/24` / `2001:db8::1/64` (host
/// bits kept).
///
/// # Errors
/// A message when the text is not an address with a valid length.
pub fn parse_cidr(s: &str) -> Result<(std::net::IpAddr, u8), String> {
    let (a, l) = s
        .trim()
        .split_once('/')
        .ok_or_else(|| format!("{s}: expected address/length"))?;
    let addr: std::net::IpAddr = a.parse().map_err(|e| format!("{a}: {e}"))?;
    let len: u8 = l.parse().map_err(|e| format!("{l}: {e}"))?;
    let max = if addr.is_ipv4() { 32 } else { 128 };
    if len == 0 || len > max {
        return Err(format!("{s}: length must be 1..{max}"));
    }
    if addr.is_unspecified() || addr.is_multicast() || addr.is_loopback() {
        return Err(format!("{s}: not a host address"));
    }
    Ok((addr, len))
}

/// `addr` with the bits after `len` cleared.
#[must_use]
pub fn mask6(addr: Ipv6Addr, len: u8) -> Ipv6Addr {
    let v = u128::from(addr);
    let m = if len == 0 {
        0
    } else {
        u128::MAX << (128 - u32::from(len.min(128)))
    };
    Ipv6Addr::from(v & m)
}

impl V4Config {
    /// Whether `ip` is in the pool.
    #[must_use]
    pub fn in_range(&self, ip: Ipv4Addr) -> bool {
        ip >= self.range_start && ip <= self.range_end
    }

    /// Whether `ip` is in the configured subnet (of the range start).
    #[must_use]
    pub fn in_subnet(&self, ip: Ipv4Addr) -> bool {
        let m = u32::from(self.subnet_mask);
        u32::from(ip) & m == u32::from(self.range_start) & m
    }

    /// The reservation of `mac`.
    #[must_use]
    pub fn reservation(&self, mac: [u8; 6]) -> Option<Ipv4Addr> {
        self.reservations
            .iter()
            .find(|r| crate::parse_mac(&r.mac) == Some(mac))
            .map(|r| r.ip)
    }
}

impl Settings {
    /// Problems that keep the server from starting (an empty list is fine).
    #[must_use]
    pub fn validate(&self) -> Vec<Problem> {
        let mut out = Vec::new();
        for (i, a) in self.addresses.iter().enumerate() {
            if let Err(e) = parse_cidr(a) {
                problem(&mut out, &format!("addresses[{i}]"), e);
            }
        }
        let v4 = &self.v4;
        if v4.enabled {
            if v4.range_start > v4.range_end {
                problem(&mut out, "v4.range_end", "the range ends before it starts");
            }
            let m = u32::from(v4.subnet_mask);
            if m.leading_ones() + m.trailing_zeros() != 32 {
                problem(&mut out, "v4.subnet_mask", "not a contiguous mask");
            } else if !v4.in_subnet(v4.range_end) {
                problem(
                    &mut out,
                    "v4.range_end",
                    "outside the subnet of the range start",
                );
            }
            if v4.lease_secs < MIN_LEASE_SECS {
                problem(
                    &mut out,
                    "v4.lease_secs",
                    format!("shorter than {MIN_LEASE_SECS} s"),
                );
            }
            for (i, r) in v4.reservations.iter().enumerate() {
                if crate::parse_mac(&r.mac).is_none() {
                    problem(
                        &mut out,
                        &format!("v4.reservations[{i}].mac"),
                        format!("{}: not a MAC address", r.mac),
                    );
                }
            }
        }
        let v6 = &self.v6;
        if v6.enabled {
            if !v6.na_enabled && !v6.pd_enabled && v6.dns.is_empty() && v6.domains.is_empty() {
                problem(
                    &mut out,
                    "v6.na_enabled",
                    "nothing to hand out (no addresses, prefixes, DNS or domains)",
                );
            }
            if v6.na_enabled && v6.range_start > v6.range_end {
                problem(&mut out, "v6.range_end", "the range ends before it starts");
            }
            if v6.pd_enabled
                && (v6.pd_prefix_len > v6.pd_delegated_len
                    || v6.pd_delegated_len > 128
                    || v6.pd_delegated_len - v6.pd_prefix_len > 32)
            {
                problem(
                    &mut out,
                    "v6.pd_delegated_len",
                    "the delegated length must be between the pool length and 32 bits more",
                );
            }
            if v6.preferred_secs < MIN_LEASE_SECS || v6.valid_secs < v6.preferred_secs {
                problem(
                    &mut out,
                    "v6.valid_secs",
                    format!("lifetimes must be at least {MIN_LEASE_SECS} s and valid >= preferred"),
                );
            }
        }
        if v6.ra_enabled {
            if !(4..=1800).contains(&v6.ra_interval_secs) {
                problem(&mut out, "v6.ra_interval_secs", "between 4 and 1800 s");
            }
            if !v6.ra_prefix.trim().is_empty() {
                if let Err(e) = parse_prefix6(&v6.ra_prefix) {
                    problem(&mut out, "v6.ra_prefix", e);
                }
            }
        }
        if !v4.enabled && !v6.enabled && !v6.ra_enabled {
            problem(
                &mut out,
                "v4.enabled",
                "neither DHCPv4, DHCPv6 nor router advertisements are enabled",
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        assert_eq!(Settings::default().validate(), Vec::new());
        let mut s = Settings::default();
        s.v6.enabled = true;
        s.v6.pd_enabled = true;
        s.v6.ra_enabled = true;
        s.v6.ra_prefix = "2001:db8:1::1/64".into();
        assert_eq!(s.validate(), Vec::new());
    }

    #[test]
    fn problems() {
        let mut s = Settings::default();
        s.v4.range_end = "192.168.101.1".parse().unwrap();
        s.v4.subnet_mask = "255.0.255.0".parse().unwrap();
        s.v4.lease_secs = 1;
        s.v4.reservations.push(Reservation {
            mac: "nope".into(),
            ip: Ipv4Addr::LOCALHOST,
        });
        let fields: Vec<_> = s.validate().into_iter().map(|p| p.field).collect();
        assert_eq!(
            fields,
            ["v4.subnet_mask", "v4.lease_secs", "v4.reservations[0].mac"]
        );
        s.v4.enabled = false;
        let fields: Vec<_> = s.validate().into_iter().map(|p| p.field).collect();
        assert_eq!(fields, ["v4.enabled"]);
    }

    #[test]
    fn prefixes() {
        assert_eq!(
            parse_prefix6("2001:db8:1::99/64").unwrap(),
            ("2001:db8:1::".parse().unwrap(), 64)
        );
        assert!(parse_prefix6("2001:db8::").is_err());
        assert!(parse_prefix6("2001:db8::/129").is_err());
        assert_eq!(
            parse_cidr(" 192.168.1.1/24").unwrap(),
            ("192.168.1.1".parse().unwrap(), 24)
        );
        assert!(parse_cidr("2001:db8::1/64").is_ok());
        assert!(parse_cidr("192.168.1.1/33").is_err());
        assert!(parse_cidr("192.168.1.1").is_err());
        assert!(parse_cidr("0.0.0.0/8").is_err());
        let s = Settings {
            addresses: vec!["x/1".into()],
            ..Settings::default()
        };
        assert_eq!(s.validate()[0].field, "addresses[0]");
        assert_eq!(mask6(Ipv6Addr::LOCALHOST, 0), Ipv6Addr::UNSPECIFIED);
        assert_eq!(mask6(Ipv6Addr::LOCALHOST, 128), Ipv6Addr::LOCALHOST);
    }
}
