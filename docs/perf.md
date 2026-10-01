# 性能ベースライン (wpg bench)

Status: draft / 計測日: 2026-09-29 / English summary は末尾 ([English summary](#english-summary))

このページは `wpg bench` で測った送出性能のベースラインと、その再現方法・解釈・実機で記録すべき項目をまとめたものです。生の JSON は `docs/perf/` に置いています。

> **重要**: ここにある数値はすべて **4 vCPU のコンテナ上の `lo` (ループバック) と null バックエンド** で測ったものです。実 NIC や CML2 上の virtio-net では測っていません。実機の値として引用しないでください。

## 計測環境

| 項目 | 値 |
|---|---|
| マシン | 4 vCPU コンテナ (microVM) |
| カーネル | Linux 6.18 |
| インタフェース | `lo` (MTU 65536)。`wpg ifaces` 上は `up, loopback` |
| フレーム | 既定 64 B (FCS なし。L1 換算は +4 FCS +20 プリアンブル/IFG)。一部 1514 B |
| バッチ | `wpg bench` の既定 (256 フレーム / `send_batch`)。`wpg send` の既定は 64 |
| ビルド | debug (`cargo build`) と release (`cargo build --release -p wpg-cli --features pcap`) |
| 送出フレーム | EtherType 0x88b5 の合成フレーム、レート無制限 |
| afpacket 既定経路 | TPACKET_V3 TX ring (`lo` では 16 KiB スロット × 1024)。`--no-tx-ring` で `sendmmsg` |

## ベースライン (前回の計測)

| Build | Backend | Rate |
|---|---|---|
| debug | null | 7.64–7.76 Mpps |
| debug | afpacket lo, TPACKET_V3 ring | 1.30–1.32 Mpps (674 Mbps L2) |
| debug | afpacket lo, sendmmsg (`--no-tx-ring`) | 1.22 Mpps |
| release | null | 33.3 Mpps |
| release | afpacket lo, ring | 1.44 Mpps |
| release | afpacket lo, sendmmsg | 1.50 Mpps |
| release | afpacket lo, ring + `--qdisc-bypass` | 1.87 Mpps |
| release | afpacket lo, ring, 1514-byte frames | 1.49 Mpps (18.1 Gbps L2) |
| release | `wpg send` null, pre-rendered | ~19 Mpps |
| release | `wpg send` null, on-the-fly | ~80 kpps (最適化中) |

## 再計測 (2026-09-29, release)

```sh
wpg bench --backend null --duration 3 --json        > docs/perf/2026-09-29-null.json
wpg bench --backend afpacket --iface lo --duration 3 --json > docs/perf/2026-09-29-afpacket-lo.json
```

| Backend | 送出数 | 経過 | pps | L2 bps | L1 bps | dropped | JSON |
|---|---|---|---|---|---|---|---|
| null | 129,386,752 | 3.000 s | 43.1 Mpps | 22.1 Gbps | 30.4 Gbps | 0 | [`perf/2026-09-29-null.json`](perf/2026-09-29-null.json) |
| afpacket `lo` (TPACKET_V3 ring, 1024 slots x 16384 B, batch 256) | 5,687,552 | 3.027 s | 1.88 Mpps | 962 Mbps | 1.32 Gbps | 0 | [`perf/2026-09-29-afpacket-lo.json`](perf/2026-09-29-afpacket-lo.json) |

同じ release ビルドでも前回の表 (null 33.3 Mpps、ring 1.44 Mpps) と差があります。コンテナは共有ホスト上の vCPU なので、他の負荷やクロック次第で 1 割から 3 割は簡単に動きます。**絶対値ではなく桁と、同一条件での相対比較**として読んでください。

## 再現手順

```sh
cargo build --release -p wpg-cli --features pcap
# null: エンジン自体の上限 (バックエンドの仕事はゼロ)
./target/release/wpg bench --backend null --duration 3
# afpacket: 権限が必要 (root か setcap。operations/cml2 参照)
sudo ./target/release/wpg bench --backend afpacket --iface lo --duration 3
sudo ./target/release/wpg bench --backend afpacket --iface lo --duration 3 --no-tx-ring
sudo ./target/release/wpg bench --backend afpacket --iface lo --duration 3 --qdisc-bypass
sudo ./target/release/wpg bench --backend afpacket --iface lo --duration 3 --frame-size 1514
# JSON で保存
./target/release/wpg bench --backend null --duration 3 --json > docs/perf/$(date +%F)-null.json
# 全オプション
./target/release/wpg bench --help
```

JSON のフィールド: `backend`, `detail` (ring/sendmmsg の別とスロット構成), `iface`, `frame_size`, `batch_size`, `packets`, `dropped`, `pps`, `bps` (L2), `l1_bps`, `elapsed`。`justfile` に専用レシピは追加していません。

`wpg bench` の `--workers` / `--ramp-up` / `--start-at` やテンプレートを使うベンチは計画中です。存在するかは `wpg bench --help` で確認してください。

## 解釈

- **null バックエンド** はフレーム生成と統計だけの上限です。ここが数十 Mpps 出ているので、エンジン側が 1 Mpps 台の律速になることはありません。
- **`lo` への afpacket は受信経路が律速** です。`lo` は送信したフレームをそのまま同じホストの受信経路 (softirq、`netif_rx`) で処理するため、1 コアで 1.4–1.9 Mpps 前後で頭打ちになります。実 NIC の送信性能を表すものではありません。
- そのうえで TODO §2.2 の期待値 (afpacket 〜1–3 Mpps/core) の範囲に収まっています。実 NIC / virtio では NIC キュー、ドライバ、ホスト側 vhost の処理が律速になるため、必ず実機で再測定します。
- **ring と sendmmsg の差は小さい** (release で 1.44 対 1.50 Mpps)。`lo` ではコピーや受信処理が支配的で、システムコール削減の効果が見えにくいためです。
- **`--qdisc-bypass`** は qdisc 層を通らない分、約 3 割速くなりました (1.44 から 1.87 Mpps)。ただし tc によるシェーピングや優先制御が効かなくなります。
- **1514 B** ではパケットレートはほぼ同じで、L2 で 18.1 Gbps。フレームあたりの固定コストが支配的で、バイト数の影響は小さいことを示します。
- `wpg send` の null は事前展開 (pre-rendered) なら ~19 Mpps、オンザフライ展開は ~80 kpps で、変数展開の最適化が残っています。

## 実機で計測するときに記録する項目

- [ ] NIC 機種とドライバ (`ethtool -i <if>`、`wpg ifaces` の DRIVER 列)、ファームウェア版
- [ ] カーネル (`uname -r`)、ディストリビューション、`wpg --version` とビルドプロファイル/features
- [ ] CPU 機種、コア数、SMT、`governor` (`cpupower frequency-info`)、NUMA 配置 (NIC とスレッドが同じノードか)
- [ ] MTU (`ip link`) とフレームサイズ、ジャンボの有無
- [ ] リング設定 `ethtool -g <if>` (TX/RX リング長)、チャネル数 `ethtool -l <if>`
- [ ] オフロード `ethtool -k <if>` (tso, gso, gro, tx-checksumming など)
- [ ] IRQ アフィニティと `--pin-core` の値、irqbalance の有無
- [ ] hugepages (`grep Huge /proc/meminfo`、DPDK 時は使用ページ数と `dpdk-devbind.py --status`)
- [ ] `wpg` のオプション一式 (`--batch`, `--ring-depth`, `--no-tx-ring`, `--qdisc-bypass`, `--frame-size`)
- [ ] 仮想環境の場合は仮想化方式 (CML2/KVM、virtio-net、vhost の有無、キュー数)
- [ ] 受信側の観測値 (相手ホストの `ip -s link`、`ethtool -S`)。送出側の `packets` はカーネルに渡した数であり、配送を保証しません
- [ ] 各条件で 3 回以上、`--duration` は 10 秒以上。JSON を `docs/perf/<日付>-<backend>-<環境>.json` に保存

## English summary

All figures here were measured in a 4-vCPU container on Linux 6.18 against the `null` backend and the loopback interface `lo`. They are not real-NIC or virtio-net numbers.

| Build | Backend | Rate |
|---|---|---|
| debug | null | 7.64-7.76 Mpps |
| debug | afpacket lo, TPACKET_V3 ring | 1.30-1.32 Mpps (674 Mbps L2) |
| debug | afpacket lo, sendmmsg (`--no-tx-ring`) | 1.22 Mpps |
| release | null | 33.3 Mpps |
| release | afpacket lo, ring | 1.44 Mpps |
| release | afpacket lo, sendmmsg | 1.50 Mpps |
| release | afpacket lo, ring + `--qdisc-bypass` | 1.87 Mpps |
| release | afpacket lo, ring, 1514-byte frames | 1.49 Mpps (18.1 Gbps L2) |
| release | `wpg send` null, pre-rendered | ~19 Mpps |
| release | `wpg send` null, on-the-fly | ~80 kpps (being optimised) |

Re-run on 2026-09-29 (release, 3 s, 64-byte frames, batch 256): null 43.1 Mpps (22.1 Gbps L2); afpacket on `lo` with the TX ring 1.88 Mpps (962 Mbps L2, 0 dropped). Raw JSON is in `docs/perf/`. Run-to-run variance on a shared container is 10-30%, so compare only within one session.

Interpretation: the null backend shows the engine is not the bottleneck; on `lo` the receive path in the same kernel is, so `lo` numbers only bound the afpacket path from above and say nothing about a real NIC. They fall inside the TODO section 2.2 expectation of about 1-3 Mpps/core for afpacket. `--qdisc-bypass` gave about +30% but disables tc shaping. Reproduce with `wpg bench --backend <null|afpacket> [--iface lo] --duration 3 --json` (see the commands above). When measuring real hardware record: NIC and driver, kernel, CPU/governor/NUMA, MTU and frame size, `ethtool -g/-l/-k`, IRQ affinity and `--pin-core`, hugepages, virtualisation details, receiver-side counters, and at least three runs of 10 s or more.
