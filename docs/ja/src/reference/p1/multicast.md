# マルチキャスト制御

Status: draft

IGMPv1 / v2 / v3、MLDv1 / v2、PIM-SM v2 (Hello / Join-Prune / Register) の補足です。定義は `crates/wpg-protocols/src/mcast.rs` にあります。

## 識別と積み方

ICMPv6 と同じく「共通ヘッダ (`type`、チェックサム) + メッセージごとの本体」の 2 レイヤです。共通ヘッダの `type` は本体レイヤから Auto で決まります。

| 共通ヘッダ | 下位レイヤ | 本体レイヤ |
|---|---|---|
| `igmp` | `ipv4` (プロトコル 2) | `igmpv2` (v1 / v2 のクエリ・レポート・Leave)、`igmpv3_query`、`igmpv3_report` |
| `icmpv6` | `ipv6` (次ヘッダ 58) | `mld` (130 / 131 / 132)、`mldv2_query` (130)、`mldv2_report` (143) |
| `pim` | `ipv4` (プロトコル 103) | `pim_hello`、`pim_join_prune` |
| `pim_register` (単独) | `ipv4` (プロトコル 103) | カプセル化するマルチキャストパケット (`ipv4` / `ipv6`) |

```text
ethernet2 / ipv4 / igmp / igmpv3_report
ethernet2 / ipv6 / ipv6_hopopts / icmpv6 / mldv2_report
ethernet2 / ipv4 / pim_register / ipv4 / udp / payload
```

IGMP / MLD は Router Alert オプション (IPv4 オプション / IPv6 Hop-by-Hop) を付けるのが通例です。サンプルを参照してください。

## Auto フィールド

| プロトコル | Auto |
|---|---|
| `igmp` / `pim` | `type` (本体レイヤから)、`checksum` (メッセージ全体) |
| `pim_register` | `checksum` (先頭 8 オクテットのみ) |
| `igmpv3_query` / `mldv2_query` | `num_sources` |
| `igmpv3_report` | `num_group_records`、各グループレコード内の `num_sources` |
| `mldv2_report` | `num_records`、各レコード内の送信元数 |
| `pim_join_prune` | `num_groups`、各グループの `num_joined` / `num_pruned` |

## 制約と回避方法

- `igmpv2` / `mld` を共通ヘッダに積むと `type` は Auto でクエリ (0x11 / 130) になります。v1 レポート (0x12)、v2 レポート (0x16)、Leave (0x17)、MLD レポート (131)、Done (132) は `type` を Manual で指定します。
- v2 と v3 のクエリは長さだけが異なります。解析では残りの長さ (IGMPv3 は 8 オクテット以上、MLDv2 は 24 オクテット以上) で v3 本体を選びます。
- **PIM Register** のチェックサムは先頭 8 オクテット (PIM ヘッダ + Register フラグ) だけを対象にし、カプセル化したパケットは含みません。この範囲を表現するため、`pim_register` は PIM ヘッダを含む単独の定義で、`pim` の上には積みません。解析では IP プロトコル 103 は `pim` として読まれ、Register 本体は `raw` になります。`pim_register` の `type` は 1 を明示してください。
- **PIM over IPv6** はチェックサムに IPv6 疑似ヘッダが必要ですが、チェックサムの種類を下位レイヤで切り替えられないため、`pim` は IPv4 からのみ解析されます。IPv6 で使う場合はチェックサムを Manual で指定します。
- PIM のエンコード済みアドレスは IPv4 (アドレスファミリ 1) だけを型付きにしています。

## サンプル

| テンプレート | 内容 |
|---|---|
| `templates/protocols/mcast/igmpv2-query.wpg.yaml` | Router Alert 付きの IGMPv2 一般クエリ |
| `templates/protocols/mcast/igmpv3-query.wpg.yaml` / `igmpv3-report.wpg.yaml` | IGMPv3 グループ・送信元指定クエリ / メンバーシップレポート |
| `templates/protocols/mcast/mldv1-query.wpg.yaml` | Hop-by-Hop Router Alert 付きの MLDv1 一般クエリ |
| `templates/protocols/mcast/mldv2-query.wpg.yaml` / `mldv2-report.wpg.yaml` | MLDv2 アドレス・送信元指定クエリ / レポート |
| `templates/protocols/mcast/pim-hello.wpg.yaml` | 224.0.0.13 への PIM Hello |
| `templates/protocols/mcast/pim-join-prune.wpg.yaml` | (S,G) Join/Prune |
| `templates/protocols/mcast/pim-register.wpg.yaml` | DR から RP への Register (マルチキャスト UDP をカプセル化) |
