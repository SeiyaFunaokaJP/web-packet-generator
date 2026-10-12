# Adding a protocol

Core protocols are defined with a Rust macro (`crates/wpg-protocols/src/`) and compile to the IR. From the IR the project generates building, parsing, the GUI editor and the reference docs. A new protocol needs a definition, Fluent strings, a sample template, a golden pcap and, where possible, a tshark cross-check. This keeps the cost of each added protocol constant.

## Steps

1. **Define it.** Add the protocol with the macro to the right module in `crates/wpg-protocols/src/` (for example `app.rs` for an application-layer protocol) and register it in the catalog. Tag each field with a Fluent key via `desc = "key"`. Before reaching for an approximation, check the building blocks below.
2. **Add Fluent strings.** Add the protocol name and every field's `desc` key to both `locales/ja/main.ftl` and `locales/en/main.ftl` (see [Adding i18n strings](add-i18n.md)).
3. **Add a sample template.** Put one or more `.wpg.yaml` files under `templates/protocols/<group>/`, with ja / en `descriptions` and `protocol:` / `layer:` tags. The `samples.rs` tests check that every protocol has a sample and that well-formed samples parse back to the same stack.
4. **Regenerate the golden files.** Rebuild `tests/golden/*.hex` / `*.pcap` with `just golden-update` and review the diff. Never edit the `.hex` / `.pcap` by hand.
5. **Cross-check.** If tshark is available, compare against Wireshark with `WPG_TSHARK=1 cargo test -p wpg-golden-tests -- --ignored`.
6. **Update the notes.** When approximations remain or the encapsulation is unusual, add matching ja / en notes under `docs/{ja,en}/src/protocols/`.
7. **Pass the pre-push checks** (CLAUDE.md).

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
cargo check -p wpg-core -p wpg-protocols -p wpg-i18n --target wasm32-unknown-unknown
```

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
| TLV items of one type whose layout depends on the value | `TlvVariant::when_value_starts` / `when_value_len` (variants sharing a type; one without a discriminator is the fallback) | BGP `MP_REACH_NLRI` per AFI / SAFI, OSPF SR SIDs of 3 or 4 octets, EVPN IPv4 / IPv6 IP Prefix routes |
| TLV items with the length before the type | `TlvDef::len_first` (not with `head_octets` / `long_length`) | RSVP objects (length, Class-Num, C-Type) |
| A type flag that widens the TLV length | `TlvDef::long_length` (`TlvLongLength { flag, len_bits }`, set automatically when the value is too long) | BGP Extended Length path attributes |
| Pad to a requested PDU size with filler TLVs | `TlvDef::fill` (`TlvFill { variant, until }`) | IS-IS Hello Padding TLVs |
| NUL-terminated string | `cstring` type | TFTP `filename` / `mode` |
| OSI address with length indicator | `nsap` type (length octet is automatic) | CLNP / ES-IS / IS-IS NSAP / NET |
| Demux on the source port too | `Binding::alt_field` | UDP / TCP `src_port` |
| Demux on the first payload bits | `Binding::peek` (`Peek { on, bits, table }`) | MPLS, GTP-U, OSI (NLPID) |
| Messages that differ only in length | `BindingEntry::with_min_len` | IGMPv3 / MLDv2 queries |
| Same number, different layout | `BindingEntry::reparsing` | VRRPv2/v3, HSRPv1/v2, TFTP |

The evaluation order is described under "次プロトコルの選択 (demux)" in the specification, `docs/spec/02-packet-builder.md`.
