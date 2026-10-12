//! Capture filters: a subset of the tcpdump / BPF filter language, evaluated in user
//! space so it behaves the same on every backend and OS.
//!
//! Grammar (keywords are case-insensitive):
//!
//! ```text
//! expr     := and ( ("or" | "||") and )*
//! and      := unary ( ("and" | "&&") unary )*
//! unary    := ("not" | "!") unary | "(" expr ")" | primitive
//! primitive:
//!   ether [src|dst] host? <mac> | ether proto <n|ip|ip6|arp|vlan|...>
//!   ether broadcast | ether multicast | broadcast | multicast
//!   vlan [<id>] | mpls
//!   ip | ip6 | arp | rarp | tcp | udp | icmp | icmp6 | sctp
//!   ip proto <n|name> | ip6 proto <n|name> | proto <n|name>
//!   [src|dst] host <ipv4|ipv6>  |  src|dst <address>
//!   [src|dst] net <cidr>        |  net <addr> mask <mask>
//!   [tcp|udp|sctp] [src|dst] port <n>
//!   [tcp|udp|sctp] [src|dst] portrange <a>-<b>
//!   less <n> | greater <n> | len (<|<=|>|>=|=|==|!=) <n>
//!   wpg                                        (frames carrying a wpg signature)
//! ```
//!
//! Differences from tcpdump: `vlan` does not shift the offsets of the primitives
//! after it — every primitive looks through all 802.1Q / 802.1ad tags; `host` /
//! `net` also match the ARP sender / target addresses; byte-offset expressions
//! (`ip[0] & 0xf`) are not supported.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// A filter syntax error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterError {
    /// Byte offset of the offending token in the filter text.
    pub pos: usize,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for FilterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "filter: {} (at offset {})", self.message, self.pos)
    }
}

impl std::error::Error for FilterError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dir {
    Any,
    Src,
    Dst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cmp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Prim {
    EtherHost(Dir, [u8; 6]),
    EtherProto(u16),
    EtherBroadcast,
    EtherMulticast,
    Vlan(Option<u16>),
    Mpls,
    Ip,
    Ip6,
    Arp,
    Rarp,
    /// IP protocol (IPv4 or IPv6) — `tcp`, `udp`, `ip proto N`, ...
    Proto(Option<bool>, u8),
    Host(Dir, IpAddr),
    Net(Dir, IpAddr, u8),
    /// `(transport, dir, lo, hi)`; transport `None` = TCP, UDP or SCTP.
    Port(Option<u8>, Dir, u16, u16),
    Len(Cmp, usize),
    Signature,
}

/// Parsed filter expression.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    Prim(Prim),
    Not(Box<Node>),
    And(Box<Node>, Box<Node>),
    Or(Box<Node>, Box<Node>),
    True,
}

/// A compiled capture filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    text: String,
    root: Node,
}

impl Filter {
    /// A filter that matches everything (the empty filter).
    #[must_use]
    pub fn all() -> Self {
        Self {
            text: String::new(),
            root: Node::True,
        }
    }

    /// Parses `text`; empty / blank text matches everything.
    ///
    /// # Errors
    /// [`FilterError`] for syntax errors, unknown keywords and invalid values.
    pub fn parse(text: &str) -> Result<Self, FilterError> {
        let toks = lex(text)?;
        if toks.is_empty() {
            return Ok(Self {
                text: String::new(),
                root: Node::True,
            });
        }
        let mut p = Parser {
            toks,
            i: 0,
            end: text.len(),
        };
        let root = p.expr()?;
        if let Some(t) = p.peek() {
            return Err(FilterError {
                pos: t.pos,
                message: format!("unexpected `{}`", t.text),
            });
        }
        Ok(Self {
            text: text.trim().to_owned(),
            root,
        })
    }

    /// The filter text (trimmed; empty for [`Filter::all`]).
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Whether the filter matches every frame.
    #[must_use]
    pub fn is_all(&self) -> bool {
        self.root == Node::True
    }

