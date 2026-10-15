# User-defined protocols (protocols.d)

Proprietary or in-house protocols that are not built in can be added as YAML files, without writing Rust. They become the same definition IR as the built-ins (`wpg_core::ProtocolDef`), so building (including Auto lengths and checksums), parsing (`wpg import`, the tree of the capture page), generators and the GUI builder and preview all work with them.

## Loading them

| Where | How |
|---|---|
| `wpg serve` | `[protocols] dirs = ["/etc/wpg/protocols.d"]` in `wpg.toml` plus `--protocols-dir <DIR>` (both are used) |
| Other subcommands (`send` / `render` / `import` / `template lint` / `capture` ...) | `--protocols-dir <DIR>` (repeatable) or the `WPG_PROTOCOLS_DIR` environment variable |
| Web UI | After login it fetches the server's definitions from `GET /api/protocols/extensions` and adds them to the in-browser catalog |

The `*.yaml` / `*.yml` files directly in each directory are loaded in file name order (subdirectories are not read). They are read once at startup: restart `wpg serve` after changing them. On an invalid definition the command stops with exit status 2 and prints the file and the reason. `wpg config validate` checks `protocols.dirs` too.

```sh
wpg render examples/acme-telemetry.wpg.yaml -o acme.pcapng --protocols-dir examples/protocols.d
wpg import acme.pcapng -o acme.wpg.yaml --protocols-dir examples/protocols.d
```

## File format

A file has `protocols` (new protocol definitions) and `bind` (entries added to the next-layer tables of existing protocols); either may be omitted. The repository has an example in [`examples/protocols.d/acme-telemetry.yaml`](https://github.com/SeiyaFunaokaJP/web-packet-generator/blob/main/examples/protocols.d/acme-telemetry.yaml).

```yaml
protocols:
  - id: acme_tlm            # unique id (the name used as `protocol:` in templates)
    name: ACME Telemetry    # display name
    layer: l7               # palette class: l2 / l3 / l4 / l7 / payload
    fields:                 # in wire order
      - name: version
        type: { kind: uint, bits: 4 }
        default: { uint: 1 }
      - name: msg_type
        type:
          kind: enum
          bits: 4
          variants:
            - { value: 1, name: report }
            - { value: 2, name: alarm }
        default: { uint: 1 }
      - name: length
        type: { kind: uint, bits: 16 }
        auto: { kind: length, of: total }   # octets of the header and what follows
      - name: readings
        type: { kind: bytes, len: { kind: to_end } }

bind:
  - under: udp          # lower protocol
    value: 47000        # value of its selector field (UDP: dst_port, and src_port when parsing)
    protocol: acme_tlm
```

### Protocol definitions

Each definition has the shape of the JSON returned by `GET /api/protocols/{id}`; the output of that API for an existing protocol can be turned into YAML as a starting point.

- Field types (`type.kind`): `uint` / `int` (`bits`), `enum` (`bits`, `variants`), `flags` (`bits`, `flags`), `bytes` (`len`), `mac`, `ipv4`, `ipv6`, `nsap`, `cstring`, `tlv`, `list`, `padding`, `raw`.
- Defaults (`default`) are single-key maps naming the value type: `{ uint: 1 }`, `{ bytes: [0, 1] }`.
- Auto computations (`auto`): lengths (`kind: length`, `of: header | payload | total`), checksums (`kind: checksum`, `algorithm: internet | internet_pseudo_header | fletcher | crc32 | crc32c ...`), next header (`kind: next_protocol`) and more.
- Next-layer selection (`next`): to carry other protocols on top of this one, give `field` and a `table` (`value` → `protocol`).

Definitions go through the same checks as the built-ins when loaded (bit widths, octet alignment, Auto targets ...), and unknown keys are errors.

### Bindings (`bind`)

| Key | Meaning |
|---|---|
| `under` | Lower protocol id (`udp`, `tcp`, `ethernet2`, `dot1q`, `ipv4` ...) |
| `value` / `max` | Selector field value (with `max`: the range `value..=max`) |
| `protocol` | Protocol selected by the value (built-in or from an extension) |
| `field` | Selector field name, only needed when the lower protocol has no next-layer table yet; otherwise it must match the existing selector |

To bind by EtherType, add one entry per lower layer you need, e.g. `ethernet2` and `dot1q` (after a VLAN tag).

## Limitations

- Built-in protocol ids can not be reused and built-in definitions can not be changed (a second file reusing an id is an error as well).
- A value the lower protocol already maps to another protocol (UDP 4789 = VXLAN, for example) can not be bound.
- UDP / TCP ports are not Auto: set `dst_port` in templates (`bind` is used for parsing and for the GUI's suggestions).
- Field descriptions (the Fluent `description_key`) are for built-ins; leave them out in user-defined protocols.
- The protocol reference (`wpg docs gen`) covers the built-in protocols only.

<!-- wpg-docs-gen:begin reference-links -->
## Generated reference

Field reference of the protocols on this page (generated from the definitions):

[`ethernet2`](../reference/protocols/ethernet2.md) · [`dot1q`](../reference/protocols/dot1q.md) · [`ipv4`](../reference/protocols/ipv4.md) · [`tcp`](../reference/protocols/tcp.md) · [`udp`](../reference/protocols/udp.md) · [`ipv6`](../reference/protocols/ipv6.md) · [`raw`](../reference/protocols/raw.md)
<!-- wpg-docs-gen:end reference-links -->
