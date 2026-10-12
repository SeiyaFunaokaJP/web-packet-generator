//! Ethernet in the First Mile OAM, IEEE 802.3 clause 57 (802.3ah) (TODO §2.1 P2,
//! M7).
//!
//! OAMPDUs are Slow Protocols frames (EtherType `0x8809`, subtype `0x03`) sent to
//! `01-80-c2-00-00-02`. Every OAMPDU is one layer: subtype, flags, code and the
//! code-specific data. The parser decodes it as the generic [`efm_oam`] first,
//! whose `code` then re-decodes the layer as the dedicated OAMPDU
//! (`efm_oam_information`, `efm_oam_event_notification`, ...).
//!
//! Information and Event TLVs have an 8-bit type and an 8-bit length counting the
//! whole TLV (Auto); they end with the End TLV (type 0), after which the frame is
//! zero padded. Variable descriptors / containers of Variable Request / Response
//! OAMPDUs are kept as octets (branch, leaf, and for containers width and value,
//! closed by a zero branch).

use wpg_core::{
    Binding, BindingEntry, EnumVariant, FieldDef, FieldType, FieldValue, FlagBit, Length,
    LengthUnit, ProtocolDef, ProtocolLayer, TlvDef, TlvItem, TlvVariant,
};

/// Id of the generic OAMPDU ([`efm_oam`]).
pub const EFM_OAM: &str = "efm_oam";

/// Slow Protocols subtype of OAM.
pub const EFM_OAM_SUBTYPE: u64 = 0x03;

/// The dedicated OAMPDUs: (code, protocol id).
pub const PDUS: [(u64, &str); 6] = [
    (0x00, "efm_oam_information"),
    (0x01, "efm_oam_event_notification"),
    (0x02, "efm_oam_variable_request"),
    (0x03, "efm_oam_variable_response"),
    (0x04, "efm_oam_loopback_control"),
    (0xfe, "efm_oam_organization_specific"),
];

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

fn flags(name: &str, bits: u8, list: &[(u8, &str)]) -> FieldDef {
    FieldDef::new(
        name,
        FieldType::Flags {
            bits,
            flags: list
                .iter()
                .map(|(bit, n)| FlagBit {
                    bit: *bit,
                    name: (*n).into(),
                })
                .collect(),
        },
    )
}

fn desc(f: FieldDef, key: &str) -> FieldDef {
    f.with_description(key)
}

/// Information / Event TLVs: 8-bit type, 8-bit length including the header,
/// closed by the End TLV (type 0).
fn tlv_def(mut variants: Vec<TlvVariant>) -> FieldType {
    variants.insert(0, TlvVariant::type_only(0x00, "end"));
    variants.push(TlvVariant::new(
        0xfe,
        "organization_specific",
        vec![u("oui", 24), to_end("value")],
    ));
    FieldType::Tlv(TlvDef {
        type_bits: 8,
        len_bits: 8,
        len_includes_header: true,
        len_unit: LengthUnit::Bytes,
        variants,
        unknown_as_raw: true,
        length: Length::UntilTlv { type_value: 0 },
        fill: None,
        head_octets: 0,
        align: 0,
        unpadded_last: false,
        long_length: None,
        len_first: false,
        uncounted_octets: 0,
    })
}

/// Local / Remote Information TLV body (802.3 §57.5.2.1 / 57.5.2.2).
fn information(revision: u64) -> Vec<FieldDef> {
    vec![
        uint("oam_version", 8, 1),
        uint("revision", 16, revision),
        uint("state_reserved", 5, 0),
        enum_field("multiplexer_action", 1, &[(0, "forward"), (1, "discard")])
            .with_default(FieldValue::UInt(0)),
        enum_field(
            "parser_action",
            2,
            &[(0, "forward"), (1, "loopback"), (2, "discard")],
        )
        .with_default(FieldValue::UInt(0)),
        flags(
            "oam_configuration",
            8,
            &[
                (0, "active_mode"),
                (1, "unidirectional"),
                (2, "remote_loopback"),
                (3, "link_events"),
                (4, "variable_retrieval"),
            ],
        )
        .with_default(FieldValue::UInt(0x09)),
        uint("oampdu_configuration_reserved", 5, 0),
        uint("max_oampdu_size", 11, 1518),
        uint("oui", 24, 0),
        uint("vendor_specific_information", 32, 0),
    ]
}

