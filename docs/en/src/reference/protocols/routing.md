# Routing protocols (P1)

Status: draft

Definitions: `crates/wpg-protocols/src/routing.rs` / tests: `crates/wpg-protocols/tests/p1_routing.rs` / samples: `templates/protocols/routing/`

## Protocols and how they are reached

| id | Protocol | Reached from below | Sample |
|---|---|---|---|
| `ospfv2` + bodies | OSPFv2 (RFC 2328) | IPv4 protocol 89 | `ospfv2-hello` / `-dbd` / `-lsr` / `-lsu-*` / `-lsack` |
| `ospfv3` + bodies | OSPFv3 (RFC 5340) | IPv6 next header 89 | `ospfv3-hello` / `-dbd` / `-lsr` / `-lsu-*` / `-lsack` |
| `bgp` + bodies | BGP-4 (RFC 4271) | TCP 179 (destination, else source) | `bgp-open` / `-update` / `-keepalive` / `-notification` / `-route-refresh` |
| `rip` | RIPv1 / RIPv2 | UDP 520 | `ripv1-request` / `ripv2-response` |
| `ripng` | RIPng (RFC 2080) | UDP 521 | `ripng-response` |
| `eigrp` | EIGRP (RFC 7868, basic TLVs) | IP protocol 88 (IPv4 / IPv6) | `eigrp-hello` / `eigrp-update` |

## Stacking

OSPF and BGP messages are two layers: a common header and a body. The header's `type` is Auto from the body layer.

| Common header | Body layers |
|---|---|
| `ospfv2` | `ospfv2_hello` / `ospfv2_dbd` / `ospfv2_lsr` / `ospfv2_lsu` / `ospfv2_lsack` |
| `ospfv3` | `ospfv3_hello` / `ospfv3_dbd` / `ospfv3_lsr` / `ospfv3_lsu` / `ospfv3_lsack` |
| `bgp` | `bgp_open` / `bgp_update` / `bgp_notification` / `bgp_route_refresh` (a KEEPALIVE is `bgp` alone, default type 4) |

An **LSU** stacks an LSA header and an LSA body on top:

- OSPFv2: `ospfv2_lsu` → `ospfv2_lsa` → `ospfv2_router_lsa` / `ospfv2_network_lsa` / `ospfv2_summary_lsa` / `ospfv2_external_lsa`
- OSPFv3: `ospfv3_lsu` → `ospfv3_lsa` → `ospfv3_router_lsa` / `ospfv3_network_lsa` / `ospfv3_inter_area_prefix_lsa` / `ospfv3_as_external_lsa` / `ospfv3_link_lsa` / `ospfv3_intra_area_prefix_lsa`

OSPF is usually sent to `224.0.0.5` (AllSPFRouters, IPv4 TTL 1) or `ff02::5`.

## Auto fields

| Field | Value |
|---|---|
| `ospfv2.type` / `ospfv3.type` / `bgp.type` | the body layer |
| `ospfv2.packet_length` / `ospfv3.packet_length` / `bgp.length` | length of common header + body |
| `ospfv2.checksum` | Internet checksum (without the 64-bit `authentication`) |
| `ospfv3.checksum` | Internet checksum with the IPv6 pseudo header |
| `ospfv2_lsa.ls_type` / `ospfv3_lsa.ls_type` | the LSA body layer |
| `ospfv2_lsa.ls_checksum` / `ospfv3_lsa.ls_checksum` | Fletcher checksum (without `ls_age`; exact for any LS age) |
| `ospfv2_lsa.length` / `ospfv3_lsa.length` | length of LSA header + body |
| `ospfv2_router_lsa.num_links` | number of `links` entries |
| `bgp_open.opt_params_length` | length of the optional parameters |
| `eigrp.checksum` | Internet checksum over the whole EIGRP packet |

## Constraints and approximations

- **OSPFv2 cryptographic authentication**: the checksum leaves out `authentication`, so it is exact for null and simple-password authentication. With cryptographic authentication (`auth_type` 2) the checksum is not computed by the standard; set `checksum: 0` manually. The cryptographic-authentication trailer after the packet is not modelled.
- **One LSA per LSU**: there is no repeated-layer construct, so an LSU carries a single typed LSA. `num_lsas` is Manual (default 1).
- **LS types sharing a body**: OSPFv2 types 3 / 4 and 5 / 7, and OSPFv3 `0x4005` / `0x2007`, share a body layer; Auto gives the first type (3, 5, `0x4005`). Set `ls_type` manually for the other one (a next-protocol mismatch warning results).
- **LSA headers inside DBD / LSAck**: `ls_checksum` / `length` are Manual.
- **Unsupported OSPF parts**: OSPFv2 TOS entries (keep the router-LSA `num_tos` at 0; summary / external TOS metrics) are not modelled. OSPFv3 prefix lists (variable, 4-octet-aligned address prefixes) are opaque `prefixes` / `address_prefix` octets and `num_prefixes` is Manual.
- **The two BGP UPDATE lengths**: `withdrawn_routes_length` and `total_path_attribute_length` are Manual (their defaults match the default contents). Withdrawn routes and NLRI are opaque `(length, prefix)` octets.
- **BGP path attributes**: an attribute is a TLV with a 16-bit type (flags << 8 | type code) and an 8-bit length. Extended Length attributes (flag `0x10`) must be written as `raw` items and do not parse back. `AS_PATH` / `AGGREGATOR` use 4-octet AS numbers (RFC 6793). Extended optional parameters (RFC 9072) are not modelled.
- **BGP parsing**: only one message per segment is decoded.
- **EIGRP route TLVs**: the trailing destination (`ceil(prefix_length / 8)` octets) is a `destination` octet string running to the end of the TLV value. Multi-destination TLVs keep all destinations in that one field.
