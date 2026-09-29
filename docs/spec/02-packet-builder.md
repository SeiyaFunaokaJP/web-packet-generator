# 02. パケットビルダー (コア)

Status: draft / 元: TODO.md §2.1 / 担当 crate: `wpg-core`, `wpg-protocols`

## 2.1.1 プロトコル定義エンジン

すべてのプロトコルを「宣言的なフィールド定義」で表現し、合成 / 解析 / GUI 生成 / ドキュメント生成を 1 ソース (IR) から行う。コアはRust マクロ、ユーザ拡張は YAML で書き、同一 IR にコンパイルする ([ADR-0007](../adr/0007-protocol-definition.md))。

### フィールド型

| 型 | 説明 |
|---|---|
| `uint(bits)` / `int` | ビット幅指定の整数 (ビットフィールド含む) |
| `bytes(len\|var)` | 固定長または可変長バイト列 |
| `mac` / `ipv4` / `ipv6` | アドレス型 |
| `nsap` | 可変長 OSI アドレス (AFI/IDI/HO-DSP/SystemID/SEL に分解表示) |
| `enum` | 名前付き値 |
| `flags(bit set)` | ビット集合 |
| `tlv` / `list<T>` | 反復・入れ子構造 |
| `padding` / `raw` | 詰め物 / 生バイト |

### 自動計算フィールド (GUI で Auto / Manual トグル)

- 長さ系: IPv4 total length, UDP length, IS-IS PDU length, TLV length など
- チェックサム系: IPv4 header, TCP/UDP (v4/v6 疑似ヘッダ), ICMP, OSPF, IS-IS (Fletcher), SCTP CRC32c, GRE など
- 次ヘッダ / EtherType / Protocol 番号: 下位レイヤの選択から自動決定し、上書き可
- ストリーム内カウンタ: シーケンス番号など

### 意図的に壊す (malformed)

任意フィールドの上書き、長さ不整合、ゴミ TLV、末尾切詰め、FCS 誤り (対応バックエンドのみ)。

### 制約検査

警告レベル (「この組合せは RFC 違反」) と エラーレベル (合成不能) を区別して GUI に表示する。malformed 指定時は警告に降格して合成を許可する (Proposal)。

## 2.1.2 対応プロトコル (優先度順)

### P0 (MVP)

- L2: Ethernet II, 802.3 + LLC/SNAP, 802.1Q, 802.1ad (QinQ), ARP
- L3: IPv4 (options 含む), IPv6 (拡張ヘッダ: HbH, Routing, Fragment, DstOpts), ICMPv4, ICMPv6 (NDP: RS/RA/NS/NA/Redirect)
- L4: TCP (options: MSS, WS, SACK, TS), UDP
- Payload: 固定 / パターン / ランダム / ファイル / インクリメントカウンタ埋め込み

### P1 (NW 検証で頻出)

- L2/制御: LLDP, CDP, STP/RSTP/MSTP (BPDU), LACP, 802.1X EAPOL, PPPoE (Discovery/Session), PPP (LCP/IPCP)
- カプセル化: VXLAN, GRE, NVGRE, MPLS (複数ラベル), MPLS over UDP, L2TPv2/v3, GTP-U, Geneve, IP-in-IP
- ルーティング: OSPFv2/v3 (Hello/DBD/LSR/LSU/LSAck, 主要 LSA), BGP (OPEN/UPDATE/KEEPALIVE/NOTIFICATION, 主要 attr/capability), RIPv1/v2, RIPng, EIGRP (基本)
- 冗長: VRRPv2/v3, HSRPv1/v2, GLBP (基本)
- マルチキャスト: IGMPv1/2/3, MLDv1/v2, PIM (Hello/Join-Prune/Register)
- アプリ: DHCPv4/DHCPv6 (Relay 含む), DNS, NTP, SNMP (v1/v2c GetRequest/Trap), Syslog, RADIUS, TFTP
- QoS/検証: DSCP/ECN/PCP 全パターン、ジャンボフレーム、パディング

### P2 (OSI / マニアック領域 — 本プロジェクトの目玉)

- IS-IS (ISO 10589 / RFC 1195 / RFC 5305 …): Hello (L1/L2 LAN IIH, P2P IIH), LSP, CSNP, PSNP
  - TLV: Area Address(1), IS Neighbors(2/6), Auth(10), Ext IS Reach(22), IP Int Reach(128/130), Prot Supported(129), IP Int Addr(132), Ext IP Reach(135), Hostname(137), IPv6 Reach(236), MT (229/222/235), SR (Router Cap 242, Prefix-SID/Adj-SID sub-TLV), Padding(8)
