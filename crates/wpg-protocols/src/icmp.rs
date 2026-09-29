//! ICMPv4 (RFC 792) and ICMPv6 (RFC 4443) with Neighbor Discovery (RFC 4861).
//!
//! The engine has no type-dependent layouts inside one protocol, so:
//!
//! * `icmpv4` is `type, code, checksum` + a 4-octet `rest` of header (for Echo:
//!   identifier and sequence number as two big-endian 16-bit values) and the
//!   message body is a following `raw` layer.
//! * `icmpv6` is only `type, code, checksum`; the message body is a separate
//!   protocol chosen by the Auto `type` through the binding (`icmpv6_echo`,
//!   `ndp_rs`, `ndp_ra`, `ndp_ns`, `ndp_na`, `ndp_redirect`). Each body starts
//!   with its own 4 octets after the checksum. For any other type set `type`
//!   manually and follow with a `raw` layer that includes those 4 octets.

use wpg_core::{protocol, ProtocolDef};

/// ICMPv4: `type`, `code`, `checksum` (Auto, over header + payload) and 4 octets
/// `rest`. Defaults to Echo Request (type 8).
#[must_use]
pub fn icmpv4() -> ProtocolDef {
    protocol! {
        id: "icmpv4",
        name: "ICMPv4",
        layer: L3,
        fields: {
            type: enum(8) {
                0 => echo_reply,
                3 => destination_unreachable,
                4 => source_quench,
                5 => redirect,
                8 => echo_request,
                9 => router_advertisement,
                10 => router_solicitation,
                11 => time_exceeded,
                12 => parameter_problem,
                13 => timestamp_request,
                14 => timestamp_reply,
                15 => information_request,
                16 => information_reply,
                17 => address_mask_request,
                18 => address_mask_reply,
                42 => extended_echo_request,
                43 => extended_echo_reply,
            } = 8, desc = "icmp-type";
            code: uint(8) = 0, desc = "icmp-code";
            checksum: uint(16), auto = checksum(internet, header_payload);
            rest: bytes(4) = [0, 0, 0, 0], desc = "icmp-rest";
        },
        constraints: {
            // The engine has no set-membership rule; this covers the assigned range.
            warning: in_range(type, 0, 43);
        },
    }
}

/// ICMPv6 common header: `type` (Auto from the following body protocol, default
/// 128 = Echo Request), `code`, `checksum` (Auto, IPv6 pseudo header, over header
/// + payload including the body layers).
#[must_use]
pub fn icmpv6() -> ProtocolDef {
    protocol! {
        id: "icmpv6",
        name: "ICMPv6",
        layer: L3,
        fields: {
            type: enum(8) {
                1 => destination_unreachable,
                2 => packet_too_big,
                3 => time_exceeded,
                4 => parameter_problem,
                128 => echo_request,
                129 => echo_reply,
                130 => mld_query,
                131 => mld_report,
                132 => mld_done,
                133 => router_solicitation,
                134 => router_advertisement,
                135 => neighbor_solicitation,
                136 => neighbor_advertisement,
                137 => redirect,
                143 => mldv2_report,
            } = 128, auto = next_protocol, desc = "icmp-type";
            code: uint(8) = 0, desc = "icmp-code";
            checksum: uint(16), auto = checksum(internet_pseudo, header_payload);
        },
        next: type {
            128 => "icmpv6_echo",
            129 => "icmpv6_echo",
            133 => "ndp_rs",
            134 => "ndp_ra",
            135 => "ndp_ns",
            136 => "ndp_na",
            137 => "ndp_redirect",
        },
        constraints: {
            warning: in_range(type, 1, 255);
        },
    }
}

/// ICMPv6 Echo Request / Reply body: identifier and sequence number (the data
/// follows as a `raw` layer). Shared by types 128 and 129; when it follows
/// `icmpv6` the type is Auto 128 — set `type` to 129 for a reply.
#[must_use]
pub fn icmpv6_echo() -> ProtocolDef {
    protocol! {
        id: "icmpv6_echo",
        name: "ICMPv6 Echo",
        layer: L3,
        fields: {
            identifier: uint(16) = 0, desc = "icmp-identifier";
            sequence: uint(16) = 0, desc = "icmp-sequence";
        },
    }
}