    /// Whether the Ethernet frame `frame` matches.
    #[must_use]
    pub fn matches(&self, frame: &[u8]) -> bool {
        if self.is_all() {
            return true;
        }
        let d = Decoded::new(frame);
        eval(&self.root, &d)
    }
}

impl std::str::FromStr for Filter {
    type Err = FilterError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

// --- lexer -----------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Tok {
    text: String,
    pos: usize,
}

fn lex(s: &str) -> Result<Vec<Tok>, FilterError> {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        let two = s.get(i..i + 2);
        if matches!(two, Some("&&" | "||" | "<=" | ">=" | "==" | "!=")) {
            i += 2;
        } else if matches!(c, b'(' | b')' | b'!' | b'<' | b'>' | b'=') {
            i += 1;
        } else if c.is_ascii_alphanumeric() || matches!(c, b'.' | b':' | b'/' | b'-' | b'_') {
            while i < b.len()
                && (b[i].is_ascii_alphanumeric()
                    || matches!(b[i], b'.' | b':' | b'/' | b'-' | b'_'))
            {
                i += 1;
            }
        } else {
            return Err(FilterError {
                pos: i,
                message: format!(
                    "unexpected character `{}`",
                    s[i..].chars().next().unwrap_or('?')
                ),
            });
        }
        out.push(Tok {
            text: s[start..i].to_owned(),
            pos: start,
        });
    }
    Ok(out)
}

// --- parser ----------------------------------------------------------------------

struct Parser {
    toks: Vec<Tok>,
    i: usize,
    end: usize,
}

fn err<T>(pos: usize, message: impl Into<String>) -> Result<T, FilterError> {
    Err(FilterError {
        pos,
        message: message.into(),
    })
}

fn parse_num(t: &Tok) -> Result<u64, FilterError> {
    let s = t.text.as_str();
    let r = if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(h, 16)
    } else {
        s.parse()
    };
    r.or_else(|_| err(t.pos, format!("expected a number, got `{s}`")))
}

fn num_in<T: TryFrom<u64>>(t: &Tok, what: &str) -> Result<T, FilterError> {
    let n = parse_num(t)?;
    T::try_from(n).or_else(|_| err(t.pos, format!("{what} {n} is out of range")))
}

fn parse_mac(t: &Tok) -> Result<[u8; 6], FilterError> {
    let parts: Vec<&str> = t.text.split([':', '-', '.']).collect();
    let mut mac = [0u8; 6];
    let ok = match parts.len() {
        6 => parts.iter().zip(mac.iter_mut()).all(|(p, m)| {
            u8::from_str_radix(p, 16)
                .map(|v| *m = v)
                .is_ok_and(|()| p.len() <= 2)
        }),
        // Cisco style 0011.2233.4455
        3 => {
            let joined: String = parts.concat();
            joined.len() == 12
                && (0..6).all(|i| {
                    u8::from_str_radix(&joined[i * 2..i * 2 + 2], 16)
                        .map(|v| mac[i] = v)
                        .is_ok()
                })
        }
        _ => false,
    };
    if ok {
        Ok(mac)
    } else {
        err(t.pos, format!("invalid MAC address `{}`", t.text))
    }
}

fn ip_proto(t: &Tok) -> Result<u8, FilterError> {
    Ok(match t.text.to_ascii_lowercase().as_str() {
        "icmp" => 1,
        "igmp" => 2,
        "tcp" => 6,
        "udp" => 17,
        "gre" => 47,
        "esp" => 50,
        "ah" => 51,
        "icmp6" => 58,
        "ospf" => 89,
        "pim" => 103,
        "vrrp" => 112,
        "sctp" => 132,
        _ => num_in(t, "protocol")?,
    })
}

