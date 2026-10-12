# プロトコルリファレンス

Status: draft

本章のフィールド一覧 (フィールド、型、自動計算値、制約) はプロトコル定義 IR から自動生成される予定で、常にコードと一致します。生成機能はマイルストーン M11 で実装予定です。プロトコルは P0 (MVP) から P3 (ユーザ定義) までの優先度で分類されます。

自動生成では表現できない内容は、手書きの補足ページにまとめます。補足ページには次を記載します。

- 下位プロトコルからの到達方法 (EtherType、IP プロトコル番号、ポート番号) と、レイヤの積み方
- 自動計算 (Auto) されるフィールドと、手動 (Manual) で設定が必要なフィールド
- 定義上の制約と近似 (opaque なバイト列として扱う領域など)
- 同梱サンプルテンプレート (`templates/protocols/`) と検証テスト

フィールドごとの説明は GUI のフィールドエディタにも表示されます (`locales/*/main.ftl`)。

## P1 プロトコル補足

- [L2 制御 (LLDP / CDP / STP / LACP / EAPOL / PPPoE / PPP)](protocols/l2-control.md)
- [カプセル化 (VXLAN / GRE / NVGRE / MPLS / GTP-U / Geneve / L2TP / IP-in-IP)](protocols/encap.md)
- [ルーティング (OSPFv2/v3 / BGP / RIP / RIPng / EIGRP)](protocols/routing.md)
- [冗長化 (VRRP / HSRP / GLBP)](protocols/fhrp.md)
- [マルチキャスト (IGMP / MLD / PIM)](protocols/mcast.md)
- [アプリケーション (DHCPv4/v6 / DNS / NTP / SNMP / Syslog / RADIUS / TFTP)](protocols/app.md)

## 共通事項

- **Auto と Manual**: Auto のフィールドは合成時に計算されます。値を指定すると Manual になり、その値がそのまま使われます。
- **次プロトコルの選択**: 下位レイヤの `type` / `protocol` / ポート番号などは、上に積んだレイヤから Auto で決まります。解析時の選択順序は仕様 `docs/spec/02-packet-builder.md` の「次プロトコルの選択」を参照してください。
- **opaque 領域**: 型付けされていない領域は `bytes` フィールドで、16 進文字列 (`"00 01 02"`) で指定します。解析時もバイト列のまま残ります。
- **検証**: 各グループには golden バイトテスト、build → parse → build の round-trip テスト、`WPG_TSHARK=1` で有効になる `tshark` 突合テストがあります (`crates/wpg-protocols/tests/p1_*.rs`)。サンプルテンプレートは `crates/wpg-template/tests/samples.rs` で lint と render が検査されます。
