//! LDP (RFC 5036) PDUs, messages and TLVs (TODO §2.1 P2, M7).
//!
//! One [`ldp`] layer holds the PDU header and its messages. Messages and TLVs
//! share one shape: a 16-bit type (the U bit, and for TLVs the F bit, are the top
//! bits of the type, so `0x8xxx` names an "unknown, ignore" type), a 16-bit length
//! counting the value only, and the value. Every message starts with its 32-bit
//! message ID, followed by its TLVs ([`tlvs`]).
//!
//! Reached through UDP 646 (Hello: basic discovery to 224.0.0.2 / `ff02::2`, or
//! extended discovery to a unicast address) and TCP 646 (the session).
//!
//! Auto fields: `pdu_length` (the PDU after the version and length fields), every
//! message and TLV length. One `ldp` layer is one PDU; the messages of a PDU end
//! at its length, so octets after it (another PDU in the same TCP segment) parse
//! as the next layer.

use wpg_core::{
    protocol, EnumVariant, FieldDef, FieldType, FieldValue, ItemDef, Length, LengthUnit,
    ProtocolDef, TlvDef, TlvItem, TlvVariant,
};

/// Id of the LDP PDU ([`ldp`]).
pub const LDP: &str = "ldp";

/// UDP (discovery) and TCP (session) port of LDP.
pub const LDP_PORT: u64 = 646;

fn u(name: &str, bits: u8) -> FieldDef {
    FieldDef::new(name, FieldType::UInt { bits })
}

