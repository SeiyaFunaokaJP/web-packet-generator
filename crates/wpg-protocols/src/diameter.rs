//! Diameter (RFC 6733) messages and AVPs (TODO §2.1 P2, M7).
//!
//! One [`diameter`] layer holds one message: the header (version, message length,
//! the R / P / E / T flags, command code, Application-ID, Hop-by-Hop and
//! End-to-End identifiers) and its AVPs ([`avps`]).
//!
//! An AVP is a 32-bit code, a flags octet (V / M / P), a 24-bit length counting
//! the AVP header and data but not the padding, the Vendor-ID when the V flag is
//! set, and the data padded to a multiple of 4 octets. The base protocol AVPs
//! (RFC 6733) and the Credit-Control ones (RFC 4006) are typed by their data type
//! (Unsigned32 / 64, Enumerated, Address, Time, `OctetString` / `UTF8String` /
//! `DiameterIdentity` octets); the grouped AVPs (Vendor-Specific-Application-Id,
//! Failed-AVP, Proxy-Info, Experimental-Result, Subscription-Id) hold AVPs again,
//! one level deep. Unknown and vendor-specific AVPs keep their octets.
//!
//! Reached through TCP 3868. Auto fields: the message length, every AVP length,
//! the V flag (set when a Vendor-ID is given) and the padding. The AVPs end at the
//! message length, so octets after it (another message in the same segment) parse
//! as the next layer. Diameter over SCTP is built by putting the message in the
//! user data of an SCTP DATA chunk; the SCTP layer does not decode its user data.

use wpg_core::{
    protocol, AutoCompute, EnumVariant, FieldDef, FieldType, FieldValue, ItemDef, Length,
    LengthUnit, ProtocolDef, TlvDef, TlvItem, TlvVariant,
};

/// Id of the Diameter message ([`diameter`]).
pub const DIAMETER: &str = "diameter";

/// TCP / SCTP port of Diameter (RFC 6733 §2.1).
pub const DIAMETER_PORT: u64 = 3868;

fn u(name: &str, bits: u8) -> FieldDef {
    FieldDef::new(name, FieldType::UInt { bits })
}

fn uint(name: &str, bits: u8, default: u64) -> FieldDef {
    u(name, bits).with_default(FieldValue::UInt(default))
}

fn enum_type(bits: u8, list: &[(u64, &str)]) -> FieldType {
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
    }
}

/// Command codes (RFC 6733 §3.1, RFC 4006, TS 29.272 S6a).
const COMMAND_CODES: &[(u64, &str)] = &[
    (257, "capabilities_exchange"),
    (258, "re_auth"),
    (271, "accounting"),
    (272, "credit_control"),
    (274, "abort_session"),
    (275, "session_termination"),
    (280, "device_watchdog"),
    (282, "disconnect_peer"),
    (316, "update_location"),
    (317, "cancel_location"),
    (318, "authentication_information"),
    (319, "insert_subscriber_data"),
    (320, "delete_subscriber_data"),
    (321, "purge_ue"),
    (323, "notify"),
];

/// Result-Code values (RFC 6733 §7.1).
const RESULT_CODES: &[(u64, &str)] = &[
    (1001, "multi_round_auth"),
    (2001, "success"),
    (2002, "limited_success"),
    (3001, "command_unsupported"),
    (3002, "unable_to_deliver"),
    (3003, "realm_not_served"),
    (3004, "too_busy"),
    (3005, "loop_detected"),
    (3006, "redirect_indication"),
    (3007, "application_unsupported"),
    (3008, "invalid_hdr_bits"),
    (3009, "invalid_avp_bits"),
    (3010, "unknown_peer"),
    (4001, "authentication_rejected"),
    (4002, "out_of_space"),
    (4003, "election_lost"),
    (5001, "avp_unsupported"),
    (5002, "unknown_session_id"),
    (5003, "authorization_rejected"),
    (5004, "invalid_avp_value"),
    (5005, "missing_avp"),
    (5006, "resources_exceeded"),
    (5007, "contradicting_avps"),
    (5008, "avp_not_allowed"),
    (5009, "avp_occurs_too_many_times"),
    (5010, "no_common_application"),
    (5011, "unsupported_version"),
    (5012, "unable_to_comply"),
    (5013, "invalid_bit_in_header"),
    (5014, "invalid_avp_length"),
    (5015, "invalid_message_length"),
    (5016, "invalid_avp_bit_combo"),
    (5017, "no_common_security"),
];

