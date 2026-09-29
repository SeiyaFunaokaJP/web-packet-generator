# ADR-0004: DPDK バインディング: 自前 bindgen (dpdk-sys)

> DPDK bindings: in-tree bindgen crate `dpdk-sys`, DPDK 24.11 LTS primary, 23.11 LTS checked in CI.

- Status: Accepted
- Date: 2026-09-29

## Context

DPDK は最高速の送出バックエンドとして必要だが、ビルドの環境依存が重い。DPDK 自体の API は LTS ごとに変化し、インライン関数が多いため、バインディングの保守性が課題となる。

## Decision

**workspace 内に自前の `dpdk-sys` crate (bindgen) を作成**する。対象は **DPDK 24.11 LTS を第一**とし、**23.11 LTS も CI でビルド確認**する。`--features dpdk` を指定した場合のみ有効にし、既定ビルドには影響させない。EAL 引数は設定ファイルから渡す。

## Alternatives considered

- capsule: Rust 製の NFV フレームワーク。高レベルで便利だが、更新が停滞しており最新の DPDK LTS への追従が期待できない。
- dpdk-rs 等の既存バインディング: 導入は容易だが、対応 DPDK バージョンやインライン関数の扱いが固定され、24.11 への追従を自前で制御できない。
- DPDK を使わず AF_XDP までに留める: ビルド負荷は減るが、10 Mpps/core 級の要件を満たせない。

## Consequences

- 対象 DPDK バージョンとラッパーの範囲を自分たちで制御できる。
- bindgen とインライン関数のラッパー (C 側 shim) を保守する負担が生じる。
- 2 つの LTS を CI で確認するため、Docker ビルド環境の整備が必要になる。
- DPDK ビルドの環境依存は feature 分離で通常開発から隔離される。
