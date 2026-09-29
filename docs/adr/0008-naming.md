# ADR-0008: 名称: wpg / wpg.toml / .wpg.yaml / .wpgbundle

> Naming: project `wpg` (web-packet-generator), binary `wpg`, config `wpg.toml`, templates `.wpg.yaml`, bundles `.wpgbundle`.

- Status: Accepted
- Date: 2026-09-29

## Context

CLI、設定ファイル、テンプレート、バンドルの名称を統一し、拡張子でファイルの種類を識別できるようにしたい。短く入力しやすいコマンド名が望ましい。

## Decision

プロジェクト名は **wpg** (`web-packet-generator`)、バイナリ名は `wpg` とする。設定ファイルは **`wpg.toml`**、テンプレート拡張子は **`.wpg.yaml`**、複数テンプレートのバンドルは **`.wpgbundle`** (zip) とする。crate 名は `wpg-` プレフィックスで統一する。

## Alternatives considered

- フルネーム `web-packet-generator` をコマンド名にする: 明快だが入力が長い。
- `packetgen` 等の汎用名: 既存ツールと衝突しやすい。
- テンプレートの拡張子を `.yaml` のみにする: エディタ対応は容易だが、他の YAML と区別できず、関連付けや lint の対象判定ができない。

## Consequences

- 短いコマンド名で CLI が扱いやすい。
- `.wpg.yaml` は YAML として編集でき、かつ wpg 用ファイルとして識別できる。
- 名称の変更は互換性に影響するため、変更時は ADR を追加する。
