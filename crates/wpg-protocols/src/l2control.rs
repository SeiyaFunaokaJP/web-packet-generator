//! Layer-2 control / encapsulation protocols (TODO §2.1 P1): LLDP, CDP, STP,
//! LACP / Marker (Slow Protocols), EAPOL, `PPPoE` Discovery / Session and PPP
//! (LCP / IPCP).
//!
//! ## Engine limitations exercised here (see the module docs of each protocol)
//!
//! * **LLDP** TLVs use a 7-bit type + 9-bit length header. [`wpg_core::TlvDef`]
//!   only supports type / length widths that are whole octets, so the TLVs are
//!   modelled as a [`wpg_core::FieldType::List`] of `{ type: uint(7), length:
//!   uint(9), value: bytes(from = length) }` structs. The length is therefore
//!   **not** auto-computed (auto rules are header-only). A proper fix would let
//!   `TlvDef` carry sub-octet `type_bits` / `len_bits` (and an auto length inside
//!   list / TLV values).
//! * **CDP** uses the Internet checksum with an odd-length quirk (Cisco sign-
//!   extends the trailing octet). The engine only has the RFC 1071 sum, so the
//!   `checksum` field auto-computes the standard value; that matches Cisco for
//!   even-length PDUs. A `ChecksumKind::CdpInternet` in wpg-core would cover the
//!   odd-length case.
//! * **STP** config vs. RST / MST BPDUs have type-dependent layouts. The engine
//!   has no per-type field groups inside one protocol, so [`stp`] models the
//!   config BPDU exactly and keeps the RST / MSTP extension (version-1 length,
//!   v3 length, MSTI records) as an opaque `trailer`.

use std::collections::BTreeMap;

use wpg_core::{
    protocol, FieldDef, FieldType, FieldValue, ItemDef, Length, LengthUnit, ProtocolDef, TlvDef,
    TlvVariant,
};

fn u(name: &str, bits: u8) -> FieldDef {
    FieldDef::new(name, FieldType::UInt { bits })
}

fn bytes_field(name: &str, octets: u32) -> FieldDef {
    FieldDef::new(
        name,
        FieldType::Bytes {
            len: Length::Fixed { octets },
        },
    )
}

// --------------------------------------------------------------------------
// LLDP (IEEE 802.1AB), EtherType 0x88cc
// --------------------------------------------------------------------------

/// The LLDP TLV sequence: a list of `{ type (7 bits), length (9 bits), value }`
/// structs consuming the rest of the PDU. See the module docs for why this is a
/// `List` rather than a [`wpg_core::FieldType::Tlv`].
#[must_use]
pub fn lldp_tlvs() -> FieldType {
    FieldType::List {
        item: ItemDef::Struct {
            fields: vec![
                u("type", 7),
                u("length", 9),
                FieldDef::new(
                    "value",
                    FieldType::Bytes {
                        len: Length::FromField {
                            field: "length".into(),
                            unit: 1,
                            adjust: 0,
                            div: 1,
                        },
                    },
                ),
            ],
        },
        length: Length::ToEnd,
    }
}

/// Builds one LLDP TLV struct value with a correct length.
fn lldp_tlv(ty: u64, value: Vec<u8>) -> FieldValue {
    FieldValue::Struct(BTreeMap::from([
        ("type".to_owned(), FieldValue::UInt(ty)),
        ("length".to_owned(), FieldValue::UInt(value.len() as u64)),
        ("value".to_owned(), FieldValue::Bytes(value)),
    ]))
}

/// LLDP (IEEE 802.1AB), EtherType `0x88cc`. A single `tlvs` list of TLVs whose
/// header is 7-bit type + 9-bit length. Known types: Chassis ID (1), Port ID (2),
/// TTL (3), Port Description (4), System Name (5), System Description (6), System
/// Capabilities (7), Management Address (8) and End (0). Default: chassis / port
/// (MAC), TTL 120 and the mandatory End TLV.
#[must_use]
pub fn lldp() -> ProtocolDef {
    let mut d = protocol! {
        id: "lldp",
        name: "LLDP",
        layer: L2,
        fields: {
            tlvs: ty(lldp_tlvs()), desc = "lldp-tlvs";
        },
    };
    let mac = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
    let mut chassis = vec![0x04]; // subtype 4 = MAC address
    chassis.extend_from_slice(&mac);
    let mut port = vec![0x03]; // subtype 3 = MAC address
    port.extend_from_slice(&mac);
    d.fields[0].default = Some(FieldValue::List(vec![
        lldp_tlv(1, chassis),
        lldp_tlv(2, port),
        lldp_tlv(3, vec![0x00, 0x78]), // TTL 120 s
        lldp_tlv(0, Vec::new()),       // End of LLDPDU
    ]));
    d
}

