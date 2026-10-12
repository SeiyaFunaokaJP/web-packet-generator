# L2 制御プロトコル

Status: draft

LLDP / CDP / STP / LACP・Marker / EAPOL / PPPoE / PPP の補足です。定義は `crates/wpg-protocols/src/l2control.rs`、サンプルは `templates/protocols/l2/` にあります。

## 到達経路とサンプル

| id | プロトコル | 下位レイヤ | サンプル |
|---|---|---|---|
| `lldp` | LLDP (IEEE 802.1AB) | `ethernet2` EtherType `0x88cc` | `lldp.wpg.yaml` |
| `cdp` | CDP | `dot3` → `llc` → `snap` (OUI `0x00000c`、PID `0x2000`) | `cdp.wpg.yaml` |
| `stp` | STP / RSTP / MSTP BPDU | `dot3` → `llc` (DSAP/SSAP `0x42`) | `stp.wpg.yaml` |
| `lacp` | LACPDU (Slow Protocols subtype 1) | `ethernet2` EtherType `0x8809` | `lacp.wpg.yaml` |
| `marker` | Marker / Marker Response (subtype 2) | `ethernet2` EtherType `0x8809` | `marker.wpg.yaml` |
| `eapol` | EAPOL (IEEE 802.1X) | `ethernet2` EtherType `0x888e` | `eapol.wpg.yaml` |
| `pppoe_discovery` | PPPoE Discovery (PADI 〜 PADT) | `ethernet2` EtherType `0x8863` | `pppoe-discovery.wpg.yaml` |
| `pppoe_session` | PPPoE Session | `ethernet2` EtherType `0x8864` | `pppoe-ppp-lcp.wpg.yaml` |
| `ppp` / `ppp_lcp` / `ppp_ipcp` | PPP (LCP / IPCP) | `pppoe_session` | `pppoe-ppp-lcp.wpg.yaml`、`pppoe-ppp-ipcp.wpg.yaml` |

## Auto になるフィールド

- `cdp.checksum`: RFC 1071 のインターネットチェックサム。
- `eapol.length`、`pppoe_discovery.length`、`ppp_lcp.length` / `ppp_ipcp.length`: 後続部分の長さ。
- `ppp.protocol`: 次のレイヤ (LCP / IPCP / IPv4 / IPv6 …) から決まります。
- CDP / PPPoE / LCP / IPCP の TLV・オプションの長さ欄。

## 制約と近似

- **LLDP の TLV 長は Auto になりません。** LLDP の TLV ヘッダは 7 ビット type + 9 ビット length で、エンジンの TLV 型はオクテット単位の幅しか扱えません。そのため `tlvs` は `{ type: uint(7), length: uint(9), value }` のリストとして表現され、`length` は手動です。値を変えたら `length` も合わせてください。末尾の End TLV (type 0) も明示的に置きます。
- **CDP のチェックサム**は標準のインターネットチェックサムを計算します。Cisco 実装は PDU 長が奇数のとき末尾オクテットを符号拡張する癖があるため、**奇数長の PDU では実機と値が一致しません。** TLV を偶数長に揃えるか、チェックサムを手動で設定してください。
- **STP** は IEEE 802.1D の Configuration BPDU (35 オクテット) を正確に型付けします。RSTP / MSTP の拡張部 (Version 1 Length、Version 3 Length、MSTI 構成レコード) は不透明な `trailer` です。RST BPDU を作るには `protocol_version` と `bpdu_type` を設定し、`trailer` に Version 1 Length (`00`) を、MST BPDU では以降の MST 部も Hex で与えます。
- ブリッジ ID / ルート ID は 2 オクテットの priority (system-id 拡張込み) と 6 オクテットの MAC に分割されています。タイマは 1/256 秒単位です (`max_age` 20 秒 = `0x1400`)。
- **EAPOL** の本体 (type 0 なら EAP パケット) は不透明な `body` です。

<!-- wpg-docs-gen:begin reference-links -->
## 自動生成リファレンス

このページのプロトコルのフィールドリファレンス (定義から自動生成):

[`ethernet2`](../reference/protocols/ethernet2.md) · [`dot3`](../reference/protocols/dot3.md) · [`llc`](../reference/protocols/llc.md) · [`snap`](../reference/protocols/snap.md) · [`lldp`](../reference/protocols/lldp.md) · [`cdp`](../reference/protocols/cdp.md) · [`stp`](../reference/protocols/stp.md) · [`lacp`](../reference/protocols/lacp.md) · [`marker`](../reference/protocols/marker.md) · [`eapol`](../reference/protocols/eapol.md) · [`pppoe_discovery`](../reference/protocols/pppoe_discovery.md) · [`pppoe_session`](../reference/protocols/pppoe_session.md) · [`ppp`](../reference/protocols/ppp.md) · [`ppp_lcp`](../reference/protocols/ppp_lcp.md) · [`ppp_ipcp`](../reference/protocols/ppp_ipcp.md)
<!-- wpg-docs-gen:end reference-links -->
