//! OSPFv2 Opaque LSAs (RFC 5250) and their main contents (TODO §2.1 P2, M7): the
//! Traffic Engineering LSA (RFC 3630), the Router Information LSA (RFC 7770), the
//! Extended Prefix / Extended Link LSAs (RFC 7684) and the Segment Routing
//! extensions carried in them (RFC 8665).
//!
//! An opaque LSA (LS type 9 link-local, 10 area, 11 AS scope) splits the Link
//! State ID into an 8-bit opaque type and a 24-bit opaque ID. The parser decodes
//! an LSA inside an LSU as [`crate::routing::ospfv2_lsa`] first, whose LS type
//! 9 / 10 / 11 re-decodes the layer as [`ospfv2_opaque_lsa`]; its `opaque_type`
//! then selects the body layer ([`ospfv2_te_lsa`], [`ospfv2_ri_lsa`],
//! [`ospfv2_ext_prefix_lsa`], [`ospfv2_ext_link_lsa`]). To build one, stack
//! `ospfv2_lsu`, `ospfv2_opaque_lsa` and the body.
//!
//! Every body is a list of TLVs with a 16-bit type and a 16-bit length counting
//! the value only; values are padded with zeros to a multiple of 4 octets that
//! the length does not count. Sub-TLVs are framed the same way. All lengths and
//! padding are Auto, as are the opaque type (from the body layer), the LSA length
//! and the Fletcher LSA checksum (exact for any LS age).
//!
//! SIDs are 3 octets (a label, low 20 bits) or 4 octets (an index); the sub-TLVs
//! that carry one exist in both forms, told apart by their length when parsing
//! (`prefix_sid` / `prefix_sid_label`, `adj_sid` / `adj_sid_index`, ...). TE
//! bandwidths are the raw 32-bit IEEE floating point values (`0x4b3ebc20` = 12.5e6
//! octets per second = 100 Mbit/s). OSPFv3 (Extended LSAs, RFC 8362 / RFC 8666) is
//! not modelled.

use wpg_core::{
    protocol, EnumVariant, FieldDef, FieldType, FieldValue, FlagBit, ItemDef, Length, LengthUnit,
    ProtocolDef, TlvDef, TlvItem, TlvVariant,
};

/// The opaque bodies: (opaque type, protocol id).
pub const BODIES: [(u64, &str); 4] = [
    (1, "ospfv2_te_lsa"),
    (4, "ospfv2_ri_lsa"),
    (7, "ospfv2_ext_prefix_lsa"),
    (8, "ospfv2_ext_link_lsa"),
];

fn u(name: &str, bits: u8) -> FieldDef {
    FieldDef::new(name, FieldType::UInt { bits })
}

fn ip4(name: &str) -> FieldDef {
    FieldDef::new(name, FieldType::Ipv4)
}

fn to_end(name: &str) -> FieldDef {
    FieldDef::new(name, FieldType::Bytes { len: Length::ToEnd })
}

fn list_of(name: &str, ty: FieldType) -> FieldDef {
    FieldDef::new(
        name,
        FieldType::List {
            item: ItemDef::Field { ty: Box::new(ty) },
            length: Length::ToEnd,
        },
    )
}

fn flags(name: &str, bits: u8, names: &[(u8, &str)]) -> FieldDef {
    FieldDef::new(
        name,
        FieldType::Flags {
            bits,
            flags: names
                .iter()
                .map(|(bit, n)| FlagBit {
                    bit: *bit,
                    name: (*n).into(),
                })
                .collect(),
        },
    )
}

