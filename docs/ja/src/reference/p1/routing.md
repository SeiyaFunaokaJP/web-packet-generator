# ルーティングプロトコル

Status: draft

OSPFv2 / OSPFv3、BGP-4、RIPv1 / v2、RIPng、EIGRP (基本) の補足です。定義は `crates/wpg-protocols/src/routing.rs` にあります。

## 識別と積み方

| プロトコル ID | 下位レイヤ | 識別子 | 宛先の例 |
|---|---|---|---|
| `ospfv2` | `ipv4` | プロトコル 89 | 224.0.0.5 (AllSPFRouters)、224.0.0.6 (AllDRouters) |
| `ospfv3` | `ipv6` | 次ヘッダ 89 | `ff02::5` / `ff02::6` |
| `bgp` | `tcp` | ポート 179 (宛先、なければ送信元) | ピアのユニキャストアドレス |
| `rip` | `udp` | ポート 520 | 224.0.0.9 (v2) / ブロードキャスト (v1) |
| `ripng` | `udp` | ポート 521 | `ff02::9` |
| `eigrp` | `ipv4` / `ipv6` | プロトコル 88 | 224.0.0.10 / `ff02::a` |

OSPF と BGP は「共通ヘッダ + 本体」の 2 レイヤで 1 メッセージです。共通ヘッダの `type` は本体レイヤから Auto で決まります。

| 共通ヘッダ | 本体レイヤ |
|---|---|
| `ospfv2` | `ospfv2_hello` / `ospfv2_dbd` / `ospfv2_lsr` / `ospfv2_lsu` / `ospfv2_lsack` |
| `ospfv3` | `ospfv3_hello` / `ospfv3_dbd` / `ospfv3_lsr` / `ospfv3_lsu` / `ospfv3_lsack` |
| `bgp` | `bgp_open` / `bgp_update` / `bgp_notification` / `bgp_route_refresh` (本体なし = KEEPALIVE) |

LSU はさらに LSA ヘッダ (`ospfv2_lsa` / `ospfv3_lsa`) と LSA 本体を積みます。

```text
ethernet2 / ipv4 / ospfv2 / ospfv2_lsu / ospfv2_lsa / ospfv2_router_lsa
```

LSA 本体: OSPFv2 は `ospfv2_router_lsa` (1)、`ospfv2_network_lsa` (2)、`ospfv2_summary_lsa` (3 / 4)、`ospfv2_external_lsa` (5 / 7)。OSPFv3 は `ospfv3_router_lsa` (`0x2001`)、`ospfv3_network_lsa` (`0x2002`)、`ospfv3_inter_area_prefix_lsa` (`0x2003`)、`ospfv3_as_external_lsa` (`0x4005` / `0x2007`)、`ospfv3_link_lsa` (`0x0008`)、`ospfv3_intra_area_prefix_lsa` (`0x2009`)。

## Auto フィールド

| プロトコル | Auto |
|---|---|
| `ospfv2` | `type`、`packet_length`、`checksum` (認証フィールドを除くインターネットチェックサム) |
| `ospfv3` | `type`、`packet_length`、`checksum` (IPv6 疑似ヘッダ付き) |
| `ospfv2_lsa` / `ospfv3_lsa` | `ls_type`、`length`、`ls_checksum` (Fletcher。LS age を除外するので任意の age で正確) |
| `ospfv2_router_lsa` | `num_links` |
| `bgp` | `length`、`type` |
| `bgp_open` | `opt_params_length` |
| `eigrp` | `checksum` |

## 制約と回避方法

