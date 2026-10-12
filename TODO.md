# web-packet-generator — 仕様検討 & TODO

> ネットワーク検証向け、Web GUI で動く高性能パケットジェネレータ（Rust 製）。
> 本ファイルは「仕様の骨子」と「実装 TODO」を 1 枚にまとめた開発の起点ドキュメント。
> 詳細仕様は実装が進んだ段階で `docs/spec/` に分割していく。

---

## 0. コンセプト / ゴール

| 項目 | 内容 |
|---|---|
| 目的 | NW 機器・NW 構成の検証用に、任意のパケットを GUI で組み立て、超高速に送出できるツール |
| 対象ユーザー | NW エンジニア、検証担当、CML2 / EVE-NG 等のラボ利用者、プロトコル実装者 |
| 差別化 | (1) IS-IS / CLNP など OSI 系まで含む「深い」プロトコル対応 (2) DPDK / AF_XDP まで使う高速送出 (3) GUI + テンプレート共有で「誰でも再現できる検証」 |
| 言語 | Rust 一択（バックエンド、パケットエンジン、フロントエンド WASM まで） |
| 配布形態 | 単一バイナリ（フロントエンド資産を埋め込み）＋ スクリプト群。Windows / macOS / Linux |
| 多言語 | 日本語・英語（UI / マニュアル / エラーメッセージ）。以降追加可能な構造 |

### 非ゴール（v1 では扱わない）
- フル機能のパケットキャプチャ/解析（Wireshark の代替はしない。受信は「検証カウンタ・簡易表示」まで）
- ステートフルなセッション生成（TCP 3-way handshake 完走、HTTP クライアントなど）→ v2 以降で検討
- 攻撃ツール用途への最適化（検証用途に限定。ドキュメントで利用者責任を明記）

---

## 1. 全体アーキテクチャ

```
┌──────────────────────────────────────────────────────────────┐
│ Browser (WASM: Leptos)                                       │
│  ・パケットビルダー GUI（レイヤ D&D、フィールド編集、Hex/ツリー同期表示）│
│  ・テンプレート管理、ジョブ制御、統計ダッシュボード、i18n           │
└───────────────▲──────────────────────────────▲───────────────┘
                │ REST (JSON)                  │ WebSocket (統計/ログ/進捗)
┌───────────────┴──────────────────────────────┴───────────────┐
│ wpg-server (axum + tokio)  … 単一バイナリ、静的資産埋め込み        │
│  ・認証 (password / session)  ・テンプレート CRUD  ・ジョブ管理      │
│  ・API スキーマ (OpenAPI 自動生成)                                 │
├──────────────────────────────────────────────────────────────┤
│ wpg-core (pure Rust, no_std 寄り)                                │
│  ・プロトコル定義エンジン（フィールド型、長さ/チェックサム自動計算）    │
│  ・パケット合成 (build) / 解析 (parse: pcap→テンプレ逆生成)        │
│  ・変数/インクリメント/ランダム生成、ストリーム展開                  │
├──────────────────────────────────────────────────────────────┤
│ wpg-tx (送出エンジン: TxBackend trait)                           │
│  ・pcap/Npcap (全 OS 共通の最小公倍数)                            │
│  ・AF_PACKET TPACKET_V3 / sendmmsg (Linux 標準)                  │
│  ・AF_XDP (Linux, ゼロコピー)                                    │
│  ・DPDK (Linux, feature フラグ、EAL 初期化)                       │
│  ・pcap ファイル出力 (ドライラン / 共有)                           │
└──────────────────────────────────────────────────────────────┘
```

### 1.1 crate 構成（Cargo workspace）

```
web-packet-generator/
├── Cargo.toml                 # workspace
├── crates/
│   ├── wpg-core/              # プロトコル定義・合成・解析・変数展開（OS 非依存、WASM でもビルド可）
│   ├── wpg-protocols/         # 各プロトコル定義（Ethernet, IPv4, ..., IS-IS, CLNP ...）
│   ├── wpg-tx/                # 送出バックエンド抽象 + 実装（feature: pcap, afpacket, afxdp, dpdk）
│   ├── wpg-rx/                # 受信・カウンタ・簡易検証（v1 は最小）
│   ├── wpg-template/          # テンプレートファイル形式 (YAML/JSON)、スキーマ、import/export、バンドル
│   ├── wpg-server/            # axum HTTP/WS サーバ、認証、ジョブ管理、静的資産埋め込み
│   ├── wpg-cli/               # CLI（サーバ起動 / ヘッドレス送出 / テンプレ検証 / pcap 変換）
│   └── wpg-web/               # Leptos (CSR) フロントエンド → WASM
├── locales/                   # Fluent (.ftl) — ja / en
├── scripts/                   # build / run / deploy / release スクリプト
├── docs/                      # mdBook（ja / en）
├── templates/                 # 同梱テンプレート（プロトコル別サンプル）
└── tests/                     # 統合テスト、golden pcap
```

### 1.2 技術選定

