# Encapsulation protocols (P1)

Status: draft

Definitions: `crates/wpg-protocols/src/encap.rs` / tests: `crates/wpg-protocols/tests/p1_encap.rs` / samples: `templates/protocols/encap/`

## Protocols and how they are reached

| id | Protocol | Reached from below | Above (payload) | Sample |
|---|---|---|---|---|
| `vxlan` | VXLAN (RFC 7348) | UDP 4789 | inner Ethernet | `vxlan` |
| `gre` | GRE (RFC 2784) | IP protocol 47 | IPv4 / IPv6 / Ethernet (`0x6558`) / MPLS by `protocol_type` | `gre-ipv4` |
| `nvgre` | NVGRE (RFC 7637) | build only (set IP `protocol: 47` manually) | inner Ethernet | `nvgre` |
| `mpls` | one MPLS label entry (RFC 3032) | EtherType `0x8847` / `0x8848`, GRE, UDP 6635 (MPLS-in-UDP) | another `mpls`, or IPv4 / IPv6 by the first nibble | `mpls-ipv4` / `mpls-in-udp` |
| `gtpu` | GTP-U (3GPP TS 29.281) | UDP 2152 | a G-PDU (type 255) carries IPv4 / IPv6 by the first nibble | `gtpu-ipv4` |
| `geneve` | Geneve (RFC 8926) | UDP 6081 | Ethernet / IPv4 / IPv6 by `protocol_type` | `geneve` |
| `l2tpv2` | L2TPv2 (RFC 2661) | UDP 1701 | opaque `body` | `l2tpv2` |
| `l2tpv3` | L2TPv3 (RFC 3931) | IP protocol 115 | opaque `body` | `l2tpv3` |

**IP-in-IP** needs no protocol of its own: stack `ipv4` (protocol 4) or `ipv6` (protocol 41, 6in4) directly on IPv4 (samples `ipip` / `6in4`).

## Stacking

- **MPLS label stack**: stack as many `mpls` layers as you need. Each `s` (bottom of stack) is Auto: 0 when another `mpls` follows, otherwise 1. Stack IPv4 / IPv6 after the bottom label.
- **NVGRE**: the IPv4 table binds protocol 47 to `gre`, so set `protocol: 47` on `ipv4` manually. A parsed NVGRE packet reads as `gre` and everything from the key on is not decoded correctly.
- **MPLS-in-UDP**: `udp` (`dst_port: 6635`) → `mpls` → `ipv4` ...
- **L2TPv3 over UDP**: UDP 1701 is bound to `l2tpv2`, so L2TPv3 over UDP parses as `l2tpv2`.

## Auto fields

| Field | Value |
|---|---|
| `gre.protocol_type` / `geneve.protocol_type` | the next layer (EtherType) |
| `mpls.s` | whether another `mpls` follows |
| `gtpu.length` | length of the payload |

## Constraints and approximations

- **GRE**: only the 4-octet base header (flags + version, `protocol_type`) is typed. The checksum / key / sequence number fields that appear per flag bit cannot be expressed. For the common keyed case use the dedicated `nvgre`.
- **MPLS**: the payload below the bottom label is told apart by its first nibble (4 → IPv4, 6 → IPv6). Pseudowires (control word / Ethernet) have no reliable marker and parse as `raw`; stack `ethernet2` explicitly when building.
- **GTP-U**: only the mandatory 8-octet header is typed. The sequence number / N-PDU number / extension headers that follow per the E / S / PN flags are not handled; keep those flags 0.
- **Geneve**: option headers have 3 flag bits + a 5-bit length and are not octet aligned, so the options are an opaque `options` region sized by `opt_len` (4-octet words, Manual). Update `opt_len` whenever you change `options`.
- **L2TPv2 / v3**: header fields appear per the flag bits and the control / data type, so everything after `flags` (v2) / `session_id` (v3) is an opaque `body`.