// --------------------------------------------------------------------------
// CDP (Cisco Discovery Protocol), SNAP OUI 0x00000c, PID 0x2000
// --------------------------------------------------------------------------

/// CDP TLVs: 16-bit type, 16-bit length that **includes** the 4-octet header.
#[must_use]
pub fn cdp_tlvs() -> FieldType {
    FieldType::Tlv(TlvDef {
        type_bits: 16,
        len_bits: 16,
        len_includes_header: true,
        len_unit: LengthUnit::Bytes,
        variants: vec![
            TlvVariant::new(0x0001, "device_id", Vec::new()),
            TlvVariant::new(0x0002, "addresses", Vec::new()),
            TlvVariant::new(0x0003, "port_id", Vec::new()),
            TlvVariant::new(0x0004, "capabilities", Vec::new()),
            TlvVariant::new(0x0005, "software_version", Vec::new()),
            TlvVariant::new(0x0006, "platform", Vec::new()),
            TlvVariant::new(0x0007, "ip_prefix", Vec::new()),
            TlvVariant::new(0x000a, "native_vlan", Vec::new()),
            TlvVariant::new(0x000b, "duplex", Vec::new()),
        ],
        unknown_as_raw: true,
        length: Length::ToEnd,
        fill: None,
        head_octets: 0,
        align: 0,
        unpadded_last: false,
    })
}

/// CDP (Cisco Discovery Protocol) over 802.3 + LLC/SNAP (OUI `0x00000c`,
/// PID `0x2000`): version, TTL, Internet `checksum` (Auto) and TLVs.
///
/// The checksum auto-computes the RFC 1071 value, which matches Cisco for
/// even-length PDUs (see the module docs for the odd-length quirk).
#[must_use]
pub fn cdp() -> ProtocolDef {
    protocol! {
        id: "cdp",
        name: "CDP",
        layer: L2,
        fields: {
            version: uint(8) = 2, desc = "cdp-version";
            ttl: uint(8) = 180, desc = "cdp-ttl";
            checksum: uint(16), auto = checksum(internet, header_payload), desc = "cdp-checksum";
            tlvs: ty(cdp_tlvs()), desc = "cdp-tlvs";
        },
    }
}

// --------------------------------------------------------------------------
// STP / RSTP / MSTP BPDU, LLC DSAP/SSAP 0x42
// --------------------------------------------------------------------------

/// Spanning Tree BPDU over 802.3 + LLC (DSAP/SSAP `0x42`). Models the IEEE 802.1D
/// **Configuration** BPDU (35 octets) exactly; the RSTP / MSTP extension
/// (version-1 length, v3 length and the MSTI configuration records) is kept as an
/// opaque `trailer` because the engine has no per-`bpdu_type` layout.
///
/// Bridge / root identifiers are split into their 2-octet priority (with the
/// embedded system-id extension) and 6-octet MAC. Timers are in 1/256-second
/// units (`max_age` 20 s, `hello_time` 2 s, `forward_delay` 15 s).
#[must_use]
pub fn stp() -> ProtocolDef {
    protocol! {
        id: "stp",
        name: "STP BPDU",
        layer: L2,
        fields: {
            protocol_id: uint(16) = 0, desc = "stp-protocol-id";
            protocol_version: enum(8) {
                0 => stp,
                2 => rstp,
                3 => mstp,
            } = 0, desc = "stp-version";
            bpdu_type: enum(8) {
                0 => config,
                2 => rapid,
                0x80 => topology_change,
            } = 0, desc = "stp-type";
            flags: flags(8) {
                0 => topology_change,
                1 => proposal,
                4 => learning,
                5 => forwarding,
                6 => agreement,
                7 => topology_change_ack,
            } = 0, desc = "stp-flags";
            root_priority: uint(16) = 0x8000, desc = "stp-root-priority";
            root_mac: mac = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00], desc = "stp-root-mac";
            root_path_cost: uint(32) = 0, desc = "stp-root-path-cost";
            bridge_priority: uint(16) = 0x8000, desc = "stp-bridge-priority";
            bridge_mac: mac = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00], desc = "stp-bridge-mac";
            port_id: uint(16) = 0x8001, desc = "stp-port-id";
            message_age: uint(16) = 0, desc = "stp-message-age";
            max_age: uint(16) = 0x1400, desc = "stp-max-age";
            hello_time: uint(16) = 0x0200, desc = "stp-hello-time";
            forward_delay: uint(16) = 0x0f00, desc = "stp-forward-delay";
            trailer: bytes(to_end) = [], desc = "stp-trailer";
        },
        constraints: {
            warning: equals(protocol_id, 0);
        },
    }
}

