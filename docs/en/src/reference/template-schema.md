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
| bytes | `"00 01 02 03"` (always quoted); text as `'"text"'` with `\r`, `\n`, `\t` escapes |
| text line (SIP lines, CRLF-terminated) | `start_line: '"INVITE sip:bob@example.com SIP/2.0"'` (the terminator is added automatically) |
| enum | a number or a variant name: `ethertype: arp`, `oper: request`, `type: echo_request` |
| flags | bit names joined by `\|`: `flags: syn\|ack`, `flags: df` |
| TLV list (TCP/IPv4 options) | `- mss: 1460`, `- sack_permitted`, `- nop`, `- timestamps: { tsval: 1, tsecr: 0 }`, `- { type: 99, data: "01 02" }` |
| list / TLV list as verbatim octets | `nlri: "18 c6 33 64"` (a quoted hex string instead of the list; written by the import for content it cannot decode) |

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

### Expressions (`expr`)

An `expr` generator computes an integer for every packet. The result is written into the field like any other value, so it can fill numbers, IPv4 / IPv6 addresses and MAC addresses.

| Element | Meaning |
|---|---|
| `12`, `0x1f`, `0b101` | literals (decimal, hexadecimal, binary) |
| `idx` | the packet index in the stream (0, 1, 2, ...) |
| `$field`, `$<layer>.field` | this packet's value of another *generated* field; `<layer>` is written as in `stream.order` (`$0.src` and `$ethernet2.src` both name the `src` generator of the first layer; `$ipv4[1].src` is the second `ipv4`) |
| `+ - * / %`, `<< >>`, `& ^ \|`, unary `- ~` | C-like operators and precedence; integer arithmetic (128-bit, wrapping) |
| `hash(x)`, `crc(x)` | FNV-1a 64 / CRC-32 of `x` (as 8 big-endian octets): spreads sequential values |
| `rand(min, max)` | a random value between `min` and `max`, fixed per packet index and `stream.seed` |
| `min(a, ...)`, `max(a, ...)` | smallest / largest argument |

A reference can only name a field that has a generator in the same template; fixed field values are not visible (put them in a variable and write `${name}` instead, which is substituted before the expression is parsed). An `expr` can read another `expr` only if that one comes earlier in the template. Examples from the bundled library:

```yaml
generators:
  chaddr: { kind: expr, expr: "$0.src" }                          # copy the Ethernet source MAC (dhcp-discover-flood)
  xid: { kind: expr, expr: "rand(0, 0xffffffff)" }               # a reproducible random transaction ID
  label: { kind: expr, expr: "${srgb_base} + ${first_index} + idx" } # SR-MPLS prefix SIDs (sr-mpls-prefix-sid-sweep)
```

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
| `order` | for `nested`: fields from slowest to fastest as `<layer>.<field>`, where `<layer>` is a 0-based index, a protocol id unique in the stack (`dot1q.vid`, `arp.tpa`), or `<protocol>[<n>]` for the n-th (0-based) occurrence of a protocol that appears more than once (`ipv4[1].src` is the inner IPv4 of a tunnel) |
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
| `scenarios/` | ready-made tests: ARP sweeps, SYN flood and port scan, QinQ, NDP and RA, ping and TTL sweeps, QoS matrix, MTU probe, frame-size sweep, MAC flood, SR-MPLS prefix SID sweep, DHCP DISCOVER load |
| `interop/` | interop checks against real devices and FRR: OSPFv2 2-Way, RIPv2 routes, IS-IS adjacency Up, VRRPv3 preemption, PIM neighbor, LDP discovery, BFD session Up (`interop` tag; procedure in the operations guide, "Interop checks") |
| `malformed/` | deliberately broken packets (bad checksums, wrong lengths, truncation, bad FCS); they lint with warnings by design and are tagged `malformed` |

List them with `wpg template list templates`.

<!-- wpg-docs-gen:begin template-schema-keys -->
## Key reference (generated)

Generated from the JSON Schema of the template format (`wpg template schema`); the descriptions are the doc comments of the code (English). `Template` is the document root.

<a id="schema-template"></a>

### `Template`

A `.wpg.yaml` template: one packet (+ generators) = one stream.

