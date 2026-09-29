# ADR-0003: DB: ハイブリッド (YAML ファイル + SQLite)

> Database: hybrid, templates as YAML files and history/users/sessions in SQLite.

- Status: Accepted
- Date: 2026-09-29

## Context

テンプレートは人が読み、Git で diff・共有したい。一方、実行履歴、ユーザー、セッションは検索・更新が頻繁で、整合性が必要である。単一バイナリで外部依存なしに配布したい。

## Decision

**テンプレートは YAML ファイル、履歴 / ユーザ / セッションは SQLite** とするハイブリッド構成にする。SQLite は `rusqlite` の bundled 機能で組み込み、外部依存を持たない。テンプレートは Git 管理しやすいディレクトリ配置とし、複数のライブラリパスを読み込めるようにする。

## Alternatives considered

- SQLite のみ: 一元管理できるが、テンプレートが Git で diff できず、ファイル単位の共有が難しくなる。
- ファイルのみ (YAML/JSON): 単純だが、履歴検索、セッション、ユーザー管理でロックや整合性の実装が必要になる。
- PostgreSQL 等の外部 DB: 機能は十分だが、単一バイナリ配布とラボへの簡易導入に反する。

## Consequences

- テンプレートを Git/ファイル共有でそのまま扱える。
- 履歴・認証情報は SQLite のトランザクションで保護できる。
- 2 種類の保存先を持つため、削除・移動時に履歴側との整合をとる処理が必要になる。
- バックアップ対象はテンプレートディレクトリと SQLite ファイルの両方になる。
