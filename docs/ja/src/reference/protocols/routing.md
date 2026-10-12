# ルーティングプロトコル (P1)

Status: draft

定義: `crates/wpg-protocols/src/routing.rs` / テスト: `crates/wpg-protocols/tests/p1_routing.rs` / サンプル: `templates/protocols/routing/`

## 対象と到達方法

| id | プロトコル | 下位からの到達 | サンプル |
|---|---|---|---|
| `ospfv2` + 本体 | OSPFv2 (RFC 2328) | IPv4 プロトコル 89 | `ospfv2-hello` / `-dbd` / `-lsr` / `-lsu-*` / `-lsack` |
| `ospfv3` + 本体 | OSPFv3 (RFC 5340) | IPv6 次ヘッダ 89 | `ospfv3-hello` / `-dbd` / `-lsr` / `-lsu-*` / `-lsack` |
| `bgp` + 本体 | BGP-4 (RFC 4271) | TCP 179 (宛先、なければ送信元) | `bgp-open` / `-update` / `-keepalive` / `-notification` / `-route-refresh` |
| `rip` | RIPv1 / RIPv2 | UDP 520 | `ripv1-request` / `ripv2-response` |
| `ripng` | RIPng (RFC 2080) | UDP 521 | `ripng-response` |
| `eigrp` | EIGRP (RFC 7868、基本 TLV) | IP プロトコル 88 (IPv4 / IPv6) | `eigrp-hello` / `eigrp-update` |

## 積み方

OSPF と BGP は「共通ヘッダ + 本体」の 2 レイヤで構成します。共通ヘッダの `type` は本体レイヤから Auto で決まります。

| 共通ヘッダ | 本体レイヤ |
|---|---|
| `ospfv2` | `ospfv2_hello` / `ospfv2_dbd` / `ospfv2_lsr` / `ospfv2_lsu` / `ospfv2_lsack` |
| `ospfv3` | `ospfv3_hello` / `ospfv3_dbd` / `ospfv3_lsr` / `ospfv3_lsu` / `ospfv3_lsack` |
| `bgp` | `bgp_open` / `bgp_update` / `bgp_notification` / `bgp_route_refresh` (KEEPALIVE は `bgp` のみ、既定 type 4) |

**LSU** はさらに LSA ヘッダと LSA 本体を積みます。

- OSPFv2: `ospfv2_lsu` → `ospfv2_lsa` → `ospfv2_router_lsa` / `ospfv2_network_lsa` / `ospfv2_summary_lsa` / `ospfv2_external_lsa`
- OSPFv3: `ospfv3_lsu` → `ospfv3_lsa` → `ospfv3_router_lsa` / `ospfv3_network_lsa` / `ospfv3_inter_area_prefix_lsa` / `ospfv3_as_external_lsa` / `ospfv3_link_lsa` / `ospfv3_intra_area_prefix_lsa`

OSPF の宛先は通常 `224.0.0.5` (AllSPFRouters、IPv4 TTL 1) または `ff02::5` です。

## Auto フィールド

| フィールド | 内容 |
|---|---|
| `ospfv2.type` / `ospfv3.type` / `bgp.type` | 本体レイヤ |
| `ospfv2.packet_length` / `ospfv3.packet_length` / `bgp.length` | 共通ヘッダ + 本体の長さ |
| `ospfv2.checksum` | インターネットチェックサム (64 ビットの `authentication` を除く) |
| `ospfv3.checksum` | IPv6 疑似ヘッダ込みのインターネットチェックサム |
| `ospfv2_lsa.ls_type` / `ospfv3_lsa.ls_type` | LSA 本体レイヤ |
| `ospfv2_lsa.ls_checksum` / `ospfv3_lsa.ls_checksum` | Fletcher チェックサム (`ls_age` を除く、どの LS age でも正確) |
| `ospfv2_lsa.length` / `ospfv3_lsa.length` | LSA ヘッダ + 本体の長さ |
| `ospfv2_router_lsa.num_links` | `links` の要素数 |
| `bgp_open.opt_params_length` | オプションパラメータの長さ |
| `eigrp.checksum` | EIGRP パケット全体のインターネットチェックサム |

## 制約と近似

- **OSPFv2 の暗号認証**: チェックサムは `authentication` を除いて計算するため、認証なし / 平文パスワードでは正確です。暗号認証 (`auth_type` 2) ではチェックサムを計算しない規定なので `checksum: 0` を手動で指定してください。パケット後ろの暗号認証トレーラは扱いません。
- **LSU の LSA は 1 つ**: 型付きの LSA を繰り返し積む構造がないため、1 つの LSU に入る型付き LSA は 1 つです。`num_lsas` は Manual (既定 1) です。
- **同じ本体を共有する LS タイプ**: OSPFv2 の 3 / 4 と 5 / 7、OSPFv3 の `0x4005` / `0x2007` は本体レイヤが共通で、Auto では先頭のタイプ (3、5、`0x4005`) になります。もう一方は `ls_type` を手動で指定します (次プロトコル不一致の警告が出ます)。
- **DBD / LSAck 内の LSA ヘッダ**: `ls_checksum` / `length` は Manual です。
- **未対応の OSPF 要素**: OSPFv2 の TOS エントリ (Router-LSA の `num_tos` は 0 にする、Summary / External の TOS メトリック) は扱いません。OSPFv3 のプレフィックスリスト (4 オクテット境界の可変長アドレスプレフィックス) は opaque な `prefixes` / `address_prefix` で、`num_prefixes` は Manual です。
- **BGP UPDATE の 2 つの長さ**: `withdrawn_routes_length` と `total_path_attribute_length` は Manual です (既定値は既定の内容と一致します)。取り消し経路と NLRI は opaque な `(長さ, プレフィックス)` のオクテット列です。
- **BGP パス属性**: 属性は 16 ビット type (フラグ << 8 | タイプコード) と 8 ビット長の TLV です。Extended Length (フラグ `0x10`) の属性は `raw` 項目として書く必要があり、解析では読み戻せません。`AS_PATH` / `AGGREGATOR` は 4 オクテット AS 番号 (RFC 6793) です。拡張オプションパラメータ (RFC 9072) は扱いません。
- **BGP の解析**: 1 セグメントにつき 1 メッセージだけを解析します。
- **EIGRP の経路 TLV**: 末尾の宛先 (`ceil(prefix_length / 8)` オクテット) は TLV 値の終わりまでの `destination` オクテット列です。複数の宛先を持つ TLV はこの 1 フィールドに収まります。
