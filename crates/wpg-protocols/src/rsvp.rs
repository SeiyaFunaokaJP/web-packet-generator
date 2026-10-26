//! RSVP (RFC 2205) with the RSVP-TE objects (RFC 3209) (spec §2.1 P2, M7).
//!
//! One [`rsvp`] layer holds the common header and the objects of the message.
//! An object is a 16-bit length (counting the 4-octet header, a multiple of 4),
//! then the Class-Num and C-Type, read together as one 16-bit type
//! ([`wpg_core::TlvDef::len_first`]): `0x0107` is `SESSION` / `LSP_TUNNEL_IPv4`.
//! Object variants are named after the class and the C-Type
//! (`session_lsp_tunnel_ipv4`, `rsvp_hop_ipv4`, ...).
//!
//! Reached through IPv4 protocol / IPv6 next header 46. Path, `PathTear` and
//! `ResvConf` are sent with the IP Router Alert option (RFC 2113 / RFC 2711).
//!
//! Auto fields: `checksum` (Internet checksum of the message), `length`, every
//! object length, the name length and padding of `SESSION_ATTRIBUTE`, and the
//! lengths of the `EXPLICIT_ROUTE` / `RECORD_ROUTE` subobjects. The traffic
//! parameters of `SENDER_TSPEC` / `FLOWSPEC` (`IntServ`, RFC 2210) are IEEE 754
//! single-precision values given as their 32-bit patterns; `ADSPEC`, `POLICY_DATA`
//! and INTEGRITY keep their value octets.

use wpg_core::{
    protocol, AutoCompute, EnumVariant, FieldDef, FieldType, FieldValue, ItemDef, Length,
    LengthUnit, ProtocolDef, TlvDef, TlvItem, TlvVariant,
};

/// Id of the RSVP message ([`rsvp`]).
pub const RSVP: &str = "rsvp";

fn u(name: &str, bits: u8) -> FieldDef {
    FieldDef::new(name, FieldType::UInt { bits })
}

fn uint(name: &str, bits: u8, default: u64) -> FieldDef {
    u(name, bits).with_default(FieldValue::UInt(default))
}

fn ipv4(name: &str, default: [u8; 4]) -> FieldDef {
    FieldDef::new(name, FieldType::Ipv4).with_default(FieldValue::Ipv4(default.into()))
}

fn ipv6(name: &str) -> FieldDef {
    FieldDef::new(name, FieldType::Ipv6)
}

fn enum_field(name: &str, bits: u8, list: &[(u64, &str)], default: u64) -> FieldDef {
    FieldDef::new(
        name,
        FieldType::Enum {
            bits,
            variants: list
                .iter()
                .map(|(v, n)| EnumVariant {
                    value: *v,
                    name: (*n).into(),
                    description: None,
                })
                .collect(),
        },
    )
    .with_default(FieldValue::UInt(default))
}

fn tlv_def(type_bits: u8, len_bits: u8, variants: Vec<TlvVariant>, length: Length) -> TlvDef {
    TlvDef {
        type_bits,
        len_bits,
        len_includes_header: true,
        len_unit: LengthUnit::Bytes,
        variants,
        unknown_as_raw: true,
        length,
        fill: None,
        head_octets: 0,
        align: 0,
        unpadded_last: false,
        long_length: None,
        len_first: false,
        uncounted_octets: 0,
    }
}

/// `EXPLICIT_ROUTE` subobjects (RFC 3209 §4.3.3, RFC 3477): the L (loose) bit is the
/// top bit of the 8-bit type, so `ipv4_prefix` is strict and `ipv4_prefix_loose`
/// loose. 8-bit length including the type and length octets.
fn ero_subobjects() -> FieldType {
    let both = |code: u64, name: &str, fields: Vec<FieldDef>| {
        [
            TlvVariant::new(code, name, fields.clone()),
            TlvVariant::new(code | 0x80, format!("{name}_loose"), fields),
        ]
    };
    let mut v = Vec::new();
    v.extend(both(
        1,
        "ipv4_prefix",
        vec![
            ipv4("address", [192, 0, 2, 2]),
            uint("prefix_length", 8, 32),
            uint("reserved", 8, 0),
        ],
    ));
    v.extend(both(
        2,
        "ipv6_prefix",
        vec![
            ipv6("address"),
            uint("prefix_length", 8, 128),
            uint("reserved", 8, 0),
        ],
    ));
    v.extend(both(
        4,
        "unnumbered_interface",
        vec![
            uint("reserved", 16, 0),
            ipv4("router_id", [192, 0, 2, 2]),
            u("interface_id", 32),
        ],
    ));
    v.extend(both(32, "as_number", vec![u("as_number", 16)]));
    FieldType::Tlv(tlv_def(8, 8, v, Length::ToEnd))
}

