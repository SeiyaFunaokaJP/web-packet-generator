# FHRP (first-hop redundancy)

Status: draft

Notes on VRRPv2 / v3, HSRPv1 / v2 and GLBP (basic). The definitions are in `crates/wpg-protocols/src/fhrp.rs`.

## Identification and stacking

| Protocol ID | Lower layer | Identified by | Destination |
|---|---|---|---|
| `vrrpv2` | `ipv4` | protocol 112 (version 2) | 224.0.0.18, TTL 255 |
| `vrrpv3` | `ipv4` | protocol 112 (version 3) | 224.0.0.18, TTL 255 |
| `vrrpv3_ipv6` | `ipv6` | next header 112 | `ff02::12`, hop limit 255 |
| `hsrp` (v1) | `udp` | port 1985 | 224.0.0.2 |
| `hsrpv2` | `udp` | port 1985 (IPv4) / 2029 (IPv6) | 224.0.0.102 / `ff02::66` |
| `glbp` | `udp` | port 3222 | 224.0.0.102 |

Protocols sharing a number are re-parsed:

- IPv4 protocol 112 is read as `vrrpv3` first and re-read as `vrrpv2` when `version` is 2.
- UDP 1985 is read as `hsrp` (v1) first and re-read as `hsrpv2` when the first octet is 1 (an HSRPv2 Group State TLV).

When building, the protocol written in the layer is used as is and the IP protocol number / UDP port are Auto.

## Auto fields

| Protocol | Auto |
|---|---|
| `vrrpv2` | `count_ip_addrs` (address count), `checksum` (no pseudo header) |
| `vrrpv3` / `vrrpv3_ipv6` | `count_ipvx_addrs`, `checksum` (with the IPv4 / IPv6 pseudo header) |

## Limitations and workarounds

- **VRRPv3** virtual addresses are IPv4 or IPv6 depending on the enclosing IP version. A field width cannot come from a lower layer, so there are two definitions: `vrrpv3` for IPv4 and `vrrpv3_ipv6` for IPv6. Over IPv6 the first address is the link-local virtual address.
- Mind the timer units: VRRPv2 advertisement interval in seconds, VRRPv3 `max_adver_int` in centiseconds, HSRPv1 in seconds, the HSRPv2 Group State TLV and the GLBP Hello TLV in milliseconds.
- **HSRPv1** `auth_data` is the 8-octet clear-text password (default `cisco`).
- **HSRPv2**'s 16-octet virtual address is IPv4 (zero padded) or IPv6 according to `ip_version`, so it is opaque 16 octets. The MD5 authentication TLV (type 4) is opaque too.
- **GLBP** has no public specification; only the Hello TLV (IPv4 virtual address) is typed. Request/Response and Authentication TLVs keep their value octets.

## Samples

| Template | Contents |
|---|---|
| `templates/protocols/fhrp/vrrpv2.wpg.yaml` | a VRRPv2 Advertisement from the master |
| `templates/protocols/fhrp/vrrpv3.wpg.yaml` / `vrrpv3-ipv6.wpg.yaml` | VRRPv3 Advertisement (IPv4 / IPv6) |
| `templates/protocols/fhrp/hsrpv1.wpg.yaml` | an HSRPv1 Hello from the Active router |
| `templates/protocols/fhrp/hsrpv2.wpg.yaml` | an HSRPv2 Hello with a Group State TLV and text authentication |
| `templates/protocols/fhrp/glbp.wpg.yaml` | a GLBP Hello from the AVG |
