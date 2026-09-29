# ADR-0006: Windows 送出: Npcap のみ

> Windows transmit path: Npcap only; WinPF and XDP for Windows are out of v1 scope.

- Status: Accepted
- Date: 2026-09-29

## Context

Windows は主に GUI 利用と低〜中速の送出を想定している。高速化手段 (WinPF、XDP for Windows など) は開発・保守コストが大きく、v1 の工数を圧迫する。

## Decision

**Windows の送出は Npcap のみ**とする。Windows は「GUI + 低〜中速送出」という位置づけとし、期待値をドキュメントで明示する。**WinPF と XDP for Windows は v1 スコープ外**とする。Npcap は再配布せず、利用者がインストールする。

## Alternatives considered

- WinPcap: 旧世代でメンテされておらず、Windows 10 以降での互換性に問題がある。
- XDP for Windows: 高速化が期待できるが、成熟度と導入条件の面で v1 には時期尚早。
- WinDivert / raw socket: 権限とフィルタの制約が強く、任意フレーム (L2) の送出に向かない。

## Consequences

- Windows の実装が pcap バックエンドに一本化され、シンプルになる。
- Windows での高レートは期待できない。
- Npcap のライセンス上、同梱せず案内と検出 (`wpg doctor`) が必要になる。
- 将来 XDP for Windows 等を追加する場合は新しい ADR で更新する。