/// NDP Router Solicitation (type 133). `options` are raw octets for now (Source
/// LLA = 1); switch to a TLV list when list fields can be built.
#[must_use]
pub fn ndp_rs() -> ProtocolDef {
    protocol! {
        id: "ndp_rs",
        name: "NDP Router Solicitation",
        layer: L3,
        fields: {
            reserved: uint(32) = 0;
            options: bytes(to_end), desc = "ndp-options";
        },
    }
}

/// NDP Router Advertisement (type 134). `options` are raw octets for now (Source
/// LLA = 1, MTU = 5, Prefix Information = 3).
#[must_use]
pub fn ndp_ra() -> ProtocolDef {
    protocol! {
        id: "ndp_ra",
        name: "NDP Router Advertisement",
        layer: L3,
        fields: {
            cur_hop_limit: uint(8) = 64;
            m: uint(1) = 0, desc = "ndp-managed";
            o: uint(1) = 0, desc = "ndp-other";
            reserved: uint(6) = 0;
            router_lifetime: uint(16) = 1800;
            reachable_time: uint(32) = 0;
            retrans_timer: uint(32) = 0;
            options: bytes(to_end), desc = "ndp-options";
        },
    }
}

/// NDP Neighbor Solicitation (type 135). `options` are raw octets for now (Source
/// LLA = 1).
#[must_use]
pub fn ndp_ns() -> ProtocolDef {
    protocol! {
        id: "ndp_ns",
        name: "NDP Neighbor Solicitation",
        layer: L3,
        fields: {
            reserved: uint(32) = 0;
            target: ipv6 = [0xfe80u16, 0, 0, 0, 0, 0, 0, 1], desc = "ndp-target";
            options: bytes(to_end), desc = "ndp-options";
        },
    }
}

/// NDP Neighbor Advertisement (type 136). `options` are raw octets for now
/// (Target LLA = 2).
#[must_use]
pub fn ndp_na() -> ProtocolDef {
    protocol! {
        id: "ndp_na",
        name: "NDP Neighbor Advertisement",
        layer: L3,
        fields: {
            r: uint(1) = 0, desc = "ndp-router-flag";
            s: uint(1) = 1, desc = "ndp-solicited-flag";
            o: uint(1) = 1, desc = "ndp-override-flag";
            reserved: uint(29) = 0;
            target: ipv6 = [0xfe80u16, 0, 0, 0, 0, 0, 0, 1], desc = "ndp-target";
            options: bytes(to_end), desc = "ndp-options";
        },
    }
}

/// NDP Redirect (type 137). `options` are raw octets for now (Target LLA = 2,
/// Redirected Header = 4).
#[must_use]
pub fn ndp_redirect() -> ProtocolDef {
    protocol! {
        id: "ndp_redirect",
        name: "NDP Redirect",
        layer: L3,
        fields: {
            reserved: uint(32) = 0;
            target: ipv6 = [0xfe80u16, 0, 0, 0, 0, 0, 0, 1], desc = "ndp-target";
            destination: ipv6 = [0x2001u16, 0xdb8, 0, 0, 0, 0, 0, 1], desc = "ndp-destination";
            options: bytes(to_end), desc = "ndp-options";
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defs_are_valid() {
        for d in [
            icmpv4(),
            icmpv6(),
            icmpv6_echo(),
            ndp_rs(),
            ndp_ra(),
            ndp_ns(),
            ndp_na(),
            ndp_redirect(),
        ] {
            d.validate().unwrap();
        }
        assert_eq!(icmpv4().fixed_header_bits(), Some(64));
        assert_eq!(icmpv6().fixed_header_bits(), Some(32));
    }
}
