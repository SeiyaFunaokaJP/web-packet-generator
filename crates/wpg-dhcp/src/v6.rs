//! DHCPv6 messages and the options the server uses (RFC 8415, RFC 3646).

use std::net::Ipv6Addr;

/// Server / relay port.
pub const SERVER_PORT: u16 = 547;
/// Client port.
pub const CLIENT_PORT: u16 = 546;
/// `All_DHCP_Relay_Agents_and_Servers`.
pub const ALL_SERVERS: Ipv6Addr = Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 1, 2);

/// Message types.
pub mod msg {
    /// SOLICIT
    pub const SOLICIT: u8 = 1;
    /// ADVERTISE
    pub const ADVERTISE: u8 = 2;
    /// REQUEST
    pub const REQUEST: u8 = 3;
    /// CONFIRM
    pub const CONFIRM: u8 = 4;
    /// RENEW
    pub const RENEW: u8 = 5;
    /// REBIND
    pub const REBIND: u8 = 6;
    /// REPLY
    pub const REPLY: u8 = 7;
    /// RELEASE
    pub const RELEASE: u8 = 8;
    /// DECLINE
    pub const DECLINE: u8 = 9;
    /// INFORMATION-REQUEST
    pub const INFORMATION_REQUEST: u8 = 11;
    /// RELAY-FORW
    pub const RELAY_FORW: u8 = 12;
    /// RELAY-REPL
    pub const RELAY_REPL: u8 = 13;
}

/// Name of a message type.
#[must_use]
pub fn msg_name(t: u8) -> &'static str {
    match t {
        msg::SOLICIT => "SOLICIT",
        msg::ADVERTISE => "ADVERTISE",
        msg::REQUEST => "REQUEST",
        msg::CONFIRM => "CONFIRM",
        msg::RENEW => "RENEW",
        msg::REBIND => "REBIND",
        msg::REPLY => "REPLY",
        msg::RELEASE => "RELEASE",
        msg::DECLINE => "DECLINE",
        10 => "RECONFIGURE",
        msg::INFORMATION_REQUEST => "INFORMATION-REQUEST",
        msg::RELAY_FORW => "RELAY-FORW",
        msg::RELAY_REPL => "RELAY-REPL",
        _ => "UNKNOWN",
    }
}

/// Option codes.
pub mod opt {
    /// Client DUID.
    pub const CLIENTID: u16 = 1;
    /// Server DUID.
    pub const SERVERID: u16 = 2;
    /// Identity association for non-temporary addresses.
    pub const IA_NA: u16 = 3;
    /// Address inside an `IA_NA`.
    pub const IAADDR: u16 = 5;
    /// Option request.
    pub const ORO: u16 = 6;
    /// Server preference.
    pub const PREFERENCE: u16 = 7;
    /// Elapsed time.
    pub const ELAPSED_TIME: u16 = 8;
    /// Status code.
    pub const STATUS_CODE: u16 = 13;
    /// Rapid commit.
    pub const RAPID_COMMIT: u16 = 14;
    /// DNS recursive name servers (RFC 3646).
    pub const DNS_SERVERS: u16 = 23;
    /// Domain search list (RFC 3646).
    pub const DOMAIN_LIST: u16 = 24;
    /// Identity association for prefix delegation.
    pub const IA_PD: u16 = 25;
    /// Prefix inside an `IA_PD`.
    pub const IAPREFIX: u16 = 26;
}

/// Status codes.
pub mod status {
    /// Success.
    pub const SUCCESS: u16 = 0;
    /// Failure.
    pub const UNSPEC_FAIL: u16 = 1;
    /// No addresses available.
    pub const NO_ADDRS_AVAIL: u16 = 2;
    /// No binding for the IA.
    pub const NO_BINDING: u16 = 3;
    /// Addresses not on this link.
    pub const NOT_ON_LINK: u16 = 4;
    /// No prefixes available.
    pub const NO_PREFIX_AVAIL: u16 = 6;
}

/// Decoding errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    /// Shorter than the header.
    #[error("truncated message ({0} bytes)")]
    Truncated(usize),
    /// An option runs past the end.
    #[error("option {0} is truncated")]
    BadOption(u16),
}

/// Splits `b` into (code, data) options.
///
/// # Errors
/// [`DecodeError::BadOption`].
pub fn decode_options(mut b: &[u8]) -> Result<Vec<(u16, Vec<u8>)>, DecodeError> {
    let mut out = Vec::new();
    while !b.is_empty() {
        if b.len() < 4 {
            return Err(DecodeError::BadOption(0));
        }
        let code = u16::from_be_bytes([b[0], b[1]]);
        let len = usize::from(u16::from_be_bytes([b[2], b[3]]));
        let data = b.get(4..4 + len).ok_or(DecodeError::BadOption(code))?;
        out.push((code, data.to_vec()));
        b = &b[4 + len..];
    }
    Ok(out)
}

