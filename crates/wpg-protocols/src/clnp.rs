//! CLNP (ISO 8473-1) and ES-IS (ISO 9542) over the OSI encapsulation (spec §2.1
//! P2, M7).
//!
//! Both are announced by their NLPID like any OSI network-layer PDU
//! ([`crate::osi::osi_peek`]): `0x81` decodes as [`clnp`], `0x82` as the generic
//! [`esis`], whose `pdu_type` then re-decodes the layer as the dedicated ES-IS PDU
//! ([`esis_esh`], [`esis_ish`], [`esis_rd`]).
//!
//! All CLNP PDU types (DT, MD, ER, ERQ, ERP) share one header layout, so [`clnp`]
//! is a single protocol with a `pdu_type` field. The header is the layer; the
//! user data (DT: the transport PDU, ER: the header of the discarded PDU, ERQ /
//! ERP: the echoed PDU) is the next layer (a `raw` / `payload` layer).
//!
//! Auto fields:
//!
//! * `length_indicator` is the header length in octets.
//! * The CLNP `segment_length` is the length of the whole PDU (header + data).
//! * `checksum` is the ISO 8473 Fletcher checksum of the header (CLNP) / of the
//!   whole PDU (ES-IS). A Manual `0` means "checksum not used" (the builder
//!   still reports it as differing from Auto; a computed value is never 0).
//! * The CLNP `sp` bit follows the segmentation part (present when it has one
//!   element); `total_length` inside it is not computed.
//! * The ESH `number_of_source_addresses` and the RD `bsnpa_length` follow their
//!   fields.
//!
//! Options (8-bit code / 8-bit length) end at the length indicator
//! ([`wpg_core::Length::UntilOffset`]), after the variable-size addresses.
//! Security and quality of service maintenance values are kept as octets.

use std::collections::BTreeMap;

use wpg_core::{
    AutoCompute, Binding, BindingEntry, ChecksumCoverage, ChecksumKind, EnumVariant, FieldDef,
    FieldType, FieldValue, ItemDef, Length, LengthOf, LengthUnit, ProtocolDef, ProtocolLayer,
    TlvDef, TlvVariant,
};

/// Id of the CLNP PDU ([`clnp`]).
pub const CLNP: &str = "clnp";
/// Id of the generic ES-IS PDU ([`esis`]).
pub const ESIS: &str = "esis";

/// The dedicated ES-IS PDUs: (PDU type, protocol id).
pub const ESIS_PDUS: [(u64, &str); 3] = [(2, "esis_esh"), (4, "esis_ish"), (6, "esis_rd")];

/// Smallest CLNP PDU: the 9-octet fixed part and two empty address lengths.
pub const CLNP_MIN_LEN: u32 = 11;
/// Smallest ES-IS PDU: the 9-octet fixed part.
pub const ESIS_MIN_LEN: u32 = 9;

const SRC_NSAP: [u8; 10] = [0x49, 0x00, 0x01, 0, 0, 0, 0, 0, 0x01, 0x00];
const DST_NSAP: [u8; 10] = [0x49, 0x00, 0x01, 0, 0, 0, 0, 0, 0x02, 0x00];
const ES_NSAP: [u8; 10] = [0x49, 0x00, 0x01, 0, 0, 0, 0, 0, 0x0a, 0x00];

fn u(name: &str, bits: u8) -> FieldDef {
    FieldDef::new(name, FieldType::UInt { bits })
}

fn uint(name: &str, bits: u8, default: u64, key: &str) -> FieldDef {
    u(name, bits)
        .with_default(FieldValue::UInt(default))
        .with_description(key)
}

fn to_end(name: &str) -> FieldDef {
    FieldDef::new(name, FieldType::Bytes { len: Length::ToEnd })
}

fn variants(list: &[(u64, &str)]) -> Vec<EnumVariant> {
    list.iter()
        .map(|(v, n)| EnumVariant {
            value: *v,
            name: (*n).into(),
            description: None,
        })
        .collect()
}

fn enum_field(name: &str, bits: u8, list: &[(u64, &str)]) -> FieldDef {
    FieldDef::new(
        name,
        FieldType::Enum {
            bits,
            variants: variants(list),
        },
    )
}

