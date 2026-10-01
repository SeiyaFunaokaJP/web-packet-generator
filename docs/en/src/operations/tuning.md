# Performance tuning

Status: draft

Measure first with `wpg bench` and compare against the baseline in `docs/perf.md` (measured on `lo` and `null` only). All flags below exist on both `wpg send` and `wpg bench` unless noted; see the [CLI reference](../reference/cli.md).

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

## Rate accuracy and the spin threshold

The rate limiter is a token bucket that waits in a hybrid manner: it `sleep`s while the next deadline is far away and busy-spins for the last 50 microseconds (the default spin threshold). Pure `sleep` has a jitter of the kernel timer slack (typically 50-100 microseconds), which ruins precise spacing at high pps; pure spin burns a full core. When the interval between frames is shorter than the wait resolution, the limiter grants several frames at once (`--burst`), so the *average* rate follows the target while individual spacing does not. `ifg=<duration>` forces a burst of 1. If the achieved rate is lower than requested, the path is the limit: a catch-up burst is capped (10 ms) so it never floods after a stall. The final report of `wpg send` includes the achieved-rate accuracy.

## IRQ affinity and CPU governor

- Spread or pin NIC interrupts: `cat /proc/interrupts | grep <if>`, `echo <mask> | sudo tee /proc/irq/<n>/smp_affinity`; stop `irqbalance` when pinning by hand. Keep the TX thread and the TX-completion IRQ on the same NUMA node (not necessarily the same core).
- Use the `performance` governor: `sudo cpupower frequency-set -g performance`. In a VM the governor is controlled by the host; ask the CML2 host admin.
- Disable deep C-states if latency spikes appear; disable SMT sibling load on the pinned core.

## Multiple queues

Multi-queue sending (`--workers`) is planned; check `wpg bench --help` and `wpg send --help` for the current flags. Until then run one instance per queue and pin each to a different core.
