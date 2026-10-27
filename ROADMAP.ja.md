# ロードマップ

[English](ROADMAP.md) | **日本語**

wpg 1.0 に向けたマイルストーンです。チェック済みの項目は `main` に取り込み済み、未チェックは進行中または未着手です。
詳細仕様は [`docs/spec/`](docs/spec/README.md)、設計上の決定事項は [`docs/adr/`](docs/adr/README.md)、
変更履歴は [CHANGELOG.md](CHANGELOG.md) にあります。マイルストーン番号 (M0〜M12) は CHANGELOG、開発者ガイド、
コード中のコメントから参照されます。

## 現状

M0〜M11 は完了しています。v1.0 のリリース判定 (M12) を進めており、一部の送出経路 (物理 NIC での AF_XDP zero-copy と
DPDK、実機の CML2) は仮想環境以外で未確認で、macOS での pcap バックエンド送出と最初のリリース公開が残っています。

## M0: プロジェクト基盤

- [x] Cargo workspace と crate 雛形 (`wpg-core`, `wpg-protocols`, `wpg-tx`, `wpg-template`, `wpg-server`, `wpg-cli`, `wpg-web`)
- [x] ツールチェーン、rustfmt、clippy、`cargo deny` の設定、MIT / Apache-2.0 デュアルライセンス
- [x] `justfile` と Linux / macOS / Windows 向けのビルド・起動スクリプト
- [x] GitHub Actions: 3 OS での fmt / clippy / test、`cargo audit`
- [x] README (ja / en)、CONTRIBUTING、行動規範
- [x] mdBook 雛形 (ja / en)、仕様の `docs/spec/` への分割
- [x] i18n 基盤 (Fluent) をサーバと Web UI に導入
- [x] 決定事項を ADR 0001〜0009 として記録

## M1: コアエンジン MVP

- [x] フィールド型システムとプロトコル定義 IR (`wpg-core`)
- [x] Rust の定義マクロと YAML ローダ (どちらも同一 IR へ)
- [x] 合成器: 長さ・チェックサム・次ヘッダの自動計算、フィールドごとの Auto / Manual
- [x] 解析器: バイト列 → レイヤツリー (import 用)
- [x] 制約検査 (warning / error)
- [x] P0 プロトコル: Ethernet II / 802.3 + LLC / SNAP / 802.1Q / QinQ / ARP / IPv4 (+options) / IPv6 (+拡張ヘッダ) / ICMPv4 / ICMPv6 + NDP / TCP (+options) / UDP / ペイロード各種
- [x] ジェネレータ: range / list / random / counter / file / expr、zip / product / nested
- [x] ストリーム展開 (事前展開とオンザフライの自動選択)
- [x] golden pcap テストと `tshark -V` 突合 (CI の任意ジョブ)
- [x] `proptest` による round-trip (build → parse → build) テスト
- [x] `wpg-core` の `wasm32-unknown-unknown` ビルドを CI で保証

## M2: 送出エンジン

- [x] `TxBackend` trait、`pcapfile` / `null` バックエンド
- [x] `pcap` バックエンド (Linux libpcap / macOS BPF / Windows Npcap)
- [x] Linux `afpacket` (TPACKET_V3 + `sendmmsg`)、コア固定、複数キュー
- [x] レート制御 (pps / bps / %)、バースト、IFG、精度計測
- [x] バックエンド別・ストリーム別の統計と 100 ms スナップショット
- [x] 3 OS でのインタフェース列挙
- [x] `wpg doctor` (権限・Npcap・setcap・hugepages の診断)
- [x] `wpg bench` と性能ベースライン (`docs/perf.md`)
- [x] CML2 (virtio-net の Ubuntu) の注意点を運用ガイドに記載 (CML2 実機での実測は未了)

## M3: サーバと CLI

- [x] axum サーバ、`rust-embed` によるフロントエンド資産の埋め込み
- [x] REST API: テンプレート、プロトコルカタログ、render / preview、ジョブ、インタフェース、統計、認証
- [x] WebSocket によるジョブ進捗・統計・ログ配信
- [x] OpenAPI (`utoipa`) と Swagger UI
- [x] 認証: argon2id パスワード、セッション、レート制限、未設定時は localhost 限定
- [x] TLS (`rustls`、自己署名証明書の自動生成)
- [x] 設定 (`wpg.toml` + 環境変数 + CLI)、`wpg config init`
- [x] ジョブマネージャ (ストリーム混合、スケジュール、安全なキャンセル)
- [x] 送出制限ポリシー (許可インタフェース、最大レート)
- [x] CLI サブコマンド: serve / send / render / import / template / ifaces / doctor / bench
- [x] `tracing` ログ (JSON、ローテーション)

## M4: GUI MVP

- [x] Leptos プロジェクト、Tailwind、ルーティング、i18n、テーマ
- [x] ログイン画面とセッション処理
- [x] ビルダー: プロトコルパレット、レイヤスタックの D&D、型別フィールドエディタ、Auto / Manual
- [x] Hex ダンプとツリーの双方向ハイライト、Hex 直接編集
- [x] ブラウザ内 WASM プレビュー (`wpg-core`) とサーバ結果の突合
- [x] ジェネレータ設定と展開件数・所要時間の見積
- [x] ジョブ画面: ストリーム混合、レート設定、開始 / 停止、リアルタイム統計グラフ
- [x] テンプレート画面: 一覧、検索、タグ、import / export、複製
- [x] 設定画面
- [x] Undo / Redo、ショートカット、下書きの自動保存
- [x] キーボードのみでの D&D 相当操作、ARIA
- [x] E2E テスト (Playwright、Chromium)

## M5: テンプレート形式と共有

