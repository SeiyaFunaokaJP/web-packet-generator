//! GTPv2-C (3GPP TS 29.274) messages and information elements (spec §2.1 P2, M7).
//!
//! One [`gtpv2`] layer holds one message: the header (version 2, piggybacking,
//! TEID presence and message priority flags, message type, length, optional TEID,
//! sequence number) and its information elements ([`ies`]).
//!
//! Every IE is an 8-bit type, a 16-bit length and a spare / instance octet that
//! the length does not count, followed by the IE data. Main IEs are typed (IMSI,
//! Cause, Recovery, APN, AMBR, EBI, IP Address, MEI, MSISDN, PAA, Bearer `QoS`, RAT
//! Type, Serving Network, F-TEID, PDN Type, Charging ID, APN Restriction, Selection
//! Mode, ...); Bearer Context is a grouped IE whose data is IEs again (one level
//! deep). Unknown IEs keep their octets.
//!
//! Reached through UDP 2123. Auto fields: the T flag (set when a TEID is given),
//! the message length and every IE length. The IEs end at the message length, so
//! a piggybacked message (P flag) follows as the next `gtpv2` layer. GTPv1-C
//! (version 1 on the same port) is not modelled.

use wpg_core::{
    protocol, BindingEntry, EnumVariant, FieldDef, FieldType, FieldValue, ItemDef, Length,
    LengthUnit, ProtocolDef, TlvDef, TlvItem, TlvVariant,
};

/// Id of the GTPv2-C message ([`gtpv2`]).
pub const GTPV2: &str = "gtpv2";

/// UDP port of GTP-C (TS 29.274 §4.4.2.2).
pub const GTPC_PORT: u64 = 2123;

fn u(name: &str, bits: u8) -> FieldDef {
    FieldDef::new(name, FieldType::UInt { bits })
}

fn uint(name: &str, bits: u8, default: u64) -> FieldDef {
    u(name, bits).with_default(FieldValue::UInt(default))
}

