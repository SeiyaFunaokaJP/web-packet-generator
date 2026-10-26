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
| 多言語 | 日本語・英語（UI / マニュアル / エラーメッセージ）。UI / CLI / エラーメッセージは中国語（簡体字・繁体字）・韓国語・ドイツ語・フランス語・スペイン語にも対応。以降追加可能な構造 |

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
| シリアライズ | `serde` + `serde_yaml`（実体は互換フォーク `serde_norway`）/ `serde_json` | テンプレ YAML、API JSON |
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
- ユーザ定義プロトコル: YAML でフィールド定義を書いてドロップインできる `protocols.d/` 機構（実装済み: `[protocols] dirs` / `--protocols-dir` / `WPG_PROTOCOLS_DIR`、既存プロトコル下への `bind`、Web UI へは `GET /api/protocols/extensions`。例は `examples/protocols.d/`、手順は docs `protocols/user-defined`）
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
- [x] PTP / ESMC / ERPS / Y.1731 CFM / 802.3ah OAM（PTP は 1 メッセージ 1 レイヤ（汎用 `ptp` → `message_type` で Sync / Delay_Req / Pdelay_Req / Pdelay_Resp / Follow_Up / Delay_Resp / Pdelay_Resp_Follow_Up / Announce / Signaling / Management に読み直し）、共通ヘッダ・本体・TLV（Management / Management Error Status / Organization Extension / ユニキャストネゴシエーション 4 種 / Path Trace / Alternate Time Offset Indicator）。messageLength と TLV 長は Auto、TLV は messageLength で終わる。EtherType `0x88F7`（802.1Q / 802.1ad の内側も）と UDP 319（イベント）/ 320（一般）。Slow Protocols `0x8809` をサブタイプで LACP / Marker / OAM / ESMC に振り分け（Marker も正しくデコード）。`esmc` は QL / 拡張 QL TLV（長さ Auto、後続の詰め物は raw 項目）。CFM（EtherType `0x8902`、VLAN 内も）は汎用 `cfm` → `opcode` で CCM / LBM / LBR / LTM / LTR / AIS / LCK / TST / R-APS / LMM / LMR / 1DM / DMM / DMR / CSF / SLM / SLR / 1SL に読み直し、OpCode ごとのフラグ、TLV（End / Sender ID / Port Status / Data / Interface Status / Reply Ingress・Egress / LTM・LTR Egress Identifier / Organization-Specific / Test、長さ Auto）。ERPS は OpCode 40 の `cfm_raps`（要求 / 状態、RB / DNF / BPR、ノード ID）。802.3ah OAM は汎用 `efm_oam` → `code` で Information / Event Notification / Variable Request・Response / Loopback Control / Organization Specific、Information TLV・リンクイベント TLV（長さ Auto）。wpg-core に `Length::UntilTlv`（End TLV で終わる TLV 列）を追加し、最内レイヤ後のゼロ詰めを最小フレームの詰め物として認識。管理 ID ごとの Management データ・PTP 認証 TLV、MAID / MEG ID の名前形式への分解、Test TLV の CRC-32、Linear APS / MCC / EXM / VSM の専用定義（汎用 `cfm`）、OAM 変数記述子 / コンテナの型付け（オクテット列）、各状態機械は対象外）
- [x] MACsec (構造のみ) / GTP-C / Diameter / SIP / RTP（`macsec` は EtherType `0x88e5`（802.1Q の後も可）で SecTAG（TCI / AN、SL、PN、SCI）+ 不透明なセキュアデータ + 不透明な ICV（16 オクテット、計算しない）。SC ビットは SCI の有無で Auto、SL は手動。`gtpv2` は UDP 2123 で 1 レイヤ = 1 メッセージ: ヘッダ（T フラグは TEID の有無、メッセージ長は Auto、P フラグでピギーバックされたメッセージは次の `gtpv2` レイヤ）と IE（IMSI、Cause、Recovery、APN、AMBR、EBI、IP Address、MEI、MSISDN、PAA、Bearer QoS、RAT Type、Serving Network、F-TEID、PDN Type、Charging ID、APN Restriction、Selection Mode など。IP Address / PAA / F-TEID は IE 長で IPv4 / IPv6 を判別、Bearer Context はグループ化 IE で 1 段）。`diameter` は TCP 3868 でヘッダ（長さ Auto、R / P / E / T、Command Code、Application-ID、Hop-by-Hop、End-to-End）と AVP（V / M / P フラグ、V は Vendor-ID の有無で Auto、長さと 4 オクテット詰めは Auto、基本プロトコルと Credit-Control の AVP をデータ型ごとに型付け、グループ化 AVP は 1 段）。`sip` は UDP / TCP 5060 で開始行・ヘッダ行（CRLF と空行は自動）・本文。`rtp` は CSRC 件数と拡張ヘッダ（X ビット・ワード長）が Auto。RTCP は 1 パケット 1 レイヤ（`rtcp_sr` / `rtcp_rr` / `rtcp_sdes` / `rtcp_bye` / `rtcp_app`、その他は汎用 `rtcp` で本体はオクテット列）で複合パケットはレイヤを重ねる。件数と長さは Auto。RTP / RTCP は固定ポートがないためレイヤを明示して作り、解析は IANA の 5004 / 5005 のみ。wpg-core に `Length::Until`（終端オクテット列で終わる bytes / list）・`Length::ToEndExcept`・`TlvDef::uncounted_octets` を追加（`Length::UntilOffset { adjust }` は SRv6 で追加済みのものを使用）。MACsec の暗号化 / ICV 計算・XPN、GTPv1-C、SIP の Content-Length の Auto、Diameter のベンダ固有 AVP 辞書と SCTP DATA チャンクからの demux、2 段以上のグループ化、RTCP フィードバック / XR の型付けは対象外）
- [x] 実機 (or CML2 上 IOS-XR / Junos / FRR) との相互接続確認シナリオをドキュメント化（`templates/interop/` に 7 シナリオ（OSPFv2 2-Way、RIPv2 経路導入、IS-IS L2 隣接 Up、VRRPv3 プリエンプト、PIM ネイバー、LDP ディスカバリ、BFD セッション Up）と `scripts/interop/frr-netns.sh`（名前空間 + veth で FRR 8.4.4 に対して一括確認、全 PASS）。手順・確認コマンドは `docs/{ja,en}/src/operations/interop.md`。IOS-XR / Junos の設定例と確認コマンドは記載のみで実機・CML2 では未確認。IPv6 シナリオ・TCP を要する BGP / LDP セッションは対象外）

