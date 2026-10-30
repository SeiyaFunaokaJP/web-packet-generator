//! The lease table of both families (saved by the caller as JSON).

use std::net::IpAddr;

use serde::{Deserialize, Serialize};

/// How long an offered (not yet requested) address is held.
pub const OFFER_HOLD_SECS: u64 = 60;

/// What a lease is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaseKind {
    /// DHCPv4 address.
    V4,
    /// DHCPv6 address (`IA_NA`).
    Na,
    /// Delegated prefix (`IA_PD`).
    Pd,
}

/// Lease state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaseState {
    /// Offered / advertised, waiting for the request.
    Offered,
    /// Acknowledged.
    Bound,
    /// The client reported the address in use (DECLINE); kept out of the
    /// pool until it expires.
    Declined,
}

/// One lease.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lease {
    /// v4 / NA / PD.
    pub kind: LeaseKind,
    /// Address or prefix.
    pub addr: IpAddr,
    /// Prefix length (32 / 128 for addresses).
    pub plen: u8,
    /// Client identity: v4 client identifier (hex) or MAC, v6 DUID (hex).
    pub client: String,
    /// IAID (v6).
    #[serde(default)]
    pub iaid: u32,
    /// Client MAC address (v4; v6 when the DUID carries one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    /// Host name the client sent (v4 option 12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    /// State.
    pub state: LeaseState,
    /// Expiry, seconds since the Unix epoch.
    pub expires: u64,
    /// Last change, seconds since the Unix epoch.
    pub updated: u64,
}

impl Lease {
    /// `192.168.1.10`, `2001:db8::5`, `2001:db8:100::/56`.
    #[must_use]
    pub fn key(&self) -> String {
        match self.kind {
            LeaseKind::Pd => format!("{}/{}", self.addr, self.plen),
            _ => self.addr.to_string(),
        }
    }

    /// Still valid at `now`.
    #[must_use]
    pub fn active(&self, now: u64) -> bool {
        self.expires > now
    }
}

/// All leases.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Leases {
    /// In allocation order.
    pub list: Vec<Lease>,
}

impl Leases {
    /// Drops expired leases; true when something was removed.
    pub fn purge(&mut self, now: u64) -> bool {
        let before = self.list.len();
        self.list.retain(|l| l.active(now));
        before != self.list.len()
    }

    /// The active lease holding `addr` (any client).
    #[must_use]
    pub fn holder(&self, kind: LeaseKind, addr: IpAddr, now: u64) -> Option<&Lease> {
        self.list
            .iter()
            .find(|l| l.kind == kind && l.addr == addr && l.active(now))
    }

    /// The lease of a client (v4: `iaid` 0).
    #[must_use]
    pub fn of_client(&self, kind: LeaseKind, client: &str, iaid: u32) -> Option<&Lease> {
        self.list.iter().find(|l| {
            l.kind == kind
                && l.client == client
                && l.iaid == iaid
                && l.state != LeaseState::Declined
        })
    }

    /// Inserts or replaces the lease of `lease.client` / `iaid` / kind (and any
    /// other lease on the same address).
    pub fn upsert(&mut self, lease: Lease) {
        self.list.retain(|l| {
            l.kind != lease.kind
                || (l.addr != lease.addr
                    && !(l.client == lease.client
                        && l.iaid == lease.iaid
                        && l.state != LeaseState::Declined))
        });
        self.list.push(lease);
    }

    /// Removes the client's lease on `addr`; true when one existed.
    pub fn release(&mut self, kind: LeaseKind, client: &str, addr: IpAddr) -> bool {
        let before = self.list.len();
        self.list
            .retain(|l| !(l.kind == kind && l.client == client && l.addr == addr));
        before != self.list.len()
    }

    /// Removes the lease with this [`Lease::key`]; true when one existed.
    pub fn remove_key(&mut self, key: &str) -> bool {
        let before = self.list.len();
        self.list.retain(|l| l.key() != key);
        before != self.list.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lease(addr: &str, client: &str, expires: u64) -> Lease {
        Lease {
            kind: LeaseKind::V4,
            addr: addr.parse().unwrap(),
            plen: 32,
            client: client.into(),
            iaid: 0,
            mac: None,
            hostname: None,
            state: LeaseState::Bound,
            expires,
            updated: 0,
        }
    }

    #[test]
    fn upsert_replaces_client_and_address() {
        let mut l = Leases::default();
        l.upsert(lease("10.0.0.1", "a", 100));
        l.upsert(lease("10.0.0.2", "b", 100));
        // a moves to .3: its old lease goes.
        l.upsert(lease("10.0.0.3", "a", 100));
        // c takes .2 (b's lease had to be removed first by the caller's logic;
        // the table never keeps two leases on one address).
        l.upsert(lease("10.0.0.2", "c", 100));
        let keys: Vec<_> = l.list.iter().map(|x| (x.key(), x.client.clone())).collect();
        assert_eq!(
            keys,
            [
                ("10.0.0.3".into(), "a".into()),
                ("10.0.0.2".into(), "c".into())
            ]
        );
        assert!(l
            .holder(LeaseKind::V4, "10.0.0.3".parse().unwrap(), 50)
            .is_some());
        assert!(l
            .holder(LeaseKind::V4, "10.0.0.3".parse().unwrap(), 100)
            .is_none());
        assert!(l.release(LeaseKind::V4, "a", "10.0.0.3".parse().unwrap()));
        assert!(l.remove_key("10.0.0.2"));
        l.upsert(lease("10.0.0.9", "z", 5));
        assert!(l.purge(10));
        assert!(l.list.is_empty());
    }
}
