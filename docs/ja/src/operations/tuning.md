# 性能チューニング

Status: draft

まず `wpg bench` で計測し、`docs/perf.md` のベースライン (`lo` と `null` のみで計測) と比較します。特記しない限り、以下のフラグは `wpg send` と `wpg bench` の両方にあります。[CLI リファレンス](../reference/cli.md)を参照してください。

## コア pin: `--pin-core`

`--pin-core <CORE>` は送出スレッドを 1 つの CPU コアに固定します (Linux)。スケジューラによる移動を防ぎ、キャッシュを温かく保ちます。NIC の割り込みを処理していないコアを選び、できれば NIC と同じ NUMA ノードのコアにします (`lscpu`、`/sys/class/net/<if>/device/numa_node`)。そのコアには他の仕事を載せないでください (残りは `isolcpus` や `taskset` で分離)。

## バッチサイズとリング深さ

- `--batch <N>`: `send_batch` 1 回でバックエンドに渡すフレーム数。大きいほどシステムコールが減ります (既定は `wpg send` が 64、`wpg bench` が 256)。大きすぎるとレート制御の粒度が粗くなります。64、128、256、512 で試します。
- `--ring-depth <N>`: afpacket の TX リングのスロット数 (既定 4096)。リング全体は 16 MiB が上限で、`lo` (16 KiB スロット) では実効 1024 になります。深いリングはバーストを吸収しますが、持続レートは上がりません。
- `--burst <N>` (`wpg send`): レート制御の 1 回の許可で連続送出するフレーム数。

## リングと `sendmmsg`: `--no-tx-ring`

afpacket の既定は TPACKET_V3 の TX リングです。フレームを共有スロットにコピーし、バッチごとに `send()` を 1 回呼んで送出を起動します。`--no-tx-ring` は代わりに `sendmmsg` を使います (リングを確保できない場合の自動フォールバックでもあります)。`lo` では両者の差は数 % 程度で (release で ring 1.44、sendmmsg 1.50 Mpps)、他のドライバでは結果が変わり得るため両方を測ってください。bench の表示にはどちらのモードかが出ます。

## `--qdisc-bypass`

qdisc 層を通らず、`lo` で約 30% 高速になりますが、そのトラフィックには tc のシェーピングや netem が効かなくなります。[CML2 での注意点](cml2.md)を参照。

## レート精度: 待ち方 (`--wait`) とクロック (`--clock`)

レート制御はトークンバケットです (`wpg send`)。待ち方は `--wait` で選びます。

| `--wait` | 動作 | CPU |
|---|---|---|
| `hybrid` (既定) | 期限が遠い間は `sleep`、最後の 50 マイクロ秒は busy-spin | 待ち時間に比例して少し |
| `sleep` | `sleep` のみ | 最小 |
| `spin` | busy-spin のみ | 1 コアを使い切る |
| `busy-poll` | spin しながらバックエンドの後処理 (afxdp の completion リング回収、ドライバの起動) も行う。`--busy-poll` を付けると既定になる | 1 コアを使い切る |

精度のための工夫 (M8):

- **送出スレッドのタイマスラックを 50 マイクロ秒から 1 マイクロ秒に下げます** (`PR_SET_TIMERSLACK`、Linux。`sleep` を使う待ち方のとき)。
- **`sleep` の寝過ごしを学習します**。実際に何マイクロ秒遅れて起きたかの移動平均を取り、次の `sleep` をその分だけ短くします (残りは spin で詰めます)。
- **クロックに TSC を使います** (`--clock auto`、既定)。x86_64 で TSC が invariant (CPUID、または Linux の `constant_tsc` と `nonstop_tsc`) なら `rdtsc` を直接読み、起動時に一度だけ `Instant` と突き合わせて周波数を求めます。1 回の読み取りが数 ns で済むので、spin の 1 周が短くなり、期限からの遅れが半分程度になります。`--clock monotonic` で従来の `Instant` に戻せます。TSC が使えない環境では自動で monotonic になります。

`lo` のコンテナでの計測結果 (前後比較を含む) は `docs/perf.md` の「レート制御の精度」にあります。要点: 1 kpps の `hybrid` で平均の遅れが 74.5 から 10–15 マイクロ秒、`sleep` では 62–121 から 2–44 マイクロ秒に減りました。中央値の遅れは spin 系で 0.1 マイクロ秒前後 (TSC で 0.03–0.06)。最大値 (数ミリ秒) は共有ホストの vCPU が奪われることによるもので、待ち方では消せません。精度が必要なら `--pin-core` / `--auto-pin` と `isolcpus` を併用してください。

