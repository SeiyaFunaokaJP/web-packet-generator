# P1 プロトコル補足

Status: draft

[プロトコルリファレンス](../reference/protocols.md) はプロトコル定義 IR から自動生成した、プロトコルごとの「フィールド一覧」です。本章はそれを補う手書きのページで、P1 / P2 プロトコル群 (M6、M7) について次をまとめます。各ページの末尾には、扱っているプロトコルの生成ページへのリンクがあります。

- どの下位レイヤ・番号から到達するか (デコード時の demux)
- Auto で計算されるフィールドと、手動 (Manual) で設定が必要なフィールド
- エンジンの表現力の都合で残っている近似・制約
- 同梱サンプルテンプレート (`templates/protocols/`)

フィールドごとの説明 (ja/en) は GUI のフィールドエディタと `GET /api/protocols` に含まれます。制約の一次情報は `crates/wpg-protocols/src/` の各モジュール doc で、本章と食い違う場合はそちらが正です。

| ページ | プロトコル |
|---|---|
| [L2 制御プロトコル](l2-control.md) | LLDP / CDP / STP / LACP・Marker / EAPOL / PPPoE / PPP (LCP, IPCP) |
| [カプセル化・トンネル](encap.md) | VXLAN / GRE / NVGRE / MPLS / MPLS-in-UDP / L2TPv2・v3 / GTP-U / Geneve / IP-in-IP |
| [ルーティングプロトコル](routing.md) | OSPFv2・v3 / BGP-4 / RIPv1・v2 / RIPng / EIGRP |
| [冗長化 (FHRP)](fhrp.md) | VRRPv2・v3 / HSRPv1・v2 / GLBP |
| [マルチキャスト制御](multicast.md) | IGMPv1・v2・v3 / MLDv1・v2 / PIM-SM v2 |
| [アプリケーションプロトコル](app.md) | DHCPv4・v6 / DNS / NTP / SNMP / Syslog / RADIUS / TFTP |
| [OSI カプセル化 (P2)](osi.md) | 802.2 LLC `0xfe` / PPP `0x0023` 上の OSI PDU、NSAP 型 |
| [CLNP / ES-IS (P2)](clnp.md) | オプション・セグメンテーション付きの CLNP DT / ER / エコー、ES-IS ESH / ISH / リダイレクト |
| [IS-IS (P2)](isis.md) | LAN / P2P Hello、LSP、CSNP / PSNP、主要 TLV とサブ TLV |
| [SCTP (P2)](sctp.md) | CRC32c 付き共通ヘッダ、バンドルされたチャンク (DATA、INIT、SACK、HEARTBEAT、ABORT、ASCONF、RE-CONFIG など)、パラメータとエラー原因 |
| [OSPF Opaque / SR、BGP MP / EVPN (P2)](routing-ext.md) | SR の SID 付き OSPFv2 TE / Router Information / Extended Prefix / Extended Link LSA、IPv6・VPN-IPv4 / VPN-IPv6・EVPN の BGP NLRI |
| [LDP / RSVP-TE / BFD (P2)](mpls-bfd.md) | LDP の PDU・メッセージ・TLV (FEC、ラベル、Hello / セッションパラメータ)、RSVP-TE の Path / Resv などとオブジェクト (ERO / RRO、SESSION_ATTRIBUTE、IntServ)、BFD 制御パケットと認証 (Micro-BFD 含む) |
| [SRv6 SRH / SR-MPLS (P2)](srv6.md) | SRv6 Segment Routing Header (セグメントリスト、Pad / HMAC TLV、最終宛先での上位層チェックサム、IPv4 / IPv6 / Ethernet のカプセル化)、SR-MPLS のラベルスタック (予約ラベル名、SRGB + インデックス、エントロピーラベル) |
| [PTP / ESMC / CFM / R-APS / 802.3ah OAM (P2)](timing-oam.md) | Ethernet / UDP 上の PTP メッセージと TLV、SyncE ESMC、Ethernet CFM / Y.1731 (CCM、LB、LT、AIS / LCK、LM / DM / SLM など) と ERPS R-APS、802.3ah OAMPDU |
| [MACsec / GTPv2-C / Diameter / SIP / RTP (P2)](mobile-voip.md) | MACsec の SecTAG (セキュアデータと ICV は不透明)、GTPv2-C のメッセージと IE (グループ化 IE の Bearer Context)、Diameter のメッセージと AVP (ベンダ固有、グループ化)、SIP の開始行・ヘッダ行・本文、RTP ヘッダと RTCP 複合パケット (SR / RR / SDES / BYE / APP) |

## 共通の考え方

- **ヘッダ + 本体の分割**: OSPF・BGP・IGMP・PIM・ICMPv6 (MLD) のように「共通ヘッダの `type` で本体の形が変わる」プロトコルは、共通ヘッダと本体を別レイヤとして積みます (例: `ospfv2` → `ospfv2_hello`)。共通ヘッダの `type` は次のレイヤから Auto で決まるので、通常は本体レイヤを選ぶだけで済みます。
- **再デコード (re-parse)**: 同じ番号・ポートで複数の形がある場合 (VRRPv2/v3、HSRPv1/v2、DHCPv6 リレー、SNMP Trap、TFTP の各 opcode) は、まず代表の定義でデコードし、版数・種別の値を見て別定義に読み直します。ビルド時は目的の定義を直接選んでください。
- **先頭ニブルによる判別**: MPLS の最下段ラベル、GTP-U、PIM Register の内側パケットは、先頭 4 ビット (4 → IPv4、6 → IPv6) で判別します。
- **送信元ポートへのフォールバック**: UDP / TCP は宛先ポートで判別し、未知なら送信元ポートで判別します。応答方向 (DNS 応答、BGP の受け側など) もデコードできます。
- **不透明 (opaque) 領域**: 型付けできない部分はバイト列のまま保持します。ビルド時は Hex で値を与え、デコード時もバイト列として表示されます。

## サンプルを試す

```console
$ wpg template lint templates/protocols/routing/*.wpg.yaml
$ wpg render templates/protocols/routing/ospfv2-hello.wpg.yaml -o /tmp/ospf.pcapng
$ tshark -r /tmp/ospf.pcapng -V
```

すべてのサンプルは `crates/wpg-template/tests/samples.rs` で lint と render が検査されます。P1 の golden バイトテストは `crates/wpg-protocols/tests/p1_*.rs` にあり、`WPG_TSHARK=1` を付けて実行すると tshark でのデコード (プロトコルが認識され malformed にならないこと) も確認します。