### M8: 高速バックエンド（3–5 週）
- [x] AF_XDP バックエンド (`xsk-rs`、copy / zero-copy 自動、kernel 5.10+)、対応 NIC 一覧（`xsk-rs` / libbpf を使わずカーネル ABI を直接呼ぶ最小実装 `wpg-tx::afxdp`、`--features afxdp` で任意化（Linux 以外ではモジュールごと無効、wasm チェック対象外）。ワーカーごとにソケット + UMEM（NIC の NUMA ノードに `mbind`）+ TX / completion リング、need_wakeup、`--xdp-mode auto|copy|zerocopy`（auto は `XDP_PASS` を `bpf_link` でドライバモードに付けて zero-copy を試し、失敗理由を表示して copy に切り替え）、`--queue`、`--busy-poll`（`SO_PREFER_BUSY_POLL`）。`lo` / 4 キュー veth で copy モードの送出・到着・zero-copy 拒否時の切り替えを確認、対応 NIC 一覧を docs に記載。**物理 NIC での zero-copy は実機未確認**。マルチバッファ（ジャンボフレーム）、RX、共有 UMEM は対象外）
- [x] `dpdk-sys` crate (bindgen、DPDK 24.11 / 23.11 LTS)（`crates/dpdk-sys`: `link` feature のときだけ pkg-config で `libdpdk` (23.11 以上) を探してリンクし、bindgen で EAL / ethdev / mempool・mbuf / lcore の関数と型、`RTE_*` 定数を生成。ヘッダ内で `static inline` の高速パス (`rte_eth_tx_burst` / `rx_burst`、`rte_pktmbuf_alloc` / `_bulk` / `free` / `append` / `mtod` など) と `rte_errno` は DPDK の cflags でコンパイルした C の shim (`wpg_` 接頭辞) で包む。feature なしでは空なので通常のビルドに DPDK は不要。`wpg-tx` の `dpdk` feature で有効化。`--no-huge --no-pci` の EAL と `net_ring` 仮想ポートで 1 フレームを送って受け戻すスモークテストを、Ubuntu 24.04 の 23.11.4 とソースからビルドした 24.11.3 で確認 (root / 非 root)。CI ジョブ `dpdk` は Debian 13 (24.11) と Ubuntu 24.04 (23.11)。静的リンク版 DPDK は対象外)
- [x] DPDK バックエンド (`--features dpdk`)、EAL 設定、hugepages / `dpdk-devbind` 手順、virtio PMD (CML2)（`wpg_tx::dpdk`: EAL はプロセスごとに 1 度、最初の `dpdk` ジョブで専用スレッド上に起動（`--eal-args` / `tx.dpdk_eal_args`、未指定なら `-l <先頭 CPU>` と `--in-memory`（`--no-huge` 時は `--no-shconf`）を補う。引数の変更・失敗後は再起動が必要）。インタフェースはポート番号・PCI アドレス・仮想デバイス（devargs 付き、`rte_dev_probe` で追加）。`--workers N` は 1 ポートを `--queue`+N 本の TX キューで設定しワーカー i がキュー `--queue`+i、ポートは同時に 1 ジョブ。NUMA ソケット上の mbuf プール、フレームを mbuf にコピーして `rte_eth_tx_burst`、リング満杯は 10 ms 再試行後 dropped、`MBUF_FAST_FREE`、送出スレッドは `rte_thread_register`。`wpg-cli` の `dpdk` feature。docs に EAL 引数・hugepages・`dpdk-devbind`・vfio / no-IOMMU・CML2 の virtio PMD を ja/en で記載。Ubuntu 24.04 の DPDK 23.11.4 で `net_null`（`wpg bench` 2 ワーカー）・`net_ring`・`net_pcap`（`wpg send` の出力が pcapfile と一致）を確認、CI ジョブ `dpdk` で 24.11 / 23.11 のテスト。**物理 NIC・hugepages・CML2 の virtio PMD は実機未確認**。DPDK 受信、2048 オクテット超のフレーム（マルチセグメント mbuf）、TX オフロードは対象外）
- [x] マルチキュー・マルチコア送出、NUMA 配慮（`--workers` の afxdp はワーカー i をキュー `--queue`+i に bind、afpacket は送出 CPU から XPS / `CPU mod キュー数` で分散。`--auto-pin` で NIC の NUMA ノードのコアを優先して固定（許可 cpuset 内、コア 0 は最後）、`wpg_tx::pin` に cpulist / NUMA / キュー数の取得と `mbind`、ワーカー別の backend 詳細をレポートに。veth 4 キューで 1→4 ワーカーの伸びを計測。XPS / IRQ アフィニティの自動設定は対象外（手順を docs に記載））
- [x] レート制御精度の改善 (busy-poll、TSC)（`wpg_tx::clock`: invariant TSC を `rdtsc` で読む時計（1 回校正、使えなければ monotonic）、送出スレッドのタイマスラックを 1 µs に、sleep の寝過ごしを EWMA で補正、`WaitMode::BusyPoll`（spin 中に `TxBackend::service` を呼ぶ）、`--wait` / `--clock`、遅れの p50 / p99 ヒストグラム。`examples/pacing.rs` で変更前後を計測し docs/perf.md に記録（hybrid 1 kpps の平均遅れ 74.5 → 10–15 µs など）。共有ホストの vCPU 横取りによる ms 級の外れ値は対象外）
- [x] 性能レグレッションテスト (`wpg bench` を CI nightly)（`.github/workflows/nightly-bench.yml`: 毎日 + 手動、release で null（1 / 2 ワーカー）・afpacket `lo`（ring / sendmmsg）・afxdp `lo`（copy、使えなければ省略）と pacer 精度を計測し、`scripts/bench-check.py` が `scripts/bench-thresholds.json` のしきい値と比較してジョブのサマリと 90 日の artifact に残す。しきい値は共有ランナー向けに緩め。前回値との自動比較・グラフ化は対象外）

