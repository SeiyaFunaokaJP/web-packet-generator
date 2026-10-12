# OSPF Opaque / SR, BGP MP / VPN / EVPN (P2)

Status: draft

Notes on the routing protocol extensions of M7: OSPFv2 Opaque LSAs (RFC 5250) with Traffic Engineering (RFC 3630), Router Information (RFC 7770), Extended Prefix / Extended Link (RFC 7684) and Segment Routing (RFC 8665), and the BGP multiprotocol NLRI (RFC 4760) for IPv4 / IPv6 unicast, VPN-IPv4 / VPN-IPv6 (RFC 4364 / RFC 4659) and EVPN (RFC 7432 / RFC 9136). The definitions are in `crates/wpg-protocols/src/ospf_opaque.rs` and `crates/wpg-protocols/src/bgp_mp.rs`; the basics of OSPF and BGP are on the [routing protocols](routing.md) page.

## OSPFv2 Opaque LSAs

```text
ipv4 / ospfv2 / ospfv2_lsu / ospfv2_opaque_lsa / ospfv2_te_lsa
ipv4 / ospfv2 / ospfv2_lsu / ospfv2_opaque_lsa / ospfv2_ri_lsa
ipv4 / ospfv2 / ospfv2_lsu / ospfv2_opaque_lsa / ospfv2_ext_prefix_lsa
ipv4 / ospfv2 / ospfv2_lsu / ospfv2_opaque_lsa / ospfv2_ext_link_lsa
```

`ospfv2_opaque_lsa` is the LSA header with the Link State ID split into `opaque_type` (8 bits, Auto from the body layer) and `opaque_id` (24 bits). `ls_type` gives the flooding scope: 9 link-local, 10 area (default), 11 AS. The LSA `length` and the Fletcher `ls_checksum` are Auto as for the other LSAs. When parsing, an LSA of type 9 / 10 / 11 is first read as `ospfv2_lsa` and then re-decoded as `ospfv2_opaque_lsa`; an unknown opaque type is followed by a `raw` layer.

| Sample | Contents |
|---|---|
| `ospfv2-lsu-te-lsa` | TE LSA: Link TLV with link type / ID, interface addresses, TE metric, bandwidths, admin group |
| `ospfv2-lsu-ri-sr` | Router Information LSA: capabilities, hostname, SR-Algorithm, SRGB (SID/Label Range), SRLB |
| `ospfv2-lsu-ext-prefix-sr` | Extended Prefix LSA: 192.0.2.1/32 with a node Prefix SID (index 1) |
| `ospfv2-lsu-ext-link-sr` | Extended Link LSA: a point-to-point link with an Adj-SID (label 24000) |

### TLVs

Each body has one field, `tlvs`. TLVs and sub-TLVs have a 16-bit type and a 16-bit length counting the value only; the value is padded with zeros to a multiple of 4 octets (not counted in the length). Lengths and padding are Auto.

| Body | TLVs | Sub-TLVs |
|---|---|---|
| `ospfv2_te_lsa` (opaque type 1) | `router_address` (1), `link` (2) | `link_type`, `link_id`, `local_address`, `remote_address`, `te_metric`, `max_bandwidth`, `max_reservable_bandwidth`, `unreserved_bandwidth`, `admin_group` |
| `ospfv2_ri_lsa` (4) | `informational_capabilities` (1), `functional_capabilities` (2), `hostname` (7), `sr_algorithm` (8), `sid_label_range` (9), `sr_local_block` (14), `srms_preference` (15) | `sid_label` / `sid_label_index` |
| `ospfv2_ext_prefix_lsa` (7) | `extended_prefix` (1), `extended_prefix_range` (2) | `prefix_sid` / `prefix_sid_label` |
| `ospfv2_ext_link_lsa` (8) | `extended_link` (1) | `adj_sid` / `adj_sid_index`, `lan_adj_sid` / `lan_adj_sid_index` |

A SID is 3 octets (a label in the low 20 bits, V and L flags set) or 4 octets (an index, V and L clear). Every sub-TLV that carries one exists in both forms; pick the item by name (`label` or `index` field) and the parser tells them apart by the value length:

```yaml
- protocol: ospfv2_ext_prefix_lsa
  fields:
    tlvs:
      - extended_prefix:
          route_type: intra_area
          prefix_length: 32
          flags: n
          prefix: 192.0.2.1
          sub_tlvs:
            - prefix_sid: { algorithm: 0, index: 1 }          # 4 octets
            # - prefix_sid_label: { flags: v|l, label: 16001 } # 3 octets
```

TE bandwidths are the raw 32-bit IEEE floating point values in octets per second (`0x4b3ebc20` = 12.5e6 = 100 Mbit/s).

## BGP multiprotocol NLRI

