//! PTP, IEEE 1588-2008 (version 2) (TODO §2.1 P2, M7).
//!
//! Every message is one layer: the 34-octet common header, the message body and
//! the TLVs that follow it (suffix TLVs, the Signaling TLVs and the Management
//! TLV). PTP is reached through EtherType `0x88f7` (IEEE 1588 annex F) and UDP
//! 319 (event messages) / 320 (general messages) over IPv4 and IPv6 (annexes D
//! and E). The parser decodes it as the generic [`ptp`] first, whose
//! `message_type` then re-decodes the layer as the dedicated message
//! (`ptp_sync`, `ptp_announce`, ...), like IS-IS PDUs.
//!
//! Auto fields: `message_length` (the whole message with its TLVs) and every TLV
//! length. The TLVs end at `message_length`, so Ethernet padding after a short
//! message is not mistaken for TLVs.
//!
//! Timestamps are split into their 48-bit seconds and 32-bit nanoseconds; port
//! identities into their clock identity (EUI-64) and port number.
//! `correction_field` is the signed correction in nanoseconds multiplied by
//! 2^16. Management TLV data fields and the authentication / security TLVs are
//! kept as octets.

use wpg_core::{
    Binding, BindingEntry, Constraint, ConstraintRule, DiagLevel, EnumVariant, FieldDef, FieldType,
    FieldValue, FlagBit, ItemDef, Length, LengthUnit, ProtocolDef, ProtocolLayer, TlvDef, TlvItem,
    TlvVariant,
};

/// Id of the generic PTP message ([`ptp`]).
pub const PTP: &str = "ptp";

/// EtherType of PTP over Ethernet (IEEE 1588 annex F).
pub const PTP_ETHERTYPE: u64 = 0x88f7;

/// UDP port of PTP event messages (Sync, `Delay_Req`, `Pdelay_Req`, `Pdelay_Resp`).
pub const PTP_EVENT_PORT: u64 = 319;

/// UDP port of PTP general messages (`Follow_Up`, `Delay_Resp`, Announce, ...).
pub const PTP_GENERAL_PORT: u64 = 320;

/// Length of the common header in octets.
const HEADER_LEN: i64 = 34;

/// The dedicated messages: (message type, protocol id, whether it is an event
/// message sent to UDP 319).
pub const MESSAGES: [(u64, &str, bool); 10] = [
    (0x0, "ptp_sync", true),
    (0x1, "ptp_delay_req", true),
    (0x2, "ptp_pdelay_req", true),
    (0x3, "ptp_pdelay_resp", true),
    (0x8, "ptp_follow_up", false),
    (0x9, "ptp_delay_resp", false),
    (0xa, "ptp_pdelay_resp_follow_up", false),
    (0xb, "ptp_announce", false),
    (0xc, "ptp_signaling", false),
    (0xd, "ptp_management", false),
];

/// Binding entries announcing PTP under `value` (an EtherType or UDP port): the
/// generic [`ptp`] first (the parser takes it), then the dedicated messages
/// (`event`: only the event messages, `Some(false)`: only the general ones).
#[must_use]
pub fn bindings(value: u64, event: Option<bool>) -> Vec<BindingEntry> {
    std::iter::once(BindingEntry::new(value, PTP))
        .chain(
            MESSAGES
                .iter()
                .filter(|m| event.is_none_or(|e| e == m.2))
                .map(|m| BindingEntry::new(value, m.1)),
        )
        .collect()
}

fn u(name: &str, bits: u8) -> FieldDef {
    FieldDef::new(name, FieldType::UInt { bits })
}

fn uint(name: &str, bits: u8, default: u64) -> FieldDef {
    u(name, bits).with_default(FieldValue::UInt(default))
}

fn int(name: &str, bits: u8, default: i64) -> FieldDef {
    FieldDef::new(name, FieldType::Int { bits }).with_default(FieldValue::Int(default))
}

