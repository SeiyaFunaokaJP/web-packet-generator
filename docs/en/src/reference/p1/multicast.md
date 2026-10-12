# Multicast control

Status: draft

Notes on IGMPv1 / v2 / v3, MLDv1 / v2 and PIM-SM v2 (Hello / Join-Prune / Register). The definitions are in `crates/wpg-protocols/src/mcast.rs`.

## Identification and stacking

Like ICMPv6, a message is two layers: a common header (`type`, checksum) and one body per message layout. The header's `type` is Auto from the body layer.

| Common header | Lower layer | Body layers |
|---|---|---|
| `igmp` | `ipv4` (protocol 2) | `igmpv2` (v1 / v2 query, report, leave), `igmpv3_query`, `igmpv3_report` |
| `icmpv6` | `ipv6` (next header 58) | `mld` (130 / 131 / 132), `mldv2_query` (130), `mldv2_report` (143) |
| `pim` | `ipv4` (protocol 103) | `pim_hello`, `pim_join_prune` |
| `pim_register` (standalone) | `ipv4` (protocol 103) | the encapsulated multicast packet (`ipv4` / `ipv6`) |

```text
ethernet2 / ipv4 / igmp / igmpv3_report
ethernet2 / ipv6 / ipv6_hopopts / icmpv6 / mldv2_report
ethernet2 / ipv4 / pim_register / ipv4 / udp / payload
```

IGMP / MLD normally carry the Router Alert option (IPv4 option / IPv6 Hop-by-Hop); see the samples.

## Auto fields

| Protocol | Auto |
|---|---|
| `igmp` / `pim` | `type` (from the body layer), `checksum` (whole message) |
| `pim_register` | `checksum` (first 8 octets only) |
| `igmpv3_query` / `mldv2_query` | `num_sources` |
| `igmpv3_report` | `num_group_records`, `num_sources` in each group record |
| `mldv2_report` | `num_records`, the source count in each record |
| `pim_join_prune` | `num_groups`, `num_joined` / `num_pruned` per group |

## Limitations and workarounds

- On a common header, `igmpv2` / `mld` make `type` Auto to a query (0x11 / 130). For a v1 report (0x12), v2 report (0x16), leave (0x17), MLD report (131) or done (132) set `type` manually.
- v2 and v3 queries differ only in length; parsing picks the v3 body by the remaining length (8 octets or more for IGMPv3, 24 or more for MLDv2).
- **PIM Register**'s checksum covers only the first 8 octets (PIM header + Register flags), not the encapsulated packet. To express that coverage, `pim_register` is a standalone definition that includes the PIM header and does not stack on `pim`. When parsing, IP protocol 103 decodes as `pim` and the Register body as `raw`. Set `type: 1` on `pim_register` explicitly.
- **PIM over IPv6** needs the IPv6 pseudo header in its checksum, but a checksum kind cannot switch on the lower layer, so `pim` decodes from IPv4 only. Set the checksum manually for IPv6.
- PIM encoded addresses are typed for IPv4 (address family 1) only.

## Samples

| Template | Contents |
|---|---|
| `templates/protocols/mcast/igmpv2-query.wpg.yaml` | an IGMPv2 general query with Router Alert |
| `templates/protocols/mcast/igmpv3-query.wpg.yaml` / `igmpv3-report.wpg.yaml` | IGMPv3 group-and-source-specific query / membership report |
| `templates/protocols/mcast/mldv1-query.wpg.yaml` | an MLDv1 general query with a Hop-by-Hop Router Alert |
| `templates/protocols/mcast/mldv2-query.wpg.yaml` / `mldv2-report.wpg.yaml` | MLDv2 address-and-source-specific query / report |
| `templates/protocols/mcast/pim-hello.wpg.yaml` | a PIM Hello to 224.0.0.13 |
| `templates/protocols/mcast/pim-join-prune.wpg.yaml` | an (S,G) Join/Prune |
| `templates/protocols/mcast/pim-register.wpg.yaml` | a Register from the DR to the RP (encapsulating multicast UDP) |
