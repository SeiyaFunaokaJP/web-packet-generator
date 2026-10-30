//! DHCPv6 server logic (RFC 8415 §18.3): addresses (`IA_NA`), delegated
//! prefixes (`IA_PD`), DNS options, Rapid Commit. No sockets here; relayed
//! messages are not handled.

use std::collections::HashSet;
use std::net::{IpAddr, Ipv6Addr};

use crate::config::V6Config;
use crate::lease::{Lease, LeaseKind, LeaseState, Leases, OFFER_HOLD_SECS};
use crate::v6::{
    encode_domains, fmt_duid, msg, msg_name, opt, status, status_data, Ia, IaAddr, IaKind, Message,
};

/// Largest number of candidate addresses / prefixes looked at per allocation.
const MAX_SCAN: u128 = 1 << 20;

/// What happened to one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// `SOLICIT`, ...
    pub request: &'static str,
    /// Client DUID (`00:01:...`).
    pub client: String,
    /// The reply, if any.
    pub reply: Option<Message>,
    /// Addresses / prefixes handed out or released.
    pub addrs: Vec<String>,
    /// Why nothing was sent, or what was refused.
    pub note: Option<String>,
    /// The lease table changed.
    pub changed: bool,
}

impl Outcome {
    fn note(mut self, n: impl Into<String>) -> Self {
        self.note = Some(n.into());
        self
    }
}

/// The MAC address inside a DUID-LLT / DUID-LL of Ethernet type.
#[must_use]
pub fn duid_mac(duid: &[u8]) -> Option<[u8; 6]> {
    let ([0, 1, 0, 1, _, _, _, _, tail @ ..] | [0, 3, 0, 1, tail @ ..]) = duid else {
        return None;
    };
    <[u8; 6]>::try_from(tail).ok()
}

struct Ctx<'a> {
    cfg: &'a V6Config,
    now: u64,
    duid: &'a [u8],
    client: String,
}

