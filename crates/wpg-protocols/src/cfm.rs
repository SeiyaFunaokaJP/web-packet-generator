//! Ethernet CFM / OAM: IEEE 802.1ag (802.1Q clause 21), ITU-T Y.1731 and the
//! ERPS R-APS messages of ITU-T G.8032 (TODO §2.1 P2, M7).
//!
//! Every CFM PDU is one layer: the 4-octet common header (MD / MEG level,
//! version, opcode, flags, First TLV Offset), the opcode-specific fields and the
//! TLVs, closed by the End TLV. CFM is reached through EtherType `0x8902`
//! (directly, or behind 802.1Q / 802.1ad tags). The parser decodes it as the
//! generic [`cfm`] first, whose `opcode` then re-decodes the layer as the
//! dedicated PDU (`cfm_ccm`, `cfm_lbm`, `cfm_dmm`, ...), like IS-IS PDUs.
//!
//! The flags are split per opcode (CCM RDI and period, LTM / LTR bits, the
//! AIS / LCK / CSF period and CSF type). `first_tlv_offset` is constant for
//! each opcode (a constraint warns when it is changed). The TLVs end with the
//! End TLV (type 0); zero padding after it belongs to the Ethernet frame.
//! TLV lengths are Auto.
//!
//! R-APS (opcode 40) carries the request / state, sub-code, the RB / DNF / BPR
//! status bits and the node ID; it is sent to `01-19-a7-00-00-<ring ID>`.
//! The MAID / MEG ID of the CCM is the 48-octet field as sent (the default is a
//! Y.1731 ICC-based MEG ID); the CRC-32 of a Test TLV is not computed.

use wpg_core::{
    Binding, BindingEntry, Constraint, ConstraintRule, DiagLevel, EnumVariant, FieldDef, FieldType,
    FieldValue, Length, LengthUnit, ProtocolDef, ProtocolLayer, TlvDef, TlvItem, TlvVariant,
};

/// Id of the generic CFM PDU ([`cfm`]).
pub const CFM: &str = "cfm";

/// EtherType of CFM (IEEE 802.1Q clause 21, Y.1731).
pub const CFM_ETHERTYPE: u64 = 0x8902;

/// The dedicated PDUs: (opcode, protocol id).
pub const PDUS: [(u64, &str); 18] = [
    (1, "cfm_ccm"),
    (2, "cfm_lbr"),
    (3, "cfm_lbm"),
    (4, "cfm_ltr"),
    (5, "cfm_ltm"),
    (33, "cfm_ais"),
    (35, "cfm_lck"),
    (37, "cfm_tst"),
    (40, "cfm_raps"),
    (42, "cfm_lmr"),
    (43, "cfm_lmm"),
    (45, "cfm_1dm"),
    (46, "cfm_dmr"),
    (47, "cfm_dmm"),
    (52, "cfm_csf"),
    (53, "cfm_1sl"),
    (54, "cfm_slr"),
    (55, "cfm_slm"),
];

/// Binding entries announcing CFM under EtherType `value`: the generic [`cfm`]
/// first (the parser takes it), then the dedicated PDUs.
#[must_use]
pub fn bindings(value: u64) -> Vec<BindingEntry> {
    std::iter::once(CFM)
        .chain(PDUS.iter().map(|p| p.1))
        .map(|p| BindingEntry::new(value, p))
        .collect()
}

const OPCODES: &[(u64, &str)] = &[
    (1, "ccm"),
    (2, "lbr"),
    (3, "lbm"),
    (4, "ltr"),
    (5, "ltm"),
    (33, "ais"),
    (35, "lck"),
    (37, "tst"),
    (39, "linear_aps"),
    (40, "ring_aps"),
    (41, "mcc"),
    (42, "lmr"),
    (43, "lmm"),
    (45, "one_dm"),
    (46, "dmr"),
    (47, "dmm"),
    (48, "exr"),
    (49, "exm"),
    (50, "vsr"),
    (51, "vsm"),
    (52, "csf"),
    (53, "one_sl"),
    (54, "slr"),
    (55, "slm"),
];

/// CCM / AIS / LCK / CSF transmission periods (802.1Q Table 21-15, Y.1731).
const PERIODS: &[(u64, &str)] = &[
    (0, "invalid"),
    (1, "ms_3_33"),
    (2, "ms_10"),
    (3, "ms_100"),
    (4, "s_1"),
    (5, "s_10"),
    (6, "min_1"),
    (7, "min_10"),
];

