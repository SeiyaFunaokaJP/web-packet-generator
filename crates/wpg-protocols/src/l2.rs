//! Layer 2 protocol definitions.

use wpg_core::{protocol, ProtocolDef};

/// Ethernet II (DIX) header: destination MAC, source MAC, EtherType (14 octets).
///
/// The EtherType is `Auto` and follows the next layer through the binding; with no
/// known next layer the default `0x88b5` (IEEE 802 local experimental) is used so
/// that a raw payload is not mistaken for IPv4 when parsed back.
#[must_use]
pub fn ethernet2() -> ProtocolDef {
    protocol! {
        id: "ethernet2",
        name: "Ethernet II",
        layer: L2,
        fields: {
            dst: mac = [0xff; 6];
            src: mac = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
            ethertype: enum(16) {
                0x0800 => ipv4,
                0x0806 => arp,
                0x8100 => vlan,
                0x86dd => ipv6,
                0x8847 => mpls,
                0x8848 => mpls_multicast,
                0x8863 => pppoe_discovery,
                0x8864 => pppoe_session,
                0x888e => eapol,
                0x88a8 => qinq,
                0x88b5 => local_experimental,
                0x88cc => lldp,
                0x88f7 => ptp,
            } = 0x88b5, auto = next_protocol;
        },
        next: ethertype {
            0x0800 => "ipv4",
            0x86dd => "ipv6",
            0x0806 => "arp",
            0x8100 => "dot1q",
        },
    }
}

/// IEEE 802.1Q VLAN tag (4 octets after the outer EtherType/TPID `0x8100`):
/// PCP, DEI, VID and the inner EtherType.
///
/// The TPID itself is the EtherType of the layer below (set automatically through
/// its binding), so stacking `dot1q` layers yields Q-in-Q style tag stacks.
#[must_use]
pub fn dot1q() -> ProtocolDef {
    protocol! {
        id: "dot1q",
        name: "802.1Q VLAN",
        layer: L2,
        fields: {
            pcp: uint(3) = 0;
            dei: uint(1) = 0;
            vid: uint(12) = 1;
            ethertype: enum(16) {
                0x0800 => ipv4,
                0x0806 => arp,
                0x8100 => vlan,
                0x86dd => ipv6,
                0x88b5 => local_experimental,
            } = 0x88b5, auto = next_protocol;
        },
        next: ethertype {
            0x0800 => "ipv4",
            0x86dd => "ipv6",
            0x0806 => "arp",
            0x8100 => "dot1q",
        },
        constraints: {
            warning: not_equals(vid, 4095);
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wpg_core::AutoCompute;

    #[test]
    fn ethernet2_is_14_octets() {
        let eth = ethernet2();
        eth.validate().unwrap();
        assert_eq!(eth.fixed_header_bits(), Some(14 * 8));
        let names: Vec<_> = eth.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["dst", "src", "ethertype"]);
        assert_eq!(
            eth.field("ethertype").and_then(|f| f.auto.clone()),
            Some(AutoCompute::NextProtocol)
        );
    }

    #[test]
    fn dot1q_is_4_octets() {
        let q = dot1q();
        q.validate().unwrap();
        assert_eq!(q.fixed_header_bits(), Some(32));
    }
}