/// Information TLVs: Local / Remote Information, Organization Specific, End.
#[must_use]
pub fn information_tlvs() -> FieldType {
    tlv_def(vec![
        TlvVariant::new(0x01, "local_information", information(0)),
        TlvVariant::new(0x02, "remote_information", information(0)),
    ])
}

/// Link event TLVs (802.3 §57.5.3): errored symbol period, errored frame, errored
/// frame period and errored frame seconds summary.
#[must_use]
pub fn event_tlvs() -> FieldType {
    let ev = |window: u8, threshold: u8, errors: u8, total: u8| {
        vec![
            uint("timestamp", 16, 0),
            uint("window", window, 10),
            uint("threshold", threshold, 1),
            uint("errors", errors, 1),
            uint("error_running_total", total, 1),
            uint("event_running_total", 32, 1),
        ]
    };
    tlv_def(vec![
        TlvVariant::new(0x01, "errored_symbol_period", ev(64, 64, 64, 64)),
        TlvVariant::new(0x02, "errored_frame", ev(16, 32, 32, 64)),
        TlvVariant::new(0x03, "errored_frame_period", ev(32, 32, 32, 64)),
        TlvVariant::new(0x04, "errored_frame_seconds_summary", ev(16, 16, 16, 32)),
    ])
}

const CODES: &[(u64, &str)] = &[
    (0x00, "information"),
    (0x01, "event_notification"),
    (0x02, "variable_request"),
    (0x03, "variable_response"),
    (0x04, "loopback_control"),
    (0xfe, "organization_specific"),
];

/// OAMPDU header: subtype, flags and code.
fn header(code: u64) -> Vec<FieldDef> {
    vec![
        desc(uint("subtype", 8, EFM_OAM_SUBTYPE), "efm-oam-subtype"),
        desc(
            flags(
                "flags",
                16,
                &[
                    (0, "link_fault"),
                    (1, "dying_gasp"),
                    (2, "critical_event"),
                    (3, "local_evaluating"),
                    (4, "local_stable"),
                    (5, "remote_evaluating"),
                    (6, "remote_stable"),
                ],
            )
            .with_default(FieldValue::UInt(0x0050)),
            "efm-oam-flags",
        ),
        desc(
            enum_field("code", 8, CODES).with_default(FieldValue::UInt(code)),
            "efm-oam-code",
        ),
    ]
}

fn pdu(id: &str, name: &str, code: u64, body: Vec<FieldDef>) -> ProtocolDef {
    let mut d = ProtocolDef::new(id, name, ProtocolLayer::L2);
    d.fields = header(code);
    d.fields.extend(body);
    d
}

/// Information OAMPDU (code 0x00): Local / Remote Information TLVs. Default: the
/// Local Information TLV of an active DTE with link events, then End.
#[must_use]
pub fn efm_oam_information() -> ProtocolDef {
    pdu(
        "efm_oam_information",
        "802.3ah OAM Information",
        0x00,
        vec![desc(
            FieldDef::new("tlvs", information_tlvs()).with_default(FieldValue::Tlv(vec![
                TlvItem::named("local_information"),
                TlvItem::named("end"),
            ])),
            "efm-oam-information-tlvs",
        )],
    )
}

