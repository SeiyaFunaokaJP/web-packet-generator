# ADR-0001: ライセンス: MIT / Apache-2.0 デュアル

> License: dual MIT / Apache-2.0.

- Status: Accepted
- Date: 2026-09-29

## Context

wpg は Rust 製で、依存する DPDK (BSD-3-Clause)、libpcap (BSD)、Npcap (独自利用規約) と組み合わせて配布する。ライセンス選定は貢献者・利用者・再配布時の互換性に影響するため、早期に固定する必要がある。

## Decision

**MIT OR Apache-2.0 のデュアルライセンス**とする。全 crate の `Cargo.toml` に `license = "MIT OR Apache-2.0"` を設定し、リポジトリに `LICENSE-MIT` と `LICENSE-APACHE` を同梱する。DPDK・libpcap とは競合しない。Npcap は再配布せず、利用者が自身でインストールする。

## Alternatives considered

- MIT 単独: 簡潔だが特許条項がない。
- Apache-2.0 単独: 特許許諾はあるが GPLv2 との互換性に難がある。
- GPL-3.0 / AGPL-3.0: コピーレフトで改変の還元を促せるが、Rust エコシステムでの組み込み・再利用を妨げ、企業内ラボでの利用障壁になる。

## Consequences

- Rust エコシステム標準の組み合わせで、利用者・貢献者の理解コストが低い。
- Apache-2.0 の特許許諾を選べる一方、MIT を選べば軽量に扱える。
- 第三者依存のライセンスは `cargo deny` で継続的に検査する必要がある。
- Npcap を同梱できないため、Windows 向け導入手順で利用者に案内する。
