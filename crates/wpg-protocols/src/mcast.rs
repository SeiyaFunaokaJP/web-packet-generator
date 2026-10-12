//! Multicast control protocols (TODO §2.1 P1): `IGMPv1` / v2 / v3 (IP protocol 2),
//! `MLDv1` / v2 (ICMPv6 types 130 / 131 / 132 / 143) and PIM-SM v2 Hello /
//! Join-Prune / Register (IP protocol 103).
//!
//! Like ICMPv6 ([`crate::icmp`]), IGMP and PIM are split into a common header
//! (`type`, checksum) whose Auto `type` comes from the following body protocol,
//! and one body protocol per message layout. MLD bodies follow
//! [`crate::icmp::icmpv6`].
//!
//! ## Engine limitations exercised here
//!
//! * A v2 and a v3 **query** (IGMP type 0x11, MLD type 130) differ only in length.
//!   The binding entries for the v3 bodies carry a minimum remaining length
//!   ([`wpg_core::BindingEntry::min_len`]: 8 octets for `IGMPv3`, 24 for `MLDv2`),
//!   so a long query decodes as `igmpv3_query` / `mldv2_query` and a short one as
//!   `igmpv2` / `mld`.
//! * Record / source **counts** (`num_group_records`, `num_sources` inside a group
//!   record, PIM `num_groups` / `num_joined` / `num_pruned`, the query
//!   `num_sources`) are Auto ([`wpg_core::AutoCompute::Count`]), also inside list
//!   elements.
//! * **PIM Register**'s checksum covers only the first 8 octets (PIM header +
//!   Register flags), not the encapsulated packet. A checksum coverage of "the
//!   next layer's header too" does not exist, so [`pim_register`] is a standalone
//!   definition that repeats the PIM header; its `checksum(internet, header)` then
//!   covers exactly those 8 octets. IP protocol 103 decodes as `pim` (the Register
//!   body then parses as `raw`); `pim_register` is listed second for building. The
//!   encapsulated packet is IPv4 or IPv6 by its first nibble.
//! * **PIM over IPv6** needs the IPv6 pseudo header in its checksum, PIM over IPv4
//!   does not. A checksum kind cannot depend on the lower layer, so `pim` uses the
//!   plain Internet checksum and is bound from IPv4 only.
//! * PIM encoded addresses are modelled for IPv4 (address family 1) only.

use std::collections::BTreeMap;

use wpg_core::{
    protocol, AutoCompute, FieldDef, FieldType, FieldValue, FlagBit, ItemDef, Length, LengthUnit,
    ProtocolDef, TlvDef, TlvItem, TlvVariant,
};

fn u(name: &str, bits: u8, default: u64) -> FieldDef {
    FieldDef::new(name, FieldType::UInt { bits }).with_default(FieldValue::UInt(default))
}

/// An Auto element count of the `list` field `of` (same field group).
fn count(name: &str, bits: u8, of: &str) -> FieldDef {
    FieldDef::new(name, FieldType::UInt { bits }).with_auto(AutoCompute::Count { field: of.into() })
}

fn from_field(field: &str, unit: u32) -> Length {
    Length::FromField {
        field: field.into(),
        unit,
        adjust: 0,
        div: 1,
    }
}

fn addr_list(ty: FieldType, count: &str, unit: u32) -> FieldType {
    FieldType::List {
        item: ItemDef::Field { ty: Box::new(ty) },
        length: from_field(count, unit),
    }
}

/// `IGMPv3` / `MLDv2` group record type (RFC 3376 §4.2.12).
fn record_type() -> FieldDef {
    FieldDef::new(
        "record_type",
        FieldType::Enum {
            bits: 8,
            variants: [
                (1, "mode_is_include"),
                (2, "mode_is_exclude"),
                (3, "change_to_include"),
                (4, "change_to_exclude"),
                (5, "allow_new_sources"),
                (6, "block_old_sources"),
            ]
            .into_iter()
            .map(|(value, name)| wpg_core::EnumVariant {
                value,
                name: name.into(),
                description: None,
            })
            .collect(),
        },
    )
    .with_default(FieldValue::UInt(4))
}

