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

pub mod app;
pub mod arp;
pub mod encap;
pub mod fhrp;
pub mod icmp;
pub mod ipv6;
pub mod l2;
pub mod l2control;
pub mod l3;
pub mod l4;
pub mod mcast;
pub mod osi;
pub mod payload;
pub mod routing;

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
        // P1 L2 control protocols.
        l2control::lldp(),
        l2control::cdp(),
        l2control::stp(),
        l2control::lacp(),
        l2control::marker(),
        l2control::eapol(),
        l2control::pppoe_discovery(),
        l2control::pppoe_session(),
        l2control::ppp(),
        l2control::ppp_lcp(),
        l2control::ppp_ipcp(),
        // P1 encapsulation protocols.
        encap::vxlan(),
        encap::gre(),
        encap::nvgre(),
        encap::mpls(),
        encap::gtpu(),
        encap::geneve(),
        encap::l2tpv2(),
        encap::l2tpv3(),
        // P1 routing protocols.
        routing::ospfv2(),
        routing::ospfv2_hello(),
        routing::ospfv2_dbd(),
        routing::ospfv2_lsr(),
        routing::ospfv2_lsu(),
        routing::ospfv2_lsack(),
        routing::ospfv2_lsa(),
        routing::ospfv2_router_lsa(),
        routing::ospfv2_network_lsa(),
        routing::ospfv2_summary_lsa(),
        routing::ospfv2_external_lsa(),
        routing::ospfv3(),
        routing::ospfv3_hello(),
        routing::ospfv3_dbd(),
        routing::ospfv3_lsr(),
        routing::ospfv3_lsu(),
        routing::ospfv3_lsack(),
        routing::ospfv3_lsa(),
        routing::ospfv3_router_lsa(),
        routing::ospfv3_network_lsa(),
        routing::ospfv3_inter_area_prefix_lsa(),
        routing::ospfv3_as_external_lsa(),
        routing::ospfv3_link_lsa(),
        routing::ospfv3_intra_area_prefix_lsa(),
        routing::bgp(),
        routing::bgp_open(),
        routing::bgp_update(),
        routing::bgp_notification(),
        routing::bgp_route_refresh(),
        routing::rip(),
        routing::ripng(),
        routing::eigrp(),
        // P1 redundancy and multicast protocols.
        fhrp::vrrpv2(),
        fhrp::vrrpv3(),
        fhrp::vrrpv3_ipv6(),
        fhrp::hsrp(),
        fhrp::hsrpv2(),
        fhrp::glbp(),
        mcast::igmp(),
        mcast::igmpv2(),
        mcast::igmpv3_query(),
        mcast::igmpv3_report(),
        mcast::mld(),
        mcast::mldv2_query(),
        mcast::mldv2_report(),
        mcast::pim(),
        mcast::pim_hello(),
        mcast::pim_join_prune(),
        mcast::pim_register(),
        // P1 application protocols.
        app::dhcpv4(),
        app::dhcpv6(),
        app::dhcpv6_relay(),
        app::dns(),
        app::ntp(),
        app::snmp(),
        app::snmp_pdu(),
        app::snmp_trap(),
        app::syslog(),
        app::radius(),
        app::tftp(),
        app::tftp_data(),
        app::tftp_ack(),
        app::tftp_error(),
        app::tftp_oack(),
        // P2 OSI encapsulation.
        osi::osi(),
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
        // Planned ids that are referenced before they exist (still on the roadmap).
        const PLANNED: &[&str] = &[];
        for def in catalog().iter() {
            let peeks = def.next.iter().flat_map(|b| b.peek.iter());
            for d in peeks.clone().filter_map(|p| p.default.as_ref()) {
                assert!(
                    catalog().get(d).is_some(),
                    "{} peeks to unknown `{d}`",
                    def.id
                );
            }
            let peeked = peeks.flat_map(|p| &p.table);
            for e in def.next.iter().flat_map(|b| &b.table).chain(peeked) {
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
