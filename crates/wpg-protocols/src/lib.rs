//! # wpg-protocols
//!
//! Built-in protocol definitions expressed in the `wpg-core` IR (written with the
//! [`wpg_core::protocol!`] macro), plus the [`Catalog`] registry served to the GUI
//! (`GET /api/protocols`) and used to build / parse packets ([`wpg_core::Resolve`]).
//!
//! Adding a protocol: write a `fn xyz() -> ProtocolDef` with `protocol!` in the
//! module of its layer, list it in [`builtin`], and add its id to the binding table
//! of the protocol(s) below it (e.g. an EtherType / IP protocol number entry).

use std::sync::OnceLock;

use serde::Serialize;
use wpg_core::{ProtocolDef, Resolve};

pub mod arp;
pub mod icmp;
pub mod ipv6;
pub mod l2;
pub mod l3;
pub mod l4;
pub mod payload;

/// Registry of protocol definitions, in palette order.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Catalog {
    protocols: Vec<ProtocolDef>,
}

impl Catalog {
    /// Builds a catalog from definitions.
    ///
    /// # Panics
    /// If a definition is internally inconsistent or two share an id; built-in
    /// definitions are covered by tests so this never fires in practice.
    #[must_use]
    pub fn new(protocols: Vec<ProtocolDef>) -> Self {
        for (i, def) in protocols.iter().enumerate() {
            if let Err(e) = def.validate() {
                panic!("invalid protocol definition `{}`: {e}", def.id);
            }
            assert!(
                protocols[..i].iter().all(|p| p.id != def.id),
                "duplicate protocol id `{}`",
                def.id
            );
        }
        Self { protocols }
    }

    /// Looks up a protocol by id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&ProtocolDef> {
        self.protocols.iter().find(|p| p.id == id)
    }

    /// Iterates over all protocols in palette order.
    pub fn iter(&self) -> impl Iterator<Item = &ProtocolDef> {
        self.protocols.iter()
    }

    /// Number of protocols.
    #[must_use]
    pub fn len(&self) -> usize {
        self.protocols.len()
    }

    /// Whether the catalog is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.protocols.is_empty()
    }

    /// All definitions as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[ProtocolDef] {
        &self.protocols
    }
}

impl Resolve for Catalog {
    fn protocol(&self, id: &str) -> Option<&ProtocolDef> {
        self.get(id)
    }
}

/// All built-in definitions, in palette order.
#[must_use]
pub fn builtin() -> Vec<ProtocolDef> {
    vec![
        l2::ethernet2(),
        l2::dot1q(),
        l2::dot1ad(),
        l2::dot3(),
        l2::llc(),
        l2::snap(),
        l3::ipv4(),
        l4::tcp(),
        l4::udp(),
        arp::arp(),
        ipv6::ipv6(),
        ipv6::ipv6_hopopts(),
        ipv6::ipv6_route(),
        ipv6::ipv6_frag(),
        ipv6::ipv6_dstopts(),
        icmp::icmpv4(),
        icmp::icmpv6(),
        icmp::icmpv6_echo(),
        icmp::ndp_rs(),
        icmp::ndp_ra(),
        icmp::ndp_ns(),
        icmp::ndp_na(),
        icmp::ndp_redirect(),
        payload::payload(),
        payload::raw(),
    ]
}

/// The built-in protocol catalog (built once, then shared).
#[must_use]
pub fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| Catalog::new(builtin()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_contains_ethernet2_first() {
        let cat = catalog();
        assert!(!cat.is_empty());
        assert_eq!(cat.iter().next().map(|p| p.id.as_str()), Some("ethernet2"));
        assert!(cat.get("ethernet2").is_some());
        assert!(cat.get("no-such-proto").is_none());
    }

    #[test]
    fn bindings_point_to_known_or_planned_protocols() {
        // Planned P0 ids that are referenced before they exist.
        const PLANNED: &[&str] = &["clnp", "stp"];
        for def in catalog().iter() {
            for e in def.next.iter().flat_map(|b| &b.table) {
                assert!(
                    catalog().get(&e.protocol).is_some() || PLANNED.contains(&e.protocol.as_str()),
                    "{} binds to unknown `{}`",
                    def.id,
                    e.protocol
                );
            }
        }
    }

    #[test]
    fn catalog_serialises() {
        let json = serde_json::to_value(catalog()).unwrap();
        assert_eq!(json["protocols"][0]["id"], "ethernet2");
    }
}