// --------------------------------------------------------------------------
// Slow Protocols (IEEE 802.3 clause 43), EtherType 0x8809: LACP + Marker
// --------------------------------------------------------------------------

/// LACPDU (IEEE 802.3ad, EtherType `0x8809`, subtype 1): actor, partner and
/// collector TLVs plus the terminator, 110 octets in all. Actor / partner state
/// are 8-bit flag fields.
#[must_use]
pub fn lacp() -> ProtocolDef {
    let state_bits = FieldType::Flags {
        bits: 8,
        flags: vec![
            wpg_core::FlagBit {
                bit: 0,
                name: "activity".into(),
            },
            wpg_core::FlagBit {
                bit: 1,
                name: "timeout".into(),
            },
            wpg_core::FlagBit {
                bit: 2,
                name: "aggregation".into(),
            },
            wpg_core::FlagBit {
                bit: 3,
                name: "synchronization".into(),
            },
            wpg_core::FlagBit {
                bit: 4,
                name: "collecting".into(),
            },
            wpg_core::FlagBit {
                bit: 5,
                name: "distributing".into(),
            },
            wpg_core::FlagBit {
                bit: 6,
                name: "defaulted".into(),
            },
            wpg_core::FlagBit {
                bit: 7,
                name: "expired".into(),
            },
        ],
    };
    let mut d = ProtocolDef::new("lacp", "LACP", wpg_core::ProtocolLayer::L2);
    d.fields = vec![
        u("subtype", 8)
            .with_default(FieldValue::UInt(1))
            .with_description("lacp-subtype"),
        u("version", 8).with_default(FieldValue::UInt(1)),
        u("actor_info_type", 8).with_default(FieldValue::UInt(1)),
        u("actor_info_length", 8).with_default(FieldValue::UInt(20)),
        u("actor_system_priority", 16).with_default(FieldValue::UInt(0x8000)),
        FieldDef::new("actor_system", FieldType::Mac).with_description("lacp-actor-system"),
        u("actor_key", 16).with_default(FieldValue::UInt(1)),
        u("actor_port_priority", 16).with_default(FieldValue::UInt(0x8000)),
        u("actor_port", 16).with_default(FieldValue::UInt(1)),
        FieldDef::new("actor_state", state_bits.clone())
            .with_default(FieldValue::UInt(0x45))
            .with_description("lacp-actor-state"),
        bytes_field("actor_reserved", 3).with_default(FieldValue::Bytes(vec![0; 3])),
        u("partner_info_type", 8).with_default(FieldValue::UInt(2)),
        u("partner_info_length", 8).with_default(FieldValue::UInt(20)),
        u("partner_system_priority", 16).with_default(FieldValue::UInt(0x8000)),
        FieldDef::new("partner_system", FieldType::Mac).with_description("lacp-partner-system"),
        u("partner_key", 16).with_default(FieldValue::UInt(1)),
        u("partner_port_priority", 16).with_default(FieldValue::UInt(0x8000)),
        u("partner_port", 16).with_default(FieldValue::UInt(1)),
        FieldDef::new("partner_state", state_bits)
            .with_default(FieldValue::UInt(0x45))
            .with_description("lacp-partner-state"),
        bytes_field("partner_reserved", 3).with_default(FieldValue::Bytes(vec![0; 3])),
        u("collector_info_type", 8).with_default(FieldValue::UInt(3)),
        u("collector_info_length", 8).with_default(FieldValue::UInt(16)),
        u("collector_max_delay", 16).with_default(FieldValue::UInt(0)),
        bytes_field("collector_reserved", 12).with_default(FieldValue::Bytes(vec![0; 12])),
        u("terminator_type", 8).with_default(FieldValue::UInt(0)),
        u("terminator_length", 8).with_default(FieldValue::UInt(0)),
        bytes_field("terminator_reserved", 50).with_default(FieldValue::Bytes(vec![0; 50])),
    ];
    d
}

