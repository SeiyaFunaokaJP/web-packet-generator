# Encapsulation and tunnels

Status: draft

Notes on VXLAN / GRE / NVGRE / MPLS / MPLS-in-UDP / L2TPv2 and v3 / GTP-U / Geneve / IP-in-IP. Definitions are in `crates/wpg-protocols/src/encap.rs`, samples in `templates/protocols/encap/`.

## How they are reached, and samples

| id | Protocol | Layer below | Layer above | Sample |
|---|---|---|---|---|
| `vxlan` | VXLAN | UDP 4789 | `ethernet2` | `vxlan.wpg.yaml` |
| `geneve` | Geneve | UDP 6081 | chosen by `protocol_type` | `geneve.wpg.yaml` |
| `gre` | GRE (4-octet base) | IPv4 protocol / IPv6 next header 47 | `protocol_type` (IPv4 / IPv6 / `0x6558` Ethernet / MPLS) | `gre-ipv4.wpg.yaml` |
| `nvgre` | NVGRE | build only (IP protocol 47 decodes as `gre`) | `ethernet2` | `nvgre.wpg.yaml` |
| `mpls` | one MPLS label | EtherType `0x8847` / `0x8848`, GRE, UDP 6635 (MPLS-in-UDP) | `s = 0`: another `mpls`; `s = 1`: IPv4 / IPv6 by first nibble | `mpls-ipv4.wpg.yaml`, `mpls-in-udp.wpg.yaml` |
| `l2tpv2` | L2TPv2 | UDP 1701 | body is opaque | `l2tpv2.wpg.yaml` |
| `l2tpv3` | L2TPv3 over IP | IPv4 protocol 115 | body is opaque | `l2tpv3.wpg.yaml` |
| `gtpu` | GTP-U (mandatory 8 octets) | UDP 2152 | IPv4 / IPv6 by first nibble | `gtpu-ipv4.wpg.yaml` |
| — | IP-in-IP / 6in4 | IPv4 protocol 4 → `ipv4`, 41 → `ipv6` | — | `ipip.wpg.yaml`, `6in4.wpg.yaml` |

There is no dedicated IP-in-IP protocol: stack `ipv4` (or `ipv6`) on top of `ipv4` and the outer `protocol` becomes 4 (41) automatically.

## Auto fields

- `gre.protocol_type`, `geneve.protocol_type`: from the next layer.
- `gtpu.length`: the length after the GTP-U header.
- `mpls.s` (bottom of stack): 0 when the next layer is `mpls`, 1 otherwise. Stack as many `mpls` layers as labels you need.
- The outer UDP destination port, EtherType and IP protocol follow from the layer stacked above them.

## Limits and approximations

- **GRE** checksum / key / sequence number fields exist only when their flag bit is set, and the engine has no presence-conditional fields. `gre` is the 4-octet base (flags + protocol type) only; setting a flag does not add the field. For NVGRE, the common keyed case, use the dedicated `nvgre` (Key bit on by default, `vsid` + `flow_id`). `nvgre` is build-only: a received packet decodes as `gre`, and the 4 key octets are misread as the start of the inner frame. For other options, put the values in a `raw` layer right after `gre` and set the flags by hand.
- **MPLS pseudowires**: below the bottom label the payload is told apart by its first nibble (4 → IPv4, 6 → IPv6). A control word or Ethernet pseudowire has no reliable marker and decodes as `raw`; when building, stack `ethernet2` explicitly.
- **GTP-U** optional fields (sequence number, N-PDU number, extension headers, present per the E/S/PN flags) are not modelled.
- **Geneve** options have a header with 3 flag bits and a 5-bit length, narrower than an octet, so they are an opaque `options` region sized by `opt_len`.
- **L2TPv2 / v3** header fields depend heavily on the message kind (control / data) and flag bits, so the body is opaque octets.
