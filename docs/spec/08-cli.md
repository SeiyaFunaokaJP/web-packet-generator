# 08. CLI (wpg-cli)

Status: draft / 仕様 §2.7

サーバと同一バイナリ `wpg` のサブコマンドとして提供する。CI や ssh 先でのヘッドレス実行に利用する。

## コマンド一覧

```
wpg serve   [--bind 0.0.0.0:8080] [--password-file] [--tls] [--config wpg.toml]
wpg send    <template.yaml> -i eth0 [--backend afpacket] [--count N | --duration 10s] [--rate 1Mpps] [--var src_ip=...] [--vars-file vars.yaml] [--set ipv4.ttl=1] [--gen udp.src_port=range:1..100]
wpg render  <template.yaml> -o out.pcapng|- [--count N] [--json]   # 送出なし
wpg import  <in.pcap> -o template.yaml
wpg template lint|list|show [--json] / export|import
wpg ifaces  [--json]
wpg doctor                                                 # 権限・ドライバ・hugepages・Npcap 診断
wpg bench   [--backend ...]                                # 送出性能計測
```

M3 で `wpg config init` (設定ファイル雛形出力) を追加する。

## 設定ファイル (`--config`)

`--config <FILE>` (環境変数 `WPG_CONFIG`) は全サブコマンド共通のグローバルオプションで、`wpg serve --config` と `wpg --config ... serve` は同じ意味になる。

- 省略時はカレントディレクトリの `wpg.toml` があれば読み、なければ既定値で動く。明示したファイルが存在しない、または内容が不正 (構文・型・未知のキー) なら終了コード 2。カレントの `wpg.toml` が不正な場合も 2。
- テンプレートを読むコマンド (`send` / `render` / `import` / `template` / `bench` / `capture`) は `[protocols] dirs` のユーザ定義プロトコルを読み込み、`--protocols-dir` の分を後ろに追加する。Web UI で使えるテンプレートが、同じ設定で `wpg render` でもそのまま使える。
- `wpg send` は `[tx] default_backend` を `--backend` 省略時の既定にする。`[tx] max_rate` はサーバのジョブと同じ扱い: レートの指定がない (`--rate` もテンプレートの `tx.rate` もない) ときは上限レートで送り、上限を超えるレート (`unlimited` を含む) は終了コード 2 で拒否する。`null` / `pcapfile` バックエンドは対象外。
- `doctor` / `ifaces` は設定を読まない。`config` サブコマンドは従来どおり自分で読む (`wpg --config x config show` も可)。

## テンプレートの実行時上書き

テンプレートを読み込んでコンパイルするコマンド (`send` / `render` / `template show` / `bench --template`) は共通で次のオプションを持つ。読み込み後・コンパイル前に共通ヘルパ (`cmd/overrides.rs` と `wpg_template::overrides`) が適用する。

| オプション | 内容 |
|---|---|
| `--vars-file FILE` | YAML または JSON のマップ (変数名 → スカラ値。真偽値は 1 / 0)。 |
| `--var NAME=VALUE` | 変数 1 つ (複数回可)。優先順位は テンプレートの `default` < `--vars-file` < `--var`。 |
| `--set LAYER.FIELD=VALUE` | フィールドを Manual の固定値にし、そのフィールドのジェネレータを外す。値の表記は `fields:` と同じ (`${var}` も展開される)。 |
| `--gen LAYER.FIELD=GEN` | フィールドにジェネレータを付け、固定値を外す。`--set` の後に適用する。 |

