# wpg 詳細仕様 (docs/spec)

wpg の詳細仕様。日本語を正とする。各文書は仕様の節番号 (§0〜§6) を持ち、コード中のコメントや [ROADMAP.md](../../ROADMAP.md) から `spec §2.1` のように参照される。決定事項は [`../adr/`](../adr/README.md) の ADR が優先される (ADR と矛盾する内容は書かない)。

Status: draft (M0 で分割開始)

| ファイル | 節 | 内容 |
|---|---|---|
| [00-concept.md](00-concept.md) | §0 | コンセプト、ゴール、非ゴール |
| [01-architecture.md](01-architecture.md) | §1 | 全体構成、crate 構成、技術選定 |
| [02-packet-builder.md](02-packet-builder.md) | §2.1 | プロトコル定義エンジン、対応プロトコル、ジェネレータ、import |
| [03-tx-engine.md](03-tx-engine.md) | §2.2 | 送出エンジン (wpg-tx) |
| [04-rx.md](04-rx.md) | §2.3 | 受信 / 検証 (wpg-rx) |
| [05-template.md](05-template.md) | §2.4 | テンプレート形式と共有、`.wpg.yaml` 例 |
| [06-gui.md](06-gui.md) | §2.5 | GUI (wpg-web) |
| [07-security.md](07-security.md) | §2.6 | 認証 / セキュリティ |
| [08-cli.md](08-cli.md) | §2.7 | CLI (wpg-cli) |
| [09-distribution.md](09-distribution.md) | §2.8 | スクリプト / 配布 / 運用 |
| [10-docs.md](10-docs.md) | §2.9 | ドキュメント方針 |
| [11-risks.md](11-risks.md) | §5 | 想定リスクと対策 |
| [12-glossary.md](12-glossary.md) | §6 | 用語 |
| [13-snmp.md](13-snmp.md) | §2.10 | SNMP マネージャ (wpg-snmp) |

## 記述ルール

- 各文書の末尾に「未決事項 (Open questions)」を置き、仕様上あいまいな点を列挙する。決定したら ADR を追加し、この一覧から削除する。
- 「確定した仕様」と「本書での提案 (Proposal)」を区別する。提案は確定事項ではない。