| 領域 | 選定 | 理由 / 備考 |
|---|---|---|
| Web サーバ | `axum` + `tokio` + `tower-http` | エコシステム最大、WS 対応、ミドルウェア豊富 |
| API 定義 | `utoipa` (OpenAPI) | フロント/CLI/外部連携で型共有。Swagger UI 同梱 |
| フロント | `leptos` (CSR) + Tailwind CSS | Rust 一貫、WASM。※ TS フレームワーク不採用は「Rust 製」要件優先のため (§3 決定) |
| D&D | HTML5 DnD API を Leptos で薄くラップ（自作） | 依存を減らす。必要なら `web-sys` 直叩き |
| Hex 表示 | 自作コンポーネント（仮想スクロール） | 1500〜9000B の同期ハイライトが必要 |
| 状態共有 | `wpg-core` を WASM でもビルド → ブラウザ側でプレビュー合成 | サーバ往復なしで即時プレビュー |
| シリアライズ | `serde` + `serde_yaml` / `serde_json` | テンプレ YAML、API JSON |
| i18n | `fluent` (`fluent-bundle`, `rust-i18n` 検討) | 複数形/性別対応、ja/en の .ftl |
| 認証 | `argon2` + `tower-sessions`（Cookie） | パスワード 1 つ〜複数ユーザ。TLS は `rustls` オプション |
| 静的資産埋め込み | `rust-embed` | 単一バイナリ配布 |
| 送出 (共通) | `pcap` crate (libpcap / Npcap / BPF) | Windows・macOS の主経路 |
| 送出 (Linux 高速) | `libc` 直叩き AF_PACKET TPACKET_V3 + `sendmmsg`、AF_XDP は `xsk-rs` | 依存を最小化しつつ Mpps 級 |
| 送出 (最高速) | DPDK 24.11 LTS (自前 `dpdk-sys` bindgen、feature = "dpdk") | ビルド任意。EAL 引数を設定ファイルから |
| ログ | `tracing` + `tracing-subscriber` | JSON ログ/ローテーション |
| 設定 | `figment` or `config` (TOML + env + CLI) | `wpg.toml` |
| DB | テンプレ = YAML ファイル、履歴/ユーザ/セッション = SQLite (`rusqlite` bundled) | ハイブリッド (§3 決定) |
| テスト | `cargo test`, `proptest`, golden pcap 比較, `tshark -V` による外部検証 (CI optional) | プロトコル正しさの担保 |
| リリース | `cargo-dist` + GitHub Actions matrix | 3 OS バイナリ + checksums |

---

## 2. 機能仕様

### 2.1 パケットビルダー（コア）

#### プロトコル定義エンジン
- すべてのプロトコルを「宣言的なフィールド定義」で表現し、合成 / 解析 / GUI 生成 / ドキュメント生成を 1 ソースから行う。
- フィールド型: `uint(bits)`, `int`, `bytes(len|var)`, `mac`, `ipv4`, `ipv6`, `nsap`（可変長 OSI アドレス）, `enum`, `flags(bit set)`, `tlv`, `list<T>`, `padding`, `raw`
- 自動計算フィールド（GUI で「Auto / Manual」トグル）:
  - 長さ系 (IPv4 total length, UDP length, IS-IS PDU length, TLV length …)
  - チェックサム系 (IPv4 hdr, TCP/UDP + pseudo header (v4/v6), ICMP, OSPF, IS-IS (Fletcher), SCTP CRC32c, GRE …)
  - 次ヘッダ / EtherType / Protocol 番号（下位レイヤの選択から自動決定、上書き可）
  - シーケンス番号など「ストリーム内カウンタ」
- 意図的に壊す (malformed) をサポート: 任意フィールドの上書き、長さ不整合、ゴミ TLV、末尾切詰め、FCS 誤り (対応バックエンドのみ)
- 制約検査: 警告レベル (「この組合せは RFC 違反」) と エラーレベル (合成不能) を区別して GUI に表示

#### 対応プロトコル（優先度順）

**P0（MVP）**
- L2: Ethernet II, 802.3 + LLC/SNAP, 802.1Q, 802.1ad (QinQ), ARP
- L3: IPv4 (options 含む), IPv6 (拡張ヘッダ: HbH, Routing, Fragment, DstOpts), ICMPv4, ICMPv6 (NDP: RS/RA/NS/NA/Redirect)
- L4: TCP (options: MSS, WS, SACK, TS), UDP
- Payload: 固定 / パターン / ランダム / ファイル / インクリメントカウンタ埋め込み

**P1（NW 検証で頻出）**
- L2/制御: LLDP, CDP, STP/RSTP/MSTP (BPDU), LACP, 802.1X EAPOL, PPPoE (Discovery/Session), PPP (LCP/IPCP)
- カプセル化: VXLAN, GRE, NVGRE, MPLS (複数ラベル), MPLS over UDP, L2TPv2/v3, GTP-U, Geneve, IP-in-IP
- ルーティング: OSPFv2/v3 (Hello/DBD/LSR/LSU/LSAck, 主要 LSA), BGP (OPEN/UPDATE/KEEPALIVE/NOTIFICATION, 主要 attr/capability), RIPv1/v2, RIPng, EIGRP (基本)
- 冗長: VRRPv2/v3, HSRPv1/v2, GLBP (基本)
- マルチキャスト: IGMPv1/2/3, MLDv1/v2, PIM (Hello/Join-Prune/Register)
- アプリ: DHCPv4/DHCPv6 (Relay 含む), DNS (query/response), NTP, SNMP (v1/v2c GetRequest/Trap), Syslog, RADIUS, TFTP
- QoS/検証: DSCP/ECN/PCP 全パターン、ジャンボフレーム、パディング

**P2（OSI / マニアック領域 — 本プロジェクトの目玉）**
- IS-IS (ISO 10589 / RFC 1195 / RFC 5305 …): Hello (L1/L2 LAN IIH, P2P IIH), LSP, CSNP, PSNP。TLV: Area Address(1), IS Neighbors(2/6), Auth(10), Ext IS Reach(22), IP Int Reach(128/130), Prot Supported(129), IP Int Addr(132), Ext IP Reach(135), Hostname(137), IPv6 Reach(236), MT (229/222/235), SR (Router Cap 242, Prefix-SID/Adj-SID sub-TLV), Padding(8)
- CLNP (ISO 8473), ES-IS (ISO 9542: ESH/ISH/RD), NSAP アドレスエディタ（AFI/IDI/HO-DSP/SystemID/SEL 分解表示）
- 802.2 LLC (DSAP/SSAP 0xFE) 上の OSI カプセル化、SNAP
- SCTP (INIT/INIT-ACK/DATA/HEARTBEAT, CRC32c)
- OSPF 拡張 (Opaque LSA / SR)、BGP 拡張 (MP-BGP / EVPN / VPNv4 NLRI)
- MPLS 制御: LDP (Hello/Init/Address/Label Mapping), RSVP-TE (Path/Resv 基本)
- BFD (制御パケット), Micro-BFD
- Segment Routing: SRv6 (SRH), MPLS-SR ラベルスタック生成補助
- 802.11 (Radiotap + 管理フレーム、送出はモニタモード対応 NIC 限定) — P3 相当、要望次第
- PTP (IEEE 1588 v2), SyncE ESMC, ERPS (G.8032 R-APS), Y.1731 CFM/OAM (CCM/LBM/LTM), EtherOAM (802.3ah)
- MACsec (フレーム構造のみ、暗号化は固定鍵)
- GTP-C, Diameter, SIP (基本), RTP