`MP_REACH_NLRI` (code 14) and `MP_UNREACH_NLRI` (code 15) have a layout that depends on the AFI / SAFI after them. Each modelled family is its own path attribute item; the parser picks it from the first three value octets (AFI, SAFI):

| Family | AFI / SAFI | Items | Next hop | NLRI element |
|---|---|---|---|---|
| IPv4 unicast | 1 / 1 | `mp_reach_ipv4_unicast`, `mp_unreach_ipv4_unicast` | list of IPv4 addresses | `{length, prefix}` |
| IPv6 unicast | 2 / 1 | `mp_reach_ipv6_unicast`, `mp_unreach_ipv6_unicast` | list of IPv6 addresses (global, link-local) | `{length, prefix}` |
| VPN-IPv4 | 1 / 128 | `mp_reach_vpnv4`, `mp_unreach_vpnv4` | list of `{rd, address}` | `{length, label, exp, bos, rd, prefix}` |
| VPN-IPv6 | 2 / 128 | `mp_reach_vpnv6`, `mp_unreach_vpnv6` | list of `{rd, address}` | same |
| EVPN | 25 / 70 | `mp_reach_evpn`, `mp_unreach_evpn` | octets (IPv4 or IPv6 address) | EVPN route items |

Other families use the generic `mp_reach_nlri` / `mp_unreach_nlri` with the next hop and NLRI as octets. `afi` / `safi` default to the family; `next_hop_length` is Auto.

- **Prefixes**: `length` is in bits and `prefix` has `ceil(length / 8)` octets. The UPDATE `withdrawn_routes` and `nlri` are prefix lists too, and both UPDATE length fields are Auto.
- **VPN prefixes**: `length` counts the label (24 bits), the route distinguisher (64 bits) and the prefix bits, so a /24 is 112. One label is modelled (`bos: 1`); a withdrawal usually carries `label: 0x80000, bos: 0` (the 0x800000 compatibility value).
- **Route distinguishers** are 8 octets: `0000 fde9 00000064` is 65001:100 (type 0), `0001 c0000201 0001` is 192.0.2.1:1 (type 1).
- **EVPN routes** (8-bit type, 8-bit length): `ethernet_ad` (1), `mac_ip` (2), `inclusive_multicast` (3), `ethernet_segment` (4), `ip_prefix` (5, IPv4, 34 octets) and `ipv6_prefix` (5, IPv6, 58 octets). ESIs are 10 octets. Label fields are the 3-octet wire value: `label << 4 | bos` for MPLS, the VNI for VXLAN (RFC 8365). IP addresses with a length field (`ip_length` in bits) are octets.
- The **PMSI Tunnel** attribute (`pmsi_tunnel`, code 22) is typed; route targets are `extended_communities` values (`0x0002fde900000064` = RT 65001:100).

```yaml
- protocol: bgp_update
  fields:
    path_attributes:
      - { origin: 0 }
      - { as_path: { segments: [] } }
      - mp_reach_vpnv4:
          next_hop: [{ rd: "0000000000000000", address: 192.0.2.1 }]
          nlri:
            - { length: 112, label: 16, bos: 1, rd: "0000 fde9 00000064", prefix: "c6 33 64" }
    nlri: []
```

| Sample | Contents |
|---|---|
| `bgp-update-mp-ipv6` | IPv6 unicast announcement (global + link-local next hop) and withdrawal |
| `bgp-update-vpnv4` | VPN-IPv4 route with label 16, RD and route target 65001:100 |
| `bgp-update-evpn` | EVPN MAC/IP and Inclusive Multicast routes for VNI 10010, route target, VXLAN encapsulation, PMSI Tunnel |

### Extended Length

A path attribute whose value exceeds 255 octets gets the Extended Length flag (`0x10`) and a 16-bit length automatically. To send the long form for a short attribute (as FRR and GoBGP do for `MP_REACH_NLRI`), give the item an explicit type:

```yaml
- { name: mp_reach_ipv4_unicast, type: 0x900e, fields: { next_hop: [192.0.2.1], nlri: [{ length: 24, prefix: "c63364" }] } }
```

Received attributes keep their form: a parsed Extended Length attribute is exported with its `type`.

## Limits

- OSPFv3 Extended LSAs (RFC 8362) and OSPFv3 Segment Routing (RFC 8666) are not modelled.
- The OSPF TE GMPLS sub-TLVs (RFC 4203) and the RI TLVs other than those listed stay raw items.
- BGP ADD-PATH path identifiers (RFC 7911), label stacks of more than one label (RFC 8277) and the BGP Prefix-SID attribute (RFC 8669) are not modelled. Write such NLRI as octets with the generic `mp_reach_nlri` / `mp_unreach_nlri` items; when parsing such NLRI, the octets still rebuild identically but the decoded fields do not line up with them.
- Several BGP messages in one segment, and several typed LSAs in one LSU, are not supported (see the [routing protocols](routing.md) page).
