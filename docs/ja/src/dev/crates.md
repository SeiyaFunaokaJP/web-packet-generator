# crate 構成

リポジトリは `crates/` 配下の Cargo workspace です。ユーザーに見える文字列は [`locales/`](add-i18n.md)、サンプルは `templates/`、ドキュメントは `docs/`、E2E テストは `e2e/` と、いずれも `crates/` と同じ階層に置かれます。

## クレート一覧

| クレート | 役割 | wasm32 |
|---|---|---|
| `wpg-core` | プロトコル定義 IR、パケットの合成・解析、フィールドジェネレータ、lint 診断 (`diag::keys`)。OS 非依存 | ○ |
| `wpg-protocols` | 組み込みプロトコル定義 (Ethernet 〜 IS-IS / CLNP)。Rust マクロで `wpg-core` の IR にコンパイルされる | ○ |
| `wpg-i18n` | Fluent ベースの i18n (ja / en)。`locales/<lang>/main.ftl` を `include_str!` で埋め込む | ○ |
| `wpg-template` | テンプレートファイル形式 (`.wpg.yaml` / JSON)、スキーマのバージョン管理と (デ)シリアライズ | |
| `wpg-tx` | 送信バックエンド (`null`、`pcap`、AF_PACKET、AF_XDP、DPDK) を `TxBackend` トレイトの裏に置く | |
| `wpg-rx` | 受信側カウンタと簡易検証 (v1 は最小限) | |
| `wpg-sys` | OS の情報: ネットワークインタフェース、権限、カーネル、hugepages、Npcap / libpcap 検出、`doctor` チェック | |
| `wpg-log` | ロギング設定: tracing サブスクライバ (pretty / JSON)、ローテーションログ、ライブ配信用のブロードキャストシンク | |
| `wpg-server` | Web UI を組み込んだ axum の HTTP / WebSocket サーバ | |
| `wpg-cli` | コマンドライン本体 (`serve`、`send`、`render`、`import`、`template`、`doctor` など)。成果物は `wpg` バイナリ | |
| `wpg-web` | Leptos (CSR) の Web UI。Trunk で WASM にコンパイルする | ○ |
| `wpg-testutil` | テスト補助: pcap / pcapng の読み書き、16 進ヘルパー、golden ファイルの突合 (非公開) | |

さらに `wpg-golden-tests` が golden pcap のバイト突合と tshark 突合を担います (`cargo test -p wpg-golden-tests`)。

## wasm32 制約

`wpg-core`・`wpg-protocols`・`wpg-i18n`・`wpg-web` は Web UI のためにブラウザ (`wasm32-unknown-unknown`) でもビルドできる必要があります。この 4 クレートではファイル I/O やスレッド、OS 依存のクレートを使わないでください。CI と `just check-wasm` が次を実行して保証します。

```sh
cargo check -p wpg-core -p wpg-protocols -p wpg-i18n --target wasm32-unknown-unknown
```

送信や OS の情報取得 (`wpg-tx`、`wpg-sys`) はネイティブ専用です。合成・解析ロジックを足すときは、OS 非依存の `wpg-core` に置けないか先に検討してください。