フレーム間隔が待ちの分解能より短い場合、制御は複数フレームをまとめて許可する (`--burst`) ので、*平均* レートは目標に沿いますが個々の間隔は揃いません。`ifg=<時間>` はバースト 1 を強制します。達成レートが指定より低い場合は経路側が限界で、停止後の追い付きバーストは 10 ms 分に制限されるため、詰まりのあとに一気に送り出すことはありません。`wpg send` の最終レポートに達成率と遅れ (最大、平均) が、`--json` には p50 / p99 (`p50_jitter`、`p99_jitter`) と TSC を使ったか (`tsc`) が含まれます。

## マルチキュー・マルチコア送出 (`--workers`)

`--workers N` は N 個の送出ワーカーを起動します。ワーカーごとにスレッド、バックエンドのインスタンス (afpacket はソケットと TX リング、afxdp はソケットと UMEM、dpdk は共有ポートの TX キュー) を持ち、ストリームとレートを N 等分します (`--rate 1Mpps --workers 4` なら 1 ワーカー 250 kpps)。カウンタは合算され、最終レポートにワーカーごとの内訳が出ます。`pcap` と `pcapfile` は 1 ワーカーのみです。

キューへの割り当て:

- **afxdp**: ワーカー `i` は NIC のキュー `--queue` + `i` に bind します。キュー数は `ethtool -L <if> combined N` で用意します ([DPDK / AF_XDP](dpdk-afxdp.md))。
- **dpdk**: ポートを `--queue` + N 本の TX キューで設定し、ワーカー `i` は TX キュー `--queue` + `i` で送ります。mbuf プールは NIC の NUMA ソケットに置きます ([DPDK](dpdk-afxdp.md#dpdk))。
- **afpacket**: キューを直接は選べません。カーネルが送出 CPU からキューを決めます。XPS (`/sys/class/net/<if>/queues/tx-<n>/xps_cpus`) が設定されていればその対応表に従い、なければ `--qdisc-bypass` 時は「CPU 番号 mod キュー数」になります。ワーカーを別々のコアに固定すれば、別々のキューに分散します。

```sh
# 4 キューの NIC に 4 ワーカー、NIC と同じ NUMA ノードのコアに自動で固定
wpg bench --backend afxdp -i ens3 --workers 4 --auto-pin
wpg send tpl.wpg.yaml -i ens3 --backend afpacket --qdisc-bypass --workers 4 --pin-cores 2,3,4,5
```

## NUMA 配慮 (`--auto-pin`)

- `--auto-pin` はワーカーを NIC の NUMA ノード (`/sys/class/net/<if>/device/numa_node`) のコアに順に固定します。プロセスに許可されたコア (`taskset` / cgroup の cpuset) だけを使い、コア 0 (割り込みや雑務が集まりがち) は最後に回します。NUMA 情報がない環境 (仮想 NIC、1 ノードのマシン) では許可されたコアを順に使います。`--pin-cores` を指定した場合はそちらが優先です。
- ワーカーは固定したあとで自分のフレームバッファを確保するので、Linux の first-touch によりメモリはそのコアのノードに置かれます。
- afxdp の UMEM は NIC のノードに割り当てます (`mbind(MPOL_PREFERRED)`)。backend の表示に `NUMA node N` と出ます。
- 割り込み (TX 完了) も同じノードのコアに寄せてください (下の「IRQ アフィニティ」)。

## IRQ アフィニティと CPU governor

- NIC の割り込みを分散または固定します: `cat /proc/interrupts | grep <if>`、`echo <mask> | sudo tee /proc/irq/<n>/smp_affinity`。手動で固定する場合は `irqbalance` を止めます。送出スレッドと TX 完了割り込みは同じ NUMA ノードに置きます (同じコアである必要はありません)。
- governor は `performance` にします: `sudo cpupower frequency-set -g performance`。VM では governor はホストが制御するため、CML2 ホストの管理者に確認してください。
- レイテンシのスパイクが出る場合は深い C-state を無効にし、pin したコアの SMT 兄弟には負荷を載せません。
