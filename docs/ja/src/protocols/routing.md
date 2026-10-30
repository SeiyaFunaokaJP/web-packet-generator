# ルーティングプロトコル

Status: draft

OSPFv2 / OSPFv3 / BGP-4 / RIPv1・v2 / RIPng / EIGRP の補足です。定義は `crates/wpg-protocols/src/routing.rs`、サンプルは `templates/protocols/routing/` にあります。OSPFv2 の Opaque LSA / Segment Routing と BGP マルチプロトコル NLRI (IPv6、VPN、EVPN) は [OSPF Opaque / SR、BGP MP / VPN / EVPN](routing-ext.md) のページで説明します。

## 到達経路とサンプル

| id | プロトコル | 下位レイヤ | サンプル |
|---|---|---|---|
| `ospfv2` + 本体 | OSPFv2 (RFC 2328) | IPv4 protocol 89 | `ospfv2-hello` / `-dbd` / `-lsr` / `-lsu-*` / `-lsack` (Opaque LSA: `ospfv2-lsu-te-lsa`、`-ri-sr`、`-ext-prefix-sr`、`-ext-link-sr`) |
| `ospfv3` + 本体 | OSPFv3 (RFC 5340) | IPv6 next header 89 | `ospfv3-hello` / `-dbd` / `-lsr` / `-lsu-*` / `-lsack` |
| `bgp` + 本体 | BGP-4 | TCP 179 (宛先、なければ送信元) | `bgp-open` / `-update` / `-keepalive` / `-notification` / `-route-refresh`、`bgp-update-mp-ipv6` / `-vpnv4` / `-evpn` |
| `rip` | RIPv1 / v2 | UDP 520 | `ripv1-request`、`ripv2-response` |
| `ripng` | RIPng | UDP 521 | `ripng-response` |
| `eigrp` | EIGRP (基本 TLV) | IP protocol 88 (IPv4 / IPv6) | `eigrp-hello`、`eigrp-update` |

### レイヤの積み方

OSPF と BGP は「共通ヘッダ + 本体」の 2 レイヤ (LSU はさらに LSA ヘッダ + LSA 本体) です。

```text
ipv4 / ospfv2 / ospfv2_hello
ipv4 / ospfv2 / ospfv2_lsu / ospfv2_lsa / ospfv2_router_lsa
ipv4 / ospfv2 / ospfv2_lsu / ospfv2_opaque_lsa / ospfv2_ri_lsa   # LS タイプ 9 / 10 / 11
ipv6 / ospfv3 / ospfv3_lsu / ospfv3_lsa / ospfv3_intra_area_prefix_lsa
ipv4 / tcp / bgp / bgp_update
ipv4 / tcp / bgp                 # KEEPALIVE はヘッダのみ (type 4 が既定)
```

## Auto になるフィールド

- OSPF ヘッダの `type`、`packet_length`、`checksum`。OSPFv3 のチェックサムは IPv6 疑似ヘッダ込みで正確です。
- LSA ヘッダの `ls_type`、`length`、`ls_checksum` (Fletcher、LS age を除外。どの age でも正確)。
- Router-LSA の `num_links`。
- BGP ヘッダの `length`、`type`。OPEN の `opt_params_length`。UPDATE の `withdrawn_routes_length` と `total_path_attribute_length`。255 オクテットを超えるパス属性の Extended Length フラグ。
- EIGRP の `checksum`。

## 手動で合わせるフィールド

- `ospfv2_lsu.num_lsas` / `ospfv3_lsu` の LSA 数 (1 LSU に型付き LSA は 1 つなので通常 1)。
- DBD / LSAck の中に並ぶ LSA ヘッダの `ls_checksum`。

## 制約と近似

- **OSPFv2 の認証**: チェックサムは 64 ビットの認証フィールドを除いて計算するため、認証なし・平文パスワードでは正確です。暗号認証 (AuType 2) ではチェックサムを 0 に手動設定してください (ビルダーでは「イレギュラーを許可」をオンにすると上書きできます)。パケット後ろの暗号認証トレーラは表現されません。
- **1 LSU に型付き LSA は 1 つ**: 繰り返しレイヤの仕組みがないため、型付きの LSA 本体の後ろに別の LSA は続けられません。
- **本体を共有する LS タイプ**: OSPFv2 の 3 / 4 と 5 / 7、OSPFv3 の `0x4005` / `0x2007` は本体の形が同じため、Auto では前者のタイプになります。後者にするには `ls_type` を手動で設定します (next-protocol 不一致の警告が出ますが送出は可能です)。
- OSPFv2 の TOS エントリ (Router-LSA の `num_tos` > 0、Summary / External の TOS メトリック) と、OSPFv3 のプレフィックスリストは型付けされず、不透明な `prefixes` / `address_prefix` オクテットです。
- **BGP の経路 (NLRI / 取り下げ経路)** は `{length, prefix}` のリストです (長さはビット単位、プレフィックスは `ceil(長さ / 8)` オクテット)。16 進文字列もそのままのオクテット列として受け付けます。
- **BGP の経路属性**は 16 ビット type (flags << 8 | type code) の TLV です。長さは 8 ビット、Extended Length フラグ (`0x10`) 付きなら 16 ビットで、フラグは 255 オクテットを超えると自動で付き、`type` を明示した場合はその形式を保ちます。`AS_PATH` / `AGGREGATOR` は 4 オクテット AS 番号 (RFC 6793) です。拡張オプションパラメータ (RFC 9072) は未対応です。
- BGP はセグメントごとに先頭の 1 メッセージだけをデコードします。
- **EIGRP** の経路 TLV の宛先は `ceil(prefix_length / 8)` オクテットの可変長のため、TLV 値の末尾までを `destination` オクテットとして保持します。複数宛先の TLV はその 1 フィールドにまとまります。

<!-- wpg-docs-gen:begin reference-links -->
## 自動生成リファレンス

このページのプロトコルのフィールドリファレンス (定義から自動生成):

[`ospfv2`](../reference/protocols/ospfv2.md) · [`ospfv3`](../reference/protocols/ospfv3.md) · [`ospfv3_lsu`](../reference/protocols/ospfv3_lsu.md) · [`bgp`](../reference/protocols/bgp.md) · [`rip`](../reference/protocols/rip.md) · [`ripng`](../reference/protocols/ripng.md) · [`eigrp`](../reference/protocols/eigrp.md)
<!-- wpg-docs-gen:end reference-links -->