/// Appends one option.
pub fn put_option(b: &mut Vec<u8>, code: u16, data: &[u8]) {
    b.extend_from_slice(&code.to_be_bytes());
    b.extend_from_slice(&u16::try_from(data.len()).unwrap_or(u16::MAX).to_be_bytes());
    b.extend_from_slice(data);
}

/// A client / server message (not a relay message).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Message {
    /// Message type.
    pub msg_type: u8,
    /// Transaction id (24 bits).
    pub xid: u32,
    /// Options in order.
    pub options: Vec<(u16, Vec<u8>)>,
}

impl Message {
    /// Decodes a datagram.
    ///
    /// # Errors
    /// [`DecodeError`].
    pub fn decode(b: &[u8]) -> Result<Self, DecodeError> {
        if b.len() < 4 {
            return Err(DecodeError::Truncated(b.len()));
        }
        Ok(Self {
            msg_type: b[0],
            xid: u32::from_be_bytes([0, b[1], b[2], b[3]]),
            options: decode_options(&b[4..])?,
        })
    }

    /// Encodes the message.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut b = vec![self.msg_type];
        b.extend_from_slice(&self.xid.to_be_bytes()[1..]);
        for (code, data) in &self.options {
            put_option(&mut b, *code, data);
        }
        b
    }

    /// First option `code`.
    #[must_use]
    pub fn option(&self, code: u16) -> Option<&[u8]> {
        self.options
            .iter()
            .find(|(c, _)| *c == code)
            .map(|(_, d)| d.as_slice())
    }

    /// Whether option `code` is present.
    #[must_use]
    pub fn has(&self, code: u16) -> bool {
        self.options.iter().any(|(c, _)| *c == code)
    }

    /// Appends an option.
    pub fn push(&mut self, code: u16, data: Vec<u8>) {
        self.options.push((code, data));
    }

    /// The `IA_NA` / `IA_PD` options.
    #[must_use]
    pub fn ias(&self) -> Vec<Ia> {
        self.options
            .iter()
            .filter_map(|(c, d)| Ia::decode(*c, d))
            .collect()
    }
}

/// `IA_NA` or `IA_PD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IaKind {
    /// Non-temporary addresses.
    Na,
    /// Prefix delegation.
    Pd,
}

/// An address (`IAADDR`, length 128) or prefix (`IAPREFIX`) of an IA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IaAddr {
    /// Address / prefix.
    pub addr: Ipv6Addr,
    /// Prefix length (128 for addresses).
    pub plen: u8,
    /// Preferred lifetime.
    pub preferred: u32,
    /// Valid lifetime.
    pub valid: u32,
}

/// An identity association.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ia {
    /// NA / PD.
    pub kind: IaKind,
    /// IAID.
    pub iaid: u32,
    /// T1.
    pub t1: u32,
    /// T2.
    pub t2: u32,
    /// Addresses / prefixes.
    pub addrs: Vec<IaAddr>,
    /// Status code and message.
    pub status: Option<(u16, String)>,
}

fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn ip6(b: &[u8]) -> Ipv6Addr {
    let mut a = [0u8; 16];
    a.copy_from_slice(&b[..16]);
    Ipv6Addr::from(a)
}

/// Encodes a status code option's data.
#[must_use]
pub fn status_data(code: u16, message: &str) -> Vec<u8> {
    let mut d = code.to_be_bytes().to_vec();
    d.extend_from_slice(message.as_bytes());
    d
}

impl Ia {
    /// Decodes an `IA_NA` (3) or `IA_PD` (25) option; `None` for other codes
    /// or malformed data.
    #[must_use]
    pub fn decode(code: u16, d: &[u8]) -> Option<Self> {
        let kind = match code {
            opt::IA_NA => IaKind::Na,
            opt::IA_PD => IaKind::Pd,
            _ => return None,
        };
        if d.len() < 12 {
            return None;
        }
        let mut ia = Self {
            kind,
            iaid: be32(d, 0),
            t1: be32(d, 4),
            t2: be32(d, 8),
            addrs: Vec::new(),
            status: None,
        };
        for (c, s) in decode_options(&d[12..]).ok()? {
            match (kind, c) {
                (IaKind::Na, opt::IAADDR) if s.len() >= 24 => ia.addrs.push(IaAddr {
                    addr: ip6(&s),
                    plen: 128,
                    preferred: be32(&s, 16),
                    valid: be32(&s, 20),
                }),
                (IaKind::Pd, opt::IAPREFIX) if s.len() >= 25 => ia.addrs.push(IaAddr {
                    addr: ip6(&s[9..]),
                    plen: s[8],
                    preferred: be32(&s, 0),
                    valid: be32(&s, 4),
                }),
                (_, opt::STATUS_CODE) if s.len() >= 2 => {
                    ia.status = Some((
                        u16::from_be_bytes([s[0], s[1]]),
                        String::from_utf8_lossy(&s[2..]).into_owned(),
                    ));
                }
                _ => {}
            }
        }
        Some(ia)
    }

