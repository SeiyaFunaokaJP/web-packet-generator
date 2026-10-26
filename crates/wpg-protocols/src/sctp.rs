//! SCTP (RFC 9260) with its CRC-32C checksum (spec §2.1 P2, M7).
//!
//! One [`sctp`] layer holds the common header and every chunk of the packet.
//! Chunks are TLV items (8-bit type, 8-bit flags, 16-bit length counting the
//! 4-octet chunk header but not the padding to a multiple of 4 octets); the flags
//! sit between the type and the length ([`wpg_core::TlvDef::head_octets`]) and
//! each item is padded ([`wpg_core::TlvDef::align`]). The variable parameters of
//! INIT / INIT ACK / HEARTBEAT / ASCONF / RE-CONFIG and the error causes of ABORT /
//! ERROR are TLVs of the same shape without flags (16-bit type, 16-bit length).
//!
//! Auto fields:
//!
//! * `checksum`: CRC-32C of the whole SCTP packet with the checksum field zeroed
//!   (RFC 9260 appendix A), sent least significant octet first as the RFC does.
//! * Every chunk / parameter / error cause length, and its zero padding.
//! * The SACK gap block / duplicate TSN counts and the number of missing
//!   parameters of a "missing mandatory parameter" cause.
//!
//! The user data of DATA / I-DATA chunks is octets inside the chunk (several
//! chunks can be bundled), so it is not demultiplexed on the payload protocol
//! identifier. The padding of the last parameter of a chunk is counted in the
//! chunk length (RFC 9260); a received chunk whose length leaves it out decodes
//! with its parameters as octets.

use wpg_core::{
    protocol, AutoCompute, EnumVariant, FieldDef, FieldType, FieldValue, FlagBit, ItemDef, Length,
    LengthUnit, ProtocolDef, TlvDef, TlvItem, TlvVariant,
};

/// Id of the SCTP packet ([`sctp`]).
pub const SCTP: &str = "sctp";

/// UDP port of SCTP over UDP (RFC 6951).
pub const SCTP_UDP_PORT: u64 = 9899;

fn u(name: &str, bits: u8) -> FieldDef {
    FieldDef::new(name, FieldType::UInt { bits })
}

fn uint(name: &str, bits: u8, default: u64) -> FieldDef {
    u(name, bits).with_default(FieldValue::UInt(default))
}

fn to_end(name: &str) -> FieldDef {
    FieldDef::new(name, FieldType::Bytes { len: Length::ToEnd })
}

fn enum_field(name: &str, bits: u8, list: &[(u64, &str)]) -> FieldDef {
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
}

/// Chunk flags without named bits.
fn no_flags() -> FieldDef {
    uint("flags", 8, 0)
}

/// Chunk flags with named bits.
fn flags(bits: &[(u8, &str)], default: u64) -> FieldDef {
    FieldDef::new(
        "flags",
        FieldType::Flags {
            bits: 8,
            flags: bits
                .iter()
                .map(|(bit, n)| FlagBit {
                    bit: *bit,
                    name: (*n).into(),
                })
                .collect(),
        },
    )
    .with_default(FieldValue::UInt(default))
}

/// DATA / I-DATA flags (RFC 9260 §3.3.1, RFC 7053, RFC 8260): I (immediate),
/// U (unordered), B (beginning), E (ending). Default: an unfragmented message.
fn data_flags() -> FieldDef {
    flags(&[(3, "i"), (2, "u"), (1, "b"), (0, "e")], 0x03)
}

/// ABORT / SHUTDOWN COMPLETE flags: T (the verification tag is reflected).
fn t_flag() -> FieldDef {
    flags(&[(0, "t")], 0)
}

/// A list of `bits`-wide integers.
fn uint_list(name: &str, bits: u8, length: Length) -> FieldDef {
    FieldDef::new(
        name,
        FieldType::List {
            item: ItemDef::Field {
                ty: Box::new(FieldType::UInt { bits }),
            },
            length,
        },
    )
}

/// TLVs of RFC 9260 shape: type, optional head (`head_octets`), 16-bit length
/// counting the header but not the padding to 4 octets, up to the end of the
/// enclosing region. Chunks (with a head) are all padded; the last parameter /
/// error cause of a chunk is not, the chunk padding covers it (RFC 9260 §3.2).
fn tlv(type_bits: u8, head_octets: u8, variants: Vec<TlvVariant>) -> FieldType {
    FieldType::Tlv(TlvDef {
        type_bits,
        len_bits: 16,
        len_includes_header: true,
        len_unit: LengthUnit::Bytes,
        variants,
        unknown_as_raw: true,
        length: Length::ToEnd,
        fill: None,
        head_octets,
        align: 4,
        unpadded_last: head_octets == 0,
        long_length: None,
        len_first: false,
        uncounted_octets: 0,
    })
}