- `LAYER` は `stream.order` と同じ層指定: 0 始まりの添字、プロトコル ID (最初に一致した層)、`proto[n]` (そのプロトコルの n 番目、0 始まり)。IR のフィールドは平坦なので `FIELD` はフィールド名 1 つ (TLV / リストのフィールドには生のバイト列を渡す)。
- `GEN` の短縮表記: `range:A..B[/STEP]`、`list:a,b,c`、`random`、`random:MIN..MAX`、`counter[:START[/STEP]]`、`expr:EXPR`。それ以外の文字列はテンプレートの `generators:` と同じ GUI 短縮表記 (`a-b`、`a..b`、`1,5,9`、CIDR、`00:11:22:*`) として扱う。
- 引数の形式が不正 (`=` や `.` がない、短縮表記が壊れている)、変数ファイルが読めない・マップでない場合は終了コード 2。存在しない層・フィールドは終了コード 3 で、層ならテンプレートの層一覧、フィールドなら近い名前 (なければ全フィールド) を示す。値や範囲がフィールドの型に合わない場合と、宣言されていない変数 (`--vars-file` 中のものを含む) もテンプレート検証エラー (3)。

## スクリプト向け出力 (`--json`)

JSON は標準出力に 1 つだけ出し、ローカライズしない。エラーは従来どおり標準エラーに出る。

| コマンド | JSON |
|---|---|
| `send --json` | ジョブレポート (`JobReport`) |
| `render --json` | `{"packets", "bytes" (書き出したファイルのバイト数), "path"}` |
| `template lint --json` | `{"files": [{"file", "issues": [{"level", "code", "path", "message", "args"}]}], "errors", "warnings"}`。読み込めないファイルは `code: "template-load-error"` の issue 1 件 |
| `template show --json` | `{"len", "hex", "layers": [{"protocol", "offset", "header_len", "len", "truncated", "fields": [{"name", "value", "bit_offset", "bit_len", "auto", "children"?}]}], "trailers", "diagnostics": [{"level", "code", "layer", "field", "message"}]}`。`value` はテキスト出力と同じ表記 |
| `template list --json` | `[{"name", "description", "tags", "layers", "path"}]` |
| `ifaces --json` / `doctor --json` / `bench --json` | 各コマンドの診断・計測結果 |

`wpg render -o -` は pcapng を標準出力に書き、完了メッセージは標準エラーに出す (標準出力には pcapng 以外を出さない)。`-o -` と `--json` は併用できない (終了コード 2)。

## コマンド別概要

| コマンド | 内容 | マイルストーン |
|---|---|---|
| `serve` | HTTP/WS サーバ起動 | M3 |
| `send` | ヘッドレス送出。`--var` でテンプレ変数を上書き | M3 (バックエンドは M2/M8) |
| `render` | 送出せず pcapng に書き出す (`pcapfile` バックエンド) | M3 |
| `import` | pcap/pcapng からテンプレート生成 | M5 |
| `template` | lint / list / export / import | M3, M5 |
| `ifaces` | インタフェース列挙 (`--json`) | M2 |
| `doctor` | 診断。出力とトラブルシュートの対応表を docs に置く | M2 |
| `bench` | 性能計測 (CI nightly で使用) | M2, M8 |

## 終了コード (Proposal)

0 成功 / 1 一般エラー / 2 引数エラー / 3 テンプレ検証エラー / 4 権限・環境エラー (`doctor` 系)。`--json` 出力時は機械可読。

- 2: 不正な引数・`--var` / `--set` / `--gen`、読めない変数ファイル、設定ファイルのエラー、`tx.max_rate` 超過。
- 3: テンプレートの構文・スキーマ・lint エラーで、テンプレートを読み込む全コマンド (`send` / `render` / `bench` / `template show`) と `template lint` (エラーが 1 件以上) が返す。テンプレートファイル自体を読めない場合は 1。

## 決定済み事項

- `--var` は複数回指定でき、`--vars-file` で変数ファイル (YAML / JSON) も読む。優先順位はテンプレートの既定値 < 変数ファイル < `--var` (「テンプレートの実行時上書き」参照)。

## 未決事項 (Open questions)

- `--rate` の単位表記 (`1Mpps`, `10Gbps`, `50%`) の文法。
- 終了コードの確定と、`doctor` の JSON スキーマ。
- 進捗表示 (TTY 時のプログレス、非 TTY 時のログ) の仕様。
- CLI のシェル補完・man ページ生成の要否。
- `wpg send` がサーバ経由のジョブとして実行できるか (リモート API 呼び出しモード)。