impl Ctx<'_> {
    fn kind(ia: IaKind) -> LeaseKind {
        match ia {
            IaKind::Na => LeaseKind::Na,
            IaKind::Pd => LeaseKind::Pd,
        }
    }

    fn plen(&self, kind: IaKind) -> u8 {
        match kind {
            IaKind::Na => 128,
            IaKind::Pd => self.cfg.pd_delegated_len,
        }
    }

    /// `addr` belongs to the pool of `kind`.
    fn in_pool(&self, kind: IaKind, addr: Ipv6Addr) -> bool {
        let c = self.cfg;
        match kind {
            IaKind::Na => addr >= c.range_start && addr <= c.range_end,
            IaKind::Pd => {
                crate::config::mask6(addr, c.pd_prefix_len)
                    == crate::config::mask6(c.pd_prefix, c.pd_prefix_len)
                    && crate::config::mask6(addr, c.pd_delegated_len) == addr
            }
        }
    }

    fn free(&self, leases: &Leases, kind: IaKind, iaid: u32, addr: Ipv6Addr) -> bool {
        leases
            .holder(Self::kind(kind), IpAddr::V6(addr), self.now)
            .is_none_or(|l| l.client == self.client && l.iaid == iaid)
    }

    fn choose(&self, leases: &Leases, ia: &Ia) -> Option<Ipv6Addr> {
        let kind = Self::kind(ia.kind);
        if let Some(IpAddr::V6(a)) = leases
            .of_client(kind, &self.client, ia.iaid)
            .map(|l| l.addr)
        {
            if self.in_pool(ia.kind, a) && self.free(leases, ia.kind, ia.iaid, a) {
                return Some(a);
            }
        }
        if let Some(h) = ia
            .addrs
            .iter()
            .map(|a| a.addr)
            .find(|a| self.in_pool(ia.kind, *a) && self.free(leases, ia.kind, ia.iaid, *a))
        {
            return Some(h);
        }
        let used: HashSet<IpAddr> = leases
            .list
            .iter()
            .filter(|l| l.kind == kind && l.active(self.now))
            .map(|l| l.addr)
            .collect();
        let c = self.cfg;
        let (first, step, count) = match ia.kind {
            IaKind::Na => (
                u128::from(c.range_start),
                1u128,
                u128::from(c.range_end).saturating_sub(u128::from(c.range_start)) + 1,
            ),
            IaKind::Pd => {
                let shift = 128 - u32::from(c.pd_delegated_len);
                (
                    u128::from(crate::config::mask6(c.pd_prefix, c.pd_prefix_len)),
                    1u128.checked_shl(shift).unwrap_or(0),
                    1u128 << (c.pd_delegated_len - c.pd_prefix_len),
                )
            }
        };
        (0..count.min(MAX_SCAN))
            .map(|i| Ipv6Addr::from(first.wrapping_add(i.wrapping_mul(step))))
            .find(|a| !used.contains(&IpAddr::V6(*a)))
    }

    fn status_ia(ia: &Ia, code: u16, text: &str) -> Ia {
        Ia {
            kind: ia.kind,
            iaid: ia.iaid,
            t1: 0,
            t2: 0,
            addrs: Vec::new(),
            status: Some((code, text.to_owned())),
        }
    }

    /// Allocates (or extends) the IA. `need_binding`: RENEW semantics.
    fn bind(
        &self,
        leases: &mut Leases,
        ia: &Ia,
        state: LeaseState,
        need_binding: bool,
        out: &mut Outcome,
    ) -> Ia {
        let c = self.cfg;
        let enabled = match ia.kind {
            IaKind::Na => c.na_enabled,
            IaKind::Pd => c.pd_enabled,
        };
        let none = match ia.kind {
            IaKind::Na => status::NO_ADDRS_AVAIL,
            IaKind::Pd => status::NO_PREFIX_AVAIL,
        };
        if !enabled {
            return Self::status_ia(ia, none, "not offered by this server");
        }
        if need_binding
            && leases
                .of_client(Self::kind(ia.kind), &self.client, ia.iaid)
                .is_none()
        {
            return Self::status_ia(ia, status::NO_BINDING, "no binding");
        }
        let Some(addr) = self.choose(leases, ia) else {
            out.note = Some("pool exhausted".into());
            return Self::status_ia(ia, none, "pool exhausted");
        };
        let plen = self.plen(ia.kind);
        let secs = if state == LeaseState::Offered {
            OFFER_HOLD_SECS
        } else {
            u64::from(c.valid_secs)
        };
        let bound_already = leases
            .of_client(Self::kind(ia.kind), &self.client, ia.iaid)
            .is_some_and(|l| {
                l.addr == IpAddr::V6(addr) && l.state == LeaseState::Bound && l.active(self.now)
            });
        if state == LeaseState::Bound || !bound_already {
            leases.upsert(Lease {
                kind: Self::kind(ia.kind),
                addr: IpAddr::V6(addr),
                plen,
                client: self.client.clone(),
                iaid: ia.iaid,
                mac: duid_mac(self.duid).map(|m| crate::fmt_mac(&m)),
                hostname: None,
                state,
                expires: self.now + secs,
                updated: self.now,
            });
            out.changed = true;
        }
        out.addrs.push(if ia.kind == IaKind::Pd {
            format!("{addr}/{plen}")
        } else {
            addr.to_string()
        });
        Ia {
            kind: ia.kind,
            iaid: ia.iaid,
            t1: c.preferred_secs / 2,
            t2: u32::try_from(u64::from(c.preferred_secs) * 4 / 5).unwrap_or(c.preferred_secs),
            addrs: vec![IaAddr {
                addr,
                plen,
                preferred: c.preferred_secs,
                valid: c.valid_secs,
            }],
            status: None,
        }
    }
}

