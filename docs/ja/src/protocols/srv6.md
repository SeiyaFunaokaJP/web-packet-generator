# SRv6 SRH / SR-MPLS (P2)

Status: draft

SRv6 の Segment Routing Header (RFC 8754) と SR-MPLS (RFC 8660) の組み立て補助 (M7)。定義は `crates/wpg-protocols/src/ipv6.rs` (`ipv6_srh`) と `encap.rs` (`mpls`)、サンプルは `templates/protocols/srv6/`、`templates/protocols/mpls/sr-mpls-entropy`、`templates/scenarios/sr-mpls-prefix-sid-sweep` にあります。

## 到達経路とサンプル

```text
ethernet2 / ipv6 / ipv6_srh / udp|tcp|icmpv6   # SRH 付きの通常パケット
ethernet2 / ipv6 / ipv6_srh / ipv4 / ...       # H.Encaps → End.DT4 / End.DX4 (next header 4)
ethernet2 / ipv6 / ipv6_srh / ipv6 / ...       # H.Encaps → End.DT6 / End.DX6 (next header 41)
ethernet2 / ipv6 / ipv6_srh / ethernet2 / ...  # H.Encaps.L2 → End.DX2 (next header 143、RFC 8986)
ethernet2 / mpls / mpls / ... / ipv4|ipv6      # SR-MPLS ラベルスタック
```

IPv6 ルーティングヘッダ (next header 43) は、ルーティングタイプ (ヘッダの 3 オクテット目) が 4 のとき `ipv6_srh`、それ以外のとき汎用の `ipv6_route` としてデコードします。IPv6 の next header 143 (Ethernet) は `ethernet2` に対応します。

| サンプル | 内容 |
|---|---|
| `srv6-srh-udp` | 3 セグメント (残り 2) の SRH 付き IPv6/UDP。UDP チェックサムは最後のセグメント宛てで計算 |
| `srv6-h-encaps-ipv4` | IPv4/UDP を外側 IPv6 + SRH (End → End.DT4) でカプセル化 (L3VPN) |
| `srv6-end-dx2-hmac` | Ethernet フレーム (ARP) を HMAC TLV 付き SRH で End.DX2 へ (L2VPN) |
| `sr-mpls-entropy` | プレフィックス SID (SRGB + インデックス を変数で指定)、隣接 SID、ELI + エントロピーラベル |
| `sr-mpls-prefix-sid-sweep` (シナリオ) | プレフィックス SID のインデックスを 1〜100 で変化させ、エントロピーラベルもパケットごとに変える |

## SRH のフィールド

| フィールド | Auto / 手動 | 説明 |
|---|---|---|
| `next_header` | Auto | 次レイヤから |
| `hdr_ext_len` | Auto | 先頭 8 オクテットを除いた 8 オクテット単位の長さ |
| `routing_type` | 既定 4 | |
| `segments_left` | 手動 (既定 1) | 次に向かうセグメントの位置 |
| `last_entry` | Auto | セグメント数 - 1 |
| `flags` / `tag` | 手動 | |
| `segment_list` | 手動 | IPv6 アドレスのリスト。**経路の逆順**: 要素 0 が最後のセグメント |
| `tlvs` | 手動 | Pad1 / PadN / HMAC TLV。後ろの詰め物 (Pad1) は Auto |

`segment_list` は経路の逆順で書きます。送信元が出すパケットの IPv6 宛先は `segment_list[segments_left]` (アクティブセグメント) なので、`ipv6.dst` はそれに合わせて手動で設定してください (IPv6 宛先は Auto になりません)。

### 上位層チェックサムの宛先

`segments_left` が 0 でない間、UDP / TCP / ICMPv6 の疑似ヘッダの宛先は `ipv6.dst` ではなく最終宛先 `segment_list[0]` になります (RFC 8200 §8.1)。wpg はこれを自動で扱い、`segments_left` をジェネレータで変えた場合 (高速パッチ経路) も追従します。`segments_left` が 0 なら `ipv6.dst` を使います。汎用の `ipv6_route` (Type 0 / 2 / 3) はアドレスを `data` のオクテット列で持つため、この置き換えをしません。

### TLV

| TLV | 型 | 値 |
|---|---|---|
| `pad1` | 0 | なし (1 オクテット) |
| `padn` | 4 | `padding` (任意長) |
| `hmac` | 5 | `d` (1 ビット)、`reserved` (15 ビット)、`hmac_key_id` (32 ビット)、`hmac` (オクテット列、手動) |

TLV の後ろは 8 オクテット境界まで 0 (= Pad1) で自動的に埋めます。HMAC 値は計算しません (鍵と対象範囲を決めて外部で計算した値を入れてください)。

## SR-MPLS の補助

SR-MPLS の SID は通常の MPLS ラベルなので、`mpls` レイヤを並べて組み立てます。

- **予約ラベルの名前**: `label` は 0〜15 の予約ラベルを名前で書けます: `ipv4_explicit_null` (0)、`router_alert` (1)、`ipv6_explicit_null` (2)、`implicit_null` (3)、`entropy_label_indicator` (7)、`gal` (13)、`oam_alert` (14)、`extension` (15)。それ以外は数値で書きます。
- **プレフィックス SID**: ラベル = SRGB ベース + SID インデックス。`expr` ジェネレータと変数で `"${srgb_base} + ${sid_index}"` のように書けば、ルータの SRGB 設定に合わせて変数だけ変えられます (`sr-mpls-entropy` 参照)。インデックスを変えて多数の宛先ノードに送るには `idx` を足します (`sr-mpls-prefix-sid-sweep` 参照)。
- **エントロピーラベル** (RFC 6790): `entropy_label_indicator` (7) のラベルの直後にエントロピーラベルを置きます。エントロピーラベルの TTL は 0 にします。
- `s` (スタック最下位ビット) は Auto です。

## 制約 / 対象外

- `segments_left` と `last_entry` の整合 (`segments_left <= last_entry`) は検査しません。
- `ipv6.dst` をアクティブセグメントに合わせる処理は自動ではありません。
- HMAC の計算、SRv6 の Behavior (End / End.X / End.DT4 など) のシミュレーションはしません。SID の Behavior はアドレスの値で表すだけです。
- SRv6 圧縮 SID (uSID / CSID、RFC 9800) の専用表現はありません (128 ビットアドレスとして書けます)。
- SR-MPLS の SRGB / SRLB の範囲検査はしません。