/// Group records of an `IGMPv3` (`addr` = IPv4, 4 octets) or `MLDv2` (IPv6, 16)
/// report: type, aux data length (32-bit words), source count, multicast
/// address, sources and auxiliary data.
fn group_records(addr: &FieldType, addr_octets: u32) -> FieldType {
    let mcast = match addr {
        FieldType::Ipv4 => FieldValue::Ipv4([239, 1, 1, 1].into()),
        _ => FieldValue::Ipv6([0xff05u16, 0, 0, 0, 0, 0, 0, 0x1].into()),
    };
    FieldType::List {
        item: ItemDef::Struct {
            fields: vec![
                record_type(),
                u("aux_data_len", 8, 0),
                count("num_sources", 16, "sources"),
                FieldDef::new("multicast_address", addr.clone()).with_default(mcast),
                FieldDef::new(
                    "sources",
                    addr_list(addr.clone(), "num_sources", addr_octets),
                )
                .with_default(FieldValue::List(Vec::new())),
                FieldDef::new(
                    "aux_data",
                    FieldType::Bytes {
                        len: from_field("aux_data_len", 4),
                    },
                )
                .with_default(FieldValue::Bytes(Vec::new())),
            ],
        },
        length: Length::ToEnd,
    }
}

/// One default group record (Change-To-Exclude with no sources = join `addr`).
fn join_record(addr: FieldValue) -> FieldValue {
    FieldValue::Struct(BTreeMap::from([
        ("record_type".to_owned(), FieldValue::UInt(4)),
        ("aux_data_len".to_owned(), FieldValue::UInt(0)),
        ("multicast_address".to_owned(), addr),
        ("sources".to_owned(), FieldValue::List(Vec::new())),
        ("aux_data".to_owned(), FieldValue::Bytes(Vec::new())),
    ]))
}

// --------------------------------------------------------------------------
// IGMP (RFC 1112 / RFC 2236 / RFC 3376), IP protocol 2
// --------------------------------------------------------------------------

/// IGMP common header: `type` (Auto from the body protocol), `max_resp_code`
/// (1/10 s; reserved in v1 and v3 reports) and the Internet `checksum` (Auto, over
/// the whole message). The body follows as `igmpv2`, `igmpv3_query` or
/// `igmpv3_report`; a type 0x11 query of 12 octets or more decodes as `IGMPv3`.
#[must_use]
pub fn igmp() -> ProtocolDef {
    let mut d = protocol! {
        id: "igmp",
        name: "IGMP",
        layer: L3,
        fields: {
            type: enum(8) {
                0x11 => membership_query,
                0x12 => v1_membership_report,
                0x16 => v2_membership_report,
                0x17 => leave_group,
                0x22 => v3_membership_report,
            } = 0x11, auto = next_protocol, desc = "igmp-type";
            max_resp_code: uint(8) = 100, desc = "igmp-max-resp-code";
            checksum: uint(16), auto = checksum(internet, header_payload), desc = "igmp-checksum";
        },
        next: type {
            0x11 => "igmpv2",
            0x12 => "igmpv2",
            0x16 => "igmpv2",
            0x17 => "igmpv2",
            0x11 => "igmpv3_query",
            0x22 => "igmpv3_report",
        },
    };
    v3_by_length(&mut d, "igmpv3_query", 8);
    d
}

/// Makes the binding entry for the v3 query body apply only to messages with at
/// least `min` octets after the common header, and moves it in front of the
/// v1/v2 entry for the same type.
pub(crate) fn v3_by_length(d: &mut ProtocolDef, v3: &str, min: u32) {
    if let Some(b) = d.next.as_mut() {
        if let Some(k) = b.table.iter().position(|e| e.protocol == v3) {
            let e = b.table.remove(k).with_min_len(min);
            b.table.insert(0, e);
        }
    }
}

/// `IGMPv1` / v2 body (query, report, leave): the group address (0.0.0.0 in a
/// general query). When it follows `igmp` the type is Auto 0x11 (query); set
/// `type` to 0x12 / 0x16 / 0x17 for a v1 report, v2 report or leave.
#[must_use]
pub fn igmpv2() -> ProtocolDef {
    protocol! {
        id: "igmpv2",
        name: "IGMPv1/v2",
        layer: L3,
        fields: {
            group: ipv4 = [0, 0, 0, 0], desc = "igmp-group";
        },
    }
}

