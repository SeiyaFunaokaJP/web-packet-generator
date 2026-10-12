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
            div: 1,
        },
        fill: None,
    })
}

/// UDP (RFC 768): `length` and `checksum` (with IPv4/IPv6 pseudo header) are Auto.
/// A computed checksum of zero is sent as `0xffff`.
///
/// The `next` binding demultiplexes on the **destination** port, falling back to
/// the **source** port when the destination is not a known one (so replies from
/// a well-known port decode too): tunnelling ports (VXLAN 4789, L2TP 1701, GTP-U 2152, MPLS-over-UDP 6635,
/// Geneve 6081), routing protocols (RIP 520, `RIPng` 521), the first-hop
/// redundancy ports (HSRP 1985, `HSRPv2` over IPv6 2029, GLBP 3222; UDP 1985
/// decodes as `HSRPv1`) and the P1 application protocols (DNS 53, `DHCPv4` 67/68,
/// TFTP 69, NTP 123, SNMP 161/162, Syslog 514, `DHCPv6` 546/547, RADIUS
/// 1812/1813); unknown ports have no default, so an ordinary UDP payload still
/// parses as `raw`. Building announces through the destination port only.
#[must_use]
pub fn udp() -> ProtocolDef {
    let mut d = protocol! {
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
            520 => "rip",
            521 => "ripng",
            1701 => "l2tpv2",
            1985 => "hsrp",
            1985 => "hsrpv2",
            2029 => "hsrpv2",
            2152 => "gtpu",
            3222 => "glbp",
            4789 => "vxlan",
            6081 => "geneve",
            6635 => "mpls",
            // P1 application protocols.
            53 => "dns",
            67 => "dhcpv4",
            68 => "dhcpv4",
            69 => "tftp",
            123 => "ntp",
            161 => "snmp",
            162 => "snmp",
            514 => "syslog",
            546 => "dhcpv6",
            547 => "dhcpv6",
            1812 => "radius",
            1813 => "radius",
        },
        constraints: {
            warning: min_length(total, 8);
        },
    };
    if let Some(b) = d.next.as_mut() {
        b.alt_field = Some("src_port".into());
    }
    d
}

/// TCP (RFC 9293) with typed options ([`tcp_options`]). `data_offset` (including
/// options and automatic zero padding to 32 bits) and `checksum` (with pseudo
/// header) are Auto.
///
/// The `next` binding demultiplexes BGP on port 179 (destination, else source);
/// other payloads parse as `raw`.
#[must_use]
pub fn tcp() -> ProtocolDef {
    let mut d = protocol! {
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
        next: dst_port { 179 => "bgp" },
        constraints: {
            error: in_range(data_offset, 5, 15);
        },
    };
    if let Some(b) = d.next.as_mut() {
        b.alt_field = Some("src_port".into());
    }
    d
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