- [x] YAML / JSON スキーマ (`schema: 1`)、JSON Schema の公開、`wpg template lint`
- [x] テンプレートの変数と実行時オーバーライド
- [x] `.wpgbundle` のエクスポート / インポート
- [x] ライブラリパスの複数指定 (Git 管理ディレクトリ)、ホットリロード
- [x] 同梱サンプルテンプレート (P0 プロトコルと代表シナリオ)
- [x] pcap / pcapng import、Hex 貼り付け import

## M6: P1 プロトコル群

- [x] LLDP / CDP / STP, RSTP, MSTP / LACP / EAPOL / PPPoE / PPP
- [x] VXLAN / GRE / NVGRE / MPLS / MPLS-in-UDP / L2TP / GTP-U / Geneve / IP-in-IP
- [x] OSPFv2 / v3 / BGP / RIP / RIPng / EIGRP
- [x] VRRP / HSRP / GLBP
- [x] IGMP / MLD / PIM
- [x] DHCPv4 / v6 / DNS / NTP / SNMP / Syslog / RADIUS / TFTP
- [x] 各プロトコルのサンプルテンプレート、golden pcap、ドキュメント
- [x] 初期の近似を解消する IR 拡張 (peek による demux、リスト件数、NUL 終端文字列、チェックサム範囲の除外、送信元ポートでの demux)

## M7: P2 OSI / 専門領域

- [x] 802.2 LLC (0xFE) 上の OSI カプセル化、NSAP 型とエディタ
- [x] IS-IS: IIH (LAN L1 / L2、P2P)、LSP、CSNP、PSNP、主要 TLV / sub-TLV、Fletcher チェックサム、Padding
- [x] CLNP / ES-IS
- [x] SCTP (CRC-32C)
- [x] OSPF Opaque LSA と SR、BGP MP / EVPN / VPNv4 NLRI
- [x] LDP / RSVP-TE / BFD
- [x] SRv6 SRH と SR-MPLS の補助
- [x] PTP / ESMC / ERPS / Y.1731 CFM / 802.3ah OAM
- [x] MACsec (構造のみ) / GTP-C / Diameter / SIP / RTP
- [x] FRR との相互接続シナリオ (`templates/interop/`、`scripts/interop/`)

## M8: 高速バックエンド

- [x] AF_XDP バックエンド (`--features afxdp`、copy / zero-copy 自動、カーネル 5.10+)
- [x] `dpdk-sys` crate (bindgen、DPDK 24.11 / 23.11 LTS)
- [x] DPDK バックエンド (`--features dpdk`)、EAL 設定、hugepages / `dpdk-devbind` 手順、virtio PMD
- [x] マルチキュー・マルチコア送出、NUMA 配慮
- [x] レート制御精度 (busy-poll、TSC 時計)
- [x] 性能レグレッションテスト (`wpg bench` を CI nightly で実行)

## M9: 受信 / 検証

- [x] 受信カウンタ (pps / bps、フィルタ一致)、tcpdump 風フィルタ
- [x] 送出シグネチャとロス / 順序 / 遅延の照合
- [x] 先頭 N パケットの表示、pcapng 保存
- [x] GUI の受信画面

## M10: デプロイ・配布・運用

- [x] `scripts/deploy.sh` (scp + setcap + systemd + ヘルスチェック)、`--dpdk` オプション
- [x] `scripts/release.sh` と 3 OS 向け GitHub Release (CI でビルドとスモークテスト済み。最初の公開は M12 で行う)
- [x] Dockerfile / compose、systemd unit、launchd plist、Windows サービス手順 (CI で確認済み)
- [x] Windows: Npcap の検出とインストール案内、`build.ps1` の動作確認
- [x] macOS: Gatekeeper / quarantine の回避手順を記載 (署名・公証は見送り)
- [x] アップグレード手順とテンプレートの移行 (`wpg template migrate`)

## M11: ドキュメント

- [x] 3 OS の Getting Started、チュートリアル 5 本 (ja / en)
- [x] プロトコルリファレンスの自動生成 (定義 IR → Markdown)
- [x] API / CLI / 設定 / テンプレートスキーマのリファレンス自動生成
- [x] DPDK / AF_XDP / CML2 のセットアップガイド、性能チューニング
- [x] 開発者ガイド (プロトコル追加、言語追加、リリース)
- [x] FAQ、トラブルシューティング、法的注意
- [x] 実際の UI から生成したスクリーンショットとビルダーの GIF

## M12: v1.0 リリース判定

- [x] P0 + P1 プロトコルの golden pcap と tshark 突合がすべて緑
- [ ] 3 OS でビルド・起動・pcap バックエンド送出を確認 (Windows は確認済み。macOS と Linux の pcap バックエンドが残り)
- [ ] Linux `afpacket` で 1 Mpps/core 以上 (参考値)。DPDK は環境依存のため参考記録
- [x] GUI の E2E が緑、i18n の抜け漏れ 0 (lint)
- [x] `cargo audit` / `cargo deny` クリーン
- [x] ドキュメントが ja / en そろっている、CHANGELOG

## 1.0 の計画外で追加した機能

- [x] SNMP マネージャ (Get / Walk / Set / Trap、v1 / v2c / v3、MIB アップロード、pcap 保存。ADR-0010)
- [x] Web ターミナル (SSH / Telnet、踏み台経由 (ProxyJump)。ADR-0011)
- [x] 対話式デプロイウィザードと無人実行 (Windows の `deploy.bat` / `deploy.ps1`、`deploy.sh`。回答ファイル、複数ホスト、定期実行)

## 1.0 以降

候補であり確約ではありません: ステートフルなセッション (TCP ハンドシェイク、HTTP クライアント)、802.11、
ユーザー定義プロトコルによる P3 プロトコルの拡充、署名付き macOS ビルド。