fn rest(name: &str) -> FieldDef {
    FieldDef::new(name, FieldType::Bytes { len: Length::ToEnd })
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

fn enum_field(name: &str, bits: u8, list: &[(u64, &str)], default: u64) -> FieldDef {
    FieldDef::new(name, enum_type(bits, list)).with_default(FieldValue::UInt(default))
}

/// The IMSI 001010123456789 in TBCD (two digits per octet, low nibble first,
/// `f` filler).
pub const DEFAULT_IMSI: [u8; 8] = [0x00, 0x01, 0x01, 0x21, 0x43, 0x65, 0x87, 0xf9];

/// The APN `internet` as DNS labels.
pub const DEFAULT_APN: &[u8] = b"\x08internet";

/// Message types (TS 29.274 §6.1).
const MESSAGE_TYPES: &[(u64, &str)] = &[
    (1, "echo_request"),
    (2, "echo_response"),
    (3, "version_not_supported"),
    (32, "create_session_request"),
    (33, "create_session_response"),
    (34, "modify_bearer_request"),
    (35, "modify_bearer_response"),
    (36, "delete_session_request"),
    (37, "delete_session_response"),
    (95, "create_bearer_request"),
    (96, "create_bearer_response"),
    (97, "update_bearer_request"),
    (98, "update_bearer_response"),
    (99, "delete_bearer_request"),
    (100, "delete_bearer_response"),
    (170, "release_access_bearers_request"),
    (171, "release_access_bearers_response"),
    (176, "downlink_data_notification"),
    (177, "downlink_data_notification_ack"),
];

/// Cause values (TS 29.274 §8.4), the common ones.
const CAUSES: &[(u64, &str)] = &[
    (2, "local_detach"),
    (3, "complete_detach"),
    (4, "rat_changed_from_3gpp_to_non_3gpp"),
    (8, "isr_deactivation"),
    (16, "request_accepted"),
    (17, "request_accepted_partially"),
    (18, "new_pdn_type_due_to_network_preference"),
    (19, "new_pdn_type_due_to_single_address_bearer_only"),
    (64, "context_not_found"),
    (65, "invalid_message_format"),
    (66, "version_not_supported_by_next_peer"),
    (67, "invalid_length"),
    (68, "service_not_supported"),
    (69, "mandatory_ie_incorrect"),
    (70, "mandatory_ie_missing"),
    (72, "system_failure"),
    (73, "no_resources_available"),
    (78, "missing_or_unknown_apn"),
    (83, "preferred_pdn_type_not_supported"),
    (84, "all_dynamic_addresses_are_occupied"),
    (89, "service_denied"),
    (92, "user_authentication_failed"),
    (93, "apn_access_denied_no_subscription"),
    (94, "request_rejected"),
    (100, "remote_peer_not_responding"),
    (110, "temporarily_rejected_due_to_handover_in_progress"),
];

/// Interface types of an F-TEID (TS 29.274 §8.22), the common ones.
const INTERFACE_TYPES: &[(u64, &str)] = &[
    (0, "s1_u_enodeb_gtp_u"),
    (1, "s1_u_sgw_gtp_u"),
    (2, "s12_rnc_gtp_u"),
    (3, "s12_sgw_gtp_u"),
    (4, "s5_s8_sgw_gtp_u"),
    (5, "s5_s8_pgw_gtp_u"),
    (6, "s5_s8_sgw_gtp_c"),
    (7, "s5_s8_pgw_gtp_c"),
    (10, "s11_mme_gtp_c"),
    (11, "s11_s4_sgw_gtp_c"),
    (16, "s2b_epdg_gtp_c"),
    (17, "s2b_u_epdg_gtp_u"),
    (18, "s2b_pgw_gtp_c"),
    (19, "s2b_u_pgw_gtp_u"),
];

/// RAT types (TS 29.274 §8.17).
const RAT_TYPES: &[(u64, &str)] = &[
    (1, "utran"),
    (2, "geran"),
    (3, "wlan"),
    (4, "gan"),
    (5, "hspa_evolution"),
    (6, "eutran"),
    (7, "virtual"),
    (8, "eutran_nb_iot"),
    (9, "lte_m"),
    (10, "nr"),
];

/// PDN types (TS 29.274 §8.34).
const PDN_TYPES: &[(u64, &str)] = &[
    (1, "ipv4"),
    (2, "ipv6"),
    (3, "ipv4v6"),
    (4, "non_ip"),
    (5, "ethernet"),
];

/// An IE variant: the spare / instance octet, then the IE data `fields`.
fn ie(code: u64, name: &str, fields: Vec<FieldDef>) -> TlvVariant {
    let mut all = vec![uint("spare", 4, 0), uint("instance", 4, 0)];
    all.extend(fields);
    TlvVariant::new(code, name, all)
}

/// The F-TEID data (TS 29.274 §8.22): V4 / V6 flags, interface type, TEID / GRE
/// key and the addresses the flags announce.
fn fteid(name: &str, v4: bool, v6: bool) -> TlvVariant {
    let mut fields = vec![
        uint("v4", 1, u64::from(v4)),
        uint("v6", 1, u64::from(v6)),
        enum_field("interface_type", 6, INTERFACE_TYPES, 10),
        uint("teid", 32, 1),
    ];
    if v4 {
        fields.push(
            FieldDef::new("ipv4", FieldType::Ipv4)
                .with_default(FieldValue::Ipv4([192, 0, 2, 1].into())),
        );
    }
    if v6 {
        fields.push(
            FieldDef::new("ipv6", FieldType::Ipv6)
                .with_default(FieldValue::Ipv6("2001:db8::1".parse().unwrap())),
        );
    }
    // Value octets: the instance octet, the flags octet, the TEID and addresses.
    let len = 6 + if v4 { 4 } else { 0 } + if v6 { 16 } else { 0 };
    ie(87, name, fields).when_value_len(len)
}

/// The PDN Address Allocation data (TS 29.274 §8.14) for one PDN type.
fn paa(name: &str, pdn_type: u64) -> TlvVariant {
    let mut fields = vec![uint("spare2", 5, 0), uint("pdn_type", 3, pdn_type)];
    let mut len = 2;
    if pdn_type != 1 {
        fields.push(uint("ipv6_prefix_length", 8, 64));
        fields.push(
            FieldDef::new("ipv6", FieldType::Ipv6)
                .with_default(FieldValue::Ipv6(std::net::Ipv6Addr::UNSPECIFIED)),
        );
        len += 17;
    }
    if pdn_type != 2 {
        fields.push(FieldDef::new("ipv4", FieldType::Ipv4));
        len += 4;
    }
    ie(79, name, fields).when_value_len(len)
}

/// Bit rate in kbps (40 bits) for the Bearer `QoS` IE.
fn rate(name: &str) -> FieldDef {
    uint(name, 40, 0)
}

/// GTPv2-C IEs (TS 29.274 §8): 8-bit type, 16-bit length of the IE data, a spare /
/// instance octet not counted by the length. With `grouped`, Bearer Context (93)
/// holds IEs again (without a further Bearer Context).
#[must_use]
pub fn ies(grouped: bool) -> FieldType {
    let mut variants = vec![
        ie(1, "imsi", vec![rest("digits")]),
        ie(
            2,
            "cause",
            vec![
                enum_field("cause", 8, CAUSES, 16),
                uint("spare2", 5, 0),
                uint("pce", 1, 0),
                uint("bce", 1, 0),
                uint("cs", 1, 0),
                rest("offending_ie"),
            ],
        ),
        ie(3, "recovery", vec![uint("restart_counter", 8, 0)]),
        ie(71, "apn", vec![rest("apn")]),
        ie(
            72,
            "ambr",
            vec![uint("uplink", 32, 1000), uint("downlink", 32, 1000)],
        ),
        ie(73, "ebi", vec![uint("spare2", 4, 0), uint("ebi", 4, 5)]),
        ie(
            74,
            "ip_address",
            vec![FieldDef::new("address", FieldType::Ipv4)],
        )
        .when_value_len(5),
        ie(
            74,
            "ip_address_ipv6",
            vec![FieldDef::new("address", FieldType::Ipv6)],
        )
        .when_value_len(17),
        ie(75, "mei", vec![rest("digits")]),
        ie(76, "msisdn", vec![rest("digits")]),
        ie(77, "indication", vec![rest("flags")]),
        ie(78, "pco", vec![rest("data")]),
        paa("paa", 1),
        paa("paa_ipv6", 2),
        paa("paa_ipv4v6", 3),
        ie(
            80,
            "bearer_qos",
            vec![
                uint("spare2", 1, 0),
                uint("pci", 1, 0),
                uint("pl", 4, 15),
                uint("spare3", 1, 0),
                uint("pvi", 1, 0),
                uint("qci", 8, 9),
                rate("mbr_uplink"),
                rate("mbr_downlink"),
                rate("gbr_uplink"),
                rate("gbr_downlink"),
            ],
        ),
        ie(
            82,
            "rat_type",
            vec![enum_field("rat_type", 8, RAT_TYPES, 6)],
        ),
        ie(
            83,
            "serving_network",
            vec![
                uint("mcc_digit2", 4, 0),
                uint("mcc_digit1", 4, 0),
                uint("mnc_digit3", 4, 0xf),
                uint("mcc_digit3", 4, 1),
                uint("mnc_digit2", 4, 1),
                uint("mnc_digit1", 4, 0),
            ],
        ),
        ie(84, "bearer_tft", vec![rest("tft")]),
        ie(86, "uli", vec![rest("data")]),
        fteid("fteid", true, false),
        fteid("fteid_ipv6", false, true),
        fteid("fteid_ipv4v6", true, true),
        ie(94, "charging_id", vec![uint("charging_id", 32, 1)]),
        ie(
            95,
            "charging_characteristics",
            vec![uint("value", 16, 0x0800)],
        ),
        ie(
            99,
            "pdn_type",
            vec![
                uint("spare2", 5, 0),
                enum_field("pdn_type", 3, PDN_TYPES, 1),
            ],
        ),
        ie(
            114,
            "ue_time_zone",
            vec![
                uint("time_zone", 8, 0),
                uint("spare2", 6, 0),
                uint("daylight_saving_time", 2, 0),
            ],
        ),
        ie(127, "apn_restriction", vec![uint("restriction", 8, 0)]),
        ie(
            128,
            "selection_mode",
            vec![uint("spare2", 6, 0), uint("selection_mode", 2, 0)],
        ),
        ie(
            255,
            "private_extension",
            vec![uint("enterprise_id", 16, 0), rest("value")],
        ),
    ];
    if grouped {
        variants.push(ie(
            93,
            "bearer_context",
            vec![FieldDef::new("ies", ies(false))
                .with_default(FieldValue::Tlv(vec![TlvItem::named("ebi")]))],
        ));
    }
    FieldType::Tlv(TlvDef {
        type_bits: 8,
        len_bits: 16,
        len_includes_header: false,
        len_unit: LengthUnit::Bytes,
        variants,
        unknown_as_raw: true,
        length: if grouped {
            // From octet 5 (after the length field) to the message length.
            Length::UntilOffset {
                field: "message_length".into(),
                unit: 1,
                adjust: 4,
            }
        } else {
            Length::ToEnd
        },
        fill: None,
        head_octets: 0,
        align: 0,
        unpadded_last: false,
        long_length: None,
        len_first: false,
        uncounted_octets: 1,
    })
}

/// The TEID, present when the T flag is set (one element).
fn teid() -> FieldType {
    FieldType::List {
        item: ItemDef::Field {
            ty: Box::new(FieldType::UInt { bits: 32 }),
        },
        length: Length::Count { field: "t".into() },
    }
}

/// GTPv2-C message (TS 29.274 §5.1): version 2, P (piggybacked message follows),
/// T (Auto: TEID present), MP (message priority present), the message type, the
/// message length (Auto, octets after the first four), the TEID, the sequence
/// number, the message priority and the IEs ([`ies`]).
///
/// Default: a Create Session Request on TEID 0 (sequence number 1) with IMSI
/// 001010123456789, RAT type E-UTRAN, the sender's S11 F-TEID, APN `internet`,
/// PDN type IPv4 and one bearer context (EBI 5, QCI 9).
#[must_use]
pub fn gtpv2() -> ProtocolDef {
    let default_ies = vec![
        TlvItem::named("imsi").with("digits", DEFAULT_IMSI.to_vec()),
        TlvItem::named("rat_type"),
        TlvItem::named("fteid"),
        TlvItem::named("apn").with("apn", DEFAULT_APN.to_vec()),
        TlvItem::named("pdn_type"),
        TlvItem::named("bearer_context").with(
            "ies",
            vec![TlvItem::named("ebi"), TlvItem::named("bearer_qos")],
        ),
    ];
    let mut d = protocol! {
        id: "gtpv2",
        name: "GTPv2-C",
        layer: L7,
        fields: {
            version: uint(3) = 2, desc = "gtpv2-version";
            p: uint(1) = 0, desc = "gtpv2-p";
            t: uint(1), auto = count(teid), desc = "gtpv2-t";
            mp: uint(1) = 0, desc = "gtpv2-mp";
            spare: uint(2) = 0;
            message_type: ty(enum_type(8, MESSAGE_TYPES)) = FieldValue::UInt(32), desc = "gtpv2-message-type";
            message_length: uint(16), auto = length(header, adjust = -4), desc = "gtpv2-length";
            teid: ty(teid()) = FieldValue::List(vec![FieldValue::UInt(0)]), desc = "gtpv2-teid";
            sequence_number: uint(24) = 1, desc = "gtpv2-sequence";
            message_priority: uint(4) = 0, desc = "gtpv2-message-priority";
            spare2: uint(4) = 0;
            ies: ty(ies(true)) = FieldValue::from(default_ies), desc = "gtpv2-ies";
        },
        next: p {},
        constraints: {
            warning: equals(version, 2);
            error: min_length(header, 8);
        },
    };
    if let Some(b) = d.next.as_mut() {
        // A piggybacked message (TS 29.274 §5.5.1) follows the IEs.
        b.table.push(BindingEntry::new(1, GTPV2).with_min_len(8));
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gtpv2_is_valid() {
        let d = gtpv2();
        d.validate().unwrap();
        assert_eq!(d.id, GTPV2);
    }
}