fn ether_proto(t: &Tok) -> Result<u16, FilterError> {
    Ok(match t.text.to_ascii_lowercase().as_str() {
        "ip" => 0x0800,
        "ip6" => 0x86dd,
        "arp" => 0x0806,
        "rarp" => 0x8035,
        "vlan" => 0x8100,
        "mpls" => 0x8847,
        "lldp" => 0x88cc,
        "pppoe" => 0x8864,
        _ => num_in(t, "EtherType")?,
    })
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i)
    }

    fn peek_kw(&self) -> Option<String> {
        self.peek().map(|t| t.text.to_ascii_lowercase())
    }

    fn next(&mut self, what: &str) -> Result<Tok, FilterError> {
        match self.toks.get(self.i) {
            Some(t) => {
                self.i += 1;
                Ok(t.clone())
            }
            None => err(
                self.end,
                format!("expected {what} at the end of the filter"),
            ),
        }
    }

    fn eat(&mut self, kw: &str) -> bool {
        if self.peek_kw().as_deref() == Some(kw) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn expr(&mut self) -> Result<Node, FilterError> {
        let mut l = self.and()?;
        while self.eat("or") || self.eat("||") {
            let r = self.and()?;
            l = Node::Or(Box::new(l), Box::new(r));
        }
        Ok(l)
    }

    fn and(&mut self) -> Result<Node, FilterError> {
        let mut l = self.unary()?;
        while self.eat("and") || self.eat("&&") {
            let r = self.unary()?;
            l = Node::And(Box::new(l), Box::new(r));
        }
        Ok(l)
    }

    fn unary(&mut self) -> Result<Node, FilterError> {
        if self.eat("not") || self.eat("!") {
            return Ok(Node::Not(Box::new(self.unary()?)));
        }
        if self.eat("(") {
            let e = self.expr()?;
            let t = self.next("`)`")?;
            if t.text != ")" {
                return err(t.pos, format!("expected `)`, got `{}`", t.text));
            }
            return Ok(e);
        }
        self.primitive().map(Node::Prim)
    }

    fn dir(&mut self) -> Dir {
        if self.eat("src") {
            Dir::Src
        } else if self.eat("dst") {
            Dir::Dst
        } else {
            Dir::Any
        }
    }

    fn addr(t: &Tok) -> Result<IpAddr, FilterError> {
        t.text
            .parse::<IpAddr>()
            .or_else(|_| err(t.pos, format!("invalid IP address `{}`", t.text)))
    }

    fn net(&mut self, dir: Dir) -> Result<Prim, FilterError> {
        let t = self.next("a network")?;
        let (a, len) = match t.text.split_once('/') {
            Some((a, l)) => {
                let addr = Self::addr(&Tok {
                    text: a.to_owned(),
                    pos: t.pos,
                })?;
                let len: u8 = l
                    .parse()
                    .or_else(|_| err(t.pos, format!("invalid prefix length `{l}`")))?;
                (addr, len)
            }
            None => {
                let addr = Self::addr(&t)?;
                if self.eat("mask") {
                    let m = self.next("a netmask")?;
                    let IpAddr::V4(mask) = Self::addr(&m)? else {
                        return err(m.pos, "`mask` needs an IPv4 netmask");
                    };
                    let bits = u32::from(mask);
                    if bits.leading_ones() + bits.trailing_zeros() != 32 {
                        return err(m.pos, format!("non-contiguous netmask `{}`", m.text));
                    }
                    #[allow(clippy::cast_possible_truncation)] // ≤ 32
                    let len = bits.leading_ones() as u8;
                    (addr, len)
                } else {
                    (addr, if addr.is_ipv4() { 32 } else { 128 })
                }
            }
        };
        let max = if a.is_ipv4() { 32 } else { 128 };
        if len > max {
            return err(t.pos, format!("prefix length {len} is longer than {max}"));
        }
        Ok(Prim::Net(dir, a, len))
    }

    fn port(&mut self, transport: Option<u8>, dir: Dir, range: bool) -> Result<Prim, FilterError> {
        let t = self.next("a port")?;
        if range {
            let Some((a, b)) = t.text.split_once('-') else {
                return err(t.pos, format!("expected `<low>-<high>`, got `{}`", t.text));
            };
            let tok = |s: &str| Tok {
                text: s.to_owned(),
                pos: t.pos,
            };
            let lo: u16 = num_in(&tok(a), "port")?;
            let hi: u16 = num_in(&tok(b), "port")?;
            if lo > hi {
                return err(t.pos, format!("empty port range `{}`", t.text));
            }
            Ok(Prim::Port(transport, dir, lo, hi))
        } else {
            let p: u16 = num_in(&t, "port")?;
            Ok(Prim::Port(transport, dir, p, p))
        }
    }

    /// `[src|dst] (host|net|port|portrange) ...` after an optional direction.
    fn qualified(&mut self, dir: Dir, transport: Option<u8>) -> Result<Option<Prim>, FilterError> {
        let Some(kw) = self.peek_kw() else {
            return Ok(None);
        };
        Ok(Some(match kw.as_str() {
            "port" | "portrange" => {
                self.i += 1;
                self.port(transport, dir, kw == "portrange")?
            }
            "host" if transport.is_none() => {
                self.i += 1;
                let t = self.next("an address")?;
                Prim::Host(dir, Self::addr(&t)?)
            }
            "net" if transport.is_none() => {
                self.i += 1;
                self.net(dir)?
            }
            _ => return Ok(None),
        }))
    }

    #[allow(clippy::too_many_lines)]
    fn primitive(&mut self) -> Result<Prim, FilterError> {
        let t = self.next("a filter primitive")?;
        let kw = t.text.to_ascii_lowercase();
        Ok(match kw.as_str() {
            "ether" => {
                let k = self.next("`host`, `src`, `dst`, `proto`, `broadcast` or `multicast`")?;
                match k.text.to_ascii_lowercase().as_str() {
                    "proto" => Prim::EtherProto(ether_proto(&self.next("an EtherType")?)?),
                    "broadcast" => Prim::EtherBroadcast,
                    "multicast" => Prim::EtherMulticast,
                    d @ ("host" | "src" | "dst") => {
                        let dir = match d {
                            "src" => Dir::Src,
                            "dst" => Dir::Dst,
                            _ => Dir::Any,
                        };
                        if dir != Dir::Any {
                            self.eat("host");
                        }
                        Prim::EtherHost(dir, parse_mac(&self.next("a MAC address")?)?)
                    }
                    other => return err(k.pos, format!("unknown `ether` qualifier `{other}`")),
                }
            }
            "broadcast" => Prim::EtherBroadcast,
            "multicast" => Prim::EtherMulticast,
            "vlan" => match self.peek() {
                Some(n) if n.text.as_bytes()[0].is_ascii_digit() => {
                    let n = self.next("a VLAN id")?;
                    let id: u16 = num_in(&n, "VLAN id")?;
                    if id > 4095 {
                        return err(n.pos, format!("VLAN id {id} is out of range"));
                    }
                    Prim::Vlan(Some(id))
                }
                _ => Prim::Vlan(None),
            },
            "mpls" => Prim::Mpls,
            "ip" | "ip6" => {
                let v6 = kw == "ip6";
                if self.eat("proto") {
                    Prim::Proto(Some(v6), ip_proto(&self.next("a protocol")?)?)
                } else if v6 {
                    Prim::Ip6
                } else {
                    Prim::Ip
                }
            }
            "proto" => Prim::Proto(None, ip_proto(&self.next("a protocol")?)?),
            "arp" => Prim::Arp,
            "rarp" => Prim::Rarp,
            "tcp" | "udp" | "sctp" => {
                let p = match kw.as_str() {
                    "tcp" => 6,
                    "udp" => 17,
                    _ => 132,
                };
                let save = self.i;
                let dir = self.dir();
                match self.qualified(dir, Some(p))? {
                    Some(Prim::Port(_, d, lo, hi)) => Prim::Port(Some(p), d, lo, hi),
                    Some(other) => other,
                    None => {
                        self.i = save;
                        Prim::Proto(None, p)
                    }
                }
            }
            "icmp" => Prim::Proto(Some(false), 1),
            "icmp6" => Prim::Proto(Some(true), 58),
            "src" | "dst" | "host" | "net" | "port" | "portrange" => {
                let dir = match kw.as_str() {
                    "src" => Dir::Src,
                    "dst" => Dir::Dst,
                    _ => {
                        self.i -= 1;
                        Dir::Any
                    }
                };
                match self.qualified(dir, None)? {
                    Some(p) => p,
                    // `src 10.0.0.1` = `src host 10.0.0.1`
                    None => Prim::Host(dir, Self::addr(&self.next("an address")?)?),
                }
            }
            "less" => Prim::Len(Cmp::Le, num_in(&self.next("a length")?, "length")?),
            "greater" => Prim::Len(Cmp::Ge, num_in(&self.next("a length")?, "length")?),
            "len" => {
                let op = self.next("a comparison")?;
                let cmp = match op.text.as_str() {
                    "<" => Cmp::Lt,
                    "<=" => Cmp::Le,
                    ">" => Cmp::Gt,
                    ">=" => Cmp::Ge,
                    "=" | "==" => Cmp::Eq,
                    "!=" => Cmp::Ne,
                    other => return err(op.pos, format!("expected a comparison, got `{other}`")),
                };
                Prim::Len(cmp, num_in(&self.next("a length")?, "length")?)
            }
            "wpg" | "signature" => Prim::Signature,
            _ => return err(t.pos, format!("unknown filter primitive `{}`", t.text)),
        })
    }
}

