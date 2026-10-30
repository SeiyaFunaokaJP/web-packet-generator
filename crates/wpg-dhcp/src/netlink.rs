//! Interface addresses through rtnetlink (Linux): bring the link up, add and
//! remove addresses, like `ip link set up` / `ip addr replace` / `ip addr del`
//! but from the server process (needs `CAP_NET_ADMIN`). Addresses added here
//! are not persistent; `scripts/deploy.sh --dhcp-iface-addr` writes a netplan
//! file for that.

use std::net::IpAddr;

/// Builds an `RTM_NEWADDR` (`add`) / `RTM_DELADDR` request.
#[must_use]
pub fn addr_request(add: bool, index: u32, addr: IpAddr, plen: u8, seq: u32) -> Vec<u8> {
    const RTM_NEWADDR: u16 = 20;
    const RTM_DELADDR: u16 = 21;
    const NLM_F_REQUEST: u16 = 0x1;
    const NLM_F_ACK: u16 = 0x4;
    const NLM_F_REPLACE: u16 = 0x100;
    const NLM_F_CREATE: u16 = 0x400;
    const IFA_ADDRESS: u16 = 1;
    const IFA_LOCAL: u16 = 2;
    const AF_INET: u8 = 2;
    const AF_INET6: u8 = 10;
    let (family, octets) = match addr {
        IpAddr::V4(a) => (AF_INET, a.octets().to_vec()),
        IpAddr::V6(a) => (AF_INET6, a.octets().to_vec()),
    };
    let (kind, flags) = if add {
        (
            RTM_NEWADDR,
            NLM_F_REQUEST | NLM_F_ACK | NLM_F_CREATE | NLM_F_REPLACE,
        )
    } else {
        (RTM_DELADDR, NLM_F_REQUEST | NLM_F_ACK)
    };
    let mut body = vec![family, plen, 0, 0]; // ifaddrmsg: family, prefixlen, flags, scope
    body.extend_from_slice(&index.to_ne_bytes());
    for t in [IFA_LOCAL, IFA_ADDRESS] {
        let len = u16::try_from(4 + octets.len()).unwrap_or(u16::MAX);
        body.extend_from_slice(&len.to_ne_bytes());
        body.extend_from_slice(&t.to_ne_bytes());
        body.extend_from_slice(&octets);
    }
    header(kind, flags, seq, &body)
}

/// Builds an `RTM_NEWLINK` request setting `IFF_UP`.
#[must_use]
pub fn up_request(index: u32, seq: u32) -> Vec<u8> {
    const RTM_NEWLINK: u16 = 16;
    const IFF_UP: u32 = 0x1;
    // ifinfomsg: family, pad, type, index, flags, change.
    let mut body = vec![0u8, 0, 0, 0];
    body.extend_from_slice(&index.to_ne_bytes());
    body.extend_from_slice(&IFF_UP.to_ne_bytes());
    body.extend_from_slice(&IFF_UP.to_ne_bytes());
    header(RTM_NEWLINK, 0x1 | 0x4, seq, &body)
}

fn header(kind: u16, flags: u16, seq: u32, body: &[u8]) -> Vec<u8> {
    let len = u32::try_from(16 + body.len()).unwrap_or(u32::MAX);
    let mut b = Vec::with_capacity(16 + body.len());
    b.extend_from_slice(&len.to_ne_bytes());
    b.extend_from_slice(&kind.to_ne_bytes());
    b.extend_from_slice(&flags.to_ne_bytes());
    b.extend_from_slice(&seq.to_ne_bytes());
    b.extend_from_slice(&0u32.to_ne_bytes());
    b.extend_from_slice(body);
    b
}

/// The errno of an `NLMSG_ERROR` answer (0 = acknowledged), `None` for other
/// messages.
#[must_use]
pub fn ack_errno(reply: &[u8]) -> Option<i32> {
    const NLMSG_ERROR: u16 = 2;
    if reply.len() < 20 || u16::from_ne_bytes([reply[4], reply[5]]) != NLMSG_ERROR {
        return None;
    }
    Some(-i32::from_ne_bytes([
        reply[16], reply[17], reply[18], reply[19],
    ]))
}

#[cfg(target_os = "linux")]
fn transact(req: &[u8]) -> std::io::Result<()> {
    use std::io::Read;

    use socket2::{Domain, Protocol, Socket, Type};
    const AF_NETLINK: i32 = 16;
    let s = Socket::new(Domain::from(AF_NETLINK), Type::RAW, Some(Protocol::from(0)))?;
    s.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
    s.send(req)?;
    let mut buf = [0u8; 4096];
    let n = (&s).read(&mut buf)?;
    match ack_errno(&buf[..n]) {
        Some(0) => Ok(()),
        Some(e) => Err(std::io::Error::from_raw_os_error(e)),
        None => Err(std::io::Error::other("unexpected netlink answer")),
    }
}

#[cfg(not(target_os = "linux"))]
fn transact(_req: &[u8]) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "interface addresses can be set on Linux only",
    ))
}

/// Brings interface `index` up.
///
/// # Errors
/// Netlink errors (`PermissionDenied` without `CAP_NET_ADMIN`).
pub fn set_up(index: u32) -> std::io::Result<()> {
    transact(&up_request(index, 1))
}

/// Adds (or refreshes) `addr/plen` on interface `index`.
///
/// # Errors
/// Netlink errors (`PermissionDenied` without `CAP_NET_ADMIN`).
pub fn add_address(index: u32, addr: IpAddr, plen: u8) -> std::io::Result<()> {
    transact(&addr_request(true, index, addr, plen, 2))
}

/// Removes `addr/plen` from interface `index`.
///
/// # Errors
/// Netlink errors (`EADDRNOTAVAIL` when it is not there).
pub fn del_address(index: u32, addr: IpAddr, plen: u8) -> std::io::Result<()> {
    transact(&addr_request(false, index, addr, plen, 3))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests() {
        let b = addr_request(true, 3, "192.168.1.1".parse().unwrap(), 24, 7);
        assert_eq!(b.len(), 16 + 8 + 2 * 8);
        assert_eq!(
            u32::from_ne_bytes(b[0..4].try_into().unwrap()) as usize,
            b.len()
        );
        assert_eq!(u16::from_ne_bytes([b[4], b[5]]), 20);
        assert_eq!(u16::from_ne_bytes([b[6], b[7]]), 0x505);
        assert_eq!(&b[16..20], &[2, 24, 0, 0]);
        assert_eq!(u32::from_ne_bytes(b[20..24].try_into().unwrap()), 3);
        assert_eq!(&b[28..32], &[192, 168, 1, 1]);
        let b = addr_request(false, 3, "2001:db8::1".parse().unwrap(), 64, 7);
        assert_eq!(b.len(), 16 + 8 + 2 * 20);
        assert_eq!(u16::from_ne_bytes([b[4], b[5]]), 21);
        assert_eq!(up_request(3, 1).len(), 32);
        let mut err = vec![0u8; 36];
        err[4..6].copy_from_slice(&2u16.to_ne_bytes());
        err[16..20].copy_from_slice(&(-17i32).to_ne_bytes());
        assert_eq!(ack_errno(&err), Some(17));
        assert_eq!(ack_errno(&[0; 8]), None);
    }
}
