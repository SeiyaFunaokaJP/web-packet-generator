//! Routing protocols (TODO §2.1 P1): OSPFv2 / OSPFv3 (Hello, DBD, LSR, LSU, `LSAck`
//! and the main LSAs), BGP-4 (OPEN, UPDATE, KEEPALIVE, NOTIFICATION, ROUTE-REFRESH
//! with the main path attributes and capabilities), `RIPv1` / v2, `RIPng` and EIGRP
//! (basic TLVs).
//!
//! Demultiplexing: IPv4 protocol 89 → [`ospfv2`], IPv6 next header 89 →
//! [`ospfv3`], IP protocol 88 → [`eigrp`] (both families), TCP destination port
//! 179 → [`bgp`], UDP destination port 520 → [`rip`] and 521 → [`ripng`].
//!
//! Each OSPF / BGP message is a common header layer whose `type` selects the body
//! layer (`ospfv2` → `ospfv2_hello`, `bgp` → `bgp_open`, ...). The header's
//! `type` (Auto, from the next layer), length (Auto, header + body) and checksum
//! are computed by the builder. A BGP KEEPALIVE is the `bgp` header alone (type 4,
//! the default).
//!
//! ## Engine limitations exercised here
//!
//! * **OSPFv2 checksum** is the Internet checksum of the whole packet *excluding*
//!   the 64-bit authentication field. The engine has no "skip a field" coverage,
//!   so [`ospfv2`] sums header + body including `authentication`: exact for null
//!   authentication (all-zero field, the default), wrong for simple-password
//!   authentication, and irrelevant for cryptographic authentication (checksum
//!   not computed, set it to 0 manually). The cryptographic-auth trailer after the
//!   packet is not modelled. **OSPFv3** uses the IPv6 pseudo header and is exact.
//! * **LSA checksum** (RFC 2328 §12.1.7) is the ISO Fletcher checksum of the LSA
//!   *without* its 2-octet LS age. The engine's Fletcher rule covers the whole
//!   LSA, which yields the identical value when `ls_age` is 0 (leading zero
//!   octets do not change either Fletcher sum) — the default here; with a
//!   non-zero age the Auto value is not the OSPF one. LSA headers inside DBD /
//!   `LSAck` carry Manual checksums.
//! * **LSU** carries a single typed LSA: [`ospfv2_lsu`] / [`ospfv3_lsu`] bind to
//!   an LSA header layer ([`ospfv2_lsa`] / [`ospfv3_lsa`]) whose `ls_type`
//!   selects the body layer. There is no repeated-layer construct, so further
//!   LSAs cannot follow a typed body. LS types sharing a body layout (v2 3 / 4 and
//!   5 / 7, v3 `0x4005` / `0x2007`) announce the first type on Auto; set the
//!   other one manually (a next-protocol mismatch warning results). OSPFv2 TOS
//!   entries (router-link `num_tos` > 0, summary / external TOS metrics) are not
//!   modelled; neither are the OSPFv3 prefix lists (variable 4-octet-aligned
//!   address prefixes), which stay opaque `prefixes` / `address_prefix` octets.
//! * **BGP UPDATE** has two length fields bracketing variable regions in one
//!   header (withdrawn routes, path attributes, then NLRI to the end). Auto
//!   lengths only measure whole header / payload spans, so
//!   `withdrawn_routes_length` and `total_path_attribute_length` are Manual
//!   (their defaults match the default values). Withdrawn routes and NLRI are
//!   opaque `(length, prefix)` octets because a prefix occupies
//!   `ceil(length / 8)` octets, which a `FromField` length cannot express.
//! * **BGP path attributes** have an 8-bit or 16-bit length depending on the
//!   Extended Length flag. [`wpg_core::TlvDef`] has one length width, so
//!   attributes are a TLV with a 16-bit type (flags << 8 | type code) and an
//!   8-bit length; extended-length attributes (flag `0x10`) must be written as
//!   verbatim `raw` items and do not parse back. `AS_PATH` / `AGGREGATOR` use
//!   4-octet AS numbers (RFC 6793, the four-octet AS capability negotiated).
//!   Extended optional parameters (RFC 9072) are not modelled.
//! * **BGP** is bound on the TCP **destination** port only (like the UDP tunnel
//!   ports), so segments from port 179 to an ephemeral port parse as raw TCP
//!   payload; only one BGP message per segment is decoded.
//! * **EIGRP** route TLVs end with a variable-length destination
//!   (`ceil(prefix_length / 8)` octets), modelled as `destination` octets running
//!   to the end of the TLV value; multi-destination TLVs therefore stay in that
//!   one field.

use std::collections::BTreeMap;

use wpg_core::{
    protocol, FieldDef, FieldType, FieldValue, ItemDef, Length, LengthUnit, ProtocolDef, TlvDef,
    TlvItem, TlvVariant,
};

fn u(name: &str, bits: u8) -> FieldDef {
    FieldDef::new(name, FieldType::UInt { bits })
}

fn ip4(name: &str) -> FieldDef {
    FieldDef::new(name, FieldType::Ipv4)
}

fn ip6(name: &str) -> FieldDef {
    FieldDef::new(name, FieldType::Ipv6)
}

fn rest(name: &str) -> FieldDef {
    FieldDef::new(name, FieldType::Bytes { len: Length::ToEnd })
}

/// A list of `item`s filling the rest of the region.
fn list_to_end(item: ItemDef) -> FieldType {
    FieldType::List {
        item,
        length: Length::ToEnd,
    }
}

/// A list of IPv4 addresses (router ids) filling the rest of the region.
fn ipv4_list() -> FieldType {
    list_to_end(ItemDef::Field {
        ty: Box::new(FieldType::Ipv4),
    })
}

/// A struct value from `(field, value)` pairs.
fn st(pairs: &[(&str, FieldValue)]) -> FieldValue {
    FieldValue::Struct(
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect::<BTreeMap<_, _>>(),
    )
}

fn ipv4_value(o: [u8; 4]) -> FieldValue {
    FieldValue::Ipv4(o.into())
}

// --------------------------------------------------------------------------
// OSPFv2 (RFC 2328), IPv4 protocol 89
// --------------------------------------------------------------------------

/// The 20-octet OSPFv2 LSA header as a struct (DBD / `LSAck` entries).
fn ospfv2_lsa_header_fields() -> Vec<FieldDef> {
    vec![
        u("ls_age", 16),
        u("options", 8),
        u("ls_type", 8),
        ip4("link_state_id"),
        ip4("advertising_router"),
        u("ls_sequence", 32),
        u("ls_checksum", 16),
        u("length", 16),
    ]
}

