//! DHCPv4 server logic (RFC 2131 §4.3): a request, the lease table and the
//! time in, the reply and where to send it out. No sockets here.

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr};

use crate::config::V4Config;
use crate::lease::{Lease, LeaseKind, LeaseState, Leases, OFFER_HOLD_SECS};
use crate::v4::{ip_list, opt, Message, MessageType, BOOTREPLY, BOOTREQUEST};
use crate::{fmt_mac, hex};

/// Where a reply goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dest {
    /// `255.255.255.255:68` on the interface.
    Broadcast,
    /// `ciaddr:68` (renewing / informing clients).
    Unicast(Ipv4Addr),
    /// `giaddr:67` (relay agent).
    Relay(Ipv4Addr),
}

/// A reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// Message.
    pub msg: Message,
    /// Destination.
    pub dest: Dest,
}

/// What happened to one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// `DISCOVER`, ... (`?` when not DHCP).
    pub request: &'static str,
    /// Client MAC.
    pub mac: String,
    /// The reply, if any.
    pub reply: Option<Reply>,
    /// Address offered / acknowledged / released.
    pub addr: Option<Ipv4Addr>,
    /// Why nothing (or a NAK) was sent.
    pub note: Option<String>,
    /// The lease table changed.
    pub changed: bool,
}

impl Outcome {
    fn new(request: &'static str, mac: String) -> Self {
        Self {
            request,
            mac,
            reply: None,
            addr: None,
            note: None,
            changed: false,
        }
    }

    fn note(mut self, n: impl Into<String>) -> Self {
        self.note = Some(n.into());
        self
    }
}

struct Ctx<'a> {
    cfg: &'a V4Config,
    server_ip: Ipv4Addr,
    now: u64,
    client: String,
    mac: [u8; 6],
}

