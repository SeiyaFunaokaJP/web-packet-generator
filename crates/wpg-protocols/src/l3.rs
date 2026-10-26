//! Layer 3 protocol definitions.

use wpg_core::{protocol, FieldDef, FieldType, Length, ProtocolDef, TlvDef, TlvVariant};

/// IPv4 options (RFC 791): EOL (0), NOP (1) and Router Alert (148, RFC 2113) are
/// typed; other options keep their value octets (`data`). The option length counts
/// the type and length octets. The region is `ihl * 4 - 20` octets.
#[must_use]
pub fn ipv4_options() -> FieldType {
    FieldType::Tlv(TlvDef {
        type_bits: 8,
        len_bits: 8,
        len_includes_header: true,
        len_unit: wpg_core::LengthUnit::Bytes,
        variants: vec![
            TlvVariant::type_only(0, "eol"),
            TlvVariant::type_only(1, "nop"),
            TlvVariant::new(
                148,
                "router_alert",
                vec![FieldDef::new("value", FieldType::UInt { bits: 16 })],
            ),
        ],
        unknown_as_raw: true,
        length: Length::FromField {
            field: "ihl".into(),
            unit: 4,
            adjust: -20,
            div: 1,
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

/// IPv4 (RFC 791) with typed options ([`ipv4_options`]).
///
/// Auto fields: `ihl` (header length in 32-bit words, including options and the
/// automatic zero padding to a 4-octet boundary), `total_length`, `protocol`
/// (from the next layer) and `header_checksum`. Supplies the pseudo header for
/// TCP / UDP checksums.
#[must_use]
pub fn ipv4() -> ProtocolDef {
    protocol! {
        id: "ipv4",
        name: "IPv4",
        layer: L3,
        fields: {
            version: uint(4) = 4, desc = "ipv4-version";
            ihl: uint(4), auto = length(header, unit = words32), desc = "ipv4-ihl";
            dscp: uint(6) = 0, desc = "ip-dscp";
            ecn: uint(2) = 0, desc = "ip-ecn";
            total_length: uint(16), auto = length(total), desc = "ipv4-total-length";
            identification: uint(16) = 0, desc = "ipv4-identification";
            flags: flags(3) { 2 => reserved, 1 => df, 0 => mf } = 0, desc = "ipv4-flags";
            frag_offset: uint(13) = 0, desc = "ipv4-frag-offset";
            ttl: uint(8) = 64, desc = "ipv4-ttl";
            protocol: enum(8) {
                1 => icmp,
                2 => igmp,
                4 => ipip,
                6 => tcp,
                17 => udp,
                41 => ipv6,
                46 => rsvp,
                47 => gre,
                50 => esp,
                51 => ah,
                58 => ipv6_icmp,
                88 => eigrp,
                89 => ospf,
                103 => pim,
                112 => vrrp,
                115 => l2tp,
                132 => sctp,
                253 => experimental,
            } = 253, auto = next_protocol, desc = "ipv4-protocol";
            header_checksum: uint(16), auto = checksum(internet, header), desc = "ipv4-header-checksum";
            src: ipv4 = [192, 0, 2, 1], desc = "ipv4-src";
            dst: ipv4 = [192, 0, 2, 2], desc = "ipv4-dst";
            options: ty(ipv4_options()), desc = "ipv4-options";
            padding: padding(4), desc = "ipv4-padding";
        },
        next: protocol {
            1 => "icmpv4",
            2 => "igmp",
            4 => "ipv4",
            6 => "tcp",
            17 => "udp",
            41 => "ipv6",
            46 => "rsvp",
            47 => "gre",
            88 => "eigrp",
            89 => "ospfv2",
            103 => "pim",
            103 => "pim_register",
            112 => "vrrpv3",
            112 => "vrrpv2",
            115 => "l2tpv3",
            132 => "sctp",
        },
        pseudo_header: (src, dst),
        constraints: {
            warning: equals(version, 4);
            error: in_range(ihl, 5, 15);
            warning: not_equals(ttl, 0);
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv4_is_valid() {
        let d = ipv4();
        d.validate().unwrap();
        let fixed: u32 = d.fields.iter().filter_map(|f| f.ty.fixed_bits()).sum();
        assert_eq!(fixed, 160);
    }
}