**P3 / 拡張機構**
- ユーザ定義プロトコル: YAML でフィールド定義を書いてドロップインできる `protocols.d/` 機構
- Lua or WASM プラグインでカスタム計算フィールド（要検討）

#### 変数・インクリメント・大量生成
- どのフィールドにも「ジェネレータ」を割当可能:
  - `range(start, end, step)` 例: `192.168.1.1 – 192.168.5.254`（IPv4/IPv6/MAC/整数/NSAP SystemID に対応、CIDR 指定も可）
  - `list([...])`、`random(min,max|bits)`、`random_mac(oui 固定)`、`file(lines)`、`counter(start, step, wrap)`
  - `expr` 簡易式（他フィールド参照、算術、`hash()`、`crc()`）
- 複数ジェネレータの合成モード: `zip`（並行）/ `product`（直積）/ `nested`（優先順位付き）
- ストリーム: 1 テンプレ = 1 ストリーム。ジョブは複数ストリームを重み付き・順次・ラウンドロビンで混合
- 送出制御: パケット数 / 時間 / 無限、レート (pps / bps / 回線率 %)、バースト (サイズ・間隔)、IFG 指定、ランプアップ、スケジュール開始
- 事前展開 (pre-render) と オンザフライ生成の自動選択（メモリ量とレートで判定。ヒント指定可）
- 上限目安: 事前展開 100M パケット / オンザフライは無制限

#### 逆生成 (import)
- pcap/pcapng → テンプレート化（各パケットを解析してレイヤ/フィールドを復元、差分からジェネレータ推定はベストエフォート）
- Hex / Wireshark 「Copy as Hex Stream」貼付け → テンプレート化
- Scapy 風テキスト (`Ether()/IP()/UDP()`) の簡易パーサ（P2）

### 2.2 送出エンジン (wpg-tx)

| バックエンド | OS | 期待性能 | 備考 |
|---|---|---|---|
| `pcap` (libpcap / Npcap / BPF) | Win / mac / Linux | 〜数十万 pps | 全 OS 共通のフォールバック。Npcap は WinPcap 互換モード要件を明記 |
| `afpacket` (TPACKET_V3 mmap ring + `sendmmsg`) | Linux | 〜1–3 Mpps/core | CML2 上 Ubuntu (virtio) の主経路。CAP_NET_RAW |
| `afxdp` | Linux 5.10+ | 〜数 Mpps/core | ドライバ対応要。zero-copy / copy モード自動 |
| `dpdk` | Linux | 10 Mpps+ /core | `--features dpdk`、DPDK 24.11 LTS (23.11 も可)。EAL 引数・hugepages・NIC バインド手順をドキュメント化。CML2 では virtio-net PMD |
| `pcapfile` | 全 | — | 送出せず pcapng に書き出す（ドライラン、共有、CI テスト） |
| `loopback/null` | 全 | — | ベンチ・テスト用 |

- 共通 trait: `TxBackend { open(iface, opts) ; send_batch(&[Frame]) ; stats() ; close() }`
- 送信スレッド: コア pin、バッチサイズ・リング深さ設定、複数キュー (RSS/TX queue) 対応
- レート制御: トークンバケット + busy-poll / `nanosleep` ハイブリッド、精度指標を UI に表示
- 統計: 送出 pps/bps、累計、ドロップ、レート達成率、per-stream カウンタ (WS で 100ms 間隔配信)
- 送出インタフェース列挙: OS 別 API (`getifaddrs` / `GetAdaptersAddresses` / Npcap) + MAC/IP/MTU/リンク状態表示
- 権限: Linux は `setcap cap_net_raw,cap_net_admin+ep` 推奨、Windows は管理者 + Npcap、mac は `/dev/bpf*` 権限。起動時に自己診断して GUI に案内

### 2.3 受信 / 検証 (wpg-rx) — v1 は最小
- 送出と同一/別インタフェースの受信カウンタ (pps/bps, フィルタ一致数)
- 簡易 BPF フィルタでの一致カウント、最初の N パケットの Hex/ツリー表示
- 送出パケットに埋め込んだシグネチャ (magic + seq) を照合してロス・順序逆転・遅延の概算 (P1)
- 本格的なキャプチャ保存は pcapng 出力までに限定

### 2.4 テンプレート (wpg-template)

- 形式: YAML（人が読める、Git で diff できる）。JSON も同等に読み書き可
- スキーマバージョニング (`schema: 1`)、後方互換のマイグレーション
- 構造: メタ (名前/説明/タグ/作者/作成日/多言語説明) + レイヤ配列 (プロトコル + フィールド + ジェネレータ) + 送出プロファイル (レート等) + 変数定義 (テンプレ引数化: 「送信元 IP を実行時に指定」)
- 管理機能: フォルダ/タグ、検索、複製、バージョン履歴 (簡易)、お気に入り、最近使った
- 共有:
  - 単体エクスポート (`.wpg.yaml`)、複数まとめて `.wpgbundle` (zip: templates + README + 添付 pcap)
  - URL 共有 (サーバ内リンク)、Git リポジトリ連携 (テンプレディレクトリを任意パスに置くだけで読み込む「ライブラリパス」複数指定)
  - 同梱ライブラリ: プロトコル別サンプル、代表的検証シナリオ (例: 「VLAN 越し ARP スイープ」「IS-IS L2 IIH フラッド」「DHCP Discover 大量」)