/// A list of OSPFv2 LSA headers filling the rest of the packet.
#[must_use]
pub fn ospfv2_lsa_headers() -> FieldType {
    list_to_end(ItemDef::Struct {
        fields: ospfv2_lsa_header_fields(),
    })
}

/// OSPFv2 common header (RFC 2328 A.3.1, IPv4 protocol 89): version, `type`
/// (Auto, from the body layer), `packet_length` (Auto, header + body), router /
/// area ids, Internet `checksum` (Auto; see the module docs for the
/// authentication-field caveat), authentication type and the 8-octet
/// authentication field.
#[must_use]
pub fn ospfv2() -> ProtocolDef {
    protocol! {
        id: "ospfv2",
        name: "OSPFv2",
        layer: L3,
        fields: {
            version: uint(8) = 2, desc = "ospf-version";
            type: enum(8) {
                1 => hello,
                2 => db_description,
                3 => ls_request,
                4 => ls_update,
                5 => ls_ack,
            } = 1, auto = next_protocol, desc = "ospf-type";
            packet_length: uint(16), auto = length(total), desc = "ospf-packet-length";
            router_id: ipv4 = [192, 0, 2, 1], desc = "ospf-router-id";
            area_id: ipv4 = [0, 0, 0, 0], desc = "ospf-area-id";
            checksum: uint(16), auto = checksum(internet, header_payload), desc = "ospf-checksum";
            au_type: enum(16) {
                0 => null,
                1 => simple_password,
                2 => cryptographic,
            } = 0, desc = "ospf-au-type";
            authentication: bytes(8) = [0u8; 8], desc = "ospf-authentication";
        },
        next: type {
            1 => "ospfv2_hello",
            2 => "ospfv2_dbd",
            3 => "ospfv2_lsr",
            4 => "ospfv2_lsu",
            5 => "ospfv2_lsack",
        },
        constraints: {
            warning: equals(version, 2);
        },
    }
}

/// OSPFv2 Hello (type 1): network mask, timers, options, priority, DR / BDR and
/// the list of neighbor router ids.
#[must_use]
pub fn ospfv2_hello() -> ProtocolDef {
    protocol! {
        id: "ospfv2_hello",
        name: "OSPFv2 Hello",
        layer: L3,
        fields: {
            network_mask: ipv4 = [255, 255, 255, 0], desc = "ospf-network-mask";
            hello_interval: uint(16) = 10, desc = "ospf-hello-interval";
            options: flags(8) {
                0 => mt,
                1 => e,
                2 => mc,
                3 => np,
                4 => l,
                5 => dc,
                6 => o,
                7 => dn,
            } = 0x02, desc = "ospf-options";
            router_priority: uint(8) = 1, desc = "ospf-router-priority";
            router_dead_interval: uint(32) = 40, desc = "ospf-router-dead-interval";
            designated_router: ipv4 = [0, 0, 0, 0], desc = "ospf-designated-router";
            backup_designated_router: ipv4 = [0, 0, 0, 0], desc = "ospf-backup-designated-router";
            neighbors: ty(ipv4_list()), desc = "ospf-neighbors";
        },
    }
}

/// OSPFv2 Database Description (type 2): interface MTU, options, I / M / MS flags,
/// DD sequence number and the LSA headers.
#[must_use]
pub fn ospfv2_dbd() -> ProtocolDef {
    protocol! {
        id: "ospfv2_dbd",
        name: "OSPFv2 DBD",
        layer: L3,
        fields: {
            interface_mtu: uint(16) = 1500, desc = "ospf-interface-mtu";
            options: flags(8) {
                0 => mt,
                1 => e,
                2 => mc,
                3 => np,
                4 => l,
                5 => dc,
                6 => o,
                7 => dn,
            } = 0x02, desc = "ospf-options";
            flags: flags(8) { 2 => init, 1 => more, 0 => master } = 0x07, desc = "ospf-dbd-flags";
            dd_sequence: uint(32) = 1, desc = "ospf-dd-sequence";
            lsa_headers: ty(ospfv2_lsa_headers()), desc = "ospf-lsa-headers";
        },
    }
}

/// OSPFv2 Link State Request (type 3): `{ls_type, link_state_id,
/// advertising_router}` entries.
#[must_use]
pub fn ospfv2_lsr() -> ProtocolDef {
    let item = ItemDef::Struct {
        fields: vec![
            u("ls_type", 32),
            ip4("link_state_id"),
            ip4("advertising_router"),
        ],
    };
    protocol! {
        id: "ospfv2_lsr",
        name: "OSPFv2 LSR",
        layer: L3,
        fields: {
            requests: ty(list_to_end(item)), desc = "ospf-lsr-requests";
        },
    }
}

/// OSPFv2 Link State Update (type 4): the LSA count; the (single, typed) LSA
/// follows as an [`ospfv2_lsa`] layer (see the module docs).
#[must_use]
pub fn ospfv2_lsu() -> ProtocolDef {
    let mut d = protocol! {
        id: "ospfv2_lsu",
        name: "OSPFv2 LSU",
        layer: L3,
        fields: {
            num_lsas: uint(32) = 1, desc = "ospf-num-lsas";
        },
        next: num_lsas {},
    };
    if let Some(b) = d.next.as_mut() {
        b.default = Some("ospfv2_lsa".into());
    }
    d
}

/// OSPFv2 Link State Acknowledgment (type 5): the acknowledged LSA headers.
#[must_use]
pub fn ospfv2_lsack() -> ProtocolDef {
    protocol! {
        id: "ospfv2_lsack",
        name: "OSPFv2 LSAck",
        layer: L3,
        fields: {
            lsa_headers: ty(ospfv2_lsa_headers()), desc = "ospf-lsa-headers";
        },
    }
}

/// OSPFv2 LSA header (RFC 2328 A.4.1) as a layer inside an LSU: `ls_type` (Auto,
/// from the body layer), Fletcher `ls_checksum` (Auto; exact for `ls_age` 0, see
/// the module docs) and `length` (Auto, header + body).
#[must_use]
pub fn ospfv2_lsa() -> ProtocolDef {
    protocol! {
        id: "ospfv2_lsa",
        name: "OSPFv2 LSA",
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
            } = 0x02, desc = "ospf-options";
            ls_type: enum(8) {
                1 => router,
                2 => network,
                3 => summary_network,
                4 => summary_asbr,
                5 => as_external,
                7 => nssa_external,
            } = 1, auto = next_protocol, desc = "ospf-ls-type";
            link_state_id: ipv4 = [192, 0, 2, 1], desc = "ospf-link-state-id";
            advertising_router: ipv4 = [192, 0, 2, 1], desc = "ospf-advertising-router";
            ls_sequence: uint(32) = 0x8000_0001, desc = "ospf-ls-sequence";
            ls_checksum: uint(16), auto = checksum(fletcher, header_payload), desc = "ospf-ls-checksum";
            length: uint(16), auto = length(total), desc = "ospf-lsa-length";
        },
        next: ls_type {
            1 => "ospfv2_router_lsa",
            2 => "ospfv2_network_lsa",
            3 => "ospfv2_summary_lsa",
            4 => "ospfv2_summary_lsa",
            5 => "ospfv2_external_lsa",
            7 => "ospfv2_external_lsa",
        },
    }
}

