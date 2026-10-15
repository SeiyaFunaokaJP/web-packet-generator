# ユーザ定義プロトコル (protocols.d)

組み込みにない独自プロトコルや社内プロトコルを、Rust を書かずに YAML ファイルで追加できます。追加したプロトコルは組み込みと同じ定義 IR (`wpg_core::ProtocolDef`) になるので、合成 (長さ・チェックサムの Auto を含む)、解析 (`wpg import`、受信画面のツリー)、ジェネレータ、GUI のビルダーとプレビューでそのまま使えます。

## 読み込ませ方

| 場所 | 指定 |
|---|---|
| `wpg serve` | `wpg.toml` の `[protocols] dirs = ["/etc/wpg/protocols.d"]` と `--protocols-dir <DIR>` (両方の和) |
| ほかのサブコマンド (`send` / `render` / `import` / `template lint` / `capture` …) | `--protocols-dir <DIR>` (繰り返し可) または環境変数 `WPG_PROTOCOLS_DIR` |
| Web UI | ログイン後に `GET /api/protocols/extensions` からサーバの定義を取得し、ブラウザ内のカタログに追加 |

ディレクトリ直下の `*.yaml` / `*.yml` をファイル名順に読み込みます (サブディレクトリは読みません)。読み込みは起動時の 1 回だけで、ファイルを変えたら `wpg serve` を再起動してください。定義に誤りがあるとコマンドは終了コード 2 で止まり、ファイル名と理由を表示します。`wpg config validate` も `protocols.dirs` を検査します。

```sh
wpg render examples/acme-telemetry.wpg.yaml -o acme.pcapng --protocols-dir examples/protocols.d
wpg import acme.pcapng -o acme.wpg.yaml --protocols-dir examples/protocols.d
```

## ファイル形式

1 ファイルに `protocols` (新しいプロトコル定義の一覧) と `bind` (既存プロトコルの次レイヤ表に足す項目の一覧) を書きます。どちらも省略できます。例はリポジトリの [`examples/protocols.d/acme-telemetry.yaml`](https://github.com/SeiyaFunaokaJP/web-packet-generator/blob/main/examples/protocols.d/acme-telemetry.yaml) にあります。

```yaml
protocols:
  - id: acme_tlm            # 一意な id (テンプレートの `protocol:` に書く名前)
    name: ACME Telemetry    # 表示名
    layer: l7               # パレットの分類: l2 / l3 / l4 / l7 / payload
    fields:                 # ワイヤ上の順
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
        auto: { kind: length, of: total }   # ヘッダ + 以降のオクテット数
      - name: readings
        type: { kind: bytes, len: { kind: to_end } }

bind:
  - under: udp          # 下位プロトコル
    value: 47000        # その選択フィールド (UDP は dst_port、解析では src_port も) の値
    protocol: acme_tlm
```

### プロトコル定義

各定義の形は `GET /api/protocols/{id}` が返す JSON と同じです。既存のプロトコルの定義を参考にするときは、その API の出力をそのまま YAML にできます。

- フィールド型 (`type.kind`): `uint` / `int` (`bits`)、`enum` (`bits`, `variants`)、`flags` (`bits`, `flags`)、`bytes` (`len`)、`mac`、`ipv4`、`ipv6`、`nsap`、`cstring`、`tlv`、`list`、`padding`、`raw`。
- 既定値 (`default`) は `{ uint: 1 }`、`{ bytes: [0, 1] }` のように型名 1 つのマップで書きます。
- 自動計算 (`auto`): 長さ (`kind: length`, `of: header | payload | total`)、チェックサム (`kind: checksum`, `algorithm: internet | internet_pseudo_header | fletcher | crc32 | crc32c …`)、次ヘッダ (`kind: next_protocol`) など。
- 次レイヤの選択 (`next`): このプロトコルの上に別のプロトコルを載せるときに `field` と `table` (`value` → `protocol`) を書きます。

定義は読み込み時に組み込みと同じ検査 (ビット幅、オクテット境界、Auto の対象など) を受け、知らないキーはエラーになります。

### 結び付け (`bind`)

| キー | 意味 |
|---|---|
| `under` | 下位プロトコルの id (`udp`、`tcp`、`ethernet2`、`dot1q`、`ipv4` …) |
| `value` / `max` | 選択フィールドの値 (`max` を書くと `value..=max` の範囲) |
| `protocol` | 選ばれるプロトコルの id (組み込みでも拡張でも可) |
| `field` | 下位プロトコルにまだ次レイヤ表がないときだけ必要な選択フィールド名。表があるときは、その選択フィールドと一致している必要があります |

EtherType で結び付けるときは、`ethernet2` と `dot1q` (VLAN の後) のように必要な下位レイヤごとに書きます。

## 制約

- 組み込みプロトコルの id は使えず、組み込みの定義は変更できません (同じ id は 2 つ目のファイルでもエラー)。
- 下位プロトコルがすでに別のプロトコルに割り当てている値 (例: UDP 4789 = VXLAN) には結び付けられません。
- UDP / TCP のポートは Auto ではないため、テンプレートでは `dst_port` を明示してください (`bind` は解析と GUI の候補表示に使われます)。
- フィールドの説明 (Fluent の `description_key`) は組み込み用です。ユーザ定義プロトコルでは省略してください。
- プロトコルリファレンス (`wpg docs gen`) は組み込みプロトコルだけを対象にします。

<!-- wpg-docs-gen:begin reference-links -->
## 自動生成リファレンス

このページのプロトコルのフィールドリファレンス (定義から自動生成):

[`ethernet2`](../reference/protocols/ethernet2.md) · [`dot1q`](../reference/protocols/dot1q.md) · [`ipv4`](../reference/protocols/ipv4.md) · [`tcp`](../reference/protocols/tcp.md) · [`udp`](../reference/protocols/udp.md) · [`ipv6`](../reference/protocols/ipv6.md) · [`raw`](../reference/protocols/raw.md)
<!-- wpg-docs-gen:end reference-links -->