// --- evaluation ------------------------------------------------------------------

#[derive(Debug, Default)]
struct Decoded<'a> {
    frame: &'a [u8],
    dst: [u8; 6],
    src: [u8; 6],
    /// `EtherType`s seen (outer first), including the tag types.
    ethertypes: Vec<u16>,
    vlans: Vec<u16>,
    /// Innermost EtherType.
    ethertype: u16,
    src_ip: Option<IpAddr>,
    dst_ip: Option<IpAddr>,
    /// (is IPv6, protocol) of the transport.
    proto: Option<(bool, u8)>,
    ports: Option<(u16, u16)>,
}

fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*b.get(o)?, *b.get(o + 1)?]))
}

fn v4_at(b: &[u8], o: usize) -> Option<IpAddr> {
    let s: [u8; 4] = b.get(o..o + 4)?.try_into().ok()?;
    Some(IpAddr::V4(Ipv4Addr::from(s)))
}

fn v6_at(b: &[u8], o: usize) -> Option<IpAddr> {
    let s: [u8; 16] = b.get(o..o + 16)?.try_into().ok()?;
    Some(IpAddr::V6(Ipv6Addr::from(s)))
}

impl<'a> Decoded<'a> {
    fn new(frame: &'a [u8]) -> Self {
        let mut d = Decoded {
            frame,
            ..Decoded::default()
        };
        if frame.len() < 14 {
            return d;
        }
        d.dst.copy_from_slice(&frame[..6]);
        d.src.copy_from_slice(&frame[6..12]);
        let mut off = 12;
        let Some(mut et) = u16_at(frame, off) else {
            return d;
        };
        off += 2;
        d.ethertypes.push(et);
        while matches!(et, 0x8100 | 0x88a8 | 0x9100) {
            let (Some(tci), Some(next)) = (u16_at(frame, off), u16_at(frame, off + 2)) else {
                break;
            };
            d.vlans.push(tci & 0x0fff);
            et = next;
            d.ethertypes.push(et);
            off += 4;
        }
        d.ethertype = et;
        let l4 = match et {
            0x0800 => {
                let Some(&vihl) = frame.get(off) else {
                    return d;
                };
                d.src_ip = v4_at(frame, off + 12);
                d.dst_ip = v4_at(frame, off + 16);
                let proto = frame.get(off + 9).copied();
                let frag = u16_at(frame, off + 6).unwrap_or(0) & 0x1fff;
                d.proto = proto.map(|p| (false, p));
                (frag == 0).then_some(off + usize::from(vihl & 0x0f) * 4)
            }
            0x86dd => {
                d.src_ip = v6_at(frame, off + 8);
                d.dst_ip = v6_at(frame, off + 24);
                let mut nh = frame.get(off + 6).copied();
                let mut p = off + 40;
                let mut first = true;
                while let Some(h @ (0 | 43 | 44 | 60)) = nh {
                    if h == 44 {
                        first = u16_at(frame, p + 2).is_some_and(|f| f & 0xfff8 == 0);
                        nh = frame.get(p).copied();
                        p += 8;
                        continue;
                    }
                    nh = frame.get(p).copied();
                    p += frame.get(p + 1).map_or(0, |l| (usize::from(*l) + 1) * 8);
                }
                d.proto = nh.map(|n| (true, n));
                first.then_some(p)
            }
            0x0806 | 0x8035 => {
                // ARP for IPv4 over Ethernet: sender / target protocol addresses.
                if u16_at(frame, off + 2) == Some(0x0800) && frame.get(off + 5) == Some(&4) {
                    d.src_ip = v4_at(frame, off + 14);
                    d.dst_ip = v4_at(frame, off + 24);
                }
                None
            }
            _ => None,
        };
        if let (Some(l4), Some((_, 6 | 17 | 132))) = (l4, d.proto) {
            if let (Some(s), Some(t)) = (u16_at(frame, l4), u16_at(frame, l4 + 2)) {
                d.ports = Some((s, t));
            }
        }
        d
    }
}