/// OSPFv2 Router-LSA body (LS type 1): V / E / B flags, link count (Manual) and
/// the links (`num_tos` must be 0, see the module docs). Default: one stub link
/// to 192.0.2.0/24, metric 10.
#[must_use]
pub fn ospfv2_router_lsa() -> ProtocolDef {
    let item = ItemDef::Struct {
        fields: vec![
            ip4("link_id"),
            ip4("link_data"),
            FieldDef::new(
                "type",
                FieldType::Enum {
                    bits: 8,
                    variants: [
                        (1, "point_to_point"),
                        (2, "transit"),
                        (3, "stub"),
                        (4, "virtual_link"),
                    ]
                    .into_iter()
                    .map(|(value, name)| wpg_core::EnumVariant {
                        value,
                        name: name.into(),
                        description: None,
                    })
                    .collect(),
                },
            ),
            u("num_tos", 8),
            u("metric", 16),
        ],
    };
    let link = st(&[
        ("link_id", ipv4_value([192, 0, 2, 0])),
        ("link_data", ipv4_value([255, 255, 255, 0])),
        ("type", FieldValue::UInt(3)),
        ("num_tos", FieldValue::UInt(0)),
        ("metric", FieldValue::UInt(10)),
    ]);
    protocol! {
        id: "ospfv2_router_lsa",
        name: "OSPFv2 Router-LSA",
        layer: L3,
        fields: {
            flags: flags(8) { 2 => v, 1 => e, 0 => b } = 0, desc = "ospf-router-lsa-flags";
            reserved: uint(8) = 0;
            num_links: uint(16) = 1, desc = "ospf-num-links";
            links: ty(list_to_end(item)) = vec![link], desc = "ospf-router-links";
        },
    }
}

/// OSPFv2 Network-LSA body (LS type 2): network mask and attached routers.
#[must_use]
pub fn ospfv2_network_lsa() -> ProtocolDef {
    protocol! {
        id: "ospfv2_network_lsa",
        name: "OSPFv2 Network-LSA",
        layer: L3,
        fields: {
            network_mask: ipv4 = [255, 255, 255, 0], desc = "ospf-network-mask";
            attached_routers: ty(ipv4_list()), desc = "ospf-attached-routers";
        },
    }
}

/// OSPFv2 Summary-LSA body (LS types 3 and 4): network mask and 24-bit metric
/// (TOS metrics are not modelled).
#[must_use]
pub fn ospfv2_summary_lsa() -> ProtocolDef {
    protocol! {
        id: "ospfv2_summary_lsa",
        name: "OSPFv2 Summary-LSA",
        layer: L3,
        fields: {
            network_mask: ipv4 = [255, 255, 255, 0], desc = "ospf-network-mask";
            reserved: uint(8) = 0;
            metric: uint(24) = 10, desc = "ospf-metric";
        },
    }
}

/// OSPFv2 AS-External-LSA body (LS types 5 and 7): network mask, E bit (type 2
/// metric), metric, forwarding address and external route tag.
#[must_use]
pub fn ospfv2_external_lsa() -> ProtocolDef {
    protocol! {
        id: "ospfv2_external_lsa",
        name: "OSPFv2 AS-External-LSA",
        layer: L3,
        fields: {
            network_mask: ipv4 = [255, 255, 255, 0], desc = "ospf-network-mask";
            e_bit: uint(1) = 1, desc = "ospf-external-e";
            reserved: uint(7) = 0;
            metric: uint(24) = 20, desc = "ospf-metric";
            forwarding_address: ipv4 = [0, 0, 0, 0], desc = "ospf-forwarding-address";
            external_route_tag: uint(32) = 0, desc = "ospf-external-route-tag";
        },
    }
}

// --------------------------------------------------------------------------
// OSPFv3 (RFC 5340), IPv6 next header 89
// --------------------------------------------------------------------------

/// OSPFv3 24-bit options field (RFC 5340 A.2).
fn ospfv3_options() -> FieldType {
    let names = [
        (0, "v6"),
        (1, "e"),
        (3, "n"),
        (4, "r"),
        (5, "dc"),
        (8, "af"),
    ];
    FieldType::Flags {
        bits: 24,
        flags: names
            .into_iter()
            .map(|(bit, name)| wpg_core::FlagBit {
                bit,
                name: name.into(),
            })
            .collect(),
    }
}

/// The 20-octet OSPFv3 LSA header as a struct (DBD / `LSAck` entries).
fn ospfv3_lsa_header_fields() -> Vec<FieldDef> {
    vec![
        u("ls_age", 16),
        u("ls_type", 16),
        u("link_state_id", 32),
        ip4("advertising_router"),
        u("ls_sequence", 32),
        u("ls_checksum", 16),
        u("length", 16),
    ]
}

/// A list of OSPFv3 LSA headers filling the rest of the packet.
#[must_use]
pub fn ospfv3_lsa_headers() -> FieldType {
    list_to_end(ItemDef::Struct {
        fields: ospfv3_lsa_header_fields(),
    })
}

/// OSPFv3 common header (RFC 5340 A.3.1, IPv6 next header 89): version, `type`
/// (Auto), `packet_length` (Auto), router / area ids, `checksum` (Auto, Internet
/// checksum with the IPv6 pseudo header) and instance id.
#[must_use]
pub fn ospfv3() -> ProtocolDef {
    protocol! {
        id: "ospfv3",
        name: "OSPFv3",
        layer: L3,
        fields: {
            version: uint(8) = 3, desc = "ospf-version";
            type: enum(8) {
                1 => hello,
                2 => db_description,
                3 => ls_request,
                4 => ls_update,
                5 => ls_ack,
            } = 1, auto = next_protocol, desc = "ospf-type";
            packet_length: uint(16), auto = length(total), desc = "ospf-packet-length";
            router_id: ipv4 = [192, 0, 2, 1], desc = "ospf-router-id";
            area_id: ipv4 = [0, 0, 0, 0], desc = "ospf-area-id";
            checksum: uint(16), auto = checksum(internet_pseudo, header_payload), desc = "ospfv3-checksum";
            instance_id: uint(8) = 0, desc = "ospfv3-instance-id";
            reserved: uint(8) = 0;
        },
        next: type {
            1 => "ospfv3_hello",
            2 => "ospfv3_dbd",
            3 => "ospfv3_lsr",
            4 => "ospfv3_lsu",
            5 => "ospfv3_lsack",
        },
        constraints: {
            warning: equals(version, 3);
        },
    }
}

