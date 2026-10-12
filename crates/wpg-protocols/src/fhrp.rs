//! First-hop redundancy protocols (TODO §2.1 P1): `VRRPv2` / v3, `HSRPv1` / v2 and
//! GLBP (basic).
//!
//! ## Engine limitations exercised here
//!
//! * **`VRRPv2` and `VRRPv3`** share IP protocol 112 and are told apart only by the
//!   version nibble: IPv4 protocol 112 decodes as [`vrrpv3`], which re-decodes
//!   itself as [`vrrpv2`] when `version` is 2 (a re-parse binding entry).
//! * **`VRRPv3`** carries IPv4 *or* IPv6 addresses depending on the IP version of the
//!   enclosing packet. The engine cannot size a field from a lower layer, so there
//!   are two definitions: [`vrrpv3`] (IPv4 addresses, bound from IPv4) and
//!   [`vrrpv3_ipv6`] (IPv6 addresses, bound from IPv6 next header 112). The address
//!   counts are Auto ([`wpg_core::AutoCompute::Count`]).
//! * **`HSRPv1` and `HSRPv2`** both use UDP port 1985. The first octet is the
//!   version (0) in v1 and the first TLV type (1, Group State) in v2, so UDP 1985
//!   decodes as [`hsrp`], which re-decodes itself as [`hsrpv2`] for a first octet
//!   of 1. `HSRPv2`'s 16-octet virtual address is IPv4 (zero padded) or IPv6
//!   according to the `ip_version` octet, so it is opaque `bytes(16)`.
//! * **GLBP** is undocumented; only the Hello TLV (IPv4 virtual address) is typed,
//!   Request/Response and Authentication TLVs keep their value octets.

use wpg_core::{
    protocol, Binding, BindingEntry, FieldDef, FieldType, FieldValue, ItemDef, Length, LengthUnit,
    ProtocolDef, TlvDef, TlvItem, TlvVariant,
};

fn u(name: &str, bits: u8, default: u64) -> FieldDef {
    FieldDef::new(name, FieldType::UInt { bits }).with_default(FieldValue::UInt(default))
}

/// A list of IPv4 addresses whose octet length is `field * unit`.
fn ipv4_list(field: &str, unit: u32) -> FieldType {
    FieldType::List {
        item: ItemDef::Field {
            ty: Box::new(FieldType::Ipv4),
        },
        length: Length::FromField {
            field: field.into(),
            unit,
            adjust: 0,
        },
    }
}

// --------------------------------------------------------------------------
// VRRP (RFC 3768 / RFC 5798), IP protocol 112
// --------------------------------------------------------------------------

/// `VRRPv2` (RFC 3768, IPv4 protocol 112): version 2 / type 1 (Advertisement),
/// VRID, priority, `count_ip_addrs` (Auto from the address list), authentication
/// type, advertisement interval (seconds), Internet `checksum` (Auto, no pseudo
/// header), the IPv4 `ip_addresses` and 8 octets of `auth_data`.
#[must_use]
pub fn vrrpv2() -> ProtocolDef {
    protocol! {
        id: "vrrpv2",
        name: "VRRPv2",
        layer: L3,
        fields: {
            version: uint(4) = 2, desc = "vrrp-version";
            type: enum(4) { 1 => advertisement } = 1, desc = "vrrp-type";
            vrid: uint(8) = 1, desc = "vrrp-vrid";
            priority: uint(8) = 100, desc = "vrrp-priority";
            count_ip_addrs: uint(8), auto = count(ip_addresses), desc = "vrrp-count";
            auth_type: enum(8) {
                0 => no_authentication,
                1 => simple_text_password,
                2 => ip_authentication_header,
            } = 0, desc = "vrrp-auth-type";
            adver_int: uint(8) = 1, desc = "vrrp-adver-int";
            checksum: uint(16), auto = checksum(internet, header_payload), desc = "vrrp-checksum";
            ip_addresses: ty(ipv4_list("count_ip_addrs", 4)) = vec![FieldValue::Ipv4([192, 0, 2, 254].into())], desc = "vrrp-addresses";
            auth_data: bytes(8) = [0u8; 8], desc = "vrrp-auth-data";
        },
        constraints: {
            warning: equals(version, 2);
        },
    }
}