/// Handles one datagram from a client.
#[must_use]
pub fn handle(
    cfg: &V6Config,
    server_duid: &[u8],
    leases: &mut Leases,
    now: u64,
    req: &Message,
) -> Outcome {
    let duid = req.option(opt::CLIENTID).unwrap_or_default();
    let mut out = Outcome {
        request: msg_name(req.msg_type),
        client: fmt_duid(duid),
        reply: None,
        addrs: Vec::new(),
        note: None,
        changed: false,
    };
    let t = req.msg_type;
    if matches!(t, msg::RELAY_FORW | msg::RELAY_REPL) {
        return out.note("relayed messages are not supported");
    }
    if !matches!(
        t,
        msg::SOLICIT
            | msg::REQUEST
            | msg::CONFIRM
            | msg::RENEW
            | msg::REBIND
            | msg::RELEASE
            | msg::DECLINE
            | msg::INFORMATION_REQUEST
    ) {
        return out.note("not a client message");
    }
    if duid.is_empty() && t != msg::INFORMATION_REQUEST {
        return out.note("no client identifier");
    }
    let sid = req.option(opt::SERVERID);
    let must_have = matches!(t, msg::REQUEST | msg::RENEW | msg::RELEASE | msg::DECLINE);
    let must_not = matches!(t, msg::SOLICIT | msg::CONFIRM | msg::REBIND);
    match sid {
        Some(_) if must_not => return out.note("unexpected server identifier"),
        Some(s) if s != server_duid => return out.note("addressed to another server"),
        None if must_have => return out.note("no server identifier"),
        _ => {}
    }
    let ctx = Ctx {
        cfg,
        now,
        duid,
        client: crate::hex(duid),
    };
    let rapid = t == msg::SOLICIT && cfg.rapid_commit && req.has(opt::RAPID_COMMIT);
    let mut reply = Message {
        msg_type: if t == msg::SOLICIT && !rapid {
            msg::ADVERTISE
        } else {
            msg::REPLY
        },
        xid: req.xid,
        options: vec![(opt::SERVERID, server_duid.to_vec())],
    };
    if !duid.is_empty() {
        reply.push(opt::CLIENTID, duid.to_vec());
    }
    if rapid {
        reply.push(opt::RAPID_COMMIT, Vec::new());
    }
    let ias = req.ias();
    match t {
        msg::SOLICIT | msg::REQUEST | msg::RENEW | msg::REBIND => {
            let state = if t == msg::SOLICIT && !rapid {
                LeaseState::Offered
            } else {
                LeaseState::Bound
            };
            for ia in &ias {
                let r = ctx.bind(leases, ia, state, t == msg::RENEW, &mut out);
                let (c, d) = r.encode();
                reply.push(c, d);
            }
            if t == msg::SOLICIT && !rapid && cfg.preference > 0 {
                reply.push(opt::PREFERENCE, vec![cfg.preference]);
            }
        }
        msg::RELEASE | msg::DECLINE => {
            for ia in &ias {
                let kind = Ctx::kind(ia.kind);
                for a in &ia.addrs {
                    out.addrs.push(a.addr.to_string());
                    out.changed |= leases.release(kind, &ctx.client, IpAddr::V6(a.addr));
                    if t == msg::DECLINE {
                        leases.upsert(Lease {
                            kind,
                            addr: IpAddr::V6(a.addr),
                            plen: a.plen,
                            client: format!("declined by {}", ctx.client),
                            iaid: ia.iaid,
                            mac: None,
                            hostname: None,
                            state: LeaseState::Declined,
                            expires: now + u64::from(cfg.valid_secs),
                            updated: now,
                        });
                        out.changed = true;
                    }
                }
            }
            reply.push(opt::STATUS_CODE, status_data(status::SUCCESS, "done"));
        }
        msg::CONFIRM => {
            let addrs: Vec<_> = ias
                .iter()
                .filter(|ia| ia.kind == IaKind::Na)
                .flat_map(|ia| ia.addrs.iter().map(|a| a.addr))
                .collect();
            if addrs.is_empty() {
                return out.note("nothing to confirm");
            }
            let on_link = cfg.na_enabled && addrs.iter().all(|a| ctx.in_pool(IaKind::Na, *a));
            out.addrs = addrs.iter().map(ToString::to_string).collect();
            reply.push(
                opt::STATUS_CODE,
                if on_link {
                    status_data(status::SUCCESS, "on link")
                } else {
                    status_data(status::NOT_ON_LINK, "not on link")
                },
            );
        }
        _ => {} // INFORMATION-REQUEST: options only.
    }
    if !cfg.dns.is_empty() {
        reply.push(
            opt::DNS_SERVERS,
            cfg.dns.iter().flat_map(Ipv6Addr::octets).collect(),
        );
    }
    if !cfg.domains.is_empty() {
        reply.push(opt::DOMAIN_LIST, encode_domains(&cfg.domains));
    }
    out.reply = Some(reply);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v6::duid_ll;

    fn cfg() -> V6Config {
        V6Config {
            enabled: true,
            range_start: "2001:db8::10".parse().unwrap(),
            range_end: "2001:db8::11".parse().unwrap(),
            pd_enabled: true,
            pd_prefix: "2001:db8:100::".parse().unwrap(),
            pd_prefix_len: 48,
            pd_delegated_len: 56,
            dns: vec!["2001:db8::53".parse().unwrap()],
            domains: vec!["lab".into()],
            preferred_secs: 1000,
            valid_secs: 2000,
            rapid_commit: true,
            preference: 255,
            ..V6Config::default()
        }
    }

    fn server() -> Vec<u8> {
        duid_ll([2, 0, 0, 0, 0, 0xff])
    }

    fn req(t: u8, client: u8, ias: &[(IaKind, u32)]) -> Message {
        let mut m = Message {
            msg_type: t,
            xid: 0x0012_3456,
            options: vec![(opt::CLIENTID, duid_ll([2, 0, 0, 0, 0, client]))],
        };
        for (kind, iaid) in ias {
            let (c, d) = Ia {
                kind: *kind,
                iaid: *iaid,
                t1: 0,
                t2: 0,
                addrs: Vec::new(),
                status: None,
            }
            .encode();
            m.push(c, d);
        }
        m
    }

    fn with_server(mut m: Message) -> Message {
        m.push(opt::SERVERID, server());
        m
    }

    #[test]
    fn solicit_advertise_request_reply() {
        let c = cfg();
        let mut leases = Leases::default();
        let sol = req(msg::SOLICIT, 1, &[(IaKind::Na, 1), (IaKind::Pd, 2)]);
        let o = handle(&c, &server(), &mut leases, 0, &sol);
        let adv = o.reply.unwrap();
        assert_eq!(adv.msg_type, msg::ADVERTISE);
        assert_eq!(adv.xid, 0x0012_3456);
        assert_eq!(adv.option(opt::SERVERID), Some(server().as_slice()));
        assert_eq!(adv.option(opt::PREFERENCE), Some(&[255][..]));
        assert_eq!(adv.option(opt::DOMAIN_LIST), Some(&b"\x03lab\x00"[..]));
        let ias = adv.ias();
        assert_eq!(
            ias[0].addrs[0].addr,
            "2001:db8::10".parse::<Ipv6Addr>().unwrap()
        );
        assert_eq!((ias[0].t1, ias[0].t2), (500, 800));
        assert_eq!(
            ias[1].addrs[0].addr,
            "2001:db8:100::".parse::<Ipv6Addr>().unwrap()
        );
        assert_eq!(ias[1].addrs[0].plen, 56);
        assert_eq!(o.addrs, ["2001:db8::10", "2001:db8:100::/56"]);
        assert!(leases.list.iter().all(|l| l.state == LeaseState::Offered));
        assert_eq!(leases.list[0].mac.as_deref(), Some("02:00:00:00:00:01"));

        let r = handle(
            &c,
            &server(),
            &mut leases,
            1,
            &with_server(req(msg::REQUEST, 1, &[(IaKind::Na, 1), (IaKind::Pd, 2)])),
        );
        let rep = r.reply.unwrap();
        assert_eq!(rep.msg_type, msg::REPLY);
        assert_eq!(rep.ias()[0].addrs[0].valid, 2000);
        assert!(leases
            .list
            .iter()
            .all(|l| l.state == LeaseState::Bound && l.expires == 2001));
        assert_eq!(leases.list.len(), 2);

        // A second client gets the next address and prefix.
        let o = handle(
            &c,
            &server(),
            &mut leases,
            1,
            &req(msg::SOLICIT, 2, &[(IaKind::Na, 1), (IaKind::Pd, 1)]),
        );
        assert_eq!(o.addrs, ["2001:db8::11", "2001:db8:100:100::/56"]);
        // A third one: NA pool exhausted, PD still available.
        let o = handle(
            &c,
            &server(),
            &mut leases,
            1,
            &req(msg::SOLICIT, 3, &[(IaKind::Na, 1), (IaKind::Pd, 1)]),
        );
        let ias = o.reply.unwrap().ias();
        assert_eq!(
            ias[0].status.as_ref().map(|s| s.0),
            Some(status::NO_ADDRS_AVAIL)
        );
        assert_eq!(
            ias[1].addrs[0].addr,
            "2001:db8:100:200::".parse::<Ipv6Addr>().unwrap()
        );
    }

    #[test]
    fn rapid_commit_renew_release_decline_confirm() {
        let c = cfg();
        let mut leases = Leases::default();
        let mut sol = req(msg::SOLICIT, 1, &[(IaKind::Na, 1)]);
        sol.push(opt::RAPID_COMMIT, Vec::new());
        let rep = handle(&c, &server(), &mut leases, 0, &sol).reply.unwrap();
        assert_eq!(rep.msg_type, msg::REPLY);
        assert!(rep.has(opt::RAPID_COMMIT));
        assert_eq!(leases.list[0].state, LeaseState::Bound);

        // RENEW extends; RENEW of an unknown IA gets NoBinding.
        let rep = handle(
            &c,
            &server(),
            &mut leases,
            500,
            &with_server(req(msg::RENEW, 1, &[(IaKind::Na, 1), (IaKind::Na, 9)])),
        )
        .reply
        .unwrap();
        assert_eq!(leases.list[0].expires, 2500);
        assert_eq!(
            rep.ias()[1].status.as_ref().map(|s| s.0),
            Some(status::NO_BINDING)
        );
        // REBIND without a server id is answered.
        assert!(handle(
            &c,
            &server(),
            &mut leases,
            600,
            &req(msg::REBIND, 1, &[(IaKind::Na, 1)])
        )
        .reply
        .is_some());

        // CONFIRM.
        let mut conf = req(msg::CONFIRM, 1, &[]);
        let (code, d) = Ia {
            kind: IaKind::Na,
            iaid: 1,
            t1: 0,
            t2: 0,
            addrs: vec![IaAddr {
                addr: "2001:db8::10".parse().unwrap(),
                plen: 128,
                preferred: 0,
                valid: 0,
            }],
            status: None,
        }
        .encode();
        conf.push(code, d.clone());
        let rep = handle(&c, &server(), &mut leases, 600, &conf)
            .reply
            .unwrap();
        assert_eq!(rep.option(opt::STATUS_CODE).map(|s| s[1]), Some(0));

        // RELEASE removes the lease.
        let mut rel = with_server(req(msg::RELEASE, 1, &[]));
        rel.push(code, d.clone());
        let o = handle(&c, &server(), &mut leases, 600, &rel);
        assert!(o.changed && leases.list.is_empty());
        // DECLINE marks the address.
        let mut dec = with_server(req(msg::DECLINE, 1, &[]));
        dec.push(code, d);
        let _ = handle(&c, &server(), &mut leases, 600, &dec);
        assert_eq!(leases.list[0].state, LeaseState::Declined);
        let o = handle(
            &c,
            &server(),
            &mut leases,
            600,
            &req(msg::SOLICIT, 1, &[(IaKind::Na, 1)]),
        );
        assert_eq!(o.addrs, ["2001:db8::11"]);
    }

    #[test]
    fn server_id_rules_and_information_request() {
        let c = cfg();
        let mut leases = Leases::default();
        let o = handle(
            &c,
            &server(),
            &mut leases,
            0,
            &req(msg::REQUEST, 1, &[(IaKind::Na, 1)]),
        );
        assert_eq!(o.note.as_deref(), Some("no server identifier"));
        let mut other = req(msg::REQUEST, 1, &[(IaKind::Na, 1)]);
        other.push(opt::SERVERID, vec![0, 3, 0, 1, 9, 9, 9, 9, 9, 9]);
        assert!(handle(&c, &server(), &mut leases, 0, &other)
            .reply
            .is_none());
        let o = handle(
            &c,
            &server(),
            &mut leases,
            0,
            &with_server(req(msg::SOLICIT, 1, &[])),
        );
        assert_eq!(o.note.as_deref(), Some("unexpected server identifier"));
        let info = Message {
            msg_type: msg::INFORMATION_REQUEST,
            xid: 1,
            options: Vec::new(),
        };
        let rep = handle(&c, &server(), &mut leases, 0, &info).reply.unwrap();
        assert_eq!(rep.option(opt::DNS_SERVERS).map(<[u8]>::len), Some(16));
        assert!(rep.option(opt::CLIENTID).is_none());
        let relay = Message {
            msg_type: msg::RELAY_FORW,
            ..Message::default()
        };
        assert!(handle(&c, &server(), &mut leases, 0, &relay)
            .reply
            .is_none());
        assert!(leases.list.is_empty());
        // NA disabled: NoAddrsAvail.
        let mut c = cfg();
        c.na_enabled = false;
        let ias = handle(
            &c,
            &server(),
            &mut leases,
            0,
            &req(msg::SOLICIT, 1, &[(IaKind::Na, 1)]),
        )
        .reply
        .unwrap()
        .ias();
        assert_eq!(
            ias[0].status.as_ref().map(|s| s.0),
            Some(status::NO_ADDRS_AVAIL)
        );
    }

    #[test]
    fn duid_macs() {
        assert_eq!(
            duid_mac(&duid_ll([1, 2, 3, 4, 5, 6])),
            Some([1, 2, 3, 4, 5, 6])
        );
        assert_eq!(
            duid_mac(&[0, 1, 0, 1, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6]),
            Some([1, 2, 3, 4, 5, 6])
        );
        assert_eq!(duid_mac(&[0, 2, 0, 0]), None);
    }
}