/// OSPFv3 Hello (type 1): interface id, priority, options, timers, DR / BDR and
/// neighbor router ids.
#[must_use]
pub fn ospfv3_hello() -> ProtocolDef {
    protocol! {
        id: "ospfv3_hello",
        name: "OSPFv3 Hello",
        layer: L3,
        fields: {
            interface_id: uint(32) = 1, desc = "ospfv3-interface-id";
            router_priority: uint(8) = 1, desc = "ospf-router-priority";
            options: ty(ospfv3_options()) = 0x13u32, desc = "ospfv3-options";
            hello_interval: uint(16) = 10, desc = "ospf-hello-interval";
            router_dead_interval: uint(16) = 40, desc = "ospf-router-dead-interval";
            designated_router: ipv4 = [0, 0, 0, 0], desc = "ospf-designated-router";
            backup_designated_router: ipv4 = [0, 0, 0, 0], desc = "ospf-backup-designated-router";
            neighbors: ty(ipv4_list()), desc = "ospf-neighbors";
        },
    }
}

/// OSPFv3 Database Description (type 2): options, interface MTU, I / M / MS flags,
/// DD sequence number and the LSA headers.
#[must_use]
pub fn ospfv3_dbd() -> ProtocolDef {
    protocol! {
        id: "ospfv3_dbd",
        name: "OSPFv3 DBD",
        layer: L3,
        fields: {
            reserved1: uint(8) = 0;
            options: ty(ospfv3_options()) = 0x13u32, desc = "ospfv3-options";
            interface_mtu: uint(16) = 1500, desc = "ospf-interface-mtu";
            reserved2: uint(8) = 0;
            flags: flags(8) { 2 => init, 1 => more, 0 => master } = 0x07, desc = "ospf-dbd-flags";
            dd_sequence: uint(32) = 1, desc = "ospf-dd-sequence";
            lsa_headers: ty(ospfv3_lsa_headers()), desc = "ospf-lsa-headers";
        },
    }
}

/// OSPFv3 Link State Request (type 3): `{reserved, ls_type, link_state_id,
/// advertising_router}` entries.
#[must_use]
pub fn ospfv3_lsr() -> ProtocolDef {
    let item = ItemDef::Struct {
        fields: vec![
            u("reserved", 16),
            u("ls_type", 16),
            u("link_state_id", 32),
            ip4("advertising_router"),
        ],
    };
    protocol! {
        id: "ospfv3_lsr",
        name: "OSPFv3 LSR",
        layer: L3,
        fields: {
            requests: ty(list_to_end(item)), desc = "ospf-lsr-requests";
        },
    }
}

/// OSPFv3 Link State Update (type 4): the LSA count; the (single, typed) LSA
/// follows as an [`ospfv3_lsa`] layer.
#[must_use]
pub fn ospfv3_lsu() -> ProtocolDef {
    let mut d = protocol! {
        id: "ospfv3_lsu",
        name: "OSPFv3 LSU",
        layer: L3,
        fields: {
            num_lsas: uint(32) = 1, desc = "ospf-num-lsas";
        },
        next: num_lsas {},
    };
    if let Some(b) = d.next.as_mut() {
        b.default = Some("ospfv3_lsa".into());
    }
    d
}

/// OSPFv3 Link State Acknowledgment (type 5): the acknowledged LSA headers.
#[must_use]
pub fn ospfv3_lsack() -> ProtocolDef {
    protocol! {
        id: "ospfv3_lsack",
        name: "OSPFv3 LSAck",
        layer: L3,
        fields: {
            lsa_headers: ty(ospfv3_lsa_headers()), desc = "ospf-lsa-headers";
        },
    }
}

/// OSPFv3 LSA header (RFC 5340 A.4.2) as a layer inside an LSU: 16-bit `ls_type`
/// (U / S bits + function code; Auto from the body layer), Fletcher
/// `ls_checksum` (Auto; exact for `ls_age` 0) and `length` (Auto).
#[must_use]
pub fn ospfv3_lsa() -> ProtocolDef {
    protocol! {
        id: "ospfv3_lsa",
        name: "OSPFv3 LSA",
        layer: L3,
        fields: {
            ls_age: uint(16) = 0, desc = "ospf-ls-age";
            ls_type: enum(16) {
                0x2001 => router,
                0x2002 => network,
                0x2003 => inter_area_prefix,
                0x2004 => inter_area_router,
                0x4005 => as_external,
                0x2007 => nssa,
                0x0008 => link,
                0x2009 => intra_area_prefix,
            } = 0x2001, auto = next_protocol, desc = "ospfv3-ls-type";
            link_state_id: uint(32) = 0, desc = "ospf-link-state-id";
            advertising_router: ipv4 = [192, 0, 2, 1], desc = "ospf-advertising-router";
            ls_sequence: uint(32) = 0x8000_0001, desc = "ospf-ls-sequence";
            ls_checksum: uint(16), auto = checksum(fletcher, header_payload), desc = "ospf-ls-checksum";
            length: uint(16), auto = length(total), desc = "ospf-lsa-length";
        },
        next: ls_type {
            0x2001 => "ospfv3_router_lsa",
            0x2002 => "ospfv3_network_lsa",
            0x2003 => "ospfv3_inter_area_prefix_lsa",
            0x4005 => "ospfv3_as_external_lsa",
            0x2007 => "ospfv3_as_external_lsa",
            0x0008 => "ospfv3_link_lsa",
            0x2009 => "ospfv3_intra_area_prefix_lsa",
        },
    }
}

