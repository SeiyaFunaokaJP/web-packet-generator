# Tunnels / encapsulation

Status: draft

Notes on VXLAN, GRE, NVGRE, MPLS, MPLS-in-UDP, L2TPv2 / v3, GTP-U, Geneve and IP-in-IP. The definitions are in `crates/wpg-protocols/src/encap.rs`.

A tunnel is just layers in the order "outer headers → tunnel header → inner packet". The inner packet stacks `ethernet2` / `ipv4` / `udp` and so on as usual.

## Identification and stacking

| Protocol ID | Lower layer | Identified by | Inner (next layer) |
|---|---|---|---|
| `vxlan` | `udp` | port 4789 | `ethernet2` |
| `gre` | `ipv4` | protocol 47 | `ipv4` / `ipv6` / `ethernet2` / `mpls` by `protocol_type` |
| `nvgre` | `ipv4` | protocol 47 (**set manually**, see below) | `ethernet2` |
| `mpls` | `ethernet2` / `gre` / `udp` | EtherType `0x8847` / GRE `0x8847` / port 6635 | `mpls` (S=0), IPv4 / IPv6 (S=1) |
| `l2tpv2` | `udp` | port 1701 | opaque `body` |
| `l2tpv3` | `ipv4` | protocol 115 | opaque `body` |
| `gtpu` | `udp` | port 2152 | `ipv4` / `ipv6` for a G-PDU (type 255) |
| `geneve` | `udp` | port 6081 | `ethernet2` / `ipv4` / `ipv6` by `protocol_type` |
| IP-in-IP | `ipv4` | protocol 4 (`ipv4`) / 41 (`ipv6`) | no dedicated protocol: stack `ipv4` / `ipv6` on `ipv4` |

## Auto fields

| Protocol | Auto |
|---|---|
| `gre` / `geneve` | `protocol_type` (from the next layer) |
| `mpls` | `s` (0 when the next layer is `mpls`, else 1) |
| `gtpu` | `length` (payload length) |
| outer `ipv4` / `udp` | `protocol`, lengths, checksums; `udp.dst_port` becomes the tunnel port too |

## Limitations and workarounds

- **GRE** models only the 4-octet base header (flags + version, `protocol_type`). The checksum / key / sequence-number extensions are not modelled; use the dedicated `nvgre` for NVGRE, the common keyed case.
- **NVGRE**: IPv4 protocol 47 decodes as `gre`, so building does not make it Auto 47. Set `protocol: 47` on the outer `ipv4` (see `nvgre.wpg.yaml`). It parses back as `gre`, with the 4 key octets read as the start of the inner frame.
- **MPLS** is one layer per label; stack as many `mpls` layers as the label stack needs. Below the bottom label the first nibble selects IPv4 (4) or IPv6 (6). Pseudowire payloads (control word / Ethernet) have no reliable marker and parse as `raw`; stack `ethernet2` and so on explicitly when building.
- **MPLS-in-UDP** (RFC 7510) is `mpls` on a `udp` with port 6635. Varying the source port for entropy (a range generator on `src_port`) is closer to real routers.
- **GTP-U** models only the mandatory 8-octet header; the sequence number / N-PDU number / extension headers present per the E / S / PN flags are not modelled.
- **Geneve** options have a sub-octet header, so they are one opaque `options` region sized by `opt_len` (in 4-octet words). `opt_len` is Manual: keep it in step with `options`.
- **L2TPv2 / v3** header fields come and go with the flag bits and control / data type, so v2 keeps everything after the 16-bit flags / version word, and v3 everything after the Session ID, as an opaque `body`. Write length, tunnel id, session id, the PPP frame and so on as octets in `body`.

## Samples

| Template | Contents |
|---|---|
| `templates/protocols/encap/vxlan.wpg.yaml` | an inner Ethernet / IPv4 / UDP frame over VXLAN (VNI as a variable) |
| `templates/protocols/encap/gre-ipv4.wpg.yaml` | IPv4 / ICMP over GRE |
| `templates/protocols/encap/nvgre.wpg.yaml` | NVGRE (VSID as a variable) |
| `templates/protocols/encap/mpls-ipv4.wpg.yaml` | IPv4 / UDP over a two-label MPLS stack |
| `templates/protocols/encap/mpls-in-udp.wpg.yaml` | MPLS-in-UDP |
| `templates/protocols/encap/l2tpv2.wpg.yaml` / `l2tpv3.wpg.yaml` | L2TPv2 data / L2TPv3 over IP |
| `templates/protocols/encap/gtpu-ipv4.wpg.yaml` | a G-PDU carrying IPv4 / UDP (TEID as a variable) |
| `templates/protocols/encap/geneve.wpg.yaml` | an inner Ethernet frame over Geneve |
| `templates/protocols/encap/ipip.wpg.yaml` / `6in4.wpg.yaml` | IP-in-IP (protocol 4) / 6in4 (protocol 41) |