/// Marker / Marker-Response PDU (Slow Protocols subtype 2), 110 octets.
#[must_use]
pub fn marker() -> ProtocolDef {
    let mut d = ProtocolDef::new("marker", "Marker Protocol", wpg_core::ProtocolLayer::L2);
    d.fields = vec![
        u("subtype", 8)
            .with_default(FieldValue::UInt(2))
            .with_description("marker-subtype"),
        u("version", 8).with_default(FieldValue::UInt(1)),
        u("info_type", 8).with_default(FieldValue::UInt(1)),
        u("info_length", 8).with_default(FieldValue::UInt(16)),
        u("requester_port", 16).with_default(FieldValue::UInt(1)),
        FieldDef::new("requester_system", FieldType::Mac),
        u("requester_transaction_id", 32).with_default(FieldValue::UInt(0)),
        u("pad", 16).with_default(FieldValue::UInt(0)),
        u("terminator_type", 8).with_default(FieldValue::UInt(0)),
        u("terminator_length", 8).with_default(FieldValue::UInt(0)),
        bytes_field("reserved", 90).with_default(FieldValue::Bytes(vec![0; 90])),
    ];
    d
}

// --------------------------------------------------------------------------
// EAPOL (IEEE 802.1X), EtherType 0x888e
// --------------------------------------------------------------------------

/// EAPOL (IEEE 802.1X, EtherType `0x888e`): version, packet type, `length` (Auto)
/// and an opaque `body` (the EAP packet for type 0). `body` is `length` octets.
#[must_use]
pub fn eapol() -> ProtocolDef {
    protocol! {
        id: "eapol",
        name: "EAPOL",
        layer: L2,
        fields: {
            version: uint(8) = 2, desc = "eapol-version";
            type: enum(8) {
                0 => eap,
                1 => start,
                2 => logoff,
                3 => key,
                4 => encapsulated_asf_alert,
            } = 0, desc = "eapol-type";
            length: uint(16), auto = length(header, adjust = -4), desc = "eapol-length";
            body: bytes(from = length) = [], desc = "eapol-body";
        },
    }
}

// --------------------------------------------------------------------------
// PPPoE (RFC 2516), EtherType 0x8863 (Discovery) / 0x8864 (Session)
// --------------------------------------------------------------------------

/// `PPPoE` Discovery TAGs: 16-bit type, 16-bit length (excluding the header).
#[must_use]
pub fn pppoe_tags() -> FieldType {
    FieldType::Tlv(TlvDef {
        type_bits: 16,
        len_bits: 16,
        len_includes_header: false,
        len_unit: LengthUnit::Bytes,
        variants: vec![
            TlvVariant::new(0x0000, "end_of_list", Vec::new()),
            TlvVariant::new(0x0101, "service_name", Vec::new()),
            TlvVariant::new(0x0102, "ac_name", Vec::new()),
            TlvVariant::new(0x0103, "host_uniq", Vec::new()),
            TlvVariant::new(0x0104, "ac_cookie", Vec::new()),
            TlvVariant::new(0x0105, "vendor_specific", Vec::new()),
            TlvVariant::new(0x0110, "relay_session_id", Vec::new()),
            TlvVariant::new(0x0201, "service_name_error", Vec::new()),
            TlvVariant::new(0x0202, "ac_system_error", Vec::new()),
            TlvVariant::new(0x0203, "generic_error", Vec::new()),
        ],
        unknown_as_raw: true,
        length: Length::FromField {
            field: "length".into(),
            unit: 1,
            adjust: 0,
            div: 1,
        },
        fill: None,
        head_octets: 0,
        align: 0,
        unpadded_last: false,
    })
}