### M9: 受信 / 検証（2–3 週）
- [x] 受信カウンタ (pps/bps、BPF フィルタ一致)（`wpg-rx`: `RxBackend` trait と `afpacket` (Linux、`any` 対応、自ホスト送出分は既定で除外、カーネルドロップ数)・`pcap` (feature)・`pcapfile` (ファイル再生、NIC なしの試験用)・`null`。フィルタは tcpdump 構文のサブセット (ether / vlan / ip / ip6 / arp / tcp / udp / sctp / icmp / host / net / port / portrange / len / `wpg`) をユーザ空間で評価し全 OS で同じ結果。全受信と一致分の pps / bps、`wpg capture`、`/api/captures`、WS `rx_stats`。カーネル BPF へのコンパイル、AF_XDP / DPDK 受信、数十万 pps 超の受信性能は対象外）
- [x] 送出シグネチャ照合 (ロス / 順序 / 遅延概算)（`wpg_core::signature`: フレーム末尾 24 オクテットに magic + stream id + seq + 送出時刻 ns を送出直前に書き込み、UDP / TCP / ICMP / ICMPv6 チェックサムは差分更新。余地のないフレームは `unsigned` に計上。`wpg send --signature [--stream-id]` / ジョブの `signature`。受信側はストリームごとに受信数・ロス概算・順序逆転・重複 (直近 65536 連番)・遅延 min/avg/max・ジッタ。遅延は時計が一致する場合のみ有効。ハードウェアタイムスタンプ、OSPF / GRE 等その他のチェックサム補正は対象外）
- [x] 最初の N パケットの表示、pcapng 保存（CLI `--show N` / `--hex` / `-w`、API `keep` (既定 100、上限 10000) と `save` (`<data_dir>/captures` 配下)、`GET /api/captures/{id}/packets` / `pcapng`。pcapng のタイムスタンプはマイクロ秒。リングバッファ・ファイル分割は対象外）
- [x] GUI 受信画面（`/rx`: 開始フォーム (フィルタの入力中検査)、キャプチャ一覧、WS によるカウンタのリアルタイム更新、ストリーム別照合表、先頭 N フレームの一覧と Hex / ツリー表示 (ブラウザ内 wpg-core で解析)、pcapng ダウンロード。ジョブ画面にシグネチャ設定。E2E `e2e/specs/rx.spec.ts`）