/// Default MAID: Y.1731 ICC-based MEG ID (no MD name, format 32, length 13,
/// ICC `WPG001` + UMC `MEG0001`), zero padded to 48 octets.
const MEG_ID: &[u8] = b"\x01\x20\x0dWPG001MEG0001";

fn u(name: &str, bits: u8) -> FieldDef {
    FieldDef::new(name, FieldType::UInt { bits })
}

fn uint(name: &str, bits: u8, default: u64) -> FieldDef {
    u(name, bits).with_default(FieldValue::UInt(default))
}

fn to_end(name: &str) -> FieldDef {
    FieldDef::new(name, FieldType::Bytes { len: Length::ToEnd })
}

fn mac(name: &str, last: u8) -> FieldDef {
    FieldDef::new(name, FieldType::Mac).with_default(FieldValue::Mac([2, 0, 0, 0, 0, last]))
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

fn sized(len: &str, name: &str) -> [FieldDef; 2] {
    [
        u(len, 8).with_auto(wpg_core::AutoCompute::Size { field: name.into() }),
        FieldDef::new(
            name,
            FieldType::Bytes {
                len: Length::FromField {
                    field: len.into(),
                    unit: 1,
                    adjust: 0,
                    div: 1,
                },
            },
        ),
    ]
}

/// CFM TLVs (802.1Q §21.5, Y.1731 §9): 8-bit type, 16-bit length of the value,
/// closed by the End TLV (type 0, no length).
#[must_use]
pub fn tlvs() -> FieldType {
    let [cid_len, cid] = sized("chassis_id_length", "chassis_id");
    let action = |name: &str| {
        enum_field(
            name,
            8,
            &[(1, "ok"), (2, "down"), (3, "blocked"), (4, "vid")],
        )
        .with_default(FieldValue::UInt(1))
    };
    let variants = vec![
        TlvVariant::type_only(0, "end"),
        TlvVariant::new(
            1,
            "sender_id",
            vec![
                cid_len,
                enum_field(
                    "chassis_id_subtype",
                    8,
                    &[
                        (1, "chassis_component"),
                        (2, "interface_alias"),
                        (3, "port_component"),
                        (4, "mac_address"),
                        (5, "network_address"),
                        (6, "interface_name"),
                        (7, "local"),
                    ],
                )
                .with_default(FieldValue::UInt(4)),
                cid.with_default(FieldValue::Bytes(vec![2, 0, 0, 0, 0, 1])),
                to_end("management_address"),
            ],
        ),
        TlvVariant::new(
            2,
            "port_status",
            vec![enum_field("port_status", 8, &[(1, "blocked"), (2, "up")])
                .with_default(FieldValue::UInt(2))],
        ),
        TlvVariant::new(3, "data_tlv", vec![to_end("value")]),
        TlvVariant::new(
            4,
            "interface_status",
            vec![enum_field(
                "interface_status",
                8,
                &[
                    (1, "up"),
                    (2, "down"),
                    (3, "testing"),
                    (4, "unknown"),
                    (5, "dormant"),
                    (6, "not_present"),
                    (7, "lower_layer_down"),
                ],
            )
            .with_default(FieldValue::UInt(1))],
        ),
        TlvVariant::new(
            5,
            "reply_ingress",
            vec![
                action("ingress_action"),
                mac("ingress_mac", 2),
                to_end("ingress_port_id"),
            ],
        ),
        TlvVariant::new(
            6,
            "reply_egress",
            vec![
                action("egress_action"),
                mac("egress_mac", 2),
                to_end("egress_port_id"),
            ],
        ),
        TlvVariant::new(
            7,
            "ltm_egress_identifier",
            vec![uint("egress_identifier", 64, 0x0000_0200_0000_0001)],
        ),
        TlvVariant::new(
            8,
            "ltr_egress_identifier",
            vec![
                uint("last_egress_identifier", 64, 0x0000_0200_0000_0001),
                uint("next_egress_identifier", 64, 0x0000_0200_0000_0002),
            ],
        ),
        TlvVariant::new(
            31,
            "organization_specific",
            vec![u("oui", 24), u("subtype", 8), to_end("value")],
        ),
        TlvVariant::new(
            32,
            "test",
            vec![
                enum_field(
                    "pattern_type",
                    8,
                    &[
                        (0, "null_without_crc"),
                        (1, "null_with_crc"),
                        (2, "prbs_without_crc"),
                        (3, "prbs_with_crc"),
                    ],
                ),
                to_end("test_pattern"),
            ],
        ),
    ];
    FieldType::Tlv(TlvDef {
        type_bits: 8,
        len_bits: 16,
        len_includes_header: false,
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

/// Common header up to the flags: MD level, version and opcode.
fn head(opcode: u64, version: u64) -> Vec<FieldDef> {
    vec![
        desc(uint("md_level", 3, 0), "cfm-md-level"),
        desc(uint("version", 5, version), "cfm-version"),
        desc(
            enum_field("opcode", 8, OPCODES).with_default(FieldValue::UInt(opcode)),
            "cfm-opcode",
        ),
    ]
}

fn first_tlv_offset(offset: u64) -> FieldDef {
    desc(uint("first_tlv_offset", 8, offset), "cfm-first-tlv-offset")
}

fn plain_flags() -> Vec<FieldDef> {
    vec![desc(uint("flags", 8, 0), "cfm-flags")]
}

/// Flags with a 3-bit period in the low bits (CCM, AIS, LCK, CSF).
fn period(default: u64) -> FieldDef {
    desc(
        enum_field("period", 3, PERIODS).with_default(FieldValue::UInt(default)),
        "cfm-period",
    )
}

/// One dedicated PDU: header with `flags` fields, `first_tlv_offset`, the body
/// and the TLVs (default `tlv_items` followed by the End TLV).
#[allow(clippy::too_many_arguments)]
fn pdu(
    id: &str,
    name: &str,
    opcode: u64,
    version: u64,
    flags: Vec<FieldDef>,
    offset: u64,
    body: Vec<FieldDef>,
    tlv_items: Vec<TlvItem>,
) -> ProtocolDef {
    let mut d = ProtocolDef::new(id, name, ProtocolLayer::L2);
    d.fields = head(opcode, version);
    d.fields.extend(flags);
    d.fields.push(first_tlv_offset(offset));
    d.fields.extend(body);
    let mut items = tlv_items;
    items.push(TlvItem::named("end"));
    d.fields.push(desc(
        FieldDef::new("tlvs", tlvs()).with_default(FieldValue::Tlv(items)),
        "cfm-tlvs",
    ));
    d.constraints.push(Constraint::new(
        DiagLevel::Warning,
        ConstraintRule::FieldEquals {
            field: "first_tlv_offset".into(),
            #[allow(clippy::cast_possible_wrap)] // small constants
            value: offset as i64,
        },
    ));
    d
}

fn transaction_id() -> FieldDef {
    desc(uint("transaction_id", 32, 1), "cfm-transaction-id")
}

/// Frame counters (`names`, 32 bits each) of the loss measurement PDUs.
fn counters(names: &[&str]) -> Vec<FieldDef> {
    names
        .iter()
        .map(|n| desc(uint(n, 32, 0), "cfm-frame-counter"))
        .collect()
}

/// IEEE 1588 timestamps (32-bit seconds, 32-bit nanoseconds) of the delay
/// measurement PDUs.
fn timestamps(names: &[&str]) -> Vec<FieldDef> {
    names
        .iter()
        .flat_map(|n| {
            [
                desc(uint(&format!("{n}_seconds"), 32, 0), "cfm-timestamp"),
                desc(uint(&format!("{n}_nanoseconds"), 32, 0), "cfm-timestamp"),
            ]
        })
        .collect()
}

/// CCM (opcode 1, 802.1Q §21.6, Y.1731 §9.2): RDI and period flags, sequence
/// number, MEP ID, the 48-octet MAID / MEG ID and the Y.1731 frame counters for
/// dual-ended loss measurement (zero when unused).
#[must_use]
pub fn cfm_ccm() -> ProtocolDef {
    let mut maid = MEG_ID.to_vec();
    maid.resize(48, 0);
    let mut body = vec![
        desc(uint("sequence_number", 32, 0), "cfm-sequence-number"),
        uint("mep_id_reserved", 3, 0),
        desc(uint("mep_id", 13, 1), "cfm-mep-id"),
        desc(
            FieldDef::new(
                "maid",
                FieldType::Bytes {
                    len: Length::Fixed { octets: 48 },
                },
            )
            .with_default(FieldValue::Bytes(maid)),
            "cfm-maid",
        ),
    ];
    body.extend(counters(&["tx_fcf", "rx_fcb", "tx_fcb"]));
    body.push(uint("reserved", 32, 0));
    pdu(
        "cfm_ccm",
        "CFM CCM",
        1,
        0,
        vec![
            desc(uint("rdi", 1, 0), "cfm-rdi"),
            uint("flags_reserved", 4, 0),
            period(4),
        ],
        70,
        body,
        Vec::new(),
    )
}

/// LBR (opcode 2): the transaction ID of the answered LBM and its TLVs.
#[must_use]
pub fn cfm_lbr() -> ProtocolDef {
    pdu(
        "cfm_lbr",
        "CFM LBR",
        2,
        0,
        plain_flags(),
        4,
        vec![transaction_id()],
        Vec::new(),
    )
}

/// LBM (opcode 3): loopback transaction ID and optional TLVs (Data / Test).
#[must_use]
pub fn cfm_lbm() -> ProtocolDef {
    pdu(
        "cfm_lbm",
        "CFM LBM",
        3,
        0,
        plain_flags(),
        4,
        vec![transaction_id()],
        Vec::new(),
    )
}

/// LTR (opcode 4): `UseFDBonly`, `FwdYes` and `TerminalMEP` flags, transaction ID,
/// reply TTL and relay action, then the LTR Egress Identifier and Reply Ingress /
/// Egress TLVs.
#[must_use]
pub fn cfm_ltr() -> ProtocolDef {
    pdu(
        "cfm_ltr",
        "CFM LTR",
        4,
        0,
        vec![
            desc(uint("use_fdb_only", 1, 1), "cfm-use-fdb-only"),
            desc(uint("fwd_yes", 1, 0), "cfm-fwd-yes"),
            desc(uint("terminal_mep", 1, 1), "cfm-terminal-mep"),
            uint("flags_reserved", 5, 0),
        ],
        6,
        vec![
            transaction_id(),
            desc(uint("reply_ttl", 8, 63), "cfm-ttl"),
            desc(
                enum_field(
                    "relay_action",
                    8,
                    &[(1, "rly_hit"), (2, "rly_fdb"), (3, "rly_mpdb")],
                )
                .with_default(FieldValue::UInt(1)),
                "cfm-relay-action",
            ),
        ],
        vec![
            TlvItem::named("ltr_egress_identifier"),
            TlvItem::named("reply_ingress"),
        ],
    )
}

/// LTM (opcode 5): `UseFDBonly` flag, transaction ID, TTL, original and target
/// MAC addresses, then the LTM Egress Identifier TLV.
#[must_use]
pub fn cfm_ltm() -> ProtocolDef {
    pdu(
        "cfm_ltm",
        "CFM LTM",
        5,
        0,
        vec![
            desc(uint("use_fdb_only", 1, 1), "cfm-use-fdb-only"),
            uint("flags_reserved", 7, 0),
        ],
        17,
        vec![
            transaction_id(),
            desc(uint("ttl", 8, 64), "cfm-ttl"),
            desc(mac("original_mac", 1), "cfm-original-mac"),
            desc(mac("target_mac", 2), "cfm-target-mac"),
        ],
        vec![TlvItem::named("ltm_egress_identifier")],
    )
}

/// AIS (opcode 33) / LCK (opcode 35): only the period flags.
fn signal(id: &str, name: &str, opcode: u64) -> ProtocolDef {
    pdu(
        id,
        name,
        opcode,
        0,
        vec![uint("flags_reserved", 5, 0), period(4)],
        0,
        Vec::new(),
        Vec::new(),
    )
}

/// AIS (opcode 33, Y.1731 §9.7): alarm indication signal, sent every period.
#[must_use]
pub fn cfm_ais() -> ProtocolDef {
    signal("cfm_ais", "Y.1731 AIS", 33)
}

/// LCK (opcode 35, Y.1731 §9.8): locked signal, sent every period.
#[must_use]
pub fn cfm_lck() -> ProtocolDef {
    signal("cfm_lck", "Y.1731 LCK", 35)
}

/// TST (opcode 37, Y.1731 §9.9): sequence number and a Test TLV.
#[must_use]
pub fn cfm_tst() -> ProtocolDef {
    pdu(
        "cfm_tst",
        "Y.1731 TST",
        37,
        0,
        plain_flags(),
        4,
        vec![desc(uint("sequence_number", 32, 0), "cfm-sequence-number")],
        vec![TlvItem::named("test").with("test_pattern", vec![0u8; 16])],
    )
}

/// R-APS (opcode 40, ITU-T G.8032 §10.3): request / state, sub-code, the RB / DNF
/// / BPR status bits, node ID and 24 reserved octets. Version 1 is ERPS version 2.
/// Default: NR, RB set (the RPL owner's idle message).
#[must_use]
pub fn cfm_raps() -> ProtocolDef {
    pdu(
        "cfm_raps",
        "ERPS R-APS",
        40,
        1,
        plain_flags(),
        32,
        vec![
            desc(
                enum_field(
                    "request_state",
                    4,
                    &[
                        (0b0000, "nr"),
                        (0b0111, "ms"),
                        (0b1011, "sf"),
                        (0b1101, "fs"),
                        (0b1110, "event"),
                    ],
                )
                .with_default(FieldValue::UInt(0)),
                "raps-request-state",
            ),
            desc(uint("sub_code", 4, 0), "raps-sub-code"),
            desc(uint("rb", 1, 1), "raps-rb"),
            desc(uint("dnf", 1, 0), "raps-dnf"),
            desc(uint("bpr", 1, 0), "raps-bpr"),
            uint("status_reserved", 5, 0),
            desc(mac("node_id", 1), "raps-node-id"),
            FieldDef::new(
                "reserved",
                FieldType::Bytes {
                    len: Length::Fixed { octets: 24 },
                },
            )
            .with_default(FieldValue::Bytes(vec![0; 24])),
        ],
        Vec::new(),
    )
}

/// LMR (opcode 42, Y.1731 §9.13): the counters of the LMM and `TxFCb`.
#[must_use]
pub fn cfm_lmr() -> ProtocolDef {
    pdu(
        "cfm_lmr",
        "Y.1731 LMR",
        42,
        0,
        plain_flags(),
        12,
        counters(&["tx_fcf", "rx_fcf", "tx_fcb"]),
        Vec::new(),
    )
}

/// LMM (opcode 43, Y.1731 §9.12): `TxFCf` and two reserved counters.
#[must_use]
pub fn cfm_lmm() -> ProtocolDef {
    pdu(
        "cfm_lmm",
        "Y.1731 LMM",
        43,
        0,
        plain_flags(),
        12,
        counters(&["tx_fcf", "rx_fcf", "tx_fcb"]),
        Vec::new(),
    )
}

/// 1DM (opcode 45, Y.1731 §9.14): `TxTimeStampf` and the timestamp reserved for
/// the receiver.
#[must_use]
pub fn cfm_1dm() -> ProtocolDef {
    pdu(
        "cfm_1dm",
        "Y.1731 1DM",
        45,
        0,
        plain_flags(),
        16,
        timestamps(&["tx_timestamp_f", "rx_timestamp_f"]),
        Vec::new(),
    )
}

/// DMR (opcode 46, Y.1731 §9.16): the four timestamps of a two-way measurement.
#[must_use]
pub fn cfm_dmr() -> ProtocolDef {
    pdu(
        "cfm_dmr",
        "Y.1731 DMR",
        46,
        0,
        plain_flags(),
        32,
        timestamps(&[
            "tx_timestamp_f",
            "rx_timestamp_f",
            "tx_timestamp_b",
            "rx_timestamp_b",
        ]),
        Vec::new(),
    )
}

/// DMM (opcode 47, Y.1731 §9.15): `TxTimeStampf` and three reserved timestamps.
#[must_use]
pub fn cfm_dmm() -> ProtocolDef {
    pdu(
        "cfm_dmm",
        "Y.1731 DMM",
        47,
        0,
        plain_flags(),
        32,
        timestamps(&[
            "tx_timestamp_f",
            "rx_timestamp_f",
            "tx_timestamp_b",
            "rx_timestamp_b",
        ]),
        Vec::new(),
    )
}

/// CSF (opcode 52, Y.1731 §9.21): client signal fail type and period.
#[must_use]
pub fn cfm_csf() -> ProtocolDef {
    pdu(
        "cfm_csf",
        "Y.1731 CSF",
        52,
        0,
        vec![
            uint("flags_reserved", 2, 0),
            desc(
                enum_field(
                    "type",
                    3,
                    &[(0, "los"), (1, "fdi_ais"), (2, "rdi"), (3, "dci")],
                )
                .with_default(FieldValue::UInt(0)),
                "cfm-csf-type",
            ),
            period(4),
        ],
        0,
        Vec::new(),
        Vec::new(),
    )
}

/// 1SL (opcode 53, Y.1731 §9.22): source MEP ID, test ID and `TxFCf`.
#[must_use]
pub fn cfm_1sl() -> ProtocolDef {
    pdu(
        "cfm_1sl",
        "Y.1731 1SL",
        53,
        0,
        plain_flags(),
        16,
        vec![
            desc(uint("source_mep_id", 16, 1), "cfm-source-mep-id"),
            uint("reserved", 16, 0),
            desc(uint("test_id", 32, 1), "cfm-test-id"),
            desc(uint("tx_fcf", 32, 0), "cfm-frame-counter"),
            uint("reserved2", 32, 0),
        ],
        Vec::new(),
    )
}

/// Synthetic loss PDU body (SLM / SLR, Y.1731 §9.22).
fn synthetic_loss(id: &str, name: &str, opcode: u64) -> ProtocolDef {
    pdu(
        id,
        name,
        opcode,
        0,
        plain_flags(),
        16,
        vec![
            desc(uint("source_mep_id", 16, 1), "cfm-source-mep-id"),
            desc(uint("responder_mep_id", 16, 0), "cfm-responder-mep-id"),
            desc(uint("test_id", 32, 1), "cfm-test-id"),
            desc(uint("tx_fcf", 32, 0), "cfm-frame-counter"),
            desc(uint("tx_fcb", 32, 0), "cfm-frame-counter"),
        ],
        Vec::new(),
    )
}

/// SLR (opcode 54): the SLM fields with the responder MEP ID and `TxFCb`.
#[must_use]
pub fn cfm_slr() -> ProtocolDef {
    synthetic_loss("cfm_slr", "Y.1731 SLR", 54)
}

/// SLM (opcode 55): source MEP ID, test ID and `TxFCf`.
#[must_use]
pub fn cfm_slm() -> ProtocolDef {
    synthetic_loss("cfm_slm", "Y.1731 SLM", 55)
}

/// Generic CFM PDU: the common header and the rest of the PDU as octets (opcodes
/// without a dedicated definition: Linear APS, MCC, EXM / EXR, VSM / VSR). The
/// parser re-decodes it as the dedicated PDU of its `opcode`.
#[must_use]
pub fn cfm() -> ProtocolDef {
    let mut d = ProtocolDef::new(CFM, "CFM", ProtocolLayer::L2);
    d.fields = head(3, 0);
    d.fields.extend(plain_flags());
    d.fields.push(first_tlv_offset(4));
    // Default: an LBM (transaction ID 1, End TLV).
    d.fields.push(desc(
        to_end("data").with_default(FieldValue::Bytes(vec![0, 0, 0, 1, 0])),
        "cfm-data",
    ));
    d.next = Some(Binding::new(
        "opcode",
        PDUS.iter()
            .map(|(t, p)| BindingEntry::new(*t, *p).reparsing())
            .collect(),
    ));
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitions_are_valid() {
        let all = [
            cfm(),
            cfm_ccm(),
            cfm_lbr(),
            cfm_lbm(),
            cfm_ltr(),
            cfm_ltm(),
            cfm_ais(),
            cfm_lck(),
            cfm_tst(),
            cfm_raps(),
            cfm_lmr(),
            cfm_lmm(),
            cfm_1dm(),
            cfm_dmr(),
            cfm_dmm(),
            cfm_csf(),
            cfm_1sl(),
            cfm_slr(),
            cfm_slm(),
        ];
        for d in &all {
            d.validate().unwrap_or_else(|e| panic!("{}: {e}", d.id));
        }
        // The fixed part after the first 4 octets is `first_tlv_offset` long.
        for d in &all[1..] {
            let bits: u32 = d.fields.iter().filter_map(|f| f.ty.fixed_bits()).sum();
            let offset = d.field("first_tlv_offset").unwrap().default.clone();
            assert_eq!(
                FieldValue::UInt(u64::from(bits / 8 - 4)),
                offset.unwrap(),
                "{}",
                d.id
            );
        }
        assert_eq!(PDUS.len(), all.len() - 1);
    }
}
