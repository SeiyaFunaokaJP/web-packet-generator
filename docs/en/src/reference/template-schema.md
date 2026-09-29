# Template schema

A template is a YAML file (`.wpg.yaml`; JSON with `.wpg.json` is equivalent) that describes one stream: a layer stack, optional generators that vary fields, and a transmit profile. The machine-readable definition is [`docs/schema/template.schema.json`](https://github.com/SeiyaFunaokaJP/web-packet-generator/blob/main/docs/schema/template.schema.json); print it with `wpg template schema`. Unknown keys are errors, so typos are caught. Check a file with `wpg template lint FILE`.

```yaml
schema: 1                  # required
meta: { ... }              # required
variables: [ ... ]         # optional
layers: [ ... ]            # required, outermost first
packet: { ... }            # optional
stream: { ... }            # optional
tx: { ... }                # optional
```

## `meta`

| Key | Meaning |
|---|---|
| `name` | required display name; unique within a library (the bundled library is tested for this) |
| `description` | free text |
| `descriptions` | per-language text: `ja:` and `en:` |
| `tags` | search tags. The bundled library uses `protocol:<id>`, `layer:l2\|l3\|l4`, `scenario`, `malformed` and topic words |
| `author`, `created` | free text; `created` is a date or RFC 3339 timestamp |

## `variables`

Template arguments. Each entry has `name`, optional `default` (string, number or bool) and `description`. Use `${name}` anywhere inside a field value or generator. Override at run time with `--var name=value` (`wpg send`, `wpg render`), or in the GUI. A variable with no default and no override is an error, and so is a `${…}` that is not declared. Declared but unused variables give a warning.

## `layers`

Each entry is `protocol` (an id from the catalog, see the [Protocol reference](protocols.md)), `fields` and `generators`.

### `fields`

Fields you write are **Manual**; fields you omit (or set to `auto`) keep their default or are computed (lengths, checksums, next-protocol numbers). Values use human notation:

| Type | Examples |
|---|---|
| integer | `64`, `0x0800`, `0b101` |
| MAC / IPv4 / IPv6 | `02:00:00:00:00:01`, `192.0.2.1`, `2001:db8::1` (quote MACs made only of digits, such as `"00:11:22:33:44:55"`, because YAML can read them as numbers) |
| bytes | `"00 01 02 03"` (always quoted) |
| enum | a number or a variant name: `ethertype: arp`, `oper: request`, `type: echo_request` |
| flags | bit names joined by `\|`: `flags: syn\|ack`, `flags: df` |
| TLV list (TCP/IPv4 options) | `- mss: 1460`, `- sack_permitted`, `- nop`, `- timestamps: { tsval: 1, tsecr: 0 }`, `- { type: 99, data: "01 02" }` |

Deliberately wrong values are accepted: a Manual length or checksum is used as written, and `wpg template lint` reports the mismatch as a warning (`diag-*`). That is how the samples in `templates/malformed/` are made.

### `generators`

A map from field name to a generator, either the full form or the shorthand used by the GUI:

| `kind` | Parameters | Shorthand | Meaning |
|---|---|---|---|
| `range` | `start`, `end`, `step` (default 1) | `192.168.1.1-192.168.5.254`, `1-1024` | inclusive sweep; wraps around when the count is larger |
| `range` (CIDR) | | `10.0.0.0/22` | host addresses of a block |
| `list` | `values` | `1,5,9` | cycle through values |
| `random` | `min`, `max`, `seed` | | uniform random (integers, IPv4, IPv6) |
| `random_mac` | `oui`, `seed` | `00:11:22:*` | random MAC, optionally with a fixed prefix |
| `counter` | `start`, `step`, `wrap` | | incrementing value; unbounded unless `wrap` is set |
| `file` | `path` | | values read from a file |
| `expr` | `expr` | | computed value |

A generator and a fixed value on the same field produce a lint warning.

## `packet`

| Key | Default | Meaning |
|---|---|---|
| `pad_to_min` | `true` | zero-pad to the 60-octet Ethernet minimum (FCS excluded) |
| `fcs` | `none` | `auto` appends a correct CRC-32, `bad` a wrong one, `none` nothing |

## `stream`

How generators combine and how long the stream is.

| Key | Meaning |
|---|---|
| `mode` | `zip` (default, lockstep), `product` (last varies fastest) or `nested` |
| `order` | for `nested`: fields from slowest to fastest as `<layer>.<field>`, where `<layer>` is a 0-based index or a protocol id unique in the stack (`dot1q.vid`, `arp.tpa`) |
| `count` | number of packets; default is `tx.count`, else the natural length of the generators |
| `seed` | seed for random generators; makes the stream reproducible |

Unbounded generators (a `counter` without `wrap`) need a `count` and cannot be in the inner position of `product` or `nested`.

## `tx`

| Key | Meaning |
|---|---|
| `count` | packets to send (no `count` and no `duration_secs` means run until stopped) |
| `duration_secs` | time limit in seconds |
| `rate` | `1000pps`, `1Mpps`, `10Gbps`, `50%`, `ifg=10us` |
| `burst` | frames sent back to back |

Command-line options of `wpg send` override these.

## Payload layers

`payload` is not a wire protocol; its fields control generation: `mode` (`fixed`, `pattern`, `random`, `counter`), `data`, `length`, `seed`, and for `counter` the `counter_offset`, `counter_width` (`u32` or `u64`) and `counter_value` (usually fed by a `counter` generator). `raw` places bytes as given.

## Lint checks

| Key | Severity | Meaning |
|---|---|---|
| `lint-schema-version` | error | unsupported `schema` |
| `lint-missing-name` | warning | empty `meta.name` |
| `lint-no-layers` | error | no layers |
| `lint-unknown-protocol`, `lint-unknown-field` | error | not in the catalog |
| `lint-invalid-value`, `lint-invalid-generator` | error | value or generator does not fit the field type |
| `lint-unknown-variable`, `lint-missing-variable` | error | undeclared variable / variable without a value |
| `lint-unused-variable` | warning | declared but unused |
| `lint-next-protocol-manual` | warning | the next layer is not in the previous layer's binding table, so its next-protocol field cannot be Auto |
| `lint-field-and-generator` | warning | a field has both a fixed value and a generator |
| `diag-*` | error / warning | diagnostics of the built packet, for example a checksum or length that differs from the computed value |

## The bundled library

`templates/` is loaded as a library by default and is checked by `crates/wpg-template/tests/samples.rs`:

| Folder | Contents |
|---|---|
| `examples/` | five small starting points |
| `protocols/l2`, `l3`, `l4`, `payload`, `frame` | one or more samples for every protocol in the catalog, plus jumbo, minimum-size, runt and FCS cases |
| `scenarios/` | ready-made tests: ARP sweeps, SYN flood and port scan, QinQ, NDP and RA, ping and TTL sweeps, QoS matrix, MTU probe, frame-size sweep, MAC flood |
| `malformed/` | deliberately broken packets (bad checksums, wrong lengths, truncation, bad FCS); they lint with warnings by design and are tagged `malformed` |

List them with `wpg template list templates`.
