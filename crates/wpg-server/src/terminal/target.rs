//! `terminal.allowed_targets`: which hosts a web terminal session may reach.
//!
//! An entry is one of
//!
//! * a host name, optionally with a port: `r1`, `r1.lab:23`, `*.lab.example`
//!   (the leading `*.` matches any subdomain);
//! * an IP address, optionally with a port: `192.0.2.1`, `192.0.2.1:2222`,
//!   `2001:db8::1`, `[2001:db8::1]:22`;
//! * a CIDR block: `10.0.0.0/8`, `2001:db8::/32`.
//!
//! Host names match the name the user typed (case-insensitive); addresses and
//! blocks match the addresses that name resolves to. [`Allowlist::permit`] returns
//! the resolved addresses the session may connect to, so the address that was
//! checked is the one that is dialled (no second lookup a DNS rebinding could
//! change).

use std::net::{IpAddr, SocketAddr};

use thiserror::Error;

/// Why an allowlist entry or a target does not parse.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum TargetError {
    /// Empty entry / host.
    #[error("empty host")]
    Empty,
    /// The port is not a number in 1..=65535.
    #[error("invalid port {0:?}")]
    Port(String),
    /// The prefix length exceeds the address size.
    #[error("invalid prefix length {0:?}")]
    Prefix(String),
    /// Not an IP address before the `/`.
    #[error("invalid network address {0:?}")]
    Network(String),
    /// Characters a host name cannot contain (or a misplaced `*`).
    #[error("invalid host name {0:?}")]
    Host(String),
}

/// One parsed allowlist entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rule {
    /// A host name (lower case, no trailing dot); `*.suffix` when `wildcard`.
    Name {
        /// The name, or the suffix after `*.`.
        name: String,
        /// `*.name`: any subdomain of `name`.
        wildcard: bool,
        /// Only this port.
        port: Option<u16>,
    },
    /// An address block (a single address is a /32 or /128).
    Net {
        /// Network address.
        addr: IpAddr,
        /// Prefix length.
        prefix: u8,
        /// Only this port.
        port: Option<u16>,
    },
}

fn parse_port(p: &str) -> Result<u16, TargetError> {
    match p.parse::<u16>() {
        Ok(n) if n > 0 => Ok(n),
        _ => Err(TargetError::Port(p.to_owned())),
    }
}

/// Splits `host[:port]` / `[v6]:port` / bare IPv6.
///
/// # Errors
/// An empty host or a bad port.
pub fn split_host_port(s: &str) -> Result<(String, Option<u16>), TargetError> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix('[') {
        let (host, after) = rest
            .split_once(']')
            .ok_or_else(|| TargetError::Host(s.to_owned()))?;
        let port = match after {
            "" => None,
            p => Some(parse_port(
                p.strip_prefix(':')
                    .ok_or_else(|| TargetError::Host(s.to_owned()))?,
            )?),
        };
        if host.is_empty() {
            return Err(TargetError::Empty);
        }
        return Ok((host.to_owned(), port));
    }
    let (host, port) = match s.matches(':').count() {
        1 => {
            let (h, p) = s.split_once(':').unwrap_or((s, ""));
            (h, Some(parse_port(p)?))
        }
        // none, or a bare IPv6 address
        _ => (s, None),
    };
    if host.is_empty() {
        return Err(TargetError::Empty);
    }
    Ok((host.to_owned(), port))
}

/// Normalises a host name for comparison (lower case, no trailing dot).
#[must_use]
pub fn normalize_host(host: &str) -> String {
    host.trim().trim_end_matches('.').to_ascii_lowercase()
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 253
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_'))
        && !name.starts_with('.')
        && !name.contains("..")
}

/// Validates the host of a connection request (a name or an IP address).
///
/// # Errors
/// [`TargetError::Empty`] / [`TargetError::Host`].
pub fn check_host(host: &str) -> Result<(), TargetError> {
    let h = host.trim();
    if h.is_empty() {
        return Err(TargetError::Empty);
    }
    if h.parse::<IpAddr>().is_ok() || valid_name(&normalize_host(h)) {
        Ok(())
    } else {
        Err(TargetError::Host(h.to_owned()))
    }
}