fn dir_match<T>(dir: Dir, src: Option<&T>, dst: Option<&T>, f: impl Fn(&T) -> bool) -> bool {
    let s = src.is_some_and(&f);
    let d = dst.is_some_and(&f);
    match dir {
        Dir::Any => s || d,
        Dir::Src => s,
        Dir::Dst => d,
    }
}

fn in_net(addr: &IpAddr, net: &IpAddr, len: u8) -> bool {
    match (addr, net) {
        (IpAddr::V4(a), IpAddr::V4(n)) => {
            let mask = u32::MAX.checked_shl(32 - u32::from(len)).unwrap_or(0);
            u32::from(*a) & mask == u32::from(*n) & mask
        }
        (IpAddr::V6(a), IpAddr::V6(n)) => {
            let mask = u128::MAX.checked_shl(128 - u32::from(len)).unwrap_or(0);
            u128::from(*a) & mask == u128::from(*n) & mask
        }
        _ => false,
    }
}

fn eval_prim(p: &Prim, d: &Decoded<'_>) -> bool {
    match p {
        Prim::EtherHost(dir, mac) => match dir {
            Dir::Any => d.src == *mac || d.dst == *mac,
            Dir::Src => d.src == *mac,
            Dir::Dst => d.dst == *mac,
        },
        Prim::EtherProto(t) => d.ethertypes.contains(t),
        Prim::EtherBroadcast => d.frame.len() >= 6 && d.dst == [0xff; 6],
        Prim::EtherMulticast => d.frame.len() >= 6 && d.dst[0] & 1 == 1,
        Prim::Vlan(id) => match id {
            None => !d.vlans.is_empty(),
            Some(id) => d.vlans.contains(id),
        },
        Prim::Mpls => matches!(d.ethertype, 0x8847 | 0x8848),
        Prim::Ip => d.ethertype == 0x0800,
        Prim::Ip6 => d.ethertype == 0x86dd,
        Prim::Arp => d.ethertype == 0x0806,
        Prim::Rarp => d.ethertype == 0x8035,
        Prim::Proto(v6, proto) => d
            .proto
            .is_some_and(|(is6, p)| p == *proto && v6.is_none_or(|v| v == is6)),
        Prim::Host(dir, a) => dir_match(*dir, d.src_ip.as_ref(), d.dst_ip.as_ref(), |x| x == a),
        Prim::Net(dir, n, len) => dir_match(*dir, d.src_ip.as_ref(), d.dst_ip.as_ref(), |x| {
            in_net(x, n, *len)
        }),
        Prim::Port(tr, dir, lo, hi) => {
            let Some((sp, dp)) = d.ports else {
                return false;
            };
            if tr.is_some_and(|t| d.proto.is_none_or(|(_, p)| p != t)) {
                return false;
            }
            dir_match(*dir, Some(&sp), Some(&dp), |p| (*lo..=*hi).contains(p))
        }
        Prim::Len(cmp, n) => {
            let l = d.frame.len();
            match cmp {
                Cmp::Lt => l < *n,
                Cmp::Le => l <= *n,
                Cmp::Gt => l > *n,
                Cmp::Ge => l >= *n,
                Cmp::Eq => l == *n,
                Cmp::Ne => l != *n,
            }
        }
        Prim::Signature => wpg_core::signature::find(d.frame).is_some(),
    }
}