/// `VRRPv3` for IPv4 (RFC 5798, IPv4 protocol 112): version 3 / type 1, VRID,
/// priority, `count_ipvx_addrs` (Auto), 12-bit `max_adver_int` (centiseconds),
/// `checksum` (Auto, with the IPv4 pseudo header) and the IPv4 `ip_addresses`.
/// A `version` of 2 re-decodes the message as [`vrrpv2`].
#[must_use]
pub fn vrrpv3() -> ProtocolDef {
    let mut d = protocol! {
        id: "vrrpv3",
        name: "VRRPv3",
        layer: L3,
        fields: {
            version: uint(4) = 3, desc = "vrrp-version";
            type: enum(4) { 1 => advertisement } = 1, desc = "vrrp-type";
            vrid: uint(8) = 1, desc = "vrrp-vrid";
            priority: uint(8) = 100, desc = "vrrp-priority";
            count_ipvx_addrs: uint(8), auto = count(ip_addresses), desc = "vrrp-count";
            reserved: uint(4) = 0;
            max_adver_int: uint(12) = 100, desc = "vrrp-max-adver-int";
            checksum: uint(16), auto = checksum(internet_pseudo, header_payload), desc = "vrrp-checksum";
            ip_addresses: ty(ipv4_list("count_ipvx_addrs", 4)) = vec![FieldValue::Ipv4([192, 0, 2, 254].into())], desc = "vrrp-addresses";
        },
        constraints: {
            warning: equals(version, 3);
        },
    };
    d.next = Some(Binding::new(
        "version",
        vec![BindingEntry::new(2, "vrrpv2").reparsing()],
    ));
    d
}

/// `VRRPv3` for IPv6 (RFC 5798, IPv6 next header 112): as [`vrrpv3`] but with IPv6
/// addresses (the first one is the link-local virtual address).
#[must_use]
pub fn vrrpv3_ipv6() -> ProtocolDef {
    protocol! {
        id: "vrrpv3_ipv6",
        name: "VRRPv3 (IPv6)",
        layer: L3,
        fields: {
            version: uint(4) = 3, desc = "vrrp-version";
            type: enum(4) { 1 => advertisement } = 1, desc = "vrrp-type";
            vrid: uint(8) = 1, desc = "vrrp-vrid";
            priority: uint(8) = 100, desc = "vrrp-priority";
            count_ipvx_addrs: uint(8), auto = count(ip_addresses), desc = "vrrp-count";
            reserved: uint(4) = 0;
            max_adver_int: uint(12) = 100, desc = "vrrp-max-adver-int";
            checksum: uint(16), auto = checksum(internet_pseudo, header_payload), desc = "vrrp-checksum";
            ip_addresses: ty(FieldType::List {
                item: ItemDef::Field { ty: Box::new(FieldType::Ipv6) },
                length: Length::FromField { field: "count_ipvx_addrs".into(), unit: 16, adjust: 0 },
            }) = vec![FieldValue::Ipv6([0xfe80u16, 0, 0, 0, 0, 0, 0, 1].into())], desc = "vrrp-addresses";
        },
        constraints: {
            warning: equals(version, 3);
        },
    }
}

// --------------------------------------------------------------------------
// HSRP (RFC 2281 / Cisco HSRPv2), UDP 1985 (IPv6: 2029)
// --------------------------------------------------------------------------

/// `HSRPv1` (RFC 2281, UDP 1985 to 224.0.0.2): the fixed 20-octet message.
/// Timers are in seconds; `auth_data` is the 8-octet clear-text password
/// (default `cisco`). A first octet of 1 (an `HSRPv2` Group State TLV) re-decodes
/// the message as [`hsrpv2`].
#[must_use]
pub fn hsrp() -> ProtocolDef {
    let mut d = protocol! {
        id: "hsrp",
        name: "HSRPv1",
        layer: L7,
        fields: {
            version: uint(8) = 0, desc = "hsrp-version";
            op_code: enum(8) {
                0 => hello,
                1 => coup,
                2 => resign,
                3 => advertise,
            } = 0, desc = "hsrp-op-code";
            state: enum(8) {
                0 => initial,
                1 => learn,
                2 => listen,
                4 => speak,
                8 => standby,
                16 => active,
            } = 16, desc = "hsrp-state";
            hello_time: uint(8) = 3, desc = "hsrp-hello-time";
            hold_time: uint(8) = 10, desc = "hsrp-hold-time";
            priority: uint(8) = 100, desc = "hsrp-priority";
            group: uint(8) = 0, desc = "hsrp-group";
            reserved: uint(8) = 0;
            auth_data: bytes(8) = *b"cisco\0\0\0", desc = "hsrp-auth-data";
            virtual_ip: ipv4 = [192, 0, 2, 254], desc = "hsrp-virtual-ip";
        },
    };
    d.next = Some(Binding::new(
        "version",
        vec![BindingEntry::new(1, "hsrpv2").reparsing()],
    ));
    d
}

