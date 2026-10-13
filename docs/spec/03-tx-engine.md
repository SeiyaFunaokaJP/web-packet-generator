# 03. 送出エンジン (wpg-tx)

Status: draft / 元: TODO.md §2.2

## バックエンド一覧

| バックエンド | OS | 期待性能 | 備考 |
|---|---|---|---|
| `pcap` (libpcap / Npcap / BPF) | Win / mac / Linux | 〜数十万 pps | 全 OS 共通のフォールバック。Windows では `wpcap.dll` を遅延ロードし、`System32\Npcap` を DLL 検索パスに加える (WinPcap 互換モード不要) ([ADR-0006](../adr/0006-windows-tx.md)) |
| `afpacket` (TPACKET_V3 mmap ring + `sendmmsg`) | Linux | 〜1–3 Mpps/core | CML2 上 Ubuntu (virtio) の主経路。CAP_NET_RAW |
| `afxdp` | Linux 5.10+ | 〜数 Mpps/core | ドライバ対応要。zero-copy / copy モード自動 ([ADR-0005](../adr/0005-af-xdp.md)) |
| `dpdk` | Linux | 10 Mpps+ /core | `--features dpdk`、DPDK 24.11 LTS (23.11 も可) ([ADR-0004](../adr/0004-dpdk-bindings.md)) |
| `pcapfile` | 全 | — | 送出せず pcapng に書き出す (ドライラン、共有、CI テスト) |
| `loopback/null` | 全 | — | ベンチ・テスト用 |

## TxBackend trait

```rust
trait TxBackend {
    fn open(iface: &str, opts: &TxOptions) -> Result<Self>;
    fn send_batch(&mut self, frames: &[Frame]) -> Result<usize>;
    fn stats(&self) -> TxStats;
    fn close(self) -> Result<()>;
}
```

上記は TODO.md の `TxBackend { open(iface, opts) ; send_batch(&[Frame]) ; stats() ; close() }` を Rust 風に書いた Proposal であり、シグネチャは M2 で確定する。

## 送信スレッド

- コア pin、バッチサイズ・リング深さ設定、複数キュー (RSS / TX queue) 対応。
- NUMA 配慮は M8 で追加。

## レート制御

- トークンバケット + busy-poll / `nanosleep` のハイブリッド。
- 精度指標 (目標レートに対する達成率、間隔のジッタ) を UI に表示。M8 で TSC 利用による改善。

## 統計

- 送出 pps/bps、累計、ドロップ、レート達成率、per-stream カウンタ。
- WebSocket で 100ms 間隔配信。

## インタフェース列挙

OS 別 API (`getifaddrs` / `GetAdaptersAddresses` / Npcap) で列挙し、MAC / IP / MTU / リンク状態を表示する。

## 権限

| OS | 必要な権限 |
|---|---|
| Linux | `setcap cap_net_raw,cap_net_admin+ep` 推奨 |
| Windows | 管理者 + Npcap |
| macOS | `/dev/bpf*` の権限 |

起動時に自己診断 (`wpg doctor` と共通ロジック) を行い、GUI に案内を表示する。

## 未決事項 (Open questions)

- `Frame` の表現 (所有バッファ / 借用スライス / DPDK mbuf / UMEM フレームをどう共通化するか。ゼロコピー経路の設計)。
- pre-render 済みパケットとオンザフライ生成をバックエンドへどう供給するか (SPSC リング等)。
- 「回線率 %」の計算基準 (IFG・プリアンブル・FCS を含むか、リンク速度の取得方法。virtio 等で速度不明の場合)。
- FCS 誤り送出に対応するバックエンドと NIC 条件。
- 複数インタフェース同時送出時のスレッド・コア割り当てポリシー。
- Linux 以外の macOS での複数キュー/高速化の余地 (現状 BPF 経由のみ)。
