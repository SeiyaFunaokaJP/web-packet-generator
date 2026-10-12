# L2 control protocols

Status: draft

Notes on LLDP, CDP, the STP family, LACP / Marker (Slow Protocols), EAPOL, PPPoE and PPP (LCP / IPCP). The definitions are in `crates/wpg-protocols/src/l2control.rs`.

## Identification and stacking

| Protocol ID | Lower layer | Identified by | Example destination MAC |
|---|---|---|---|
| `lldp` | `ethernet2` | EtherType `0x88cc` | `01:80:c2:00:00:0e` |
| `cdp` | `dot3` + `llc` + `snap` | OUI `0x00000c` / PID `0x2000` | `01:00:0c:cc:cc:cc` |
| `stp` | `dot3` + `llc` | DSAP / SSAP `0x42` | `01:80:c2:00:00:00` |
| `lacp` / `marker` | `ethernet2` | EtherType `0x8809` (Slow Protocols, subtype 1 / 2) | `01:80:c2:00:00:02` |
| `eapol` | `ethernet2` | EtherType `0x888e` | `01:80:c2:00:00:03` |
| `pppoe_discovery` | `ethernet2` | EtherType `0x8863` | broadcast for PADI |
| `pppoe_session` → `ppp` | `ethernet2` | EtherType `0x8864` | |
| `ppp_lcp` / `ppp_ipcp` | `ppp` | PPP protocol `0xc021` / `0x8021` | |

`ppp_lcp`, `ppp_ipcp`, `ipv4` and `ipv6` can go on top of PPP; `ppp.protocol` is Auto from the next layer.

## Auto fields

| Protocol | Auto |
|---|---|
| `cdp` | `checksum` (RFC 1071 Internet checksum) |
| `eapol` | `length` (of `body`) |
| `pppoe_discovery` | `length` (of the tags) |
| `pppoe_session` | `length` (of the PPP frame) |
| `ppp_lcp` / `ppp_ipcp` | `length` (whole CP packet) |

## Limitations and workarounds

- **LLDP TLV lengths are Manual.** The LLDP TLV header is a 7-bit type and a 9-bit length, and the engine's TLV type only handles whole-octet widths, so `tlvs` is a list of `{ type, length, value }`. Lengths inside list elements are not computed: set `length` to the octet count of `value`. Write the closing End TLV (`type: 0, length: 0`) explicitly.
- **The CDP checksum** is the standard RFC 1071 sum. Cisco sign-extends the trailing octet of an odd-length PDU, so odd-length PDUs do not match Cisco's value; set `checksum` manually if you need it.
- **STP** models the IEEE 802.1D Configuration BPDU (35 octets) exactly. The RSTP / MSTP extension (version-1 length, v3 length, MSTI records) goes in the opaque `trailer`. For an RST BPDU set `protocol_version: rstp` (2), `bpdu_type: rapid` (0x02) and a `trailer` of `00` (version-1 length).
- Timers (`max_age` / `hello_time` / `forward_delay`) are in 1/256-second units. Bridge / root identifiers are split into the 2-octet priority (with the system-id extension) and the 6-octet MAC.
- **LACPDU / Marker PDU** are a fixed 110 octets. Actor / partner state are flag fields, written like `activity|aggregation|synchronization`.
- **EAPOL**'s `body` (the EAP packet) is opaque.

## Samples

| Template | Contents |
|---|---|
| `templates/protocols/l2/lldp.wpg.yaml` | Chassis ID / Port ID / TTL / System Name / End |
| `templates/protocols/l2/cdp.wpg.yaml` | CDPv2 (Device ID / Port ID / Capabilities) |
| `templates/protocols/l2/stp.wpg.yaml` | an 802.1D Configuration BPDU from the root bridge |
| `templates/protocols/l2/lacp.wpg.yaml` / `marker.wpg.yaml` | LACPDU / Marker PDU |
| `templates/protocols/l2/eapol.wpg.yaml` | EAPOL-EAP carrying an EAP Response/Identity |
| `templates/protocols/l2/pppoe-discovery.wpg.yaml` | PADI |
| `templates/protocols/l2/pppoe-ppp-lcp.wpg.yaml` / `pppoe-ppp-ipcp.wpg.yaml` | LCP / IPCP Configure-Request over a PPPoE session |
