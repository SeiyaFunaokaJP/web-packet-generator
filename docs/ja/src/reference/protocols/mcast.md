# マルチキャストプロトコル (P1)

Status: draft

定義: `crates/wpg-protocols/src/mcast.rs` / テスト: `crates/wpg-protocols/tests/p1_fhrp_mcast.rs` / サンプル: `templates/protocols/mcast/`

## 対象と到達方法

| 共通ヘッダ | 本体レイヤ | 下位からの到達 | サンプル |
|---|---|---|---|
| `igmp` | `igmpv2` (v1/v2 のクエリ・レポート・Leave) / `igmpv3_query` / `igmpv3_report` | IPv4 プロトコル 2 | `igmpv2-query` / `igmpv3-query` / `igmpv3-report` |
| `icmpv6` | `mld` (MLDv1) / `mldv2_query` / `mldv2_report` | ICMPv6 type 130 / 131 / 132 / 143 | `mldv1-query` / `mldv2-query` / `mldv2-report` |
| `pim` | `pim_hello` / `pim_join_prune` | IPv4 プロトコル 103 | `pim-hello` / `pim-join-prune` |
| (単独) | `pim_register` | IPv4 プロトコル 103 (構築用) | `pim-register` |

## 積み方

- **IGMP**: `ipv4` → `igmp` → 本体。`igmp.type` は本体から Auto で決まります (`igmpv2` の場合は 0x11 = クエリ)。v1 レポート / v2 レポート / Leave は `type` を 0x12 / 0x16 / 0x17 に手動で設定します。通常は IPv4 の Router Alert オプション付きで送ります (サンプル参照)。
- **MLD**: `ipv6` → `ipv6_hopopts` (Router Alert) → `icmpv6` → 本体。`icmpv6.type` は本体から Auto (`mld` の場合は 130)。レポート / Done は 131 / 132 を手動で設定します。
- **PIM Register**: `ipv4` → `pim_register` → カプセル化するマルチキャストパケット (`ipv4` / `ipv6` …)。`pim` の上には積みません。

## 解析時の判別

- IGMP type 0x11 / MLD type 130 のクエリは、v2 と v3 で長さだけが違います。残りが IGMPv3 で 8 オクテット以上、MLDv2 で 24 オクテット以上なら `igmpv3_query` / `mldv2_query`、短ければ `igmpv2` / `mld` として解析します。
- IP プロトコル 103 は `pim` として解析されます。Register の本体は `raw` になります。
- PIM Register に含まれるパケットは、先頭ニブルで IPv4 / IPv6 を判別します。

## Auto フィールド

| フィールド | 内容 |
|---|---|
| `igmp.type` / `pim.type` | 本体レイヤ |
| `igmp.checksum` / `pim.checksum` | メッセージ全体のインターネットチェックサム |
| `pim_register.checksum` | Register の先頭 8 オクテット (PIM ヘッダ + フラグ) だけのチェックサム |
| `igmpv3_query.num_sources` / `mldv2_query.num_sources` | 送信元アドレスの数 |
| `igmpv3_report.num_group_records` / `mldv2_report.num_records` | レコードの数 (各レコード内の `num_sources` も Auto) |
| `pim_join_prune.num_groups` | グループセットの数 (各セット内の `num_joined` / `num_pruned` も Auto) |

## 制約と近似

- **PIM Register は単独の定義**: Register のチェックサムは先頭 8 オクテットだけを対象とし、カプセル化されたパケットを含みません。「次のレイヤのヘッダまで」を範囲とするチェックサムがないため、PIM ヘッダを繰り返した単独の定義にしています。
- **PIM は IPv4 のみ**: IPv6 上の PIM はチェックサムに IPv6 疑似ヘッダを含めますが、チェックサムの種類を下位レイヤで切り替えられないため、`pim` は IPv4 からのみ到達します。エンコードアドレスも IPv4 (アドレスファミリ 1) だけを型付けしています。
- **時間の単位**: IGMP の `max_resp_code` は 1/10 秒 (v1 と v3 レポートでは予約)、MLDv1 の最大応答遅延はミリ秒、PIM のホールドタイムは秒です。
