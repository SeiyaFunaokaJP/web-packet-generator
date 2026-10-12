//! BGP multiprotocol extensions (TODO §2.1 P2, M7): typed NLRI for IPv4 / IPv6
//! unicast (RFC 4760), VPN-IPv4 / VPN-IPv6 (RFC 4364 / RFC 4659) and EVPN (RFC
//! 7432, RFC 9136) inside `MP_REACH_NLRI` / `MP_UNREACH_NLRI`, the prefix lists of
//! the UPDATE itself and the PMSI Tunnel attribute (RFC 6514).
//!
//! Both MP attributes are one path attribute code each, whose layout after the
//! AFI / SAFI depends on the family. They are therefore several TLV variants of
//! the same type told apart by the first three value octets (AFI, SAFI,
//! [`wpg_core::TlvVariant::value_prefix`]): `mp_reach_ipv4_unicast`,
//! `mp_reach_vpnv4`, `mp_reach_evpn`, ... The generic `mp_reach_nlri` /
//! `mp_unreach_nlri` (next hop and NLRI as octets) remain for other families.
//!
//! * A prefix is `{length, prefix}` with `ceil(length / 8)` prefix octets.
//! * A VPN prefix is `{length, label, exp, bos, rd, prefix}`: `length` counts the
//!   label (24 bits), the route distinguisher (64 bits) and the prefix bits, so a
//!   /24 is 112. One label is modelled (the usual case); a label stack is written
//!   as verbatim octets.
//! * Route distinguishers are 8 octets (type, then administrator and assigned
//!   number), e.g. `00 00 fd e9 00 00 00 64` for 65001:100.
//! * EVPN routes are 8-bit type / 8-bit length items. MPLS label fields are the
//!   3-octet wire value (label << 4, or the VNI for VXLAN, RFC 8365). The IP
//!   Prefix route (type 5) is decoded by its length: 34 octets IPv4, 58 IPv6.
//! * `next_hop_length` is Auto (size of the next hop). The withdrawn routes and
//!   path attribute lengths of the UPDATE are Auto too, and an attribute longer
//!   than 255 octets gets the Extended Length flag on its own
//!   ([`wpg_core::TlvLongLength`]).
//!
//! ADD-PATH path identifiers (RFC 7911) are not modelled.

use wpg_core::{
    AutoCompute, FieldDef, FieldType, FieldValue, ItemDef, Length, LengthUnit, TlvDef, TlvItem,
    TlvVariant,
};

/// The modelled families: (name, AFI, SAFI).
pub const FAMILIES: [(&str, u16, u8); 5] = [
    ("ipv4_unicast", 1, 1),
    ("ipv6_unicast", 2, 1),
    ("vpnv4", 1, 128),
    ("vpnv6", 2, 128),
    ("evpn", 25, 70),
];

/// Route distinguisher 65001:100 (type 0).
pub const RD: [u8; 8] = [0, 0, 0xfd, 0xe9, 0, 0, 0, 0x64];

fn u(name: &str, bits: u8) -> FieldDef {
    FieldDef::new(name, FieldType::UInt { bits })
}

fn octets(name: &str, n: u32) -> FieldDef {
    FieldDef::new(
        name,
        FieldType::Bytes {
            len: Length::Fixed { octets: n },
        },
    )
}

fn from_field(name: &str, field: &str, adjust: i64, div: u32) -> FieldDef {
    FieldDef::new(
        name,
        FieldType::Bytes {
            len: Length::FromField {
                field: field.into(),
                unit: 1,
                adjust,
                div,
            },
        },
    )
}

fn list(name: &str, item: ItemDef, length: Length) -> FieldDef {
    FieldDef::new(name, FieldType::List { item, length })
}

fn to_end(name: &str) -> FieldDef {
    FieldDef::new(name, FieldType::Bytes { len: Length::ToEnd })
}