/// Event Notification OAMPDU (code 0x01): sequence number and link event TLVs.
/// Default: one Errored Frame event, then End.
#[must_use]
pub fn efm_oam_event_notification() -> ProtocolDef {
    pdu(
        "efm_oam_event_notification",
        "802.3ah OAM Event Notification",
        0x01,
        vec![
            desc(uint("sequence_number", 16, 1), "efm-oam-sequence-number"),
            desc(
                FieldDef::new("tlvs", event_tlvs()).with_default(FieldValue::Tlv(vec![
                    TlvItem::named("errored_frame"),
                    TlvItem::named("end"),
                ])),
                "efm-oam-event-tlvs",
            ),
        ],
    )
}

/// Variable Request OAMPDU (code 0x02): variable descriptors (branch, leaf) as
/// octets. Default: `aFramesTransmittedOK` (branch 0x07, leaf 0x0002), then the
/// closing zero branch.
#[must_use]
pub fn efm_oam_variable_request() -> ProtocolDef {
    pdu(
        "efm_oam_variable_request",
        "802.3ah OAM Variable Request",
        0x02,
        vec![desc(
            to_end("variable_descriptors").with_default(FieldValue::Bytes(vec![0x07, 0, 0x02, 0])),
            "efm-oam-variable-descriptors",
        )],
    )
}

/// Variable Response OAMPDU (code 0x03): variable containers (branch, leaf,
/// width, value) as octets. Default: `aFramesTransmittedOK` = 42 (4 octets), then
/// the closing zero branch.
#[must_use]
pub fn efm_oam_variable_response() -> ProtocolDef {
    pdu(
        "efm_oam_variable_response",
        "802.3ah OAM Variable Response",
        0x03,
        vec![desc(
            to_end("variable_containers")
                .with_default(FieldValue::Bytes(vec![0x07, 0, 0x02, 0x04, 0, 0, 0, 42, 0])),
            "efm-oam-variable-containers",
        )],
    )
}

/// Loopback Control OAMPDU (code 0x04): the remote loopback command.
#[must_use]
pub fn efm_oam_loopback_control() -> ProtocolDef {
    pdu(
        "efm_oam_loopback_control",
        "802.3ah OAM Loopback Control",
        0x04,
        vec![desc(
            enum_field(
                "remote_loopback_command",
                8,
                &[(1, "enable"), (2, "disable")],
            )
            .with_default(FieldValue::UInt(1)),
            "efm-oam-loopback-command",
        )],
    )
}

/// Organization Specific OAMPDU (code 0xfe): OUI and data.
#[must_use]
pub fn efm_oam_organization_specific() -> ProtocolDef {
    pdu(
        "efm_oam_organization_specific",
        "802.3ah OAM Organization Specific",
        0xfe,
        vec![
            desc(uint("oui", 24, 0), "efm-oam-oui"),
            desc(
                to_end("data").with_default(FieldValue::Bytes(Vec::new())),
                "efm-oam-data",
            ),
        ],
    )
}

/// Generic OAMPDU: header and the rest of the PDU as octets. The parser re-decodes
/// it as the dedicated OAMPDU of its `code`.
#[must_use]
pub fn efm_oam() -> ProtocolDef {
    let mut d = pdu(
        EFM_OAM,
        "802.3ah OAM",
        0x00,
        vec![desc(
            to_end("data").with_default(FieldValue::Bytes(Vec::new())),
            "efm-oam-data",
        )],
    );
    d.next = Some(Binding::new(
        "code",
        PDUS.iter()
            .map(|(c, p)| BindingEntry::new(*c, *p).reparsing())
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
            efm_oam(),
            efm_oam_information(),
            efm_oam_event_notification(),
            efm_oam_variable_request(),
            efm_oam_variable_response(),
            efm_oam_loopback_control(),
            efm_oam_organization_specific(),
        ] {
            d.validate().unwrap_or_else(|e| panic!("{}: {e}", d.id));
        }
        // Local Information TLV value: 14 octets (16 with the header).
        let bits: u32 = information(0)
            .iter()
            .filter_map(|f| f.ty.fixed_bits())
            .sum();
        assert_eq!(bits, 14 * 8);
    }
}