/// `IGMPv3` Membership Query body (RFC 3376 §4.1, type 0x11): group address, S
/// flag, QRV, QQIC, `num_sources` (Auto) and the source addresses.
#[must_use]
pub fn igmpv3_query() -> ProtocolDef {
    protocol! {
        id: "igmpv3_query",
        name: "IGMPv3 Query",
        layer: L3,
        fields: {
            group: ipv4 = [0, 0, 0, 0], desc = "igmp-group";
            reserved: uint(4) = 0;
            s: uint(1) = 0, desc = "igmp-s-flag";
            qrv: uint(3) = 2, desc = "igmp-qrv";
            qqic: uint(8) = 125, desc = "igmp-qqic";
            num_sources: uint(16), auto = count(sources), desc = "igmp-num-sources";
            sources: ty(addr_list(FieldType::Ipv4, "num_sources", 4)) = Vec::<FieldValue>::new(), desc = "igmp-sources";
        },
    }
}

/// `IGMPv3` Membership Report body (RFC 3376 §4.2, type 0x22): reserved,
/// `num_group_records` (Auto) and the group records. Defaults to one
/// Change-To-Exclude record for 239.1.1.1 (a join).
#[must_use]
pub fn igmpv3_report() -> ProtocolDef {
    protocol! {
        id: "igmpv3_report",
        name: "IGMPv3 Report",
        layer: L3,
        fields: {
            reserved: uint(16) = 0;
            num_group_records: uint(16), auto = count(group_records), desc = "igmp-num-records";
            group_records: ty(group_records(&FieldType::Ipv4, 4)) = vec![join_record(FieldValue::Ipv4([239, 1, 1, 1].into()))], desc = "igmp-records";
        },
    }
}

// --------------------------------------------------------------------------
// MLD (RFC 2710 / RFC 3810), ICMPv6 types 130 / 131 / 132 / 143
// --------------------------------------------------------------------------

/// `MLDv1` body (RFC 2710; ICMPv6 type 130 query, 131 report, 132 done): maximum
/// response delay (ms), reserved and the multicast address (`::` in a general
/// query). When it follows `icmpv6` the type is Auto 130; set it to 131 / 132 for
/// a report / done.
#[must_use]
pub fn mld() -> ProtocolDef {
    protocol! {
        id: "mld",
        name: "MLDv1",
        layer: L3,
        fields: {
            max_resp_delay: uint(16) = 10000, desc = "mld-max-resp";
            reserved: uint(16) = 0;
            multicast_address: ipv6 = [0u16, 0, 0, 0, 0, 0, 0, 0], desc = "mld-address";
        },
    }
}

/// `MLDv2` Query body (RFC 3810 §5.1, ICMPv6 type 130): maximum response code,
/// multicast address, S flag, QRV, QQIC, `num_sources` (Auto) and the source
/// addresses.
#[must_use]
pub fn mldv2_query() -> ProtocolDef {
    protocol! {
        id: "mldv2_query",
        name: "MLDv2 Query",
        layer: L3,
        fields: {
            max_resp_code: uint(16) = 10000, desc = "mld-max-resp";
            reserved: uint(16) = 0;
            multicast_address: ipv6 = [0u16, 0, 0, 0, 0, 0, 0, 0], desc = "mld-address";
            reserved2: uint(4) = 0;
            s: uint(1) = 0, desc = "igmp-s-flag";
            qrv: uint(3) = 2, desc = "igmp-qrv";
            qqic: uint(8) = 125, desc = "igmp-qqic";
            num_sources: uint(16), auto = count(sources), desc = "igmp-num-sources";
            sources: ty(addr_list(FieldType::Ipv6, "num_sources", 16)) = Vec::<FieldValue>::new(), desc = "igmp-sources";
        },
    }
}

/// `MLDv2` Report body (RFC 3810 §5.2, ICMPv6 type 143): reserved,
/// `num_records` (Auto) and the multicast address records. Defaults to one
/// Change-To-Exclude record for `ff05::1` (a join).
#[must_use]
pub fn mldv2_report() -> ProtocolDef {
    protocol! {
        id: "mldv2_report",
        name: "MLDv2 Report",
        layer: L3,
        fields: {
            reserved: uint(16) = 0;
            num_records: uint(16), auto = count(records), desc = "igmp-num-records";
            records: ty(group_records(&FieldType::Ipv6, 16)) = vec![join_record(FieldValue::Ipv6([0xff05u16, 0, 0, 0, 0, 0, 0, 1].into()))], desc = "igmp-records";
        },
    }
}

// --------------------------------------------------------------------------
// PIM-SM v2 (RFC 7761), IP protocol 103
// --------------------------------------------------------------------------