impl Ctx<'_> {
    /// Reserved for a different MAC address.
    fn reserved_for_other(&self, ip: Ipv4Addr) -> bool {
        self.cfg
            .reservations
            .iter()
            .any(|r| r.ip == ip && crate::parse_mac(&r.mac) != Some(self.mac))
    }

    /// `ip` may be given to this client.
    fn usable(&self, leases: &Leases, ip: Ipv4Addr) -> bool {
        if self.cfg.reservation(self.mac) == Some(ip) {
            return true;
        }
        self.cfg.in_range(ip)
            && ip != self.server_ip
            && !self.reserved_for_other(ip)
            && leases
                .holder(LeaseKind::V4, IpAddr::V4(ip), self.now)
                .is_none_or(|l| l.client == self.client)
    }

    fn choose(&self, leases: &Leases, requested: Option<Ipv4Addr>) -> Option<Ipv4Addr> {
        if let Some(ip) = self.cfg.reservation(self.mac) {
            return Some(ip);
        }
        if let Some(IpAddr::V4(ip)) = leases
            .of_client(LeaseKind::V4, &self.client, 0)
            .map(|l| l.addr)
        {
            if self.usable(leases, ip) {
                return Some(ip);
            }
        }
        if let Some(ip) = requested.filter(|ip| self.usable(leases, *ip)) {
            return Some(ip);
        }
        let used: HashSet<IpAddr> = leases
            .list
            .iter()
            .filter(|l| l.kind == LeaseKind::V4 && l.active(self.now))
            .map(|l| l.addr)
            .collect();
        (u32::from(self.cfg.range_start)..=u32::from(self.cfg.range_end))
            .map(Ipv4Addr::from)
            .find(|ip| {
                !used.contains(&IpAddr::V4(*ip))
                    && *ip != self.server_ip
                    && !self.reserved_for_other(*ip)
            })
    }

    fn lease(&self, req: &Message, ip: Ipv4Addr, state: LeaseState, secs: u64) -> Lease {
        Lease {
            kind: LeaseKind::V4,
            addr: IpAddr::V4(ip),
            plen: 32,
            client: self.client.clone(),
            iaid: 0,
            mac: Some(fmt_mac(&self.mac)),
            hostname: req.hostname(),
            state,
            expires: self.now + secs,
            updated: self.now,
        }
    }

    fn reply(&self, req: &Message, t: MessageType, yiaddr: Ipv4Addr, with_lease: bool) -> Reply {
        let cfg = self.cfg;
        let mut m = Message {
            op: BOOTREPLY,
            htype: req.htype,
            hlen: req.hlen,
            xid: req.xid,
            flags: req.flags,
            giaddr: req.giaddr,
            chaddr: req.chaddr,
            yiaddr,
            ciaddr: if matches!(t, MessageType::Ack) {
                req.ciaddr
            } else {
                Ipv4Addr::UNSPECIFIED
            },
            ..Message::default()
        };
        m.set_option(opt::MESSAGE_TYPE, vec![t.as_u8()]);
        m.set_option(opt::SERVER_ID, self.server_ip.octets().to_vec());
        if t == MessageType::Nak {
            m.set_option(opt::MESSAGE, b"address not available".to_vec());
        } else {
            if with_lease {
                let l = cfg.lease_secs;
                m.set_option(opt::LEASE_TIME, l.to_be_bytes().to_vec());
                m.set_option(opt::T1, (l / 2).to_be_bytes().to_vec());
                m.set_option(
                    opt::T2,
                    (u32::try_from(u64::from(l) * 7 / 8).unwrap_or(l))
                        .to_be_bytes()
                        .to_vec(),
                );
            }
            m.set_option(opt::SUBNET_MASK, cfg.subnet_mask.octets().to_vec());
            for (code, ips) in [
                (opt::ROUTER, &cfg.routers),
                (opt::DNS, &cfg.dns),
                (opt::NTP, &cfg.ntp),
            ] {
                if !ips.is_empty() {
                    m.set_option(code, ip_list(ips));
                }
            }
            if !cfg.domain.trim().is_empty() {
                m.set_option(opt::DOMAIN, cfg.domain.trim().as_bytes().to_vec());
            }
        }
        if let Some(r) = req.option(opt::RELAY_AGENT) {
            m.set_option(opt::RELAY_AGENT, r.to_vec());
        }
        let dest = if !req.giaddr.is_unspecified() {
            Dest::Relay(req.giaddr)
        } else if t == MessageType::Nak || req.ciaddr.is_unspecified() {
            Dest::Broadcast
        } else {
            Dest::Unicast(req.ciaddr)
        };
        Reply { msg: m, dest }
    }
}