/// OSPFv3 Router-LSA body (`0x2001`): flags, options and the interface entries.
/// Default: one point-to-point link, metric 10.
#[must_use]
pub fn ospfv3_router_lsa() -> ProtocolDef {
    let item = ItemDef::Struct {
        fields: vec![
            u("type", 8),
            u("reserved", 8),
            u("metric", 16),
            u("interface_id", 32),
            u("neighbor_interface_id", 32),
            ip4("neighbor_router_id"),
        ],
    };
    let link = st(&[
        ("type", FieldValue::UInt(1)),
        ("reserved", FieldValue::UInt(0)),
        ("metric", FieldValue::UInt(10)),
        ("interface_id", FieldValue::UInt(1)),
        ("neighbor_interface_id", FieldValue::UInt(1)),
        ("neighbor_router_id", ipv4_value([192, 0, 2, 2])),
    ]);
    protocol! {
        id: "ospfv3_router_lsa",
        name: "OSPFv3 Router-LSA",
        layer: L3,
        fields: {
            flags: flags(8) { 2 => v, 1 => e, 0 => b } = 0, desc = "ospf-router-lsa-flags";
            options: ty(ospfv3_options()) = 0x13u32, desc = "ospfv3-options";
            links: ty(list_to_end(item)) = vec![link], desc = "ospfv3-router-links";
        },
    }
}

/// OSPFv3 Network-LSA body (`0x2002`): options and attached routers.
#[must_use]
pub fn ospfv3_network_lsa() -> ProtocolDef {
    protocol! {
        id: "ospfv3_network_lsa",
        name: "OSPFv3 Network-LSA",
        layer: L3,
        fields: {
            reserved: uint(8) = 0;
            options: ty(ospfv3_options()) = 0x13u32, desc = "ospfv3-options";
            attached_routers: ty(ipv4_list()), desc = "ospf-attached-routers";
        },
    }
}

/// OSPFv3 Inter-Area-Prefix-LSA body (`0x2003`): metric and one prefix
/// (`address_prefix` holds `ceil(prefix_length / 32)` 4-octet words).
#[must_use]
pub fn ospfv3_inter_area_prefix_lsa() -> ProtocolDef {
    protocol! {
        id: "ospfv3_inter_area_prefix_lsa",
        name: "OSPFv3 Inter-Area-Prefix-LSA",
        layer: L3,
        fields: {
            reserved: uint(8) = 0;
            metric: uint(24) = 10, desc = "ospf-metric";
            prefix_length: uint(8) = 64, desc = "ospfv3-prefix-length";
            prefix_options: flags(8) { 0 => nu, 1 => la, 3 => p, 4 => dn } = 0, desc = "ospfv3-prefix-options";
            reserved2: uint(16) = 0;
            address_prefix: bytes(to_end) = [0x20, 0x01, 0x0d, 0xb8, 0, 1, 0, 0], desc = "ospfv3-address-prefix";
        },
    }
}

/// OSPFv3 AS-External-LSA body (`0x4005`, and NSSA `0x2007`): E / F / T flags,
/// metric, one prefix and the referenced LS type. The optional forwarding
/// address / route tag / referenced link state id (per F / T / referenced type)
/// follow the prefix inside `address_prefix`.
#[must_use]
pub fn ospfv3_as_external_lsa() -> ProtocolDef {
    protocol! {
        id: "ospfv3_as_external_lsa",
        name: "OSPFv3 AS-External-LSA",
        layer: L3,
        fields: {
            flags: flags(8) { 2 => e, 1 => f, 0 => t } = 0x04, desc = "ospfv3-external-flags";
            metric: uint(24) = 20, desc = "ospf-metric";
            prefix_length: uint(8) = 64, desc = "ospfv3-prefix-length";
            prefix_options: flags(8) { 0 => nu, 1 => la, 3 => p, 4 => dn } = 0, desc = "ospfv3-prefix-options";
            referenced_ls_type: uint(16) = 0, desc = "ospfv3-referenced-ls-type";
            address_prefix: bytes(to_end) = [0x20, 0x01, 0x0d, 0xb8, 0, 2, 0, 0], desc = "ospfv3-address-prefix";
        },
    }
}

/// OSPFv3 Link-LSA body (`0x0008`): priority, options, link-local address,
/// prefix count (Manual) and the opaque prefix list.
#[must_use]
pub fn ospfv3_link_lsa() -> ProtocolDef {
    protocol! {
        id: "ospfv3_link_lsa",
        name: "OSPFv3 Link-LSA",
        layer: L3,
        fields: {
            router_priority: uint(8) = 1, desc = "ospf-router-priority";
            options: ty(ospfv3_options()) = 0x13u32, desc = "ospfv3-options";
            link_local_address: ipv6 = [0xfe80u16, 0, 0, 0, 0, 0, 0, 1], desc = "ospfv3-link-local-address";
            num_prefixes: uint(32) = 0, desc = "ospfv3-num-prefixes";
            prefixes: bytes(to_end) = [], desc = "ospfv3-prefixes";
        },
    }
}

/// OSPFv3 Intra-Area-Prefix-LSA body (`0x2009`): prefix count (Manual), the
/// referenced LSA and the opaque prefix list (`{prefix_length, prefix_options,
/// metric, address_prefix}` entries).
#[must_use]
pub fn ospfv3_intra_area_prefix_lsa() -> ProtocolDef {
    protocol! {
        id: "ospfv3_intra_area_prefix_lsa",
        name: "OSPFv3 Intra-Area-Prefix-LSA",
        layer: L3,
        fields: {
            num_prefixes: uint(16) = 1, desc = "ospfv3-num-prefixes";
            referenced_ls_type: uint(16) = 0x2001, desc = "ospfv3-referenced-ls-type";
            referenced_link_state_id: uint(32) = 0, desc = "ospfv3-referenced-link-state-id";
            referenced_advertising_router: ipv4 = [192, 0, 2, 1], desc = "ospfv3-referenced-advertising-router";
            // one prefix: 2001:db8:3::/64, metric 10
            prefixes: bytes(to_end) = [64, 0, 0, 10, 0x20, 0x01, 0x0d, 0xb8, 0, 3, 0, 0], desc = "ospfv3-prefixes";
        },
    }
}

// --------------------------------------------------------------------------
// BGP-4 (RFC 4271), TCP port 179
// --------------------------------------------------------------------------

/// BGP message header (RFC 4271 §4.1, TCP port 179): 16-octet all-ones marker,
/// `length` (Auto, header + body) and `type` (Auto from the body layer; the
/// default 4 = KEEPALIVE has no body).
#[must_use]
pub fn bgp() -> ProtocolDef {
    protocol! {
        id: "bgp",
        name: "BGP",
        layer: L7,
        fields: {
            marker: bytes(16) = [0xffu8; 16], desc = "bgp-marker";
            length: uint(16), auto = length(total), desc = "bgp-length";
            type: enum(8) {
                1 => open,
                2 => update,
                3 => notification,
                4 => keepalive,
                5 => route_refresh,
            } = 4, auto = next_protocol, desc = "bgp-type";
        },
        next: type {
            1 => "bgp_open",
            2 => "bgp_update",
            3 => "bgp_notification",
            5 => "bgp_route_refresh",
        },
        constraints: {
            error: min_length(total, 19);
            warning: max_length(total, 4096);
        },
    }
}

