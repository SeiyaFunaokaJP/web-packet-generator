//! DHCPv4 messages (RFC 2131) and options (RFC 2132).

use std::net::Ipv4Addr;

/// Server port.
pub const SERVER_PORT: u16 = 67;
/// Client port.
pub const CLIENT_PORT: u16 = 68;
/// `op` of client messages.
pub const BOOTREQUEST: u8 = 1;
/// `op` of server messages.
pub const BOOTREPLY: u8 = 2;
/// The magic cookie in front of the options.
pub const MAGIC: [u8; 4] = [99, 130, 83, 99];
/// Fixed header length (up to and including the cookie).
pub const HEADER_LEN: usize = 240;
/// Replies are padded to the BOOTP minimum.
pub const MIN_LEN: usize = 300;
/// `flags` bit asking for broadcast replies.
pub const FLAG_BROADCAST: u16 = 0x8000;

/// Option codes used by the server.
pub mod opt {
    /// Pad.
    pub const PAD: u8 = 0;
    /// Subnet mask.
    pub const SUBNET_MASK: u8 = 1;
    /// Routers.
    pub const ROUTER: u8 = 3;
    /// DNS servers.
    pub const DNS: u8 = 6;
    /// Host name.
    pub const HOSTNAME: u8 = 12;
    /// Domain name.
    pub const DOMAIN: u8 = 15;
    /// Broadcast address.
    pub const BROADCAST: u8 = 28;
    /// NTP servers.
    pub const NTP: u8 = 42;
    /// Requested IP address.
    pub const REQUESTED_IP: u8 = 50;
    /// Lease time.
    pub const LEASE_TIME: u8 = 51;
    /// Message type.
    pub const MESSAGE_TYPE: u8 = 53;
    /// Server identifier.
    pub const SERVER_ID: u8 = 54;
    /// Parameter request list.
    pub const PARAMS: u8 = 55;
    /// Message.
    pub const MESSAGE: u8 = 56;
    /// Renewal (T1) time.
    pub const T1: u8 = 58;
    /// Rebinding (T2) time.
    pub const T2: u8 = 59;
    /// Client identifier.
    pub const CLIENT_ID: u8 = 61;
    /// Relay agent information (RFC 3046).
    pub const RELAY_AGENT: u8 = 82;
    /// End.
    pub const END: u8 = 255;
}

/// DHCP message types (option 53).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    /// 1
    Discover,
    /// 2
    Offer,
    /// 3
    Request,
    /// 4
    Decline,
    /// 5
    Ack,
    /// 6
    Nak,
    /// 7
    Release,
    /// 8
    Inform,
}

impl MessageType {
    /// From the option value.
    #[must_use]
    pub fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            1 => Self::Discover,
            2 => Self::Offer,
            3 => Self::Request,
            4 => Self::Decline,
            5 => Self::Ack,
            6 => Self::Nak,
            7 => Self::Release,
            8 => Self::Inform,
            _ => return None,
        })
    }

    /// The option value.
    #[must_use]
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Discover => 1,
            Self::Offer => 2,
            Self::Request => 3,
            Self::Decline => 4,
            Self::Ack => 5,
            Self::Nak => 6,
            Self::Release => 7,
            Self::Inform => 8,
        }
    }

    /// `DISCOVER`, `OFFER`, ...
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Discover => "DISCOVER",
            Self::Offer => "OFFER",
            Self::Request => "REQUEST",
            Self::Decline => "DECLINE",
            Self::Ack => "ACK",
            Self::Nak => "NAK",
            Self::Release => "RELEASE",
            Self::Inform => "INFORM",
        }
    }
}

/// Decoding errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    /// Shorter than the fixed header.
    #[error("truncated message ({0} bytes)")]
    Truncated(usize),
    /// The magic cookie is missing (plain BOOTP).
    #[error("no DHCP magic cookie")]
    NoCookie,
    /// An option runs past the end.
    #[error("option {0} is truncated")]
    BadOption(u8),
}