fn uint(name: &str, bits: u8, default: u64) -> FieldDef {
    u(name, bits).with_default(FieldValue::UInt(default))
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

fn list(name: &str, ty: FieldType) -> FieldDef {
    FieldDef::new(
        name,
        FieldType::List {
            item: ItemDef::Field { ty: Box::new(ty) },
            length: Length::ToEnd,
        },
    )
}

/// LDP TLVs / messages: 16-bit type, 16-bit length of the value, filling `length`.
fn tlv(variants: Vec<TlvVariant>, length: Length) -> FieldType {
    FieldType::Tlv(TlvDef {
        type_bits: 16,
        len_bits: 16,
        len_includes_header: false,
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
    })
}

/// Address family numbers (IANA) used by LDP: 1 = IPv4, 2 = IPv6.
const FAMILIES: &[(u64, &str)] = &[(1, "ipv4"), (2, "ipv6")];

/// Status codes (RFC 5036 §3.9, RFC 5561 / 5918 / 7552 extensions not listed).
const STATUS_CODES: &[(u64, &str)] = &[
    (0x00, "success"),
    (0x01, "bad_ldp_identifier"),
    (0x02, "bad_protocol_version"),
    (0x03, "bad_pdu_length"),
    (0x04, "unknown_message_type"),
    (0x05, "bad_message_length"),
    (0x06, "unknown_tlv"),
    (0x07, "bad_tlv_length"),
    (0x08, "malformed_tlv_value"),
    (0x09, "hold_timer_expired"),
    (0x0a, "shutdown"),
    (0x0b, "loop_detected"),
    (0x0c, "unknown_fec"),
    (0x0d, "no_route"),
    (0x0e, "no_label_resources"),
    (0x0f, "label_resources_available"),
    (0x10, "session_rejected_no_hello"),
    (0x11, "session_rejected_advertisement_mode"),
    (0x12, "session_rejected_max_pdu_length"),
    (0x13, "session_rejected_label_range"),
    (0x14, "keepalive_timer_expired"),
    (0x15, "label_request_aborted"),
    (0x16, "missing_message_parameters"),
    (0x17, "unsupported_address_family"),
    (0x18, "session_rejected_bad_keepalive_time"),
    (0x19, "internal_error"),
];

/// FEC elements of a FEC TLV (RFC 5036 §3.4.1): prefix elements (type 2:
/// address family, prefix length in bits, `ceil(length / 8)` prefix octets).
/// The wildcard element (type 1) is the `fec_wildcard` TLV.
fn fec_elements() -> FieldDef {
    FieldDef::new(
        "elements",
        FieldType::List {
            item: ItemDef::Struct {
                fields: vec![
                    uint("element_type", 8, 2),
                    enum_field("address_family", 16, FAMILIES).with_default(FieldValue::UInt(1)),
                    u("prefix_length", 8),
                    FieldDef::new(
                        "prefix",
                        FieldType::Bytes {
                            len: Length::FromField {
                                field: "prefix_length".into(),
                                unit: 1,
                                adjust: 7,
                                div: 8,
                            },
                        },
                    ),
                ],
            },
            length: Length::ToEnd,
        },
    )
}

/// TLVs carried by LDP messages (RFC 5036 §3.4-3.5, RFC 7552 IPv6 transport
/// address): FEC (prefix elements, or the wildcard element), address lists
/// (IPv4 / IPv6, told apart by the address family), hop count, path vector,
/// generic label, status, the Hello parameters (with the GTSM bit of RFC 6720) and
/// Initialization parameters, and the label request message ID. ATM / Frame Relay labels and session parameters and
/// vendor-private TLVs keep their value octets.
#[must_use]
pub fn tlvs() -> FieldType {
    tlv(
        vec![
            TlvVariant::new(0x0100, "fec_wildcard", vec![uint("element_type", 8, 1)])
                .when_value_starts(&[1]),
            TlvVariant::new(0x0100, "fec", vec![fec_elements()]),
            TlvVariant::new(
                0x0101,
                "address_list",
                vec![
                    uint("address_family", 16, 1),
                    list("addresses", FieldType::Ipv4),
                ],
            )
            .when_value_starts(&[0, 1]),
            TlvVariant::new(
                0x0101,
                "address_list_ipv6",
                vec![
                    uint("address_family", 16, 2),
                    list("addresses", FieldType::Ipv6),
                ],
            )
            .when_value_starts(&[0, 2]),
            TlvVariant::new(0x0103, "hop_count", vec![uint("hop_count", 8, 1)]),
            TlvVariant::new(
                0x0104,
                "path_vector",
                vec![list("lsr_ids", FieldType::Ipv4)],
            ),
            TlvVariant::new(
                0x0200,
                "generic_label",
                vec![uint("reserved", 12, 0), uint("label", 20, 16)],
            ),
            TlvVariant::new(
                0x0300,
                "status",
                vec![
                    uint("e", 1, 0),
                    uint("f", 1, 0),
                    enum_field("status_code", 30, STATUS_CODES).with_default(FieldValue::UInt(0)),
                    u("message_id", 32),
                    u("message_type", 16),
                ],
            ),
            TlvVariant::new(
                0x0400,
                "common_hello_parameters",
                vec![
                    uint("hold_time", 16, 15),
                    uint("t", 1, 0),
                    uint("r", 1, 0),
                    uint("g", 1, 0),
                    uint("reserved", 13, 0),
                ],
            ),
            TlvVariant::new(
                0x0401,
                "ipv4_transport_address",
                vec![FieldDef::new("address", FieldType::Ipv4)],
            ),
            TlvVariant::new(
                0x0402,
                "configuration_sequence_number",
                vec![uint("sequence_number", 32, 1)],
            ),
            TlvVariant::new(
                0x0403,
                "ipv6_transport_address",
                vec![FieldDef::new("address", FieldType::Ipv6)],
            ),
            TlvVariant::new(
                0x0500,
                "common_session_parameters",
                vec![
                    uint("protocol_version", 16, 1),
                    uint("keepalive_time", 16, 180),
                    uint("a", 1, 0),
                    uint("d", 1, 0),
                    uint("reserved", 6, 0),
                    uint("pv_lim", 8, 0),
                    uint("max_pdu_length", 16, 0),
                    FieldDef::new("receiver_lsr_id", FieldType::Ipv4)
                        .with_default(FieldValue::Ipv4([192, 0, 2, 2].into())),
                    uint("receiver_label_space", 16, 0),
                ],
            ),
            TlvVariant::new(
                0x0600,
                "label_request_message_id",
                vec![u("message_id", 32)],
            ),
        ],
        Length::ToEnd,
    )
}

/// A message variant: the message ID, then TLVs (default `defaults`).
fn message(code: u64, name: &str, defaults: Vec<TlvItem>) -> TlvVariant {
    TlvVariant::new(
        code,
        name,
        vec![
            uint("message_id", 32, 1),
            FieldDef::new("tlvs", tlvs()).with_default(FieldValue::Tlv(defaults)),
        ],
    )
}

/// LDP messages (RFC 5036 §3.5): Notification, Hello, Initialization, `KeepAlive`,
/// Address / Address Withdraw and the label messages (Mapping, Request,
/// Withdraw, Release, Abort Request). They end at the PDU length.
#[must_use]
pub fn messages() -> FieldType {
    tlv(
        vec![
            message(0x0001, "notification", vec![TlvItem::named("status")]),
            message(
                0x0100,
                "hello",
                vec![TlvItem::named("common_hello_parameters")],
            ),
            message(
                0x0200,
                "initialization",
                vec![TlvItem::named("common_session_parameters")],
            ),
            message(0x0201, "keepalive", Vec::new()),
            message(0x0300, "address", Vec::new()),
            message(0x0301, "address_withdraw", Vec::new()),
            message(0x0400, "label_mapping", Vec::new()),
            message(0x0401, "label_request", Vec::new()),
            message(0x0402, "label_withdraw", Vec::new()),
            message(0x0403, "label_release", Vec::new()),
            message(0x0404, "label_abort_request", Vec::new()),
        ],
        Length::FromField {
            field: "pdu_length".into(),
            unit: 1,
            adjust: -6,
            div: 1,
        },
    )
}

/// LDP PDU (RFC 5036 §3.1): version, PDU length (Auto), LDP identifier (LSR ID
/// and label space) and the messages ([`messages`]).
///
/// Default: a Hello with a 15 s hold time from LSR 192.0.2.1, label space 0.
#[must_use]
pub fn ldp() -> ProtocolDef {
    protocol! {
        id: "ldp",
        name: "LDP",
        layer: L7,
        fields: {
            version: uint(16) = 1, desc = "ldp-version";
            pdu_length: uint(16), auto = length(header, adjust = -4), desc = "ldp-pdu-length";
            lsr_id: ipv4 = [192, 0, 2, 1], desc = "ldp-lsr-id";
            label_space: uint(16) = 0, desc = "ldp-label-space";
            messages: ty(messages()) = FieldValue::from(vec![TlvItem::named("hello")]), desc = "ldp-messages";
        },
        constraints: {
            warning: equals(version, 1);
            error: min_length(header, 10);
            warning: max_length(header, 4096);
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ldp_is_valid() {
        let d = ldp();
        d.validate().unwrap();
        assert_eq!(d.id, LDP);
    }
}