fn eval(n: &Node, d: &Decoded<'_>) -> bool {
    match n {
        Node::True => true,
        Node::Prim(p) => eval_prim(p, d),
        Node::Not(x) => !eval(x, d),
        Node::And(a, b) => eval(a, d) && eval(b, d),
        Node::Or(a, b) => eval(a, d) || eval(b, d),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eth(dst: [u8; 6], ethertype: u16) -> Vec<u8> {
        let mut f = dst.to_vec();
        f.extend_from_slice(&[0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        f.extend_from_slice(&ethertype.to_be_bytes());
        f
    }

    fn udp4(src: [u8; 4], dst: [u8; 4], sport: u16, dport: u16) -> Vec<u8> {
        let mut f = eth([0xff; 6], 0x0800);
        f.extend_from_slice(&[0x45, 0, 0, 36, 0, 0, 0, 0, 64, 17, 0, 0]);
        f.extend_from_slice(&src);
        f.extend_from_slice(&dst);
        f.extend_from_slice(&sport.to_be_bytes());
        f.extend_from_slice(&dport.to_be_bytes());
        f.extend_from_slice(&[0, 16, 0, 0]);
        f.resize(60, 0);
        f
    }

    fn vlan(inner: &[u8], id: u16) -> Vec<u8> {
        let mut f = inner[..12].to_vec();
        f.extend_from_slice(&[0x81, 0x00]);
        f.extend_from_slice(&id.to_be_bytes());
        f.extend_from_slice(&inner[12..]);
        f
    }

    fn tcp6(sport: u16, dport: u16) -> Vec<u8> {
        let mut f = eth([0x33, 0x33, 0, 0, 0, 1], 0x86dd);
        f.extend_from_slice(&[0x60, 0, 0, 0, 0, 20, 6, 64]);
        f.extend_from_slice(&"2001:db8::1".parse::<Ipv6Addr>().unwrap().octets());
        f.extend_from_slice(&"2001:db8::2".parse::<Ipv6Addr>().unwrap().octets());
        f.extend_from_slice(&sport.to_be_bytes());
        f.extend_from_slice(&dport.to_be_bytes());
        f.resize(f.len() + 16, 0);
        f
    }

    fn m(filter: &str, frame: &[u8]) -> bool {
        Filter::parse(filter)
            .unwrap_or_else(|e| panic!("{filter}: {e}"))
            .matches(frame)
    }

    #[test]
    fn basics() {
        let u = udp4([10, 0, 0, 1], [192, 168, 1, 2], 12345, 53);
        assert!(m("", &u));
        assert!(Filter::parse("  ").unwrap().is_all());
        assert!(m("udp", &u) && m("ip", &u) && !m("tcp", &u) && !m("ip6", &u));
        assert!(m("udp port 53", &u) && m("port 12345", &u) && !m("tcp port 53", &u));
        assert!(m("dst port 53", &u) && !m("src port 53", &u));
        assert!(m("udp dst portrange 50-60", &u) && !m("portrange 1-52", &u));
        assert!(m("host 10.0.0.1", &u) && m("src 10.0.0.1", &u) && !m("dst host 10.0.0.1", &u));
        assert!(m("net 192.168.0.0/16", &u) && m("dst net 192.168.1.0 mask 255.255.255.0", &u));
        assert!(!m("src net 192.168.0.0/16", &u));
        assert!(m("ether broadcast and ip proto udp", &u));
        assert!(m("ether src 00:11:22:33:44:55", &u) && m("ether host 0011.2233.4455", &u));
        assert!(!m("ether dst 00:11:22:33:44:55", &u));
        assert!(m("ether proto 0x0800 && proto 17", &u));
        assert!(m("not tcp and (udp or arp)", &u) && m("! (tcp || icmp)", &u));
        assert!(
            m("len == 60", &u) && m("less 60", &u) && m("greater 60", &u) && !m("len > 60", &u)
        );
        assert!(!m("wpg", &u));
        assert_eq!(Filter::parse(" udp ").unwrap().text(), "udp");
    }

    #[test]
    fn vlan_ipv6_arp() {
        let u = vlan(&udp4([10, 0, 0, 1], [10, 0, 0, 2], 1, 2), 100);
        assert!(m("vlan", &u) && m("vlan 100", &u) && !m("vlan 7", &u));
        assert!(m("vlan 100 and udp port 2", &u) && m("ether proto vlan", &u));
        let t = tcp6(443, 50000);
        assert!(m("ip6 and tcp src port 443", &t) && m("host 2001:db8::2", &t));
        assert!(m("net 2001:db8::/32", &t) && m("multicast", &t) && !m("broadcast", &t));
        assert!(m("ip6 proto tcp", &t) && !m("ip proto tcp", &t) && !m("vlan", &t));
        let mut arp = eth([0xff; 6], 0x0806);
        arp.extend_from_slice(&[0, 1, 8, 0, 6, 4, 0, 1]);
        arp.extend_from_slice(&[0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 10, 0, 0, 9]);
        arp.extend_from_slice(&[0; 6]);
        arp.extend_from_slice(&[10, 0, 0, 1]);
        assert!(m("arp and host 10.0.0.9", &arp) && m("arp and dst 10.0.0.1", &arp));
        assert!(!m("port 1", &arp) && !m("ip", &arp));
    }

    #[test]
    fn signature_primitive() {
        let mut f = udp4([1, 1, 1, 1], [2, 2, 2, 2], 1, 2);
        f.resize(80, 0);
        let sig = wpg_core::signature::Signature {
            stream_id: 1,
            seq: 2,
            ts_ns: 3,
        };
        wpg_core::signature::stamp(&mut f, &sig).unwrap();
        assert!(m("wpg", &f) && m("udp and signature", &f));
    }

    #[test]
    fn errors() {
        for (f, pos) in [
            ("udp port", 8),
            ("bogus", 0),
            ("udp port 70000", 9),
            ("host 1.2.3", 5),
            ("(udp", 4),
            ("udp)", 3),
            ("net 10.0.0.0/33", 4),
            ("ether host zz:00:00:00:00:00", 11),
            ("vlan 5000", 5),
            ("len ~ 3", 4),
            ("portrange 9-1", 10),
            ("udp and", 7),
            ("net 10.0.0.0 mask 255.0.255.0", 18),
        ] {
            let e = Filter::parse(f).expect_err(f);
            assert_eq!(e.pos, pos, "{f}: {e}");
        }
    }
}
