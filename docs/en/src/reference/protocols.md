# Protocol reference

Status: draft

The field tables of this chapter (fields, types, auto-calculated values and constraints) will be generated automatically from the protocol definition IR, so they always match the code. Generation is planned for milestone M11. Protocols are grouped by priority from P0 (MVP) to P3 (user-defined).

What generation cannot express lives in hand-written supplement pages. Each supplement page covers:

- How the protocol is reached from the layer below (EtherType, IP protocol number, port) and how to stack its layers
- Which fields are computed automatically (Auto) and which must be set manually (Manual)
- Constraints and approximations of the definition (regions kept as opaque octets, and so on)
- The bundled sample templates (`templates/protocols/`) and the tests that verify them

Per-field descriptions are also shown in the GUI field editor (`locales/*/main.ftl`).

## P1 protocol supplements

- [L2 control (LLDP / CDP / STP / LACP / EAPOL / PPPoE / PPP)](protocols/l2-control.md)
- [Encapsulation (VXLAN / GRE / NVGRE / MPLS / GTP-U / Geneve / L2TP / IP-in-IP)](protocols/encap.md)
- [Routing (OSPFv2/v3 / BGP / RIP / RIPng / EIGRP)](protocols/routing.md)
- [Redundancy (VRRP / HSRP / GLBP)](protocols/fhrp.md)
- [Multicast (IGMP / MLD / PIM)](protocols/mcast.md)
- [Applications (DHCPv4/v6 / DNS / NTP / SNMP / Syslog / RADIUS / TFTP)](protocols/app.md)

## Common notes

- **Auto and Manual**: Auto fields are computed when the packet is built. Giving a value makes the field Manual and the value is used verbatim.
- **Next-protocol selection**: a lower layer's `type` / `protocol` / port is Auto from the layer stacked on top of it. The order in which parsing picks the next protocol is described in "次プロトコルの選択" of the specification `docs/spec/02-packet-builder.md`.
- **Opaque regions**: untyped regions are `bytes` fields written as hex strings (`"00 01 02"`). They stay as octets when parsed.
- **Verification**: every group has golden byte tests, build → parse → build round-trip tests and a `tshark` cross-check enabled with `WPG_TSHARK=1` (`crates/wpg-protocols/tests/p1_*.rs`). The sample templates are linted and rendered by `crates/wpg-template/tests/samples.rs`.
