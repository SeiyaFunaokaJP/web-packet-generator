# 05. テンプレート (wpg-template)

Status: M5 (実装済み: wpg-template) / 元: TODO.md §2.4

## 概要

- 形式: YAML (人が読める、Git で diff できる)。JSON も同等に読み書き可。拡張子は `.wpg.yaml` / `.wpg.json` ([ADR-0008](../adr/0008-naming.md))。
- スキーマバージョニング: `schema: 1`。読み込み時に `migrate()` が古い版を現行版へ変換する (現状は 1 のみ)。より新しい版は `UnsupportedSchema` で拒否する。
- 1 テンプレート = 1 ストリーム (展開結果 + 送出プロファイル)。
- JSON Schema: [`docs/schema/template.schema.json`](../schema/template.schema.json) (`wpg template schema` でも出力。`WPG_UPDATE_GOLDEN=1 cargo test -p wpg-template` で再生成)。
- 未知のキーはエラーになる (タイプミスの検出)。

## トップレベル構造

| キー | 必須 | 内容 |
|---|---|---|
| `schema` | 必須 | スキーマバージョン (整数) |
| `meta` | 必須 | `name` / `description` / `descriptions` (`ja`, `en`) / `tags` / `author` / `created` |
| `variables` | 任意 | テンプレ引数。`name` / `default` / `description` の配列 |
| `layers` | 必須 | レイヤ配列 (`protocol` + `fields` + `generators`)。外側から内側の順 |
| `packet` | 任意 | フレーム設定: `pad_to_min` (既定 true)、`fcs` (`auto` / `bad` / `none`、既定 `none`) |
| `stream` | 任意 | ストリーム展開: `mode` (`zip` / `product` / `nested`)、`order`、`count`、`seed` |
| `tx` | 任意 | 送出プロファイル: `count` / `duration_secs` / `rate` / `burst` |

### フィールド値 (`layers[].fields`)

人間向け表記の文字列 (または数値) で書く。書いたフィールドは Manual、書かなかったもの (または `auto`) は既定値 / Auto。

- 数値: `64`, `0x0800`, `0b101`。IPv4/IPv6/MAC/NSAP/バイト列 (`"00 01 02"`、16 進は必ず引用符で囲む)。
- enum: 数値のほか **バリアント名** (`ethertype: arp`, `oper: request`)。
- flags: **ビット名を `|` で連結** (`flags: syn|ack`, `flags: df`)。
- TLV (TCP オプション等): 項目のリスト。`nop` / `{ mss: 1460 }` / `{ timestamps: { tsval: 1, tsecr: 0 } }` / 明示形 `{ type: 99, data: "01 02" }`, `{ name: mss, length: 4, fields: { value: 1 } }`。
- 変数参照: `${name}` (文字列内の任意の位置)。値は `variables[].default`、実行時は `--var name=value` で上書き。宣言のない変数・値のない変数はエラー。

### ジェネレータ (`layers[].generators`)

フィールド名 → ジェネレータ。完全形 `{ kind: range|list|random|random_mac|counter|file|expr, ... }` か、GUI と同じ短縮表記の文字列 (`192.168.1.1-192.168.5.254`, `10.0.0.0/24`, `1,5,9`, `00:11:22:*`)。数値の `min` / `max` / `start` / `end` はそのまま書ける。enum / flags は名前でも指定できる。

### ストリーム (`stream`)

複数ジェネレータの合成モードをここに書く。`nested` では `order` にフィールドを遅い方から速い方へ `<レイヤ>.<フィールド>` で列挙する (`<レイヤ>` は 0 始まりの添字か、スタック内で一意なプロトコル ID: `ipv4.src`)。`count` の既定は `tx.count`、無ければジェネレータの自然な長さ。無限のジェネレータ (counter 等) は `count` が必要で、product / nested の内側には置けない。

コンパイル結果の `StreamSpec` では `FieldRef.layer` にレイヤ添字 (10 進文字列) が入る。

## 例: Ethernet / IPv4 / UDP (src IP に range ジェネレータ)

`templates/examples/eth-ipv4-udp-range.wpg.yaml` と同一 (テストで全サンプルのパース・lint・レンダリングを検証している)。