fn nsap(name: &str, default: &[u8], key: &str) -> FieldDef {
    FieldDef::new(name, FieldType::Nsap)
        .with_default(FieldValue::Nsap(default.to_vec()))
        .with_description(key)
}

/// A list of `{address: nsap}` items, `length` long.
fn nsap_list(name: &str, length: Length) -> FieldDef {
    FieldDef::new(
        name,
        FieldType::List {
            item: ItemDef::Struct {
                fields: vec![FieldDef::new("address", FieldType::Nsap)],
            },
            length,
        },
    )
}

fn nsap_item(address: &[u8]) -> FieldValue {
    FieldValue::Struct(BTreeMap::from([(
        "address".to_owned(),
        FieldValue::Nsap(address.to_vec()),
    )]))
}

/// Header length indicator (Auto).
fn length_indicator(key: &str) -> FieldDef {
    u("length_indicator", 8)
        .with_auto(AutoCompute::Length {
            of: LengthOf::Header,
            unit: LengthUnit::Bytes,
            adjust: 0,
        })
        .with_description(key)
}

/// ISO 8473 Fletcher checksum of the header (Auto).
fn checksum(key: &str) -> FieldDef {
    u("checksum", 16)
        .with_auto(AutoCompute::Checksum {
            algorithm: ChecksumKind::Fletcher,
            covers: ChecksumCoverage::Header,
            nonzero: false,
            exclude: Vec::new(),
        })
        .with_description(key)
}

/// Options (8-bit code / 8-bit length) up to the length indicator.
fn options(list: Vec<TlvVariant>, key: &str) -> FieldDef {
    FieldDef::new(
        "options",
        FieldType::Tlv(TlvDef {
            type_bits: 8,
            len_bits: 8,
            len_includes_header: false,
            len_unit: LengthUnit::Bytes,
            variants: list,
            unknown_as_raw: true,
            length: Length::UntilOffset {
                field: "length_indicator".into(),
                unit: 1,
                adjust: 0,
            },
            fill: None,
            head_octets: 0,
            align: 0,
            unpadded_last: false,
            long_length: None,
            len_first: false,
            uncounted_octets: 0,
        }),
    )
    .with_default(FieldValue::Tlv(Vec::new()))
    .with_description(key)
}

/// Options common to CLNP and ES-IS (ISO 8473 §7.5, ISO 9542 §7.4).
fn common_options() -> Vec<TlvVariant> {
    vec![
        TlvVariant::new(0xc5, "security", vec![to_end("value")]),
        TlvVariant::new(0xc3, "qos_maintenance", vec![to_end("value")]),
        TlvVariant::new(0xcd, "priority", vec![u("value", 8)]),
    ]
}

/// Source routing / recording of route: type (0x00 partial, 0xff complete),
/// next address indicator and the addresses.
fn route_option(code: u64, name: &str) -> TlvVariant {
    TlvVariant::new(
        code,
        name,
        vec![
            enum_field("route_type", 8, &[(0x00, "partial"), (0xff, "complete")]),
            u("next_address", 8),
            nsap_list("addresses", Length::ToEnd),
        ],
    )
}

/// CLNP reasons for discard (ISO 8473 §7.9.5).
const DISCARD_REASONS: [(u64, &str); 21] = [
    (0x00, "general"),
    (0x01, "checksum"),
    (0x02, "congestion"),
    (0x03, "header_syntax"),
    (0x04, "segmentation_needed"),
    (0x05, "incomplete_pdu"),
    (0x06, "duplicate_option"),
    (0x80, "destination_unreachable"),
    (0x81, "destination_unknown"),
    (0x90, "source_routing_unspecified"),
    (0x91, "source_routing_syntax"),
    (0x92, "source_routing_unknown_address"),
    (0x93, "source_routing_path_not_acceptable"),
    (0xa0, "lifetime_expired_in_transit"),
    (0xa1, "lifetime_expired_reassembly"),
    (0xb0, "unsupported_option"),
    (0xb1, "unsupported_version"),
    (0xb2, "unsupported_security"),
    (0xb3, "unsupported_source_routing"),
    (0xb4, "unsupported_recording_of_route"),
    (0xc0, "reassembly_interference"),
];

