# テンプレートスキーマ

テンプレートは 1 つのストリームを記述する YAML ファイル (`.wpg.yaml`。`.wpg.json` の JSON も同等) です。レイヤスタック、フィールドを変化させるジェネレータ、送出プロファイルから成ります。機械可読な定義は [`docs/schema/template.schema.json`](https://github.com/SeiyaFunaokaJP/web-packet-generator/blob/main/docs/schema/template.schema.json) で、`wpg template schema` でも出力できます。未知のキーはエラーになるのでタイプミスを検出できます。ファイルの検査は `wpg template lint FILE` です。

```yaml
schema: 1                  # 必須
meta: { ... }              # 必須
variables: [ ... ]         # 任意
layers: [ ... ]            # 必須。外側から内側の順
packet: { ... }            # 任意
stream: { ... }            # 任意
tx: { ... }                # 任意
```

## `meta`

| キー | 意味 |
|---|---|
| `name` | 必須の表示名。ライブラリ内で一意 (同梱ライブラリはテストで確認) |
| `description` | 自由記述 |
| `descriptions` | 言語別の説明: `ja:` と `en:` |
| `tags` | 検索用タグ。同梱ライブラリは `protocol:<id>`、`layer:l2\|l3\|l4`、`scenario`、`malformed` と話題語を使う |
| `author`, `created` | 自由記述。`created` は日付または RFC 3339 の時刻 |

## `variables`

テンプレート引数です。各項目は `name`、任意の `default` (文字列・数値・真偽値)、`description` を持ちます。フィールド値やジェネレータの中で `${name}` と書き、実行時に `--var name=value` (`wpg send`、`wpg render`) や GUI で上書きします。既定値も上書きも無い変数はエラーで、宣言の無い `${…}` もエラーです。宣言だけで使われていない変数は警告になります。

## `layers`

各項目は `protocol` (カタログの ID。[プロトコルリファレンス](protocols.md) を参照)、`fields`、`generators` から成ります。

### `fields`

書いたフィールドは **Manual**、書かない (または `auto` にした) フィールドは既定値になるか、計算されます (長さ、チェックサム、次プロトコル番号)。値は人間向け表記です。

| 型 | 例 |
|---|---|
| 整数 | `64`、`0x0800`、`0b101` |
| MAC / IPv4 / IPv6 | `02:00:00:00:00:01`、`192.0.2.1`、`2001:db8::1` (`"00:11:22:33:44:55"` のように数字だけの MAC は YAML が数値と解釈しうるので引用符で囲む) |
| バイト列 | `"00 01 02 03"` (必ず引用符)。テキストは `'"text"'` (`\r`、`\n`、`\t` のエスケープ可) |
| テキスト行 (CRLF で終わる SIP の行) | `start_line: '"INVITE sip:bob@example.com SIP/2.0"'` (終端は自動で付く) |
| enum | 数値かバリアント名: `ethertype: arp`、`oper: request`、`type: echo_request` |
| flags | ビット名を `\|` で連結: `flags: syn\|ack`、`flags: df` |
| TLV リスト (TCP/IPv4 オプション) | `- mss: 1460`、`- sack_permitted`、`- nop`、`- timestamps: { tsval: 1, tsecr: 0 }`、`- { type: 99, data: "01 02" }` |
| リスト / TLV リストをそのままのオクテット列で | `nlri: "18 c6 33 64"` (リストの代わりに引用符付きの 16 進文字列。インポートがデコードできない内容をこの形で書く) |

わざと誤った値も受け付けます。Manual の長さやチェックサムは書いたとおりに使われ、`wpg template lint` は不一致を警告 (`diag-*`) として報告します。`templates/malformed/` のサンプルはこの仕組みで作っています。

### `generators`

フィールド名からジェネレータへの対応です。完全形か、GUI と同じ短縮表記で書きます。

| `kind` | パラメータ | 短縮表記 | 意味 |
|---|---|---|---|
| `range` | `start`、`end`、`step` (既定 1) | `192.168.1.1-192.168.5.254`、`1-1024` | 両端を含むスイープ。件数が多いと先頭に戻る |
| `range` (CIDR) | | `10.0.0.0/22` | ブロックのホストアドレス |
| `list` | `values` | `1,5,9` | 値を順に繰り返す |
| `random` | `min`、`max`、`seed` | | 一様乱数 (整数、IPv4、IPv6) |
| `random_mac` | `oui`、`seed` | `00:11:22:*` | ランダムな MAC (上位を固定可) |
| `counter` | `start`、`step`、`wrap` | | 増加する値。`wrap` が無ければ無限 |
| `file` | `path` | | ファイルから読む値 |
| `expr` | `expr` | | 計算式による値 |

同じフィールドに固定値とジェネレータを書くと lint が警告します。

### 式 (`expr`)

`expr` ジェネレータはパケットごとに整数を計算します。結果は他の値と同じようにフィールドに書き込まれるので、数値のほか IPv4 / IPv6 アドレスや MAC アドレスも埋められます。

| 要素 | 意味 |
|---|---|
| `12`、`0x1f`、`0b101` | リテラル (10 進、16 進、2 進) |
| `idx` | ストリーム内のパケット番号 (0、1、2、…) |
| `$field`、`$<layer>.field` | 同じパケットでの、別の *ジェネレータ付き* フィールドの値。`<layer>` の書き方は `stream.order` と同じ (`$0.src` と `$ethernet2.src` はどちらも先頭レイヤの `src` ジェネレータ、`$ipv4[1].src` は 2 つ目の `ipv4`) |
| `+ - * / %`、`<< >>`、`& ^ \|`、単項 `- ~` | C と同じ演算子と優先順位。整数演算 (128 ビット、桁あふれは折り返し) |
| `hash(x)`、`crc(x)` | `x` (8 オクテットのビッグエンディアン) の FNV-1a 64 / CRC-32。連番を散らすのに使う |
| `rand(min, max)` | `min` から `max` までの乱数。パケット番号と `stream.seed` で決まる |
| `min(a, ...)`、`max(a, ...)` | 引数の最小 / 最大 |

参照できるのは、同じテンプレートでジェネレータを持つフィールドだけです。固定値のフィールドは見えません (変数にして `${name}` と書けば、式の解析より前に置き換わります)。`expr` が別の `expr` を参照できるのは、参照先がテンプレート内で先にある場合だけです。同梱ライブラリの例:

```yaml
generators:
  chaddr: { kind: expr, expr: "$0.src" }                          # Ethernet の送信元 MAC を写す (dhcp-discover-flood)
  xid: { kind: expr, expr: "rand(0, 0xffffffff)" }               # 再現可能なランダムのトランザクション ID
  label: { kind: expr, expr: "${srgb_base} + ${first_index} + idx" } # SR-MPLS の Prefix SID (sr-mpls-prefix-sid-sweep)
```

## `packet`

| キー | 既定 | 意味 |
|---|---|---|
| `pad_to_min` | `true` | Ethernet の最小 60 オクテット (FCS 除く) までゼロ埋め |
| `fcs` | `none` | `auto` は正しい CRC-32、`bad` は誤った CRC-32 を付け、`none` は付けない |

## `stream`

ジェネレータの組み合わせ方とストリームの長さです。

| キー | 意味 |
|---|---|
| `mode` | `zip` (既定。同時に進める)、`product` (最後が最速)、`nested` |
| `order` | `nested` 用。遅い方から速い方へ `<レイヤ>.<フィールド>` で列挙。`<レイヤ>` は 0 始まりの添字、スタック内で一意なプロトコル ID (`dot1q.vid`、`arp.tpa`)、または同じプロトコルが複数あるときの `<プロトコル>[<n>]` (0 始まりで n 番目。`ipv4[1].src` はトンネル内側の IPv4) |
| `count` | パケット数。既定は `tx.count`、無ければジェネレータの自然な長さ |
| `seed` | ランダムジェネレータの seed。ストリームを再現可能にする |

無限のジェネレータ (`wrap` 無しの `counter`) には `count` が必要で、`product` や `nested` の内側には置けません。

## `tx`

| キー | 意味 |
|---|---|
| `count` | 送信パケット数 (`count` も `duration_secs` も無ければ停止するまで) |
| `duration_secs` | 秒単位の時間制限 |
| `rate` | `1000pps`、`1Mpps`、`10Gbps`、`50%`、`ifg=10us` |
| `burst` | 連続送信するフレーム数 |

`wpg send` のコマンドラインオプションが優先されます。

## payload レイヤ

`payload` は実際のプロトコルではなく、フィールドが生成を制御します。`mode` (`fixed`、`pattern`、`random`、`counter`)、`data`、`length`、`seed`、`counter` モードでは `counter_offset`、`counter_width` (`u32` または `u64`)、`counter_value` (通常は `counter` ジェネレータで与える) です。`raw` は指定したバイト列をそのまま置きます。

## lint の検査項目

| キー | 重大度 | 意味 |
|---|---|---|
| `lint-schema-version` | error | 未対応の `schema` |
| `lint-missing-name` | warning | `meta.name` が空 |
| `lint-no-layers` | error | レイヤなし |
| `lint-unknown-protocol`, `lint-unknown-field` | error | カタログにない |
| `lint-invalid-value`, `lint-invalid-generator` | error | 値やジェネレータがフィールド型に合わない |
| `lint-unknown-variable`, `lint-missing-variable` | error | 宣言のない変数 / 値のない変数 |
| `lint-unused-variable` | warning | 宣言だけで未使用 |
| `lint-next-protocol-manual` | warning | 次レイヤが前のレイヤのバインディング表になく、次プロトコルのフィールドを Auto にできない |
| `lint-field-and-generator` | warning | 同じフィールドに固定値とジェネレータ |
| `diag-*` | error / warning | 構築したパケットの診断 (計算値と異なるチェックサムや長さなど) |

## 同梱ライブラリ

`templates/` は既定でライブラリとして読み込まれ、`crates/wpg-template/tests/samples.rs` で検証されています。

| フォルダ | 内容 |
|---|---|
| `examples/` | 小さな出発点 5 本 |
| `protocols/l2`, `l3`, `l4`, `payload`, `frame` | カタログの全プロトコルのサンプル。ジャンボ、最小サイズ、ランツ、FCS の例を含む |
| `scenarios/` | すぐ使える試験: ARP スイープ、SYN フラッドとポートスキャン、QinQ、NDP と RA、ping と TTL スイープ、QoS マトリクス、MTU 探索、フレーム長スイープ、MAC フラッド、SR-MPLS Prefix SID スイープ、DHCP DISCOVER 負荷 |
| `interop/` | 実機・FRR との相互接続確認: OSPFv2 2-Way、RIPv2 経路、IS-IS 隣接 Up、VRRPv3 プリエンプト、PIM ネイバー、LDP ディスカバリ、BFD セッション Up (`interop` タグ。手順は運用ガイドの「相互接続確認」) |
| `malformed/` | わざと壊したパケット (チェックサム不正、長さ不一致、切り詰め、FCS 不正)。設計上 lint が警告を出し、`malformed` タグが付く |

一覧は `wpg template list templates` で表示できます。

<!-- wpg-docs-gen:begin template-schema-keys -->
## キーリファレンス (自動生成)

テンプレート形式の JSON Schema (`wpg template schema`) から自動生成しています。説明文はコードの doc コメント (英語) です。`Template` が文書のルートです。

<a id="schema-template"></a>

### `Template`

A `.wpg.yaml` template: one packet (+ generators) = one stream.

| キー | 型 | 必須 | 説明 |
|---|---|---|---|
| `layers` | [`TemplateLayer`](#schema-templatelayer)`[]` |   | Layer stack, outermost first. |
| `meta` | [`Meta`](#schema-meta) | はい | Metadata. |
| `packet` | [`PacketOptions`](#schema-packetoptions) |   | Frame-level options (padding, FCS). |
| `schema` | `integer` | はい | Schema version, see [`SCHEMA_VERSION`]. |
| `stream` | [`StreamOptions`](#schema-streamoptions) |   | Stream expansion options. |
| `tx` | [`TxProfile`](#schema-txprofile) |   | Transmit profile. |
| `variables` | [`Variable`](#schema-variable)`[]` |   | Template variables. |

<a id="schema-fcschoice"></a>

### `FcsChoice`

Frame check sequence choice.

| 値 | 説明 |
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

| 値 | 説明 |
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

| キー | 型 | 必須 | 説明 |
|---|---|---|---|
| `end` | `string` \| `integer` | はい |   |
| `kind` | `"range"` | はい |   |
| `start` | `string` \| `integer` | はい |   |
| `step` | `integer` |   |   |

#### `kind: "list"`

| キー | 型 | 必須 | 説明 |
|---|---|---|---|
| `kind` | `"list"` | はい |   |
| `values` | (`string` \| `integer`)`[]` | はい |   |

#### `kind: "random"`

| キー | 型 | 必須 | 説明 |
|---|---|---|---|
| `kind` | `"random"` | はい |   |
| `max` | `string` \| `integer` |   |   |
| `min` | `string` \| `integer` |   |   |
| `seed` | `integer` |   |   |

#### `kind: "random_mac"`

| キー | 型 | 必須 | 説明 |
|---|---|---|---|
| `kind` | `"random_mac"` | はい |   |
| `oui` | `string` |   |   |
| `seed` | `integer` |   |   |

#### `kind: "counter"`

| キー | 型 | 必須 | 説明 |
|---|---|---|---|
| `kind` | `"counter"` | はい |   |
| `start` | `integer` |   |   |
| `step` | `integer` |   |   |
| `wrap` | `integer` |   |   |

#### `kind: "file"`

| キー | 型 | 必須 | 説明 |
|---|---|---|---|
| `kind` | `"file"` | はい |   |
| `path` | `string` | はい |   |

#### `kind: "expr"`

| キー | 型 | 必須 | 説明 |
|---|---|---|---|
| `expr` | `string` | はい |   |
| `kind` | `"expr"` | はい |   |

<a id="schema-generatorspec"></a>

### `GeneratorSpec`

A generator of a layer field: either a full [`Generator`] or the human shorthand
of the GUI (`192.168.1.1-192.168.5.254`, `10.0.0.0/24`, `1,5,9`, `00:11:22:*`).

| 値 | 説明 |
|---|---|
| [`Generator`](#schema-generator) | Full form, e.g. `{ kind: range, start: 192.168.1.1, end: 192.168.1.254 }`. |
| `string` | Shorthand parsed with `wpg_core::notation::parse_generator`. |

<a id="schema-meta"></a>

### `Meta`

Template metadata.

| キー | 型 | 必須 | 説明 |
|---|---|---|---|
| `author` | `string` \| `null` |   | Author. |
| `created` | `string` \| `null` |   | Creation timestamp (RFC 3339 or a plain date). |
| `description` | `string` \| `null` |   | Free-form description. |
| `descriptions` | `map<string, ``string``>` |   | Localised descriptions keyed by language tag (`ja`, `en`). |
| `name` | `string` | はい | Display name. |
| `tags` | `string``[]` |   | Tags for search / grouping. |

<a id="schema-packetoptions"></a>

### `PacketOptions`

Frame-level packet options.

| キー | 型 | 必須 | 説明 |
|---|---|---|---|
| `fcs` | [`FcsChoice`](#schema-fcschoice) |   | Frame check sequence. |
| `pad_to_min` | `boolean` |   | Zero-pad to the minimum frame length (Ethernet: 60 octets). |

<a id="schema-scalar"></a>

### `Scalar`

A YAML scalar; variable defaults may be written as numbers.

| 値 | 説明 |
|---|---|
| `string` |   |
| `integer` |   |
| `integer` |   |
| `boolean` |   |

<a id="schema-streammode"></a>

### `StreamMode`

How generators of several fields are combined (see `wpg_core::stream::Mode`).

| 値 | 説明 |
|---|---|
| `"zip"` | Advance all generators in lockstep. |
| `"product"` | Cartesian product, the last generator varies fastest. |
| `"nested"` | Product with an explicit priority given by [`StreamOptions::order`]. |

<a id="schema-streamoptions"></a>

### `StreamOptions`

Stream expansion options.

| キー | 型 | 必須 | 説明 |
|---|---|---|---|
| `count` | `integer` \| `null` |   | Number of packets; defaults to `tx.count`, then to the natural length. |
| `mode` | [`StreamMode`](#schema-streammode) |   | Combination mode of the generators. |
| `order` | `string``[]` |   | For `nested`: fields from the slowest to the fastest, as `<layer>.<field>` where `<layer>` is a 0-based index, a protocol id (`ipv4.src`) or the n-th occurrence of a protocol (`ipv4[1].src`). |
| `seed` | `integer` \| `null` |   | Seed for random generators. |

<a id="schema-templatelayer"></a>

### `TemplateLayer`

One layer of the template's packet.

| キー | 型 | 必須 | 説明 |
|---|---|---|---|
| `fields` | `map<string, `[`FieldInput`](#schema-fieldinput)`>` |   | Manually set fields in human notation (`192.168.1.1`, `0x0800`, `syn\|ack`, `${src_ip}`). Unlisted fields use their default or Auto value; the literal `auto` also leaves a field Auto. TLV / list fields take structured values. |
| `generators` | `map<string, `[`GeneratorSpec`](#schema-generatorspec)`>` |   | Generators varying fields across the stream. |
| `protocol` | `string` | はい | Protocol id from the catalog (e.g. `ethernet2`). |

<a id="schema-txprofile"></a>

### `TxProfile`

How the stream is transmitted.

| キー | 型 | 必須 | 説明 |
|---|---|---|---|
| `burst` | `integer` \| `null` |   | Burst size in packets. |
| `count` | `integer` \| `null` |   | Number of packets (`None` together with `duration_secs: None` = infinite). |
| `duration_secs` | `number` \| `null` |   | Duration limit in seconds. |
| `rate` | `string` \| `null` |   | Rate in human notation: `1000pps`, `1Mpps`, `10Gbps`, `50%`. |

<a id="schema-variable"></a>

### `Variable`

A template argument (`${name}` placeholder) overridable at run time.

| キー | 型 | 必須 | 説明 |
|---|---|---|---|
| `default` | [`Scalar`](#schema-scalar) \| `null` |   | Default value (human notation; numbers and booleans are accepted too). |
| `description` | `string` \| `null` |   | Description shown in the GUI / CLI. |
| `name` | `string` | はい | Placeholder name. |
<!-- wpg-docs-gen:end template-schema-keys -->