fn enum_field(name: &str, bits: u8, variants: &[(u64, &str)]) -> FieldDef {
    FieldDef::new(
        name,
        FieldType::Enum {
            bits,
            variants: variants
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

/// OSPF TLV framing (RFC 7770 §2.3): 16-bit type, 16-bit value length, values
/// padded to 4 octets.
fn tlvs(variants: Vec<TlvVariant>) -> FieldType {
    FieldType::Tlv(TlvDef {
        type_bits: 16,
        len_bits: 16,
        len_includes_header: false,
        len_unit: LengthUnit::Bytes,
        variants,
        unknown_as_raw: true,
        length: Length::ToEnd,
        fill: None,
        head_octets: 0,
        align: 4,
        unpadded_last: false,
        long_length: None,
        len_first: false,
    })
}

fn sub_tlvs(variants: Vec<TlvVariant>) -> FieldDef {
    FieldDef::new("sub_tlvs", tlvs(variants))
}

/// A SID-carrying variant in its two forms: a 3-octet label (`<name>_label`, or
/// `name` when `label_first`) and a 4-octet index.
fn sid_variants(ty: u64, name: &str, head: &[FieldDef], label_first: bool) -> [TlvVariant; 2] {
    let fixed: u32 = head
        .iter()
        .map(|f| f.ty.fixed_bits().unwrap_or(0) / 8)
        .sum();
    let with = |sid: FieldDef| {
        let mut f = head.to_vec();
        f.push(sid);
        f
    };
    let (label_name, index_name) = if label_first {
        (name.to_owned(), format!("{name}_index"))
    } else {
        (format!("{name}_label"), name.to_owned())
    };
    [
        TlvVariant::new(ty, label_name, with(u("label", 24))).when_value_len(fixed + 3),
        TlvVariant::new(ty, index_name, with(u("index", 32))).when_value_len(fixed + 4),
    ]
}

/// The SID/Label sub-TLV (type 1, RFC 8665 §2.1) of the SID / Label Range TLVs.
fn sid_label_sub_tlvs() -> Vec<TlvVariant> {
    sid_variants(1, "sid_label", &[], true).into()
}

/// OSPFv2 opaque LSA header (RFC 5250 §3): the LSA header with the Link State ID
/// split into `opaque_type` (Auto, from the body layer) and `opaque_id`. Default:
/// an area-scope (type 10) LSA with opaque ID 0.
#[must_use]
pub fn ospfv2_opaque_lsa() -> ProtocolDef {
    protocol! {
        id: "ospfv2_opaque_lsa",
        name: "OSPFv2 Opaque LSA",
        layer: L3,
        fields: {
            ls_age: uint(16) = 0, desc = "ospf-ls-age";
            options: flags(8) {
                0 => mt,
                1 => e,
                2 => mc,
                3 => np,
                4 => l,
                5 => dc,
                6 => o,
                7 => dn,
            } = 0x42, desc = "ospf-options";
            ls_type: enum(8) {
                9 => link_local_opaque,
                10 => area_opaque,
                11 => as_opaque,
            } = 10, desc = "ospf-opaque-ls-type";
            opaque_type: enum(8) {
                1 => traffic_engineering,
                4 => router_information,
                7 => extended_prefix,
                8 => extended_link,
            } = 4, auto = next_protocol, desc = "ospf-opaque-type";
            opaque_id: uint(24) = 0, desc = "ospf-opaque-id";
            advertising_router: ipv4 = [192, 0, 2, 1], desc = "ospf-advertising-router";
            ls_sequence: uint(32) = 0x8000_0001, desc = "ospf-ls-sequence";
            ls_checksum: uint(16), auto = checksum(fletcher, header_payload, exclude = [ls_age]), desc = "ospf-ls-checksum";
            length: uint(16), auto = length(total), desc = "ospf-lsa-length";
        },
        next: opaque_type {
            1 => "ospfv2_te_lsa",
            4 => "ospfv2_ri_lsa",
            7 => "ospfv2_ext_prefix_lsa",
            8 => "ospfv2_ext_link_lsa",
        },
        constraints: {
            warning: in_range(ls_type, 9, 11);
        },
    }
}

/// Link sub-TLVs of the TE LSA (RFC 3630 §2.5).
fn te_link_sub_tlvs() -> Vec<TlvVariant> {
    let float = |ty: u64, name: &str| TlvVariant::new(ty, name, vec![u("bandwidth", 32)]);
    vec![
        TlvVariant::new(
            1,
            "link_type",
            vec![enum_field(
                "link_type",
                8,
                &[(1, "point_to_point"), (2, "multi_access")],
            )],
        ),
        TlvVariant::new(2, "link_id", vec![ip4("link_id")]),
        TlvVariant::new(
            3,
            "local_address",
            vec![list_of("addresses", FieldType::Ipv4)],
        ),
        TlvVariant::new(
            4,
            "remote_address",
            vec![list_of("addresses", FieldType::Ipv4)],
        ),
        TlvVariant::new(5, "te_metric", vec![u("metric", 32)]),
        float(6, "max_bandwidth"),
        float(7, "max_reservable_bandwidth"),
        TlvVariant::new(
            8,
            "unreserved_bandwidth",
            vec![list_of("bandwidths", FieldType::UInt { bits: 32 })],
        ),
        TlvVariant::new(9, "admin_group", vec![u("groups", 32)]),
    ]
}

/// Traffic Engineering LSA body (opaque type 1, RFC 3630): a Router Address TLV
/// (opaque ID 0) or a Link TLV with its sub-TLVs. Default: a point-to-point link
/// to 192.0.2.2 with TE metric 10 and 100 Mbit/s bandwidths.
#[must_use]
pub fn ospfv2_te_lsa() -> ProtocolDef {
    let bw = 0x4b3e_bc20u32; // 12.5e6 octets/s = 100 Mbit/s
    let link = TlvItem::named("link").with(
        "sub_tlvs",
        vec![
            TlvItem::named("link_type").with("link_type", 1u8),
            TlvItem::named("link_id").with("link_id", std::net::Ipv4Addr::new(192, 0, 2, 2)),
            TlvItem::named("local_address").with(
                "addresses",
                vec![FieldValue::Ipv4([198, 51, 100, 1].into())],
            ),
            TlvItem::named("remote_address").with(
                "addresses",
                vec![FieldValue::Ipv4([198, 51, 100, 2].into())],
            ),
            TlvItem::named("te_metric").with("metric", 10u32),
            TlvItem::named("max_bandwidth").with("bandwidth", bw),
            TlvItem::named("max_reservable_bandwidth").with("bandwidth", bw),
            TlvItem::named("unreserved_bandwidth")
                .with("bandwidths", vec![FieldValue::UInt(u64::from(bw)); 8]),
            TlvItem::named("admin_group").with("groups", 0u32),
        ],
    );
    let tlv_types = vec![
        TlvVariant::new(1, "router_address", vec![ip4("address")]),
        TlvVariant::new(2, "link", vec![sub_tlvs(te_link_sub_tlvs())]),
    ];
    protocol! {
        id: "ospfv2_te_lsa",
        name: "OSPFv2 TE LSA",
        layer: L3,
        fields: {
            tlvs: ty(tlvs(tlv_types)) = vec![link], desc = "ospf-opaque-tlvs";
        },
    }
}

/// The SID / Label Range TLV value (RFC 8665 §3.2 / §3.3): range size and the
/// first SID / label.
fn range_fields() -> Vec<FieldDef> {
    vec![
        u("range_size", 24),
        u("reserved", 8),
        sub_tlvs(sid_label_sub_tlvs()),
    ]
}

fn range(name: &str, size: u32, first_label: u32) -> TlvItem {
    TlvItem::named(name).with("range_size", size).with(
        "sub_tlvs",
        vec![TlvItem::named("sid_label").with("label", first_label)],
    )
}

/// Router Information LSA body (opaque type 4, RFC 7770): capabilities, dynamic
/// hostname (RFC 5642) and the Segment Routing TLVs (RFC 8665 §3): SR-Algorithm,
/// SID / Label Range (SRGB), SR Local Block and SRMS Preference. Default: SPF
/// and strict SPF, SRGB 16000-23999, SRLB 15000-15999.
#[must_use]
pub fn ospfv2_ri_lsa() -> ProtocolDef {
    let tlv_types = vec![
        TlvVariant::new(
            1,
            "informational_capabilities",
            vec![flags(
                "capabilities",
                32,
                &[
                    (31, "graceful_restart"),
                    (30, "graceful_restart_helper"),
                    (29, "stub_router"),
                    (28, "traffic_engineering"),
                    (27, "p2p_over_lan"),
                    (26, "experimental_te"),
                ],
            )],
        ),
        TlvVariant::new(2, "functional_capabilities", vec![u("capabilities", 32)]),
        TlvVariant::new(7, "hostname", vec![to_end("hostname")]),
        TlvVariant::new(
            8,
            "sr_algorithm",
            vec![list_of("algorithms", FieldType::UInt { bits: 8 })],
        ),
        TlvVariant::new(9, "sid_label_range", range_fields()),
        TlvVariant::new(14, "sr_local_block", range_fields()),
        TlvVariant::new(
            15,
            "srms_preference",
            vec![u("preference", 8), u("reserved", 24)],
        ),
    ];
    let default = vec![
        TlvItem::named("informational_capabilities").with("capabilities", 0u32),
        TlvItem::named("sr_algorithm")
            .with("algorithms", vec![FieldValue::UInt(0), FieldValue::UInt(1)]),
        range("sid_label_range", 8000, 16000),
        range("sr_local_block", 1000, 15000),
    ];
    protocol! {
        id: "ospfv2_ri_lsa",
        name: "OSPFv2 Router Information LSA",
        layer: L3,
        fields: {
            tlvs: ty(tlvs(tlv_types)) = default, desc = "ospf-opaque-tlvs";
        },
    }
}

/// Sub-TLVs of the Extended Prefix (Range) TLV: the Prefix SID (RFC 8665 §5).
fn prefix_sub_tlvs() -> Vec<TlvVariant> {
    let head = [
        flags(
            "flags",
            8,
            &[(6, "np"), (5, "m"), (4, "e"), (3, "v"), (2, "l")],
        ),
        u("reserved", 8),
        u("mt_id", 8),
        u("algorithm", 8),
    ];
    sid_variants(2, "prefix_sid", &head, false).into()
}

/// Extended Prefix Opaque LSA body (opaque type 7, RFC 7684 §2): Extended Prefix
/// TLVs (route type, prefix, A / N flags) and Extended Prefix Range TLVs (RFC 8665
/// §4) with their Prefix SID sub-TLVs. Default: 192.0.2.1/32 intra-area, node
/// flag, prefix SID index 1.
#[must_use]
pub fn ospfv2_ext_prefix_lsa() -> ProtocolDef {
    let route_type = || {
        enum_field(
            "route_type",
            8,
            &[
                (0, "unspecified"),
                (1, "intra_area"),
                (3, "inter_area"),
                (5, "as_external"),
                (7, "nssa_external"),
            ],
        )
    };
    let tlv_types = vec![
        TlvVariant::new(
            1,
            "extended_prefix",
            vec![
                route_type(),
                u("prefix_length", 8),
                u("address_family", 8),
                flags("flags", 8, &[(7, "a"), (6, "n")]),
                ip4("prefix"),
                sub_tlvs(prefix_sub_tlvs()),
            ],
        ),
        TlvVariant::new(
            2,
            "extended_prefix_range",
            vec![
                u("prefix_length", 8),
                u("address_family", 8),
                u("range_size", 16),
                flags("flags", 8, &[(7, "ia")]),
                u("reserved", 24),
                ip4("prefix"),
                sub_tlvs(prefix_sub_tlvs()),
            ],
        ),
    ];
    let default = vec![TlvItem::named("extended_prefix")
        .with("route_type", 1u8)
        .with("prefix_length", 32u8)
        .with("flags", 0x40u8)
        .with("prefix", std::net::Ipv4Addr::new(192, 0, 2, 1))
        .with(
            "sub_tlvs",
            vec![TlvItem::named("prefix_sid").with("index", 1u32)],
        )];
    protocol! {
        id: "ospfv2_ext_prefix_lsa",
        name: "OSPFv2 Extended Prefix LSA",
        layer: L3,
        fields: {
            tlvs: ty(tlvs(tlv_types)) = default, desc = "ospf-opaque-tlvs";
        },
    }
}

/// Sub-TLVs of the Extended Link TLV: Adj-SID and LAN Adj-SID (RFC 8665 §6).
fn link_sub_tlvs() -> Vec<TlvVariant> {
    let head = vec![
        flags(
            "flags",
            8,
            &[(7, "b"), (6, "v"), (5, "l"), (4, "g"), (3, "p")],
        ),
        u("reserved", 8),
        u("mt_id", 8),
        u("weight", 8),
    ];
    let mut lan = head.clone();
    lan.push(ip4("neighbor_id"));
    let mut v: Vec<TlvVariant> = sid_variants(2, "adj_sid", &head, true).into();
    v.extend(sid_variants(3, "lan_adj_sid", &lan, true));
    v
}

/// Extended Link Opaque LSA body (opaque type 8, RFC 7684 §3): Extended Link TLVs
/// (link type, ID and data as in the Router-LSA) with their Adj-SID sub-TLVs.
/// Default: a point-to-point link to 192.0.2.2 with Adj-SID label 24000 (V and L
/// flags).
#[must_use]
pub fn ospfv2_ext_link_lsa() -> ProtocolDef {
    let tlv_types = vec![TlvVariant::new(
        1,
        "extended_link",
        vec![
            enum_field(
                "link_type",
                8,
                &[
                    (1, "point_to_point"),
                    (2, "transit"),
                    (3, "stub"),
                    (4, "virtual"),
                ],
            ),
            u("reserved", 24),
            ip4("link_id"),
            ip4("link_data"),
            sub_tlvs(link_sub_tlvs()),
        ],
    )];
    let default = vec![TlvItem::named("extended_link")
        .with("link_type", 1u8)
        .with("link_id", std::net::Ipv4Addr::new(192, 0, 2, 2))
        .with("link_data", std::net::Ipv4Addr::new(198, 51, 100, 1))
        .with(
            "sub_tlvs",
            vec![TlvItem::named("adj_sid")
                .with("flags", 0x60u8)
                .with("label", 24000u32)],
        )];
    protocol! {
        id: "ospfv2_ext_link_lsa",
        name: "OSPFv2 Extended Link LSA",
        layer: L3,
        fields: {
            tlvs: ty(tlvs(tlv_types)) = default, desc = "ospf-opaque-tlvs";
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitions_are_valid() {
        for d in [
            ospfv2_opaque_lsa(),
            ospfv2_te_lsa(),
            ospfv2_ri_lsa(),
            ospfv2_ext_prefix_lsa(),
            ospfv2_ext_link_lsa(),
        ] {
            d.validate().unwrap_or_else(|e| panic!("{}: {e}", d.id));
        }
    }
}