    /// Encodes the IA as (option code, data).
    #[must_use]
    pub fn encode(&self) -> (u16, Vec<u8>) {
        let mut d = Vec::new();
        d.extend_from_slice(&self.iaid.to_be_bytes());
        d.extend_from_slice(&self.t1.to_be_bytes());
        d.extend_from_slice(&self.t2.to_be_bytes());
        for a in &self.addrs {
            let mut s = Vec::new();
            match self.kind {
                IaKind::Na => {
                    s.extend_from_slice(&a.addr.octets());
                    s.extend_from_slice(&a.preferred.to_be_bytes());
                    s.extend_from_slice(&a.valid.to_be_bytes());
                    put_option(&mut d, opt::IAADDR, &s);
                }
                IaKind::Pd => {
                    s.extend_from_slice(&a.preferred.to_be_bytes());
                    s.extend_from_slice(&a.valid.to_be_bytes());
                    s.push(a.plen);
                    s.extend_from_slice(&a.addr.octets());
                    put_option(&mut d, opt::IAPREFIX, &s);
                }
            }
        }
        if let Some((code, m)) = &self.status {
            put_option(&mut d, opt::STATUS_CODE, &status_data(*code, m));
        }
        let code = match self.kind {
            IaKind::Na => opt::IA_NA,
            IaKind::Pd => opt::IA_PD,
        };
        (code, d)
    }
}

/// A domain search list in DNS wire format (RFC 1035 §3.1, uncompressed).
/// Empty labels are skipped; labels are cut to 63 bytes.
#[must_use]
pub fn encode_domains(domains: &[String]) -> Vec<u8> {
    let mut b = Vec::new();
    for d in domains {
        for label in d.trim().trim_end_matches('.').split('.') {
            let label = &label.as_bytes()[..label.len().min(63)];
            if !label.is_empty() {
                b.push(u8::try_from(label.len()).unwrap_or(63));
                b.extend_from_slice(label);
            }
        }
        b.push(0);
    }
    b
}

/// A DUID as `00:03:00:01:...`.
#[must_use]
pub fn fmt_duid(d: &[u8]) -> String {
    crate::fmt_mac(d)
}

/// DUID-LL (type 3, Ethernet) of `mac`.
#[must_use]
pub fn duid_ll(mac: [u8; 6]) -> Vec<u8> {
    let mut d = vec![0, 3, 0, 1];
    d.extend_from_slice(&mac);
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let na = Ia {
            kind: IaKind::Na,
            iaid: 7,
            t1: 100,
            t2: 160,
            addrs: vec![IaAddr {
                addr: "2001:db8::10".parse().unwrap(),
                plen: 128,
                preferred: 200,
                valid: 300,
            }],
            status: None,
        };
        let pd = Ia {
            kind: IaKind::Pd,
            iaid: 9,
            t1: 0,
            t2: 0,
            addrs: vec![IaAddr {
                addr: "2001:db8:100::".parse().unwrap(),
                plen: 56,
                preferred: 1,
                valid: 2,
            }],
            status: Some((status::SUCCESS, "ok".into())),
        };
        let mut m = Message {
            msg_type: msg::SOLICIT,
            xid: 0x00ab_cdef,
            options: vec![(opt::CLIENTID, duid_ll([0, 1, 2, 3, 4, 5]))],
        };
        let (c, d) = na.encode();
        m.push(c, d);
        let (c, d) = pd.encode();
        m.push(c, d);
        m.push(opt::RAPID_COMMIT, Vec::new());
        let b = m.encode();
        assert_eq!(&b[..4], &[1, 0xab, 0xcd, 0xef]);
        let back = Message::decode(&b).unwrap();
        assert_eq!(back, m);
        assert_eq!(back.ias(), vec![na, pd]);
        assert!(back.has(opt::RAPID_COMMIT));
        assert_eq!(
            fmt_duid(back.option(opt::CLIENTID).unwrap()),
            "00:03:00:01:00:01:02:03:04:05"
        );
    }

    #[test]
    fn domains_and_errors() {
        assert_eq!(
            encode_domains(&["example.com.".into(), "lab".into()]),
            b"\x07example\x03com\x00\x03lab\x00"
        );
        assert_eq!(Message::decode(&[1, 2]), Err(DecodeError::Truncated(2)));
        assert_eq!(
            Message::decode(&[1, 0, 0, 1, 0, 1, 0, 9, 0]),
            Err(DecodeError::BadOption(1))
        );
    }
}
