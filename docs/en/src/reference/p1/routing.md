# Routing protocols

Status: draft

Notes on OSPFv2 / OSPFv3, BGP-4, RIPv1 / v2, RIPng and EIGRP (basic). The definitions are in `crates/wpg-protocols/src/routing.rs`.

## Identification and stacking

| Protocol ID | Lower layer | Identified by | Example destination |
|---|---|---|---|
| `ospfv2` | `ipv4` | protocol 89 | 224.0.0.5 (AllSPFRouters), 224.0.0.6 (AllDRouters) |
| `ospfv3` | `ipv6` | next header 89 | `ff02::5` / `ff02::6` |
| `bgp` | `tcp` | port 179 (destination, else source) | the peer's unicast address |
| `rip` | `udp` | port 520 | 224.0.0.9 (v2) / broadcast (v1) |
| `ripng` | `udp` | port 521 | `ff02::9` |
| `eigrp` | `ipv4` / `ipv6` | protocol 88 | 224.0.0.10 / `ff02::a` |

An OSPF or BGP message is two layers, a common header and a body. The header's `type` is Auto from the body layer.

| Common header | Body layers |
|---|---|
| `ospfv2` | `ospfv2_hello` / `ospfv2_dbd` / `ospfv2_lsr` / `ospfv2_lsu` / `ospfv2_lsack` |
| `ospfv3` | `ospfv3_hello` / `ospfv3_dbd` / `ospfv3_lsr` / `ospfv3_lsu` / `ospfv3_lsack` |
| `bgp` | `bgp_open` / `bgp_update` / `bgp_notification` / `bgp_route_refresh` (no body = KEEPALIVE) |

An LSU further stacks an LSA header (`ospfv2_lsa` / `ospfv3_lsa`) and an LSA body:

```text
ethernet2 / ipv4 / ospfv2 / ospfv2_lsu / ospfv2_lsa / ospfv2_router_lsa
```

LSA bodies: OSPFv2 `ospfv2_router_lsa` (1), `ospfv2_network_lsa` (2), `ospfv2_summary_lsa` (3 / 4), `ospfv2_external_lsa` (5 / 7). OSPFv3 `ospfv3_router_lsa` (`0x2001`), `ospfv3_network_lsa` (`0x2002`), `ospfv3_inter_area_prefix_lsa` (`0x2003`), `ospfv3_as_external_lsa` (`0x4005` / `0x2007`), `ospfv3_link_lsa` (`0x0008`), `ospfv3_intra_area_prefix_lsa` (`0x2009`).

## Auto fields

| Protocol | Auto |
|---|---|
| `ospfv2` | `type`, `packet_length`, `checksum` (Internet checksum without the authentication field) |
| `ospfv3` | `type`, `packet_length`, `checksum` (with the IPv6 pseudo header) |
| `ospfv2_lsa` / `ospfv3_lsa` | `ls_type`, `length`, `ls_checksum` (Fletcher; LS age is left out, so it is exact for any age) |
| `ospfv2_router_lsa` | `num_links` |
| `bgp` | `length`, `type` |
| `bgp_open` | `opt_params_length` |
| `eigrp` | `checksum` |

## Limitations and workarounds

- **OSPFv2 cryptographic authentication** (auth type 2): RFC 2328 says the checksum is not calculated (left 0), but Auto does not distinguish this case. Set `checksum: 0` manually. The authentication trailer after the packet is not modelled either. Null and simple-password authentication are exact.
- **An LSU carries one typed LSA.** There is no repeated-layer construct, so no further LSA can follow a typed LSA body. `num_lsas` is Manual (default 1).
- LS types sharing a body layout (OSPFv2 3 / 4 and 5 / 7, OSPFv3 `0x4005` / `0x2007`) announce the first type on Auto. Set `ls_type` manually for the other one (a next-protocol mismatch warning results).
- LSA headers inside DBD / LSAck carry Manual checksums.
- OSPFv2 TOS entries (router-link `num_tos` > 0, summary / external TOS metrics) are not modelled. OSPFv3 prefix lists (variable, 4-octet-aligned address prefixes) are opaque `prefixes` / `address_prefix` octets, and `num_prefixes` of the Link-LSA / Intra-Area-Prefix-LSA is Manual.
- **BGP UPDATE**'s `withdrawn_routes_length` and `total_path_attribute_length` are Manual (two lengths bracket variable regions). The defaults match the default field values; recompute the total when you change the attributes. Withdrawn routes and NLRI are `(prefix length, prefix)` octets (e.g. `18 c6 33 64` = 198.51.100.0/24).
- **BGP path attributes** are a TLV with a 16-bit type (attribute flags << 8 | type code) and an 8-bit length. Extended-length attributes (flag `0x10`) must be written as `raw` items and do not parse back. `AS_PATH` / `AGGREGATOR` use 4-octet AS numbers (RFC 6793). Extended optional parameters (RFC 9072) are not modelled.
- **BGP**: only one message per TCP segment is decoded.
- **EIGRP** route TLVs end with a variable `ceil(prefix_length / 8)`-octet destination, kept as `destination` octets running to the end of the TLV value; multi-destination TLVs stay in that one field.
- **RIP** authentication entries (`afi: 0xffff`) are written through the same 20-octet fields as route entries. A RIPng next-hop RTE is an entry with `metric: 0xff`.

## Samples

| Template | Contents |
|---|---|
| `templates/protocols/routing/ospfv2-hello.wpg.yaml` and the other `ospfv2-*.wpg.yaml` | Hello / DBD / LSR / LSAck, LSU (Router / Network / Summary / External LSA) |
| `templates/protocols/routing/ospfv3-hello.wpg.yaml` and the other `ospfv3-*.wpg.yaml` | Hello / DBD / LSR / LSAck, LSU (Router / Network / Inter-Area-Prefix / AS-External / Link / Intra-Area-Prefix LSA) |
| `templates/protocols/routing/bgp-open.wpg.yaml` | an OPEN with Multiprotocol, Route Refresh and 4-octet AS capabilities |
| `templates/protocols/routing/bgp-update.wpg.yaml` | announces 198.51.100.0/24 with ORIGIN / AS_PATH / NEXT_HOP / MED / COMMUNITIES |
| `templates/protocols/routing/bgp-keepalive.wpg.yaml` / `bgp-notification.wpg.yaml` / `bgp-route-refresh.wpg.yaml` | KEEPALIVE / NOTIFICATION / ROUTE-REFRESH |
| `templates/protocols/routing/ripv1-request.wpg.yaml` / `ripv2-response.wpg.yaml` / `ripng-response.wpg.yaml` | RIPv1 Request / RIPv2 Response / RIPng Response |
| `templates/protocols/routing/eigrp-hello.wpg.yaml` / `eigrp-update.wpg.yaml` | Hello / Update with an IPv4 internal route |