### M10: デプロイ・配布・運用（2 週）
- [x] `scripts/deploy.sh` (scp + setcap + systemd + healthcheck)、`--dpdk` オプション（ホスト側は `scripts/lib/deploy-remote.sh`。`--service` で `wpg` ユーザ / `/etc/wpg/wpg.env` / unit 投入、パスワード無しの非ループバック起動を拒否、`/api/health` 待ち、`wpg.prev` と `--rollback`。systemd + sshd の Ubuntu 24.04 コンテナで確認、shellcheck 済み。`--dpdk` は hugepages / hugetlbfs / vfio-pci / IOMMU 確認の準備のみで NIC バインドと DPDK 本体は対象外 (M8)。CML2 実機は未確認）
- [ ] `scripts/release.sh` / cargo-dist、GitHub Release 3 OS（進捗: `release.sh` と release.yml（cargo-dist は見送り、理由は dev/release）。macOS / Windows 版は `pcap` 付きでビルドし、Windows は固定版の Npcap SDK を取得、`wpcap.dll` は遅延ロード。`scripts/smoke-archive.sh` と PR でも走る 3 OS のスモークテストを追加。Linux のアーカイブはローカルで作成・スモークテスト済み。残り: release.yml の 3 OS ジョブが緑になること（2026-10-13 時点で Actions が全ジョブ即時失敗し未実行）、タグでの実リリース）
- [ ] Dockerfile / compose、systemd unit、launchd plist、Windows サービス手順（進捗: すべて作成済み、Docker と systemd はコンテナで確認済み。launchd（macOS 14）とタスクスケジューラ（Windows）、docker compose を CI で確認するジョブを release.yml に追加。残り: そのジョブが緑になること）
- [ ] Windows: Npcap 検出とインストール案内、`build.ps1` 動作確認（進捗: `wpg doctor` / `build.ps1` / `wpg-service.ps1` の検出と案内、WinPcap 互換モード不要化、Npcap 無しでの送出がクラッシュせずエラーになる処理、SDK があれば `build.ps1` が `pcap` を自動で付ける。`build.ps1` と Npcap 無しの経路を確認する CI ジョブを追加。2026-10-13 に Windows 11 実機（Npcap 1.10.4、既定インストール）で `build.ps1`（trunk + SDK 検出で `pcap` 自動付与）、`wpg doctor`、`wpg serve`（Web UI と `/api/health`）、管理者権限なしでの Npcap 経由の送出と受信（`wpg send --backend pcap --signature` 200 フレームを `wpg capture --backend pcap --verify` で 200 受信、ロス 0）を確認。その際 `wpg doctor` が 32 ビット側（`WOW6432Node`）の Npcap レジストリキーを見落とす点と、Npcap の管理者専用モード（`AdminOnly`）を見ずに権限を警告する点を修正、`NO_COLOR=1` で trunk が失敗する点を回避。残り: CI が緑になること（2026-10-13 時点で GitHub Actions が課金エラーで全ジョブ未実行）。タスクスケジューラ登録（`wpg-service.ps1`）は管理者権限が要るため実機未確認）
- [x] macOS: 署名/公証は要検討 (未署名の回避手順をまず記載)（Gatekeeper / quarantine の回避手順を install-macos に記載、署名は Developer ID 未取得のため見送りと dev/release に記載。実機での確認は未実施）
- [x] アップグレード手順・テンプレマイグレーション（`wpg template migrate [--check] [--no-backup]` を追加。`schema:` の移行機構 `MIGRATIONS` と DB の `schema_version` は既存。docs `operations/upgrade.md` に手順、DB / 設定 / テンプレの互換方針、スキーマ追加手順。schema は 1 のみのため実移行のテストは対象外）

