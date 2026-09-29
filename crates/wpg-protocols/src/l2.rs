//! Layer 2 protocol definitions.

use wpg_core::{protocol, BindingEntry, ProtocolDef};

/// Minimum Ethernet frame length without FCS (64 - 4).
pub const ETHERNET_MIN_FRAME: u32 = 60;

/// Largest 802.3 length value; EtherType values start at 0x0600.
pub const DOT3_MAX_LENGTH: u64 = 0x05ff;

/// Ethernet II (DIX) header: destination MAC, source MAC, EtherType (14 octets).
///
/// The EtherType is `Auto` and follows the next layer through the binding; with no
/// known next layer the default `0x88b5` (IEEE 802 local experimental) is used so
/// that a raw payload is not mistaken for IPv4 when parsed back.
///
/// A type/length value up to `0x05ff` means the frame is really IEEE 802.3: the
/// parser then re-decodes it as [`dot3`]. Short frames are zero padded to 60
/// octets (see `Packet::pad_to_min`).
#[must_use]
pub fn ethernet2() -> ProtocolDef {
    let mut d = protocol! {
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
            0x88a8 => "dot1ad",
        },
    };
    if let Some(b) = d.next.as_mut() {
        b.table
            .push(BindingEntry::range(0, DOT3_MAX_LENGTH, "dot3").reparsing());
    }
    d.min_frame_len = Some(ETHERNET_MIN_FRAME);
    d
}

/// IEEE 802.3 MAC header with a length field (`length` = octets after this
/// header, Auto; padding up to the minimum frame is not counted). Always followed
/// by [`llc`]. Ethernet II frames whose type/length is ≤ `0x05ff` parse as this.
#[must_use]
pub fn dot3() -> ProtocolDef {
    let mut d = protocol! {
        id: "dot3",
        name: "IEEE 802.3",
        layer: L2,
        fields: {
            dst: mac = [0xff; 6];
            src: mac = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
            length: uint(16), auto = length(payload);
        },
        next: length {},
        constraints: {
            warning: in_range(length, 0, 1500);
        },
    };
    if let Some(b) = d.next.as_mut() {
        b.default = Some("llc".into());
    }
    d.min_frame_len = Some(ETHERNET_MIN_FRAME);
    d
}

/// IEEE 802.2 LLC header (U-format, 1-octet control): DSAP, SSAP, control.
///
/// `dsap` and `ssap` are both Auto from the next layer (`0xaa` SNAP, `0xfe` OSI /
/// CLNP, `0x42` spanning tree). Control defaults to `0x03` (UI).
#[must_use]
pub fn llc() -> ProtocolDef {
    protocol! {
        id: "llc",
        name: "IEEE 802.2 LLC",
        layer: L2,
        fields: {
            dsap: enum(8) {
                0x42 => stp,
                0xaa => snap,
                0xe0 => ipx,
                0xf0 => netbios,
                0xfe => osi,
                0xff => global,
            } = 0xaa, auto = next_protocol;
            ssap: enum(8) {
                0x42 => stp,
                0xaa => snap,
                0xe0 => ipx,
                0xf0 => netbios,
                0xfe => osi,
            } = 0xaa, auto = next_protocol;
            control: enum(8) {
                0x03 => ui,
                0xaf => xid,
                0xe3 => test,
            } = 0x03;
        },
        next: dsap {
            0xaa => "snap",
            0xfe => "clnp",
            0x42 => "stp",
        },
    }
}

/// SNAP header (RFC 1042): OUI and protocol id (an EtherType when the OUI is 0).
/// `pid` is Auto from the next layer.
#[must_use]
pub fn snap() -> ProtocolDef {
    protocol! {
        id: "snap",
        name: "SNAP",
        layer: L2,
        fields: {
            oui: uint(24) = 0;
            pid: enum(16) {
                0x0800 => ipv4,
                0x0806 => arp,
                0x8100 => vlan,
                0x86dd => ipv6,
                0x88b5 => local_experimental,
            } = 0x88b5, auto = next_protocol;
        },
        next: pid {
            0x0800 => "ipv4",
            0x86dd => "ipv6",
            0x0806 => "arp",
            0x8100 => "dot1q",
        },
    }
}

/// IEEE 802.1ad service tag (S-tag, TPID `0x88a8` in the layer below): PCP, DEI,
/// VID and the inner EtherType (normally `0x8100`, followed by a [`dot1q`] C-tag).
#[must_use]
pub fn dot1ad() -> ProtocolDef {
    protocol! {
        id: "dot1ad",
        name: "802.1ad S-VLAN",
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
                0x88a8 => qinq,
                0x88b5 => local_experimental,
            } = 0x8100, auto = next_protocol;
        },
        next: ethertype {
            0x8100 => "dot1q",
            0x88a8 => "dot1ad",
            0x0800 => "ipv4",
            0x86dd => "ipv6",
            0x0806 => "arp",
        },
        constraints: {
            warning: not_equals(vid, 4095);
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
            0x88a8 => "dot1ad",
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
    fn llc_snap_dot3_dot1ad_are_valid() {
        for d in [dot3(), llc(), snap(), dot1ad()] {
            d.validate().unwrap();
        }
        assert_eq!(llc().fixed_header_bits(), Some(24));
        assert_eq!(snap().fixed_header_bits(), Some(40));
        let b = ethernet2().next.unwrap();
        assert_eq!(b.protocol_for(0x05dc), Some("dot3"));
        assert_eq!(b.protocol_for(0x0800), Some("ipv4"));
        assert_eq!(b.value_for("dot3"), None, "re-parse entries are not built");
        assert_eq!(dot3().next.unwrap().protocol_for(46), Some("llc"));
    }

    #[test]
    fn dot1q_is_4_octets() {
        let q = dot1q();
        q.validate().unwrap();
        assert_eq!(q.fixed_header_bits(), Some(32));
    }
}