/// Handles one datagram from a client.
#[must_use]
pub fn handle(
    cfg: &V4Config,
    server_ip: Ipv4Addr,
    leases: &mut Leases,
    now: u64,
    req: &Message,
) -> Outcome {
    let mac = req.mac();
    let mut out = Outcome::new("?", fmt_mac(&mac));
    if req.op != BOOTREQUEST || req.htype != 1 || req.hlen != 6 {
        return out.note("not an Ethernet BOOTREQUEST");
    }
    let Some(t) = req.message_type() else {
        return out.note("no DHCP message type");
    };
    out.request = t.name();
    let ctx = Ctx {
        cfg,
        server_ip,
        now,
        client: req
            .option(opt::CLIENT_ID)
            .map_or_else(|| fmt_mac(&mac), hex),
        mac,
    };
    let requested = req.ip_option(opt::REQUESTED_IP);
    match t {
        MessageType::Discover => {
            let Some(ip) = ctx.choose(leases, requested) else {
                return out.note("pool exhausted");
            };
            // A bound client that discovers again keeps its lease.
            let bound = leases
                .of_client(LeaseKind::V4, &ctx.client, 0)
                .is_some_and(|l| {
                    l.addr == IpAddr::V4(ip) && l.state == LeaseState::Bound && l.active(now)
                });
            if !bound {
                leases.upsert(ctx.lease(req, ip, LeaseState::Offered, OFFER_HOLD_SECS));
                out.changed = true;
            }
            out.addr = Some(ip);
            out.reply = Some(ctx.reply(req, MessageType::Offer, ip, true));
        }
        MessageType::Request => {
            let server_id = req.ip_option(opt::SERVER_ID);
            if server_id.is_some_and(|s| s != server_ip) {
                if let Some(IpAddr::V4(ip)) = leases
                    .of_client(LeaseKind::V4, &ctx.client, 0)
                    .filter(|l| l.state == LeaseState::Offered)
                    .map(|l| l.addr)
                {
                    out.changed = leases.release(LeaseKind::V4, &ctx.client, IpAddr::V4(ip));
                }
                return out.note("the client selected another server");
            }
            let Some(ip) = requested.or((!req.ciaddr.is_unspecified()).then_some(req.ciaddr))
            else {
                return out.note("no requested address");
            };
            out.addr = Some(ip);
            if ctx.usable(leases, ip) {
                leases.upsert(ctx.lease(req, ip, LeaseState::Bound, u64::from(cfg.lease_secs)));
                out.changed = true;
                out.reply = Some(ctx.reply(req, MessageType::Ack, ip, true));
            } else {
                out.reply = Some(ctx.reply(req, MessageType::Nak, Ipv4Addr::UNSPECIFIED, false));
                out = out.note("address not available to this client");
            }
        }
        MessageType::Decline => {
            if let Some(ip) = requested {
                out.addr = Some(ip);
                leases.release(LeaseKind::V4, &ctx.client, IpAddr::V4(ip));
                let mut l = ctx.lease(req, ip, LeaseState::Declined, u64::from(cfg.lease_secs));
                l.client = format!("declined by {}", l.client);
                leases.upsert(l);
                out.changed = true;
            }
        }
        MessageType::Release => {
            out.addr = Some(req.ciaddr);
            out.changed = leases.release(LeaseKind::V4, &ctx.client, IpAddr::V4(req.ciaddr));
        }
        MessageType::Inform => {
            out.addr = Some(req.ciaddr);
            out.reply = Some(ctx.reply(req, MessageType::Ack, Ipv4Addr::UNSPECIFIED, false));
        }
        MessageType::Offer | MessageType::Ack | MessageType::Nak => {
            return out.note("server message ignored");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Reservation;
    use crate::v4::FLAG_BROADCAST;

    const SERVER: Ipv4Addr = Ipv4Addr::new(192, 168, 100, 1);

    fn cfg() -> V4Config {
        V4Config {
            range_start: "192.168.100.10".parse().unwrap(),
            range_end: "192.168.100.12".parse().unwrap(),
            routers: vec![SERVER],
            dns: vec!["8.8.8.8".parse().unwrap()],
            domain: "lab".into(),
            lease_secs: 600,
            ..V4Config::default()
        }
    }

    fn msg(t: MessageType, mac: u8) -> Message {
        let mut m = Message {
            xid: 42,
            flags: FLAG_BROADCAST,
            ..Message::default()
        };
        m.chaddr[..6].copy_from_slice(&[2, 0, 0, 0, 0, mac]);
        m.set_option(opt::MESSAGE_TYPE, vec![t.as_u8()]);
        m
    }

    fn dora(c: &V4Config, leases: &mut Leases, mac: u8, now: u64) -> Option<Ipv4Addr> {
        let o = handle(c, SERVER, leases, now, &msg(MessageType::Discover, mac));
        let offer = o.reply?.msg;
        assert_eq!(offer.message_type(), Some(MessageType::Offer));
        let mut req = msg(MessageType::Request, mac);
        req.set_option(opt::REQUESTED_IP, offer.yiaddr.octets().to_vec());
        req.set_option(opt::SERVER_ID, SERVER.octets().to_vec());
        let ack = handle(c, SERVER, leases, now, &req).reply.unwrap();
        assert_eq!(ack.msg.message_type(), Some(MessageType::Ack));
        assert_eq!(ack.dest, Dest::Broadcast);
        Some(ack.msg.yiaddr)
    }

    #[test]
    fn discover_offer_request_ack() {
        let c = cfg();
        let mut leases = Leases::default();
        let o = handle(&c, SERVER, &mut leases, 0, &msg(MessageType::Discover, 1));
        let r = o.reply.unwrap();
        assert_eq!(r.dest, Dest::Broadcast);
        assert_eq!(r.msg.op, BOOTREPLY);
        assert_eq!(r.msg.xid, 42);
        assert_eq!(r.msg.yiaddr, Ipv4Addr::new(192, 168, 100, 10));
        assert_eq!(r.msg.ip_option(opt::SERVER_ID), Some(SERVER));
        assert_eq!(
            r.msg.option(opt::LEASE_TIME),
            Some(&600u32.to_be_bytes()[..])
        );
        assert_eq!(r.msg.option(opt::T1), Some(&300u32.to_be_bytes()[..]));
        assert_eq!(r.msg.option(opt::T2), Some(&525u32.to_be_bytes()[..]));
        assert_eq!(r.msg.option(opt::DOMAIN), Some(&b"lab"[..]));
        assert_eq!(leases.list[0].state, LeaseState::Offered);

        assert_eq!(
            dora(&c, &mut leases, 1, 1),
            Some("192.168.100.10".parse().unwrap())
        );
        assert_eq!(leases.list.len(), 1);
        assert_eq!(leases.list[0].state, LeaseState::Bound);
        assert_eq!(leases.list[0].expires, 601);
        assert_eq!(
            dora(&c, &mut leases, 2, 1),
            Some("192.168.100.11".parse().unwrap())
        );
        assert_eq!(
            dora(&c, &mut leases, 3, 1),
            Some("192.168.100.12".parse().unwrap())
        );
        // Pool exhausted.
        let o = handle(&c, SERVER, &mut leases, 1, &msg(MessageType::Discover, 4));
        assert!(o.reply.is_none());
        assert_eq!(o.note.as_deref(), Some("pool exhausted"));
        // Client 1 comes back and gets its address again; after expiry the
        // address is free for others.
        assert_eq!(
            dora(&c, &mut leases, 1, 2),
            Some("192.168.100.10".parse().unwrap())
        );
        assert_eq!(
            dora(&c, &mut leases, 4, 10_000),
            Some("192.168.100.10".parse().unwrap())
        );
    }

    #[test]
    fn renew_nak_release_inform_decline() {
        let c = cfg();
        let mut leases = Leases::default();
        let ip = dora(&c, &mut leases, 1, 0).unwrap();
        // Renew: unicast to ciaddr.
        let mut renew = msg(MessageType::Request, 1);
        renew.flags = 0;
        renew.ciaddr = ip;
        let r = handle(&c, SERVER, &mut leases, 100, &renew).reply.unwrap();
        assert_eq!(r.dest, Dest::Unicast(ip));
        assert_eq!(r.msg.ciaddr, ip);
        assert_eq!(leases.list[0].expires, 700);
        // Another client asking for that address is refused.
        let mut steal = msg(MessageType::Request, 2);
        steal.set_option(opt::REQUESTED_IP, ip.octets().to_vec());
        let r = handle(&c, SERVER, &mut leases, 100, &steal).reply.unwrap();
        assert_eq!(r.msg.message_type(), Some(MessageType::Nak));
        assert_eq!(r.dest, Dest::Broadcast);
        // Outside the pool: NAK.
        let mut out = msg(MessageType::Request, 2);
        out.set_option(opt::REQUESTED_IP, vec![10, 0, 0, 1]);
        let r = handle(&c, SERVER, &mut leases, 100, &out).reply.unwrap();
        assert_eq!(r.msg.message_type(), Some(MessageType::Nak));
        // INFORM: options only.
        let mut inform = msg(MessageType::Inform, 9);
        inform.ciaddr = "192.168.100.50".parse().unwrap();
        let r = handle(&c, SERVER, &mut leases, 100, &inform).reply.unwrap();
        assert_eq!(r.msg.message_type(), Some(MessageType::Ack));
        assert_eq!(r.msg.option(opt::LEASE_TIME), None);
        assert_eq!(r.msg.yiaddr, Ipv4Addr::UNSPECIFIED);
        assert_eq!(r.dest, Dest::Unicast(inform.ciaddr));
        // RELEASE frees it.
        let mut rel = msg(MessageType::Release, 1);
        rel.ciaddr = ip;
        assert!(handle(&c, SERVER, &mut leases, 100, &rel).changed);
        assert!(leases.list.is_empty());
        // DECLINE keeps the address out of the pool.
        let ip = dora(&c, &mut leases, 1, 200).unwrap();
        let mut dec = msg(MessageType::Decline, 1);
        dec.set_option(opt::REQUESTED_IP, ip.octets().to_vec());
        assert!(handle(&c, SERVER, &mut leases, 200, &dec).reply.is_none());
        assert_eq!(leases.list[0].state, LeaseState::Declined);
        assert_eq!(
            dora(&c, &mut leases, 1, 201),
            Some("192.168.100.11".parse().unwrap())
        );
    }

    #[test]
    fn reservations_relay_and_other_server() {
        let mut c = cfg();
        c.reservations.push(Reservation {
            mac: "02:00:00:00:00:07".into(),
            ip: "192.168.100.10".parse().unwrap(),
        });
        let mut leases = Leases::default();
        // .10 is reserved: client 1 gets .11, client 7 gets .10.
        assert_eq!(
            dora(&c, &mut leases, 1, 0),
            Some("192.168.100.11".parse().unwrap())
        );
        assert_eq!(
            dora(&c, &mut leases, 7, 0),
            Some("192.168.100.10".parse().unwrap())
        );
        // Relayed DISCOVER: reply to the relay, option 82 echoed.
        let mut d = msg(MessageType::Discover, 3);
        d.giaddr = "10.9.9.1".parse().unwrap();
        d.set_option(opt::RELAY_AGENT, vec![1, 2, 0, 5]);
        let r = handle(&c, SERVER, &mut leases, 0, &d).reply.unwrap();
        assert_eq!(r.dest, Dest::Relay(d.giaddr));
        assert_eq!(r.msg.giaddr, d.giaddr);
        assert_eq!(r.msg.option(opt::RELAY_AGENT), Some(&[1, 2, 0, 5][..]));
        // The client picks another server: our offer is withdrawn.
        assert!(leases
            .of_client(LeaseKind::V4, "02:00:00:00:00:03", 0)
            .is_some());
        let mut req = msg(MessageType::Request, 3);
        req.set_option(opt::SERVER_ID, vec![10, 1, 1, 1]);
        req.set_option(opt::REQUESTED_IP, r.msg.yiaddr.octets().to_vec());
        let o = handle(&c, SERVER, &mut leases, 0, &req);
        assert!(o.reply.is_none() && o.changed);
        assert!(leases
            .of_client(LeaseKind::V4, "02:00:00:00:00:03", 0)
            .is_none());
        // Client identifiers take precedence over the MAC address.
        let mut d = msg(MessageType::Discover, 5);
        d.set_option(opt::CLIENT_ID, vec![1, 2, 0, 0, 0, 0, 5]);
        let _ = handle(&c, SERVER, &mut leases, 0, &d);
        assert!(leases
            .of_client(LeaseKind::V4, "01020000000005", 0)
            .is_some());
    }

    #[test]
    fn ignores_garbage() {
        let c = cfg();
        let mut leases = Leases::default();
        let mut m = msg(MessageType::Discover, 1);
        m.op = BOOTREPLY;
        assert!(handle(&c, SERVER, &mut leases, 0, &m).reply.is_none());
        let mut m = msg(MessageType::Discover, 1);
        m.options.clear();
        assert_eq!(
            handle(&c, SERVER, &mut leases, 0, &m).note.as_deref(),
            Some("no DHCP message type")
        );
        assert!(leases.list.is_empty());
    }
}
