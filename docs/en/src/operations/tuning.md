# Performance tuning

Status: draft

Measure first with `wpg bench` and compare against the baseline in `docs/perf.md` (measured on `lo`, `null` and a veth / dummy interface on WSL2, not on real NICs; afpacket reached 2-3 Mpps on one core on WSL2). All flags below exist on both `wpg send` and `wpg bench` unless noted; see the [CLI reference](../reference/cli.md).

## Where to start

Change one thing at a time and measure each step with `wpg bench` (64-byte frames, about `--duration 10` seconds).

1. **Know the ceiling**: `wpg bench --backend null` measures frame generation without any transmit path. If a real interface gets close to it, wpg itself is the limit.
2. **afpacket basics**: `--qdisc-bypass` (unless you need tc), `--batch` 64 / 128 / 256 / 512, and a comparison with `--no-tx-ring`.
3. **Pin cores**: `--pin-core` (one worker) or `--auto-pin`. Keep the thread on the NIC's NUMA node with its interrupts, and set the governor to `performance` (see "IRQ affinity and CPU governor" below).
4. **Add workers**: `--workers 2`, `4`, ... up to the number of queues, until the gain stops.
5. **Change the backend**: if that is still not enough, try `afxdp` (`--xdp-mode`, `--busy-poll`), then `dpdk` ([DPDK and AF_XDP setup](dpdk-afxdp.md)). On virtual NICs (CML2 and the like) the gap is often small; see [CML2 notes](cml2.md#choosing-a-backend-on-cml2).
6. **Tighten the rate**: run `wpg send` at the target rate, read the achieved rate and lateness in the final report, and adjust `--wait` / `--clock` (see "Rate accuracy" below).
7. **Record it**: keep the environment (CPU, NIC and driver, kernel, command, `wpg bench --json` output) with the checklist in `docs/perf.md`.

## Core pinning: `--pin-core`

`--pin-core <CORE>` pins the transmit thread to one CPU core (Linux). This stops the scheduler from migrating the thread and keeps caches warm. Choose a core that is not handling the NIC's interrupts, and preferably on the same NUMA node as the NIC (`lscpu`, `/sys/class/net/<if>/device/numa_node`). Keep the core free of other work (`isolcpus` or `taskset` for the rest).

## Batch size and ring depth

- `--batch <N>`: frames handed to the backend per `send_batch` call. Larger batches mean fewer system calls (default 64 for `wpg send`, 256 for `wpg bench`). Too large a batch delays the rate limiter's pacing granularity. Try 64, 128, 256, 512.
- `--ring-depth <N>`: number of frame slots in the afpacket TX ring (default 4096). The ring is capped at 16 MiB in total, so on `lo` (16 KiB slots) the effective depth is 1024. Larger rings absorb bursts; they do not raise the sustained rate.
- `--burst <N>` (`wpg send`): frames sent back to back per rate-control grant.

## Ring versus `sendmmsg`: `--no-tx-ring`

The default afpacket path is the TPACKET_V3 TX ring: frames are copied into shared slots and a single `send()` per batch triggers transmission. `--no-tx-ring` uses `sendmmsg` instead (also the automatic fallback when the ring cannot be set up). On `lo` the two are within a few percent of each other (ring 1.44, sendmmsg 1.50 Mpps in release); on other drivers the result can differ, so measure both. The bench header prints which mode is active.

## `--qdisc-bypass`

Skips the qdisc layer, about 30% faster on `lo`, but disables tc shaping/netem for the traffic. See [CML2 notes](cml2.md).

## Rate accuracy: wait mode (`--wait`) and clock (`--clock`)

The rate limiter (`wpg send`) is a token bucket. `--wait` chooses how it waits:

| `--wait` | Behaviour | CPU |
|---|---|---|
| `hybrid` (default) | `sleep` while the deadline is far away, busy-spin for the last 50 microseconds | a little, proportional to waiting |
| `sleep` | `sleep` only | lowest |
| `spin` | busy-spin only | a full core |
| `busy-poll` | spin and service the backend meanwhile (afxdp: reap the completion ring, wake the driver); the default with `--busy-poll` | a full core |

Precision work (M8):

- **The TX thread's timer slack is lowered from 50 to 1 microsecond** (`PR_SET_TIMERSLACK`, Linux, for the modes that sleep).
- **Oversleep is learned**: a moving average of how late `sleep` actually woke up is subtracted from the next sleep (the rest is spun away).
- **The clock is the TSC** (`--clock auto`, the default): on x86_64 with an invariant TSC (CPUID, or the Linux flags `constant_tsc` and `nonstop_tsc`) the pacer reads `rdtsc` directly, with the frequency calibrated once against `Instant`. A read costs a few ns, so each spin iteration is shorter and the lateness about halves. `--clock monotonic` restores `Instant`; where the TSC is not usable the pacer falls back automatically.

Measurements in a container on `lo` (including a before/after comparison) are in `docs/perf.md`, section "Rate control precision". In short: `hybrid` at 1 kpps went from 74.5 to 10-15 microseconds mean lateness, `sleep` from 62-121 to 2-44 microseconds; the median lateness of the spinning modes is about 0.1 microsecond (0.03-0.06 with the TSC). The maximum (milliseconds) comes from the shared host taking the vCPU away and no wait mode removes it; for precision combine `--pin-core` / `--auto-pin` with `isolcpus`.

When the interval between frames is shorter than the wait resolution, the limiter grants several frames at once (`--burst`), so the *average* rate follows the target while individual spacing does not. `ifg=<duration>` forces a burst of 1. If the achieved rate is lower than requested, the path is the limit: a catch-up burst is capped (10 ms) so it never floods after a stall. The final report of `wpg send` includes the achieved ratio and the lateness (max, mean); `--json` also has p50 / p99 (`p50_jitter`, `p99_jitter`) and whether the TSC was used (`tsc`).

## Multi-queue, multi-core sending (`--workers`)

`--workers N` starts N TX workers. Each has its own thread and backend instance (afpacket: socket and TX ring; afxdp: socket and UMEM; dpdk: a TX queue of the shared port) and gets 1/N of the stream and of the rate (`--rate 1Mpps --workers 4` is 250 kpps per worker). Counters are summed and the final report lists every worker. `pcap` and `pcapfile` are limited to one worker.

Queue assignment:

- **afxdp**: worker `i` binds NIC queue `--queue` + `i`. Create the queues with `ethtool -L <if> combined N` ([DPDK / AF_XDP](dpdk-afxdp.md)).
- **dpdk**: the port is configured with `--queue` + N TX queues and worker `i` sends on TX queue `--queue` + `i`; the mbuf pool lives on the NIC's NUMA socket ([DPDK](dpdk-afxdp.md#dpdk)).
- **afpacket**: the queue cannot be chosen directly; the kernel derives it from the sending CPU. With XPS configured (`/sys/class/net/<if>/queues/tx-<n>/xps_cpus`) it follows that map, otherwise with `--qdisc-bypass` it is "CPU number mod queue count". Pinning the workers to different cores spreads them over different queues.

```sh
# 4 workers on a 4-queue NIC, pinned automatically to cores on the NIC's NUMA node
wpg bench --backend afxdp -i ens3 --workers 4 --auto-pin
wpg send tpl.wpg.yaml -i ens3 --backend afpacket --qdisc-bypass --workers 4 --pin-cores 2,3,4,5
```

## NUMA awareness (`--auto-pin`)

- `--auto-pin` pins the workers, in order, to cores on the NIC's NUMA node (`/sys/class/net/<if>/device/numa_node`). Only cores the process may use (`taskset` / cgroup cpuset) are chosen, and core 0 (which tends to collect interrupts and housekeeping) comes last. Without NUMA information (virtual NICs, single-node machines) the allowed cores are used in order. `--pin-cores` takes precedence.
- Workers allocate their frame buffers after pinning themselves, so Linux's first-touch policy puts that memory on the core's node.
- The afxdp UMEM is allocated on the NIC's node (`mbind(MPOL_PREFERRED)`); the backend line shows `NUMA node N`.
- Keep the (TX completion) interrupts on cores of the same node too (see "IRQ affinity" below).

## IRQ affinity and CPU governor

- Spread or pin NIC interrupts: `cat /proc/interrupts | grep <if>`, `echo <mask> | sudo tee /proc/irq/<n>/smp_affinity`; stop `irqbalance` when pinning by hand. Keep the TX thread and the TX-completion IRQ on the same NUMA node (not necessarily the same core).
- Use the `performance` governor: `sudo cpupower frequency-set -g performance`. In a VM the governor is controlled by the host; ask the CML2 host admin.
- Disable deep C-states if latency spikes appear; disable SMT sibling load on the pinned core.
