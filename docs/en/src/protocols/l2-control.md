# L2 control protocols

Status: draft

Notes on LLDP / CDP / STP / LACP and Marker / EAPOL / PPPoE / PPP. Definitions are in `crates/wpg-protocols/src/l2control.rs`, samples in `templates/protocols/l2/`.

## How they are reached, and samples

| id | Protocol | Layer below | Sample |
|---|---|---|---|
| `lldp` | LLDP (IEEE 802.1AB) | `ethernet2` EtherType `0x88cc` | `lldp.wpg.yaml` |
| `cdp` | CDP | `dot3` → `llc` → `snap` (OUI `0x00000c`, PID `0x2000`) | `cdp.wpg.yaml` |
| `stp` | STP / RSTP / MSTP BPDU | `dot3` → `llc` (DSAP/SSAP `0x42`) | `stp.wpg.yaml` |
| `lacp` | LACPDU (Slow Protocols subtype 1) | `ethernet2` EtherType `0x8809` | `lacp.wpg.yaml` |
| `marker` | Marker / Marker Response (subtype 2) | `ethernet2` EtherType `0x8809` | `marker.wpg.yaml` |
| `eapol` | EAPOL (IEEE 802.1X) | `ethernet2` EtherType `0x888e` | `eapol.wpg.yaml` |
| `pppoe_discovery` | PPPoE Discovery (PADI to PADT) | `ethernet2` EtherType `0x8863` | `pppoe-discovery.wpg.yaml` |
| `pppoe_session` | PPPoE Session | `ethernet2` EtherType `0x8864` | `pppoe-ppp-lcp.wpg.yaml` |
| `ppp` / `ppp_lcp` / `ppp_ipcp` | PPP (LCP / IPCP) | `pppoe_session` | `pppoe-ppp-lcp.wpg.yaml`, `pppoe-ppp-ipcp.wpg.yaml` |

## Auto fields

- `cdp.checksum`: the RFC 1071 Internet checksum.
- `eapol.length`, `pppoe_discovery.length`, `ppp_lcp.length` / `ppp_ipcp.length`: the length of what follows.
- `ppp.protocol`: from the next layer (LCP / IPCP / IPv4 / IPv6 ...).
- The length octets of CDP / PPPoE / LCP / IPCP TLVs and options.

## Limits and approximations

- **LLDP TLV lengths are not Auto.** An LLDP TLV header is a 7-bit type plus a 9-bit length, and the engine's TLV type only handles whole-octet widths. `tlvs` is therefore a list of `{ type: uint(7), length: uint(9), value }` structs and `length` is Manual. When you change a value, change its `length` too. The trailing End TLV (type 0) is an explicit item as well.
- **CDP checksum**: the standard Internet checksum is computed. Cisco's implementation sign-extends the trailing octet of an odd-length PDU, so **for odd-length PDUs the value differs from real devices.** Keep the TLVs at an even total length, or set the checksum by hand.
- **STP** types the IEEE 802.1D Configuration BPDU (35 octets) exactly. The RSTP / MSTP extension (Version 1 Length, Version 3 Length, MSTI configuration records) is an opaque `trailer`. For an RST BPDU set `protocol_version` and `bpdu_type` and put the Version 1 Length (`00`) in `trailer`; for an MST BPDU give the rest of the MST part as hex as well.
- Bridge and root identifiers are split into the 2-octet priority (including the system-id extension) and the 6-octet MAC. Timers are in 1/256-second units (`max_age` 20 s = `0x1400`).
- The **EAPOL** body (the EAP packet for type 0) is an opaque `body`.