impl Rule {
    /// Parses one `terminal.allowed_targets` entry.
    ///
    /// # Errors
    /// See [`TargetError`].
    pub fn parse(entry: &str) -> Result<Self, TargetError> {
        let entry = entry.trim();
        if entry.is_empty() {
            return Err(TargetError::Empty);
        }
        if let Some((net, len)) = entry.split_once('/') {
            let addr: IpAddr = net
                .trim()
                .parse()
                .map_err(|_| TargetError::Network(net.to_owned()))?;
            let max = if addr.is_ipv4() { 32 } else { 128 };
            let prefix = len
                .trim()
                .parse::<u8>()
                .ok()
                .filter(|p| *p <= max)
                .ok_or_else(|| TargetError::Prefix(len.to_owned()))?;
            return Ok(Rule::Net {
                addr: canonical(addr),
                prefix,
                port: None,
            });
        }
        let (host, port) = split_host_port(entry)?;
        if let Ok(addr) = host.parse::<IpAddr>() {
            let addr = canonical(addr);
            return Ok(Rule::Net {
                addr,
                prefix: if addr.is_ipv4() { 32 } else { 128 },
                port,
            });
        }
        let host = normalize_host(&host);
        let (name, wildcard) = match host.strip_prefix("*.") {
            Some(rest) => (rest.to_owned(), true),
            None => (host, false),
        };
        if !valid_name(&name) {
            return Err(TargetError::Host(entry.to_owned()));
        }
        Ok(Rule::Name {
            name,
            wildcard,
            port,
        })
    }

    fn port_ok(port: Option<u16>, p: u16) -> bool {
        port.is_none_or(|x| x == p)
    }

    /// Whether the rule admits host name `host` (normalised) on `port`.
    fn matches_name(&self, host: &str, port: u16) -> bool {
        match self {
            Rule::Name {
                name,
                wildcard,
                port: rp,
            } => {
                let hit = if *wildcard {
                    host.len() > name.len()
                        && host.ends_with(name.as_str())
                        && host[..host.len() - name.len()].ends_with('.')
                } else {
                    host == name
                };
                hit && Self::port_ok(*rp, port)
            }
            Rule::Net { .. } => false,
        }
    }

    /// Whether the rule admits address `sa`.
    fn matches_addr(&self, sa: SocketAddr) -> bool {
        match self {
            Rule::Net { addr, prefix, port } => {
                Self::port_ok(*port, sa.port()) && in_net(canonical(sa.ip()), *addr, *prefix)
            }
            Rule::Name { .. } => false,
        }
    }
}

/// IPv4-mapped IPv6 addresses compare as IPv4.
fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        v4 @ IpAddr::V4(_) => v4,
    }
}

fn in_net(ip: IpAddr, net: IpAddr, prefix: u8) -> bool {
    match (ip, net) {
        (IpAddr::V4(a), IpAddr::V4(n)) => {
            let mask = u32::MAX.checked_shl(32 - u32::from(prefix)).unwrap_or(0);
            u32::from(a) & mask == u32::from(n) & mask
        }
        (IpAddr::V6(a), IpAddr::V6(n)) => {
            let mask = u128::MAX.checked_shl(128 - u32::from(prefix)).unwrap_or(0);
            u128::from(a) & mask == u128::from(n) & mask
        }
        _ => false,
    }
}

/// The parsed `terminal.allowed_targets` (invalid entries are skipped; `wpg
/// config validate` reports them).
#[derive(Debug, Clone, Default)]
pub struct Allowlist {
    rules: Vec<Rule>,
    any: bool,
}

impl Allowlist {
    /// Parses the configured entries; an empty list allows every target.
    #[must_use]
    pub fn new(entries: &[String]) -> Self {
        let rules: Vec<Rule> = entries
            .iter()
            .filter_map(|e| match Rule::parse(e) {
                Ok(r) => Some(r),
                Err(err) => {
                    tracing::warn!("terminal.allowed_targets: ignoring {e:?}: {err}");
                    None
                }
            })
            .collect();
        Self {
            any: entries.is_empty(),
            rules,
        }
    }

    /// Every target is allowed.
    #[must_use]
    pub fn is_any(&self) -> bool {
        self.any
    }