/// Disconnect-Cause values (RFC 6733 §5.4.3).
const DISCONNECT_CAUSES: &[(u64, &str)] = &[
    (0, "rebooting"),
    (1, "busy"),
    (2, "do_not_want_to_talk_to_you"),
];

/// Auth-Session-State values (RFC 6733 §8.11).
const AUTH_SESSION_STATES: &[(u64, &str)] = &[(0, "state_maintained"), (1, "no_state_maintained")];

/// Termination-Cause values (RFC 6733 §8.15).
const TERMINATION_CAUSES: &[(u64, &str)] = &[
    (1, "logout"),
    (2, "service_not_provided"),
    (3, "bad_answer"),
    (4, "administrative"),
    (5, "link_broken"),
    (6, "auth_expired"),
    (7, "user_moved"),
    (8, "session_timeout"),
];

/// Accounting-Record-Type values (RFC 6733 §9.8.1).
const ACCOUNTING_RECORD_TYPES: &[(u64, &str)] = &[
    (1, "event_record"),
    (2, "start_record"),
    (3, "interim_record"),
    (4, "stop_record"),
];

/// CC-Request-Type values (RFC 4006 §8.3).
const CC_REQUEST_TYPES: &[(u64, &str)] = &[
    (1, "initial_request"),
    (2, "update_request"),
    (3, "termination_request"),
    (4, "event_request"),
];

/// Subscription-Id-Type values (RFC 4006 §8.47).
const SUBSCRIPTION_ID_TYPES: &[(u64, &str)] = &[
    (0, "end_user_e164"),
    (1, "end_user_imsi"),
    (2, "end_user_sip_uri"),
    (3, "end_user_nai"),
    (4, "end_user_private"),
];

/// The Vendor-ID, present when the V flag is set (one element).
fn vendor_id() -> FieldDef {
    FieldDef::new(
        "vendor_id",
        FieldType::List {
            item: ItemDef::Field {
                ty: Box::new(FieldType::UInt { bits: 32 }),
            },
            length: Length::Count { field: "v".into() },
        },
    )
}

/// An AVP variant: the flags (in the octet between the code and the length), the
/// optional Vendor-ID, then the AVP data `fields`. `m` is the default of the
/// Mandatory flag.
fn avp(code: u64, name: &str, m: bool, fields: Vec<FieldDef>) -> TlvVariant {
    let mut all = vec![
        u("v", 1).with_auto(AutoCompute::Count {
            field: "vendor_id".into(),
        }),
        uint("m", 1, u64::from(m)),
        uint("p", 1, 0),
        uint("reserved", 5, 0),
        vendor_id(),
    ];
    all.extend(fields);
    TlvVariant::new(code, name, all)
}

/// `OctetString`, `UTF8String`, `DiameterIdentity` and `DiameterURI` data.
fn octets(code: u64, name: &str, m: bool) -> TlvVariant {
    avp(
        code,
        name,
        m,
        vec![FieldDef::new(
            "value",
            FieldType::Bytes { len: Length::ToEnd },
        )],
    )
}

/// Unsigned32 / Time data.
fn u32_avp(code: u64, name: &str, m: bool) -> TlvVariant {
    avp(code, name, m, vec![u("value", 32)])
}

/// Enumerated data with named values.
fn enumerated(code: u64, name: &str, list: &[(u64, &str)], default: u64) -> TlvVariant {
    avp(
        code,
        name,
        true,
        vec![FieldDef::new("value", enum_type(32, list)).with_default(FieldValue::UInt(default))],
    )
}