/// A DHCPv4 message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// `BOOTREQUEST` / `BOOTREPLY`.
    pub op: u8,
    /// Hardware type (1 = Ethernet).
    pub htype: u8,
    /// Hardware address length.
    pub hlen: u8,
    /// Relay hops.
    pub hops: u8,
    /// Transaction id.
    pub xid: u32,
    /// Seconds since the client began.
    pub secs: u16,
    /// Flags (broadcast bit).
    pub flags: u16,
    /// Client address (renewing clients).
    pub ciaddr: Ipv4Addr,
    /// "Your" address.
    pub yiaddr: Ipv4Addr,
    /// Next server.
    pub siaddr: Ipv4Addr,
    /// Relay agent.
    pub giaddr: Ipv4Addr,
    /// Client hardware address (padded to 16 bytes).
    pub chaddr: [u8; 16],
    /// Options in order (code, data); pad / end are not listed.
    pub options: Vec<(u8, Vec<u8>)>,
}

impl Default for Message {
    fn default() -> Self {
        Self {
            op: BOOTREQUEST,
            htype: 1,
            hlen: 6,
            hops: 0,
            xid: 0,
            secs: 0,
            flags: 0,
            ciaddr: Ipv4Addr::UNSPECIFIED,
            yiaddr: Ipv4Addr::UNSPECIFIED,
            siaddr: Ipv4Addr::UNSPECIFIED,
            giaddr: Ipv4Addr::UNSPECIFIED,
            chaddr: [0; 16],
            options: Vec::new(),
        }
    }
}

fn ip_at(b: &[u8], at: usize) -> Ipv4Addr {
    Ipv4Addr::new(b[at], b[at + 1], b[at + 2], b[at + 3])
}

impl Message {
    /// Decodes a datagram. Options in `sname` / `file` (overload) are ignored.
    ///
    /// # Errors
    /// [`DecodeError`].
    pub fn decode(b: &[u8]) -> Result<Self, DecodeError> {
        if b.len() < HEADER_LEN {
            return Err(DecodeError::Truncated(b.len()));
        }
        if b[236..240] != MAGIC {
            return Err(DecodeError::NoCookie);
        }
        let mut chaddr = [0u8; 16];
        chaddr.copy_from_slice(&b[28..44]);
        let mut options: Vec<(u8, Vec<u8>)> = Vec::new();
        let mut i = HEADER_LEN;
        while i < b.len() {
            let code = b[i];
            match code {
                opt::PAD => i += 1,
                opt::END => break,
                _ => {
                    let len = *b.get(i + 1).ok_or(DecodeError::BadOption(code))? as usize;
                    let data = b
                        .get(i + 2..i + 2 + len)
                        .ok_or(DecodeError::BadOption(code))?;
                    // RFC 3396: repeated options are concatenated.
                    if let Some((_, d)) = options.iter_mut().find(|(c, _)| *c == code) {
                        d.extend_from_slice(data);
                    } else {
                        options.push((code, data.to_vec()));
                    }
                    i += 2 + len;
                }
            }
        }
        Ok(Self {
            op: b[0],
            htype: b[1],
            hlen: b[2],
            hops: b[3],
            xid: u32::from_be_bytes([b[4], b[5], b[6], b[7]]),
            secs: u16::from_be_bytes([b[8], b[9]]),
            flags: u16::from_be_bytes([b[10], b[11]]),
            ciaddr: ip_at(b, 12),
            yiaddr: ip_at(b, 16),
            siaddr: ip_at(b, 20),
            giaddr: ip_at(b, 24),
            chaddr,
            options,
        })
    }

