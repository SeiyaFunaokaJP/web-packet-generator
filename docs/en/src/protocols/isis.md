# IS-IS (P2)

Status: draft

Notes on IS-IS (ISO/IEC 10589, RFC 1195 and the TLV RFCs below) (M7). The definitions are in `crates/wpg-protocols/src/isis.rs`, the samples in `templates/protocols/routing/isis-*.wpg.yaml`. IS-IS runs directly over the [OSI encapsulation](osi.md).

## PDUs, how they are reached, and samples

| id | PDU (type) | Samples |
|---|---|---|
| `isis_l1_lan_hello` / `isis_l2_lan_hello` | LAN Hello L1 (15) / L2 (16) | `isis-l1-lan-hello` (padded), `isis-l2-lan-hello` |
| `isis_p2p_hello` | Point-to-point Hello (17) | `isis-p2p-hello` (three-way adjacency, padded) |
| `isis_l1_lsp` / `isis_l2_lsp` | LSP L1 (18) / L2 (20) | `isis-l1-lsp` (narrow metrics), `isis-l2-lsp` (wide metrics, TE, SR) |
| `isis_l1_csnp` / `isis_l2_csnp` | CSNP L1 (24) / L2 (25) | `isis-l1-csnp`, `isis-l2-csnp` |
| `isis_l1_psnp` / `isis_l2_psnp` | PSNP L1 (26) / L2 (27) | `isis-l1-psnp`, `isis-l2-psnp` |
| `isis` | Any PDU: common header + opaque octets | `isis-generic` |

Each PDU is a single layer: the 8-octet common header, the PDU's fixed fields and the TLVs.

```text
dot3 / llc / isis_l1_lan_hello      # to 01:80:c2:00:00:14 (AllL1ISs); L2: ...:15
dot3 / llc / isis_p2p_hello         # to 09:00:2b:00:00:05 (AllISs) on Ethernet
ppp / isis_l2_lsp                   # PPP protocol 0x0023
```

The LLC DSAP / SSAP (`0xfe`) and the PPP protocol (`0x0023`) are Auto. When parsing, the NLPID `0x83` selects the generic `isis`, whose `pdu_type` then re-decodes the layer as the dedicated PDU; unknown PDU types stay `isis`.

## Auto fields

| Field | Rule |
|---|---|
| `pdu_length` | Length of the whole PDU |
| LSP `checksum` | ISO 8473 Fletcher checksum from the LSP ID to the end of the PDU. The fields before the LSP ID (common header, PDU length, remaining lifetime) are excluded, so it stays correct for any lifetime |
| Hello padding | When `pdu_length` of a Hello is Manual, Padding TLVs (type 8, up to 255 octets each) are appended until the PDU is exactly that long, e.g. `pdu_length: 1497` for an Ethernet MTU of 1500 (minus the 3-octet LLC header). On Auto no padding is added. If the missing size cannot be made of TLVs (one octet), a warning is shown |
| `sub_tlvs_length` (extended IS / IP / IPv6 reachability) | Size of the `sub_tlvs` that follow |
| `sub_tlvs_present` (S bit, TLV 135 / 236) | 1 when the entry has a `sub_tlv_block`, else 0 |

`header_length` is a constant per PDU type (27 / 20 / 33 / 17) and has a default, not an Auto rule; a warning appears when it is changed.

## TLVs

| Code | Name (`name`) | Value fields |
|---|---|---|
| 1 | `area_addresses` | `addresses`: list of `{address}` (NSAP, e.g. `49.0001`) |
| 2 | `is_reachability` | `virtual_flag`, `neighbors`: `{default_metric, delay_metric, expense_metric, error_metric, neighbor_id}` |
| 6 | `is_neighbors` | `neighbors`: MAC list |
| 8 | `padding` | `data` |
| 9 | `lsp_entries` | `entries`: `{remaining_lifetime, lsp_id, sequence_number, checksum}` |
| 10 | `authentication` | `auth_type` (`cleartext` / `hmac_md5`), `value` |
| 22 | `extended_is_reachability` | `neighbors`: `{neighbor_id, metric, sub_tlvs_length, sub_tlvs}` |
| 128 / 130 | `ip_internal_reachability` / `ip_external_reachability` | `prefixes`: `{4 metrics, ip_address, subnet_mask}` |
| 129 | `protocols_supported` | `nlpids` (`ipv4`, `ipv6`, `clnp`) |
| 132 | `ip_interface_addresses` | `addresses` |
| 134 | `te_router_id` | `router_id` |
| 135 | `extended_ip_reachability` | `prefixes`: `{metric, up_down, sub_tlvs_present, prefix_length, prefix, sub_tlv_block}` |
| 137 | `dynamic_hostname` | `hostname` |
| 232 | `ipv6_interface_addresses` | `addresses` |
| 236 | `ipv6_reachability` | `prefixes`: `{metric, up_down, external, sub_tlvs_present, prefix_length, prefix, sub_tlv_block}` |
| 240 | `p2p_adjacency_state` | `adjacency_state`, `extended_local_circuit_id`, `neighbor_system_id`, `neighbor_extended_local_circuit_id` |
| 242 | `router_capability` | `router_id`, `flags` (S / D), `sub_tlvs` |

Sub-TLVs:

- Extended IS reachability (22): administrative group (3), link local / remote IDs (4), IPv4 interface / neighbor address (6 / 8), maximum / reservable / unreserved bandwidth (9 / 10 / 11), TE default metric (18), Adj-SID / LAN-Adj-SID (31 / 32).
- Extended IP / IPv6 reachability (135 / 236): 32 / 64-bit administrative tags (1 / 2), Prefix-SID (3), prefix attribute flags (4).
- Router capability (242): SR capabilities (2), SR algorithms (19).

`prefix` holds `ceil(prefix_length / 8)` octets; give the significant octets only (`192.0.2.0/24` is `prefix_length: 24, prefix: "c00002"`). The optional sub-TLVs of a prefix are written as a one-element `sub_tlv_block` list:

```yaml
- extended_ip_reachability:
    prefixes:
      - metric: 0
        prefix_length: 32
        prefix: "c0000201"
        sub_tlv_block:
          - sub_tlvs:
              - { prefix_sid: { flags: 0x40, algorithm: 0, sid: "00000001" } }
```

Other TLVs are kept as `{ type: N, data: "..." }`.

## Limits

- System IDs are 6 octets (`id_length` 0); other ID lengths are not modelled.
- TE bandwidths are the raw 32-bit IEEE floating point values. SIDs / labels and the SR capability ranges are octets (a 3-octet label or a 4-octet index).
- The HMAC-MD5 authentication value (and the cryptographic authentication of RFC 5310) is not computed.
- TLV 240 is modelled in its 15-octet form; the shorter forms (1 and 5 octets) read back as `data`.
