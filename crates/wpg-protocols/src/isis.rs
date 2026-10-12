//! IS-IS (ISO/IEC 10589, RFC 1195) over the OSI encapsulation (TODO §2.1 P2, M7):
//! LAN / point-to-point Hellos, LSPs and CSNP / PSNP, with the main TLVs and
//! sub-TLVs.
//!
//! Every PDU is one layer: the 8-octet common header followed by the PDU-specific
//! fixed fields and the TLVs. An IS-IS PDU is announced by its NLPID `0x83` like
//! any OSI network-layer PDU ([`crate::osi::osi_peek`]); the parser decodes it as
//! the generic [`isis`] first, whose `pdu_type` then re-decodes the layer as the
//! dedicated PDU (`isis_l1_lan_hello`, `isis_l2_lsp`, ...). L1 and L2 variants of a
//! PDU are separate protocols because the PDU type is their only difference.
//!
//! Auto fields:
//!
//! * `pdu_length` is the length of the whole PDU.
//! * The LSP `checksum` is the ISO 8473 Fletcher checksum from the LSP ID to the
//!   end of the PDU (ISO 10589 §7.3.11): the rule leaves out the fields before the
//!   LSP ID, which as leading zeros do not change either Fletcher sum.
//! * Hellos are padded (ISO 10589 §8.2.3, Padding TLV 8) when `pdu_length` is
//!   given a Manual value: the builder appends as many Padding TLVs as needed for
//!   the PDU to be exactly that long, e.g. 1497 for an Ethernet MTU of 1500 minus
//!   the LLC header. Left on Auto, no padding is added.
//! * Sub-TLV lengths of the extended IS / IP reachability entries, the
//!   "sub-TLVs present" bits and the prefix sizes (`ceil(prefix_length / 8)`
//!   octets) follow the entries.
//!
//! `header_length` is constant for each PDU type (a constraint warns when it is
//! changed) and system IDs are 6 octets (`id_length` 0). TE bandwidths are the raw
//! 32-bit IEEE floating point values; HMAC-MD5 authentication values are not
//! computed.

use std::collections::BTreeMap;

use wpg_core::{
    Binding, BindingEntry, Constraint, ConstraintRule, DiagLevel, EnumVariant, FieldDef, FieldType,
    FieldValue, FlagBit, ItemDef, Length, LengthUnit, ProtocolDef, ProtocolLayer, TlvDef, TlvFill,
    TlvItem, TlvVariant,
};

/// Id of the generic IS-IS PDU ([`isis`]).
pub const ISIS: &str = "isis";

/// The dedicated PDUs: (PDU type, protocol id).
pub const PDUS: [(u64, &str); 9] = [
    (15, "isis_l1_lan_hello"),
    (16, "isis_l2_lan_hello"),
    (17, "isis_p2p_hello"),
    (18, "isis_l1_lsp"),
    (20, "isis_l2_lsp"),
    (24, "isis_l1_csnp"),
    (25, "isis_l2_csnp"),
    (26, "isis_l1_psnp"),
    (27, "isis_l2_psnp"),
];

const SYSTEM_ID: [u8; 6] = [0, 0, 0, 0, 0, 1];
const NEIGHBOR_ID: [u8; 6] = [0, 0, 0, 0, 0, 2];

fn u(name: &str, bits: u8) -> FieldDef {
    FieldDef::new(name, FieldType::UInt { bits })
}

fn desc(f: FieldDef, key: &str) -> FieldDef {
    f.with_description(key)
}