| Key | Type | Required | Description |
|---|---|---|---|
| `layers` | [`TemplateLayer`](#schema-templatelayer)`[]` |   | Layer stack, outermost first. |
| `meta` | [`Meta`](#schema-meta) | yes | Metadata. |
| `packet` | [`PacketOptions`](#schema-packetoptions) |   | Frame-level options (padding, FCS). |
| `schema` | `integer` | yes | Schema version, see [`SCHEMA_VERSION`]. |
| `stream` | [`StreamOptions`](#schema-streamoptions) |   | Stream expansion options. |
| `tx` | [`TxProfile`](#schema-txprofile) |   | Transmit profile. |
| `variables` | [`Variable`](#schema-variable)`[]` |   | Template variables. |

<a id="schema-fcschoice"></a>

### `FcsChoice`

Frame check sequence choice.

| Value | Description |
|---|---|
| `"auto"` | Append the correct CRC-32. |
| `"bad"` | Append a deliberately wrong CRC-32. |
| `"none"` | No FCS (what capture / TX backends usually expect). |

<a id="schema-fieldinput"></a>

### `FieldInput`

A field value as written in YAML / JSON: a scalar in human notation or a
structured value for TLV (`options`) and list fields.

* scalars: `64`, `"192.168.1.1"`, `0x0800`, `syn|ack` (flag names), `ipv4` (enum
  variant names), `auto`. Hex byte strings must be quoted (`"00 01 02"`);
* TLV: a list of items, each a bare variant name (`nop`), a one-key map
  (`{ mss: 1460 }`, `{ timestamps: { tsval: 1, tsecr: 0 } }`) or the explicit form
  `{ type: 99, data: "01 02" }` / `{ name: mss, length: 4, fields: { value: 1 } }`;
* list: a list of scalars, or of maps for lists of field groups.

| Value | Description |
|---|---|
| `null` | Empty (`~`): e.g. an option without value. |
| `boolean` | `true` / `false` (1 / 0). |
| `integer` | Unsigned integer. |
| `integer` | Negative integer. |
| `string` | Text in human notation. |
| [`FieldInput`](#schema-fieldinput)`[]` | Sequence (TLV items, list elements). |
| `map<string, `[`FieldInput`](#schema-fieldinput)`>` | Mapping (TLV item, field group). |

<a id="schema-generator"></a>

### `Generator`

Field value generator, tagged by `kind`.

#### `kind: "range"`

| Key | Type | Required | Description |
|---|---|---|---|
| `end` | `string` \| `integer` | yes |   |
| `kind` | `"range"` | yes |   |
| `start` | `string` \| `integer` | yes |   |
| `step` | `integer` |   |   |

#### `kind: "list"`

| Key | Type | Required | Description |
|---|---|---|---|
| `kind` | `"list"` | yes |   |
| `values` | (`string` \| `integer`)`[]` | yes |   |

#### `kind: "random"`

| Key | Type | Required | Description |
|---|---|---|---|
| `kind` | `"random"` | yes |   |
| `max` | `string` \| `integer` |   |   |
| `min` | `string` \| `integer` |   |   |
| `seed` | `integer` |   |   |

#### `kind: "random_mac"`

| Key | Type | Required | Description |
|---|---|---|---|
| `kind` | `"random_mac"` | yes |   |
| `oui` | `string` |   |   |
| `seed` | `integer` |   |   |

#### `kind: "counter"`

| Key | Type | Required | Description |
|---|---|---|---|
| `kind` | `"counter"` | yes |   |
| `start` | `integer` |   |   |
| `step` | `integer` |   |   |
| `wrap` | `integer` |   |   |

#### `kind: "file"`

| Key | Type | Required | Description |
|---|---|---|---|
| `kind` | `"file"` | yes |   |
| `path` | `string` | yes |   |

#### `kind: "expr"`

| Key | Type | Required | Description |
|---|---|---|---|
| `expr` | `string` | yes |   |
| `kind` | `"expr"` | yes |   |

<a id="schema-generatorspec"></a>

### `GeneratorSpec`

A generator of a layer field: either a full [`Generator`] or the human shorthand
of the GUI (`192.168.1.1-192.168.5.254`, `10.0.0.0/24`, `1,5,9`, `00:11:22:*`).

| Value | Description |
|---|---|
| [`Generator`](#schema-generator) | Full form, e.g. `{ kind: range, start: 192.168.1.1, end: 192.168.1.254 }`. |
| `string` | Shorthand parsed with `wpg_core::notation::parse_generator`. |

<a id="schema-meta"></a>

### `Meta`

Template metadata.

| Key | Type | Required | Description |
|---|---|---|---|
| `author` | `string` \| `null` |   | Author. |
| `created` | `string` \| `null` |   | Creation timestamp (RFC 3339 or a plain date). |
| `description` | `string` \| `null` |   | Free-form description. |
| `descriptions` | `map<string, ``string``>` |   | Localised descriptions keyed by language tag (`ja`, `en`). |
| `name` | `string` | yes | Display name. |
| `tags` | `string``[]` |   | Tags for search / grouping. |

<a id="schema-packetoptions"></a>

### `PacketOptions`

Frame-level packet options.

| Key | Type | Required | Description |
|---|---|---|---|
| `fcs` | [`FcsChoice`](#schema-fcschoice) |   | Frame check sequence. |
| `pad_to_min` | `boolean` |   | Zero-pad to the minimum frame length (Ethernet: 60 octets). |

<a id="schema-scalar"></a>

### `Scalar`

A YAML scalar; variable defaults may be written as numbers.

| Value | Description |
|---|---|
| `string` |   |
| `integer` |   |
| `integer` |   |
| `boolean` |   |

<a id="schema-streammode"></a>

### `StreamMode`

How generators of several fields are combined (see `wpg_core::stream::Mode`).

| Value | Description |
|---|---|
| `"zip"` | Advance all generators in lockstep. |
| `"product"` | Cartesian product, the last generator varies fastest. |
| `"nested"` | Product with an explicit priority given by [`StreamOptions::order`]. |

<a id="schema-streamoptions"></a>

### `StreamOptions`

Stream expansion options.

| Key | Type | Required | Description |
|---|---|---|---|
| `count` | `integer` \| `null` |   | Number of packets; defaults to `tx.count`, then to the natural length. |
| `mode` | [`StreamMode`](#schema-streammode) |   | Combination mode of the generators. |
| `order` | `string``[]` |   | For `nested`: fields from the slowest to the fastest, as `<layer>.<field>` where `<layer>` is a 0-based index, a protocol id (`ipv4.src`) or the n-th occurrence of a protocol (`ipv4[1].src`). |
| `seed` | `integer` \| `null` |   | Seed for random generators. |

<a id="schema-templatelayer"></a>

### `TemplateLayer`

One layer of the template's packet.

| Key | Type | Required | Description |
|---|---|---|---|
| `fields` | `map<string, `[`FieldInput`](#schema-fieldinput)`>` |   | Manually set fields in human notation (`192.168.1.1`, `0x0800`, `syn\|ack`, `${src_ip}`). Unlisted fields use their default or Auto value; the literal `auto` also leaves a field Auto. TLV / list fields take structured values. |
| `generators` | `map<string, `[`GeneratorSpec`](#schema-generatorspec)`>` |   | Generators varying fields across the stream. |
| `protocol` | `string` | yes | Protocol id from the catalog (e.g. `ethernet2`). |

<a id="schema-txprofile"></a>

### `TxProfile`

How the stream is transmitted.

| Key | Type | Required | Description |
|---|---|---|---|
| `burst` | `integer` \| `null` |   | Burst size in packets. |
| `count` | `integer` \| `null` |   | Number of packets (`None` together with `duration_secs: None` = infinite). |
| `duration_secs` | `number` \| `null` |   | Duration limit in seconds. |
| `rate` | `string` \| `null` |   | Rate in human notation: `1000pps`, `1Mpps`, `10Gbps`, `50%`. |

<a id="schema-variable"></a>

### `Variable`

A template argument (`${name}` placeholder) overridable at run time.

| Key | Type | Required | Description |
|---|---|---|---|
| `default` | [`Scalar`](#schema-scalar) \| `null` |   | Default value (human notation; numbers and booleans are accepted too). |
| `description` | `string` \| `null` |   | Description shown in the GUI / CLI. |
| `name` | `string` | yes | Placeholder name. |
<!-- wpg-docs-gen:end template-schema-keys -->