/// BGP capabilities (RFC 5492) inside the Capabilities optional parameter:
/// 8-bit code, 8-bit length (value only).
#[must_use]
pub fn bgp_capabilities() -> FieldType {
    FieldType::Tlv(TlvDef {
        type_bits: 8,
        len_bits: 8,
        len_includes_header: false,
        len_unit: LengthUnit::Bytes,
        variants: vec![
            TlvVariant::new(
                1,
                "multiprotocol",
                vec![u("afi", 16), u("reserved", 8), u("safi", 8)],
            ),
            TlvVariant::new(2, "route_refresh", Vec::new()),
            TlvVariant::new(
                64,
                "graceful_restart",
                vec![
                    u("restart_flags", 4),
                    u("restart_time", 12),
                    rest("address_families"),
                ],
            ),
            TlvVariant::new(65, "four_octet_as", vec![u("asn", 32)]),
            TlvVariant::new(69, "add_path", vec![rest("address_families")]),
            TlvVariant::new(70, "enhanced_route_refresh", Vec::new()),
            TlvVariant::new(128, "route_refresh_cisco", Vec::new()),
        ],
        unknown_as_raw: true,
        length: Length::ToEnd,
    })
}

/// BGP OPEN optional parameters: 8-bit type, 8-bit length (value only); the
/// region is `opt_params_length` octets.
#[must_use]
pub fn bgp_open_params() -> FieldType {
    FieldType::Tlv(TlvDef {
        type_bits: 8,
        len_bits: 8,
        len_includes_header: false,
        len_unit: LengthUnit::Bytes,
        variants: vec![TlvVariant::new(
            2,
            "capabilities",
            vec![FieldDef::new("capabilities", bgp_capabilities())],
        )],
        unknown_as_raw: true,
        length: Length::FromField {
            field: "opt_params_length".into(),
            unit: 1,
            adjust: 0,
        },
    })
}

/// BGP OPEN (type 1): version, my AS, hold time, BGP identifier,
/// `opt_params_length` (Auto) and the optional parameters. Default: one
/// Capabilities parameter with Multiprotocol IPv4 unicast, Route Refresh and
/// 4-octet AS 65001.
#[must_use]
pub fn bgp_open() -> ProtocolDef {
    let caps = FieldValue::Tlv(vec![
        TlvItem::named("multiprotocol")
            .with("afi", 1u16)
            .with("reserved", 0u8)
            .with("safi", 1u8),
        TlvItem::named("route_refresh"),
        TlvItem::named("four_octet_as").with("asn", 65001u32),
    ]);
    protocol! {
        id: "bgp_open",
        name: "BGP OPEN",
        layer: L7,
        fields: {
            version: uint(8) = 4, desc = "bgp-version";
            my_as: uint(16) = 65001, desc = "bgp-my-as";
            hold_time: uint(16) = 180, desc = "bgp-hold-time";
            bgp_identifier: ipv4 = [192, 0, 2, 1], desc = "bgp-identifier";
            opt_params_length: uint(8), auto = length(header, adjust = -10), desc = "bgp-opt-params-length";
            opt_params: ty(bgp_open_params()) = vec![
                TlvItem::named("capabilities").with("capabilities", caps),
            ], desc = "bgp-opt-params";
        },
        constraints: {
            warning: equals(version, 4);
        },
    }
}

/// BGP path attributes: a 16-bit type (attribute flags << 8 | type code) and an
/// 8-bit length (see the module docs for extended-length attributes).
#[must_use]
pub fn bgp_path_attributes() -> FieldType {
    let as_segment = ItemDef::Struct {
        fields: vec![
            FieldDef::new(
                "segment_type",
                FieldType::Enum {
                    bits: 8,
                    variants: [(1, "as_set"), (2, "as_sequence")]
                        .into_iter()
                        .map(|(value, name)| wpg_core::EnumVariant {
                            value,
                            name: name.into(),
                            description: None,
                        })
                        .collect(),
                },
            ),
            u("segment_length", 8),
            FieldDef::new(
                "asns",
                FieldType::List {
                    item: ItemDef::Field {
                        ty: Box::new(FieldType::UInt { bits: 32 }),
                    },
                    length: Length::FromField {
                        field: "segment_length".into(),
                        unit: 4,
                        adjust: 0,
                    },
                },
            ),
        ],
    };
    let u32_list = || {
        FieldDef::new(
            "values",
            list_to_end(ItemDef::Field {
                ty: Box::new(FieldType::UInt { bits: 32 }),
            }),
        )
    };
    FieldType::Tlv(TlvDef {
        type_bits: 16,
        len_bits: 8,
        len_includes_header: false,
        len_unit: LengthUnit::Bytes,
        variants: vec![
            TlvVariant::new(0x4001, "origin", vec![u("origin", 8)]),
            TlvVariant::new(
                0x4002,
                "as_path",
                vec![FieldDef::new("segments", list_to_end(as_segment))],
            ),
            TlvVariant::new(0x4003, "next_hop", vec![ip4("next_hop")]),
            TlvVariant::new(0x8004, "multi_exit_disc", vec![u("med", 32)]),
            TlvVariant::new(0x4005, "local_pref", vec![u("local_pref", 32)]),
            TlvVariant::new(0x4006, "atomic_aggregate", Vec::new()),
            TlvVariant::new(0xc007, "aggregator", vec![u("asn", 32), ip4("address")]),
            TlvVariant::new(0xc008, "communities", vec![u32_list()]),
            TlvVariant::new(
                0x800e,
                "mp_reach_nlri",
                vec![
                    u("afi", 16),
                    u("safi", 8),
                    u("next_hop_length", 8),
                    FieldDef::new(
                        "next_hop",
                        FieldType::Bytes {
                            len: Length::FromField {
                                field: "next_hop_length".into(),
                                unit: 1,
                                adjust: 0,
                            },
                        },
                    ),
                    u("reserved", 8),
                    rest("nlri"),
                ],
            ),
            TlvVariant::new(
                0x800f,
                "mp_unreach_nlri",
                vec![u("afi", 16), u("safi", 8), rest("withdrawn")],
            ),
            TlvVariant::new(
                0xc010,
                "extended_communities",
                vec![FieldDef::new(
                    "values",
                    list_to_end(ItemDef::Field {
                        ty: Box::new(FieldType::UInt { bits: 64 }),
                    }),
                )],
            ),
            TlvVariant::new(
                0xc020,
                "large_communities",
                vec![FieldDef::new(
                    "values",
                    list_to_end(ItemDef::Struct {
                        fields: vec![
                            u("global_admin", 32),
                            u("local_data1", 32),
                            u("local_data2", 32),
                        ],
                    }),
                )],
            ),
        ],
        unknown_as_raw: true,
        length: Length::FromField {
            field: "total_path_attribute_length".into(),
            unit: 1,
            adjust: 0,
        },
    })
}

