# Multicast protocols (P1)

Status: draft

Definitions: `crates/wpg-protocols/src/mcast.rs` / tests: `crates/wpg-protocols/tests/p1_fhrp_mcast.rs` / samples: `templates/protocols/mcast/`

## Protocols and how they are reached

| Common header | Body layers | Reached from below | Sample |
|---|---|---|---|
| `igmp` | `igmpv2` (v1/v2 query, report, leave) / `igmpv3_query` / `igmpv3_report` | IPv4 protocol 2 | `igmpv2-query` / `igmpv3-query` / `igmpv3-report` |
| `icmpv6` | `mld` (MLDv1) / `mldv2_query` / `mldv2_report` | ICMPv6 types 130 / 131 / 132 / 143 | `mldv1-query` / `mldv2-query` / `mldv2-report` |
| `pim` | `pim_hello` / `pim_join_prune` | IPv4 protocol 103 | `pim-hello` / `pim-join-prune` |
| (standalone) | `pim_register` | IPv4 protocol 103 (for building) | `pim-register` |

## Stacking

- **IGMP**: `ipv4` → `igmp` → body. `igmp.type` is Auto from the body (0x11 = query for `igmpv2`). For a v1 report, v2 report or leave set `type` to 0x12 / 0x16 / 0x17 manually. IGMP is normally sent with the IPv4 Router Alert option (see the samples).
- **MLD**: `ipv6` → `ipv6_hopopts` (Router Alert) → `icmpv6` → body. `icmpv6.type` is Auto from the body (130 for `mld`); set 131 / 132 manually for a report / done.
- **PIM Register**: `ipv4` → `pim_register` → the encapsulated multicast packet (`ipv4` / `ipv6` ...). It does not stack on `pim`.

## How parsing tells them apart

- v2 and v3 queries (IGMP type 0x11, MLD type 130) differ only in length. With at least 8 remaining octets for IGMPv3 or 24 for MLDv2 they decode as `igmpv3_query` / `mldv2_query`, otherwise as `igmpv2` / `mld`.
- IP protocol 103 decodes as `pim`; a Register body then parses as `raw`.
- The packet inside a PIM Register is IPv4 or IPv6 by its first nibble.

## Auto fields

| Field | Value |
|---|---|
| `igmp.type` / `pim.type` | the body layer |
| `igmp.checksum` / `pim.checksum` | Internet checksum over the whole message |
| `pim_register.checksum` | checksum over the first 8 octets of the Register only (PIM header + flags) |
| `igmpv3_query.num_sources` / `mldv2_query.num_sources` | number of source addresses |
| `igmpv3_report.num_group_records` / `mldv2_report.num_records` | number of records (`num_sources` inside each record is Auto too) |
| `pim_join_prune.num_groups` | number of group sets (`num_joined` / `num_pruned` inside each set are Auto too) |

## Constraints and approximations

- **PIM Register is a standalone definition**: the Register checksum covers only the first 8 octets, not the encapsulated packet. There is no checksum coverage of "the next layer's header too", so the Register repeats the PIM header in a definition of its own.
- **PIM over IPv4 only**: PIM over IPv6 includes the IPv6 pseudo header in its checksum, and a checksum kind cannot depend on the lower layer, so `pim` is reached from IPv4 only. Encoded addresses are typed for IPv4 (address family 1) only.
- **Time units**: IGMP `max_resp_code` is in 1/10 s (reserved in v1 and in v3 reports), the MLDv1 maximum response delay in milliseconds, PIM holdtimes in seconds.
