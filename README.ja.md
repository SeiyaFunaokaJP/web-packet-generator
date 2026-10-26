# wpg — web-packet-generator

[English](README.md) | **日本語**

[![CI](https://github.com/SeiyaFunaokaJP/web-packet-generator/actions/workflows/ci.yml/badge.svg)](https://github.com/SeiyaFunaokaJP/web-packet-generator/actions/workflows/ci.yml)
[![Security audit](https://github.com/SeiyaFunaokaJP/web-packet-generator/actions/workflows/audit.yml/badge.svg)](https://github.com/SeiyaFunaokaJP/web-packet-generator/actions/workflows/audit.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#ライセンス)

ネットワーク検証向けの、Web GUI で動く高性能パケットジェネレータです。バックエンド、パケットエンジン、
WASM フロントエンドまですべて Rust で実装しています。GUI で任意のパケットを組み立て、超高速に送出できます。

> **ステータス: リリース前（0.x）。** マイルストーン M0〜M11（エンジン、送出と受信、サーバ、GUI、テンプレート、
> P0〜P2 プロトコル、デプロイ、ドキュメント）は実装済みで、v1.0 のリリース判定（M12）を進めています。
> 一部の経路（物理 NIC での AF_XDP zero-copy と DPDK、CML2）は実機で未確認です。ロードマップは [ROADMAP.ja.md](ROADMAP.ja.md)、
> 変更履歴は [CHANGELOG.md](CHANGELOG.md) を参照してください。

## wpg とは

NW エンジニア、検証担当、CML2 / EVE-NG 等のラボ利用者、プロトコル実装者が、Web GUI 上でレイヤを積み上げて
パケットを組み立て、高速なバックエンドで送出するためのツールです。バイナリ名は `wpg`。フロントエンド資産は
バイナリに埋め込まれるため、単一の実行ファイルだけで動作します。

![パケットビルダーで Ethernet / IPv4 / UDP を組み、送信元アドレスに範囲を指定する様子](docs/ja/src/images/builder.gif)

差別化ポイント:

1. IS-IS / CLNP など OSI 系まで含む「深い」プロトコル対応
2. AF_PACKET / AF_XDP / DPDK まで使う高速送出
3. GUI とテンプレート共有による「誰でも再現できる検証」

v1 の非ゴール: 本格的なキャプチャ/解析（Wireshark の代替ではありません）、TCP 3-way handshake 完走などの
ステートフルなセッション生成、攻撃ツール用途への最適化。

## 主な機能

- Ethernet、VLAN、IPv4/IPv6、TCP/UDP からルーティング（OSPF、BGP、IS-IS）、OSI（CLNP、ES-IS）、MPLS / SR / SRv6、
  トンネル、タイミング / OAM、アプリケーションまで 180 以上のプロトコル（[プロトコルリファレンス](docs/ja/src/reference/protocols.md)）
- 宣言的なプロトコル定義 1 つから、合成・解析・GUI 生成・ドキュメント生成を行う
- 長さ / チェックサム / 次ヘッダの自動計算（Auto/Manual 切替）と、意図的な malformed パケット生成
- フィールドジェネレータ（range / list / random / counter / file / expr）と大量ストリーム生成
- ブラウザ内 WASM による即時プレビュー（`wpg-core` をサーバと共有）
- YAML テンプレート（`.wpg.yaml`）、`.wpgbundle` による共有、pcap/pcapng インポート
- レート制御（pps / bps / 回線率 %）、バースト、マルチキュー・マルチコア送出、WebSocket によるリアルタイム統計
- tcpdump 形式のフィルタ付き受信カウンタ、シグネチャによるロス / 順序 / 遅延の照合、pcapng 保存
- OpenAPI 付き REST API、パスワード認証（argon2id）、任意で TLS
- 日本語・英語・中国語 (簡体字 / 繁体字)・韓国語・ドイツ語・フランス語・スペイン語の UI / メッセージ (マニュアルは日本語・英語)

## クイックスタート

前提: [rustup](https://rustup.rs)、WASM ターゲット（`rustup target add wasm32-unknown-unknown`）、
[trunk](https://trunkrs.dev)（`cargo install trunk --locked`）、任意で [just](https://github.com/casey/just)。
libpcap / Npcap 送出バックエンドは任意です（`cargo build --release --features pcap`、`wpg-tx/pcap` に転送されます）。Linux では `libpcap-dev`、macOS は標準搭載、Windows では Npcap ランタイムと [Npcap SDK](https://npcap.com/#download)（`LIB` に `wpcap.lib`）が必要です。機能を有効にしなければ pcap ライブラリは不要です。不足しているものは `wpg doctor` で確認できます。

```sh
# ビルド（フロントエンド → バックエンドの順）
just build            # または ./scripts/build.sh      （Windows: .\scripts\build.ps1）
just release          # 最適化ビルド

# 起動
./target/release/wpg serve    # または ./scripts/run.sh   （Windows: .\scripts\run.ps1）
```

ブラウザで <http://127.0.0.1:8080> を開きます。

ホットリロード付きの開発起動（`cargo install cargo-watch --locked` が必要）:

```sh
just run              # または ./scripts/run.sh --dev
```

raw ソケット送出には権限が必要です。Linux は root 実行、または
`sudo setcap cap_net_raw,cap_net_admin+ep ./target/release/wpg`。Windows は Npcap をインストールして管理者で実行。
macOS は `/dev/bpf*` への権限が必要です。`wpg doctor` で自己診断できます。

## アーキテクチャ

```
┌──────────────────────────────────────────────────────────────┐
│ Browser (WASM: Leptos)                                       │
│  パケットビルダー GUI、テンプレート管理、ジョブ、統計、i18n      │
└───────────────▲──────────────────────────────▲───────────────┘
                │ REST (JSON)                  │ WebSocket (統計/ログ)
┌───────────────┴──────────────────────────────┴───────────────┐
│ wpg-server (axum + tokio) … 単一バイナリ、静的資産埋め込み      │
│  認証、テンプレート CRUD、ジョブ管理、OpenAPI                  │
├──────────────────────────────────────────────────────────────┤
│ wpg-core (pure Rust, WASM でもビルド可)                        │
│  プロトコル定義エンジン、合成/解析、ジェネレータ                 │
├──────────────────────────────────────────────────────────────┤
│ wpg-tx (TxBackend trait)                                     │
│  pcap/Npcap | AF_PACKET | AF_XDP | DPDK | pcap ファイル出力    │
└──────────────────────────────────────────────────────────────┘
```

workspace の crate: `wpg-core`, `wpg-protocols`, `wpg-tx`, `wpg-rx`, `wpg-template`, `wpg-i18n`,
`wpg-server`, `wpg-cli`（バイナリ `wpg`）, `wpg-web`。

## 送出バックエンド

| バックエンド | OS | 期待性能 | 備考 |
|---|---|---|---|
| `pcap` (libpcap / Npcap / BPF) | Win / mac / Linux | 〜数十万 pps | 全 OS 共通のフォールバック |
| `afpacket` (TPACKET_V3 + `sendmmsg`) | Linux | 〜1–3 Mpps/core | CML2 上 Ubuntu (virtio) の主経路。CAP_NET_RAW 必要 |
| `afxdp` | Linux 5.10+ (`--features afxdp`) | 〜数 Mpps/core | zero-copy はドライバ対応要。zero-copy / copy 自動 (zero-copy は実機未確認) |
| `dpdk` | Linux | 10 Mpps+/core | `--features dpdk`、DPDK 24.11 LTS（23.11 も可）。確認済みは仮想デバイスのみ |
| `pcapfile` | 全 | — | 送出せず pcapng に書き出す（ドライラン、CI） |
| `null` | 全 | — | ベンチ・テスト用 |

最低対応 OS: Windows 10 21H2+ / macOS 12+ / Ubuntu 20.04+（glibc 2.31）。Windows の送出は Npcap のみで、
「GUI + 低〜中速送出」の位置づけです。

## ドキュメント

利用者・開発者向けのマニュアルは日本語（[`docs/ja`](docs/ja/src/SUMMARY.md)）と英語（[`docs/en`](docs/en/src/SUMMARY.md)）の
mdBook です。3 OS へのインストール、チュートリアル、自動生成のプロトコル / CLI / API / 設定リファレンス、運用（デプロイ、
サービス化、DPDK / AF_XDP、CML2、性能チューニング）、開発者ガイドを収録しています。
<https://seiyafunaokajp.github.io/web-packet-generator/> で公開しており、ローカルでは `just docs` でビルドします
（`cargo install mdbook --locked` が必要）。

## ロードマップ

M0〜M12 のマイルストーンと進捗は [ROADMAP.ja.md](ROADMAP.ja.md) にあります。詳細仕様は [`docs/spec/`](docs/spec/README.md)、
設計上の決定事項は [`docs/adr/`](docs/adr/README.md) にあります。変更履歴は [CHANGELOG.md](CHANGELOG.md) にあります。

## 法的注意・利用上の責任

wpg は **ネットワーク検証ツール** です。**自身が所有する、または明示的にテストを許可されたネットワーク・機器
（隔離されたラボ・検証環境など）でのみ** 使用してください。本番ネットワークや第三者のシステムに対する
細工パケットや大量送出は、サービス障害を引き起こす可能性があり、違法となる場合があります。攻撃用途を想定した
設計・最適化はしていません。**本ソフトウェアの使用に関する責任はすべて利用者にあります。** 作者および
コントリビューターは、損害や誤用について一切責任を負いません（各ライセンスの無保証条項も参照）。

## コントリビュート

[CONTRIBUTING.ja.md](CONTRIBUTING.ja.md)（[English](CONTRIBUTING.md)）と
[行動規範](CODE_OF_CONDUCT.md) を参照してください。

## ライセンス

以下のいずれかを選択して利用できます。

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) または <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) または <http://opensource.org/licenses/MIT>)

### コントリビューション

明示的に別段の表明をしない限り、Apache-2.0 ライセンスで定義される、本プロジェクトへの取り込みを意図して
あなたが提出したコントリビューションは、追加の条件なしに上記のデュアルライセンスで提供されるものとします。