### M11: ドキュメント仕上げ（継続 + 2 週）
- [x] Getting Started (3 OS)、チュートリアル 5 本 (ja/en)（Linux / macOS / Windows のインストール・権限・初回ログイン、ARP スイープ / VLAN SYN / IPv6 NDP / レート制御 / pcap 再生のチュートリアルを ja/en で整備し、CLI の実出力と tshark 突合で確認）
- [x] プロトコルリファレンス自動生成 (定義 IR → Markdown)（`wpg docs gen`（隠しサブコマンド、`just docs-gen`）がカタログから `docs/{ja,en}/src/reference/protocols.md` と 1 プロトコル 1 ページ（フィールドの型・既定値・Auto 規則・説明（Fluent）、enum / flags / TLV / リストの詳細、下位レイヤからの到達、次レイヤ表、制約、`protocol:<id>` タグのサンプル）、`SUMMARY.md` のレイヤ別エントリ、手書き補足ページ末尾の双方向リンク（本文のバッククォート ID から抽出）を生成。新プロトコルは再実行で自動反映、wpg-cli のテストが生成物の鮮度を検査。P0 プロトコル（Ethernet / 802.3 / LLC / SNAP / VLAN / IPv4 / IPv6 と拡張ヘッダ / ICMP / NDP / TCP / UDP / ペイロード）のフィールド説明は 2026-10-26 に全 8 言語で追加し、ビルダーのフィールド名にツールチップとして表示。優先度 P0〜P3 の分類は対象外）
- [x] API / CLI / 設定 / テンプレスキーマ リファレンス（同じ生成ツールで ja/en を生成: API は utoipa の OpenAPI からエンドポイント・パラメータ・レスポンス・スキーマ（`docs/schema/openapi.json` も出力）、CLI は clap 定義から全コマンドの `--help`（コマンドごとの補足は任意の Fluent キー）、設定は `Config` の JSON Schema（schemars）からキー・型・既定値・環境変数名と Fluent の説明、`wpg config init` の例、テンプレートスキーマは JSON Schema のキー表を手書きページ末尾に。API / テンプレートスキーマの説明文は doc コメント由来の英語のまま）
- [x] DPDK / AF_XDP / CML2 セットアップガイド、性能チューニング（`operations/dpdk-afxdp`（AF_XDP の要件・copy / zero-copy・対応 NIC、DPDK のホスト準備・EAL 引数・CML2 の virtio PMD・バインディングのビルド）、`operations/cml2`（ラボの組み方・権限・doctor・MTU・CML2 でのバックエンドの選び方）、`operations/tuning`（進め方の手順、pin / batch / ring / qdisc bypass / workers / NUMA / IRQ / レート精度）を ja/en で整備。物理 NIC・CML2 実機での確認と計測は M12 の性能項目で行う）
- [x] 開発者ガイド (プロトコル追加、i18n 追加、リリース)（crate 構成・プロトコル追加の手順と部品・i18n キー追加・リリースゲートを ja/en で整備し、CONTRIBUTING.ja.md と整合）
- [x] FAQ / トラブルシュート、法的注意（`wpg doctor` チェック表とよくある質問、責任ある利用 / デュアルライセンス / 第三者コンポーネントの法的注意を ja/en で整備）
- [x] スクリーンショット・動画 GIF（`e2e/screenshots/capture.spec.ts` + `screenshots.config.ts`（`just screenshots`）が実際の `wpg serve` からログイン / ダッシュボード / テンプレート / ビルダー / 新しいジョブ / ジョブ詳細 / 受信 (シグネチャ照合) の 7 画面を ja / en で撮り、ビルダー操作の GIF を 1 手順 1 コマの静止画から ffmpeg で組み立てて `docs/{ja,en}/src/images/` に出力。README (ja/en)、はじめに、初回ログイン、受信と照合のページに掲載し、リリース手順に UI 変更時の再生成を追記。撮影は Linux コンテナ上 (インタフェースと doctor の警告はその環境のもの)。ダークテーマ版・動画 (MP4 / WebM)・チュートリアルごとの画面は対象外）