- 検証コマンド: `wpg template lint <file>`（スキーマ・制約チェック）

### 2.5 GUI (wpg-web)

- 画面構成
  1. **ダッシュボード**: 実行中ジョブ、インタフェース状態、直近の統計グラフ
  2. **ビルダー**: 左=プロトコルパレット (検索/カテゴリ/お気に入り) → 中央=レイヤスタック (D&D で追加・並替・削除) → 右=フィールドエディタ (型に応じた入力 UI、Auto/Manual、ジェネレータ設定) → 下=Hex ダンプ + ツリー (双方向ハイライト、クリックでフィールドへジャンプ、Hex 直接編集で逆反映)
  3. **ストリーム/ジョブ**: 複数テンプレを混合、レート・回数・インタフェース設定、ドライラン (pcap 出力)、開始/停止/一時停止
  4. **テンプレート**: 一覧・検索・タグ・インポート/エクスポート・共有
  5. **受信/検証**: カウンタ・簡易表示
  6. **設定**: 言語、テーマ (ライト/ダーク)、認証、バックエンド設定、ライブラリパス
- 操作性
  - 全操作にキーボードショートカット、Undo/Redo、自動保存 (ローカル draft)
  - フィールド入力は「人間向け表記」(例 `192.168.1.1-192.168.5.254`, `00:11:22:*`, `49.0001.1921.6800.1001.00`) を受け付け、パースエラーは即時表示
  - プレビュー: ブラウザ内 WASM で即時合成 (サーバ往復なし)、サーバ側合成結果と一致を保証 (同一 crate)
  - レイヤ追加時の「次ヘッダ自動設定」と、逆に「上位レイヤに合わせて下位を提案」
  - モバイル/狭幅は最低限の閲覧のみ (編集はデスクトップ想定)
- i18n: 全文字列 Fluent 化、ブラウザ言語自動判定 + 手動切替、プロトコル名/フィールド説明も多言語
- アクセシビリティ: aria 対応、コントラスト、キーボードのみで D&D 相当操作可能

### 2.6 認証 / セキュリティ
- 起動時に `--password` / 環境変数 / 設定ファイル / 初回アクセス時セットアップ画面のいずれかで管理パスワード設定（未設定なら localhost のみ bind して警告）
- ハッシュは argon2id、セッションは Cookie (HttpOnly, SameSite=Strict)、ログイン失敗レート制限
- 複数ユーザ + 役割 (admin / operator / viewer) は P1
- TLS: 自己署名自動生成 or 証明書指定 (`rustls`)。リバースプロキシ配置もドキュメント化
- API トークン (CLI・自動化用) は P1
- 送出は危険操作なので「送出先インタフェース許可リスト」「最大レート上限」を管理者設定で制限可能
- CSRF 対策、CSP ヘッダ、依存の `cargo audit` / `cargo deny` を CI で実行

### 2.7 CLI (wpg-cli)
```
wpg serve   [--bind 0.0.0.0:8080] [--password-file] [--tls] [--config wpg.toml]
wpg send    <template.yaml> -i eth0 [--backend afpacket] [--count N | --duration 10s] [--rate 1Mpps] [--var src_ip=...]
wpg render  <template.yaml> -o out.pcapng [--count N]      # 送出なし
wpg import  <in.pcap> -o template.yaml
wpg template lint|list|export|import
wpg ifaces  [--json]
wpg doctor                                                 # 権限・ドライバ・hugepages・Npcap 診断
wpg bench   [--backend ...]                                # 送出性能計測
```
- サーバと同一バイナリ (サブコマンド)。CI や ssh 先ヘッドレス実行に利用

### 2.8 スクリプト / 配布 / 運用
- `scripts/build.sh` / `build.ps1`: 依存チェック (rustup, wasm target, trunk, libpcap-dev / Npcap SDK)、フロント → バックエンドの順でビルド、`--release`、`--features dpdk`
- `scripts/run.sh` / `run.ps1`: 開発起動 (ホットリロード: `trunk serve` + `cargo watch`) / 本番起動
- `scripts/deploy.sh <user@host> [--iface eth1] [--service]`: ビルド済みバイナリを scp、`setcap`、systemd unit 投入・起動、ヘルスチェック。CML2 上 Ubuntu への投入を第一ターゲット
- `scripts/release.sh`: cargo-dist による 3 OS クロスビルド、チェックサム、GitHub Release
- `justfile` (or `Makefile`) で上記の統一エントリ: `just build`, `just run`, `just deploy host=...`, `just test`, `just docs`
- Docker: `Dockerfile` (multi-stage) + `docker-compose.yml`（`--network host` + `NET_RAW`）
- systemd unit / Windows サービス (`sc create` 手順) / macOS launchd plist のサンプル
- 設定ファイル `wpg.toml` のフル例と `wpg config init`

### 2.9 ドキュメント (docs/, mdBook, ja/en)
- Getting Started (OS 別インストール、権限設定、初回ログイン)
- チュートリアル (5 本程度): ARP スイープ、VLAN タグ付き TCP SYN 大量、IS-IS IIH 生成、DHCP 負荷、pcap → テンプレ再生
- リファレンス: プロトコル一覧 (定義から自動生成)、テンプレスキーマ、API (OpenAPI)、CLI、設定
- 運用: DPDK / AF_XDP セットアップ、CML2 での注意点 (virtio, promiscuous, MAC 制限)、性能チューニング
- 開発者ガイド: crate 構成、プロトコル追加手順、i18n 追加手順、リリース手順
- FAQ / トラブルシュート (`wpg doctor` 出力との対応表)
- 法的注意: 検証環境以外での使用禁止の明記、ライセンス (MIT / Apache-2.0 デュアル)

---

## 3. 決定事項（2026-09-29 確定）

