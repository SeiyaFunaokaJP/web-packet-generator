# wpg へのコントリビュート

[English](CONTRIBUTING.md) | **日本語**

ご関心をありがとうございます。まず [行動規範](CODE_OF_CONDUCT.md) をお読みください。ロードマップは [ROADMAP.ja.md](ROADMAP.ja.md)、仕様は
[`docs/spec/`](docs/spec/README.md) にあります。

## 開発環境

1. [rustup](https://rustup.rs) をインストール（`rust-toolchain.toml` のツールチェーンが自動で使われます）
2. WASM ターゲット追加: `rustup target add wasm32-unknown-unknown`
3. ツール: `cargo install just trunk cargo-watch cargo-deny cargo-audit mdbook --locked`
   （最低限のビルドには `just` と `trunk` のみ必要）
4. Linux は `sudo apt install libpcap-dev`、Windows は Npcap SDK（pcap バックエンド実装までは任意）
5. `just build && ./target/debug/wpg serve` の後、<http://127.0.0.1:8080> を開く

## よく使うレシピ

`just --list` で一覧を確認できます。`just build` / `release`（ビルド）、`run`（ホットリロード開発起動）、
`test`、`e2e`（Web UI の Playwright テスト、[e2e/README.md](e2e/README.md)）、`fmt`、`lint`（clippy `-D warnings`）、`check-wasm`、`audit`、`deny`、`docs`、`clean`。
PR 前に `just fmt lint test check-wasm` を実行してください。CI でも同じチェックが走ります。

## コミット / PR 規約

- [Conventional Commits](https://www.conventionalcommits.org) を使用（例: `feat(core): ...`）。
  scope は `wpg-` を除いた crate 名。
- PR は小さく、1 PR 1 論理変更。ROADMAP.ja.md のマイルストーンや issue を参照してください。
- テストを追加してください。プロトコルは golden pcap と、可能なら `tshark -V` での突合が必要です。
- ROADMAP.ja.md のチェックは項目が完全に完了したときのみ更新します。`docs/adr/` の決定事項を変える場合は
  新しい ADR を追加します。

## プロトコルの追加

`docs/ja/src/dev/add-protocol.md` の開発者ガイド（プロトコル追加手順）を参照してください。定義、Fluent 文字列（名称・フィールド説明）、
サンプルテンプレート、golden pcap、テストが最低限必要です。
そのあと `just docs-gen` でリファレンス（プロトコル・CLI・API・設定・テンプレートスキーマ）を再生成してコミットしてください
（生成物が古いとテストが失敗します）。

## 国際化 (i18n) ルール

- サーバ / CLI / Web UI（エラーメッセージ含む）に **ユーザー向け文字列を直書きしない**。Fluent のキーを使う。
- すべてのキーは `locales/*/main.ftl` の **全言語**（ja / en / zh-CN / zh-TW / ko / de / fr / es）に存在すること
  （抜けは `cargo test -p wpg-i18n` が検出し、CI で失敗します）。
- 言語の追加手順は開発者ガイド `docs/ja/src/dev/add-i18n.md` を参照してください。

## ライセンスと DCO

本プロジェクトは MIT OR Apache-2.0 のデュアルライセンスです。明示的な別段の表明がない限り、Apache-2.0 で
定義される、本プロジェクトへの取り込みを意図して提出されたコントリビューションは、追加条件なしに同デュアル
ライセンスで提供されるものとします。コントリビューションを提出することで、自身が作成したもの、または当該
ライセンスで提出する権利を持つものであることを表明したものとみなします（Developer Certificate of Origin の
趣旨）。`git commit -s` で sign-off してください。

## 責任ある利用

wpg は自身が所有する、または許可を得たネットワークの検証用です。攻撃ツール化を目的とする変更は受け付けません。
