# 01. 全体アーキテクチャ

Status: draft / 元: TODO.md §1

## 構成図

```
┌──────────────────────────────────────────────────────────────┐
│ Browser (WASM: Leptos)                                       │
│  ・パケットビルダー GUI（レイヤ D&D、フィールド編集、Hex/ツリー同期表示）│
│  ・テンプレート管理、ジョブ制御、統計ダッシュボード、i18n           │
└───────────────▲──────────────────────────────▲───────────────┘
                │ REST (JSON)                  │ WebSocket (統計/ログ/進捗)
┌───────────────┴──────────────────────────────┴───────────────┐
│ wpg-server (axum + tokio)  … 単一バイナリ、静的資産埋め込み        │
│  ・認証 (password / session)  ・テンプレート CRUD  ・ジョブ管理      │
│  ・API スキーマ (OpenAPI 自動生成)                                 │
├──────────────────────────────────────────────────────────────┤
│ wpg-core (pure Rust, no_std 寄り)                                │
│  ・プロトコル定義エンジン（フィールド型、長さ/チェックサム自動計算）    │
│  ・パケット合成 (build) / 解析 (parse: pcap→テンプレ逆生成)        │
│  ・変数/インクリメント/ランダム生成、ストリーム展開                  │
├──────────────────────────────────────────────────────────────┤
│ wpg-tx (送出エンジン: TxBackend trait)                           │
│  ・pcap/Npcap  ・AF_PACKET TPACKET_V3 / sendmmsg  ・AF_XDP        │
│  ・DPDK (feature)  ・pcap ファイル出力 (ドライラン / 共有)          │
└──────────────────────────────────────────────────────────────┘
```

### 設計上の要点

- `wpg-core` はブラウザ (WASM) とサーバの双方でビルドし、プレビュー結果とサーバ側合成結果の一致を同一コードで保証する。
- 通信は REST (JSON) が操作系、WebSocket が統計・ログ・進捗の配信系 (統計は 100ms 間隔)。
- 送出は `TxBackend` trait で抽象化し、バックエンドは feature フラグで選択する。

## 1.1 crate 構成 (Cargo workspace)

```
web-packet-generator/
├── Cargo.toml                 # workspace
├── crates/
│   ├── wpg-core/              # プロトコル定義・合成・解析・変数展開（OS 非依存、WASM でもビルド可）
│   ├── wpg-protocols/         # 各プロトコル定義（Ethernet, IPv4, ..., IS-IS, CLNP ...）
│   ├── wpg-tx/                # 送出バックエンド抽象 + 実装（feature: pcap, afpacket, afxdp, dpdk）
│   ├── wpg-rx/                # 受信・カウンタ・簡易検証（v1 は最小）
│   ├── wpg-template/          # テンプレートファイル形式 (YAML/JSON)、スキーマ、import/export、バンドル
│   ├── wpg-server/            # axum HTTP/WS サーバ、認証、ジョブ管理、静的資産埋め込み
│   ├── wpg-cli/               # CLI（サーバ起動 / ヘッドレス送出 / テンプレ検証 / pcap 変換）
│   └── wpg-web/               # Leptos (CSR) フロントエンド → WASM
├── locales/                   # Fluent (.ftl) — ja / en
├── scripts/                   # build / run / deploy / release スクリプト
├── docs/                      # mdBook（ja / en）
├── templates/                 # 同梱テンプレート（プロトコル別サンプル）
└── tests/                     # 統合テスト、golden pcap
```

### 依存方向 (Proposal)

`wpg-web`, `wpg-server`, `wpg-cli` → `wpg-template`, `wpg-tx`, `wpg-rx` → `wpg-protocols` → `wpg-core`。`wpg-core` は他 crate に依存しない。`dpdk-sys` は `wpg-tx` の optional 依存として workspace 内に追加する ([ADR-0004](../adr/0004-dpdk-bindings.md))。

## 1.2 技術選定

| 領域 | 選定 | 理由 / 備考 |
|---|---|---|
| Web サーバ | `axum` + `tokio` + `tower-http` | エコシステム最大、WS 対応、ミドルウェア豊富 |
| API 定義 | `utoipa` (OpenAPI) | フロント/CLI/外部連携で型共有。Swagger UI 同梱 |
| フロント | `leptos` (CSR) + Tailwind CSS | Rust 一貫、WASM ([ADR-0002](../adr/0002-frontend.md)) |
| D&D | HTML5 DnD API を Leptos で薄くラップ (自作) | 依存を減らす。必要なら `web-sys` 直叩き |
| Hex 表示 | 自作コンポーネント (仮想スクロール) | 1500〜9000B の同期ハイライトが必要 |
| 状態共有 | `wpg-core` を WASM でもビルド | サーバ往復なしで即時プレビュー |
| シリアライズ | `serde` + `serde_yaml` / `serde_json` | テンプレ YAML、API JSON |
| i18n | `fluent` (`fluent-bundle`, `rust-i18n` 検討) | 複数形/性別対応、ja/en の .ftl |
| 認証 | `argon2` + `tower-sessions` (Cookie) | TLS は `rustls` オプション |
| 静的資産埋め込み | `rust-embed` | 単一バイナリ配布 |
| 送出 (共通) | `pcap` crate (libpcap / Npcap / BPF) | Windows・macOS の主経路 |
| 送出 (Linux 高速) | `libc` 直叩き AF_PACKET TPACKET_V3 + `sendmmsg`、AF_XDP は `xsk-rs` | 依存を最小化しつつ Mpps 級 |
| 送出 (最高速) | DPDK 24.11 LTS (自前 `dpdk-sys`) | feature = "dpdk"。EAL 引数は設定ファイルから |
| ログ | `tracing` + `tracing-subscriber` | JSON ログ/ローテーション |
| 設定 | `figment` or `config` (TOML + env + CLI) | `wpg.toml` |
| DB | テンプレ = YAML ファイル、履歴/ユーザ/セッション = SQLite (`rusqlite` bundled) | [ADR-0003](../adr/0003-database.md) |
| テスト | `cargo test`, `proptest`, golden pcap, `tshark -V` | プロトコル正しさの担保 |
| リリース | `cargo-dist` + GitHub Actions matrix | 3 OS バイナリ + checksums |

## 未決事項 (Open questions)

- 設定 crate を `figment` と `config` のどちらにするか (優先順位: CLI > env > ファイルのマージ挙動と、WASM 側で不要であること)。
- i18n を `fluent` 直接利用とするか `rust-i18n` を併用するか。server と web (WASM) で同一 `.ftl` を共有する方式。
- `serde_yaml` はメンテ終了が告知されているため、後継 (`serde_yml` 等) の選定を M5 までに行う。
- `wpg-core` の「no_std 寄り」の範囲 (alloc 前提か、`std` feature を切るか)。
- `wpg-rx` を `wpg-tx` と同一 crate にしない理由の再確認 (依存が pcap/AF_XDP で共通)。