| 項目 | 決定 | 補足 |
|---|---|---|
| ライセンス | **MIT / Apache-2.0 デュアル** | Rust エコシステム標準。DPDK (BSD-3) / libpcap (BSD) / Npcap (利用規約: 再配布せず利用者がインストール) と競合なし。`LICENSE-MIT` / `LICENSE-APACHE` を同梱 |
| フロントエンド | **Leptos (CSR) + Tailwind CSS** | WASM。将来 SSR/ハイドレーションへ移行可能な構成を維持 |
| DB | **ハイブリッド**: テンプレはファイル (YAML)、履歴 / ユーザ / セッションは SQLite | `rusqlite` (bundled) で外部依存なし。テンプレは Git 管理しやすいディレクトリ配置 |
| DPDK バインディング | **自前 bindgen (`dpdk-sys` crate を workspace 内に作成)** | 対象 DPDK **24.11 LTS** を第一、**23.11 LTS** も CI でビルド確認。`--features dpdk` でのみ有効 |
| AF_XDP | **`xsk-rs` を採用、不足分は libc 直叩きで補完** | カーネル最低 **5.10** (Ubuntu 20.04 HWE / 22.04 以降)。zero-copy 非対応 NIC は copy モードへ自動フォールバック |
| Windows 送出 | **Npcap のみ** | Windows は「GUI + 低〜中速送出」の位置づけ。WinPF / XDP for Windows は v1 スコープ外 |
| プロトコル定義方式 | **ハイブリッド**: コア = Rust マクロ、ユーザ拡張 = YAML、どちらも同一 IR へ | IR から合成 / 解析 / GUI / ドキュメントを生成 |
| 名称 | **wpg** (`web-packet-generator`)、バイナリ名 `wpg` | 設定ファイル `wpg.toml`、テンプレ拡張子 `.wpg.yaml`、バンドル `.wpgbundle` |
| 最低対応 OS | **Windows 10 21H2+ / macOS 12+ / Ubuntu 20.04+ (glibc 2.31)** | Linux は他ディストリも glibc 2.31 以上なら動作想定。musl 静的ビルドは P2 で検討 |

> 変更が必要になった場合は `docs/adr/` に ADR を追加して上書きする（M0 で ADR 化）。

---

## 4. マイルストーン & TODO

### M0: プロジェクト基盤（1–2 週）
- [x] Cargo workspace 作成、crate 雛形 (`wpg-core`, `wpg-protocols`, `wpg-tx`, `wpg-template`, `wpg-server`, `wpg-cli`, `wpg-web`)
- [x] `Cargo.toml` の `license = "MIT OR Apache-2.0"`、`rust-toolchain.toml`、`rustfmt.toml`、`clippy` 設定、`deny.toml`
- [x] `justfile` + `scripts/build.sh|ps1`、`scripts/run.sh|ps1` の骨格
- [x] GitHub Actions: fmt / clippy / test (Linux, macOS, Windows matrix)、`cargo audit`
- [x] ライセンス (`LICENSE-MIT` / `LICENSE-APACHE`)
- [x] `README.md`（ja/en）・`CONTRIBUTING.md`・`CODE_OF_CONDUCT.md`
- [x] mdBook 雛形 (ja/en 切替)、`docs/spec/` に本ファイルの詳細版を分割開始
- [x] i18n 基盤 (Fluent) を server / web 両方に導入、`locales/ja`, `locales/en`
- [x] 決定事項 (§3) を `docs/adr/0001`〜`0009` として ADR 化

### M1: コアエンジン MVP（3–4 週）
- [x] フィールド型システム & プロトコル定義 IR (`wpg-core`)
- [x] 定義マクロ (Rust) と YAML ローダ → 同一 IR
- [x] 合成器 (build): 長さ・チェックサム・次ヘッダ自動計算、Auto/Manual
- [x] 解析器 (parse): バイト列 → レイヤツリー (import 用)
- [x] 制約検査 (warning/error)
- [x] P0 プロトコル実装: Ethernet II / 802.3+LLC/SNAP / 802.1Q / QinQ / ARP / IPv4(+options) / IPv6(+ext hdr) / ICMPv4 / ICMPv6+NDP / TCP(+options) / UDP / Payload 各種
- [x] ジェネレータ: range / list / random / counter / file / expr、zip / product / nested
- [x] ストリーム展開器 (pre-render / on-the-fly 自動選択)
- [x] golden pcap テスト + `tshark -V` 突合テスト (CI optional job)
- [x] `proptest` による round-trip (build→parse→build) テスト
- [x] `wpg-core` を `wasm32-unknown-unknown` でビルドできることを CI で保証

### M2: 送出エンジン（2–3 週）
- [x] `TxBackend` trait、`pcapfile` / `null` バックエンド
- [x] `pcap` バックエンド (Linux libpcap / macOS BPF / Windows Npcap)
- [x] Linux `afpacket` (TPACKET_V3 + sendmmsg)、コア pin、複数キュー
- [x] レート制御 (pps/bps/%)、バースト、IFG、精度計測
- [x] 統計収集 (per-backend / per-stream) と 100ms スナップショット
- [x] インタフェース列挙 (3 OS)
- [x] `wpg doctor`（権限・Npcap・setcap・hugepages 診断）
- [x] `wpg bench` と性能ベースライン記録 (`docs/perf.md`)
- [x] CML2 上 Ubuntu (virtio-net) での実測・注意点ドキュメント化（注意点は docs/operations/cml2 に記載済み。実測は `lo` のみで CML2 実機は未計測）

### M3: サーバ & CLI（2–3 週）
- [x] axum サーバ骨格、`rust-embed` で静的資産埋め込み
- [x] REST API: templates CRUD / protocols catalog / render(preview) / jobs (create/start/stop/pause) / ifaces / stats / auth
- [x] WebSocket: ジョブ進捗・統計・ログ配信
- [x] `utoipa` OpenAPI + Swagger UI
- [x] 認証: パスワード (argon2id)、セッション、レート制限、未設定時 localhost 限定
- [x] TLS (`rustls`、自己署名自動生成)
- [x] 設定 (`wpg.toml` + env + CLI)、`wpg config init`
- [x] ジョブマネージャ (複数ストリーム混合、スケジュール、キャンセル安全性)
- [x] 送出制限ポリシー (許可インタフェース、最大レート)
- [x] CLI サブコマンド: serve / send / render / import / template / ifaces / doctor / bench
- [x] `tracing` ログ設定 (JSON / ローテーション)

