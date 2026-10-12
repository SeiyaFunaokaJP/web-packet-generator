# L2 control protocols (P1)

Status: draft

Definitions: `crates/wpg-protocols/src/l2control.rs` / tests: `crates/wpg-protocols/tests/p1_l2.rs` / samples: `templates/protocols/l2/`

## Protocols and how they are reached

| id | Protocol | Reached from below | Sample |
|---|---|---|---|
| `lldp` | LLDP (IEEE 802.1AB) | EtherType `0x88cc` | `lldp` |
| `cdp` | CDP | 802.3 + LLC + SNAP (OUI `0x00000c`, PID `0x2000`) | `cdp` |
| `stp` | STP / RSTP / MSTP BPDU | 802.3 + LLC (DSAP/SSAP `0x42`) | `stp` |
| `lacp` | LACPDU (Slow Protocols subtype 1) | EtherType `0x8809` | `lacp` |
| `marker` | Marker / Marker-Response (subtype 2) | build only (set `ethertype: 0x8809` manually) | `marker` |
| `eapol` | EAPOL (IEEE 802.1X) | EtherType `0x888e` | `eapol` |
| `pppoe_discovery` | PPPoE Discovery (PADI to PADT) | EtherType `0x8863` | `pppoe-discovery` |
| `pppoe_session` | PPPoE Session | EtherType `0x8864` | `pppoe-ppp-lcp` |
| `ppp` | PPP frame | after `pppoe_session` (default) | `pppoe-ppp-lcp` |
| `ppp_lcp` / `ppp_ipcp` | LCP / IPCP | PPP protocol `0xc021` / `0x8021` | `pppoe-ppp-lcp` / `pppoe-ppp-ipcp` |

IPv4 (`0x0021`) and IPv6 (`0x0057`) stack directly on PPP as well.

## Stacking

- **CDP**: `dot3` → `llc` → `snap` (`oui: 0x00000c`; `pid` is Auto `0x2000`) → `cdp`. The destination is `01:00:0c:cc:cc:cc`. Set the `dot3` `length` to `auto`.
- **STP**: `dot3` → `llc` → `stp`. The `llc` DSAP / SSAP become `0x42` automatically. The destination is `01:80:c2:00:00:00`.
- **LACP / Marker**: `ethernet2` → `lacp` (EtherType Auto). Marker is not in the EtherType table, so set `ethertype: 0x8809` manually. A parsed Marker PDU reads as `lacp`.
- **PPPoE**: `ethernet2` → `pppoe_session` → `ppp` → `ppp_lcp` / `ppp_ipcp` / `ipv4` / `ipv6`. `ppp.protocol` is Auto from the layer above.

## Auto fields

| Field | Value |
|---|---|
| `cdp.checksum` | RFC 1071 Internet checksum (header + TLVs) |
| `eapol.length` | length of `body` |
| `pppoe_discovery.length` | length of the tags |
| `pppoe_session.length` | length of the payload (the PPP frame) |
| `ppp.protocol` | the next layer (LCP / IPCP / IPv4 / IPv6) |
| `ppp_lcp.length` / `ppp_ipcp.length` | length of the whole control-protocol packet |

## Constraints and approximations

- **LLDP TLV lengths are Manual**: an LLDP TLV header is a 7-bit type + 9-bit length, and TLV definitions only support whole-octet widths. `tlvs` is therefore a list of `{type, length, value}` structs and `length` is not computed. Update `length` whenever you change `value`. The default is Chassis ID / Port ID (MAC), TTL 120 and the End TLV.
- **CDP checksum**: Auto gives the standard RFC 1071 value, which matches Cisco for even-length PDUs. For an odd length Cisco sign-extends the trailing octet and the values differ; set `checksum` manually in that case.
- **STP**: the 802.1D Configuration BPDU (35 octets) is typed exactly. The RSTP / MSTP extension (version-1 length, v3 length, MSTI records) is an opaque `trailer`. Bridge / root identifiers are split into the 2-octet priority (including the system-id extension) and the 6-octet MAC. Timers are in 1/256-second units (`max_age` 20 s = `0x1400`).
- **LACP / Marker**: fixed 110 octets. Each TLV's `*_info_type` / `*_info_length` defaults to the fixed value of the standard and is not Auto.
- **EAPOL**: `body` (the EAP packet for type 0) is opaque.
