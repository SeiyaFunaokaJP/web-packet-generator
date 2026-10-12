# Redundancy protocols (FHRP)

Status: draft

Notes on VRRPv2 and v3 / HSRPv1 and v2 / GLBP. Definitions are in `crates/wpg-protocols/src/fhrp.rs`, samples in `templates/protocols/fhrp/`.

## How they are reached, and samples

| id | Protocol | Layer below | Sample |
|---|---|---|---|
| `vrrpv3` | VRRPv3 (IPv4 addresses) | IPv4 protocol 112 | `vrrpv3.wpg.yaml` |
| `vrrpv2` | VRRPv2 | IPv4 protocol 112 (re-decoded when `version` is 2) | `vrrpv2.wpg.yaml` |
| `vrrpv3_ipv6` | VRRPv3 (IPv6 addresses) | IPv6 next header 112 | `vrrpv3-ipv6.wpg.yaml` |
| `hsrp` | HSRPv1 | UDP 1985 | `hsrpv1.wpg.yaml` |
| `hsrpv2` | HSRPv2 | UDP 1985 (re-decoded for a first octet of 1), UDP 2029 (IPv6) | `hsrpv2.wpg.yaml` |
| `glbp` | GLBP (basic) | UDP 3222 | `glbp.wpg.yaml` |

## Auto fields

- VRRP `checksum` (Internet checksum for v2; including the IP pseudo header for v3) and the address counts (`count_ip_addrs` / `count_ipvx_addrs`).

## Limits and approximations

- **VRRPv2 and v3** share IP protocol 112 and differ only in the version nibble. Decoding starts as `vrrpv3` and re-decodes as `vrrpv2` when `version` is 2. When building, pick the definition you want.
- **VRRPv3 address family** follows the IP version of the enclosing packet, but a field cannot be sized from a lower layer, so there are two definitions: `vrrpv3` (IPv4) and `vrrpv3_ipv6` (IPv6).
- **HSRPv1 and v2** share UDP 1985. The first octet is the version (0) in v1 and the first TLV type (1, Group State) in v2, so decoding starts as `hsrp` and re-decodes as `hsrpv2` for 1. The 16-octet HSRPv2 virtual address is IPv4 (zero padded) or IPv6 depending on `ip_version`, so it is opaque `bytes(16)`.
- **GLBP** is undocumented; only the Hello TLV (IPv4 virtual address) is typed. Request/Response and Authentication TLVs keep their value octets.
- Only clear-text authentication is typed: VRRPv2 `auth_data` (8 octets), HSRPv1 `auth_data` and the HSRPv2 Text Authentication TLV (both default to `cisco`). The HSRPv2 MD5 authentication TLV stays value octets, and no digest is computed.

<!-- wpg-docs-gen:begin reference-links -->
## Generated reference

Field reference of the protocols on this page (generated from the definitions):

[`vrrpv2`](../reference/protocols/vrrpv2.md) · [`vrrpv3`](../reference/protocols/vrrpv3.md) · [`vrrpv3_ipv6`](../reference/protocols/vrrpv3_ipv6.md) · [`hsrp`](../reference/protocols/hsrp.md) · [`hsrpv2`](../reference/protocols/hsrpv2.md) · [`glbp`](../reference/protocols/glbp.md)
<!-- wpg-docs-gen:end reference-links -->