### M4: GUI MVP（4–6 週）
- [x] Leptos プロジェクト、Tailwind、ルーティング、i18n、テーマ
- [x] 認証画面・セッション処理
- [x] ビルダー: プロトコルパレット / レイヤスタック D&D / フィールドエディタ (型別 UI) / Auto-Manual トグル
- [x] Hex ダンプ + ツリーの双方向ハイライト、Hex 直接編集
- [x] ブラウザ内 WASM プレビュー (`wpg-core`)、サーバ結果との一致テスト（実行時に `POST /api/render` と突合しバッジ表示）
- [x] ジェネレータ設定 UI (range / list / random …) と展開件数・所要時間の見積表示（所要時間はビルダーのレート欄 = `tx.rate` から算出。`%` / unlimited はリンク速度不明のため非表示）
- [x] ジョブ画面: ストリーム混合、レート設定、開始/停止、リアルタイム統計グラフ
- [x] テンプレート画面: 一覧 / 検索 / タグ / import / export / 複製
- [x] 設定画面
- [x] Undo/Redo、ショートカット、自動保存 (draft)
- [x] キーボードのみで D&D 相当操作、aria
- [x] E2E テスト (Playwright)（`e2e/`、CI job `e2e`。Chromium のみ）

### M5: テンプレート形式 & 共有（1–2 週）
- [x] YAML/JSON スキーマ (`schema: 1`)、JSON Schema 公開、`wpg template lint`
- [x] テンプレ引数化 (変数)、実行時オーバーライド
- [x] `.wpgbundle` エクスポート/インポート (zip)
- [x] ライブラリパス複数指定 (Git 管理ディレクトリ読込)、ホットリロード
- [x] 同梱サンプルテンプレ (P0 プロトコル + 代表シナリオ 5 本)（`templates/protocols` / `templates/scenarios`、`wpg-template/tests/samples.rs` で lint + render を検査）
- [x] pcap/pcapng import → テンプレ化、Hex 貼付け import

### M6: P1 プロトコル群（4–6 週、並行可）
- [x] LLDP / CDP / STP,RSTP,MSTP / LACP / EAPOL / PPPoE / PPP（RST/MSTP 拡張部は opaque 領域で近似）
- [x] VXLAN / GRE / NVGRE / MPLS / MPLS-in-UDP / L2TP / GTP-U / Geneve / IP-in-IP
- [x] OSPFv2/v3 / BGP / RIP / RIPng / EIGRP(基本)（制約: LSA は 1 LSU に 1 つ、BGP UPDATE の長さ 2 種は手動）
- [x] VRRP / HSRP / GLBP
- [x] IGMP / MLD / PIM（制約: PIM は IPv4 のみチェックサム自動）
- [x] DHCPv4/v6 / DNS / NTP / SNMP / Syslog / RADIUS / TFTP（制約: DNS セクションは opaque、SNMP は短形式長 <128 octets、RADIUS Authenticator は手動）
- [x] 各プロトコルのサンプルテンプレ + golden pcap + ドキュメント (自動生成分 + 補足)（サンプルテンプレ・golden バイトテスト・tshark 突合・フィールド説明 ja/en、補足ページ `docs/{ja,en}/src/protocols/`。リファレンスの自動生成は M11）
- [x] wpg-core 拡張で上記の近似を解消: 先頭ニブル/残長による demux、リスト件数の Auto、NUL 終端文字列型、チェックサム範囲の除外指定、送信元ポートでの demux（`Peek` / `min_len` / `count` / `cstring` / `exclude` / `alt_field`。MPLS・GTP-U・PIM Register の IP 判別、IGMPv3/MLDv2 クエリ、VRRPv2・HSRPv2 の読み直し、OSPF/LSA チェックサム、TFTP 文字列、応答方向の UDP/TCP が正確に。残る近似は各モジュール doc 参照）

