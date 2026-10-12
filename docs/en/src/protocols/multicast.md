# Multicast control protocols

Status: draft

Notes on IGMPv1, v2, v3 / MLDv1 and v2 / PIM-SM v2. Definitions are in `crates/wpg-protocols/src/mcast.rs`, samples in `templates/protocols/mcast/`.

## How they are reached, and samples

| id | Protocol | Layer below | Sample |
|---|---|---|---|
| `igmp` + `igmpv2` | IGMPv1 / v2 (query, report, leave) | IPv4 protocol 2 | `igmpv2-query.wpg.yaml` |
| `igmp` + `igmpv3_query` | IGMPv3 query | IPv4 protocol 2, type `0x11` with at least 8 octets left | `igmpv3-query.wpg.yaml` |
| `igmp` + `igmpv3_report` | IGMPv3 report | IPv4 protocol 2, type `0x22` | `igmpv3-report.wpg.yaml` |
| `icmpv6` + `mld` | MLDv1 (query, report, done) | ICMPv6 type 130 / 131 / 132 | `mldv1-query.wpg.yaml` |
| `icmpv6` + `mldv2_query` | MLDv2 query | ICMPv6 type 130 with at least 24 octets left | `mldv2-query.wpg.yaml` |
| `icmpv6` + `mldv2_report` | MLDv2 report | ICMPv6 type 143 | `mldv2-report.wpg.yaml` |
| `pim` + `pim_hello` / `pim_join_prune` | PIM-SM v2 | IPv4 protocol 103 | `pim-hello.wpg.yaml`, `pim-join-prune.wpg.yaml` |
| `pim_register` | PIM Register (standalone) | chosen when building (decodes as `pim`) | `pim-register.wpg.yaml` |

Like ICMPv6, IGMP and PIM are two layers, a common header (`type`, checksum) and a body; the header `type` is Auto from the body. MLD bodies stack on `icmpv6`.

```text
ipv4 / igmp / igmpv3_report
ipv6 / ipv6_hopopts / icmpv6 / mldv2_report
ipv4 / pim / pim_join_prune
ipv4 / pim_register / ipv4 / udp / payload
```

## Auto fields

- IGMP / PIM header `type` and `checksum`. The MLD checksum is computed by `icmpv6`, including the IPv6 pseudo header.
- Counts: `num_sources` of IGMPv3 / MLDv2 queries, `num_group_records` / `num_records` of reports, `num_sources` inside a group record, and PIM Join/Prune `num_groups` / `num_joined` / `num_pruned`. Adding or removing list elements keeps them right.
- `pim_register.checksum`: covers only the 8 octets of PIM header + Register flags (RFC 7761).

## Limits and approximations

- **v2 and v3 queries** differ only in length: with at least 8 (IGMPv3) or 24 (MLDv2) octets left, a query decodes as the v3 / v2 query body, otherwise as `igmpv2` / `mld`.
- **PIM Register**'s checksum does not cover the encapsulated packet, so `pim_register` is a standalone definition that repeats the PIM header. IP protocol 103 decodes as `pim`, so the body of a received Register decodes as `raw`. The inner packet is IPv4 or IPv6 by its first nibble.
- **PIM over IPv6** needs the IPv6 pseudo header in its checksum, but a checksum kind cannot depend on the lower layer, so `pim` is reached from IPv4 only and uses the plain Internet checksum.
- PIM encoded addresses are typed for IPv4 (address family 1) only.
- IGMP / MLD need the Router Alert option; the samples put it in the `ipv4` `options` / the `ipv6_hopopts` layer.

<!-- wpg-docs-gen:begin reference-links -->
## Generated reference

Field reference of the protocols on this page (generated from the definitions):

[`ipv4`](../reference/protocols/ipv4.md) · [`ipv6_hopopts`](../reference/protocols/ipv6_hopopts.md) · [`icmpv6`](../reference/protocols/icmpv6.md) · [`igmp`](../reference/protocols/igmp.md) · [`igmpv2`](../reference/protocols/igmpv2.md) · [`igmpv3_query`](../reference/protocols/igmpv3_query.md) · [`igmpv3_report`](../reference/protocols/igmpv3_report.md) · [`mld`](../reference/protocols/mld.md) · [`mldv2_query`](../reference/protocols/mldv2_query.md) · [`mldv2_report`](../reference/protocols/mldv2_report.md) · [`pim`](../reference/protocols/pim.md) · [`pim_hello`](../reference/protocols/pim_hello.md) · [`pim_join_prune`](../reference/protocols/pim_join_prune.md) · [`pim_register`](../reference/protocols/pim_register.md) · [`raw`](../reference/protocols/raw.md)
<!-- wpg-docs-gen:end reference-links -->
