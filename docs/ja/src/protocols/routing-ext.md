# OSPF Opaque / SR、BGP MP / VPN / EVPN (P2)

Status: draft

M7 のルーティング拡張の補足です。OSPFv2 の Opaque LSA (RFC 5250) と、その中身の Traffic Engineering (RFC 3630)、Router Information (RFC 7770)、Extended Prefix / Extended Link (RFC 7684)、Segment Routing (RFC 8665)、および BGP マルチプロトコル (RFC 4760) の IPv4 / IPv6 ユニキャスト、VPN-IPv4 / VPN-IPv6 (RFC 4364 / RFC 4659)、EVPN (RFC 7432 / RFC 9136) の NLRI を扱います。定義は `crates/wpg-protocols/src/ospf_opaque.rs` と `crates/wpg-protocols/src/bgp_mp.rs` にあります。OSPF / BGP の基本は [ルーティングプロトコル](routing.md) のページを参照してください。

## OSPFv2 Opaque LSA

```text
ipv4 / ospfv2 / ospfv2_lsu / ospfv2_opaque_lsa / ospfv2_te_lsa
ipv4 / ospfv2 / ospfv2_lsu / ospfv2_opaque_lsa / ospfv2_ri_lsa
ipv4 / ospfv2 / ospfv2_lsu / ospfv2_opaque_lsa / ospfv2_ext_prefix_lsa
ipv4 / ospfv2 / ospfv2_lsu / ospfv2_opaque_lsa / ospfv2_ext_link_lsa
```

`ospfv2_opaque_lsa` は Link State ID を `opaque_type` (8 ビット、本体レイヤから自動) と `opaque_id` (24 ビット) に分けた LSA ヘッダです。`ls_type` がフラッディング範囲を表します: 9 リンクローカル、10 エリア (既定)、11 AS。LSA の `length` と Fletcher の `ls_checksum` は他の LSA と同じく自動です。解析では、タイプ 9 / 10 / 11 の LSA をまず `ospfv2_lsa` として読み、`ospfv2_opaque_lsa` として読み直します。未知の Opaque タイプの後ろは `raw` レイヤになります。

| サンプル | 内容 |
|---|---|
| `ospfv2-lsu-te-lsa` | TE LSA: リンクタイプ / ID、インタフェースアドレス、TE メトリック、帯域、管理グループを持つ Link TLV |
| `ospfv2-lsu-ri-sr` | Router Information LSA: 機能フラグ、ホスト名、SR-Algorithm、SRGB (SID/Label Range)、SRLB |
| `ospfv2-lsu-ext-prefix-sr` | Extended Prefix LSA: 192.0.2.1/32 にノード Prefix SID (インデックス 1) |
| `ospfv2-lsu-ext-link-sr` | Extended Link LSA: ポイントツーポイントリンクに Adj-SID (ラベル 24000) |

### TLV

本体はどれも `tlvs` フィールド 1 つです。TLV / sub-TLV は 16 ビットのタイプと、値だけを数える 16 ビットの長さを持ち、値は 4 オクテット境界までゼロで詰められます (詰め物は長さに含めません)。長さと詰め物は自動です。

| 本体 | TLV | sub-TLV |
|---|---|---|
| `ospfv2_te_lsa` (Opaque タイプ 1) | `router_address` (1)、`link` (2) | `link_type`、`link_id`、`local_address`、`remote_address`、`te_metric`、`max_bandwidth`、`max_reservable_bandwidth`、`unreserved_bandwidth`、`admin_group` |
| `ospfv2_ri_lsa` (4) | `informational_capabilities` (1)、`functional_capabilities` (2)、`hostname` (7)、`sr_algorithm` (8)、`sid_label_range` (9)、`sr_local_block` (14)、`srms_preference` (15) | `sid_label` / `sid_label_index` |
| `ospfv2_ext_prefix_lsa` (7) | `extended_prefix` (1)、`extended_prefix_range` (2) | `prefix_sid` / `prefix_sid_label` |
| `ospfv2_ext_link_lsa` (8) | `extended_link` (1) | `adj_sid` / `adj_sid_index`、`lan_adj_sid` / `lan_adj_sid_index` |