### M7: P2 OSI / マニアック領域（4–6 週）
- [x] 802.2 LLC (0xFE) / SNAP 上の OSI カプセル化基盤、NSAP 型とエディタ（LLC `0xfe` / PPP `0x0023` で NLPID を peek、未対応 NLPID は汎用 `osi` PDU。`nsap` は長さ指示子付きで合成・解析、ビルダーで AFI / エリア / System ID / NSEL を分解表示。SNAP 上の OSI は一般的でないため対象外）
- [x] IS-IS: IIH (LAN L1/L2, P2P) / LSP / CSNP / PSNP、主要 TLV / sub-TLV、Fletcher checksum、Padding 自動（NLPID `0x83` → 汎用 `isis` → `pdu_type` で各 PDU に読み直し。Padding は Hello の `pdu_length` を Manual にするとその長さまで TLV 8 を自動追加。wpg-core に `size(field)` Auto・`FromField { div }`・`Length::Count`・`TlvFill` を追加。HMAC-MD5 認証値は手動、System ID は 6 オクテット固定）
- [x] CLNP / ES-IS（NLPID `0x81` → `clnp`（DT / MD / ER / ERQ / ERP を 1 レイヤ、データは次レイヤ）、`0x82` → 汎用 `esis` → `pdu_type` で ESH / ISH / RD に読み直し。長さ指示子・セグメント長・Fletcher チェックサム・SP ビット・ESH アドレス数は Auto、オプションは長さ指示子まで（wpg-core に `Length::UntilOffset` を追加）。セグメンテーション部の全長は手動、分割 / 再構成はしない）
- [x] SCTP (CRC32c)（`sctp` 1 レイヤに共通ヘッダと全チャンク（DATA / INIT / INIT ACK / SACK / HEARTBEAT / ABORT / SHUTDOWN 系 / ERROR / COOKIE 系 / AUTH / I-DATA / ASCONF / RE-CONFIG / FORWARD TSN / PAD）、パラメータ・エラー原因。CRC32c・長さ・4 オクテット詰め・SACK 件数は Auto。IPv4/IPv6 protocol 132、UDP 9899。wpg-core の TLV に `head_octets`（型と長さの間のフラグ）・`align` / `unpadded_last`（長さに含めない詰め物）を追加。DATA のユーザデータはチャンク内のオクテット列で PPID による demux はしない）
- [x] OSPF Opaque/SR、BGP MP/EVPN/VPNv4 NLRI（OSPFv2 の Opaque LSA（LS タイプ 9/10/11 を `ospfv2_opaque_lsa` に読み直し、`opaque_type` で TE / Router Information / Extended Prefix / Extended Link 本体）と SR の TLV / sub-TLV（SR-Algorithm、SID/Label Range、SRLB、SRMS Preference、Prefix-SID、Adj-SID / LAN Adj-SID。SID は 3 / 4 オクテットを値長で判別）。TLV の長さ・4 オクテット詰め・Opaque タイプ・LSA チェックサムは Auto。BGP は `MP_REACH_NLRI` / `MP_UNREACH_NLRI` を AFI / SAFI ごとの型付き項目に（IPv4・IPv6 ユニキャスト、VPN-IPv4 / VPN-IPv6（ラベル + RD + プレフィックス）、EVPN ルートタイプ 1〜5）、PMSI Tunnel 属性、UPDATE の経路をプレフィックスのリストに、2 つの長さを Auto に、Extended Length（255 オクテット超で自動）に対応。wpg-core に `TlvVariant::value_prefix` / `value_len`（同じタイプのバリアントを値で判別）と `TlvDef::long_length` を追加。OSPFv3 の E-LSA / SR、ADD-PATH、複数ラベルのスタック、BGP Prefix-SID 属性は対象外）
- [x] LDP / RSVP-TE (基本) / BFD（`ldp` 1 レイヤ = 1 PDU に全メッセージ（Notification / Hello / Initialization / KeepAlive / Address 系 / Label Mapping・Request・Withdraw・Release・Abort）と TLV（FEC のプレフィックス要素とワイルドカード、IPv4 / IPv6 Address List、Hop Count、Path Vector、Generic Label、Status、Common Hello（GTSM ビット）/ Session Parameters、トランスポートアドレス、構成シーケンス番号）。PDU 長・メッセージ / TLV 長は Auto、メッセージは PDU 長で終わる。UDP / TCP 646。`rsvp` 1 レイヤに共通ヘッダと全オブジェクト（SESSION / HOP / TIME_VALUES / ERROR_SPEC / SCOPE / STYLE / FLOWSPEC / FILTER_SPEC / SENDER_TEMPLATE / SENDER_TSPEC / RESV_CONFIRM / LABEL / LABEL_REQUEST / ERO / RRO / HELLO / MESSAGE_ID / SESSION_ATTRIBUTE、IPv4 / IPv6 / LSP トンネル）、ERO / RRO サブオブジェクト。チェックサム・長さ・セッション名長と詰め物は Auto。IPv4 / IPv6 protocol 46。`bfd` 制御パケットと認証セクション（簡易パスワード、Keyed / Meticulous MD5・SHA1）、長さと A ビットは Auto。UDP 3784 / 4784 / 6784（Micro-BFD）。wpg-core の TLV に `len_first`（長さが型より前）を追加。状態機械・タイマー、BFD のダイジェスト計算、GMPLS オブジェクト・ADSPEC・POLICY_DATA の型付け、IntServ の 2 つ目以降のパラメータ、同一セグメント内の複数 LDP PDU のデコードは対象外）
- [x] SRv6 SRH / SR-MPLS 補助（`ipv6_srh`: ルーティングタイプ 4 を IPv6 next header 43 のルーティングタイプ peek で判別、セグメントリスト（IPv6 アドレスのリスト）と Pad1 / PadN / HMAC TLV。`hdr_ext_len`・`last_entry`・Pad1 詰め物は Auto、`segments_left` が 0 でない間は UDP / TCP / ICMPv6 の疑似ヘッダ宛先を最終宛先（セグメントリスト要素 0）に置き換え（高速パッチ経路も追従）。IPv6 next header 143 → Ethernet（End.DX2）。SR-MPLS は `mpls.label` の予約ラベル名（ELI 7、GAL 13 など）と SRGB + インデックスを `expr` で書くサンプル / シナリオ、エントロピーラベル。wpg-core に `Peek::skip`・`count(field, adjust)`・`Length::UntilOffset { adjust }`・`ProtocolDef::final_destination` を追加。HMAC の計算、`ipv6.dst` のアクティブセグメントへの自動設定、`segments_left <= last_entry` の検査、uSID / CSID の専用表現、SRGB の範囲検査は対象外）
- [ ] PTP / ESMC / ERPS / Y.1731 CFM / 802.3ah OAM
- [ ] MACsec (構造のみ) / GTP-C / Diameter / SIP / RTP
- [x] 実機 (or CML2 上 IOS-XR / Junos / FRR) との相互接続確認シナリオをドキュメント化（`templates/interop/` に 7 シナリオ（OSPFv2 2-Way、RIPv2 経路導入、IS-IS L2 隣接 Up、VRRPv3 プリエンプト、PIM ネイバー、LDP ディスカバリ、BFD セッション Up）と `scripts/interop/frr-netns.sh`（名前空間 + veth で FRR 8.4.4 に対して一括確認、全 PASS）。手順・確認コマンドは `docs/{ja,en}/src/operations/interop.md`。IOS-XR / Junos の設定例と確認コマンドは記載のみで実機・CML2 では未確認。IPv6 シナリオ・TCP を要する BGP / LDP セッションは対象外）

