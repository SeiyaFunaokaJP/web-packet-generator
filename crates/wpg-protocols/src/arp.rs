//! ARP (RFC 826).

use wpg_core::{protocol, ProtocolDef};

/// ARP for Ethernet / IPv4 (28 octets): hardware / protocol type and sizes,
/// operation and the sender / target hardware and protocol addresses.
///
/// `hlen` / `plen` default to 6 / 4 and are plain fields (not Auto) so that other
/// address sizes can be produced on purpose; constraints warn when they differ
/// from the Ethernet / IPv4 form this definition encodes. Bound from Ethernet II
/// (EtherType `0x0806`) and 802.1Q.
#[must_use]
pub fn arp() -> ProtocolDef {
    protocol! {
        id: "arp",
        name: "ARP",
        layer: L2,
        fields: {
            htype: enum(16) {
                1 => ethernet,
                6 => ieee802,
                15 => frame_relay,
                16 => atm,
            } = 1, desc = "arp-htype";
            ptype: enum(16) {
                0x0800 => ipv4,
                0x86dd => ipv6,
            } = 0x0800, desc = "arp-ptype";
            hlen: uint(8) = 6, desc = "arp-hlen";
            plen: uint(8) = 4, desc = "arp-plen";
            oper: enum(16) {
                1 => request,
                2 => reply,
                3 => rarp_request,
                4 => rarp_reply,
                8 => inarp_request,
                9 => inarp_reply,
            } = 1, desc = "arp-oper";
            sha: mac = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01], desc = "arp-sha";
            spa: ipv4 = [192, 0, 2, 1], desc = "arp-spa";
            tha: mac = [0, 0, 0, 0, 0, 0], desc = "arp-tha";
            tpa: ipv4 = [192, 0, 2, 2], desc = "arp-tpa";
        },
        constraints: {
            warning: equals(hlen, 6);
            warning: equals(plen, 4);
            warning: in_range(oper, 1, 2);
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arp_is_28_octets() {
        let d = arp();
        d.validate().unwrap();
        assert_eq!(d.fixed_header_bits(), Some(28 * 8));
    }
}