/// IPv4 / IPv6 address parameters (also nested in ASCONF parameters).
fn address_params() -> Vec<TlvVariant> {
    vec![
        TlvVariant::new(
            5,
            "ipv4_address",
            vec![FieldDef::new("address", FieldType::Ipv4)],
        ),
        TlvVariant::new(
            6,
            "ipv6_address",
            vec![FieldDef::new("address", FieldType::Ipv6)],
        ),
    ]
}

/// Error causes (RFC 9260 §3.3.10, RFC 5061, RFC 4895). The parameters carried by
/// an "unrecognized parameters" cause are octets.
#[must_use]
pub fn error_causes() -> FieldType {
    let tsn = |code, name| TlvVariant::new(code, name, vec![u("tsn", 32)]);
    let octets = |code, name, field| TlvVariant::new(code, name, vec![to_end(field)]);
    tlv(
        16,
        0,
        vec![
            TlvVariant::new(
                1,
                "invalid_stream_identifier",
                vec![u("stream_identifier", 16), uint("reserved", 16, 0)],
            ),
            TlvVariant::new(
                2,
                "missing_mandatory_parameter",
                vec![
                    u("number_of_missing_parameters", 32).with_auto(AutoCompute::Count {
                        field: "missing_parameters".into(),
                        adjust: 0,
                    }),
                    uint_list(
                        "missing_parameters",
                        16,
                        Length::Count {
                            field: "number_of_missing_parameters".into(),
                        },
                    ),
                ],
            ),
            TlvVariant::new(3, "stale_cookie", vec![u("staleness_us", 32)]),
            TlvVariant::new(4, "out_of_resource", Vec::new()),
            octets(5, "unresolvable_address", "address"),
            octets(6, "unrecognized_chunk_type", "chunk"),
            TlvVariant::new(7, "invalid_mandatory_parameter", Vec::new()),
            octets(8, "unrecognized_parameters", "parameters"),
            tsn(9, "no_user_data"),
            TlvVariant::new(10, "cookie_received_while_shutting_down", Vec::new()),
            octets(11, "restart_with_new_addresses", "addresses"),
            octets(12, "user_initiated_abort", "reason"),
            octets(13, "protocol_violation", "information"),
            TlvVariant::new(0x00a0, "request_to_delete_last_address", Vec::new()),
            TlvVariant::new(0x00a1, "operation_refused", Vec::new()),
            TlvVariant::new(0x00a2, "request_to_delete_source_address", Vec::new()),
            TlvVariant::new(0x00a3, "illegal_asconf_ack", Vec::new()),
            TlvVariant::new(0x00a4, "request_refused", Vec::new()),
            TlvVariant::new(
                0x0105,
                "unsupported_hmac_identifier",
                vec![u("hmac_identifier", 16)],
            ),
        ],
    )
}