    /// The addresses of `addrs` (the resolution of `host`) a session to
    /// `host:port` may connect to. Empty = refused.
    #[must_use]
    pub fn permit(&self, host: &str, port: u16, addrs: &[SocketAddr]) -> Vec<SocketAddr> {
        let host = normalize_host(host);
        if self.any || self.rules.iter().any(|r| r.matches_name(&host, port)) {
            return addrs.to_vec();
        }
        addrs
            .iter()
            .copied()
            .filter(|a| self.rules.iter().any(|r| r.matches_addr(*a)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sa(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    #[test]
    fn parses_entries() {
        assert_eq!(
            Rule::parse("R1.Lab.").unwrap(),
            Rule::Name {
                name: "r1.lab".into(),
                wildcard: false,
                port: None
            }
        );
        assert_eq!(
            Rule::parse("*.lab.example:23").unwrap(),
            Rule::Name {
                name: "lab.example".into(),
                wildcard: true,
                port: Some(23)
            }
        );
        assert_eq!(
            Rule::parse("10.1.2.0/24").unwrap(),
            Rule::Net {
                addr: "10.1.2.0".parse().unwrap(),
                prefix: 24,
                port: None
            }
        );
        assert_eq!(
            Rule::parse("[2001:db8::1]:830").unwrap(),
            Rule::Net {
                addr: "2001:db8::1".parse().unwrap(),
                prefix: 128,
                port: Some(830)
            }
        );
        assert_eq!(
            Rule::parse("2001:db8::1").unwrap(),
            Rule::Net {
                addr: "2001:db8::1".parse().unwrap(),
                prefix: 128,
                port: None
            }
        );
        assert!(matches!(
            Rule::parse("192.0.2.1:22"),
            Ok(Rule::Net {
                prefix: 32,
                port: Some(22),
                ..
            })
        ));
        for bad in [
            "",
            "10.0.0.0/33",
            "x/8",
            "r1:0",
            "r1:99999",
            "r1:x",
            "lab*",
            "a b",
            "[::1",
            "[]:22",
            "*.",
        ] {
            assert!(Rule::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn empty_list_allows_everything() {
        let a = Allowlist::new(&[]);
        assert!(a.is_any());
        let addrs = [sa("203.0.113.9:22")];
        assert_eq!(a.permit("anything", 22, &addrs), addrs);
    }

    #[test]
    fn names_ports_and_networks() {
        let a = Allowlist::new(&[
            "r1".into(),
            "sw1.lab:23".into(),
            "*.cml.example".into(),
            "10.10.0.0/16".into(),
            "[2001:db8::5]:22".into(),
        ]);
        assert!(!a.is_any());
        let far = [sa("198.51.100.1:22")];
        // by name, any port; resolution does not matter
        assert_eq!(a.permit("R1", 2222, &far), far);
        // name + port
        assert_eq!(a.permit("sw1.lab.", 23, &far), far);
        assert!(a.permit("sw1.lab", 22, &far).is_empty());
        // wildcard: subdomains only
        assert_eq!(a.permit("xr.cml.example", 22, &far), far);
        assert!(a.permit("cml.example", 22, &far).is_empty());
        assert!(a.permit("evilcml.example", 22, &far).is_empty());
        // by address: only the matching resolved addresses
        let mixed = [sa("10.10.3.4:22"), sa("10.11.0.1:22")];
        assert_eq!(a.permit("dev", 22, &mixed), vec![mixed[0]]);
        // IPv4-mapped IPv6 counts as IPv4
        let mapped = [sa("[::ffff:10.10.0.1]:22")];
        assert_eq!(a.permit("dev", 22, &mapped), mapped);
        // IPv6 with port
        assert_eq!(a.permit("v6", 22, &[sa("[2001:db8::5]:22")]).len(), 1);
        assert!(a.permit("v6", 23, &[sa("[2001:db8::5]:23")]).is_empty());
        assert!(a.permit("unknown", 22, &far).is_empty());
    }

    #[test]
    fn host_port_split_and_check() {
        assert_eq!(split_host_port("r1").unwrap(), ("r1".into(), None));
        assert_eq!(split_host_port("r1:23").unwrap(), ("r1".into(), Some(23)));
        assert_eq!(split_host_port("::1").unwrap(), ("::1".into(), None));
        assert_eq!(
            split_host_port("[::1]:22").unwrap(),
            ("::1".into(), Some(22))
        );
        assert!(check_host("r1.lab").is_ok());
        assert!(check_host("2001:db8::1").is_ok());
        assert!(check_host("").is_err());
        assert!(check_host("a b").is_err());
        assert!(check_host("x;rm").is_err());
    }

    #[test]
    fn network_edges() {
        assert!(in_net(
            "0.0.0.1".parse().unwrap(),
            "0.0.0.0".parse().unwrap(),
            0
        ));
        assert!(in_net(
            "10.0.0.255".parse().unwrap(),
            "10.0.0.0".parse().unwrap(),
            24
        ));
        assert!(!in_net(
            "10.0.1.0".parse().unwrap(),
            "10.0.0.0".parse().unwrap(),
            24
        ));
        assert!(!in_net(
            "::1".parse().unwrap(),
            "10.0.0.0".parse().unwrap(),
            8
        ));
        assert!(in_net(
            "2001:db8::abcd".parse().unwrap(),
            "2001:db8::".parse().unwrap(),
            32
        ));
    }
}
