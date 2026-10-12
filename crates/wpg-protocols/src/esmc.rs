//! `SyncE` ESMC, ITU-T G.8264 (TODO §2.1 P2, M7).
//!
//! The Ethernet Synchronization Messaging Channel is an organization specific
//! Slow Protocol (EtherType `0x8809`, subtype `0x0a`, ITU-T OUI `00-19-a7`, ITU
//! subtype 1) sent to `01-80-c2-00-00-02`. One [`esmc`] layer holds the header
//! (version, event flag) and the TLVs: the QL TLV with the SSM code and the
//! Extended QL TLV (G.8264 amendment 1) with the enhanced SSM code, the `SyncE`
//! clock identity and the cascaded clock counts.
//!
//! TLV lengths (which count the 3-octet header) are Auto. Octets after the last
//! known TLV, the padding of the PDU to its minimum size, are kept as one raw
//! item.

use wpg_core::{
    protocol, EnumVariant, FieldDef, FieldType, FieldValue, FlagBit, Length, LengthUnit,
    ProtocolDef, TlvDef, TlvItem, TlvVariant,
};

/// Id of the ESMC PDU ([`esmc`]).
pub const ESMC: &str = "esmc";

/// Slow Protocols subtype of the organization specific Slow Protocol (OSSP).
pub const ESMC_SUBTYPE: u64 = 0x0a;

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

/// SSM codes (G.781): option 1 (PRC, SSU-A, SSU-B, SEC, DNU) and the option 2
/// codes that do not clash with them.
const SSM_CODES: &[(u64, &str)] = &[
    (0x0, "stu"),
    (0x1, "prs"),
    (0x2, "prc"),
    (0x4, "ssu_a"),
    (0x7, "st2"),
    (0x8, "ssu_b"),
    (0xa, "st3"),
    (0xb, "sec"),
    (0xc, "smc"),
    (0xd, "st3e"),
    (0xe, "prov"),
    (0xf, "dnu"),
];

/// Enhanced SSM codes (G.8264 amendment 1, Table 11-3).
const ENHANCED_SSM_CODES: &[(u64, &str)] = &[
    (0x20, "eprtc"),
    (0x21, "prtc"),
    (0x22, "eeec"),
    (0x23, "eprc"),
    (0xff, "none"),
];

/// ESMC TLVs: 8-bit type, 16-bit length including the 3-octet header. Unknown
/// types (the padding) end the typed decoding.
#[must_use]
pub fn tlvs() -> FieldType {
    FieldType::Tlv(TlvDef {
        type_bits: 8,
        len_bits: 16,
        len_includes_header: true,
        len_unit: LengthUnit::Bytes,
        variants: vec![
            TlvVariant::new(
                0x01,
                "ql",
                vec![
                    uint("unused", 4, 0),
                    enum_field("ssm_code", 4, SSM_CODES).with_default(FieldValue::UInt(0x2)),
                ],
            ),
            TlvVariant::new(
                0x02,
                "extended_ql",
                vec![
                    enum_field("enhanced_ssm_code", 8, ENHANCED_SSM_CODES)
                        .with_default(FieldValue::UInt(0xff)),
                    uint("synce_clock_identity", 64, 0x0200_00ff_fe00_0001),
                    FieldDef::new(
                        "flags",
                        FieldType::Flags {
                            bits: 8,
                            flags: vec![
                                FlagBit {
                                    bit: 0,
                                    name: "mixed_eec_eeec".into(),
                                },
                                FlagBit {
                                    bit: 1,
                                    name: "partial_chain".into(),
                                },
                            ],
                        },
                    )
                    .with_default(FieldValue::UInt(0)),
                    uint("cascaded_eeecs", 8, 1),
                    uint("cascaded_eecs", 8, 0),
                    FieldDef::new(
                        "reserved",
                        FieldType::Bytes {
                            len: Length::Fixed { octets: 5 },
                        },
                    )
                    .with_default(FieldValue::Bytes(vec![0; 5])),
                ],
            ),
        ],
        unknown_as_raw: false,
        length: Length::ToEnd,
        fill: None,
        head_octets: 0,
        align: 0,
        unpadded_last: false,
        long_length: None,
        len_first: false,
        uncounted_octets: 0,
    })
}

/// ESMC PDU (G.8264 §11.3.1): Slow Protocols subtype `0x0a`, ITU-T OUI and
/// subtype, version 1, the event flag (set for event PDUs, clear for the
/// once-a-second information PDUs) and the TLVs. Default: QL-PRC information
/// PDU with an Extended QL TLV.
#[must_use]
pub fn esmc() -> ProtocolDef {
    let mut d = protocol! {
        id: "esmc",
        name: "ESMC",
        layer: L2,
        fields: {
            subtype: uint(8) = 0x0a, desc = "esmc-subtype";
            itu_oui: uint(24) = 0x0019a7, desc = "esmc-itu-oui";
            itu_subtype: uint(16) = 0x0001, desc = "esmc-itu-subtype";
            version: uint(4) = 1, desc = "esmc-version";
            event_flag: uint(1) = 0, desc = "esmc-event-flag";
            reserved: uint(3) = 0;
            reserved2: uint(24) = 0;
            tlvs: ty(tlvs()), desc = "esmc-tlvs";
        },
        constraints: {
            warning: equals(subtype, 0x0a);
            warning: equals(itu_oui, 0x0019a7);
            warning: equals(version, 1);
        },
    };
    if let Some(f) = d.fields.last_mut() {
        f.default = Some(FieldValue::Tlv(vec![
            TlvItem::named("ql"),
            TlvItem::named("extended_ql"),
        ]));
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esmc_is_valid() {
        let d = esmc();
        d.validate().unwrap();
        assert_eq!(d.id, ESMC);
    }
}