/// BGP UPDATE (type 2): withdrawn routes, path attributes and NLRI. Both length
/// fields are Manual (see the module docs). Default: ORIGIN IGP, `AS_PATH`
/// `AS_SEQUENCE` 65001, `NEXT_HOP` 192.0.2.1 announcing 198.51.100.0/24.
#[must_use]
pub fn bgp_update() -> ProtocolDef {
    let segment = st(&[
        ("segment_type", FieldValue::UInt(2)),
        ("segment_length", FieldValue::UInt(1)),
        ("asns", FieldValue::List(vec![FieldValue::UInt(65001)])),
    ]);
    let attrs = vec![
        TlvItem::named("origin").with("origin", 0u8),
        TlvItem::named("as_path").with("segments", vec![segment]),
        TlvItem::named("next_hop").with("next_hop", std::net::Ipv4Addr::new(192, 0, 2, 1)),
    ];
    protocol! {
        id: "bgp_update",
        name: "BGP UPDATE",
        layer: L7,
        fields: {
            withdrawn_routes_length: uint(16) = 0, desc = "bgp-withdrawn-routes-length";
            withdrawn_routes: bytes(from = withdrawn_routes_length) = [], desc = "bgp-withdrawn-routes";
            total_path_attribute_length: uint(16) = 20, desc = "bgp-total-path-attribute-length";
            path_attributes: ty(bgp_path_attributes()) = attrs, desc = "bgp-path-attributes";
            nlri: bytes(to_end) = [24, 198, 51, 100], desc = "bgp-nlri";
        },
    }
}

/// BGP NOTIFICATION (type 3): error code, subcode and diagnostic data.
#[must_use]
pub fn bgp_notification() -> ProtocolDef {
    protocol! {
        id: "bgp_notification",
        name: "BGP NOTIFICATION",
        layer: L7,
        fields: {
            error_code: enum(8) {
                1 => message_header_error,
                2 => open_message_error,
                3 => update_message_error,
                4 => hold_timer_expired,
                5 => fsm_error,
                6 => cease,
                7 => route_refresh_message_error,
            } = 6, desc = "bgp-error-code";
            error_subcode: uint(8) = 2, desc = "bgp-error-subcode";
            data: bytes(to_end) = [], desc = "bgp-error-data";
        },
    }
}

/// BGP ROUTE-REFRESH (type 5, RFC 2918 / RFC 7313): AFI, subtype and SAFI.
#[must_use]
pub fn bgp_route_refresh() -> ProtocolDef {
    protocol! {
        id: "bgp_route_refresh",
        name: "BGP ROUTE-REFRESH",
        layer: L7,
        fields: {
            afi: uint(16) = 1, desc = "bgp-afi";
            subtype: enum(8) { 0 => normal, 1 => borr, 2 => eorr } = 0, desc = "bgp-rr-subtype";
            safi: uint(8) = 1, desc = "bgp-safi";
        },
    }
}

// --------------------------------------------------------------------------
// RIP (RFC 1058 / RFC 2453), UDP 520; RIPng (RFC 2080), UDP 521
// --------------------------------------------------------------------------

/// `RIPv1` / `RIPv2` (UDP 520): command, version and 20-octet route entries
/// (`{afi, route_tag, address, subnet_mask, next_hop, metric}`; `RIPv1` leaves
/// tag / mask / next hop zero). An authentication entry (`afi` `0xffff`) is the
/// same 20 octets, written through the same fields. Default: a `RIPv2` response
/// with 192.0.2.0/24, metric 1.
#[must_use]
pub fn rip() -> ProtocolDef {
    let item = ItemDef::Struct {
        fields: vec![
            u("afi", 16),
            u("route_tag", 16),
            ip4("address"),
            ip4("subnet_mask"),
            ip4("next_hop"),
            u("metric", 32),
        ],
    };
    let entry = st(&[
        ("afi", FieldValue::UInt(2)),
        ("route_tag", FieldValue::UInt(0)),
        ("address", ipv4_value([192, 0, 2, 0])),
        ("subnet_mask", ipv4_value([255, 255, 255, 0])),
        ("next_hop", ipv4_value([0, 0, 0, 0])),
        ("metric", FieldValue::UInt(1)),
    ]);
    protocol! {
        id: "rip",
        name: "RIP",
        layer: L7,
        fields: {
            command: enum(8) { 1 => request, 2 => response } = 2, desc = "rip-command";
            version: uint(8) = 2, desc = "rip-version";
            zero: uint(16) = 0;
            entries: ty(list_to_end(item)) = vec![entry], desc = "rip-entries";
        },
        constraints: {
            warning: in_range(version, 1, 2);
        },
    }
}

/// `RIPng` (RFC 2080, UDP 521): command, version 1 and 20-octet route table
/// entries `{prefix, route_tag, prefix_length, metric}` (a next-hop RTE is
/// metric `0xff`). Default: a response with `2001:db8:1::/64`, metric 1.
#[must_use]
pub fn ripng() -> ProtocolDef {
    let item = ItemDef::Struct {
        fields: vec![
            ip6("prefix"),
            u("route_tag", 16),
            u("prefix_length", 8),
            u("metric", 8),
        ],
    };
    let entry = st(&[
        (
            "prefix",
            FieldValue::Ipv6([0x2001u16, 0xdb8, 1, 0, 0, 0, 0, 0].into()),
        ),
        ("route_tag", FieldValue::UInt(0)),
        ("prefix_length", FieldValue::UInt(64)),
        ("metric", FieldValue::UInt(1)),
    ]);
    protocol! {
        id: "ripng",
        name: "RIPng",
        layer: L7,
        fields: {
            command: enum(8) { 1 => request, 2 => response } = 2, desc = "rip-command";
            version: uint(8) = 1, desc = "rip-version";
            reserved: uint(16) = 0;
            entries: ty(list_to_end(item)) = vec![entry], desc = "ripng-entries";
        },
    }
}

// --------------------------------------------------------------------------
// EIGRP (RFC 7868), IP protocol 88
// --------------------------------------------------------------------------

/// Classic EIGRP route metric fields shared by the route TLVs.
fn eigrp_metric() -> Vec<FieldDef> {
    vec![
        u("delay", 32),
        u("bandwidth", 32),
        u("mtu", 24),
        u("hop_count", 8),
        u("reliability", 8),
        u("load", 8),
        u("internal_tag", 8),
        u("flags", 8),
    ]
}