SID は 3 オクテット (下位 20 ビットがラベル。V / L フラグを立てる) か 4 オクテット (インデックス。V / L は立てない) です。SID を運ぶ sub-TLV はどれも両方の形を持つので、名前で項目を選び (`label` か `index` フィールド)、解析では値の長さで区別されます:

```yaml
- protocol: ospfv2_ext_prefix_lsa
  fields:
    tlvs:
      - extended_prefix:
          route_type: intra_area
          prefix_length: 32
          flags: n
          prefix: 192.0.2.1
          sub_tlvs:
            - prefix_sid: { algorithm: 0, index: 1 }          # 4 オクテット
            # - prefix_sid_label: { flags: v|l, label: 16001 } # 3 オクテット
```

TE の帯域は、オクテット / 秒を表す 32 ビット IEEE 浮動小数点数の生の値です (`0x4b3ebc20` = 12.5e6 = 100 Mbit/s)。

## BGP マルチプロトコル NLRI

`MP_REACH_NLRI` (コード 14) と `MP_UNREACH_NLRI` (コード 15) は、続く AFI / SAFI によって形が変わります。対応するファミリはそれぞれ別のパス属性項目で、解析では値の先頭 3 オクテット (AFI、SAFI) で選ばれます:

| ファミリ | AFI / SAFI | 項目 | ネクストホップ | NLRI の要素 |
|---|---|---|---|---|
| IPv4 ユニキャスト | 1 / 1 | `mp_reach_ipv4_unicast`、`mp_unreach_ipv4_unicast` | IPv4 アドレスのリスト | `{length, prefix}` |
| IPv6 ユニキャスト | 2 / 1 | `mp_reach_ipv6_unicast`、`mp_unreach_ipv6_unicast` | IPv6 アドレスのリスト (グローバル、リンクローカル) | `{length, prefix}` |
| VPN-IPv4 | 1 / 128 | `mp_reach_vpnv4`、`mp_unreach_vpnv4` | `{rd, address}` のリスト | `{length, label, exp, bos, rd, prefix}` |
| VPN-IPv6 | 2 / 128 | `mp_reach_vpnv6`、`mp_unreach_vpnv6` | `{rd, address}` のリスト | 同上 |
| EVPN | 25 / 70 | `mp_reach_evpn`、`mp_unreach_evpn` | オクテット列 (IPv4 か IPv6 アドレス) | EVPN ルート項目 |

それ以外のファミリは汎用の `mp_reach_nlri` / `mp_unreach_nlri` を使い、ネクストホップと NLRI はオクテット列です。`afi` / `safi` の既定値はそのファミリの値、`next_hop_length` は自動です。

- **プレフィックス**: `length` はビット単位で、`prefix` は `ceil(length / 8)` オクテットです。UPDATE 自体の `withdrawn_routes` と `nlri` もプレフィックスのリストで、UPDATE の 2 つの長さフィールドは自動です。
- **VPN プレフィックス**: `length` はラベル (24 ビット)、ルート識別子 (64 ビット)、プレフィックスのビット数の合計で、/24 なら 112 です。ラベルは 1 つ (`bos: 1`) を扱います。撤回では通常 `label: 0x80000, bos: 0` (互換値 0x800000) を使います。
- **ルート識別子 (RD)** は 8 オクテットです: `0000 fde9 00000064` が 65001:100 (タイプ 0)、`0001 c0000201 0001` が 192.0.2.1:1 (タイプ 1)。
- **EVPN ルート** (8 ビットのタイプ、8 ビットの長さ): `ethernet_ad` (1)、`mac_ip` (2)、`inclusive_multicast` (3)、`ethernet_segment` (4)、`ip_prefix` (5、IPv4、34 オクテット)、`ipv6_prefix` (5、IPv6、58 オクテット)。ESI は 10 オクテットです。ラベルのフィールドは 3 オクテットのワイヤ上の値で、MPLS なら `label << 4 | bos`、VXLAN なら VNI です (RFC 8365)。長さフィールド (`ip_length`、ビット単位) を持つ IP アドレスはオクテット列です。
- **PMSI Tunnel** 属性 (`pmsi_tunnel`、コード 22) は型付きです。ルートターゲットは `extended_communities` の値です (`0x0002fde900000064` = RT 65001:100)。