/// Chunk parameters (RFC 9260 §3.3.2-3.3.6, RFC 3758, RFC 4895, RFC 5061,
/// RFC 6525): addresses, cookies, supported address types / extensions, the
/// authentication parameters, the dynamic address reconfiguration (ASCONF) and
/// stream reconfiguration (RE-CONFIG) requests.
#[must_use]
pub fn parameters() -> FieldType {
    let correlated = |code, name, rest: Vec<FieldDef>| {
        let mut fields = vec![u("correlation_id", 32)];
        fields.extend(rest);
        TlvVariant::new(code, name, fields)
    };
    let address = || FieldDef::new("address", tlv(16, 0, address_params()));
    let request_seq = || u("request_sequence", 32);
    let streams = || uint_list("streams", 16, Length::ToEnd);
    let mut v = address_params();
    v.extend([
        TlvVariant::new(1, "heartbeat_info", vec![to_end("info")]),
        TlvVariant::new(7, "state_cookie", vec![to_end("cookie")]),
        TlvVariant::new(8, "unrecognized_parameter", vec![to_end("parameter")]),
        TlvVariant::new(9, "cookie_preservative", vec![u("increment_ms", 32)]),
        TlvVariant::new(11, "host_name_address", vec![to_end("host_name")]),
        TlvVariant::new(
            12,
            "supported_address_types",
            vec![FieldDef::new(
                "types",
                FieldType::List {
                    item: ItemDef::Field {
                        ty: Box::new(
                            enum_field("type", 16, &[(5, "ipv4"), (6, "ipv6"), (11, "host_name")])
                                .ty,
                        ),
                    },
                    length: Length::ToEnd,
                },
            )],
        ),
        TlvVariant::new(
            13,
            "outgoing_ssn_reset_request",
            vec![
                request_seq(),
                u("response_sequence", 32),
                u("last_tsn", 32),
                streams(),
            ],
        ),
        TlvVariant::new(
            14,
            "incoming_ssn_reset_request",
            vec![request_seq(), streams()],
        ),
        TlvVariant::new(15, "ssn_tsn_reset_request", vec![request_seq()]),
        TlvVariant::new(
            16,
            "reconfiguration_response",
            vec![
                u("response_sequence", 32),
                enum_field(
                    "result",
                    32,
                    &[
                        (0, "success_nothing_to_do"),
                        (1, "success_performed"),
                        (2, "denied"),
                        (3, "error_wrong_ssn"),
                        (4, "error_request_already_in_progress"),
                        (5, "error_bad_sequence_number"),
                        (6, "in_progress"),
                    ],
                ),
                to_end("tsns"),
            ],
        ),
        TlvVariant::new(
            17,
            "add_outgoing_streams_request",
            vec![
                request_seq(),
                u("number_of_new_streams", 16),
                uint("reserved", 16, 0),
            ],
        ),
        TlvVariant::new(
            18,
            "add_incoming_streams_request",
            vec![
                request_seq(),
                u("number_of_new_streams", 16),
                uint("reserved", 16, 0),
            ],
        ),
        TlvVariant::new(0x8000, "ecn_capable", Vec::new()),
        TlvVariant::new(0x8002, "random", vec![to_end("random")]),
        TlvVariant::new(
            0x8003,
            "chunk_list",
            vec![uint_list("chunk_types", 8, Length::ToEnd)],
        ),
        TlvVariant::new(
            0x8004,
            "requested_hmac_algorithm",
            vec![uint_list("hmac_identifiers", 16, Length::ToEnd)],
        ),
        TlvVariant::new(0x8005, "padding", vec![to_end("padding")]),
        TlvVariant::new(
            0x8008,
            "supported_extensions",
            vec![uint_list("chunk_types", 8, Length::ToEnd)],
        ),
        TlvVariant::new(0xc000, "forward_tsn_supported", Vec::new()),
        correlated(0xc001, "add_ip_address", vec![address()]),
        correlated(0xc002, "delete_ip_address", vec![address()]),
        correlated(
            0xc003,
            "error_cause_indication",
            vec![FieldDef::new("causes", error_causes())],
        ),
        correlated(0xc004, "set_primary_address", vec![address()]),
        correlated(0xc005, "success_indication", Vec::new()),
        TlvVariant::new(
            0xc006,
            "adaptation_layer_indication",
            vec![u("indication", 32)],
        ),
    ]);
    tlv(16, 0, v)
}

/// INIT / INIT ACK body: initiate tag, advertised receiver window, stream counts,
/// initial TSN and parameters.
fn init_body(cookie: bool) -> Vec<FieldDef> {
    let params: Vec<TlvItem> = if cookie {
        vec![TlvItem::named("state_cookie").with("cookie", vec![0u8; 8])]
    } else {
        Vec::new()
    };
    vec![
        no_flags(),
        uint("initiate_tag", 32, if cookie { 2 } else { 1 }),
        uint("a_rwnd", 32, 106_496),
        uint("outbound_streams", 16, 10),
        uint("inbound_streams", 16, 65_535),
        uint("initial_tsn", 32, if cookie { 2 } else { 1 }),
        FieldDef::new("parameters", parameters()).with_default(FieldValue::Tlv(params)),
    ]
}