/// External route data shared by the external route TLVs.
fn eigrp_external() -> Vec<FieldDef> {
    vec![
        ip4("originating_router"),
        u("originating_as", 32),
        u("arbitrary_tag", 32),
        u("external_metric", 32),
        u("reserved", 16),
        u("external_protocol", 8),
        u("external_flags", 8),
    ]
}

/// EIGRP TLVs (RFC 7868 §6): 16-bit type, 16-bit length including the header.
/// Route TLVs end with `prefix_length` and the variable `destination` octets.
#[must_use]
pub fn eigrp_tlvs() -> FieldType {
    let route = |next_hop: FieldDef, external: bool| {
        let mut f = vec![next_hop];
        if external {
            f.extend(eigrp_external());
        }
        f.extend(eigrp_metric());
        f.push(u("prefix_length", 8));
        f.push(rest("destination"));
        f
    };
    FieldType::Tlv(TlvDef {
        type_bits: 16,
        len_bits: 16,
        len_includes_header: true,
        len_unit: LengthUnit::Bytes,
        variants: vec![
            TlvVariant::new(
                0x0001,
                "parameters",
                vec![
                    u("k1", 8),
                    u("k2", 8),
                    u("k3", 8),
                    u("k4", 8),
                    u("k5", 8),
                    u("k6", 8),
                    u("hold_time", 16),
                ],
            ),
            TlvVariant::new(0x0002, "authentication", vec![rest("data")]),
            TlvVariant::new(
                0x0003,
                "sequence",
                vec![u("address_length", 8), rest("address")],
            ),
            TlvVariant::new(
                0x0004,
                "software_version",
                vec![
                    u("os_major", 8),
                    u("os_minor", 8),
                    u("tlv_major", 8),
                    u("tlv_minor", 8),
                ],
            ),
            TlvVariant::new(0x0005, "next_multicast_sequence", vec![u("sequence", 32)]),
            TlvVariant::new(0x0007, "peer_termination", vec![rest("data")]),
            TlvVariant::new(0x0102, "ipv4_internal", route(ip4("next_hop"), false)),
            TlvVariant::new(0x0103, "ipv4_external", route(ip4("next_hop"), true)),
            TlvVariant::new(0x0402, "ipv6_internal", route(ip6("next_hop"), false)),
            TlvVariant::new(0x0403, "ipv6_external", route(ip6("next_hop"), true)),
        ],
        unknown_as_raw: true,
        length: Length::ToEnd,
    })
}

/// EIGRP (RFC 7868, IP protocol 88): version, opcode, Internet `checksum` (Auto,
/// over the whole EIGRP packet), flags, sequence / acknowledgment numbers,
/// virtual router id, AS number and TLVs. Default: a Hello with the Parameters
/// (K1 = K3 = 1, hold time 15 s) and Software Version TLVs.
#[must_use]
pub fn eigrp() -> ProtocolDef {
    let tlvs = vec![
        TlvItem::named("parameters")
            .with("k1", 1u8)
            .with("k2", 0u8)
            .with("k3", 1u8)
            .with("k4", 0u8)
            .with("k5", 0u8)
            .with("k6", 0u8)
            .with("hold_time", 15u16),
        TlvItem::named("software_version")
            .with("os_major", 12u8)
            .with("os_minor", 4u8)
            .with("tlv_major", 1u8)
            .with("tlv_minor", 2u8),
    ];
    protocol! {
        id: "eigrp",
        name: "EIGRP",
        layer: L3,
        fields: {
            version: uint(8) = 2, desc = "eigrp-version";
            opcode: enum(8) {
                1 => update,
                3 => query,
                4 => reply,
                5 => hello,
                10 => sia_query,
                11 => sia_reply,
            } = 5, desc = "eigrp-opcode";
            checksum: uint(16), auto = checksum(internet, header_payload), desc = "eigrp-checksum";
            flags: flags(32) { 0 => init, 1 => conditional_receive, 2 => restart, 3 => end_of_table } = 0, desc = "eigrp-flags";
            sequence: uint(32) = 0, desc = "eigrp-sequence";
            acknowledge: uint(32) = 0, desc = "eigrp-acknowledge";
            virtual_router_id: uint(16) = 0, desc = "eigrp-virtual-router-id";
            autonomous_system: uint(16) = 100, desc = "eigrp-autonomous-system";
            tlvs: ty(eigrp_tlvs()) = tlvs, desc = "eigrp-tlvs";
        },
        constraints: {
            warning: equals(version, 2);
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_defs_are_valid() {
        for d in [
            ospfv2(),
            ospfv2_hello(),
            ospfv2_dbd(),
            ospfv2_lsr(),
            ospfv2_lsu(),
            ospfv2_lsack(),
            ospfv2_lsa(),
            ospfv2_router_lsa(),
            ospfv2_network_lsa(),
            ospfv2_summary_lsa(),
            ospfv2_external_lsa(),
            ospfv3(),
            ospfv3_hello(),
            ospfv3_dbd(),
            ospfv3_lsr(),
            ospfv3_lsu(),
            ospfv3_lsack(),
            ospfv3_lsa(),
            ospfv3_router_lsa(),
            ospfv3_network_lsa(),
            ospfv3_inter_area_prefix_lsa(),
            ospfv3_as_external_lsa(),
            ospfv3_link_lsa(),
            ospfv3_intra_area_prefix_lsa(),
            bgp(),
            bgp_open(),
            bgp_update(),
            bgp_notification(),
            bgp_route_refresh(),
            rip(),
            ripng(),
            eigrp(),
        ] {
            d.validate()
                .unwrap_or_else(|e| panic!("{} invalid: {e}", d.id));
        }
        assert_eq!(ospfv2().fixed_header_bits(), Some(24 * 8));
        assert_eq!(ospfv3().fixed_header_bits(), Some(16 * 8));
        assert_eq!(ospfv2_lsa().fixed_header_bits(), Some(20 * 8));
        assert_eq!(ospfv3_lsa().fixed_header_bits(), Some(20 * 8));
        assert_eq!(bgp().fixed_header_bits(), Some(19 * 8));
        assert_eq!(ospfv2_summary_lsa().fixed_header_bits(), Some(8 * 8));
        assert_eq!(ospfv2_external_lsa().fixed_header_bits(), Some(16 * 8));
        let eigrp_bits: u32 = eigrp()
            .fields
            .iter()
            .filter_map(|f| f.ty.fixed_bits())
            .sum();
        assert_eq!(eigrp_bits, 20 * 8);
    }
}