/// `RECORD_ROUTE` subobjects (RFC 3209 §4.4.1, RFC 3477): IPv4 / IPv6 address,
/// unnumbered interface and label, each with its flags (local protection
/// available / in use, ...). 8-bit type, 8-bit length including the header.
fn rro_subobjects() -> FieldType {
    let v = vec![
        TlvVariant::new(
            1,
            "ipv4_address",
            vec![
                ipv4("address", [192, 0, 2, 1]),
                uint("prefix_length", 8, 32),
                uint("flags", 8, 0),
            ],
        ),
        TlvVariant::new(
            2,
            "ipv6_address",
            vec![
                ipv6("address"),
                uint("prefix_length", 8, 128),
                uint("flags", 8, 0),
            ],
        ),
        TlvVariant::new(
            3,
            "label",
            vec![uint("flags", 8, 1), uint("c_type", 8, 1), u("label", 32)],
        ),
        TlvVariant::new(
            4,
            "unnumbered_interface",
            vec![
                uint("flags", 8, 0),
                uint("reserved", 8, 0),
                ipv4("router_id", [192, 0, 2, 1]),
                u("interface_id", 32),
            ],
        ),
    ];
    FieldType::Tlv(tlv_def(8, 8, v, Length::ToEnd))
}

/// `IntServ` traffic parameters of `SENDER_TSPEC` (service 1) and `FLOWSPEC`
/// (controlled-load, service 5) (RFC 2210 §3): message header, service header,
/// the token bucket parameter (rate, bucket size and peak rate as IEEE 754
/// single-precision bit patterns, policed unit and maximum packet size) and any
/// further parameters as octets (the guaranteed service rate / slack term).
///
/// Default token bucket: 125000 octets/s (1 Mbit/s), 1000 octets, infinite peak
/// rate, minimum policed unit 0, maximum packet size 1500.
fn intserv(service: u64) -> Vec<FieldDef> {
    vec![
        uint("version", 4, 0),
        uint("reserved", 12, 0),
        uint("overall_length", 16, 7),
        uint("service_number", 8, service),
        uint("service_reserved", 8, 0),
        uint("service_length", 16, 6),
        uint("parameter_id", 8, 127),
        uint("parameter_flags", 8, 0),
        uint("parameter_length", 16, 5),
        uint("token_bucket_rate", 32, 0x47f4_2400),
        uint("token_bucket_size", 32, 0x447a_0000),
        uint("peak_data_rate", 32, 0x7f80_0000),
        uint("minimum_policed_unit", 32, 0),
        uint("maximum_packet_size", 32, 1500),
        FieldDef::new("parameters", FieldType::Bytes { len: Length::ToEnd }),
    ]
}

/// `SESSION_ATTRIBUTE` fields after the optional resource affinities (RFC 3209
/// §4.7): priorities, flags, the session name and its length (Auto) and zero
/// padding to 4 octets (Auto).
fn session_attribute(mut fields: Vec<FieldDef>) -> Vec<FieldDef> {
    fields.extend([
        uint("setup_priority", 8, 7),
        uint("holding_priority", 8, 7),
        FieldDef::new(
            "flags",
            FieldType::Flags {
                bits: 8,
                flags: [
                    (0, "local_protection_desired"),
                    (1, "label_recording_desired"),
                    (2, "se_style_desired"),
                    (3, "bandwidth_protection_desired"),
                    (4, "node_protection_desired"),
                ]
                .iter()
                .map(|(bit, n)| wpg_core::FlagBit {
                    bit: *bit,
                    name: (*n).into(),
                })
                .collect(),
            },
        )
        .with_default(FieldValue::UInt(0x04)),
        u("name_length", 8).with_auto(AutoCompute::Size {
            field: "name".into(),
        }),
        FieldDef::new(
            "name",
            FieldType::Bytes {
                len: Length::FromField {
                    field: "name_length".into(),
                    unit: 1,
                    adjust: 0,
                    div: 1,
                },
            },
        )
        .with_default(FieldValue::Bytes(b"tunnel1".to_vec())),
        FieldDef::new("padding", FieldType::Padding { align: 4 }),
    ]);
    fields
}