/// PIM common header: version 2, `type` (Auto from the body), reserved and the
/// Internet `checksum` (Auto, over the whole message). Bodies: `pim_hello` and
/// `pim_join_prune`; a Register is the standalone [`pim_register`].
#[must_use]
pub fn pim() -> ProtocolDef {
    protocol! {
        id: "pim",
        name: "PIM",
        layer: L3,
        fields: {
            version: uint(4) = 2, desc = "pim-version";
            type: enum(4) {
                0 => hello,
                1 => register,
                2 => register_stop,
                3 => join_prune,
                4 => bootstrap,
                5 => assert,
                6 => graft,
                7 => graft_ack,
                8 => candidate_rp_advertisement,
            } = 0, auto = next_protocol, desc = "pim-type";
            reserved: uint(8) = 0;
            checksum: uint(16), auto = checksum(internet, header_payload), desc = "pim-checksum";
        },
        next: type {
            0 => "pim_hello",
            3 => "pim_join_prune",
        },
        constraints: {
            warning: equals(version, 2);
        },
    }
}

/// PIM Hello options: 16-bit type, 16-bit length excluding the header.
#[must_use]
pub fn pim_hello_options() -> FieldType {
    FieldType::Tlv(TlvDef {
        type_bits: 16,
        len_bits: 16,
        len_includes_header: false,
        len_unit: LengthUnit::Bytes,
        variants: vec![
            TlvVariant::new(1, "holdtime", vec![u("holdtime", 16, 105)]),
            TlvVariant::new(
                2,
                "lan_prune_delay",
                vec![
                    u("t", 1, 0),
                    u("propagation_delay", 15, 500),
                    u("override_interval", 16, 2500),
                ],
            ),
            TlvVariant::new(19, "dr_priority", vec![u("priority", 32, 1)]),
            TlvVariant::new(20, "generation_id", vec![u("generation_id", 32, 0)]),
            TlvVariant::new(24, "address_list", Vec::new()),
        ],
        unknown_as_raw: true,
        length: Length::ToEnd,
        fill: None,
        head_octets: 0,
        align: 0,
        unpadded_last: false,
        long_length: None,
    })
}

/// PIM Hello body (type 0): the option TLVs ([`pim_hello_options`]). Defaults to
/// Holdtime 105 s, DR Priority 1 and a Generation ID.
#[must_use]
pub fn pim_hello() -> ProtocolDef {
    protocol! {
        id: "pim_hello",
        name: "PIM Hello",
        layer: L3,
        fields: {
            options: ty(pim_hello_options()) = vec![
                TlvItem::named("holdtime"),
                TlvItem::named("dr_priority"),
                TlvItem::named("generation_id"),
            ], desc = "pim-hello-options";
        },
    }
}

/// Encoded-Source address (RFC 7761 §4.9.1, IPv4): family, encoding, S/W/R flags,
/// mask length and address — 8 octets.
fn encoded_sources(count: &str) -> FieldType {
    FieldType::List {
        item: ItemDef::Struct {
            fields: vec![
                u("family", 8, 1),
                u("encoding", 8, 0),
                FieldDef::new(
                    "flags",
                    FieldType::Flags {
                        bits: 8,
                        flags: vec![
                            FlagBit {
                                bit: 2,
                                name: "sparse".into(),
                            },
                            FlagBit {
                                bit: 1,
                                name: "wildcard".into(),
                            },
                            FlagBit {
                                bit: 0,
                                name: "rpt".into(),
                            },
                        ],
                    },
                )
                .with_default(FieldValue::UInt(0x04)),
                u("mask_len", 8, 32),
                FieldDef::new("address", FieldType::Ipv4)
                    .with_default(FieldValue::Ipv4([192, 0, 2, 10].into())),
            ],
        },
        length: from_field(count, 8),
    }
}

/// Join/Prune group sets: Encoded-Group address, join / prune source counts and
/// the joined / pruned Encoded-Source lists.
fn join_prune_groups() -> FieldType {
    FieldType::List {
        item: ItemDef::Struct {
            fields: vec![
                u("family", 8, 1),
                u("encoding", 8, 0),
                FieldDef::new(
                    "flags",
                    FieldType::Flags {
                        bits: 8,
                        flags: vec![
                            FlagBit {
                                bit: 7,
                                name: "bidir".into(),
                            },
                            FlagBit {
                                bit: 0,
                                name: "admin_scope".into(),
                            },
                        ],
                    },
                )
                .with_default(FieldValue::UInt(0)),
                u("mask_len", 8, 32),
                FieldDef::new("group_address", FieldType::Ipv4)
                    .with_default(FieldValue::Ipv4([239, 1, 1, 1].into())),
                count("num_joined", 16, "joined"),
                count("num_pruned", 16, "pruned"),
                FieldDef::new("joined", encoded_sources("num_joined"))
                    .with_default(FieldValue::List(Vec::new())),
                FieldDef::new("pruned", encoded_sources("num_pruned"))
                    .with_default(FieldValue::List(Vec::new())),
            ],
        },
        length: Length::ToEnd,
    }
}