/// `HSRPv2` TLVs: 8-bit type, 8-bit length excluding the header. Group State (1,
/// 40 octets, timers in milliseconds), Interface State (2), Text Authentication
/// (3) and MD5 Authentication (4, opaque).
#[must_use]
pub fn hsrpv2_tlvs() -> FieldType {
    FieldType::Tlv(TlvDef {
        type_bits: 8,
        len_bits: 8,
        len_includes_header: false,
        len_unit: LengthUnit::Bytes,
        variants: vec![
            TlvVariant::new(
                1,
                "group_state",
                vec![
                    u("version", 8, 2),
                    u("op_code", 8, 0),
                    u("state", 8, 6),
                    u("ip_version", 8, 4),
                    u("group", 16, 0),
                    FieldDef::new("identifier", FieldType::Mac)
                        .with_default(FieldValue::Mac([0x02, 0, 0, 0, 0, 0x01])),
                    u("priority", 32, 100),
                    u("hello_time", 32, 3000),
                    u("hold_time", 32, 10000),
                    FieldDef::new(
                        "virtual_ip",
                        FieldType::Bytes {
                            len: Length::Fixed { octets: 16 },
                        },
                    )
                    .with_default(FieldValue::Bytes({
                        let mut v = vec![192, 0, 2, 254];
                        v.resize(16, 0);
                        v
                    })),
                ],
            ),
            TlvVariant::new(
                2,
                "interface_state",
                vec![u("active_groups", 16, 1), u("passive_groups", 16, 0)],
            ),
            TlvVariant::new(
                3,
                "text_auth",
                vec![FieldDef::new(
                    "auth_data",
                    FieldType::Bytes {
                        len: Length::Fixed { octets: 8 },
                    },
                )
                .with_default(FieldValue::Bytes(b"cisco\0\0\0".to_vec()))],
            ),
            TlvVariant::new(4, "md5_auth", Vec::new()),
        ],
        unknown_as_raw: true,
        length: Length::ToEnd,
    })
}

/// `HSRPv2` (Cisco, UDP 1985 to 224.0.0.102 or UDP 2029 to `ff02::66`): a sequence of
/// TLVs ([`hsrpv2_tlvs`]); defaults to one Group State TLV (Hello, Active, group
/// 0, IPv4 virtual address 192.0.2.254).
#[must_use]
pub fn hsrpv2() -> ProtocolDef {
    protocol! {
        id: "hsrpv2",
        name: "HSRPv2",
        layer: L7,
        fields: {
            tlvs: ty(hsrpv2_tlvs()) = vec![TlvItem::named("group_state")], desc = "hsrpv2-tlvs";
        },
    }
}

// --------------------------------------------------------------------------
// GLBP (Cisco), UDP 3222
// --------------------------------------------------------------------------

/// GLBP TLVs: 8-bit type, 8-bit length including the header. Hello (1) is typed
/// (IPv4 virtual address, timers in milliseconds); Request/Response (2) and Auth
/// (3) keep their value octets.
#[must_use]
pub fn glbp_tlvs() -> FieldType {
    FieldType::Tlv(TlvDef {
        type_bits: 8,
        len_bits: 8,
        len_includes_header: true,
        len_unit: LengthUnit::Bytes,
        variants: vec![
            TlvVariant::new(
                1,
                "hello",
                vec![
                    u("unknown1", 8, 0),
                    u("vg_state", 8, 32),
                    u("unknown2", 8, 0),
                    u("priority", 8, 100),
                    u("unknown3", 16, 0),
                    u("hello_interval", 32, 3000),
                    u("hold_interval", 32, 10000),
                    u("redirect", 16, 600),
                    u("timeout", 16, 14400),
                    u("unknown4", 16, 0),
                    u("address_type", 8, 1),
                    u("address_length", 8, 4),
                    FieldDef::new("virtual_ip", FieldType::Ipv4)
                        .with_default(FieldValue::Ipv4([192, 0, 2, 254].into())),
                ],
            ),
            TlvVariant::new(2, "request_response", Vec::new()),
            TlvVariant::new(3, "auth", Vec::new()),
        ],
        unknown_as_raw: true,
        length: Length::ToEnd,
    })
}

/// GLBP (Cisco Gateway Load Balancing Protocol, UDP 3222 to 224.0.0.102): version,
/// group, the sender's `owner_id` MAC and the TLVs ([`glbp_tlvs`]). Defaults to a
/// single Hello TLV.
#[must_use]
pub fn glbp() -> ProtocolDef {
    protocol! {
        id: "glbp",
        name: "GLBP",
        layer: L7,
        fields: {
            version: uint(8) = 1, desc = "glbp-version";
            unknown1: uint(8) = 0;
            group: uint(16) = 1, desc = "glbp-group";
            unknown2: uint(16) = 0;
            owner_id: mac = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01], desc = "glbp-owner-id";
            tlvs: ty(glbp_tlvs()) = vec![TlvItem::named("hello")], desc = "glbp-tlvs";
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_defs_are_valid() {
        for d in [vrrpv2(), vrrpv3(), vrrpv3_ipv6(), hsrp(), hsrpv2(), glbp()] {
            d.validate()
                .unwrap_or_else(|e| panic!("{} invalid: {e}", d.id));
        }
        assert_eq!(hsrp().fixed_header_bits(), Some(160));
        // VRRPv2 fixed part: 8-octet header + 8-octet auth data.
        let bits: u32 = vrrpv2()
            .fields
            .iter()
            .filter_map(|f| f.ty.fixed_bits())
            .sum();
        assert_eq!(bits, 128);
    }
}