/// RSVP objects (RFC 2205 appendix A, RFC 3209 §4, RFC 2961 §4, RFC 3473 not
/// included). The type is `class_num << 8 | c_type`.
#[must_use]
pub fn objects() -> FieldType {
    let filter_v4 = |code, name| {
        TlvVariant::new(
            code,
            name,
            vec![
                ipv4("source_address", [192, 0, 2, 1]),
                uint("reserved", 16, 0),
                u("source_port", 16),
            ],
        )
    };
    let lsp_v4 = |code, name| {
        TlvVariant::new(
            code,
            name,
            vec![
                ipv4("sender_address", [192, 0, 2, 1]),
                uint("reserved", 16, 0),
                uint("lsp_id", 16, 1),
            ],
        )
    };
    let lsp_v6 = |code, name| {
        TlvVariant::new(
            code,
            name,
            vec![
                ipv6("sender_address"),
                uint("reserved", 16, 0),
                uint("lsp_id", 16, 1),
            ],
        )
    };
    let message_id = |code, name| {
        TlvVariant::new(
            code,
            name,
            vec![uint("flags", 8, 0), u("epoch", 24), u("message_id", 32)],
        )
    };
    let hello = |code, name| {
        TlvVariant::new(
            code,
            name,
            vec![u("source_instance", 32), u("destination_instance", 32)],
        )
    };
    let ip_session = |code, name, addr: FieldDef| {
        TlvVariant::new(
            code,
            name,
            vec![
                addr,
                uint("protocol_id", 8, 17),
                uint("flags", 8, 0),
                u("destination_port", 16),
            ],
        )
    };
    let error_spec = |code, name, addr: FieldDef| {
        TlvVariant::new(
            code,
            name,
            vec![
                addr,
                uint("flags", 8, 0),
                enum_field(
                    "error_code",
                    8,
                    &[
                        (0, "confirmation"),
                        (1, "admission_control_failure"),
                        (2, "policy_control_failure"),
                        (3, "no_path_information"),
                        (4, "no_sender_information"),
                        (5, "conflicting_reservation_style"),
                        (6, "unknown_reservation_style"),
                        (7, "conflicting_destination_ports"),
                        (8, "conflicting_sender_ports"),
                        (12, "service_preempted"),
                        (13, "unknown_object_class"),
                        (14, "unknown_object_c_type"),
                        (20, "api_error"),
                        (21, "traffic_control_error"),
                        (22, "traffic_control_system_error"),
                        (23, "rsvp_system_error"),
                        (24, "routing_problem"),
                        (25, "notify"),
                    ],
                    0,
                ),
                u("error_value", 16),
            ],
        )
    };
    let hop = |code, name, addr: FieldDef| {
        TlvVariant::new(code, name, vec![addr, u("logical_interface_handle", 32)])
    };
    let route = |code, name, ty: FieldType| {
        TlvVariant::new(code, name, vec![FieldDef::new("subobjects", ty)])
    };
    let v = vec![
        ip_session(0x0101, "session_ipv4", ipv4("destination", [192, 0, 2, 2])),
        ip_session(0x0102, "session_ipv6", ipv6("destination")),
        TlvVariant::new(
            0x0107,
            "session_lsp_tunnel_ipv4",
            vec![
                ipv4("tunnel_endpoint", [192, 0, 2, 2]),
                uint("reserved", 16, 0),
                uint("tunnel_id", 16, 1),
                ipv4("extended_tunnel_id", [192, 0, 2, 1]),
            ],
        ),
        TlvVariant::new(
            0x0108,
            "session_lsp_tunnel_ipv6",
            vec![
                ipv6("tunnel_endpoint"),
                uint("reserved", 16, 0),
                uint("tunnel_id", 16, 1),
                ipv6("extended_tunnel_id"),
            ],
        ),
        hop(0x0301, "rsvp_hop_ipv4", ipv4("address", [192, 0, 2, 1])),
        hop(0x0302, "rsvp_hop_ipv6", ipv6("address")),
        TlvVariant::new(
            0x0501,
            "time_values",
            vec![uint("refresh_period", 32, 30_000)],
        ),
        error_spec(
            0x0601,
            "error_spec_ipv4",
            ipv4("node_address", [192, 0, 2, 2]),
        ),
        error_spec(0x0602, "error_spec_ipv6", ipv6("node_address")),
        TlvVariant::new(
            0x0701,
            "scope_ipv4",
            vec![FieldDef::new(
                "addresses",
                FieldType::List {
                    item: ItemDef::Field {
                        ty: Box::new(FieldType::Ipv4),
                    },
                    length: Length::ToEnd,
                },
            )],
        ),
        TlvVariant::new(
            0x0801,
            "style",
            vec![
                uint("flags", 8, 0),
                uint("reserved", 19, 0),
                enum_field("sharing", 2, &[(1, "distinct"), (2, "shared")], 2),
                enum_field(
                    "sender_selection",
                    3,
                    &[(1, "wildcard"), (2, "explicit")],
                    2,
                ),
            ],
        ),
        TlvVariant::new(0x0902, "flowspec", intserv(5)),
        filter_v4(0x0a01, "filter_spec_ipv4"),
        lsp_v4(0x0a07, "filter_spec_lsp_tunnel_ipv4"),
        lsp_v6(0x0a08, "filter_spec_lsp_tunnel_ipv6"),
        filter_v4(0x0b01, "sender_template_ipv4"),
        lsp_v4(0x0b07, "sender_template_lsp_tunnel_ipv4"),
        lsp_v6(0x0b08, "sender_template_lsp_tunnel_ipv6"),
        TlvVariant::new(0x0c02, "sender_tspec", intserv(1)),
        TlvVariant::new(
            0x0f01,
            "resv_confirm_ipv4",
            vec![ipv4("receiver_address", [192, 0, 2, 2])],
        ),
        TlvVariant::new(0x1001, "label", vec![uint("label", 32, 16)]),
        TlvVariant::new(
            0x1301,
            "label_request",
            vec![uint("reserved", 16, 0), uint("l3pid", 16, 0x0800)],
        ),
        route(0x1401, "explicit_route", ero_subobjects()),
        route(0x1501, "record_route", rro_subobjects()),
        hello(0x1601, "hello_request"),
        hello(0x1602, "hello_ack"),
        message_id(0x1701, "message_id"),
        message_id(0x1801, "message_id_ack"),
        message_id(0x1802, "message_id_nack"),
        TlvVariant::new(
            0xcf01,
            "session_attribute_ra",
            session_attribute(vec![
                u("exclude_any", 32),
                u("include_any", 32),
                u("include_all", 32),
            ]),
        ),
        TlvVariant::new(0xcf07, "session_attribute", session_attribute(Vec::new())),
    ];
    let mut t = tlv_def(
        16,
        16,
        v,
        Length::FromField {
            field: "length".into(),
            unit: 1,
            adjust: -8,
            div: 1,
        },
    );
    t.len_first = true;
    FieldType::Tlv(t)
}