/// One default group set: join (S,G) = (192.0.2.10, 239.1.1.1).
fn default_group_set() -> FieldValue {
    let source = FieldValue::Struct(BTreeMap::from([
        ("family".to_owned(), FieldValue::UInt(1)),
        ("encoding".to_owned(), FieldValue::UInt(0)),
        ("flags".to_owned(), FieldValue::UInt(0x04)),
        ("mask_len".to_owned(), FieldValue::UInt(32)),
        (
            "address".to_owned(),
            FieldValue::Ipv4([192, 0, 2, 10].into()),
        ),
    ]));
    FieldValue::Struct(BTreeMap::from([
        ("family".to_owned(), FieldValue::UInt(1)),
        ("encoding".to_owned(), FieldValue::UInt(0)),
        ("flags".to_owned(), FieldValue::UInt(0)),
        ("mask_len".to_owned(), FieldValue::UInt(32)),
        (
            "group_address".to_owned(),
            FieldValue::Ipv4([239, 1, 1, 1].into()),
        ),
        ("joined".to_owned(), FieldValue::List(vec![source])),
        ("pruned".to_owned(), FieldValue::List(Vec::new())),
    ]))
}

/// PIM Join/Prune body (type 3): Encoded-Unicast upstream neighbor (IPv4),
/// `num_groups` (Auto), holdtime (s) and the group sets.
#[must_use]
pub fn pim_join_prune() -> ProtocolDef {
    protocol! {
        id: "pim_join_prune",
        name: "PIM Join/Prune",
        layer: L3,
        fields: {
            upstream_family: uint(8) = 1, desc = "pim-addr-family";
            upstream_encoding: uint(8) = 0;
            upstream_neighbor: ipv4 = [192, 0, 2, 1], desc = "pim-upstream-neighbor";
            reserved: uint(8) = 0;
            num_groups: uint(8), auto = count(groups), desc = "pim-num-groups";
            holdtime: uint(16) = 210, desc = "pim-holdtime";
            groups: ty(join_prune_groups()) = vec![default_group_set()], desc = "pim-groups";
        },
    }
}

/// PIM Register (type 1) as one standalone 8-octet header: the PIM header,
/// `checksum` (Auto over these 8 octets only) and the Border / Null-Register bits.
/// The encapsulated multicast packet follows (IPv4 or IPv6 by its first nibble).
/// See the module docs for why it does not stack on [`pim`].
#[must_use]
pub fn pim_register() -> ProtocolDef {
    let mut d = protocol! {
        id: "pim_register",
        name: "PIM Register",
        layer: L3,
        fields: {
            version: uint(4) = 2, desc = "pim-version";
            type: uint(4) = 1, desc = "pim-type";
            reserved: uint(8) = 0;
            checksum: uint(16), auto = checksum(internet, header), desc = "pim-checksum";
            border: uint(1) = 0, desc = "pim-register-border";
            null_register: uint(1) = 0, desc = "pim-register-null";
            reserved2: uint(30) = 0;
        },
        next: type {},
        constraints: {
            warning: equals(type, 1);
        },
    };
    if let Some(b) = d.next.as_mut() {
        b.peek = Some(crate::encap::ip_version_peek(1));
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_defs_are_valid() {
        for d in [
            igmp(),
            igmpv2(),
            igmpv3_query(),
            igmpv3_report(),
            mld(),
            mldv2_query(),
            mldv2_report(),
            pim(),
            pim_hello(),
            pim_join_prune(),
            pim_register(),
        ] {
            d.validate()
                .unwrap_or_else(|e| panic!("{} invalid: {e}", d.id));
        }
        assert_eq!(igmp().fixed_header_bits(), Some(32));
        assert_eq!(mld().fixed_header_bits(), Some(160));
        assert_eq!(pim_register().fixed_header_bits(), Some(64));
    }
}
