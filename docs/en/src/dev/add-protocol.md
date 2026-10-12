# Adding a protocol

Status: draft

Core protocols are defined with a Rust macro and user extensions with YAML files in `protocols.d/`; both compile to the same IR. From the IR the project generates building, parsing, the GUI editor and the reference docs. A new protocol needs a definition, a sample template, a golden pcap and, where possible, a tshark cross-check. This keeps the cost of each added protocol constant.

## Building blocks

Before falling back to an approximation (opaque octets or a Manual value), check whether one of these covers the case.

| Need | Building block | Example |
|---|---|---|
| Element count field | `auto = count(list_field)` (also inside list elements and TLV values) | IGMPv3 `num_group_records`, VRRP `count_ip_addrs` |
| Checksum that skips a field | `auto = checksum(kind, coverage, exclude = [field])` | OSPFv2 `authentication`, LSA `ls_age` |
| Octet size of a later variable field | `auto = size(field)` (also inside list elements and TLV values) | IS-IS sub-TLV lengths |
| Prefix of `ceil(bits / 8)` octets | `bytes(from = len, adjust = 7, div = 8)` (`Length::FromField { div }`) | IS-IS extended IP / IPv6 reachability |
| List of N variable-size elements | `Length::Count { field }` | IS-IS optional sub-TLV block (S bit) |
| Region ending at a header length field, after variable-size fields | `Length::UntilOffset { field, unit }` | CLNP / ES-IS options up to the length indicator |
| TLV value octets between the type and the length | `TlvDef::head_octets` | SCTP chunk flags |
| TLV items padded to a multiple of N octets, padding outside the length | `TlvDef::align` (`unpadded_last`: the enclosing item pads the last one) | SCTP chunks, parameters and error causes |
| Pad to a requested PDU size with filler TLVs | `TlvDef::fill` (`TlvFill { variant, until }`) | IS-IS Hello Padding TLVs |
| NUL-terminated string | `cstring` type | TFTP `filename` / `mode` |
| OSI address with length indicator | `nsap` type (length octet is automatic) | CLNP / ES-IS / IS-IS NSAP / NET |
| Demux on the source port too | `Binding::alt_field` | UDP / TCP `src_port` |
| Demux on the first payload bits | `Binding::peek` (`Peek { on, bits, table }`) | MPLS, GTP-U, OSI (NLPID) |
| Messages that differ only in length | `BindingEntry::with_min_len` | IGMPv3 / MLDv2 queries |
| Same number, different layout | `BindingEntry::reparsing` | VRRPv2/v3, HSRPv1/v2, TFTP |

The evaluation order is described under "次プロトコルの選択 (demux)" in the specification, `docs/spec/02-packet-builder.md`.
