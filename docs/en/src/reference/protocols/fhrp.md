# Redundancy protocols (P1)

Status: draft

Definitions: `crates/wpg-protocols/src/fhrp.rs` / tests: `crates/wpg-protocols/tests/p1_fhrp_mcast.rs` / samples: `templates/protocols/fhrp/`

## Protocols and how they are reached

| id | Protocol | Reached from below | Destination | Sample |
|---|---|---|---|---|
| `vrrpv2` | VRRPv2 (RFC 3768) | IPv4 protocol 112 (`version` 2) | `224.0.0.18` | `vrrpv2` |
| `vrrpv3` | VRRPv3 for IPv4 (RFC 5798) | IPv4 protocol 112 | `224.0.0.18` | `vrrpv3` |
| `vrrpv3_ipv6` | VRRPv3 for IPv6 (RFC 5798) | IPv6 next header 112 | `ff02::12` | `vrrpv3-ipv6` |
| `hsrp` | HSRPv1 (RFC 2281) | UDP 1985 | `224.0.0.2` | `hsrpv1` |
| `hsrpv2` | HSRPv2 | UDP 1985 (first octet 1) / UDP 2029 | `224.0.0.102` / `ff02::66` | `hsrpv2` |
| `glbp` | GLBP (basic) | UDP 3222 | `224.0.0.102` | `glbp` |

## How parsing tells them apart

- **VRRPv2 / v3**: both are IP protocol 112 and differ only in the version nibble. IPv4 112 first decodes as `vrrpv3` and is re-decoded as `vrrpv2` when `version` is 2.
- **HSRPv1 / v2**: both use UDP 1985. The first octet is the version (0) in v1 and the first TLV type (1, Group State) in v2, so UDP 1985 first decodes as `hsrp` and is re-decoded as `hsrpv2` for a first octet of 1.

## Auto fields

| Field | Value |
|---|---|
| `vrrpv2.count_ip_addrs` / `vrrpv3*.count_ipvx_addrs` | number of `ip_addresses` entries |
| `vrrpv2.checksum` | Internet checksum (no pseudo header) |
| `vrrpv3.checksum` / `vrrpv3_ipv6.checksum` | Internet checksum with the (IPv4 / IPv6) pseudo header |

## Constraints and approximations

- **Two VRRPv3 definitions**: VRRPv3 carries IPv4 or IPv6 addresses depending on the enclosing IP version, but a field cannot be sized from a lower layer, so there are two definitions. Use `vrrpv3` over IPv4 and `vrrpv3_ipv6` over IPv6. In the IPv6 one the first address is the link-local virtual address.
- **Time units**: the VRRPv2 advertisement interval is in seconds, the 12-bit VRRPv3 `max_adver_int` in centiseconds, HSRP timers in seconds.
- **HSRPv1 authentication**: `auth_data` is the 8-octet clear-text password (default `cisco`).
- **HSRPv2 virtual address**: the 16-octet virtual address is IPv4 (zero padded) or IPv6 according to `ip_version`, so it is an opaque `bytes(16)`.
- **GLBP**: the protocol is undocumented, so only the Hello TLV (IPv4 virtual address) is typed. Request/Response and Authentication TLVs keep their value octets.
