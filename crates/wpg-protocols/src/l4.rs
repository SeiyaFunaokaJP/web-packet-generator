//! Layer 4 protocol definitions.

use wpg_core::{
    protocol, FieldDef, FieldType, ItemDef, Length, LengthUnit, ProtocolDef, TlvDef, TlvVariant,
};

fn u(name: &str, bits: u8) -> FieldDef {
    FieldDef::new(name, FieldType::UInt { bits })
}

/// TCP options (RFC 9293 and friends) as a TLV sequence: EOL (0), NOP (1),
/// MSS (2, `value`), window scale (3, `shift`), SACK permitted (4), SACK (5,
/// `blocks`: list of `{left, right}`) and timestamps (8, `tsval`, `tsecr`).
/// Unknown kinds keep their value octets (`data`). The option length counts the
/// kind and length octets. The region is `data_offset * 4 - 20` octets.
#[must_use]
pub fn tcp_options() -> FieldType {
    FieldType::Tlv(TlvDef {
        type_bits: 8,
        len_bits: 8,
        len_includes_header: true,
        len_unit: LengthUnit::Bytes,
        variants: vec![
            TlvVariant::type_only(0, "eol"),
            TlvVariant::type_only(1, "nop"),
            TlvVariant::new(2, "mss", vec![u("value", 16)]),
            TlvVariant::new(3, "ws", vec![u("shift", 8)]),
            TlvVariant::new(4, "sack_permitted", Vec::new()),
            TlvVariant::new(
                5,
                "sack",
                vec![FieldDef::new(
                    "blocks",
                    FieldType::List {
                        item: ItemDef::Struct {
                            fields: vec![u("left", 32), u("right", 32)],
                        },
                        length: Length::ToEnd,
                    },
                )],
            ),
            TlvVariant::new(8, "timestamps", vec![u("tsval", 32), u("tsecr", 32)]),
        ],
        unknown_as_raw: true,
        length: Length::FromField {
            field: "data_offset".into(),
            unit: 4,
            adjust: -20,
        },
    })
}

/// UDP (RFC 768): `length` and `checksum` (with IPv4/IPv6 pseudo header) are Auto.
/// A computed checksum of zero is sent as `0xffff`.
///
/// The `next` binding demultiplexes well-known tunnelling ports on the
/// **destination** port (VXLAN 4789, L2TP 1701, GTP-U 2152, MPLS-over-UDP 6635,
/// Geneve 6081) and the first-hop redundancy ports (HSRP 1985, `HSRPv2` over IPv6
/// 2029, GLBP 3222; UDP 1985 decodes as `HSRPv1`); unknown ports have no default, so
/// an ordinary UDP payload still parses as `raw`.
#[must_use]
pub fn udp() -> ProtocolDef {
    protocol! {
        id: "udp",
        name: "UDP",
        layer: L4,
        fields: {
            src_port: uint(16) = 49152;
            dst_port: uint(16) = 9;
            length: uint(16), auto = length(total);
            checksum: uint(16), auto = checksum(internet_pseudo, header_payload, nonzero);
        },
        next: dst_port {
            1701 => "l2tpv2",
            1985 => "hsrp",
            1985 => "hsrpv2",
            2029 => "hsrpv2",
            2152 => "gtpu",
            3222 => "glbp",
            4789 => "vxlan",
            6081 => "geneve",
            6635 => "mpls",
        },
        constraints: {
            warning: min_length(total, 8);
        },
    }
}

/// TCP (RFC 9293) with typed options ([`tcp_options`]). `data_offset` (including
/// options and automatic zero padding to 32 bits) and `checksum` (with pseudo
/// header) are Auto.
#[must_use]
pub fn tcp() -> ProtocolDef {
    protocol! {
        id: "tcp",
        name: "TCP",
        layer: L4,
        fields: {
            src_port: uint(16) = 49152;
            dst_port: uint(16) = 80;
            seq: uint(32) = 0;
            ack: uint(32) = 0;
            data_offset: uint(4), auto = length(header, unit = words32);
            reserved: uint(4) = 0;
            flags: flags(8) {
                7 => cwr,
                6 => ece,
                5 => urg,
                4 => ack,
                3 => psh,
                2 => rst,
                1 => syn,
                0 => fin,
            } = 0x02;
            window: uint(16) = 65535;
            checksum: uint(16), auto = checksum(internet_pseudo, header_payload);
            urgent_ptr: uint(16) = 0;
            options: ty(tcp_options());
            padding: padding(4);
        },
        constraints: {
            error: in_range(data_offset, 5, 15);
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l4_defs_are_valid() {
        udp().validate().unwrap();
        tcp().validate().unwrap();
        assert_eq!(udp().fixed_header_bits(), Some(64));
    }
}