/// `PPPoE` Discovery (RFC 2516, EtherType `0x8863`): version / type nibbles, `code`
/// (PADI / PADO / PADR / PADS / PADT), session id, `length` (Auto over the tags)
/// and the discovery `tags`.
#[must_use]
pub fn pppoe_discovery() -> ProtocolDef {
    protocol! {
        id: "pppoe_discovery",
        name: "PPPoE Discovery",
        layer: L2,
        fields: {
            version: uint(4) = 1, desc = "pppoe-version";
            type: uint(4) = 1, desc = "pppoe-type";
            code: enum(8) {
                0x00 => session_data,
                0x09 => padi,
                0x07 => pado,
                0x19 => padr,
                0x65 => pads,
                0xa7 => padt,
            } = 0x09, desc = "pppoe-code";
            session_id: uint(16) = 0, desc = "pppoe-session-id";
            length: uint(16), auto = length(header, adjust = -6), desc = "pppoe-length";
            tags: ty(pppoe_tags()), desc = "pppoe-tags";
        },
    }
}

/// `PPPoE` Session (RFC 2516, EtherType `0x8864`): the 6-octet header; the payload
/// is a PPP frame ([`ppp`]).
#[must_use]
pub fn pppoe_session() -> ProtocolDef {
    let mut d = protocol! {
        id: "pppoe_session",
        name: "PPPoE Session",
        layer: L2,
        fields: {
            version: uint(4) = 1, desc = "pppoe-version";
            type: uint(4) = 1, desc = "pppoe-type";
            code: uint(8) = 0, desc = "pppoe-code";
            session_id: uint(16) = 1, desc = "pppoe-session-id";
            length: uint(16), auto = length(payload), desc = "pppoe-length";
        },
        next: session_id {},
    };
    if let Some(b) = d.next.as_mut() {
        b.default = Some("ppp".into());
    }
    d
}

// --------------------------------------------------------------------------
// PPP (RFC 1661) and its control protocols
// --------------------------------------------------------------------------

/// PPP protocol frame (RFC 1661): a 16-bit `protocol` selector (LCP / IPCP / IP /
/// IPv6 / OSI / ...) chosen from the next layer. OSI PDUs (`0x0023`, RFC 1377) are
/// selected by their NLPID octet ([`crate::osi::osi_peek`]).
#[must_use]
pub fn ppp() -> ProtocolDef {
    let mut d = protocol! {
        id: "ppp",
        name: "PPP",
        layer: L2,
        fields: {
            protocol: enum(16) {
                0x0021 => ipv4,
                0x0023 => osi,
                0x0057 => ipv6,
                0x8021 => ipcp,
                0x8057 => ipv6cp,
                0xc021 => lcp,
                0xc023 => pap,
                0xc223 => chap,
            } = 0xc021, auto = next_protocol, desc = "ppp-protocol";
        },
        next: protocol {
            0xc021 => "ppp_lcp",
            0x8021 => "ppp_ipcp",
            0x0021 => "ipv4",
            0x0057 => "ipv6",
        },
    };
    if let Some(b) = d.next.as_mut() {
        b.peek = Some(crate::osi::osi_peek(0x0023));
    }
    d
}

/// LCP option TLVs (RFC 1661): 8-bit type / length (length includes the header).
fn lcp_options() -> FieldType {
    FieldType::Tlv(TlvDef {
        type_bits: 8,
        len_bits: 8,
        len_includes_header: true,
        len_unit: LengthUnit::Bytes,
        variants: vec![
            TlvVariant::new(1, "mru", vec![u("mru", 16)]),
            TlvVariant::new(2, "async_control_char_map", vec![u("map", 32)]),
            TlvVariant::new(3, "authentication_protocol", Vec::new()),
            TlvVariant::new(4, "quality_protocol", Vec::new()),
            TlvVariant::new(5, "magic_number", vec![u("magic", 32)]),
            TlvVariant::new(7, "protocol_field_compression", Vec::new()),
            TlvVariant::new(8, "address_control_compression", Vec::new()),
        ],
        unknown_as_raw: true,
        length: Length::FromField {
            field: "length".into(),
            unit: 1,
            adjust: -4,
            div: 1,
        },
        fill: None,
        head_octets: 0,
        align: 0,
        unpadded_last: false,
    })
}

