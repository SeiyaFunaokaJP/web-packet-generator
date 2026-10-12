# SRv6 SRH / SR-MPLS (P2)

Status: draft

Building blocks for the SRv6 Segment Routing Header (RFC 8754) and SR-MPLS (RFC 8660) (M7). The definitions are in `crates/wpg-protocols/src/ipv6.rs` (`ipv6_srh`) and `encap.rs` (`mpls`); samples are in `templates/protocols/srv6/`, `templates/protocols/mpls/sr-mpls-entropy` and `templates/scenarios/sr-mpls-prefix-sid-sweep`.

## Layer stacks and samples

```text
ethernet2 / ipv6 / ipv6_srh / udp|tcp|icmpv6   # a plain packet with an SRH
ethernet2 / ipv6 / ipv6_srh / ipv4 / ...       # H.Encaps → End.DT4 / End.DX4 (next header 4)
ethernet2 / ipv6 / ipv6_srh / ipv6 / ...       # H.Encaps → End.DT6 / End.DX6 (next header 41)
ethernet2 / ipv6 / ipv6_srh / ethernet2 / ...  # H.Encaps.L2 → End.DX2 (next header 143, RFC 8986)
ethernet2 / mpls / mpls / ... / ipv4|ipv6      # SR-MPLS label stack
```

An IPv6 Routing header (next header 43) decodes as `ipv6_srh` when its routing type (third octet of the header) is 4 and as the generic `ipv6_route` otherwise. IPv6 next header 143 (Ethernet) maps to `ethernet2`.

| Sample | Contents |
|---|---|
| `srv6-srh-udp` | IPv6/UDP with a three-segment SRH (two left); the UDP checksum is computed toward the last segment |
| `srv6-h-encaps-ipv4` | IPv4/UDP encapsulated in outer IPv6 + SRH (End then End.DT4) for an L3VPN |
| `srv6-end-dx2-hmac` | An Ethernet frame (ARP) over an SRH with an HMAC TLV toward End.DX2 (L2VPN) |
| `sr-mpls-entropy` | Prefix SID (SRGB + index as variables), adjacency SID, ELI + entropy label |
| `sr-mpls-prefix-sid-sweep` (scenario) | Sweeps the prefix SID index over 1-100 with a per-packet entropy label |

## SRH fields

| Field | Auto / Manual | Notes |
|---|---|---|
| `next_header` | Auto | From the next layer |
| `hdr_ext_len` | Auto | Length in 8-octet units, not counting the first 8 octets |
| `routing_type` | default 4 | |
| `segments_left` | Manual (default 1) | Index of the next segment to visit |
| `last_entry` | Auto | Number of segments - 1 |
| `flags` / `tag` | Manual | |
| `segment_list` | Manual | List of IPv6 addresses in **reverse path order**: element 0 is the last segment |
| `tlvs` | Manual | Pad1 / PadN / HMAC TLVs; the trailing Pad1 padding is Auto |

Write `segment_list` in reverse path order. The IPv6 destination of a packet leaving the source is `segment_list[segments_left]` (the active segment); set `ipv6.dst` to it by hand (the IPv6 destination is not Auto).

### Destination of upper-layer checksums

While `segments_left` is non-zero, the pseudo header of UDP / TCP / ICMPv6 checksums uses the final destination `segment_list[0]` instead of `ipv6.dst` (RFC 8200 §8.1). wpg handles this automatically, including when a generator changes `segments_left` (fast patch path). With `segments_left` = 0 it uses `ipv6.dst`. The generic `ipv6_route` (types 0 / 2 / 3) keeps its addresses as `data` octets and does not do this replacement.

### TLVs

| TLV | Type | Value |
|---|---|---|
| `pad1` | 0 | none (one octet) |
| `padn` | 4 | `padding` (any length) |
| `hmac` | 5 | `d` (1 bit), `reserved` (15 bits), `hmac_key_id` (32 bits), `hmac` (octets, Manual) |

After the TLVs, the header is filled with zero octets (= Pad1) up to an 8-octet boundary automatically. The HMAC is not computed (compute it externally for your key and coverage and paste it in).

## SR-MPLS helpers

SR-MPLS SIDs are ordinary MPLS labels, so a stack is built from `mpls` layers.

- **Reserved label names**: `label` accepts names for the reserved labels 0-15: `ipv4_explicit_null` (0), `router_alert` (1), `ipv6_explicit_null` (2), `implicit_null` (3), `entropy_label_indicator` (7), `gal` (13), `oam_alert` (14), `extension` (15). Other labels are numbers.
- **Prefix SIDs**: label = SRGB base + SID index. With the `expr` generator and variables, `"${srgb_base} + ${sid_index}"` follows the routers' SRGB by changing only the variables (see `sr-mpls-entropy`). Add `idx` to reach many egress nodes in turn (see `sr-mpls-prefix-sid-sweep`).
- **Entropy labels** (RFC 6790): put the entropy label right after an `entropy_label_indicator` (7) label, with TTL 0.
- `s` (bottom of stack) is Auto.

## Limitations / out of scope

- The consistency of `segments_left` and `last_entry` (`segments_left <= last_entry`) is not checked.
- `ipv6.dst` is not set to the active segment automatically.
- No HMAC computation and no simulation of SRv6 behaviors (End / End.X / End.DT4 ...): a SID's behavior is only its address value.
- No dedicated representation of compressed SIDs (uSID / CSID, RFC 9800); write them as 128-bit addresses.
- SRGB / SRLB ranges are not checked.