fn octets(name: &str, n: u32) -> FieldDef {
    FieldDef::new(
        name,
        FieldType::Bytes {
            len: Length::Fixed { octets: n },
        },
    )
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

fn desc(f: FieldDef, key: &str) -> FieldDef {
    f.with_description(key)
}

const MESSAGE_TYPES: &[(u64, &str)] = &[
    (0x0, "sync"),
    (0x1, "delay_req"),
    (0x2, "pdelay_req"),
    (0x3, "pdelay_resp"),
    (0x8, "follow_up"),
    (0x9, "delay_resp"),
    (0xa, "pdelay_resp_follow_up"),
    (0xb, "announce"),
    (0xc, "signaling"),
    (0xd, "management"),
];

/// Clock accuracy (IEEE 1588-2008 Table 6).
const CLOCK_ACCURACY: &[(u64, &str)] = &[
    (0x20, "within_25ns"),
    (0x21, "within_100ns"),
    (0x22, "within_250ns"),
    (0x23, "within_1us"),
    (0x24, "within_2_5us"),
    (0x25, "within_10us"),
    (0x26, "within_25us"),
    (0x27, "within_100us"),
    (0x28, "within_250us"),
    (0x29, "within_1ms"),
    (0x2a, "within_2_5ms"),
    (0x2b, "within_10ms"),
    (0x2c, "within_25ms"),
    (0x2d, "within_100ms"),
    (0x2e, "within_250ms"),
    (0x2f, "within_1s"),
    (0x30, "within_10s"),
    (0x31, "over_10s"),
    (0xfe, "unknown"),
];

/// Time source (IEEE 1588-2008 Table 7).
const TIME_SOURCES: &[(u64, &str)] = &[
    (0x10, "atomic_clock"),
    (0x20, "gps"),
    (0x30, "terrestrial_radio"),
    (0x40, "ptp"),
    (0x50, "ntp"),
    (0x60, "hand_set"),
    (0x90, "other"),
    (0xa0, "internal_oscillator"),
];

/// A few management IDs (IEEE 1588-2008 Table 40).
const MANAGEMENT_IDS: &[(u64, &str)] = &[
    (0x0000, "null_management"),
    (0x0001, "clock_description"),
    (0x0002, "user_description"),
    (0x2000, "default_data_set"),
    (0x2001, "current_data_set"),
    (0x2002, "parent_data_set"),
    (0x2003, "time_properties_data_set"),
    (0x2004, "port_data_set"),
    (0x2005, "priority1"),
    (0x2006, "priority2"),
    (0x2007, "domain"),
    (0x2008, "slave_only"),
    (0x2009, "log_announce_interval"),
    (0x200d, "log_sync_interval"),
    (0x2015, "utc_properties"),
];

/// Common header (IEEE 1588-2008 §13.3) with the message type, control field and
/// log message interval defaults of one message.
fn header(message_type: u64, control: u64, log_interval: i64) -> Vec<FieldDef> {
    vec![
        desc(uint("major_sdo_id", 4, 0), "ptp-major-sdo-id"),
        desc(
            enum_field("message_type", 4, MESSAGE_TYPES)
                .with_default(FieldValue::UInt(message_type)),
            "ptp-message-type",
        ),
        desc(uint("minor_version", 4, 0), "ptp-minor-version"),
        desc(uint("version", 4, 2), "ptp-version"),
        desc(
            u("message_length", 16).with_auto(wpg_core::AutoCompute::Length {
                of: wpg_core::LengthOf::Total,
                unit: LengthUnit::Bytes,
                adjust: 0,
            }),
            "ptp-message-length",
        ),
        desc(uint("domain_number", 8, 0), "ptp-domain-number"),
        desc(uint("minor_sdo_id", 8, 0), "ptp-minor-sdo-id"),
        desc(
            FieldDef::new(
                "flags",
                FieldType::Flags {
                    bits: 16,
                    flags: [
                        (15, "security"),
                        (14, "profile_specific_2"),
                        (13, "profile_specific_1"),
                        (10, "unicast"),
                        (9, "two_step"),
                        (8, "alternate_master"),
                        (6, "synchronization_uncertain"),
                        (5, "frequency_traceable"),
                        (4, "time_traceable"),
                        (3, "ptp_timescale"),
                        (2, "current_utc_offset_valid"),
                        (1, "leap59"),
                        (0, "leap61"),
                    ]
                    .iter()
                    .map(|(bit, n)| FlagBit {
                        bit: *bit,
                        name: (*n).into(),
                    })
                    .collect(),
                },
            )
            .with_default(FieldValue::UInt(0)),
            "ptp-flags",
        ),
        desc(int("correction_field", 64, 0), "ptp-correction-field"),
        desc(
            uint("message_type_specific", 32, 0),
            "ptp-message-type-specific",
        ),
        desc(
            uint("clock_identity", 64, 0x0200_00ff_fe00_0001),
            "ptp-clock-identity",
        ),
        desc(uint("port_number", 16, 1), "ptp-port-number"),
        desc(uint("sequence_id", 16, 0), "ptp-sequence-id"),
        desc(
            enum_field(
                "control_field",
                8,
                &[
                    (0, "sync"),
                    (1, "delay_req"),
                    (2, "follow_up"),
                    (3, "delay_resp"),
                    (4, "management"),
                    (5, "other"),
                ],
            )
            .with_default(FieldValue::UInt(control)),
            "ptp-control-field",
        ),
        desc(
            int("log_message_interval", 8, log_interval),
            "ptp-log-message-interval",
        ),
    ]
}

/// A timestamp: 48-bit seconds and 32-bit nanoseconds.
fn timestamp(name: &str) -> [FieldDef; 2] {
    [
        desc(
            uint(&format!("{name}_seconds"), 48, 0),
            "ptp-timestamp-seconds",
        ),
        desc(
            uint(&format!("{name}_nanoseconds"), 32, 0),
            "ptp-timestamp-nanoseconds",
        ),
    ]
}

/// A port identity: clock identity and port number.
fn port_identity(name: &str, clock: u64, port: u64) -> [FieldDef; 2] {
    [
        desc(
            uint(&format!("{name}_clock_identity"), 64, clock),
            "ptp-port-identity-clock",
        ),
        desc(
            uint(&format!("{name}_port_number"), 16, port),
            "ptp-port-identity-port",
        ),
    ]
}

/// `unit_message_type` (high nibble) and its reserved low nibble, as in the
/// unicast negotiation TLVs.
fn unicast_message_type() -> [FieldDef; 2] {
    [
        enum_field("message_type", 4, MESSAGE_TYPES).with_default(FieldValue::UInt(0xb)),
        uint("reserved", 4, 0),
    ]
}

/// TLVs (IEEE 1588-2008 §14): 16-bit type, 16-bit length of the value. They end
/// at `message_length`, after a body of `body` octets.
#[must_use]
pub fn tlvs(body: i64) -> FieldType {
    let [mt, rsv] = unicast_message_type();
    let interval = || int("log_inter_message_period", 8, 1);
    let duration = || uint("duration_field", 32, 300);
    let variants = vec![
        TlvVariant::new(
            0x0001,
            "management",
            vec![
                enum_field("management_id", 16, MANAGEMENT_IDS)
                    .with_default(FieldValue::UInt(0x2000)),
                to_end("data_field"),
            ],
        ),
        TlvVariant::new(
            0x0002,
            "management_error_status",
            vec![
                enum_field(
                    "management_error_id",
                    16,
                    &[
                        (0x0001, "response_too_big"),
                        (0x0002, "no_such_id"),
                        (0x0003, "wrong_length"),
                        (0x0004, "wrong_value"),
                        (0x0005, "not_setable"),
                        (0x0006, "not_supported"),
                        (0xfffe, "general_error"),
                    ],
                ),
                enum_field("management_id", 16, MANAGEMENT_IDS),
                uint("reserved", 32, 0),
                to_end("display_data"),
            ],
        ),
        TlvVariant::new(
            0x0003,
            "organization_extension",
            vec![
                u("organization_id", 24),
                u("organization_sub_type", 24),
                to_end("data_field"),
            ],
        ),
        TlvVariant::new(
            0x0004,
            "request_unicast_transmission",
            vec![mt.clone(), rsv.clone(), interval(), duration()],
        ),
        TlvVariant::new(
            0x0005,
            "grant_unicast_transmission",
            vec![
                mt.clone(),
                rsv.clone(),
                interval(),
                duration(),
                uint("reserved2", 8, 0),
                uint("reserved3", 7, 0),
                uint("renewal_invited", 1, 0),
            ],
        ),
        TlvVariant::new(
            0x0006,
            "cancel_unicast_transmission",
            vec![mt.clone(), rsv.clone(), uint("reserved2", 8, 0)],
        ),
        TlvVariant::new(
            0x0007,
            "acknowledge_cancel_unicast_transmission",
            vec![mt, rsv, uint("reserved2", 8, 0)],
        ),
        TlvVariant::new(
            0x0008,
            "path_trace",
            vec![FieldDef::new(
                "path_sequence",
                FieldType::List {
                    item: ItemDef::Field {
                        ty: Box::new(FieldType::UInt { bits: 64 }),
                    },
                    length: Length::ToEnd,
                },
            )],
        ),
        TlvVariant::new(
            0x0009,
            "alternate_time_offset_indicator",
            vec![
                uint("key_field", 8, 0),
                int("current_offset", 32, 0),
                int("jump_seconds", 32, 0),
                uint("time_of_next_jump", 48, 0),
                u("display_name_length", 8).with_auto(wpg_core::AutoCompute::Size {
                    field: "display_name".into(),
                }),
                FieldDef::new(
                    "display_name",
                    FieldType::Bytes {
                        len: Length::FromField {
                            field: "display_name_length".into(),
                            unit: 1,
                            adjust: 0,
                            div: 1,
                        },
                    },
                ),
                to_end("pad"),
            ],
        ),
    ];
    FieldType::Tlv(TlvDef {
        type_bits: 16,
        len_bits: 16,
        len_includes_header: false,
        len_unit: LengthUnit::Bytes,
        variants,
        unknown_as_raw: true,
        length: Length::FromField {
            field: "message_length".into(),
            unit: 1,
            adjust: -(HEADER_LEN + body),
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

/// Size in octets of a body made of `fields` (all fixed).
fn body_len(fields: &[FieldDef]) -> i64 {
    let bits: u32 = fields.iter().filter_map(|f| f.ty.fixed_bits()).sum();
    i64::from(bits / 8)
}

/// One message: header (`message_type`, `control_field`, `log_message_interval`
/// defaults), `body` and the TLVs (default items `tlv_items`).
fn message(
    id: &str,
    name: &str,
    head: (u64, u64, i64),
    body: Vec<FieldDef>,
    tlv_items: Vec<TlvItem>,
) -> ProtocolDef {
    let mut d = ProtocolDef::new(id, name, ProtocolLayer::L7);
    let n = body_len(&body);
    d.fields = header(head.0, head.1, head.2);
    d.fields.extend(body);
    d.fields.push(desc(
        FieldDef::new("tlvs", tlvs(n)).with_default(FieldValue::Tlv(tlv_items)),
        "ptp-tlvs",
    ));
    d.constraints = vec![Constraint::new(
        DiagLevel::Warning,
        ConstraintRule::FieldEquals {
            field: "version".into(),
            value: 2,
        },
    )];
    d
}

fn concat<const N: usize>(parts: [&[FieldDef]; N]) -> Vec<FieldDef> {
    parts.iter().flat_map(|p| p.iter().cloned()).collect()
}

/// Sync (message type 0): the origin timestamp (zero for two-step clocks, whose
/// `Follow_Up` carries the precise one).
#[must_use]
pub fn ptp_sync() -> ProtocolDef {
    message(
        "ptp_sync",
        "PTP Sync",
        (0x0, 0, 0),
        timestamp("origin_timestamp").to_vec(),
        Vec::new(),
    )
}

/// `Delay_Req` (message type 1): the origin timestamp.
#[must_use]
pub fn ptp_delay_req() -> ProtocolDef {
    message(
        "ptp_delay_req",
        "PTP Delay_Req",
        (0x1, 1, 0x7f),
        timestamp("origin_timestamp").to_vec(),
        Vec::new(),
    )
}

/// `Pdelay_Req` (message type 2): the origin timestamp and 10 reserved octets.
#[must_use]
pub fn ptp_pdelay_req() -> ProtocolDef {
    let mut body = timestamp("origin_timestamp").to_vec();
    body.push(octets("reserved", 10).with_default(FieldValue::Bytes(vec![0; 10])));
    message(
        "ptp_pdelay_req",
        "PTP Pdelay_Req",
        (0x2, 5, 0x7f),
        body,
        Vec::new(),
    )
}

/// `Pdelay_Resp` (message type 3): the request receipt timestamp and the
/// requesting port identity.
#[must_use]
pub fn ptp_pdelay_resp() -> ProtocolDef {
    message(
        "ptp_pdelay_resp",
        "PTP Pdelay_Resp",
        (0x3, 5, 0x7f),
        concat([
            &timestamp("request_receipt_timestamp"),
            &port_identity("requesting_port", 0x0200_00ff_fe00_0002, 1),
        ]),
        Vec::new(),
    )
}

/// `Follow_Up` (message type 8): the precise origin timestamp of the Sync with the
/// same sequence ID.
#[must_use]
pub fn ptp_follow_up() -> ProtocolDef {
    message(
        "ptp_follow_up",
        "PTP Follow_Up",
        (0x8, 2, 0),
        timestamp("precise_origin_timestamp").to_vec(),
        Vec::new(),
    )
}

/// `Delay_Resp` (message type 9): the receive timestamp of the `Delay_Req` and
/// the requesting port identity.
#[must_use]
pub fn ptp_delay_resp() -> ProtocolDef {
    message(
        "ptp_delay_resp",
        "PTP Delay_Resp",
        (0x9, 3, 0),
        concat([
            &timestamp("receive_timestamp"),
            &port_identity("requesting_port", 0x0200_00ff_fe00_0002, 1),
        ]),
        Vec::new(),
    )
}

/// `Pdelay_Resp_Follow_Up` (message type 10): the response origin timestamp and
/// the requesting port identity.
#[must_use]
pub fn ptp_pdelay_resp_follow_up() -> ProtocolDef {
    message(
        "ptp_pdelay_resp_follow_up",
        "PTP Pdelay_Resp_Follow_Up",
        (0xa, 5, 0x7f),
        concat([
            &timestamp("response_origin_timestamp"),
            &port_identity("requesting_port", 0x0200_00ff_fe00_0002, 1),
        ]),
        Vec::new(),
    )
}

/// Announce (message type 11): origin timestamp, current UTC offset and the
/// grandmaster's priorities, clock quality and identity, steps removed and time
/// source. Default: an unsynchronised free-running clock (class 248, priorities
/// 128, internal oscillator).
#[must_use]
pub fn ptp_announce() -> ProtocolDef {
    let mut body = timestamp("origin_timestamp").to_vec();
    body.extend([
        desc(int("current_utc_offset", 16, 37), "ptp-current-utc-offset"),
        uint("reserved", 8, 0),
        desc(
            uint("grandmaster_priority1", 8, 128),
            "ptp-grandmaster-priority",
        ),
        desc(
            uint("grandmaster_clock_class", 8, 248),
            "ptp-grandmaster-clock-class",
        ),
        desc(
            enum_field("grandmaster_clock_accuracy", 8, CLOCK_ACCURACY)
                .with_default(FieldValue::UInt(0xfe)),
            "ptp-grandmaster-clock-accuracy",
        ),
        desc(
            uint("grandmaster_offset_scaled_log_variance", 16, 0xffff),
            "ptp-grandmaster-variance",
        ),
        desc(
            uint("grandmaster_priority2", 8, 128),
            "ptp-grandmaster-priority",
        ),
        desc(
            uint("grandmaster_identity", 64, 0x0200_00ff_fe00_0001),
            "ptp-grandmaster-identity",
        ),
        desc(uint("steps_removed", 16, 0), "ptp-steps-removed"),
        desc(
            enum_field("time_source", 8, TIME_SOURCES).with_default(FieldValue::UInt(0xa0)),
            "ptp-time-source",
        ),
    ]);
    message(
        "ptp_announce",
        "PTP Announce",
        (0xb, 5, 1),
        body,
        Vec::new(),
    )
}

/// Signaling (message type 12): the target port identity (all ones = every port)
/// and the signaling TLVs. Default: a unicast request for Announce messages
/// every 2 s for 300 s.
#[must_use]
pub fn ptp_signaling() -> ProtocolDef {
    message(
        "ptp_signaling",
        "PTP Signaling",
        (0xc, 5, 0x7f),
        port_identity("target_port", u64::MAX, 0xffff).to_vec(),
        vec![TlvItem::named("request_unicast_transmission")],
    )
}

/// Management (message type 13): target port identity, boundary hops, the action
/// and one management TLV. Default: GET of the default data set.
#[must_use]
pub fn ptp_management() -> ProtocolDef {
    let mut body = port_identity("target_port", u64::MAX, 0xffff).to_vec();
    body.extend([
        desc(
            uint("starting_boundary_hops", 8, 1),
            "ptp-starting-boundary-hops",
        ),
        desc(uint("boundary_hops", 8, 1), "ptp-boundary-hops"),
        uint("reserved", 4, 0),
        desc(
            enum_field(
                "action",
                4,
                &[
                    (0, "get"),
                    (1, "set"),
                    (2, "response"),
                    (3, "command"),
                    (4, "acknowledge"),
                ],
            )
            .with_default(FieldValue::UInt(0)),
            "ptp-action",
        ),
        uint("reserved2", 8, 0),
    ]);
    message(
        "ptp_management",
        "PTP Management",
        (0xd, 4, 0x7f),
        body,
        vec![TlvItem::named("management")],
    )
}

/// Generic PTP message: the common header and the rest of the message (up to
/// `message_length`) as octets.
/// The parser re-decodes it as the dedicated message of its `message_type`.
#[must_use]
pub fn ptp() -> ProtocolDef {
    let mut d = ProtocolDef::new(PTP, "PTP", ProtocolLayer::L7);
    d.fields = header(0x0, 0, 0);
    d.fields.push(desc(
        FieldDef::new(
            "data",
            FieldType::Bytes {
                len: Length::FromField {
                    field: "message_length".into(),
                    unit: 1,
                    adjust: -HEADER_LEN,
                    div: 1,
                },
            },
        )
        .with_default(FieldValue::Bytes(vec![0; 10])),
        "ptp-data",
    ));
    d.next = Some(Binding::new(
        "message_type",
        MESSAGES
            .iter()
            .map(|(t, p, _)| BindingEntry::new(*t, *p).reparsing())
            .collect(),
    ));
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitions_are_valid() {
        for d in [
            ptp(),
            ptp_sync(),
            ptp_delay_req(),
            ptp_pdelay_req(),
            ptp_pdelay_resp(),
            ptp_follow_up(),
            ptp_delay_resp(),
            ptp_pdelay_resp_follow_up(),
            ptp_announce(),
            ptp_signaling(),
            ptp_management(),
        ] {
            d.validate().unwrap_or_else(|e| panic!("{}: {e}", d.id));
        }
        // Header 34 octets; Announce 64 without TLVs.
        assert_eq!(body_len(&header(0, 0, 0)), HEADER_LEN);
        let announce = ptp_announce();
        let fixed: u32 = announce
            .fields
            .iter()
            .filter_map(|f| f.ty.fixed_bits())
            .sum();
        assert_eq!(fixed, 64 * 8);
        assert_eq!(bindings(PTP_EVENT_PORT, Some(true)).len(), 5);
    }
}
