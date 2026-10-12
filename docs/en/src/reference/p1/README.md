# P1 protocol notes

Status: draft

The field tables in the [Protocol reference](../protocols.md) are generated from the protocol definition IR (M11). This section supplements them with what generated tables cannot convey, grouped by P1 protocol family:

- **Stacking**: which layer a protocol goes on and which lower-layer value (EtherType, IP protocol number, UDP / TCP port) identifies it
- **Auto fields**: lengths, counts, checksums and next-protocol numbers computed at build time
- **Limitations (approximations)**: where the engine's expressiveness forces opaque octets or Manual values, and how to work with them
- **Samples**: the bundled templates under `templates/protocols/`

| Family | Protocols |
|---|---|
| [L2 control](l2-control.md) | LLDP / CDP / STP, RSTP, MSTP / LACP, Marker / EAPOL / PPPoE / PPP (LCP, IPCP) |
| [Tunnels / encapsulation](tunnels.md) | VXLAN / GRE / NVGRE / MPLS / MPLS-in-UDP / L2TPv2, v3 / GTP-U / Geneve / IP-in-IP |
| [Routing](routing.md) | OSPFv2 / OSPFv3 / BGP-4 / RIPv1, v2 / RIPng / EIGRP |
| [FHRP](fhrp.md) | VRRPv2, v3 / HSRPv1, v2 / GLBP |
| [Multicast](multicast.md) | IGMPv1, v2, v3 / MLDv1, v2 / PIM-SM v2 |
| [Application](application.md) | DHCPv4 / DHCPv6 / DNS / NTP / SNMP / Syslog / RADIUS / TFTP |

## Common conventions

- A field marked **Auto** is computed at build time when the template does not set it. Setting a value makes it Manual and the value is used as is (a mismatch is a warning).
- **Next-protocol numbers** (EtherType, IP protocol, a header's `type`, ...) are Auto from the next layer. When several protocols share a number (for example IPv4 protocol 112 = VRRPv3 / VRRPv2), parsing reads the first candidate and re-parses when needed.
- **UDP / TCP** select the next protocol by destination port, falling back to the source port when the destination is unknown, so replies from a server (DNS responses and so on) decode too. Building makes only the destination port Auto.
- Every sample template is linted and rendered by `crates/wpg-template/tests/samples.rs`; their contents are checked by golden byte tests and by the `tshark -V` comparison (an optional CI job).
- `wpg template show <file>` (hex dump and tree) and `wpg render <file> -o out.pcapng` are handy for checking.