- **OSPFv2 の暗号認証** (auth type 2) では、RFC 2328 によりチェックサムは計算しない (0 にする) 決まりですが、Auto はこれを区別しません。`checksum: 0` を Manual で指定してください。パケット後ろの認証トレーラも表現しません。null 認証と平文パスワード認証では正確です。
- **LSU に載せられる型付き LSA は 1 つ**です。繰り返しレイヤの仕組みが無いため、型付きの LSA 本体の後ろに別の LSA は続けられません。`num_lsas` は Manual (既定 1) です。
- 本体レイアウトを共有する LS タイプ (OSPFv2 の 3 / 4 と 5 / 7、OSPFv3 の `0x4005` / `0x2007`) は Auto で先頭のタイプになります。もう一方にするには `ls_type` を Manual で指定してください (次プロトコル不一致の warning が出ます)。
- DBD / LSAck 内の LSA ヘッダのチェックサムは Manual です。
- OSPFv2 の TOS エントリ (Router-LSA の `num_tos` > 0、Summary / External の TOS メトリック) は表現しません。OSPFv3 のプレフィックスリスト (4 オクテット境界の可変長アドレスプレフィックス) は opaque な `prefixes` / `address_prefix` で、Link-LSA / Intra-Area-Prefix-LSA の `num_prefixes` も Manual です。
- **BGP UPDATE** の `withdrawn_routes_length` と `total_path_attribute_length` は Manual です (可変長領域を 2 つの長さが挟む構造のため)。既定値は既定のフィールド値に合っています。属性を変えたら合計オクテット数を書き直してください。Withdrawn Routes と NLRI は `(プレフィックス長, プレフィックス)` のバイト列で書きます (例: `18 c6 33 64` = 198.51.100.0/24)。
- **BGP パス属性**は「16 ビットの type (属性フラグ << 8 | type code) + 8 ビット長」の TLV です。Extended Length フラグ (`0x10`) の属性は `raw` 項目としてバイト列で書く必要があり、解析では読み戻せません。`AS_PATH` / `AGGREGATOR` は 4 オクテット AS 番号 (RFC 6793) です。Extended Optional Parameters (RFC 9072) は表現しません。
- **BGP** は 1 セグメントに 1 メッセージだけ解析します。
- **EIGRP** のルート TLV の宛先は `ceil(prefix_length / 8)` オクテットの可変長で、TLV 値の終わりまでの `destination` バイト列です。複数宛先の TLV もこの 1 フィールドに入ります。
- **RIP** の認証エントリ (`afi: 0xffff`) は通常のルートエントリと同じ 20 オクテットのフィールドで書きます。RIPng のネクストホップ RTE は `metric: 0xff` のエントリです。

## サンプル

| テンプレート | 内容 |
|---|---|
| `templates/protocols/routing/ospfv2-hello.wpg.yaml` ほか `ospfv2-*.wpg.yaml` | Hello / DBD / LSR / LSAck、LSU (Router / Network / Summary / External LSA) |
| `templates/protocols/routing/ospfv3-hello.wpg.yaml` ほか `ospfv3-*.wpg.yaml` | Hello / DBD / LSR / LSAck、LSU (Router / Network / Inter-Area-Prefix / AS-External / Link / Intra-Area-Prefix LSA) |
| `templates/protocols/routing/bgp-open.wpg.yaml` | Multiprotocol / Route Refresh / 4 オクテット AS 能力付きの OPEN |
| `templates/protocols/routing/bgp-update.wpg.yaml` | ORIGIN / AS_PATH / NEXT_HOP / MED / COMMUNITIES 付きで 198.51.100.0/24 を広告 |
| `templates/protocols/routing/bgp-keepalive.wpg.yaml` / `bgp-notification.wpg.yaml` / `bgp-route-refresh.wpg.yaml` | KEEPALIVE / NOTIFICATION / ROUTE-REFRESH |
| `templates/protocols/routing/ripv1-request.wpg.yaml` / `ripv2-response.wpg.yaml` / `ripng-response.wpg.yaml` | RIPv1 Request / RIPv2 Response / RIPng Response |
| `templates/protocols/routing/eigrp-hello.wpg.yaml` / `eigrp-update.wpg.yaml` | Hello / IPv4 内部ルート付き Update |
