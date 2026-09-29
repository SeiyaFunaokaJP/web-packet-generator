# Tutorial: Load and rate control

Time: about 20 minutes. Privileges: none (the `null` and `pcapfile` backends are used throughout).

> This page replaces the planned "DHCP load" tutorial. DHCP is a P1 protocol that is not implemented yet; the skills below (mass generation, rate, duration, ramp-up, workers) are the ones that tutorial would have used, and they work with any template.

## What you will build

Commands that push a template at a chosen rate for a chosen amount of time, read the summary that comes back, and stay inside the limits of a lab. The example templates are the MAC flood (`templates/scenarios/mac-flood.wpg.yaml`, 100,000 frames with random source MACs) and the SYN flood (`templates/scenarios/vlan-syn-flood.wpg.yaml`).

## Before you start

Build wpg and run from the repository root. Real transmission needs privileges and a lab interface; this tutorial uses `--backend null` (frames are built and counted but not sent), so you can run every command anywhere. Replace `--backend null -i lo` with `-i eth1` to send for real.

Only load devices you own or are authorised to test.

## Step 1. Count, duration and rate

The stopping condition and the speed come from the template's `tx:` block and can be overridden on the command line:

| Option | Meaning | Examples |
|---|---|---|
| `--count N` | stop after N frames (the stream wraps around if N is longer than the stream) | `--count 50000` |
| `--duration T` | stop after T seconds/minutes | `--duration 10s`, `--duration 2m` |
| `--rate R` | target rate | `250kpps`, `1Mpps`, `1Gbps`, `50%`, `ifg=10us`, `unlimited` |
| `--burst B` | frames sent back to back per rate-control step | `--burst 32` |

If both count and duration are set, whichever comes first ends the run. Rates in `bps` count Layer-1 bytes (preamble, FCS and inter-frame gap included, as on the wire); `%` needs a real interface with a known link speed (it fails on `lo`).

```console
$ wpg send templates/scenarios/udp-qos-matrix.wpg.yaml --backend null -i lo --rate 250kpps --count 50000
Sending on lo via null: rate 250000pps, burst 1, 50000 packets, 1 worker(s), plan PreRender (...)
state      finished (count reached)
backend    null
packets    50000
bytes      10100000
dropped    0
elapsed    0.200 s
average    250.00 kpps  404.00 Mbps (L2)  452.00 Mbps (L1)
accuracy   target 250.00 kpps  achieved 100.00%  max jitter 454.12µs  mean jitter 202ns
```

(The numbers on your machine will differ.) Read the last two lines first: `average` is what was actually sent, and `accuracy` tells you how close the pacing got to the target. A large `dropped` means the backend could not keep up.

## Step 2. Ramp up

Devices that fall over on the first burst are easier to characterise if the rate climbs. `--ramp-up` increases the rate linearly from zero to the target over the given time:

```console
$ wpg send templates/scenarios/vlan-syn-flood.wpg.yaml --backend null -i lo --rate 20kpps --ramp-up 1s --duration 3s
[    3.0 s]        50023 pkts  16.67 kpps  9.34 Mbps  dropped 0  rate 100.0%
state      finished (duration elapsed)
packets    50023
elapsed    3.000 s
average    16.67 kpps  9.34 Mbps (L2)  12.54 Mbps (L1)
accuracy   target 20.00 kpps  achieved 100.04%  max jitter 3.196862ms  mean jitter 115.686µs
```

The average (16.67 kpps) is below the 20 kpps target because the first second was the ramp.

## Step 3. Start at a chosen time

`--start-at` waits before the first frame: a delay (`+10s`) or an RFC 3339 time (`2026-09-29T12:00:00Z`). It is handy for starting two senders together or for giving yourself time to start a capture:

```console
$ wpg send templates/scenarios/vlan-syn-flood.wpg.yaml --backend null -i lo --start-at +2s --count 10
Scheduled start at +2s (in 2.0 s); Ctrl-C cancels.
```

## Step 4. Use several workers

A single worker is limited by one CPU core. `--workers N` starts N transmit threads, each with its own backend instance and its own share of the stream and of the rate; `--pin-cores 2,3` pins them to cores (Linux):

```console
$ wpg send templates/scenarios/vlan-syn-flood.wpg.yaml --backend null -i lo --rate unlimited --count 100000 --workers 2
average    10.35 Mpps  5.79 Gbps (L2)  7.78 Gbps (L1)
worker 0     50000 pkts  3500000 bytes
worker 1     50000 pkts  3500000 bytes
```

`unlimited` on the `null` backend measures how fast frames can be built; it is not a network figure. For the `afpacket` backend also see `--batch`, `--ring-depth`, `--qdisc-bypass` and `--no-tx-ring` in `wpg send --help`, and [Performance tuning](../operations/tuning.md).

## Step 5. Pre-render or generate on the fly

`--plan auto` (default) pre-renders the stream into memory when it is small (256 MiB threshold) so the transmit loop only copies bytes, and expands it on the fly when it is larger. Force one with `--plan prerender` or `--plan onthefly`. Random generators take their values from `stream.seed`, so set one when you need a reproducible stream.

## Step 6. Machine-readable results

`--json` prints the final report as JSON on standard output (progress and banners go to standard error), for CI jobs and scripts:

```console
$ wpg send templates/scenarios/mac-flood.wpg.yaml --backend null -i lo --count 100000 --json | jq '{state: .state.state, packets: .backend.packets, dropped: .backend.dropped}'
```

The exit status is non-zero when the run failed (for example a refused backend), so `wpg send` can be a step in a pipeline.

## Step 7. Keep it inside limits

* Set an absolute upper bound for everyone with `[tx] max_rate` in `wpg.toml` (for example `max_rate = "100kpps"`) and restrict interfaces with `allowed_ifaces`; see [Configuration](../reference/config.md).
* `Ctrl-C` cancels a run.
* Use a rehearsal backend (`null`, or `pcapfile` with `-i out.pcapng`) before every new template.

## Next steps

* [pcap to template replay](pcap-replay.md) to load a device with traffic recorded from a real network.
* [Performance tuning](../operations/tuning.md) for the per-backend knobs.
