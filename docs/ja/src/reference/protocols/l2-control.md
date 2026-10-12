# L2 制御プロトコル (P1)

Status: draft

定義: `crates/wpg-protocols/src/l2control.rs` / テスト: `crates/wpg-protocols/tests/p1_l2.rs` / サンプル: `templates/protocols/l2/`

## 対象と到達方法

| id | プロトコル | 下位からの到達 | サンプル |
|---|---|---|---|
| `lldp` | LLDP (IEEE 802.1AB) | EtherType `0x88cc` | `lldp` |
| `cdp` | CDP | 802.3 + LLC + SNAP (OUI `0x00000c`, PID `0x2000`) | `cdp` |
| `stp` | STP / RSTP / MSTP BPDU | 802.3 + LLC (DSAP/SSAP `0x42`) | `stp` |
| `lacp` | LACPDU (Slow Protocols subtype 1) | EtherType `0x8809` | `lacp` |
| `marker` | Marker / Marker-Response (subtype 2) | 構築のみ (`ethertype: 0x8809` を手動指定) | `marker` |
| `eapol` | EAPOL (IEEE 802.1X) | EtherType `0x888e` | `eapol` |
| `pppoe_discovery` | PPPoE Discovery (PADI〜PADT) | EtherType `0x8863` | `pppoe-discovery` |
| `pppoe_session` | PPPoE Session | EtherType `0x8864` | `pppoe-ppp-lcp` |
| `ppp` | PPP フレーム | `pppoe_session` の次 (既定) | `pppoe-ppp-lcp` |
| `ppp_lcp` / `ppp_ipcp` | LCP / IPCP | PPP protocol `0xc021` / `0x8021` | `pppoe-ppp-lcp` / `pppoe-ppp-ipcp` |

PPP の上には IPv4 (`0x0021`) と IPv6 (`0x0057`) もそのまま積めます。

## 積み方

- **CDP**: `dot3` → `llc` → `snap` (`oui: 0x00000c`、`pid` は Auto で `0x2000`) → `cdp`。宛先は `01:00:0c:cc:cc:cc`。`dot3` の `length` は `auto` にします。
- **STP**: `dot3` → `llc` → `stp`。`llc` の DSAP / SSAP は Auto で `0x42` になります。宛先は `01:80:c2:00:00:00`。
- **LACP / Marker**: `ethernet2` → `lacp` (EtherType は Auto)。Marker は EtherType の対応表に載っていないため `ethertype: 0x8809` を手動で指定します。解析すると Marker PDU も `lacp` として読まれます。
- **PPPoE**: `ethernet2` → `pppoe_session` → `ppp` → `ppp_lcp` / `ppp_ipcp` / `ipv4` / `ipv6`。`ppp.protocol` は上のレイヤから Auto。

## Auto フィールド

| フィールド | 内容 |
|---|---|
| `cdp.checksum` | RFC 1071 のインターネットチェックサム (ヘッダ + TLV) |
| `eapol.length` | `body` の長さ |
| `pppoe_discovery.length` | タグ部分の長さ |
| `pppoe_session.length` | ペイロード (PPP フレーム) の長さ |
| `ppp.protocol` | 次のレイヤ (LCP / IPCP / IPv4 / IPv6) |
| `ppp_lcp.length` / `ppp_ipcp.length` | コントロールプロトコルパケット全体の長さ |

## 制約と近似

- **LLDP の TLV 長は Manual**: LLDP の TLV ヘッダは 7 ビット type + 9 ビット length で、TLV 定義が扱えるのはオクテット単位の幅だけです。そのため `tlvs` は `{type, length, value}` 構造体のリストで、`length` は自動計算されません。`value` を変えたら `length` も合わせてください。既定値は Chassis ID / Port ID (MAC)、TTL 120、End TLV です。
- **CDP のチェックサム**: Auto は標準の RFC 1071 値です。PDU 長が偶数なら Cisco 実装と一致します。奇数長では Cisco は末尾オクテットを符号拡張するため一致しません。その場合は `checksum` を手動で指定してください。
- **STP**: 802.1D の Configuration BPDU (35 オクテット) は正確に型付けされています。RSTP / MSTP の拡張部 (version-1 length、v3 length、MSTI レコード) は opaque な `trailer` です。ブリッジ ID / ルート ID は 2 オクテットの優先度 (system-id 拡張込み) と 6 オクテットの MAC に分かれています。タイマは 1/256 秒単位です (`max_age` 20 秒 = `0x1400`)。
- **LACP / Marker**: 110 オクテット固定です。各 TLV の `*_info_type` / `*_info_length` は既定値が規格どおりの固定値で、Auto ではありません。
- **EAPOL**: `body` (タイプ 0 なら EAP パケット) は opaque です。
