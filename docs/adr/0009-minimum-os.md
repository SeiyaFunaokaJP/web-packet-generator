# ADR-0009: 最低対応 OS

> Minimum supported OS: Windows 10 21H2+, macOS 12+, Ubuntu 20.04+ (glibc 2.31).

- Status: Accepted
- Date: 2026-09-29

## Context

3 OS 向けにバイナリを配布する。サポート範囲が広いほどビルド・テスト負荷が増え、狭いと CML2 の Ubuntu など検証現場で使えない。glibc バージョンはバイナリの実行可否を決める。

## Decision

**Windows 10 21H2 以降 / macOS 12 以降 / Ubuntu 20.04 以降 (glibc 2.31)** を最低対応とする。Linux は glibc 2.31 以上であれば他ディストリビューションでも動作を想定する。musl 静的ビルドは P2 で検討する。AF_XDP には別途カーネル 5.10 以上が必要 ([ADR-0005](0005-af-xdp.md))。

## Alternatives considered

- 最新 LTS のみ (Ubuntu 22.04+ など): ビルドは簡単だが、CML2 や既存ラボの古いイメージで動かない。
- Ubuntu 18.04 (glibc 2.27) まで対応: 対象は広がるが、既に EOL であり、CI と保守コストが増える。
- musl 静的ビルドを最初から主軸にする: glibc 依存を排除できるが、libpcap や DPDK との組み合わせが複雑になる。

## Consequences

- glibc 2.31 を基準に古いビルドホスト (または Docker) でリリースビルドする必要がある。
- Windows/macOS は各 OS の現行に近い範囲に絞り、CI マトリクスを維持しやすい。
- 古い環境のユーザーには非対応であることを明記する。
