//! IPv6 (RFC 8200) and its extension headers, including the `SRv6` Segment Routing
//! Header (RFC 8754).
//!
//! A Routing header (next header 43) is decoded as [`ipv6_srh`] when its routing
//! type (the third octet, read with a [`Peek`] that skips 16 bits) is 4, and as the
//! generic [`ipv6_route`] otherwise.

use std::net::Ipv6Addr;

use wpg_core::{
    protocol, Binding, BindingEntry, FieldDef, FieldType, FieldValue, FinalDestination, ItemDef,
    Length, LengthUnit, Peek, ProtocolDef, TlvDef, TlvVariant,
};

/// Routing type of the Segment Routing Header (RFC 8754).
pub const ROUTING_TYPE_SRH: u64 = 4;

/// IPv6 next header "Ethernet" (RFC 8986: `SRv6` `End.DX2` / `End.DT2` payload).
pub const NEXT_HEADER_ETHERNET: u64 = 143;

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
        (46, "rsvp"),
        (47, "gre"),
        (58, "icmpv6"),
        (60, "ipv6_dstopts"),
        (88, "eigrp"),
        (89, "ospfv3"),
        (112, "vrrpv3_ipv6"),
        (115, "l2tpv3"),
        (132, "sctp"),
        (NEXT_HEADER_ETHERNET, "ethernet2"),
    ];
    let mut b = Binding::new(
        "next_header",
        table
            .iter()
            .map(|(value, p)| BindingEntry::new(*value, *p))
            .collect(),
    );
    // Routing header: the routing type (third octet) picks the layout.
    let mut peek =
        Peek::new(43, 8, vec![BindingEntry::new(ROUTING_TYPE_SRH, "ipv6_srh")]).skipping(16);
    peek.default = Some("ipv6_route".into());
    b.peek = Some(peek);
    b
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
            version: uint(4) = 6, desc = "ipv6-version";
            dscp: uint(6) = 0, desc = "ip-dscp";
            ecn: uint(2) = 0, desc = "ip-ecn";
            flow_label: uint(20) = 0, desc = "ipv6-flow-label";
            payload_length: uint(16), auto = length(payload), desc = "ipv6-payload-length";
            next_header: enum(8) {
                0 => hop_by_hop,
                4 => ipv4,
                6 => tcp,
                17 => udp,
                41 => ipv6,
                43 => routing,
                44 => fragment,
                46 => rsvp,
                50 => esp,
                51 => ah,
                58 => icmpv6,
                59 => no_next_header,
                60 => destination_options,
                88 => eigrp,
                89 => ospf,
                112 => vrrp,
                132 => sctp,
                143 => ethernet,
                253 => experimental,
            } = 253, auto = next_protocol, desc = "ipv6-next-header";
            hop_limit: uint(8) = 64, desc = "ipv6-hop-limit";
            src: ipv6 = [0x2001u16, 0xdb8, 0, 0, 0, 0, 0, 1], desc = "ipv6-src";
            dst: ipv6 = [0x2001u16, 0xdb8, 0, 0, 0, 0, 0, 2], desc = "ipv6-dst";
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
            next_header: uint(8) = 253, auto = next_protocol, desc = "ipv6-ext-next-header";
            hdr_ext_len: uint(8), auto = length(header, unit = words64, adjust = -1), desc = "ipv6-hdr-ext-len";
            options: bytes(from = hdr_ext_len, unit = 8, adjust = 6) = [1, 4, 0, 0, 0, 0], desc = "ipv6-options";
            padding: padding(8), desc = "ipv6-ext-padding";
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
            next_header: uint(8) = 253, auto = next_protocol, desc = "ipv6-ext-next-header";
            hdr_ext_len: uint(8), auto = length(header, unit = words64, adjust = -1), desc = "ipv6-hdr-ext-len";
            options: bytes(from = hdr_ext_len, unit = 8, adjust = 6) = [1, 4, 0, 0, 0, 0], desc = "ipv6-options";
            padding: padding(8), desc = "ipv6-ext-padding";
        },
    };
    d.next = Some(next_header_binding());
    d
}

/// Routing header (next header 43). Defaults to Type 2 (Mobile IPv6): one segment
/// left and `data` = 4 reserved octets + a 16-octet home address (all zero).
/// For Type 0 / 3 put the reserved word followed by the address list in `data`;
/// Type 4 has its own definition, [`ipv6_srh`].
#[must_use]
pub fn ipv6_route() -> ProtocolDef {
    let mut d = protocol! {
        id: "ipv6_route",
        name: "IPv6 Routing",
        layer: L3,
        fields: {
            next_header: uint(8) = 253, auto = next_protocol, desc = "ipv6-ext-next-header";
            hdr_ext_len: uint(8), auto = length(header, unit = words64, adjust = -1), desc = "ipv6-hdr-ext-len";
            routing_type: enum(8) {
                0 => source_route_deprecated,
                2 => mobile_ipv6,
                3 => rpl,
                4 => segment_routing,
            } = 2, desc = "ipv6-routing-type";
            segments_left: uint(8) = 1, desc = "ipv6-segments-left";
            data: bytes(from = hdr_ext_len, unit = 8, adjust = 4) = [0; 20], desc = "ipv6-routing-data";
            padding: padding(8), desc = "ipv6-ext-padding";
        },
        constraints: {
            warning: not_equals(routing_type, 0);
        },
    };
    d.next = Some(next_header_binding());
    d
}