/// Address data (RFC 6733 §4.3.1): the address family and the address, told
/// apart by the data length (flags octet, family and 4 or 16 address octets).
fn address(code: u64, name: &str) -> [TlvVariant; 2] {
    [
        avp(
            code,
            name,
            true,
            vec![
                uint("address_family", 16, 1),
                FieldDef::new("address", FieldType::Ipv4)
                    .with_default(FieldValue::Ipv4([192, 0, 2, 1].into())),
            ],
        )
        .when_value_len(7),
        avp(
            code,
            &format!("{name}_ipv6"),
            true,
            vec![
                uint("address_family", 16, 2),
                FieldDef::new("address", FieldType::Ipv6)
                    .with_default(FieldValue::Ipv6("2001:db8::1".parse().unwrap())),
            ],
        )
        .when_value_len(19),
    ]
}

/// A grouped AVP: its data is AVPs again (the non-grouped ones), defaulting to
/// `defaults`.
fn grouped(code: u64, name: &str, m: bool, defaults: Vec<TlvItem>) -> TlvVariant {
    avp(
        code,
        name,
        m,
        vec![FieldDef::new("avps", avps(false)).with_default(FieldValue::Tlv(defaults))],
    )
}

/// Diameter AVPs (RFC 6733 §4): 32-bit code, flags octet, 24-bit length including
/// the header, optional Vendor-ID, data padded to 4 octets. With `nested`, the
/// grouped AVPs are included (their data is the AVPs without grouped ones).
#[must_use]
pub fn avps(nested: bool) -> FieldType {
    let mut variants = vec![
        octets(1, "user_name", true),
        octets(25, "class", true),
        u32_avp(27, "session_timeout", true),
        octets(33, "proxy_state", true),
        octets(44, "accounting_session_id", true),
        octets(50, "acct_multi_session_id", true),
        u32_avp(55, "event_timestamp", true),
        u32_avp(85, "acct_interim_interval", true),
        u32_avp(258, "auth_application_id", true),
        u32_avp(259, "acct_application_id", true),
        u32_avp(262, "redirect_max_cache_time", true),
        octets(263, "session_id", true),
        octets(264, "origin_host", true),
        u32_avp(265, "supported_vendor_id", true),
        u32_avp(266, "vendor_id", true),
        u32_avp(267, "firmware_revision", false),
        enumerated(268, "result_code", RESULT_CODES, 2001),
        octets(269, "product_name", false),
        u32_avp(270, "session_binding", true),
        u32_avp(272, "multi_round_time_out", true),
        enumerated(273, "disconnect_cause", DISCONNECT_CAUSES, 0),
        enumerated(
            274,
            "auth_request_type",
            &[
                (1, "authenticate_only"),
                (2, "authorize_only"),
                (3, "authorize_authenticate"),
            ],
            3,
        ),
        u32_avp(276, "auth_grace_period", true),
        enumerated(277, "auth_session_state", AUTH_SESSION_STATES, 1),
        u32_avp(278, "origin_state_id", true),
        octets(280, "proxy_host", true),
        octets(281, "error_message", false),
        octets(282, "route_record", true),
        octets(283, "destination_realm", true),
        enumerated(
            285,
            "re_auth_request_type",
            &[(0, "authorize_only"), (1, "authorize_authenticate")],
            0,
        ),
        avp(287, "accounting_sub_session_id", true, vec![u("value", 64)]),
        u32_avp(291, "authorization_lifetime", true),
        octets(292, "redirect_host", true),
        octets(293, "destination_host", true),
        octets(294, "error_reporting_host", false),
        enumerated(295, "termination_cause", TERMINATION_CAUSES, 1),
        octets(296, "origin_realm", true),
        u32_avp(298, "experimental_result_code", true),
        u32_avp(299, "inband_security_id", true),
        u32_avp(415, "cc_request_number", true),
        enumerated(416, "cc_request_type", CC_REQUEST_TYPES, 1),
        octets(444, "subscription_id_data", true),
        enumerated(450, "subscription_id_type", SUBSCRIPTION_ID_TYPES, 0),
        enumerated(
            455,
            "multiple_services_indicator",
            &[(0, "not_supported"), (1, "supported")],
            0,
        ),
        octets(461, "service_context_id", true),
        enumerated(480, "accounting_record_type", ACCOUNTING_RECORD_TYPES, 1),
        u32_avp(485, "accounting_record_number", true),
    ];
    variants.extend(address(257, "host_ip_address"));
    if nested {
        variants.extend([
            grouped(
                260,
                "vendor_specific_application_id",
                true,
                vec![
                    TlvItem::named("vendor_id").with("value", 10415u32),
                    TlvItem::named("auth_application_id").with("value", 16_777_251u32),
                ],
            ),
            grouped(279, "failed_avp", true, Vec::new()),
            grouped(
                284,
                "proxy_info",
                true,
                vec![
                    TlvItem::named("proxy_host").with("value", b"proxy.example.com".to_vec()),
                    TlvItem::named("proxy_state").with("value", b"\x00\x01".to_vec()),
                ],
            ),
            grouped(
                297,
                "experimental_result",
                true,
                vec![
                    TlvItem::named("vendor_id").with("value", 10415u32),
                    TlvItem::named("experimental_result_code").with("value", 5001u32),
                ],
            ),
            grouped(
                443,
                "subscription_id",
                true,
                vec![
                    TlvItem::named("subscription_id_type"),
                    TlvItem::named("subscription_id_data").with("value", b"819012345678".to_vec()),
                ],
            ),
        ]);
    }
    FieldType::Tlv(TlvDef {
        type_bits: 32,
        len_bits: 24,
        len_includes_header: true,
        len_unit: LengthUnit::Bytes,
        variants,
        unknown_as_raw: true,
        length: if nested {
            Length::UntilOffset {
                field: "message_length".into(),
                unit: 1,
                adjust: 0,
            }
        } else {
            Length::ToEnd
        },
        fill: None,
        head_octets: 1,
        align: 4,
        unpadded_last: false,
        long_length: None,
        len_first: false,
        uncounted_octets: 0,
    })
}

