# 09. スクリプト / 配布 / 運用

Status: draft / 仕様 §2.8

## スクリプト

| スクリプト | 内容 |
|---|---|
| `scripts/build.sh` / `build.ps1` | 依存チェック (rustup, wasm target, trunk, libpcap-dev / Npcap SDK)、フロント → バックエンドの順でビルド。`--release`、`--features dpdk` |
| `scripts/run.sh` / `run.ps1` | 開発起動 (ホットリロード: `trunk serve` + `cargo watch`) / 本番起動 |
| `scripts/deploy.sh <user@host> [--iface eth1] [--service]` | ビルド済みバイナリを scp、`setcap`、systemd unit 投入・起動、ヘルスチェック。CML2 上 Ubuntu が第一ターゲット |
| `scripts/release.sh` | cargo-dist による 3 OS クロスビルド、チェックサム、GitHub Release |

`justfile` (or `Makefile`) を統一エントリとする: `just build`, `just run`, `just deploy host=...`, `just test`, `just docs`。

## コンテナ・サービス

- Docker: `Dockerfile` (multi-stage) + `docker-compose.yml` (`--network host` + `NET_RAW`)
- systemd unit、Windows サービス (`sc create` 手順)、macOS launchd plist のサンプル

## 設定ファイル

`wpg.toml` のフル例と `wpg config init`。

## 対象 OS

Windows 10 21H2+ / macOS 12+ / Ubuntu 20.04+ (glibc 2.31) ([ADR-0009](../adr/0009-minimum-os.md))。musl 静的ビルドは P2 で検討。

## 未決事項 (Open questions)

- Windows 用 `build.ps1` と `justfile` の役割分担 (just を Windows で必須にするか)。
- macOS の署名/公証 (Apple Developer ID の要否)。未署名時の回避手順のみ先に記載。
- Docker イメージの配布先 (GHCR 等) と DPDK 入りイメージの扱い。
- アップグレード手順とテンプレマイグレーションの自動実行タイミング。
- systemd unit のハードニング項目 (AmbientCapabilities 等) の既定値。
- `scripts/deploy.sh` の対象 OS を Ubuntu に限定するか。