/// SRH TLVs (RFC 8754 §2.1): 8-bit type and 8-bit length of the value. Pad1 (0,
/// one octet without length), `PadN` (4) and HMAC (5, §2.1.2: D flag, reserved, key
/// ID and the HMAC, which is Manual). The region ends at the header length
/// (`hdr_ext_len`); the header is then padded with Pad1 octets to a multiple of 8.
#[must_use]
pub fn srh_tlvs() -> FieldType {
    let u = |name: &str, bits: u8| FieldDef::new(name, FieldType::UInt { bits });
    let rest = |name: &str| FieldDef::new(name, FieldType::Bytes { len: Length::ToEnd });
    FieldType::Tlv(TlvDef {
        type_bits: 8,
        len_bits: 8,
        len_includes_header: false,
        len_unit: LengthUnit::Bytes,
        variants: vec![
            TlvVariant::type_only(0, "pad1"),
            TlvVariant::new(4, "padn", vec![rest("padding")]),
            TlvVariant::new(
                5,
                "hmac",
                vec![
                    u("d", 1).with_default(FieldValue::UInt(0)),
                    u("reserved", 15).with_default(FieldValue::UInt(0)),
                    u("hmac_key_id", 32).with_default(FieldValue::UInt(1)),
                    FieldDef::new("hmac", FieldType::Bytes { len: Length::ToEnd })
                        .with_default(FieldValue::Bytes(vec![0; 32])),
                ],
            ),
        ],
        unknown_as_raw: true,
        length: Length::UntilOffset {
            field: "hdr_ext_len".into(),
            unit: 8,
            adjust: 8,
        },
        fill: None,
        head_octets: 0,
        align: 0,
        unpadded_last: false,
        long_length: None,
        len_first: false,
        uncounted_octets: 0,
    })
}

/// `SRv6` Segment Routing Header (RFC 8754; Routing header type 4).
///
/// `segment_list` holds the SIDs in wire order: element 0 is the *last* segment
/// of the path and element `last_entry` the first, so the IPv6 destination of a
/// packet leaving the source is `segment_list[segments_left]`. Auto fields:
/// `hdr_ext_len`, `last_entry` (number of segments - 1), `next_header` and the
/// Pad1 padding to a multiple of 8 octets. While `segments_left` is non-zero the
/// upper-layer checksums (UDP / TCP / ICMPv6) use `segment_list[0]` as the
/// destination of the pseudo header (RFC 8200 §8.1). Optional TLVs: see
/// [`srh_tlvs`].
///
/// Default: two segments (`2001:db8:0:2::1` then `2001:db8:0:3::1`), one left.
#[must_use]
pub fn ipv6_srh() -> ProtocolDef {
    let seg = |a: [u16; 8]| FieldValue::Ipv6(Ipv6Addr::from(a));
    let segment_list = FieldType::List {
        item: ItemDef::Field {
            ty: Box::new(FieldType::Ipv6),
        },
        length: Length::FromField {
            field: "last_entry".into(),
            unit: 16,
            adjust: 16,
            div: 1,
        },
    };
    let mut d = protocol! {
        id: "ipv6_srh",
        name: "IPv6 Segment Routing Header",
        layer: L3,
        fields: {
            next_header: uint(8) = 253, auto = next_protocol, desc = "ipv6-ext-next-header";
            hdr_ext_len: uint(8), auto = length(header, unit = words64, adjust = -1), desc = "ipv6-hdr-ext-len";
            routing_type: enum(8) { 4 => segment_routing } = 4, desc = "ipv6-routing-type";
            segments_left: uint(8) = 1, desc = "srh-segments-left";
            last_entry: uint(8), auto = count(segment_list, adjust = -1), desc = "srh-last-entry";
            flags: uint(8) = 0, desc = "srh-flags";
            tag: uint(16) = 0, desc = "srh-tag";
            segment_list: ty(segment_list) = vec![
                seg([0x2001, 0xdb8, 0, 3, 0, 0, 0, 1]),
                seg([0x2001, 0xdb8, 0, 2, 0, 0, 0, 1]),
            ], desc = "srh-segment-list";
            tlvs: ty(srh_tlvs()), desc = "srh-tlvs";
            padding: padding(8), desc = "ipv6-ext-padding";
        },
        constraints: {
            warning: equals(routing_type, 4);
        },
    };
    d.next = Some(next_header_binding());
    d.final_destination = Some(FinalDestination {
        list: "segment_list".into(),
        when: "segments_left".into(),
    });
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
            next_header: uint(8) = 253, auto = next_protocol, desc = "ipv6-ext-next-header";
            reserved: uint(8) = 0, desc = "field-reserved";
            fragment_offset: uint(13) = 0, desc = "ipv6-fragment-offset";
            res: uint(2) = 0, desc = "field-reserved";
            m: uint(1) = 0, desc = "ipv6-more-fragments";
            identification: uint(32) = 0, desc = "ipv6-frag-identification";
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
            ipv6_srh(),
            ipv6_frag(),
        ] {
            d.validate().unwrap();
        }
        assert_eq!(ipv6().fixed_header_bits(), Some(320));
        assert_eq!(ipv6_frag().fixed_header_bits(), Some(64));
    }
}