fn uint(name: &str, bits: u8, default: u64, key: &str) -> FieldDef {
    desc(u(name, bits).with_default(FieldValue::UInt(default)), key)
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

fn list(name: &str, item: ItemDef) -> FieldDef {
    FieldDef::new(
        name,
        FieldType::List {
            item,
            length: Length::ToEnd,
        },
    )
}

fn list_of(name: &str, ty: FieldType) -> FieldDef {
    list(name, ItemDef::Field { ty: Box::new(ty) })
}

fn structs(name: &str, fields: Vec<FieldDef>) -> FieldDef {
    list(name, ItemDef::Struct { fields })
}

/// IS-IS TLV framing: 8-bit code, 8-bit length of the value.
fn tlv_def(variants: Vec<TlvVariant>, length: Length) -> TlvDef {
    TlvDef {
        type_bits: 8,
        len_bits: 8,
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
        uncounted_octets: 0,
    }
}

/// `length` (Auto, size of `sub_tlvs`) followed by the `sub_tlvs` it bounds.
fn sub_tlv_fields(sub: Vec<TlvVariant>) -> [FieldDef; 2] {
    [
        u("sub_tlvs_length", 8).with_auto(wpg_core::AutoCompute::Size {
            field: "sub_tlvs".into(),
        }),
        FieldDef::new(
            "sub_tlvs",
            FieldType::Tlv(tlv_def(
                sub,
                Length::FromField {
                    field: "sub_tlvs_length".into(),
                    unit: 1,
                    adjust: 0,
                    div: 1,
                },
            )),
        ),
    ]
}

/// Sub-TLVs of the extended IS reachability (TLV 22, RFC 5305 / 5307 / 8667).
fn is_sub_tlvs() -> Vec<TlvVariant> {
    let sid = |extra: Vec<FieldDef>| {
        let mut f = vec![
            flags(
                "flags",
                8,
                &[(7, "f"), (6, "b"), (5, "v"), (4, "l"), (3, "s"), (2, "p")],
            ),
            u("weight", 8),
        ];
        f.extend(extra);
        f.push(to_end("sid"));
        f
    };
    vec![
        TlvVariant::new(3, "admin_group", vec![u("value", 32)]),
        TlvVariant::new(4, "link_ids", vec![u("local_id", 32), u("remote_id", 32)]),
        TlvVariant::new(
            6,
            "ipv4_interface_address",
            vec![FieldDef::new("address", FieldType::Ipv4)],
        ),
        TlvVariant::new(
            8,
            "ipv4_neighbor_address",
            vec![FieldDef::new("address", FieldType::Ipv4)],
        ),
        TlvVariant::new(9, "max_link_bandwidth", vec![u("value", 32)]),
        TlvVariant::new(10, "max_reservable_bandwidth", vec![u("value", 32)]),
        TlvVariant::new(11, "unreserved_bandwidth", vec![octets("values", 32)]),
        TlvVariant::new(18, "te_default_metric", vec![u("value", 24)]),
        TlvVariant::new(31, "adj_sid", sid(Vec::new())),
        TlvVariant::new(32, "lan_adj_sid", sid(vec![octets("neighbor_id", 6)])),
    ]
}

/// Sub-TLVs of the extended IP / IPv6 reachability (TLV 135 / 236, RFC 5130 /
/// 7794 / 8667).
fn prefix_sub_tlvs() -> Vec<TlvVariant> {
    vec![
        TlvVariant::new(
            1,
            "admin_tag",
            vec![list_of("tags", FieldType::UInt { bits: 32 })],
        ),
        TlvVariant::new(
            2,
            "admin_tag64",
            vec![list_of("tags", FieldType::UInt { bits: 64 })],
        ),
        TlvVariant::new(
            3,
            "prefix_sid",
            vec![
                flags(
                    "flags",
                    8,
                    &[(7, "r"), (6, "n"), (5, "p"), (4, "e"), (3, "v"), (2, "l")],
                ),
                u("algorithm", 8),
                to_end("sid"),
            ],
        ),
        TlvVariant::new(4, "prefix_attribute_flags", vec![to_end("flags")]),
    ]
}

/// One extended IP / IPv6 reachability entry: `head` (metric and flags up to the
/// prefix length), the prefix (`ceil(prefix_length / 8)` octets) and the optional
/// sub-TLV block, present when `sub_tlvs_present` (Auto) is set.
fn prefix_entry(head: Vec<FieldDef>) -> Vec<FieldDef> {
    let mut f = head;
    f.push(FieldDef::new(
        "prefix",
        FieldType::Bytes {
            len: Length::FromField {
                field: "prefix_length".into(),
                unit: 1,
                adjust: 7,
                div: 8,
            },
        },
    ));
    f.push(FieldDef::new(
        "sub_tlv_block",
        FieldType::List {
            item: ItemDef::Struct {
                fields: sub_tlv_fields(prefix_sub_tlvs()).into(),
            },
            length: Length::Count {
                field: "sub_tlvs_present".into(),
            },
        },
    ));
    f
}

fn sub_tlvs_present() -> FieldDef {
    u("sub_tlvs_present", 1).with_auto(wpg_core::AutoCompute::Count {
        field: "sub_tlv_block".into(),
    })
}

/// The four ISO 10589 metrics (I/E and S bits included) of the narrow-metric TLVs.
fn narrow_metrics() -> Vec<FieldDef> {
    vec![
        u("default_metric", 8).with_default(FieldValue::UInt(10)),
        u("delay_metric", 8).with_default(FieldValue::UInt(0x80)),
        u("expense_metric", 8).with_default(FieldValue::UInt(0x80)),
        u("error_metric", 8).with_default(FieldValue::UInt(0x80)),
    ]
}

/// Router capability sub-TLVs (TLV 242, RFC 8667).
fn capability_sub_tlvs() -> Vec<TlvVariant> {
    vec![
        TlvVariant::new(
            2,
            "sr_capabilities",
            vec![flags("flags", 8, &[(7, "i"), (6, "v")]), to_end("ranges")],
        ),
        TlvVariant::new(
            19,
            "sr_algorithms",
            vec![list_of("algorithms", FieldType::UInt { bits: 8 })],
        ),
    ]
}

/// The IS-IS TLVs (8-bit code / 8-bit length). Typed: area addresses (1), IS
/// reachability (2), IS neighbors (6), padding (8), LSP entries (9),
/// authentication (10), extended IS reachability (22), IP internal / external
/// reachability (128 / 130), protocols supported (129), IP interface addresses
/// (132), TE router ID (134), extended IP reachability (135), dynamic hostname
/// (137), IPv6 interface addresses (232), IPv6 reachability (236), P2P three-way
/// adjacency (240) and router capability (242); others are kept as octets.
/// `fill` pads with Padding TLVs (Hellos).
#[must_use]
pub fn isis_tlvs(fill: bool) -> FieldType {
    let nsap = ItemDef::Struct {
        fields: vec![FieldDef::new("address", FieldType::Nsap)],
    };
    let ip_reach = || {
        let mut f = narrow_metrics();
        f.push(FieldDef::new("ip_address", FieldType::Ipv4));
        f.push(FieldDef::new("subnet_mask", FieldType::Ipv4));
        vec![structs("prefixes", f)]
    };
    let nlpid = FieldType::Enum {
        bits: 8,
        variants: [(0x81u64, "clnp"), (0x8e, "ipv6"), (0xcc, "ipv4")]
            .iter()
            .map(|(v, n)| EnumVariant {
                value: *v,
                name: (*n).into(),
                description: None,
            })
            .collect(),
    };
    let mut is_neighbor = vec![octets("neighbor_id", 7), u("metric", 24)];
    is_neighbor.extend(sub_tlv_fields(is_sub_tlvs()));
    let mut is_reach = narrow_metrics();
    is_reach.push(octets("neighbor_id", 7));
    let mut t = tlv_def(
        vec![
            TlvVariant::new(1, "area_addresses", vec![list("addresses", nsap)]),
            TlvVariant::new(
                2,
                "is_reachability",
                vec![u("virtual_flag", 8), structs("neighbors", is_reach)],
            ),
            TlvVariant::new(
                6,
                "is_neighbors",
                vec![list_of("neighbors", FieldType::Mac)],
            ),
            TlvVariant::new(8, "padding", vec![to_end("data")]),
            TlvVariant::new(
                9,
                "lsp_entries",
                vec![structs(
                    "entries",
                    vec![
                        u("remaining_lifetime", 16),
                        octets("lsp_id", 8),
                        u("sequence_number", 32),
                        u("checksum", 16),
                    ],
                )],
            ),
            TlvVariant::new(
                10,
                "authentication",
                vec![
                    enum_field("auth_type", 8, &[(1, "cleartext"), (54, "hmac_md5")]),
                    to_end("value"),
                ],
            ),
            TlvVariant::new(
                22,
                "extended_is_reachability",
                vec![structs("neighbors", is_neighbor)],
            ),
            TlvVariant::new(128, "ip_internal_reachability", ip_reach()),
            TlvVariant::new(129, "protocols_supported", vec![list_of("nlpids", nlpid)]),
            TlvVariant::new(130, "ip_external_reachability", ip_reach()),
            TlvVariant::new(
                132,
                "ip_interface_addresses",
                vec![list_of("addresses", FieldType::Ipv4)],
            ),
            TlvVariant::new(
                134,
                "te_router_id",
                vec![FieldDef::new("router_id", FieldType::Ipv4)],
            ),
            TlvVariant::new(
                135,
                "extended_ip_reachability",
                vec![structs(
                    "prefixes",
                    prefix_entry(vec![
                        u("metric", 32),
                        u("up_down", 1),
                        sub_tlvs_present(),
                        u("prefix_length", 6),
                    ]),
                )],
            ),
            TlvVariant::new(137, "dynamic_hostname", vec![to_end("hostname")]),
            TlvVariant::new(
                232,
                "ipv6_interface_addresses",
                vec![list_of("addresses", FieldType::Ipv6)],
            ),
            TlvVariant::new(
                236,
                "ipv6_reachability",
                vec![structs(
                    "prefixes",
                    prefix_entry(vec![
                        u("metric", 32),
                        u("up_down", 1),
                        u("external", 1),
                        sub_tlvs_present(),
                        u("reserved", 5),
                        u("prefix_length", 8),
                    ]),
                )],
            ),
            TlvVariant::new(
                240,
                "p2p_adjacency_state",
                vec![
                    enum_field(
                        "adjacency_state",
                        8,
                        &[(0, "up"), (1, "initializing"), (2, "down")],
                    ),
                    u("extended_local_circuit_id", 32),
                    octets("neighbor_system_id", 6),
                    u("neighbor_extended_local_circuit_id", 32),
                ],
            ),
            TlvVariant::new(
                242,
                "router_capability",
                vec![
                    FieldDef::new("router_id", FieldType::Ipv4),
                    flags("flags", 8, &[(0, "s"), (1, "d")]),
                    FieldDef::new(
                        "sub_tlvs",
                        FieldType::Tlv(tlv_def(capability_sub_tlvs(), Length::ToEnd)),
                    ),
                ],
            ),
        ],
        Length::ToEnd,
    );
    if fill {
        t.fill = Some(TlvFill {
            variant: "padding".into(),
            until: "pdu_length".into(),
        });
    }
    FieldType::Tlv(t)
}

/// The 8-octet common header with the PDU type and its fixed header length.
fn common_header(pdu_type: u64, header_length: u64) -> Vec<FieldDef> {
    vec![
        desc(
            enum_field("irpd", 8, &[(0x83, "isis")]).with_default(FieldValue::UInt(0x83)),
            "isis-irpd",
        ),
        uint("header_length", 8, header_length, "isis-header-length"),
        uint("version_ext", 8, 1, "isis-version-ext"),
        uint("id_length", 8, 0, "isis-id-length"),
        u("reserved", 3).with_default(FieldValue::UInt(0)),
        desc(
            enum_field(
                "pdu_type",
                5,
                &[
                    (15, "l1_lan_hello"),
                    (16, "l2_lan_hello"),
                    (17, "p2p_hello"),
                    (18, "l1_lsp"),
                    (20, "l2_lsp"),
                    (24, "l1_csnp"),
                    (25, "l2_csnp"),
                    (26, "l1_psnp"),
                    (27, "l2_psnp"),
                ],
            )
            .with_default(FieldValue::UInt(pdu_type)),
            "isis-pdu-type",
        ),
        uint("version", 8, 1, "isis-version"),
        u("reserved2", 8).with_default(FieldValue::UInt(0)),
        uint("max_area_addresses", 8, 0, "isis-max-area-addresses"),
    ]
}

fn pdu_length() -> FieldDef {
    desc(
        u("pdu_length", 16).with_auto(wpg_core::AutoCompute::Length {
            of: wpg_core::LengthOf::Total,
            unit: LengthUnit::Bytes,
            adjust: 0,
        }),
        "isis-pdu-length",
    )
}

fn bytes_default(f: FieldDef, v: &[u8], key: &str) -> FieldDef {
    desc(f.with_default(FieldValue::Bytes(v.to_vec())), key)
}

fn ext(base: [u8; 6], tail: &[u8]) -> Vec<u8> {
    let mut v = base.to_vec();
    v.extend_from_slice(tail);
    v
}

fn pdu(
    id: &str,
    name: &str,
    pdu_type: u64,
    header_length: u64,
    body: Vec<FieldDef>,
    tlvs: (FieldType, Vec<TlvItem>),
) -> ProtocolDef {
    let mut d = ProtocolDef::new(id, name, ProtocolLayer::L3);
    d.fields = common_header(pdu_type, header_length);
    d.fields.extend(body);
    d.fields.push(desc(
        FieldDef::new("tlvs", tlvs.0).with_default(FieldValue::Tlv(tlvs.1)),
        "isis-tlvs",
    ));
    d.constraints.push(Constraint::new(
        DiagLevel::Warning,
        ConstraintRule::FieldEquals {
            field: "header_length".into(),
            #[allow(clippy::cast_possible_wrap)] // small constants
            value: header_length as i64,
        },
    ));
    d
}

fn nsap_item(area: &[u8]) -> FieldValue {
    FieldValue::Struct(BTreeMap::from([(
        "address".to_owned(),
        FieldValue::Nsap(area.to_vec()),
    )]))
}

fn area_tlv() -> TlvItem {
    TlvItem::named("area_addresses").with("addresses", vec![nsap_item(&[0x49, 0x00, 0x01])])
}

fn protocols_tlv() -> TlvItem {
    TlvItem::named("protocols_supported").with("nlpids", vec![FieldValue::UInt(0xcc)])
}

fn ip_interface_tlv() -> TlvItem {
    TlvItem::named("ip_interface_addresses")
        .with("addresses", vec![FieldValue::Ipv4([192, 0, 2, 1].into())])
}

fn circuit_type(default: u64) -> [FieldDef; 2] {
    [
        u("reserved_ct", 6).with_default(FieldValue::UInt(0)),
        desc(
            enum_field("circuit_type", 2, &[(1, "l1"), (2, "l2"), (3, "l1_l2")])
                .with_default(FieldValue::UInt(default)),
            "isis-circuit-type",
        ),
    ]
}

fn lan_hello(id: &str, name: &str, pdu_type: u64) -> ProtocolDef {
    let mut body = circuit_type(3).to_vec();
    body.extend([
        bytes_default(octets("source_id", 6), &SYSTEM_ID, "isis-source-id"),
        uint("holding_time", 16, 30, "isis-holding-time"),
        pdu_length(),
        u("reserved_pri", 1).with_default(FieldValue::UInt(0)),
        uint("priority", 7, 64, "isis-priority"),
        bytes_default(octets("lan_id", 7), &ext(SYSTEM_ID, &[1]), "isis-lan-id"),
    ]);
    let tlvs = vec![protocols_tlv(), area_tlv(), ip_interface_tlv()];
    pdu(id, name, pdu_type, 27, body, (isis_tlvs(true), tlvs))
}

/// IS-IS Level 1 LAN Hello (PDU type 15, ISO 10589 §9.5): circuit type, source
/// ID, holding time, PDU length (Auto; Manual pads with Padding TLVs), priority,
/// LAN ID and the TLVs. Sent to `01:80:c2:00:00:14`.
#[must_use]
pub fn isis_l1_lan_hello() -> ProtocolDef {
    lan_hello("isis_l1_lan_hello", "IS-IS L1 LAN Hello", 15)
}

/// IS-IS Level 2 LAN Hello (PDU type 16): as [`isis_l1_lan_hello`], sent to
/// `01:80:c2:00:00:15`.
#[must_use]
pub fn isis_l2_lan_hello() -> ProtocolDef {
    lan_hello("isis_l2_lan_hello", "IS-IS L2 LAN Hello", 16)
}

/// IS-IS Point-to-Point Hello (PDU type 17, ISO 10589 §9.7): circuit type,
/// source ID, holding time, PDU length (Auto; Manual pads), local circuit ID and
/// the TLVs, including the three-way adjacency state (RFC 5303).
#[must_use]
pub fn isis_p2p_hello() -> ProtocolDef {
    let mut body = circuit_type(3).to_vec();
    body.extend([
        bytes_default(octets("source_id", 6), &SYSTEM_ID, "isis-source-id"),
        uint("holding_time", 16, 30, "isis-holding-time"),
        pdu_length(),
        uint("local_circuit_id", 8, 1, "isis-local-circuit-id"),
    ]);
    let adjacency = TlvItem::named("p2p_adjacency_state")
        .with("adjacency_state", 2u8)
        .with("extended_local_circuit_id", 1u32)
        .with("neighbor_system_id", vec![0u8; 6])
        .with("neighbor_extended_local_circuit_id", 0u32);
    let tlvs = vec![protocols_tlv(), area_tlv(), ip_interface_tlv(), adjacency];
    pdu(
        "isis_p2p_hello",
        "IS-IS P2P Hello",
        17,
        20,
        body,
        (isis_tlvs(true), tlvs),
    )
}

fn lsp(id: &str, name: &str, pdu_type: u64, is_type: u64) -> ProtocolDef {
    const BEFORE_LSP_ID: [&str; 11] = [
        "irpd",
        "header_length",
        "version_ext",
        "id_length",
        "reserved",
        "pdu_type",
        "version",
        "reserved2",
        "max_area_addresses",
        "pdu_length",
        "remaining_lifetime",
    ];
    let body = vec![
        pdu_length(),
        uint("remaining_lifetime", 16, 1199, "isis-remaining-lifetime"),
        bytes_default(octets("lsp_id", 8), &ext(SYSTEM_ID, &[0, 0]), "isis-lsp-id"),
        uint("sequence_number", 32, 1, "isis-sequence-number"),
        desc(
            u("checksum", 16).with_auto(wpg_core::AutoCompute::Checksum {
                algorithm: wpg_core::ChecksumKind::Fletcher,
                covers: wpg_core::ChecksumCoverage::HeaderAndPayload,
                nonzero: false,
                exclude: BEFORE_LSP_ID.iter().map(|s| (*s).to_owned()).collect(),
            }),
            "isis-lsp-checksum",
        ),
        uint("partition_repair", 1, 0, "isis-partition-repair"),
        desc(
            flags(
                "attached",
                4,
                &[(0, "default"), (1, "delay"), (2, "expense"), (3, "error")],
            )
            .with_default(FieldValue::UInt(0)),
            "isis-attached",
        ),
        uint("overload", 1, 0, "isis-overload"),
        desc(
            enum_field("is_type", 2, &[(1, "l1"), (3, "l1_l2")])
                .with_default(FieldValue::UInt(is_type)),
            "isis-is-type",
        ),
    ];
    let tlvs = vec![
        area_tlv(),
        protocols_tlv(),
        TlvItem::named("dynamic_hostname").with("hostname", b"r1".to_vec()),
        ip_interface_tlv(),
        TlvItem::named("extended_is_reachability").with(
            "neighbors",
            vec![FieldValue::Struct(BTreeMap::from([
                (
                    "neighbor_id".to_owned(),
                    FieldValue::Bytes(ext(NEIGHBOR_ID, &[0])),
                ),
                ("metric".to_owned(), FieldValue::UInt(10)),
            ]))],
        ),
        TlvItem::named("extended_ip_reachability").with(
            "prefixes",
            vec![FieldValue::Struct(BTreeMap::from([
                ("metric".to_owned(), FieldValue::UInt(10)),
                ("prefix_length".to_owned(), FieldValue::UInt(24)),
                ("prefix".to_owned(), FieldValue::Bytes(vec![192, 0, 2])),
            ]))],
        ),
    ];
    pdu(id, name, pdu_type, 27, body, (isis_tlvs(false), tlvs))
}

/// IS-IS Level 1 LSP (PDU type 18, ISO 10589 §9.8): PDU length (Auto), remaining
/// lifetime, LSP ID, sequence number, Fletcher checksum (Auto), P / ATT / OL /
/// IS type and the TLVs. Default: area 49.0001, IPv4, hostname `r1`, one
/// neighbor and 192.0.2.0/24.
#[must_use]
pub fn isis_l1_lsp() -> ProtocolDef {
    lsp("isis_l1_lsp", "IS-IS L1 LSP", 18, 1)
}

/// IS-IS Level 2 LSP (PDU type 20): as [`isis_l1_lsp`] with IS type L1/L2.
#[must_use]
pub fn isis_l2_lsp() -> ProtocolDef {
    lsp("isis_l2_lsp", "IS-IS L2 LSP", 20, 3)
}

fn lsp_entries_tlv() -> TlvItem {
    TlvItem::named("lsp_entries").with(
        "entries",
        vec![FieldValue::Struct(BTreeMap::from([
            ("remaining_lifetime".to_owned(), FieldValue::UInt(1199)),
            (
                "lsp_id".to_owned(),
                FieldValue::Bytes(ext(SYSTEM_ID, &[0, 0])),
            ),
            ("sequence_number".to_owned(), FieldValue::UInt(1)),
            ("checksum".to_owned(), FieldValue::UInt(0)),
        ]))],
    )
}

fn csnp(id: &str, name: &str, pdu_type: u64) -> ProtocolDef {
    let body = vec![
        pdu_length(),
        bytes_default(
            octets("source_id", 7),
            &ext(SYSTEM_ID, &[0]),
            "isis-snp-source-id",
        ),
        bytes_default(octets("start_lsp_id", 8), &[0; 8], "isis-start-lsp-id"),
        bytes_default(octets("end_lsp_id", 8), &[0xff; 8], "isis-end-lsp-id"),
    ];
    pdu(
        id,
        name,
        pdu_type,
        33,
        body,
        (isis_tlvs(false), vec![lsp_entries_tlv()]),
    )
}

/// IS-IS Level 1 CSNP (PDU type 24, ISO 10589 §9.10): PDU length (Auto), source
/// ID, start / end LSP ID and LSP entries (TLV 9).
#[must_use]
pub fn isis_l1_csnp() -> ProtocolDef {
    csnp("isis_l1_csnp", "IS-IS L1 CSNP", 24)
}

/// IS-IS Level 2 CSNP (PDU type 25).
#[must_use]
pub fn isis_l2_csnp() -> ProtocolDef {
    csnp("isis_l2_csnp", "IS-IS L2 CSNP", 25)
}

fn psnp(id: &str, name: &str, pdu_type: u64) -> ProtocolDef {
    let body = vec![
        pdu_length(),
        bytes_default(
            octets("source_id", 7),
            &ext(SYSTEM_ID, &[0]),
            "isis-snp-source-id",
        ),
    ];
    pdu(
        id,
        name,
        pdu_type,
        17,
        body,
        (isis_tlvs(false), vec![lsp_entries_tlv()]),
    )
}

/// IS-IS Level 1 PSNP (PDU type 26, ISO 10589 §9.12): PDU length (Auto), source
/// ID and LSP entries (TLV 9).
#[must_use]
pub fn isis_l1_psnp() -> ProtocolDef {
    psnp("isis_l1_psnp", "IS-IS L1 PSNP", 26)
}

/// IS-IS Level 2 PSNP (PDU type 27).
#[must_use]
pub fn isis_l2_psnp() -> ProtocolDef {
    psnp("isis_l2_psnp", "IS-IS L2 PSNP", 27)
}

/// Generic IS-IS PDU: the common header and the rest as opaque octets. The parser
/// reaches it through the NLPID and re-decodes the layer as the dedicated PDU of
/// its `pdu_type`; unknown PDU types stay here.
#[must_use]
pub fn isis() -> ProtocolDef {
    let mut d = ProtocolDef::new(ISIS, "IS-IS", ProtocolLayer::L3);
    d.fields = common_header(15, 27);
    d.fields.push(desc(
        to_end("data").with_default(FieldValue::Bytes(Vec::new())),
        "isis-data",
    ));
    d.next = Some(Binding::new(
        "pdu_type",
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
        for d in [
            isis(),
            isis_l1_lan_hello(),
            isis_l2_lan_hello(),
            isis_p2p_hello(),
            isis_l1_lsp(),
            isis_l2_lsp(),
            isis_l1_csnp(),
            isis_l2_csnp(),
            isis_l1_psnp(),
            isis_l2_psnp(),
        ] {
            d.validate().unwrap_or_else(|e| panic!("{}: {e}", d.id));
        }
    }
}