/// IPCP option TLVs (RFC 1332): 8-bit type / length (length includes the header).
fn ipcp_options() -> FieldType {
    FieldType::Tlv(TlvDef {
        type_bits: 8,
        len_bits: 8,
        len_includes_header: true,
        len_unit: LengthUnit::Bytes,
        variants: vec![
            TlvVariant::new(2, "ip_compression_protocol", Vec::new()),
            TlvVariant::new(
                3,
                "ip_address",
                vec![FieldDef::new("address", FieldType::Ipv4)],
            ),
            TlvVariant::new(
                129,
                "primary_dns",
                vec![FieldDef::new("address", FieldType::Ipv4)],
            ),
            TlvVariant::new(
                131,
                "secondary_dns",
                vec![FieldDef::new("address", FieldType::Ipv4)],
            ),
        ],
        unknown_as_raw: true,
        length: Length::FromField {
            field: "length".into(),
            unit: 1,
            adjust: -4,
            div: 1,
        },
        fill: None,
        head_octets: 0,
        align: 0,
        unpadded_last: false,
    })
}

/// PPP LCP (RFC 1661): code, identifier, `length` (Auto over the whole CP packet)
/// and the option TLVs.
#[must_use]
pub fn ppp_lcp() -> ProtocolDef {
    protocol! {
        id: "ppp_lcp",
        name: "PPP LCP",
        layer: L2,
        fields: {
            code: enum(8) {
                1 => configure_request,
                2 => configure_ack,
                3 => configure_nak,
                4 => configure_reject,
                5 => terminate_request,
                6 => terminate_ack,
                7 => code_reject,
                8 => protocol_reject,
                9 => echo_request,
                10 => echo_reply,
                11 => discard_request,
            } = 1, desc = "ppp-cp-code";
            identifier: uint(8) = 0, desc = "ppp-cp-identifier";
            length: uint(16), auto = length(total), desc = "ppp-cp-length";
            options: ty(lcp_options()), desc = "ppp-cp-options";
        },
    }
}

/// PPP IPCP (RFC 1332): code, identifier, `length` (Auto) and the option TLVs.
#[must_use]
pub fn ppp_ipcp() -> ProtocolDef {
    protocol! {
        id: "ppp_ipcp",
        name: "PPP IPCP",
        layer: L2,
        fields: {
            code: enum(8) {
                1 => configure_request,
                2 => configure_ack,
                3 => configure_nak,
                4 => configure_reject,
                5 => terminate_request,
                6 => terminate_ack,
                7 => code_reject,
            } = 1, desc = "ppp-cp-code";
            identifier: uint(8) = 0, desc = "ppp-cp-identifier";
            length: uint(16), auto = length(total), desc = "ppp-cp-length";
            options: ty(ipcp_options()), desc = "ppp-cp-options";
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_defs_are_valid() {
        for d in [
            lldp(),
            cdp(),
            stp(),
            lacp(),
            marker(),
            eapol(),
            pppoe_discovery(),
            pppoe_session(),
            ppp(),
            ppp_lcp(),
            ppp_ipcp(),
        ] {
            d.validate()
                .unwrap_or_else(|e| panic!("{} invalid: {e}", d.id));
        }
        assert_eq!(stp().fixed_header_bits(), None); // trailer is variable
                                                     // LACPDU / Marker PDU are 110 octets.
        let lacp_bits: u32 = lacp().fields.iter().filter_map(|f| f.ty.fixed_bits()).sum();
        assert_eq!(lacp_bits, 110 * 8);
        let marker_bits: u32 = marker()
            .fields
            .iter()
            .filter_map(|f| f.ty.fixed_bits())
            .sum();
        assert_eq!(marker_bits, 110 * 8);
    }
}