### M12: v1.0 リリース判定
- [ ] P0 + P1 プロトコル全 golden pcap 緑、tshark 突合緑
- [ ] 3 OS でビルド・起動・pcap バックエンド送出確認（進捗: Windows 11 実機で `build.ps1` ビルド・起動・Npcap 経由の送出と受信を確認（2026-10-13）。残り: macOS、Linux の pcap バックエンド）
- [ ] Linux afpacket で 1 Mpps/core 以上 (参考値)、DPDK は環境依存のため参考記録
- [ ] GUI E2E 緑、i18n ja/en 抜け漏れ 0 (lint)
- [x] `cargo audit` / `deny` クリーン（`serde_yaml` 0.9（非推奨、`unsafe-libyaml` の RUSTSEC-2024-0449 で `cargo deny` が失敗）をメンテされている互換フォーク `serde_norway` 0.9.42 に置き換え（ワークスペースで `serde_yaml` の名前のまま、コード変更なし）。`cargo audit` は警告 0、`cargo deny check` は advisories / bans / licenses / sources すべて ok。leptos 0.8 経由のビルド時専用 proc-macro 2 件（`paste`、`proc-macro-error2`、いずれも unmaintained のみ）は理由付きで `deny.toml` / `.cargo/audit.toml` の ignore に残す。重複バージョンは `multiple-versions = "warn"` の警告のみで対象外）
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
