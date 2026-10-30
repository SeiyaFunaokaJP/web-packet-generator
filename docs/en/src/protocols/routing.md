# Routing protocols

Status: draft

Notes on OSPFv2 / OSPFv3 / BGP-4 / RIPv1 and v2 / RIPng / EIGRP. Definitions are in `crates/wpg-protocols/src/routing.rs`, samples in `templates/protocols/routing/`. OSPFv2 Opaque LSAs / Segment Routing and the BGP multiprotocol NLRI (IPv6, VPN, EVPN) are described on the [OSPF Opaque / SR, BGP MP / VPN / EVPN](routing-ext.md) page.

## How they are reached, and samples

| id | Protocol | Layer below | Samples |
|---|---|---|---|
| `ospfv2` + body | OSPFv2 (RFC 2328) | IPv4 protocol 89 | `ospfv2-hello` / `-dbd` / `-lsr` / `-lsu-*` / `-lsack` (opaque LSAs: `ospfv2-lsu-te-lsa`, `-ri-sr`, `-ext-prefix-sr`, `-ext-link-sr`) |
| `ospfv3` + body | OSPFv3 (RFC 5340) | IPv6 next header 89 | `ospfv3-hello` / `-dbd` / `-lsr` / `-lsu-*` / `-lsack` |
| `bgp` + body | BGP-4 | TCP 179 (destination, else source) | `bgp-open` / `-update` / `-keepalive` / `-notification` / `-route-refresh`, `bgp-update-mp-ipv6` / `-vpnv4` / `-evpn` |
| `rip` | RIPv1 / v2 | UDP 520 | `ripv1-request`, `ripv2-response` |
| `ripng` | RIPng | UDP 521 | `ripng-response` |
| `eigrp` | EIGRP (basic TLVs) | IP protocol 88 (IPv4 / IPv6) | `eigrp-hello`, `eigrp-update` |

### Stacking the layers

OSPF and BGP are two layers, the common header and the body (an LSU adds an LSA header and an LSA body).

```text
ipv4 / ospfv2 / ospfv2_hello
ipv4 / ospfv2 / ospfv2_lsu / ospfv2_lsa / ospfv2_router_lsa
ipv4 / ospfv2 / ospfv2_lsu / ospfv2_opaque_lsa / ospfv2_ri_lsa   # LS type 9 / 10 / 11
ipv6 / ospfv3 / ospfv3_lsu / ospfv3_lsa / ospfv3_intra_area_prefix_lsa
ipv4 / tcp / bgp / bgp_update
ipv4 / tcp / bgp                 # a KEEPALIVE is the header alone (type 4 is the default)
```

## Auto fields

- OSPF header `type`, `packet_length` and `checksum`. The OSPFv3 checksum includes the IPv6 pseudo header and is exact.
- LSA header `ls_type`, `length` and `ls_checksum` (Fletcher, LS age excluded; exact for any age).
- Router-LSA `num_links`.
- BGP header `length` and `type`; OPEN `opt_params_length`; UPDATE `withdrawn_routes_length` and `total_path_attribute_length`; the Extended Length flag of path attributes longer than 255 octets.
- EIGRP `checksum`.

## Fields you set by hand

- `num_lsas` of `ospfv2_lsu` / `ospfv3_lsu` (normally 1: an LSU carries one typed LSA).
- `ls_checksum` of the LSA headers listed inside DBD / LSAck.

## Limits and approximations

- **OSPFv2 authentication**: the checksum leaves out the 64-bit authentication field, so it is exact for null and simple-password authentication. With cryptographic authentication (AuType 2) set the checksum to 0 by hand (in the builder, turn on “Allow irregular” to override it). The cryptographic-auth trailer after the packet is not modelled.
- **One typed LSA per LSU**: there is no repeated-layer construct, so no further LSA can follow a typed LSA body.
- **LS types sharing a body**: OSPFv2 types 3 / 4 and 5 / 7, and OSPFv3 `0x4005` / `0x2007`, have the same body layout, so Auto announces the first type. For the other one set `ls_type` by hand (a next-protocol mismatch warning results, but the packet can be sent).
- OSPFv2 TOS entries (router-link `num_tos` > 0, summary / external TOS metrics) and the OSPFv3 prefix lists are not typed; they stay opaque `prefixes` / `address_prefix` octets.
- **BGP routes (NLRI / withdrawn routes)** are lists of `{length, prefix}` (length in bits, `ceil(length / 8)` prefix octets). A hex string is still accepted as verbatim octets.
- **BGP path attributes** are a TLV with a 16-bit type (flags << 8 | type code); the length is 8 bits, or 16 bits with the Extended Length flag (`0x10`), which is set automatically above 255 octets and kept when given in an explicit `type`. `AS_PATH` / `AGGREGATOR` use 4-octet AS numbers (RFC 6793). Extended optional parameters (RFC 9072) are not supported.
- Only the first BGP message in a segment is decoded.
- **EIGRP** route TLVs end in a variable-length destination (`ceil(prefix_length / 8)` octets), kept as `destination` octets running to the end of the TLV value; a multi-destination TLV stays in that one field.

<!-- wpg-docs-gen:begin reference-links -->
## Generated reference

Field reference of the protocols on this page (generated from the definitions):

[`ospfv2`](../reference/protocols/ospfv2.md) · [`ospfv2_lsu`](../reference/protocols/ospfv2_lsu.md) · [`ospfv3`](../reference/protocols/ospfv3.md) · [`ospfv3_lsu`](../reference/protocols/ospfv3_lsu.md) · [`bgp`](../reference/protocols/bgp.md) · [`rip`](../reference/protocols/rip.md) · [`ripng`](../reference/protocols/ripng.md) · [`eigrp`](../reference/protocols/eigrp.md)
<!-- wpg-docs-gen:end reference-links -->
