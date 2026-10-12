# P1 protocol notes

Status: draft

The [protocol reference](../reference/protocols.md) is the field listing that will be generated from the protocol definition IR (M11). This chapter is the hand-written supplement to it for the P1 protocols (M6). For each group it covers:

- how a protocol is reached from the layer below (demultiplexing on decode)
- which fields are Auto and which you must set by hand (Manual)
- the approximations and limits that remain because of what the engine can express
- the bundled sample templates (`templates/protocols/`)

Per-field descriptions (ja/en) come with the GUI field editor and `GET /api/protocols`. The primary source for the limits is the module docs in `crates/wpg-protocols/src/`; where this chapter disagrees, the code wins.

| Page | Protocols |
|---|---|
| [L2 control protocols](l2-control.md) | LLDP / CDP / STP / LACP and Marker / EAPOL / PPPoE / PPP (LCP, IPCP) |
| [Encapsulation and tunnels](encap.md) | VXLAN / GRE / NVGRE / MPLS / MPLS-in-UDP / L2TPv2 and v3 / GTP-U / Geneve / IP-in-IP |
| [Routing protocols](routing.md) | OSPFv2 and v3 / BGP-4 / RIPv1 and v2 / RIPng / EIGRP |
| [Redundancy (FHRP)](fhrp.md) | VRRPv2 and v3 / HSRPv1 and v2 / GLBP |
| [Multicast control](multicast.md) | IGMPv1, v2, v3 / MLDv1 and v2 / PIM-SM v2 |
| [Application protocols](app.md) | DHCPv4 and v6 / DNS / NTP / SNMP / Syslog / RADIUS / TFTP |
| [OSI encapsulation (P2)](osi.md) | OSI PDUs over 802.2 LLC `0xfe` / PPP `0x0023`, the NSAP type |
| [CLNP / ES-IS (P2)](clnp.md) | CLNP DT / ER / echo with options and segmentation, ES-IS ESH / ISH / Redirect |
| [IS-IS (P2)](isis.md) | LAN / P2P Hellos, LSPs, CSNP / PSNP, main TLVs and sub-TLVs |
| [SCTP (P2)](sctp.md) | Common header with CRC32c, bundled chunks (DATA, INIT, SACK, HEARTBEAT, ABORT, ASCONF, RE-CONFIG, ...), parameters and error causes |
| [OSPF Opaque / SR, BGP MP / EVPN (P2)](routing-ext.md) | OSPFv2 TE / Router Information / Extended Prefix / Extended Link LSAs with SR SIDs, BGP NLRI for IPv6, VPN-IPv4 / VPN-IPv6 and EVPN |
| [LDP / RSVP-TE / BFD (P2)](mpls-bfd.md) | LDP PDUs, messages and TLVs (FEC, labels, Hello / session parameters), RSVP-TE Path / Resv and other messages with their objects (ERO / RRO, SESSION_ATTRIBUTE, IntServ), BFD control packets with authentication (Micro-BFD included) |
| [SRv6 SRH / SR-MPLS (P2)](srv6.md) | SRv6 Segment Routing Header (segment list, Pad / HMAC TLVs, upper-layer checksums toward the final destination, IPv4 / IPv6 / Ethernet encapsulation), SR-MPLS label stacks (reserved label names, SRGB + index, entropy labels) |

## Common patterns

- **Header + body split**: protocols whose common header `type` selects the body layout (OSPF, BGP, IGMP, PIM, ICMPv6 for MLD) are stacked as two layers, the header and the body (for example `ospfv2` → `ospfv2_hello`). The header `type` is Auto from the next layer, so usually you only pick the body layer.
- **Re-parse**: where one number or port carries several layouts (VRRPv2/v3, HSRPv1/v2, DHCPv6 relay, SNMP Trap, the TFTP opcodes), decoding starts with a representative definition and re-decodes as another one based on a version or type value. When building, pick the definition you want directly.
- **First-nibble demux**: the payload under the bottom MPLS label, GTP-U and the packet inside PIM Register are told apart by their first 4 bits (4 → IPv4, 6 → IPv6).
- **Source-port fallback**: UDP and TCP demultiplex on the destination port and fall back to the source port, so the reply direction (DNS responses, the passive side of BGP, ...) decodes too.
- **Opaque regions**: parts that cannot be typed are kept as octets. Give them as hex when building; decoding shows them as octets too.

## Trying the samples

```console
$ wpg template lint templates/protocols/routing/*.wpg.yaml
$ wpg render templates/protocols/routing/ospfv2-hello.wpg.yaml -o /tmp/ospf.pcapng
$ tshark -r /tmp/ospf.pcapng -V
```

Every sample is linted and rendered by `crates/wpg-template/tests/samples.rs`. The P1 golden byte tests live in `crates/wpg-protocols/tests/p1_*.rs`; run them with `WPG_TSHARK=1` and they also check that tshark dissects the frames (the protocol is recognised and nothing is flagged malformed).