fn struct_value(pairs: Vec<(&str, FieldValue)>) -> FieldValue {
    FieldValue::Struct(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

/// One IPv4 / IPv6 prefix: `length` in bits and `ceil(length / 8)` octets.
#[must_use]
pub fn prefix_item() -> ItemDef {
    ItemDef::Struct {
        fields: vec![u("length", 8), from_field("prefix", "length", 7, 8)],
    }
}

/// A prefix list value: `(length, octets)` pairs.
#[must_use]
pub fn prefixes(list: &[(u8, &[u8])]) -> FieldValue {
    FieldValue::List(
        list.iter()
            .map(|(len, p)| {
                struct_value(vec![
                    ("length", FieldValue::UInt(u64::from(*len))),
                    ("prefix", FieldValue::Bytes(p.to_vec())),
                ])
            })
            .collect(),
    )
}

/// A list of prefixes filling the region given by `length`.
#[must_use]
pub fn prefix_list(length: Length) -> FieldType {
    FieldType::List {
        item: prefix_item(),
        length,
    }
}

/// One labelled VPN prefix (RFC 4364 §4.3.4 / RFC 8277): `length` counts the label,
/// the route distinguisher and the prefix bits.
#[must_use]
pub fn vpn_prefix_item() -> ItemDef {
    ItemDef::Struct {
        fields: vec![
            u("length", 8),
            u("label", 20),
            u("exp", 3),
            u("bos", 1),
            octets("rd", 8),
            // ceil((length - 24 - 64) / 8)
            from_field("prefix", "length", 7 - 88, 8),
        ],
    }
}

/// A VPN prefix value with label `label` (bottom of stack).
#[must_use]
pub fn vpn_prefix(prefix_len: u8, label: u32, prefix: &[u8]) -> FieldValue {
    struct_value(vec![
        ("length", FieldValue::UInt(88 + u64::from(prefix_len))),
        ("label", FieldValue::UInt(u64::from(label))),
        ("exp", FieldValue::UInt(0)),
        ("bos", FieldValue::UInt(1)),
        ("rd", FieldValue::Bytes(RD.to_vec())),
        ("prefix", FieldValue::Bytes(prefix.to_vec())),
    ])
}

fn ip_length_and_address(len_name: &str, name: &str) -> [FieldDef; 2] {
    [u(len_name, 8), from_field(name, len_name, 0, 8)]
}

/// EVPN routes (RFC 7432 §7, RFC 9136 §3): 8-bit route type, 8-bit length.
#[must_use]
pub fn evpn_routes() -> FieldType {
    let head = |with_esi: bool, with_tag: bool| {
        let mut f = vec![octets("rd", 8)];
        if with_esi {
            f.push(octets("esi", 10));
        }
        if with_tag {
            f.push(u("ethernet_tag", 32));
        }
        f
    };
    let mut ad = head(true, true);
    ad.push(u("mpls_label", 24));
    let mut mac_ip = head(true, true);
    mac_ip.extend([u("mac_length", 8), FieldDef::new("mac", FieldType::Mac)]);
    mac_ip.extend(ip_length_and_address("ip_length", "ip_address"));
    mac_ip.push(list(
        "labels",
        ItemDef::Field {
            ty: Box::new(FieldType::UInt { bits: 24 }),
        },
        Length::ToEnd,
    ));
    let mut imet = head(false, true);
    imet.extend(ip_length_and_address("ip_length", "originating_ip"));
    let mut es = head(true, false);
    es.extend(ip_length_and_address("ip_length", "originating_ip"));
    let ip_prefix = |addr: FieldType| {
        let mut f = head(true, true);
        f.extend([
            u("ip_prefix_length", 8),
            FieldDef::new("ip_prefix", addr.clone()),
            FieldDef::new("gateway", addr),
            u("mpls_label", 24),
        ]);
        f
    };
    FieldType::Tlv(TlvDef {
        type_bits: 8,
        len_bits: 8,
        len_includes_header: false,
        len_unit: LengthUnit::Bytes,
        variants: vec![
            TlvVariant::new(1, "ethernet_ad", ad),
            TlvVariant::new(2, "mac_ip", mac_ip),
            TlvVariant::new(3, "inclusive_multicast", imet),
            TlvVariant::new(4, "ethernet_segment", es),
            TlvVariant::new(5, "ip_prefix", ip_prefix(FieldType::Ipv4)).when_value_len(34),
            TlvVariant::new(5, "ipv6_prefix", ip_prefix(FieldType::Ipv6)).when_value_len(58),
        ],
        unknown_as_raw: true,
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

/// The NLRI list of a family (`name` from [`FAMILIES`]).
fn nlri_field(family: &str, name: &str) -> FieldDef {
    match family {
        "vpnv4" | "vpnv6" => list(name, vpn_prefix_item(), Length::ToEnd),
        "evpn" => FieldDef::new(name, evpn_routes()),
        _ => FieldDef::new(name, prefix_list(Length::ToEnd)),
    }
}

/// The next hop of a family: addresses (unicast: global [+ link-local]), RD +
/// address pairs (VPN, RFC 4364 §5 / RFC 4659 §3.2.1) or octets (EVPN: an IPv4
/// or IPv6 address).
fn next_hop_field(family: &str) -> FieldDef {
    let len = Length::FromField {
        field: "next_hop_length".into(),
        unit: 1,
        adjust: 0,
        div: 1,
    };
    let addr = |ty: FieldType| ItemDef::Field { ty: Box::new(ty) };
    let rd_addr = |ty: FieldType| ItemDef::Struct {
        fields: vec![octets("rd", 8), FieldDef::new("address", ty)],
    };
    match family {
        "ipv4_unicast" => list("next_hop", addr(FieldType::Ipv4), len),
        "ipv6_unicast" => list("next_hop", addr(FieldType::Ipv6), len),
        "vpnv4" => list("next_hop", rd_addr(FieldType::Ipv4), len),
        "vpnv6" => list("next_hop", rd_addr(FieldType::Ipv6), len),
        _ => FieldDef::new("next_hop", FieldType::Bytes { len }),
    }
}

fn afi_safi(afi: u16, safi: u8) -> [FieldDef; 2] {
    [
        u("afi", 16).with_default(FieldValue::UInt(u64::from(afi))),
        u("safi", 8).with_default(FieldValue::UInt(u64::from(safi))),
    ]
}

/// `MP_REACH_NLRI` (flags `0x80`, code 14) and `MP_UNREACH_NLRI` (code 15)
/// variants of every family in [`FAMILIES`], before the generic ones.
#[must_use]
pub fn mp_variants() -> Vec<TlvVariant> {
    let mut reach = Vec::new();
    let mut unreach = Vec::new();
    for (family, afi, safi) in FAMILIES {
        let [a, b] = afi_safi(afi, safi);
        let [hi, lo] = afi.to_be_bytes();
        let key = [hi, lo, safi];
        reach.push(
            TlvVariant::new(
                0x800e,
                format!("mp_reach_{family}"),
                vec![
                    a.clone(),
                    b.clone(),
                    u("next_hop_length", 8).with_auto(AutoCompute::Size {
                        field: "next_hop".into(),
                    }),
                    next_hop_field(family),
                    u("reserved", 8),
                    nlri_field(family, "nlri"),
                ],
            )
            .when_value_starts(&key),
        );
        unreach.push(
            TlvVariant::new(
                0x800f,
                format!("mp_unreach_{family}"),
                vec![a, b, nlri_field(family, "withdrawn")],
            )
            .when_value_starts(&key),
        );
    }
    reach.push(TlvVariant::new(
        0x800e,
        "mp_reach_nlri",
        vec![
            u("afi", 16),
            u("safi", 8),
            u("next_hop_length", 8).with_auto(AutoCompute::Size {
                field: "next_hop".into(),
            }),
            next_hop_field("other"),
            u("reserved", 8),
            to_end("nlri"),
        ],
    ));
    unreach.push(TlvVariant::new(
        0x800f,
        "mp_unreach_nlri",
        vec![u("afi", 16), u("safi", 8), to_end("withdrawn")],
    ));
    reach.extend(unreach);
    reach
}

/// PMSI Tunnel attribute (flags `0xc0`, code 22, RFC 6514 §5): flags, tunnel type
/// (6 = ingress replication for EVPN, RFC 7432 §11.2), MPLS label and tunnel
/// identifier.
#[must_use]
pub fn pmsi_tunnel() -> TlvVariant {
    TlvVariant::new(
        0xc016,
        "pmsi_tunnel",
        vec![
            u("flags", 8),
            FieldDef::new(
                "tunnel_type",
                FieldType::Enum {
                    bits: 8,
                    variants: [
                        (0, "none"),
                        (1, "rsvp_te_p2mp"),
                        (2, "mldp_p2mp"),
                        (3, "pim_ssm"),
                        (4, "pim_sm"),
                        (5, "bidir_pim"),
                        (6, "ingress_replication"),
                        (7, "mldp_mp2mp"),
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
            u("mpls_label", 24),
            to_end("tunnel_id"),
        ],
    )
}

/// An EVPN MAC/IP Advertisement route (type 2) value with one label.
#[must_use]
pub fn evpn_mac_ip(mac: [u8; 6], ip: &[u8], label: u32) -> TlvItem {
    TlvItem::named("mac_ip")
        .with("rd", RD.to_vec())
        .with("esi", vec![0u8; 10])
        .with("ethernet_tag", 0u32)
        .with("mac_length", 48u8)
        .with("mac", FieldValue::Mac(mac))
        .with("ip_length", u8::try_from(ip.len() * 8).unwrap_or(0))
        .with("ip_address", ip.to_vec())
        .with("labels", vec![FieldValue::UInt(u64::from(label))])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_variants_are_discriminated() {
        let v = mp_variants();
        assert_eq!(v.len(), 2 * FAMILIES.len() + 2);
        let evpn = v.iter().find(|v| v.name == "mp_reach_evpn").unwrap();
        assert_eq!(evpn.value_prefix, [0, 25, 70]);
        assert!(v
            .iter()
            .filter(|v| v.name.ends_with("_nlri"))
            .all(|v| v.value_prefix.is_empty()));
    }
}
