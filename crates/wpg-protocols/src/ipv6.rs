//! IPv6 (RFC 8200) and its extension headers.

use wpg_core::{protocol, Binding, BindingEntry, ProtocolDef};

/// Next Header table shared by IPv6 and every extension header.
fn next_header_binding() -> Binding {
    let table = [
        (0, "ipv6_hopopts"),
        (4, "ipv4"),
        (6, "tcp"),
        (17, "udp"),
        (41, "ipv6"),
        (43, "ipv6_route"),
        (44, "ipv6_frag"),
        (47, "gre"),
        (58, "icmpv6"),
        (60, "ipv6_dstopts"),
        (88, "eigrp"),
        (89, "ospfv3"),
        (112, "vrrpv3_ipv6"),
        (115, "l2tpv3"),
    ];
    Binding::new(
        "next_header",
        table
            .iter()
            .map(|(value, p)| BindingEntry::new(*value, *p))
            .collect(),
    )
}

/// IPv6 fixed header (40 octets).
///
/// The traffic class is split into `dscp` (6) and `ecn` (2). Auto fields:
/// `payload_length` (everything after this header, including extension headers)
/// and `next_header` (from the next layer's binding). Supplies the pseudo header
/// for TCP / UDP / ICMPv6 checksums; with extension headers in between, the
/// upper-layer protocol number is taken from the binding, not from the first
/// extension header.
#[must_use]
pub fn ipv6() -> ProtocolDef {
    let mut d = protocol! {
        id: "ipv6",
        name: "IPv6",
        layer: L3,
        fields: {
            version: uint(4) = 6;
            dscp: uint(6) = 0;
            ecn: uint(2) = 0;
            flow_label: uint(20) = 0;
            payload_length: uint(16), auto = length(payload);
            next_header: enum(8) {
                0 => hop_by_hop,
                4 => ipv4,
                6 => tcp,
                17 => udp,
                41 => ipv6,
                43 => routing,
                44 => fragment,
                50 => esp,
                51 => ah,
                58 => icmpv6,
                59 => no_next_header,
                60 => destination_options,
                88 => eigrp,
                89 => ospf,
                112 => vrrp,
                253 => experimental,
            } = 253, auto = next_protocol;
            hop_limit: uint(8) = 64;
            src: ipv6 = [0x2001u16, 0xdb8, 0, 0, 0, 0, 0, 1];
            dst: ipv6 = [0x2001u16, 0xdb8, 0, 0, 0, 0, 0, 2];
        },
        pseudo_header: (src, dst),
        constraints: {
            warning: equals(version, 6);
            warning: not_equals(hop_limit, 0);
        },
    };
    d.next = Some(next_header_binding());
    d
}

/// Hop-by-Hop Options (next header 0). `options` are raw TLV octets (default one
/// `PadN`); automatic zero padding (`Pad1`) extends the header to a multiple of 8.
/// `hdr_ext_len` counts 8-octet units beyond the first.
#[must_use]
pub fn ipv6_hopopts() -> ProtocolDef {
    let mut d = protocol! {
        id: "ipv6_hopopts",
        name: "IPv6 Hop-by-Hop Options",
        layer: L3,
        fields: {
            next_header: uint(8) = 253, auto = next_protocol;
            hdr_ext_len: uint(8), auto = length(header, unit = words64, adjust = -1);
            options: bytes(from = hdr_ext_len, unit = 8, adjust = 6) = [1, 4, 0, 0, 0, 0], desc = "ipv6-options";
            padding: padding(8);
        },
    };
    d.next = Some(next_header_binding());
    d
}

/// Destination Options (next header 60); same layout as [`ipv6_hopopts`].
#[must_use]
pub fn ipv6_dstopts() -> ProtocolDef {
    let mut d = protocol! {
        id: "ipv6_dstopts",
        name: "IPv6 Destination Options",
        layer: L3,
        fields: {
            next_header: uint(8) = 253, auto = next_protocol;
            hdr_ext_len: uint(8), auto = length(header, unit = words64, adjust = -1);
            options: bytes(from = hdr_ext_len, unit = 8, adjust = 6) = [1, 4, 0, 0, 0, 0], desc = "ipv6-options";
            padding: padding(8);
        },
    };
    d.next = Some(next_header_binding());
    d
}

/// Routing header (next header 43). Defaults to Type 2 (Mobile IPv6): one segment
/// left and `data` = 4 reserved octets + a 16-octet home address (all zero).
/// For Type 0 / 4 put the reserved word followed by the address list in `data`.
#[must_use]
pub fn ipv6_route() -> ProtocolDef {
    let mut d = protocol! {
        id: "ipv6_route",
        name: "IPv6 Routing",
        layer: L3,
        fields: {
            next_header: uint(8) = 253, auto = next_protocol;
            hdr_ext_len: uint(8), auto = length(header, unit = words64, adjust = -1);
            routing_type: enum(8) {
                0 => source_route_deprecated,
                2 => mobile_ipv6,
                3 => rpl,
                4 => segment_routing,
            } = 2, desc = "ipv6-routing-type";
            segments_left: uint(8) = 1, desc = "ipv6-segments-left";
            data: bytes(from = hdr_ext_len, unit = 8, adjust = 4) = [0; 20], desc = "ipv6-routing-data";
            padding: padding(8);
        },
        constraints: {
            warning: not_equals(routing_type, 0);
        },
    };
    d.next = Some(next_header_binding());
    d
}

/// Fragment header (next header 44), always 8 octets.
#[must_use]
pub fn ipv6_frag() -> ProtocolDef {
    let mut d = protocol! {
        id: "ipv6_frag",
        name: "IPv6 Fragment",
        layer: L3,
        fields: {
            next_header: uint(8) = 253, auto = next_protocol;
            reserved: uint(8) = 0;
            fragment_offset: uint(13) = 0, desc = "ipv6-fragment-offset";
            res: uint(2) = 0;
            m: uint(1) = 0, desc = "ipv6-more-fragments";
            identification: uint(32) = 0;
        },
    };
    d.next = Some(next_header_binding());
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defs_are_valid() {
        for d in [
            ipv6(),
            ipv6_hopopts(),
            ipv6_dstopts(),
            ipv6_route(),
            ipv6_frag(),
        ] {
            d.validate().unwrap();
        }
        assert_eq!(ipv6().fixed_header_bits(), Some(320));
        assert_eq!(ipv6_frag().fixed_header_bits(), Some(64));
    }
}
