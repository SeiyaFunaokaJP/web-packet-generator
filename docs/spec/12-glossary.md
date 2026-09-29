# 12. 用語

Status: draft / 元: TODO.md §6

| 用語 | 定義 |
|---|---|
| テンプレート | 1 パケット (＋ジェネレータ) の定義ファイル。展開すると 1 ストリーム。拡張子 `.wpg.yaml` |
| ストリーム | テンプレ展開結果のパケット列 + 送出プロファイル |
| ジョブ | 複数ストリームを 1 つ以上のインタフェースへ送出する実行単位 |
| バックエンド | 送出方式 (pcap / afpacket / afxdp / dpdk / pcapfile) |
| ジェネレータ | フィールド値を変化させる規則 (range / list / random / counter / expr) |

## 追加用語 (本書で使用)

| 用語 | 定義 |
|---|---|
| IR | プロトコル定義の中間表現。合成 / 解析 / GUI / ドキュメント生成の共通入力 ([ADR-0007](../adr/0007-protocol-definition.md)) |
| バンドル | 複数テンプレート + README + 添付 pcap をまとめた zip (`.wpgbundle`) |
| ライブラリパス | テンプレートを読み込む追加ディレクトリ。Git 管理下でも可 |
| golden pcap | 期待出力として固定した pcap。合成結果の回帰テストに使う |
| NSAP | OSI ネットワークアドレス (AFI/IDI/HO-DSP/SystemID/SEL) |
| IIH | IS-IS Hello (IS-to-IS Hello) |
| EAL | DPDK の Environment Abstraction Layer |
| CML2 | Cisco Modeling Labs 2 |
| pre-render / on-the-fly | パケットを事前展開する方式 / 送出中に逐次生成する方式 |
