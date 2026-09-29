# ADR-0005: AF_XDP: xsk-rs + libc で補完

> AF_XDP: use `xsk-rs`, complement with direct libc calls; kernel 5.10+, automatic fallback to copy mode.

- Status: Accepted
- Date: 2026-09-29

## Context

Linux で DPDK ほど重くなく Mpps 級を狙う中間の経路として AF_XDP が必要である。NIC ドライバによって zero-copy 対応が異なり、環境ごとの差を吸収する必要がある。

## Decision

**`xsk-rs` を採用**し、不足する部分は **libc 直叩きで補完**する。最低カーネルは **5.10** (Ubuntu 20.04 HWE / 22.04 以降) とする。zero-copy 非対応の NIC では **copy モードへ自動フォールバック**する。

## Alternatives considered

- libbpf-rs + 自前 UMEM 管理: 制御は最大だが、実装量と保守が大きい。
- aya (eBPF フレームワーク): eBPF プログラムを Rust で書ける利点はあるが、送出用途では XSK ソケット操作が主で過剰になる。
- AF_PACKET (TPACKET_V3) のみ: 実装は簡単だが、数 Mpps/core を目標とする用途には性能が不足する。

## Consequences

- 外部 crate に依存することで実装量を抑えられる。
- `xsk-rs` の更新が止まった場合は libc 側に置き換える余地を残す。
- 対応 NIC/ドライバ一覧をドキュメント化し、`wpg doctor` で診断する必要がある。
- カーネル 5.10 未満の環境では AF_XDP を利用できない。