/// Chunk types (RFC 9260 §3.2 and the extensions: RFC 3758 FORWARD TSN, RFC 4895
/// AUTH, RFC 4820 PAD, RFC 5061 ASCONF / ASCONF-ACK, RFC 6525 RE-CONFIG, RFC 8260
/// I-DATA). Unknown types keep their flags and value as `data` (first octet: the
/// flags).
#[must_use]
pub fn chunks() -> FieldType {
    let params = || FieldDef::new("parameters", parameters());
    let causes = || FieldDef::new("causes", error_causes());
    let cum_tsn = || u("cumulative_tsn_ack", 32);
    tlv(
        8,
        1,
        vec![
            TlvVariant::new(
                0,
                "data",
                vec![
                    data_flags(),
                    uint("tsn", 32, 1),
                    u("stream_identifier", 16),
                    u("stream_sequence_number", 16),
                    u("payload_protocol_identifier", 32),
                    to_end("user_data").with_default(FieldValue::Bytes(vec![0; 4])),
                ],
            ),
            TlvVariant::new(1, "init", init_body(false)),
            TlvVariant::new(2, "init_ack", init_body(true)),
            TlvVariant::new(
                3,
                "sack",
                vec![
                    no_flags(),
                    uint("cumulative_tsn_ack", 32, 1),
                    uint("a_rwnd", 32, 106_496),
                    u("number_of_gap_ack_blocks", 16).with_auto(AutoCompute::Count {
                        field: "gap_ack_blocks".into(),
                        adjust: 0,
                    }),
                    u("number_of_duplicate_tsns", 16).with_auto(AutoCompute::Count {
                        field: "duplicate_tsns".into(),
                        adjust: 0,
                    }),
                    FieldDef::new(
                        "gap_ack_blocks",
                        FieldType::List {
                            item: ItemDef::Struct {
                                fields: vec![u("start", 16), u("end", 16)],
                            },
                            length: Length::Count {
                                field: "number_of_gap_ack_blocks".into(),
                            },
                        },
                    ),
                    uint_list(
                        "duplicate_tsns",
                        32,
                        Length::Count {
                            field: "number_of_duplicate_tsns".into(),
                        },
                    ),
                ],
            ),
            TlvVariant::new(4, "heartbeat", vec![no_flags(), params()]),
            TlvVariant::new(5, "heartbeat_ack", vec![no_flags(), params()]),
            TlvVariant::new(6, "abort", vec![t_flag(), causes()]),
            TlvVariant::new(7, "shutdown", vec![no_flags(), cum_tsn()]),
            TlvVariant::new(8, "shutdown_ack", vec![no_flags()]),
            TlvVariant::new(9, "error", vec![no_flags(), causes()]),
            TlvVariant::new(10, "cookie_echo", vec![no_flags(), to_end("cookie")]),
            TlvVariant::new(11, "cookie_ack", vec![no_flags()]),
            TlvVariant::new(14, "shutdown_complete", vec![t_flag()]),
            TlvVariant::new(
                15,
                "auth",
                vec![
                    no_flags(),
                    u("shared_key_identifier", 16),
                    uint("hmac_identifier", 16, 1),
                    to_end("hmac"),
                ],
            ),
            TlvVariant::new(
                0x40,
                "i_data",
                vec![
                    data_flags(),
                    uint("tsn", 32, 1),
                    u("stream_identifier", 16),
                    uint("reserved", 16, 0),
                    u("message_identifier", 32),
                    u("ppid_or_fsn", 32),
                    to_end("user_data").with_default(FieldValue::Bytes(vec![0; 4])),
                ],
            ),
            TlvVariant::new(
                0x80,
                "asconf_ack",
                vec![no_flags(), u("serial_number", 32), params()],
            ),
            TlvVariant::new(0x82, "re_config", vec![no_flags(), params()]),
            TlvVariant::new(0x84, "pad", vec![no_flags(), to_end("padding")]),
            TlvVariant::new(
                0xc0,
                "forward_tsn",
                vec![
                    no_flags(),
                    u("new_cumulative_tsn", 32),
                    FieldDef::new(
                        "streams",
                        FieldType::List {
                            item: ItemDef::Struct {
                                fields: vec![u("stream", 16), u("stream_sequence", 16)],
                            },
                            length: Length::ToEnd,
                        },
                    ),
                ],
            ),
            TlvVariant::new(
                0xc1,
                "asconf",
                vec![no_flags(), u("serial_number", 32), params()],
            ),
        ],
    )
}

/// SCTP (RFC 9260): source / destination port, verification tag, CRC-32C checksum
/// (Auto) and the chunks ([`chunks`]).
///
/// Default: an INIT chunk (verification tag 0) from port 49152 to port 9.
#[must_use]
pub fn sctp() -> ProtocolDef {
    protocol! {
        id: "sctp",
        name: "SCTP",
        layer: L4,
        fields: {
            src_port: uint(16) = 49152, desc = "sctp-src-port";
            dst_port: uint(16) = 9, desc = "sctp-dst-port";
            verification_tag: uint(32) = 0, desc = "sctp-verification-tag";
            checksum: uint(32), auto = checksum(crc32c, header_payload), desc = "sctp-checksum";
            chunks: ty(chunks()) = FieldValue::from(vec![TlvItem::named("init")]), desc = "sctp-chunks";
        },
        constraints: {
            warning: min_length(total, 12);
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sctp_is_valid() {
        let d = sctp();
        d.validate().unwrap();
        assert_eq!(d.id, SCTP);
    }
}
