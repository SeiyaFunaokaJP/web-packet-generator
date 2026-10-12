# L2 制御プロトコル

Status: draft

LLDP、CDP、STP 系、LACP / Marker (Slow Protocols)、EAPOL、PPPoE、PPP (LCP / IPCP) の補足です。定義は `crates/wpg-protocols/src/l2control.rs` にあります。

## 識別と積み方

| プロトコル ID | 下位レイヤ | 識別子 | 宛先 MAC の例 |
|---|---|---|---|
| `lldp` | `ethernet2` | EtherType `0x88cc` | `01:80:c2:00:00:0e` |
| `cdp` | `dot3` + `llc` + `snap` | OUI `0x00000c` / PID `0x2000` | `01:00:0c:cc:cc:cc` |
| `stp` | `dot3` + `llc` | DSAP / SSAP `0x42` | `01:80:c2:00:00:00` |
| `lacp` / `marker` | `ethernet2` | EtherType `0x8809` (Slow Protocols、subtype 1 / 2) | `01:80:c2:00:00:02` |
| `eapol` | `ethernet2` | EtherType `0x888e` | `01:80:c2:00:00:03` |
| `pppoe_discovery` | `ethernet2` | EtherType `0x8863` | PADI はブロードキャスト |
| `pppoe_session` → `ppp` | `ethernet2` | EtherType `0x8864` | |
| `ppp_lcp` / `ppp_ipcp` | `ppp` | PPP protocol `0xc021` / `0x8021` | |

PPP の上には `ppp_lcp`、`ppp_ipcp`、`ipv4`、`ipv6` を置けます。`ppp.protocol` は次のレイヤから Auto です。

## Auto フィールド

| プロトコル | Auto |
|---|---|
| `cdp` | `checksum` (RFC 1071 のインターネットチェックサム) |
| `eapol` | `length` (`body` の長さ) |
| `pppoe_discovery` | `length` (TAG 群の長さ) |
| `pppoe_session` | `length` (PPP フレームの長さ) |
| `ppp_lcp` / `ppp_ipcp` | `length` (CP パケット全体) |

## 制約と回避方法

- **LLDP の TLV 長は Manual です。** LLDP の TLV ヘッダは 7 ビットの type と 9 ビットの length で、エンジンの TLV 型はオクテット単位の幅しか扱えないため、`tlvs` は `{ type, length, value }` のリストとして表現しています。リスト要素内の長さは自動計算されないので、`length` を `value` のオクテット数に合わせて書いてください。末尾の End TLV (`type: 0, length: 0`) も明示します。
- **CDP のチェックサム**は標準の RFC 1071 で計算します。Cisco 実装は PDU が奇数長のとき末尾オクテットを符号拡張するため、奇数長の PDU では Cisco の値と一致しません。必要なら `checksum` を Manual で指定します。
- **STP** は IEEE 802.1D の Configuration BPDU (35 オクテット) を正確に表現します。RSTP / MSTP の拡張部 (version-1 length、v3 length、MSTI レコード) は opaque な `trailer` に入れます。RST BPDU は `protocol_version: rstp` (2)、`bpdu_type: rapid` (0x02) にして `trailer` に `00` (version-1 length) を入れてください。
- タイマ (`max_age` / `hello_time` / `forward_delay`) は 1/256 秒単位です。ブリッジ / ルート ID は 2 オクテットの優先度 (システム ID 拡張込み) と 6 オクテットの MAC に分かれています。
- **LACPDU / Marker PDU** は 110 オクテット固定です。Actor / Partner の state はフラグフィールドで、`activity|aggregation|synchronization` のように書けます。
- **EAPOL** の `body` (EAP パケット) は opaque です。

## サンプル

| テンプレート | 内容 |
|---|---|
| `templates/protocols/l2/lldp.wpg.yaml` | Chassis ID / Port ID / TTL / System Name / End |
| `templates/protocols/l2/cdp.wpg.yaml` | CDPv2 (Device ID / Port ID / Capabilities) |
| `templates/protocols/l2/stp.wpg.yaml` | ルートブリッジの 802.1D Configuration BPDU |
| `templates/protocols/l2/lacp.wpg.yaml` / `marker.wpg.yaml` | LACPDU / Marker PDU |
| `templates/protocols/l2/eapol.wpg.yaml` | EAP Response/Identity を運ぶ EAPOL-EAP |
| `templates/protocols/l2/pppoe-discovery.wpg.yaml` | PADI |
| `templates/protocols/l2/pppoe-ppp-lcp.wpg.yaml` / `pppoe-ppp-ipcp.wpg.yaml` | PPPoE Session 上の LCP / IPCP Configure-Request |