/// Diameter message (RFC 6733 §3): version 1, message length (Auto), the R
/// (request) / P (proxiable) / E (error) / T (retransmission) flags, command code,
/// Application-ID, Hop-by-Hop and End-to-End identifiers and the AVPs ([`avps`]).
///
/// Default: a Capabilities-Exchange-Request (Application-ID 0) from
/// `client.example.com` / `example.com` at 192.0.2.1, Vendor-Id 0 and
/// Product-Name `wpg`.
#[must_use]
pub fn diameter() -> ProtocolDef {
    let default_avps = vec![
        TlvItem::named("origin_host").with("value", b"client.example.com".to_vec()),
        TlvItem::named("origin_realm").with("value", b"example.com".to_vec()),
        TlvItem::named("host_ip_address"),
        TlvItem::named("vendor_id").with("value", 0u32),
        TlvItem::named("product_name").with("value", b"wpg".to_vec()),
    ];
    protocol! {
        id: "diameter",
        name: "Diameter",
        layer: L7,
        fields: {
            version: uint(8) = 1, desc = "diameter-version";
            message_length: uint(24), auto = length(header), desc = "diameter-length";
            r: uint(1) = 1, desc = "diameter-r";
            p: uint(1) = 0, desc = "diameter-p";
            e: uint(1) = 0, desc = "diameter-e";
            t: uint(1) = 0, desc = "diameter-t";
            reserved: uint(4) = 0;
            command_code: ty(enum_type(24, COMMAND_CODES)) = FieldValue::UInt(257), desc = "diameter-command-code";
            application_id: uint(32) = 0, desc = "diameter-application-id";
            hop_by_hop: uint(32) = 1, desc = "diameter-hop-by-hop";
            end_to_end: uint(32) = 1, desc = "diameter-end-to-end";
            avps: ty(avps(true)) = FieldValue::from(default_avps), desc = "diameter-avps";
        },
        constraints: {
            warning: equals(version, 1);
            error: min_length(header, 20);
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diameter_is_valid() {
        let d = diameter();
        d.validate().unwrap();
        assert_eq!(d.id, DIAMETER);
    }
}