```yaml
# Ethernet / IPv4 / UDP whose source IP sweeps 192.168.1.1 - 192.168.5.254.
schema: 1

meta:
  name: udp-src-sweep
  descriptions:
    ja: 送信元 IP を 192.168.1.1-192.168.5.254 で変化させる UDP パケット
    en: UDP packets whose source IP sweeps 192.168.1.1-192.168.5.254
  tags: [udp, sample, l3]
  author: example
  created: 2026-09-29

# Overridable at run time: wpg render / send --var dst_ip=10.1.2.3
variables:
  - name: dst_ip
    default: 10.0.0.1
    description: Destination IPv4 address
  - name: dst_port
    default: 5001
    description: Destination UDP port

layers:
  - protocol: ethernet2
    fields:
      dst: ff:ff:ff:ff:ff:ff
      src: "00:11:22:33:44:55"
      ethertype: auto # announces the next layer (IPv4)

  - protocol: ipv4
    fields:
      dst: ${dst_ip}
      ttl: 64
      dscp: 0
    generators:
      src:
        kind: range
        start: 192.168.1.1
        end: 192.168.5.254
        step: 1

  - protocol: udp
    fields:
      src_port: 12345
      dst_port: ${dst_port}

  - protocol: payload
    fields:
      mode: pattern # repeat `data` up to `length` octets
      data: "00 01 02 03"
      length: 64

stream:
  mode: zip # product | nested (+ order) combine several generators

tx:
  count: 1278 # one pass over the range
  rate: 100000pps
  burst: 32
```

### 補足

- range は 192.168.1.1 から 192.168.5.254 まで 1 ずつ増加する (1278 通り)。終端に達した後は先頭に戻る (`tx.count` が自然な長さより大きい場合)。
- `auto` は Auto/Manual トグルの Auto に対応。値を書けば Manual として扱う。
- `payload` の `mode: pattern` は `data` を `length` バイトまで繰り返す。
- 同梱サンプル: `templates/examples/` に P0 プロトコルを網羅する 5 本 (`eth-ipv4-udp-range`, `arp-sweep`, `vlan-tcp-syn`, `ipv6-ndp-ns`, `icmp-echo-pattern-payload`)。

## CLI

- `wpg template lint <files…>`: スキーマ・プロトコル・フィールド・値・ジェネレータ型・変数・バインディング・ビルド診断を検査。エラーがあれば終了コード 1。
- `wpg template list [dir]`: 名前・タグ・プロトコルスタックを一覧 (既定 `templates/`)。
- `wpg template show <file> [--var k=v]`: 1 パケット目の Hex ダンプとレイアウトツリーを表示。
- `wpg template schema`: JSON Schema を出力。
- `wpg render <template> -o out.pcapng [--count N] [--var k=v]`: 送出せず pcapng に書き出す。件数は `--count` > `stream.count` / `tx.count` > ジェネレータの自然な長さ。無限ストリームで `--count` が無ければ拒否する。

## lint の検査項目

| キー | 重大度 | 内容 |
|---|---|---|
| `lint-schema-version` | error | 未対応の `schema` |
| `lint-missing-name` | warning | `meta.name` が空 |
| `lint-no-layers` | error | レイヤなし |
| `lint-unknown-protocol` / `lint-unknown-field` | error | カタログにないプロトコル / フィールド |
| `lint-invalid-value` | error | 値が型に合わない (範囲外・書式・未知の名前) |
| `lint-invalid-generator` | error | ジェネレータがフィールド型に合わない |
| `lint-unknown-variable` / `lint-missing-variable` | error | 宣言のない変数 / 値のない変数 (compile 時) |
| `lint-unused-variable` | warning | 使われていない変数 |
| `lint-next-protocol-manual` | warning | 次レイヤが上位のバインディング表にない (next-protocol が Manual になる) |
| `lint-field-and-generator` | warning | 同じフィールドに固定値とジェネレータ |
| `diag-*` | error/warning | `Packet::build` の診断 (チェックサム不一致など) |

## 管理機能

フォルダ/タグ、検索、複製、バージョン履歴 (簡易)、お気に入り、最近使った。ファイル実体は YAML でディスクに置き、履歴等のメタは SQLite ([ADR-0003](../adr/0003-database.md))。

## 共有

- 単体エクスポート: `.wpg.yaml`
- 複数まとめ: `.wpgbundle` (zip: templates + README + 添付 pcap)
- URL 共有 (サーバ内リンク)
- Git 連携: テンプレートディレクトリを任意パスに置くだけで読み込む「ライブラリパス」を複数指定可能
- 同梱ライブラリ: プロトコル別サンプル、代表シナリオ (「VLAN 越し ARP スイープ」「IS-IS L2 IIH フラッド」「DHCP Discover 大量」)

## 検証

`wpg template lint <file>` でスキーマ・制約チェック。JSON Schema は `docs/schema/template.schema.json`。

## 未決事項 (Open questions)

- `layers` 内の入れ子表現 (トンネルの内側パケット、TLV/list の記述形式)。
- `variables` の型システム (プロトコルのフィールド型と共通化するか) と、実行時オーバーライドの文法。
- `.wpgbundle` 内のディレクトリ構造、マニフェストの有無、添付 pcap の参照方法。
- `wpg template migrate` コマンド (読み込み時変換は実装済み)。
- Git ライブラリパスの競合 (同名テンプレート) の解決規則と、ホットリロードの検出方法。
