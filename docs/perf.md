# 性能ベースライン (wpg bench)

Status: draft / 計測日: 2026-09-29、2026-10-12 (M8) / English summary は末尾 ([English summary](#english-summary))

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

`wpg bench` は `--workers`、`--template`、`--auto-pin` なども受け付けます (`wpg bench --help`)。

## 解釈

- **null バックエンド** はフレーム生成と統計だけの上限です。ここが数十 Mpps 出ているので、エンジン側が 1 Mpps 台の律速になることはありません。
- **`lo` への afpacket は受信経路が律速** です。`lo` は送信したフレームをそのまま同じホストの受信経路 (softirq、`netif_rx`) で処理するため、1 コアで 1.4–1.9 Mpps 前後で頭打ちになります。実 NIC の送信性能を表すものではありません。
- そのうえで spec §2.2 の期待値 (afpacket 〜1–3 Mpps/core) の範囲に収まっています。実 NIC / virtio では NIC キュー、ドライバ、ホスト側 vhost の処理が律速になるため、必ず実機で再測定します。
- **ring と sendmmsg の差は小さい** (release で 1.44 対 1.50 Mpps)。`lo` ではコピーや受信処理が支配的で、システムコール削減の効果が見えにくいためです。
- **`--qdisc-bypass`** は qdisc 層を通らない分、約 3 割速くなりました (1.44 から 1.87 Mpps)。ただし tc によるシェーピングや優先制御が効かなくなります。
- **1514 B** ではパケットレートはほぼ同じで、L2 で 18.1 Gbps。フレームあたりの固定コストが支配的で、バイト数の影響は小さいことを示します。
- `wpg send` の null は事前展開 (pre-rendered) なら ~19 Mpps、オンザフライ展開は ~80 kpps で、変数展開の最適化が残っています。

## 高速バックエンドとマルチワーカー (2026-10-12, release, M8)

`cargo build --release -p wpg-cli --features afxdp`、64 B フレーム、`--duration 3`、バッチ 256。環境は上と同じ 4 vCPU コンテナ (Linux 6.18、TSC 2.1 GHz)。`vxa` は 4 キューの veth (`ip link add vxa numtxqueues 4 numrxqueues 4 type veth peer name vxb numtxqueues 4 numrxqueues 4`) で、相手側 `vxb` には受信アプリがなく、カーネルが受け取って捨てます。

| Backend | 条件 | pps | dropped |
|---|---|---|---|
| afxdp `lo` | copy mode (zero-copy は `lo` に native XDP がなく不可), queue 0, 2048 slots | 2.48 Mpps | 0 |
| afxdp `lo` | 同上 + `--busy-poll` | 2.23 Mpps | 0 |
| afpacket `lo` | TPACKET_V3 ring (比較用) | 1.74 Mpps | 0 |
| afxdp `vxa` | copy mode (veth は zero-copy の bind を拒否), 1 worker | 2.61 Mpps | 0 |
| afxdp `vxa` | `--workers 2 --auto-pin` (queue 0, 1) | 4.51 Mpps | 0 |
| afxdp `vxa` | `--workers 4 --auto-pin` (queue 0–3) | 5.10 Mpps | 0 |
| afpacket `vxa` | ring, 1 worker | 1.48 Mpps | 0 |
| afpacket `vxa` | ring + `--qdisc-bypass`, `--workers 2 --auto-pin` | 2.74 Mpps | 0 |

- AF_XDP の copy モードでも afpacket より 4〜8 割速くなりました (skb 生成までの経路が短く、qdisc を通らないため)。物理 NIC の zero-copy は未計測です。
- ワーカーを増やすとほぼ比例して伸び、4 ワーカー (4 vCPU すべて) では頭打ちになります。`lo` の受信経路と違い、veth は相手側の処理も同じ 4 vCPU を使います。
- `--busy-poll` は copy モードでは効果がありません (NAPI を回すのは zero-copy のドライバ)。むしろ毎回 `sendto` を呼ぶ分だけ遅くなります。

## レート制御の精度 (2026-10-12, release)

`cargo run --release -p wpg-tx --example pacing -- 1 --json > docs/perf/2026-10-12-pacing.json` ([`perf/2026-10-12-pacing.json`](perf/2026-10-12-pacing.json))。最後の vCPU に固定したスレッドで pacer を 1 秒ずつ動かし (各 3 回、p99 が中央の回を採用)、許可されたフレームを 1 回あたり最大 64 個ずつ消費します。「遅れ」は pacer が待っていた期限から実際に起きた時刻までで、単位はマイクロ秒。p50 / p99 は対数ヒストグラムからの値です (±10 %)。

### 変更前 (main の pacer、同じ測り方、平均と最大のみ)

| wait | rate | 達成率 / achieved | mean | max |
|---|---|---|---|---|
| hybrid | 1,000 pps | 100.094 % | 74.50 | 5000 |
| hybrid | 10,000 pps | 100.008 % | 13.89 | 1455 |
| hybrid | 100,000 pps | 100.001 % | 0.52 | 2082 |
| hybrid | 1,000,000 pps | 100.000 % | 0.12 | 1132 |
| sleep | 1,000 pps | 100.091 % | 121.07 | 1394 |
| sleep | 10,000 pps | 100.004 % | 66.51 | 9338 |
| sleep | 100,000 pps | 100.000 % | 63.96 | 926 |
| sleep | 1,000,000 pps | 100.000 % | 61.94 | 569 |
| spin | 1,000 pps | 100.100 % | 15.11 | 1794 |
| spin | 10,000 pps | 100.010 % | 1.96 | 6149 |
| spin | 100,000 pps | 100.000 % | 0.54 | 6057 |
| spin | 1,000,000 pps | 100.000 % | 0.10 | 1859 |

### 変更後 (タイマスラック 1 µs、寝過ごし学習、TSC クロック、busy-poll)

| wait | clock | rate | 達成率 / achieved | mean | p50 | p99 | max |
|---|---|---|---|---|---|---|---|
| hybrid | monotonic | 1,000 pps | 100.100 % | 10.14 | 0.10 | 170 | 1613 |
| hybrid | monotonic | 10,000 pps | 100.010 % | 2.29 | 0.10 | 17.87 | 1870 |
| hybrid | monotonic | 100,000 pps | 100.001 % | 0.50 | 0.09 | 10.62 | 2276 |
| hybrid | monotonic | 1,000,000 pps | 100.000 % | 0.11 | 0.07 | 0.13 | 1670 |
| hybrid | tsc | 1,000 pps | 100.100 % | 14.58 | 0.05 | 286 | 4094 |
| hybrid | tsc | 10,000 pps | 100.011 % | 1.79 | 0.05 | 27.55 | 1312 |
| hybrid | tsc | 100,000 pps | 100.001 % | 0.44 | 0.04 | 10.62 | 3676 |
| hybrid | tsc | 1,000,000 pps | 100.001 % | 0.08 | 0.03 | 0.08 | 1515 |
| sleep | monotonic | 1,000 pps | 100.099 % | 24.03 | 13.78 | 156 | 838 |
| sleep | monotonic | 10,000 pps | 100.010 % | 3.78 | 0.30 | 35.73 | 3562 |
| sleep | monotonic | 100,000 pps | 100.001 % | 12.21 | 13.78 | 42.49 | 2022 |
| sleep | monotonic | 1,000,000 pps | 100.000 % | 2.05 | 0.08 | 27.55 | 1076 |
| sleep | tsc | 1,000 pps | 100.097 % | 43.68 | 25.27 | 240 | 2384 |
| sleep | tsc | 10,000 pps | 100.010 % | 3.54 | 0.23 | 32.77 | 3395 |
| sleep | tsc | 100,000 pps | 100.001 % | 14.63 | 16.38 | 42.49 | 4279 |
| sleep | tsc | 1,000,000 pps | 100.001 % | 2.72 | 0.06 | 35.73 | 166 |
| spin | monotonic | 1,000 pps | 100.100 % | 13.40 | 0.10 | 312 | 3575 |
| spin | monotonic | 10,000 pps | 100.010 % | 3.22 | 0.10 | 32.77 | 2374 |
| spin | monotonic | 100,000 pps | 100.001 % | 0.54 | 0.08 | 11.59 | 1756 |
| spin | monotonic | 1,000,000 pps | 100.000 % | 0.11 | 0.07 | 0.12 | 2614 |
| spin | tsc | 1,000 pps | 100.100 % | 13.33 | 0.05 | 220 | 2274 |
| spin | tsc | 10,000 pps | 100.010 % | 1.34 | 0.05 | 10.62 | 1742 |
| spin | tsc | 100,000 pps | 100.002 % | 0.52 | 0.05 | 11.59 | 3349 |
| spin | tsc | 1,000,000 pps | 100.001 % | 0.09 | 0.04 | 0.08 | 806 |
| busy-poll | monotonic | 1,000 pps | 100.100 % | 5.43 | 0.11 | 92.68 | 1684 |
| busy-poll | monotonic | 10,000 pps | 100.010 % | 1.78 | 0.10 | 9.74 | 2225 |
| busy-poll | monotonic | 100,000 pps | 100.001 % | 0.39 | 0.08 | 9.74 | 1514 |
| busy-poll | monotonic | 1,000,000 pps | 100.000 % | 0.14 | 0.08 | 0.13 | 3124 |
| busy-poll | tsc | 1,000 pps | 100.100 % | 9.70 | 0.06 | 312 | 1692 |
| busy-poll | tsc | 10,000 pps | 100.011 % | 1.24 | 0.05 | 10.62 | 2321 |
| busy-poll | tsc | 100,000 pps | 100.002 % | 0.51 | 0.04 | 10.62 | 2071 |
| busy-poll | tsc | 1,000,000 pps | 100.001 % | 0.09 | 0.04 | 0.08 | 5268 |

読み方:

- **`sleep` を使う待ち方で効果が大きい**: hybrid の 1 kpps で平均 74.5 → 10–15 µs、10 kpps で 13.9 → 1.8–2.3 µs。sleep のみでは 62–121 → 2–44 µs。タイマスラック (50 µs) の分と、毎回の寝過ごしが消えたためです。
- **TSC は中央値を約半分にします** (spin 系の p50 が 0.07–0.11 → 0.03–0.06 µs)。1 回の時刻読み取りが `clock_gettime` より安いためです。
- **p99 と最大値はホスト次第**: 数百 µs〜数 ms の外れ値は共有ホストで vCPU が奪われたもので、待ち方では消せません。`isolcpus` と `--pin-core` を使った専有コアでは小さくなるはずですが、未計測です。
- **平均レートは全条件で目標の ±0.1 % 以内** (1 kpps の 100.1 % は 1 秒の計測で先頭フレームが 1 つ多い分)。

## 性能レグレッションテスト (nightly)

`.github/workflows/nightly-bench.yml` が毎日 18:17 UTC (03:17 JST) と手動実行 (`workflow_dispatch`) で動きます。release ビルドで次を実行し、`scripts/bench-check.py` が `scripts/bench-thresholds.json` のしきい値と比べ、1 つでも外れればジョブを失敗させます。結果の表はジョブのサマリに、生の JSON は 90 日間 artifact に残ります。

- `wpg bench --backend null` (1 / 2 ワーカー)、`afpacket` の `lo` (ring / sendmmsg)、`afxdp` の `lo` (copy mode、ランナーで使えなければ省略)
- `cargo run --example pacing` の達成率と p50 の遅れ

しきい値は GitHub の共有ランナーの揺れ (1〜3 割) を見込んでかなり緩く (null 5 Mpps、afpacket / afxdp 0.2–0.25 Mpps、達成率 ±1〜2 % など)、桁が変わるような退行を捕まえる目的です。値を変えるときは artifact の推移を見てから `scripts/bench-thresholds.json` を更新してください。手元では次で同じ確認ができます。

```sh
mkdir -p bench
wpg bench --backend null --duration 5 --json > bench/null.json
# ... (.github/workflows/nightly-bench.yml の Benchmarks ステップと同じ)
python3 scripts/bench-check.py scripts/bench-thresholds.json bench
```

## 実機で計測するときに記録する項目

- [ ] NIC 機種とドライバ (`ethtool -i <if>`、`wpg ifaces` の DRIVER 列)、ファームウェア版
- [ ] カーネル (`uname -r`)、ディストリビューション、`wpg --version` とビルドプロファイル/features
- [ ] CPU 機種、コア数、SMT、`governor` (`cpupower frequency-info`)、NUMA 配置 (NIC とスレッドが同じノードか)
- [ ] MTU (`ip link`) とフレームサイズ、ジャンボの有無
- [ ] リング設定 `ethtool -g <if>` (TX/RX リング長)、チャネル数 `ethtool -l <if>`
- [ ] オフロード `ethtool -k <if>` (tso, gso, gro, tx-checksumming など)
- [ ] IRQ アフィニティと `--pin-core` の値、irqbalance の有無
- [ ] hugepages (`grep Huge /proc/meminfo`、DPDK 時は使用ページ数と `dpdk-devbind.py --status`)
- [ ] `wpg` のオプション一式 (`--batch`, `--ring-depth`, `--no-tx-ring`, `--qdisc-bypass`, `--frame-size`, `--workers`, `--pin-cores` / `--auto-pin`, afxdp なら `--xdp-mode` / `--queue` と表示された zero-copy / copy)
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

M8 additions (2026-10-12, release, same 4-vCPU container, 64-byte frames, 3 s, batch 256; `vxa` is a 4-queue veth whose peer discards the frames): AF_XDP copy mode reached 2.48 Mpps on `lo` (afpacket ring 1.74 Mpps) and 2.61 Mpps on veth with one worker, 4.51 Mpps with `--workers 2 --auto-pin` and 5.10 Mpps with 4 workers (all four vCPUs); afpacket on veth went from 1.48 Mpps (1 worker) to 2.74 Mpps (2 workers, `--qdisc-bypass`). `--busy-poll` does not help in copy mode (2.23 Mpps on `lo`). Zero-copy on physical NICs has not been measured.

Rate control precision (`cargo run --release -p wpg-tx --example pacing -- 1 --json`, raw data in `docs/perf/2026-10-12-pacing.json`; lateness = how late the pacer woke up relative to its deadline): lowering the TX thread's timer slack to 1 µs and learning the oversleep cut the mean lateness of `hybrid` at 1 kpps from 74.5 to 10-15 µs (10 kpps: 13.9 to 1.8-2.3 µs) and of `sleep` from 62-121 to 2-44 µs; the TSC clock halves the median lateness of the spinning modes (0.07-0.11 to 0.03-0.06 µs). The average rate is within ±0.1 % of the target everywhere. p99 and maximum values (hundreds of µs to ms) are vCPU preemption on the shared host and are not removed by any wait mode.

Performance regression test: `.github/workflows/nightly-bench.yml` runs nightly (18:17 UTC) and on demand, benchmarks null / afpacket `lo` / afxdp `lo` and the pacer, and `scripts/bench-check.py` compares the JSON with the deliberately loose thresholds in `scripts/bench-thresholds.json` (they catch order-of-magnitude regressions on noisy shared runners). Results go to the job summary and a 90-day artifact.