/// RSVP message (RFC 2205 §3.1.1): version, flags, message type, checksum
/// (Auto), send TTL, length (Auto) and the objects ([`objects`]).
///
/// Default: a Path message for an RSVP-TE LSP tunnel (RFC 3209) from 192.0.2.1 to
/// 192.0.2.2: `SESSION`, `RSVP_HOP`, `TIME_VALUES`, `LABEL_REQUEST`, `SESSION_ATTRIBUTE`,
/// `SENDER_TEMPLATE` and `SENDER_TSPEC`.
#[must_use]
pub fn rsvp() -> ProtocolDef {
    let path = vec![
        TlvItem::named("session_lsp_tunnel_ipv4"),
        TlvItem::named("rsvp_hop_ipv4"),
        TlvItem::named("time_values"),
        TlvItem::named("label_request"),
        TlvItem::named("session_attribute"),
        TlvItem::named("sender_template_lsp_tunnel_ipv4"),
        TlvItem::named("sender_tspec"),
    ];
    protocol! {
        id: "rsvp",
        name: "RSVP",
        layer: L4,
        fields: {
            version: uint(4) = 1, desc = "rsvp-version";
            flags: flags(4) { 0 => refresh_reduction_capable } = 0, desc = "rsvp-flags";
            msg_type: enum(8) {
                1 => path,
                2 => resv,
                3 => path_err,
                4 => resv_err,
                5 => path_tear,
                6 => resv_tear,
                7 => resv_conf,
                12 => bundle,
                13 => ack,
                15 => srefresh,
                20 => hello,
            } = 1, desc = "rsvp-msg-type";
            checksum: uint(16), auto = checksum(internet, header), desc = "rsvp-checksum";
            send_ttl: uint(8) = 64, desc = "rsvp-send-ttl";
            reserved: uint(8) = 0, desc = "rsvp-reserved";
            length: uint(16), auto = length(header), desc = "rsvp-length";
            objects: ty(objects()) = FieldValue::from(path), desc = "rsvp-objects";
        },
        constraints: {
            warning: equals(version, 1);
            error: min_length(header, 8);
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rsvp_is_valid() {
        let d = rsvp();
        d.validate().unwrap();
        assert_eq!(d.id, RSVP);
    }
}
