# 11. 想定リスクと対策

Status: draft / 元: TODO.md §5

| リスク | 対策 | 関連 |
|---|---|---|
| Leptos + 大量 DOM (Hex 9000B) の描画性能 | 仮想スクロール、canvas フォールバック検討 | [06-gui.md](06-gui.md) |
| DPDK ビルドの環境依存が重い | feature フラグで分離、Docker ビルド環境提供、CI は Linux のみ | [ADR-0004](../adr/0004-dpdk-bindings.md) |
| Windows 高速送出の限界 (Npcap) | 期待値をドキュメントで明示、Windows は「GUI + 低〜中速送出」 | [ADR-0006](../adr/0006-windows-tx.md) |
| プロトコル数の膨張でメンテ不能 | 宣言的定義 + golden pcap + 自動ドキュメント生成で 1 プロトコル追加コストを一定化 | [ADR-0007](../adr/0007-protocol-definition.md) |
| 権限周り (raw socket) の OS 差 | `wpg doctor` で自己診断し、OS 別に具体的コマンドを提示 | [08-cli.md](08-cli.md) |
| 誤用 (本番 NW への大量送出) | 送出制限ポリシー、確認ダイアログ、監査ログ、ドキュメントでの明記 | [07-security.md](07-security.md) |
| i18n の抜け漏れ | 文字列直書き禁止の lint (CI)、Fluent キー未定義検出 | [06-gui.md](06-gui.md) |

## 追加で検討したいリスク (Proposal)

- `xsk-rs` のメンテ状況・カーネル/ドライバ差異による AF_XDP の不安定さ (libc 補完で吸収する方針、[ADR-0005](../adr/0005-af-xdp.md))。
- `serde_yaml` の非メンテ化によるテンプレート形式の依存リスク。(M12 でメンテされている互換フォーク `serde_norway` に移行済み。)
- WASM と server の合成結果の乖離 (浮動小数点、乱数、時刻を core に持ち込まない)。

## 未決事項 (Open questions)

- リスクごとの担当マイルストーンと再評価のタイミング。
- 誤用対策として、既定の最大レート上限を設けるか。