```yaml
- protocol: bgp_update
  fields:
    path_attributes:
      - { origin: 0 }
      - { as_path: { segments: [] } }
      - mp_reach_vpnv4:
          next_hop: [{ rd: "0000000000000000", address: 192.0.2.1 }]
          nlri:
            - { length: 112, label: 16, bos: 1, rd: "0000 fde9 00000064", prefix: "c6 33 64" }
    nlri: []
```

| サンプル | 内容 |
|---|---|
| `bgp-update-mp-ipv6` | IPv6 ユニキャストの広告 (グローバル + リンクローカルのネクストホップ) と撤回 |
| `bgp-update-vpnv4` | ラベル 16、RD とルートターゲット 65001:100 の VPN-IPv4 ルート |
| `bgp-update-evpn` | VNI 10010 の EVPN MAC/IP と Inclusive Multicast ルート、ルートターゲット、VXLAN カプセル化、PMSI Tunnel |

### Extended Length

値が 255 オクテットを超えるパス属性には、Extended Length フラグ (`0x10`) と 16 ビットの長さが自動で付きます。短い属性を長い形式で送る (FRR や GoBGP が `MP_REACH_NLRI` でそうするように) には、項目にタイプを明示します:

```yaml
- { name: mp_reach_ipv4_unicast, type: 0x900e, fields: { next_hop: [192.0.2.1], nlri: [{ length: 24, prefix: "c63364" }] } }
```

受信した属性は形式を保ちます: 解析した Extended Length の属性は `type` 付きでエクスポートされます。

## 制約

- OSPFv3 の Extended LSA (RFC 8362) と OSPFv3 の Segment Routing (RFC 8666) は扱いません。
- OSPF TE の GMPLS sub-TLV (RFC 4203) と、上に挙げた以外の RI TLV は raw 項目のままです。
- BGP の ADD-PATH のパス識別子 (RFC 7911)、2 つ以上のラベルのスタック (RFC 8277)、BGP Prefix-SID 属性 (RFC 8669) は扱いません。そうした NLRI は汎用の `mp_reach_nlri` / `mp_unreach_nlri` 項目にオクテット列で書いてください。こうした NLRI を解析するとオクテットは同一に再構成されますが、デコードされたフィールドは実際の区切りとずれます。
- 1 セグメント内の複数の BGP メッセージ、1 LSU 内の複数の型付き LSA は扱いません ([ルーティングプロトコル](routing.md) を参照)。

<!-- wpg-docs-gen:begin reference-links -->
## 自動生成リファレンス

このページのプロトコルのフィールドリファレンス (定義から自動生成):

[`ospfv2_lsa`](../reference/protocols/ospfv2_lsa.md) · [`ospfv2_opaque_lsa`](../reference/protocols/ospfv2_opaque_lsa.md) · [`ospfv2_te_lsa`](../reference/protocols/ospfv2_te_lsa.md) · [`ospfv2_ri_lsa`](../reference/protocols/ospfv2_ri_lsa.md) · [`ospfv2_ext_prefix_lsa`](../reference/protocols/ospfv2_ext_prefix_lsa.md) · [`ospfv2_ext_link_lsa`](../reference/protocols/ospfv2_ext_link_lsa.md) · [`raw`](../reference/protocols/raw.md)
<!-- wpg-docs-gen:end reference-links -->
