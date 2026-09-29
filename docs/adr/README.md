# Architecture Decision Records

主要な設計判断を [MADR](https://adr.github.io/madr/) 風に記録する。TODO.md §3「決定事項 (2026-09-29 確定)」の各行を 0001〜0009 として ADR 化した。決定を変える場合は既存 ADR を書き換えず、新しい ADR を追加して `Superseded by` を記載する。

## 一覧

| ADR | タイトル | Status | Date |
|---|---|---|---|
| [0001](0001-license.md) | ライセンス: MIT / Apache-2.0 デュアル | Accepted | 2026-09-29 |
| [0002](0002-frontend.md) | フロントエンド: Leptos (CSR) + Tailwind CSS | Accepted | 2026-09-29 |
| [0003](0003-database.md) | DB: ハイブリッド (YAML ファイル + SQLite) | Accepted | 2026-09-29 |
| [0004](0004-dpdk-bindings.md) | DPDK バインディング: 自前 bindgen (dpdk-sys) | Accepted | 2026-09-29 |
| [0005](0005-af-xdp.md) | AF_XDP: xsk-rs + libc で補完 | Accepted | 2026-09-29 |
| [0006](0006-windows-tx.md) | Windows 送出: Npcap のみ | Accepted | 2026-09-29 |
| [0007](0007-protocol-definition.md) | プロトコル定義方式: Rust マクロ + YAML (同一 IR) | Accepted | 2026-09-29 |
| [0008](0008-naming.md) | 名称: wpg / wpg.toml / .wpg.yaml / .wpgbundle | Accepted | 2026-09-29 |
| [0009](0009-minimum-os.md) | 最低対応 OS | Accepted | 2026-09-29 |

## 追加手順

1. [`0000-template.md`](0000-template.md) を `NNNN-短い名前.md` にコピーする (番号は連番)。
2. Status を `Proposed` で起票し、合意後に `Accepted` にする。
3. 本ファイルの一覧に追記する。
4. 置き換える場合は、旧 ADR の Status を `Superseded by ADR-NNNN` に更新する。

各 ADR は日本語で書き、冒頭に英語の一行要約を置く。
