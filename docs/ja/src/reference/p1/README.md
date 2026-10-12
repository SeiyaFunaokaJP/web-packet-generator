# P1 プロトコル補足

Status: draft

[プロトコルリファレンス](../protocols.md) のフィールド表は定義 IR から自動生成されます (M11)。本節はその補足として、自動生成では伝わらない次の情報を P1 プロトコルの分類ごとにまとめます。

- **積み方**: どのレイヤの上に置くか、下位レイヤのどの値 (EtherType / IP プロトコル番号 / UDP・TCP ポート) で識別されるか
- **Auto フィールド**: 合成時に自動計算される長さ・件数・チェックサム・次プロトコル番号
- **制約 (近似)**: エンジンの表現力の都合で opaque なバイト列や Manual にしている箇所と、その回避方法
- **サンプル**: `templates/protocols/` の同梱テンプレート

| 分類 | プロトコル |
|---|---|
| [L2 制御](l2-control.md) | LLDP / CDP / STP・RSTP・MSTP / LACP・Marker / EAPOL / PPPoE / PPP (LCP・IPCP) |
| [トンネル / カプセル化](tunnels.md) | VXLAN / GRE / NVGRE / MPLS / MPLS-in-UDP / L2TPv2・v3 / GTP-U / Geneve / IP-in-IP |
| [ルーティング](routing.md) | OSPFv2 / OSPFv3 / BGP-4 / RIPv1・v2 / RIPng / EIGRP |
| [FHRP](fhrp.md) | VRRPv2・v3 / HSRPv1・v2 / GLBP |
| [マルチキャスト](multicast.md) | IGMPv1・v2・v3 / MLDv1・v2 / PIM-SM v2 |
| [アプリケーション](application.md) | DHCPv4 / DHCPv6 / DNS / NTP / SNMP / Syslog / RADIUS / TFTP |

## 共通の約束事

- **Auto** と書いたフィールドは、テンプレートで値を書かなければ合成時に計算されます。値を書くと Manual になり、その値がそのまま使われます (不整合は warning)。
- **次プロトコル番号** (EtherType、IP プロトコル、ヘッダ内の `type` など) は次のレイヤから Auto で決まります。同じ番号を複数のプロトコルが共有する場合 (例: IPv4 プロトコル 112 = VRRPv3 / VRRPv2) は、解析時は先頭の候補で読んでから必要に応じて読み直し (re-parse) ます。
- **UDP / TCP** は宛先ポートで次のプロトコルを選び、未知なら送信元ポートで選び直します。サーバ発の応答 (DNS 応答など) も解析できます。合成時は宛先ポートだけを Auto にします。
- 各サンプルテンプレートは `crates/wpg-template/tests/samples.rs` で lint と render を、golden バイトと `tshark -V` 突合 (CI の optional job) で内容を検査しています。
- 確認には `wpg template show <file>` (Hex ダンプとツリー表示) と `wpg render <file> -o out.pcapng` が便利です。