    /// Encodes the message (options longer than 255 bytes are split, the
    /// result is padded to [`MIN_LEN`]).
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(MIN_LEN);
        b.extend_from_slice(&[self.op, self.htype, self.hlen, self.hops]);
        b.extend_from_slice(&self.xid.to_be_bytes());
        b.extend_from_slice(&self.secs.to_be_bytes());
        b.extend_from_slice(&self.flags.to_be_bytes());
        for a in [self.ciaddr, self.yiaddr, self.siaddr, self.giaddr] {
            b.extend_from_slice(&a.octets());
        }
        b.extend_from_slice(&self.chaddr);
        b.resize(HEADER_LEN - 4, 0); // sname + file
        b.extend_from_slice(&MAGIC);
        for (code, data) in &self.options {
            if data.is_empty() {
                b.extend_from_slice(&[*code, 0]);
            }
            for chunk in data.chunks(255) {
                b.push(*code);
                b.push(u8::try_from(chunk.len()).unwrap_or(u8::MAX));
                b.extend_from_slice(chunk);
            }
        }
        b.push(opt::END);
        if b.len() < MIN_LEN {
            b.resize(MIN_LEN, 0);
        }
        b
    }

    /// Data of option `code`.
    #[must_use]
    pub fn option(&self, code: u8) -> Option<&[u8]> {
        self.options
            .iter()
            .find(|(c, _)| *c == code)
            .map(|(_, d)| d.as_slice())
    }

    /// Adds (or replaces) an option.
    pub fn set_option(&mut self, code: u8, data: Vec<u8>) {
        if let Some((_, d)) = self.options.iter_mut().find(|(c, _)| *c == code) {
            *d = data;
        } else {
            self.options.push((code, data));
        }
    }

    /// Option 53.
    #[must_use]
    pub fn message_type(&self) -> Option<MessageType> {
        self.option(opt::MESSAGE_TYPE)
            .and_then(|d| d.first().copied())
            .and_then(MessageType::from_u8)
    }

    /// An IPv4 address option (50, 54).
    #[must_use]
    pub fn ip_option(&self, code: u8) -> Option<Ipv4Addr> {
        match self.option(code)? {
            [a, b, c, d] => Some(Ipv4Addr::new(*a, *b, *c, *d)),
            _ => None,
        }
    }

    /// The Ethernet address of the client.
    #[must_use]
    pub fn mac(&self) -> [u8; 6] {
        let mut m = [0u8; 6];
        m.copy_from_slice(&self.chaddr[..6]);
        m
    }

    /// Option 12 as text.
    #[must_use]
    pub fn hostname(&self) -> Option<String> {
        self.option(opt::HOSTNAME)
            .map(|d| String::from_utf8_lossy(d).trim_end_matches('\0').to_owned())
            .filter(|s| !s.is_empty())
    }
}

/// Concatenated addresses (options 3, 6, 42).
#[must_use]
pub fn ip_list(ips: &[Ipv4Addr]) -> Vec<u8> {
    ips.iter().flat_map(Ipv4Addr::octets).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut m = Message {
            xid: 0x1234_5678,
            flags: FLAG_BROADCAST,
            ciaddr: Ipv4Addr::new(10, 0, 0, 5),
            ..Message::default()
        };
        m.chaddr[..6].copy_from_slice(&[0, 1, 2, 3, 4, 5]);
        m.set_option(opt::MESSAGE_TYPE, vec![1]);
        m.set_option(opt::HOSTNAME, b"pc1".to_vec());
        m.set_option(opt::PARAMS, (0..=255).collect());
        let b = m.encode();
        assert!(b.len() >= MIN_LEN);
        let d = Message::decode(&b).unwrap();
        // The 256-byte option was split and joined again.
        assert_eq!(d, m);
        assert_eq!(d.message_type(), Some(MessageType::Discover));
        assert_eq!(d.hostname().as_deref(), Some("pc1"));
        assert_eq!(d.mac(), [0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn errors() {
        assert_eq!(Message::decode(&[0; 10]), Err(DecodeError::Truncated(10)));
        assert_eq!(Message::decode(&[0; 240]), Err(DecodeError::NoCookie));
        let mut b = Message::default().encode();
        b.truncate(HEADER_LEN);
        b.extend_from_slice(&[opt::HOSTNAME, 10, b'a']);
        assert_eq!(Message::decode(&b), Err(DecodeError::BadOption(12)));
    }
}