/// CLNP options: security, quality of service maintenance, priority, source routing, recording
/// of route, padding and reason for discard (ER PDUs).
fn clnp_options() -> Vec<TlvVariant> {
    let mut v = common_options();
    v.extend([
        route_option(0xc8, "source_routing"),
        route_option(0xcb, "recording_of_route"),
        TlvVariant::new(0xcc, "padding", vec![to_end("data")]),
        TlvVariant::new(
            0xc1,
            "reason_for_discard",
            vec![enum_field("reason", 8, &DISCARD_REASONS), u("pointer", 8)],
        ),
    ]);
    v
}

/// CLNP (ISO 8473-1): fixed part (NLPID, length indicator, version, lifetime,
/// SP / MS / E/R flags and PDU type, segment length, checksum), destination and
/// source NSAPs, the optional segmentation part and the options. The data follows
/// as the next layer.
///
/// Default: a DT PDU from 49.0001.0000.0000.0001.00 to 49.0001.0000.0000.0002.00
/// with a lifetime of 32 (16 s), segmentation not permitted.
#[must_use]
pub fn clnp() -> ProtocolDef {
    let mut d = ProtocolDef::new(CLNP, "CLNP", ProtocolLayer::L3);
    let segment = ItemDef::Struct {
        fields: vec![
            u("data_unit_identifier", 16),
            u("segment_offset", 16),
            u("total_length", 16),
        ],
    };
    d.fields = vec![
        enum_field("nlpid", 8, &[(0x81, "clnp")])
            .with_default(FieldValue::UInt(0x81))
            .with_description("clnp-nlpid"),
        length_indicator("clnp-length-indicator"),
        uint("version", 8, 1, "clnp-version"),
        uint("lifetime", 8, 32, "clnp-lifetime"),
        u("sp", 1)
            .with_auto(AutoCompute::Count {
                field: "segmentation".into(),
                adjust: 0,
            })
            .with_description("clnp-sp"),
        uint("ms", 1, 0, "clnp-ms"),
        uint("er", 1, 1, "clnp-er"),
        enum_field(
            "pdu_type",
            5,
            &[
                (0x01, "er"),
                (0x1c, "dt"),
                (0x1d, "md"),
                (0x1e, "erq"),
                (0x1f, "erp"),
            ],
        )
        .with_default(FieldValue::UInt(0x1c))
        .with_description("clnp-pdu-type"),
        u("segment_length", 16)
            .with_auto(AutoCompute::Length {
                of: LengthOf::Total,
                unit: LengthUnit::Bytes,
                adjust: 0,
            })
            .with_description("clnp-segment-length"),
        checksum("clnp-checksum"),
        nsap("destination", &DST_NSAP, "clnp-destination"),
        nsap("source", &SRC_NSAP, "clnp-source"),
        FieldDef::new(
            "segmentation",
            FieldType::List {
                item: segment,
                length: Length::Count { field: "sp".into() },
            },
        )
        .with_default(FieldValue::List(Vec::new()))
        .with_description("clnp-segmentation"),
        options(clnp_options(), "clnp-options"),
    ];
    d
}

/// ES-IS options: security, quality of service maintenance, priority, address mask, SNPA mask
/// and the suggested ES configuration timer (ISHs).
fn esis_options() -> Vec<TlvVariant> {
    let mut v = common_options();
    v.extend([
        TlvVariant::new(0xe1, "address_mask", vec![to_end("mask")]),
        TlvVariant::new(0xe2, "snpa_mask", vec![to_end("mask")]),
        TlvVariant::new(0xc6, "es_configuration_timer", vec![u("seconds", 16)]),
    ]);
    v
}

