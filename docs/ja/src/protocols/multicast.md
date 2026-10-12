# マルチキャスト制御プロトコル

Status: draft

IGMPv1・v2・v3 / MLDv1・v2 / PIM-SM v2 の補足です。定義は `crates/wpg-protocols/src/mcast.rs`、サンプルは `templates/protocols/mcast/` にあります。

## 到達経路とサンプル

| id | プロトコル | 下位レイヤ | サンプル |
|---|---|---|---|
| `igmp` + `igmpv2` | IGMPv1 / v2 (クエリ、レポート、Leave) | IPv4 protocol 2 | `igmpv2-query.wpg.yaml` |
| `igmp` + `igmpv3_query` | IGMPv3 クエリ | IPv4 protocol 2、type `0x11` で残り 8 オクテット以上 | `igmpv3-query.wpg.yaml` |
| `igmp` + `igmpv3_report` | IGMPv3 レポート | IPv4 protocol 2、type `0x22` | `igmpv3-report.wpg.yaml` |
| `icmpv6` + `mld` | MLDv1 (クエリ、レポート、Done) | ICMPv6 type 130 / 131 / 132 | `mldv1-query.wpg.yaml` |
| `icmpv6` + `mldv2_query` | MLDv2 クエリ | ICMPv6 type 130 で残り 24 オクテット以上 | `mldv2-query.wpg.yaml` |
| `icmpv6` + `mldv2_report` | MLDv2 レポート | ICMPv6 type 143 | `mldv2-report.wpg.yaml` |
| `pim` + `pim_hello` / `pim_join_prune` | PIM-SM v2 | IPv4 protocol 103 | `pim-hello.wpg.yaml`、`pim-join-prune.wpg.yaml` |
| `pim_register` | PIM Register (単独定義) | ビルド時に選択 (デコードは `pim`) | `pim-register.wpg.yaml` |

IGMP と PIM は ICMPv6 と同じく「共通ヘッダ (`type`、チェックサム) + 本体」の 2 レイヤで、ヘッダの `type` は本体から Auto で決まります。MLD の本体は `icmpv6` の上に積みます。

```text
ipv4 / igmp / igmpv3_report
ipv6 / ipv6_hopopts / icmpv6 / mldv2_report
ipv4 / pim / pim_join_prune
ipv4 / pim_register / ipv4 / udp / payload
```

## Auto になるフィールド

- IGMP / PIM ヘッダの `type` と `checksum`。MLD のチェックサムは `icmpv6` が IPv6 疑似ヘッダ込みで計算します。
- 件数: IGMPv3 / MLDv2 クエリの `num_sources`、レポートの `num_group_records` / `num_records`、グループレコード内の `num_sources`、PIM Join/Prune の `num_groups` / `num_joined` / `num_pruned`。リストの要素を増減するだけで合います。
- `pim_register.checksum`: PIM ヘッダ + Register フラグの 8 オクテットだけを対象に計算します (RFC 7761)。

## 制約と近似

- **v2 と v3 のクエリ**は長さでしか区別できないため、残りの長さが IGMPv3 で 8 オクテット以上、MLDv2 で 24 オクテット以上なら v3 / v2 のクエリとしてデコードし、それより短ければ `igmpv2` / `mld` としてデコードします。
- **PIM Register** のチェックサムは内側のパケットを含まないため、PIM ヘッダを繰り返した単独定義 `pim_register` になっています。IP protocol 103 は `pim` としてデコードされるので、受信した Register の本体は `raw` になります。内側のパケットは先頭ニブルで IPv4 / IPv6 を判別します。
- **PIM over IPv6** はチェックサムに IPv6 疑似ヘッダが必要ですが、チェックサムの種類を下位レイヤで切り替えられないため、`pim` は IPv4 からのみ到達し、通常のインターネットチェックサムを使います。
- PIM のエンコード済みアドレスは IPv4 (アドレスファミリ 1) のみ型付けされています。
- IGMP / MLD はルータアラートオプションを必要とします。サンプルは `ipv4` の `options` / `ipv6_hopopts` に Router Alert を入れてあります。

<!-- wpg-docs-gen:begin reference-links -->
## 自動生成リファレンス

このページのプロトコルのフィールドリファレンス (定義から自動生成):

[`ipv4`](../reference/protocols/ipv4.md) · [`ipv6_hopopts`](../reference/protocols/ipv6_hopopts.md) · [`icmpv6`](../reference/protocols/icmpv6.md) · [`igmp`](../reference/protocols/igmp.md) · [`igmpv2`](../reference/protocols/igmpv2.md) · [`igmpv3_query`](../reference/protocols/igmpv3_query.md) · [`igmpv3_report`](../reference/protocols/igmpv3_report.md) · [`mld`](../reference/protocols/mld.md) · [`mldv2_query`](../reference/protocols/mldv2_query.md) · [`mldv2_report`](../reference/protocols/mldv2_report.md) · [`pim`](../reference/protocols/pim.md) · [`pim_hello`](../reference/protocols/pim_hello.md) · [`pim_join_prune`](../reference/protocols/pim_join_prune.md) · [`pim_register`](../reference/protocols/pim_register.md) · [`raw`](../reference/protocols/raw.md)
<!-- wpg-docs-gen:end reference-links -->
