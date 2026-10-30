//! Router advertisements (RFC 4861 §4.2) telling hosts to use DHCPv6: the M
//! flag (addresses) when `IA_NA` is offered, the O flag (other configuration)
//! always, and optionally an on-link prefix.

use std::net::Ipv6Addr;

use crate::config::{parse_prefix6, V6Config};

/// ICMPv6 type of a router solicitation.
pub const ROUTER_SOLICITATION: u8 = 133;
/// ICMPv6 type of a router advertisement.
pub const ROUTER_ADVERTISEMENT: u8 = 134;
/// `All_Nodes`.
pub const ALL_NODES: Ipv6Addr = Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 1);
/// `All_Routers` (router solicitations are sent here).
pub const ALL_ROUTERS: Ipv6Addr = Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 2);

/// The ICMPv6 message (checksum 0: the kernel fills it in on raw ICMPv6
/// sockets).
#[must_use]
pub fn build(cfg: &V6Config, mac: Option<[u8; 6]>) -> Vec<u8> {
    let mut flags = 0x40; // O
    if cfg.enabled && cfg.na_enabled {
        flags |= 0x80; // M
    }
    let mut b = vec![ROUTER_ADVERTISEMENT, 0, 0, 0, 64, flags];
    b.extend_from_slice(&cfg.ra_router_lifetime_secs.to_be_bytes());
    b.extend_from_slice(&[0; 8]); // reachable time, retrans timer
    if let Some(mac) = mac {
        // Source link-layer address.
        b.extend_from_slice(&[1, 1]);
        b.extend_from_slice(&mac);
    }
    if let Ok((prefix, len)) = parse_prefix6(&cfg.ra_prefix) {
        let pflags = if cfg.ra_autonomous { 0xc0 } else { 0x80 }; // L (+ A)
        b.extend_from_slice(&[3, 4, len, pflags]);
        b.extend_from_slice(&cfg.valid_secs.to_be_bytes());
        b.extend_from_slice(&cfg.preferred_secs.to_be_bytes());
        b.extend_from_slice(&[0; 4]);
        b.extend_from_slice(&prefix.octets());
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advertisement() {
        let mut c = V6Config {
            enabled: true,
            ra_router_lifetime_secs: 1800,
            ..V6Config::default()
        };
        let b = build(&c, None);
        assert_eq!(
            b,
            [134, 0, 0, 0, 64, 0xc0, 0x07, 0x08, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        c.na_enabled = false;
        c.ra_prefix = "2001:db8:1::/64".into();
        c.ra_autonomous = true;
        let b = build(&c, Some([2, 0, 0, 0, 0, 1]));
        assert_eq!(b[5], 0x40);
        assert_eq!(&b[16..24], &[1, 1, 2, 0, 0, 0, 0, 1]);
        assert_eq!(&b[24..28], &[3, 4, 64, 0xc0]);
        assert_eq!(&b[28..32], &c.valid_secs.to_be_bytes());
        assert_eq!(b.len(), 24 + 32);
        assert_eq!(
            &b[40..56],
            &"2001:db8:1::".parse::<Ipv6Addr>().unwrap().octets()
        );
    }
}