/// The 9-octet fixed part with the PDU type.
fn esis_header(pdu_type: u64) -> Vec<FieldDef> {
    vec![
        enum_field("nlpid", 8, &[(0x82, "esis")])
            .with_default(FieldValue::UInt(0x82))
            .with_description("esis-nlpid"),
        length_indicator("esis-length-indicator"),
        uint("version", 8, 1, "esis-version"),
        u("reserved", 8).with_default(FieldValue::UInt(0)),
        u("reserved_type", 3).with_default(FieldValue::UInt(0)),
        enum_field("pdu_type", 5, &[(2, "esh"), (4, "ish"), (6, "rd")])
            .with_default(FieldValue::UInt(pdu_type))
            .with_description("esis-pdu-type"),
        uint("holding_time", 16, 30, "esis-holding-time"),
        checksum("esis-checksum"),
    ]
}

fn esis_pdu(id: &str, name: &str, pdu_type: u64, body: Vec<FieldDef>) -> ProtocolDef {
    let mut d = ProtocolDef::new(id, name, ProtocolLayer::L3);
    d.fields = esis_header(pdu_type);
    d.fields.extend(body);
    d.fields.push(options(esis_options(), "esis-options"));
    d
}

/// ES-IS End System Hello (type 2, ISO 9542 §9.5): the NSAPs served by the end
/// system (`number_of_source_addresses` Auto). Sent to `09:00:2b:00:00:05`
/// (all intermediate systems).
#[must_use]
pub fn esis_esh() -> ProtocolDef {
    esis_pdu(
        "esis_esh",
        "ES-IS ESH",
        2,
        vec![
            u("number_of_source_addresses", 8)
                .with_auto(AutoCompute::Count {
                    field: "source_addresses".into(),
                    adjust: 0,
                })
                .with_description("esis-number-of-source-addresses"),
            nsap_list(
                "source_addresses",
                Length::Count {
                    field: "number_of_source_addresses".into(),
                },
            )
            .with_default(FieldValue::List(vec![nsap_item(&ES_NSAP)]))
            .with_description("esis-source-addresses"),
        ],
    )
}

/// ES-IS Intermediate System Hello (type 4, ISO 9542 §9.6): the network entity
/// title of the IS. Sent to `09:00:2b:00:00:04` (all end systems).
#[must_use]
pub fn esis_ish() -> ProtocolDef {
    esis_pdu(
        "esis_ish",
        "ES-IS ISH",
        4,
        vec![nsap("net", &SRC_NSAP, "esis-net")],
    )
}

/// ES-IS Redirect (type 6, ISO 9542 §9.7): destination NSAP, subnetwork address
/// of the better next hop (`bsnpa_length` Auto) and its NET (empty when the
/// destination is an end system on the same subnetwork).
#[must_use]
pub fn esis_rd() -> ProtocolDef {
    esis_pdu(
        "esis_rd",
        "ES-IS Redirect",
        6,
        vec![
            nsap("destination", &DST_NSAP, "esis-destination"),
            u("bsnpa_length", 8)
                .with_auto(AutoCompute::Size {
                    field: "bsnpa".into(),
                })
                .with_description("esis-bsnpa-length"),
            FieldDef::new(
                "bsnpa",
                FieldType::Bytes {
                    len: Length::FromField {
                        field: "bsnpa_length".into(),
                        unit: 1,
                        adjust: 0,
                        div: 1,
                    },
                },
            )
            .with_default(FieldValue::Bytes(vec![0x02, 0, 0, 0, 0, 0x02]))
            .with_description("esis-bsnpa"),
            nsap("net", &DST_NSAP, "esis-net"),
        ],
    )
}

/// Generic ES-IS PDU: the fixed part and the rest as opaque octets. The parser
/// reaches it through the NLPID and re-decodes the layer as the dedicated PDU of
/// its `pdu_type`; unknown PDU types stay here.
#[must_use]
pub fn esis() -> ProtocolDef {
    let mut d = ProtocolDef::new(ESIS, "ES-IS", ProtocolLayer::L3);
    d.fields = esis_header(2);
    d.fields.push(
        to_end("data")
            .with_default(FieldValue::Bytes(Vec::new()))
            .with_description("esis-data"),
    );
    d.next = Some(Binding::new(
        "pdu_type",
        ESIS_PDUS
            .iter()
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
        for d in [clnp(), esis(), esis_esh(), esis_ish(), esis_rd()] {
            d.validate().unwrap_or_else(|e| panic!("{}: {e}", d.id));
        }
    }
}
