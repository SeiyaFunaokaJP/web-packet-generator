# IS-IS (P2)

Status: draft

IS-IS (ISO/IEC 10589、RFC 1195 と下記の TLV 関連 RFC) の補足です (M7)。定義は `crates/wpg-protocols/src/isis.rs`、サンプルは `templates/protocols/routing/isis-*.wpg.yaml` にあります。IS-IS は [OSI カプセル化](osi.md) の上で直接動作します。

## PDU・到達経路・サンプル

| id | PDU (種別) | サンプル |
|---|---|---|
| `isis_l1_lan_hello` / `isis_l2_lan_hello` | LAN Hello L1 (15) / L2 (16) | `isis-l1-lan-hello` (パディングあり)、`isis-l2-lan-hello` |
| `isis_p2p_hello` | ポイントツーポイント Hello (17) | `isis-p2p-hello` (3-way 隣接、パディングあり) |
| `isis_l1_lsp` / `isis_l2_lsp` | LSP L1 (18) / L2 (20) | `isis-l1-lsp` (ナローメトリック)、`isis-l2-lsp` (ワイドメトリック・TE・SR) |
| `isis_l1_csnp` / `isis_l2_csnp` | CSNP L1 (24) / L2 (25) | `isis-l1-csnp`、`isis-l2-csnp` |
| `isis_l1_psnp` / `isis_l2_psnp` | PSNP L1 (26) / L2 (27) | `isis-l1-psnp`、`isis-l2-psnp` |
| `isis` | 任意の PDU: 共通ヘッダ + 不透明なバイト列 | `isis-generic` |

各 PDU は 1 レイヤで、8 オクテットの共通ヘッダ、PDU 固有の固定フィールド、TLV から成ります。

```text
dot3 / llc / isis_l1_lan_hello      # 宛先 01:80:c2:00:00:14 (AllL1ISs)、L2 は ...:15
dot3 / llc / isis_p2p_hello         # Ethernet では 09:00:2b:00:00:05 (AllISs)
ppp / isis_l2_lsp                   # PPP protocol 0x0023
```

LLC の DSAP / SSAP (`0xfe`) と PPP の protocol (`0x0023`) は Auto です。解析時は NLPID `0x83` で汎用の `isis` が選ばれ、その `pdu_type` でレイヤが専用 PDU として読み直されます。未知の PDU 種別は `isis` のままです。

## Auto フィールド

| フィールド | 規則 |
|---|---|
| `pdu_length` | PDU 全体の長さ |
| LSP の `checksum` | LSP ID から PDU 末尾までの ISO 8473 Fletcher チェックサム。LSP ID より前のフィールド (共通ヘッダ・PDU 長・残り寿命) は除外されるので、寿命の値によらず正しい |
| Hello のパディング | Hello の `pdu_length` を Manual にすると、PDU がちょうどその長さになるまで Padding TLV (種別 8、1 つ最大 255 オクテット) を追加します。例: Ethernet MTU 1500 なら LLC ヘッダ 3 オクテットを引いて `pdu_length: 1497`。Auto ならパディングしません。不足分が TLV で作れない (1 オクテット) 場合は警告が出ます |
| `sub_tlvs_length` (拡張 IS / IP / IPv6 到達性) | 続く `sub_tlvs` のサイズ |
| `sub_tlvs_present` (TLV 135 / 236 の S ビット) | エントリに `sub_tlv_block` があれば 1、なければ 0 |

`header_length` は PDU 種別ごとの定数 (27 / 20 / 33 / 17) で、Auto ではなく既定値を持ちます。変更すると警告が出ます。

## TLV

| コード | 名前 (`name`) | 値のフィールド |
|---|---|---|
| 1 | `area_addresses` | `addresses`: `{address}` のリスト (NSAP、例 `49.0001`) |
| 2 | `is_reachability` | `virtual_flag`、`neighbors`: `{default_metric, delay_metric, expense_metric, error_metric, neighbor_id}` |
| 6 | `is_neighbors` | `neighbors`: MAC のリスト |
| 8 | `padding` | `data` |
| 9 | `lsp_entries` | `entries`: `{remaining_lifetime, lsp_id, sequence_number, checksum}` |
| 10 | `authentication` | `auth_type` (`cleartext` / `hmac_md5`)、`value` |
| 22 | `extended_is_reachability` | `neighbors`: `{neighbor_id, metric, sub_tlvs_length, sub_tlvs}` |
| 128 / 130 | `ip_internal_reachability` / `ip_external_reachability` | `prefixes`: `{4 種のメトリック, ip_address, subnet_mask}` |
| 129 | `protocols_supported` | `nlpids` (`ipv4`、`ipv6`、`clnp`) |
| 132 | `ip_interface_addresses` | `addresses` |
| 134 | `te_router_id` | `router_id` |
| 135 | `extended_ip_reachability` | `prefixes`: `{metric, up_down, sub_tlvs_present, prefix_length, prefix, sub_tlv_block}` |
| 137 | `dynamic_hostname` | `hostname` |
| 232 | `ipv6_interface_addresses` | `addresses` |
| 236 | `ipv6_reachability` | `prefixes`: `{metric, up_down, external, sub_tlvs_present, prefix_length, prefix, sub_tlv_block}` |
| 240 | `p2p_adjacency_state` | `adjacency_state`、`extended_local_circuit_id`、`neighbor_system_id`、`neighbor_extended_local_circuit_id` |
| 242 | `router_capability` | `router_id`、`flags` (S / D)、`sub_tlvs` |

サブ TLV:

- 拡張 IS 到達性 (22): 管理グループ (3)、リンクローカル / リモート ID (4)、IPv4 インタフェース / 隣接アドレス (6 / 8)、最大 / 予約可能 / 未予約帯域 (9 / 10 / 11)、TE デフォルトメトリック (18)、Adj-SID / LAN-Adj-SID (31 / 32)。
- 拡張 IP / IPv6 到達性 (135 / 236): 32 / 64 ビット管理タグ (1 / 2)、Prefix-SID (3)、プレフィックス属性フラグ (4)。
- ルータ能力 (242): SR 能力 (2)、SR アルゴリズム (19)。

`prefix` は `ceil(prefix_length / 8)` オクテットで、有効なオクテットだけを書きます (`192.0.2.0/24` は `prefix_length: 24, prefix: "c00002"`)。プレフィックスの省略可能なサブ TLV は要素 1 個の `sub_tlv_block` リストとして書きます。

```yaml
- extended_ip_reachability:
    prefixes:
      - metric: 0
        prefix_length: 32
        prefix: "c0000201"
        sub_tlv_block:
          - sub_tlvs:
              - { prefix_sid: { flags: 0x40, algorithm: 0, sid: "00000001" } }
```

その他の TLV は `{ type: N, data: "..." }` として保持されます。

## 制約

- System ID は 6 オクテット (`id_length` 0) 固定で、それ以外の ID 長はモデル化していません。
- TE の帯域は 32 ビット IEEE 浮動小数点の生値です。SID / ラベルと SR 能力の範囲はオクテット列 (3 オクテットのラベルまたは 4 オクテットのインデックス) です。
- HMAC-MD5 認証値 (および RFC 5310 の暗号認証) は計算しません。
- TLV 240 は 15 オクテット形式でモデル化しています。短い形式 (1 / 5 オクテット) は `data` として読み戻されます。