### M8: 高速バックエンド（3–5 週）
- [ ] AF_XDP バックエンド (`xsk-rs`、copy / zero-copy 自動、kernel 5.10+)、対応 NIC 一覧
- [ ] `dpdk-sys` crate (bindgen、DPDK 24.11 / 23.11 LTS)
- [ ] DPDK バックエンド (`--features dpdk`)、EAL 設定、hugepages / `dpdk-devbind` 手順、virtio PMD (CML2)
- [ ] マルチキュー・マルチコア送出、NUMA 配慮
- [ ] レート制御精度の改善 (busy-poll、TSC)
- [ ] 性能レグレッションテスト (`wpg bench` を CI nightly)

### M9: 受信 / 検証（2–3 週）
- [ ] 受信カウンタ (pps/bps、BPF フィルタ一致)
- [ ] 送出シグネチャ照合 (ロス / 順序 / 遅延概算)
- [ ] 最初の N パケットの表示、pcapng 保存
- [ ] GUI 受信画面

### M10: デプロイ・配布・運用（2 週）
- [x] `scripts/deploy.sh` (scp + setcap + systemd + healthcheck)、`--dpdk` オプション（ホスト側は `scripts/lib/deploy-remote.sh`。`--service` で `wpg` ユーザ / `/etc/wpg/wpg.env` / unit 投入、パスワード無しの非ループバック起動を拒否、`/api/health` 待ち、`wpg.prev` と `--rollback`。systemd + sshd の Ubuntu 24.04 コンテナで確認、shellcheck 済み。`--dpdk` は hugepages / hugetlbfs / vfio-pci / IOMMU 確認の準備のみで NIC バインドと DPDK 本体は対象外 (M8)。CML2 実機は未確認）
- [ ] `scripts/release.sh` / cargo-dist、GitHub Release 3 OS
- [ ] Dockerfile / compose、systemd unit、launchd plist、Windows サービス手順
- [ ] Windows: Npcap 検出とインストール案内、`build.ps1` 動作確認
- [x] macOS: 署名/公証は要検討 (未署名の回避手順をまず記載)（Gatekeeper / quarantine の回避手順を install-macos に記載、署名は Developer ID 未取得のため見送りと dev/release に記載。実機での確認は未実施）
- [x] アップグレード手順・テンプレマイグレーション（`wpg template migrate [--check] [--no-backup]` を追加。`schema:` の移行機構 `MIGRATIONS` と DB の `schema_version` は既存。docs `operations/upgrade.md` に手順、DB / 設定 / テンプレの互換方針、スキーマ追加手順。schema は 1 のみのため実移行のテストは対象外）

### M11: ドキュメント仕上げ（継続 + 2 週）
- [x] Getting Started (3 OS)、チュートリアル 5 本 (ja/en)（Linux / macOS / Windows のインストール・権限・初回ログイン、ARP スイープ / VLAN SYN / IPv6 NDP / レート制御 / pcap 再生のチュートリアルを ja/en で整備し、CLI の実出力と tshark 突合で確認）
- [ ] プロトコルリファレンス自動生成 (定義 IR → Markdown)
- [ ] API / CLI / 設定 / テンプレスキーマ リファレンス
- [ ] DPDK / AF_XDP / CML2 セットアップガイド、性能チューニング
- [x] 開発者ガイド (プロトコル追加、i18n 追加、リリース)（crate 構成・プロトコル追加の手順と部品・i18n キー追加・リリースゲートを ja/en で整備し、CONTRIBUTING.ja.md と整合）
- [x] FAQ / トラブルシュート、法的注意（`wpg doctor` チェック表とよくある質問、責任ある利用 / デュアルライセンス / 第三者コンポーネントの法的注意を ja/en で整備）
- [ ] スクリーンショット・動画 GIF

### M12: v1.0 リリース判定
- [ ] P0 + P1 プロトコル全 golden pcap 緑、tshark 突合緑
- [ ] 3 OS でビルド・起動・pcap バックエンド送出確認
- [ ] Linux afpacket で 1 Mpps/core 以上 (参考値)、DPDK は環境依存のため参考記録
- [ ] GUI E2E 緑、i18n ja/en 抜け漏れ 0 (lint)
- [ ] `cargo audit` / `deny` クリーン
- [ ] ドキュメント ja/en 完備、CHANGELOG

---

## 5. 想定リスクと対策

| リスク | 対策 |
|---|---|
| Leptos + 大量 DOM (Hex 9000B) の描画性能 | 仮想スクロール、canvas フォールバック検討 |
| DPDK ビルドの環境依存が重い | feature フラグで分離、Docker ビルド環境提供、CI は Linux のみ |
| Windows 高速送出の限界 (Npcap) | 期待値をドキュメントで明示、Windows は「GUI + 低〜中速送出」位置づけ |
| プロトコル数の膨張でメンテ不能 | 宣言的定義 + golden pcap + 自動ドキュメント生成で 1 プロトコル追加コストを一定化 |
| 権限周り (raw socket) の OS 差 | `wpg doctor` で自己診断し、OS 別に具体的コマンドを提示 |
| 誤用 (本番 NW への大量送出) | 送出制限ポリシー、確認ダイアログ、監査ログ、ドキュメントでの明記 |
| i18n の抜け漏れ | 文字列直書き禁止の lint (CI)、Fluent キー未定義検出 |

---

## 6. 用語

- **テンプレート**: 1 パケット（＋ジェネレータ）の定義ファイル。展開すると 1 ストリーム
- **ストリーム**: テンプレ展開結果のパケット列 + 送出プロファイル
- **ジョブ**: 複数ストリームを 1 つ以上のインタフェースへ送出する実行単位
- **バックエンド**: 送出方式 (pcap / afpacket / afxdp / dpdk / pcapfile)
- **ジェネレータ**: フィールド値を変化させる規則 (range / list / random / counter / expr)