- CLNP (ISO 8473), ES-IS (ISO 9542: ESH/ISH/RD), NSAP アドレスエディタ
- 802.2 LLC (DSAP/SSAP 0xFE) 上の OSI カプセル化、SNAP
- SCTP (INIT/INIT-ACK/DATA/HEARTBEAT, CRC32c)
- OSPF 拡張 (Opaque LSA / SR)、BGP 拡張 (MP-BGP / EVPN / VPNv4 NLRI)
- MPLS 制御: LDP (Hello/Init/Address/Label Mapping), RSVP-TE (Path/Resv 基本)
- BFD (制御パケット), Micro-BFD
- Segment Routing: SRv6 (SRH), MPLS-SR ラベルスタック生成補助
- 802.11 (Radiotap + 管理フレーム、モニタモード対応 NIC 限定) — P3 相当、要望次第
- PTP (IEEE 1588 v2), SyncE ESMC, ERPS (G.8032 R-APS), Y.1731 CFM/OAM (CCM/LBM/LTM), EtherOAM (802.3ah)
- MACsec (フレーム構造のみ、暗号化は固定鍵)
- GTP-C, Diameter, SIP (基本), RTP

### P3 / 拡張機構

- ユーザ定義プロトコル: YAML でフィールド定義を書いてドロップインできる `protocols.d/` 機構
- Lua or WASM プラグインでカスタム計算フィールド (要検討)

## 2.1.3 ジェネレータ (変数・インクリメント・大量生成)

どのフィールドにもジェネレータを割り当てられる。

| ジェネレータ | 内容 |
|---|---|
| `range(start, end, step)` | 例 `192.168.1.1 – 192.168.5.254`。IPv4/IPv6/MAC/整数/NSAP SystemID 対応、CIDR 指定可 |
| `list([...])` | 列挙値を順に使用 |
| `random(min,max\|bits)` / `random_mac(oui)` | 乱数 / OUI 固定のランダム MAC |
| `file(lines)` | ファイルの各行 |
| `counter(start, step, wrap)` | カウンタ |
| `expr` | 簡易式 (他フィールド参照、算術、`hash()`、`crc()`) |

- 合成モード: `zip` (並行) / `product` (直積) / `nested` (優先順位付き)
- ストリーム: 1 テンプレ = 1 ストリーム。ジョブは複数ストリームを重み付き・順次・ラウンドロビンで混合。
- 送出制御: パケット数 / 時間 / 無限、レート (pps / bps / 回線率 %)、バースト (サイズ・間隔)、IFG、ランプアップ、スケジュール開始
- 展開方式: 事前展開 (pre-render) とオンザフライ生成をメモリ量とレートで自動選択 (ヒント指定可)。上限目安は事前展開 100M パケット、オンザフライは無制限。

## 2.1.4 逆生成 (import)

- pcap/pcapng → テンプレート化 (各パケットを解析してレイヤ/フィールドを復元。差分からのジェネレータ推定はベストエフォート)
- Hex / Wireshark 「Copy as Hex Stream」貼付け → テンプレート化
- Scapy 風テキスト (`Ether()/IP()/UDP()`) の簡易パーサ (P2)

## 未決事項 (Open questions)

- IR の具体的な表現 (フィールド間依存、条件付きフィールド、可変長 TLV の再帰表現) と、マクロ/YAML の記述力の差をどう吸収するか。
- `expr` の文法・評価器の実装方式 (自作 or 既存クレート)、無限ループ/DoS 防止。ブラウザ内評価との互換性。
- `zip` で長さが異なるジェネレータの扱い (最短で打ち切り / 循環 / エラー)。
- `nested` の「優先順位」の指定方法 (テンプレート上のキー名、配列順)。
- pre-render / on-the-fly 自動選択のしきい値 (メモリ上限の既定値)。
- malformed 指定と制約検査 (警告/エラー) の関係、FCS 誤りに対応するバックエンドの一覧。
- Lua / WASM プラグインの要否とサンドボックス方針 (P3)。
- 802.11 を P2 と P3 のどちらに置くか (TODO 内で P2 一覧にあるが P3 相当と注記)。
- 解析器のベストエフォート復元で、未知の TLV / 拡張ヘッダをどう保持するか (`raw` として保存するか)。
