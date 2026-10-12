# Adding a protocol

Status: draft

Core protocols are defined with a Rust macro and user extensions with YAML files in `protocols.d/`; both compile to the same IR. From the IR the project generates building, parsing, the GUI editor and the reference docs. A new protocol needs a definition, a sample template, a golden pcap and, where possible, a tshark cross-check. This keeps the cost of each added protocol constant.

## Building blocks

Before falling back to an approximation (opaque octets or a Manual value), check whether one of these covers the case.

| Need | Building block | Example |
|---|---|---|
| Element count field | `auto = count(list_field)` (also inside list elements and TLV values; `count(list_field, adjust = -1)` gives the index of the last element) | IGMPv3 `num_group_records`, VRRP `count_ip_addrs`, SRH `last_entry` |
| Checksum that skips a field | `auto = checksum(kind, coverage, exclude = [field])` | OSPFv2 `authentication`, LSA `ls_age` |
| Octet size of a later variable field | `auto = size(field)` (also inside list elements and TLV values) | IS-IS sub-TLV lengths |
| Prefix of `ceil(bits / 8)` octets | `bytes(from = len, adjust = 7, div = 8)` (`Length::FromField { div }`) | IS-IS extended IP / IPv6 reachability |
| List of N variable-size elements | `Length::Count { field }` | IS-IS optional sub-TLV block (S bit) |
| Region ending at a header length field, after variable-size fields | `Length::UntilOffset { field, unit, adjust }` (ends at `field * unit + adjust`) | CLNP / ES-IS options up to the length indicator, SRH TLVs (`hdr_ext_len * 8 + 8`) |
| TLV value octets between the type and the length | `TlvDef::head_octets` | SCTP chunk flags |
| TLV items padded to a multiple of N octets, padding outside the length | `TlvDef::align` (`unpadded_last`: the enclosing item pads the last one) | SCTP chunks, parameters and error causes |
| TLV items of one type whose layout depends on the value | `TlvVariant::when_value_starts` / `when_value_len` (variants sharing a type; one without a discriminator is the fallback) | BGP `MP_REACH_NLRI` per AFI / SAFI, OSPF SR SIDs of 3 or 4 octets, EVPN IPv4 / IPv6 IP Prefix routes |
| TLV items with the length before the type | `TlvDef::len_first` (not with `head_octets` / `long_length`) | RSVP objects (length, Class-Num, C-Type) |
| A type flag that widens the TLV length | `TlvDef::long_length` (`TlvLongLength { flag, len_bits }`, set automatically when the value is too long) | BGP Extended Length path attributes |
| Pad to a requested PDU size with filler TLVs | `TlvDef::fill` (`TlvFill { variant, until }`) | IS-IS Hello Padding TLVs |
| NUL-terminated string | `cstring` type | TFTP `filename` / `mode` |
| OSI address with length indicator | `nsap` type (length octet is automatic) | CLNP / ES-IS / IS-IS NSAP / NET |
| Demux on the source port too | `Binding::alt_field` | UDP / TCP `src_port` |
| Demux on the first payload bits | `Binding::peek` (`Peek { on, bits, table }`; `skipping(n)` skips `n` bits before reading) | MPLS, GTP-U, OSI (NLPID), IPv6 routing type (`skip = 16`) |
| Routing header that changes the destination of upper-layer checksums | `ProtocolDef::final_destination` (`FinalDestination { list, when }`: element 0 of `list` becomes the pseudo-header destination while `when` is non-zero) | SRv6 SRH |
| Messages that differ only in length | `BindingEntry::with_min_len` | IGMPv3 / MLDv2 queries |
| Same number, different layout | `BindingEntry::reparsing` | VRRPv2/v3, HSRPv1/v2, TFTP |

The evaluation order is described under "次プロトコルの選択 (demux)" in the specification, `docs/spec/02-packet-builder.md`.
