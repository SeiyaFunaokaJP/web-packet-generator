# DPDK and AF_XDP setup

Status: draft

> **The `afxdp` backend is implemented (M8) but optional:** it is only available in builds with `--features afxdp` (`cargo build --release -p wpg-cli --features afxdp`). **Copy mode has been verified on `lo` and `veth`; zero-copy has not been verified on real (physical) NICs.** The `dpdk` backend (M8) is optional too (`--features dpdk`) and has only been verified on DPDK virtual devices (`net_null`, `net_ring`, `net_pcap`); physical NICs and CML2's virtio PMD have not been tried yet.

## AF_XDP

### How it works

The `afxdp` backend is a minimal implementation on the kernel ABI (`<linux/if_xdp.h>`, `bpf(2)`) without libbpf or libxdp, so it adds no build dependencies. Each worker (`--workers`) has:

- one AF_XDP socket bound to one NIC queue; worker `i` uses queue `--queue` + `i` (from 0 by default);
- a UMEM (the frame area, at most 16 MiB) whose pages are allocated on the NIC's NUMA node (`mbind`);
- a TX ring and a completion ring. Frames are copied into UMEM chunks, their descriptors are queued on the TX ring, and the kernel is woken with `sendto` only when it asks for it (`XDP_USE_NEED_WAKEUP`).

### Automatic copy / zero-copy (`--xdp-mode`)

| Mode | Behaviour |
|---|---|
| `auto` (default) | try zero-copy first, fall back to copy mode |
| `copy` | always copy mode (`XDP_COPY`); works on any interface including `lo` and `veth` |
| `zerocopy` | zero-copy (`XDP_ZEROCOPY`) only; an error if unavailable |

Many drivers (i40e, ice, ixgbe, igc, ...) only create the XDP rings that zero-copy transmission uses while an XDP program is attached. `auto` and `zerocopy` therefore attach a two-instruction `XDP_PASS` program in driver mode before the zero-copy bind (a `bpf_link`, detached when the backend closes; an XDP program that is already attached is used as is). If the program cannot be loaded, the driver refuses the zero-copy bind, or the first wake-up fails, the backend falls back to copy mode. The backend line of `wpg bench` / `wpg send` shows the outcome (`AF_XDP zero-copy` / `AF_XDP copy mode`, with the reason after a fallback).

> **Caution**: some drivers rebuild their queues when an XDP program is attached or detached, which briefly takes the link down. On a production interface use `--xdp-mode copy`, or check the effect beforehand.

### Requirements

- Linux 5.10 or later (`wpg doctor` check `kernel.af_xdp`); `--busy-poll` needs 5.11.
- Privileges: copy mode needs `CAP_NET_RAW`; zero-copy also `CAP_NET_ADMIN` and `CAP_BPF` (`CAP_SYS_ADMIN` before 5.8).

```sh
sudo setcap cap_net_raw,cap_net_admin,cap_bpf,cap_ipc_lock+ep "$(command -v wpg)"
```

- Before Linux 5.11 the UMEM is charged to `RLIMIT_MEMLOCK`. wpg raises the soft limit to the hard limit; if that is not enough set `ulimit -l unlimited` (`LimitMEMLOCK=infinity` in systemd).
- Only one AF_XDP socket can be bound to a queue. Configure as many channels as queues you use; with too few queues wpg fails with an `ethtool -L` hint.

```sh
ethtool -l ens3               # current and maximum channels
sudo ethtool -L ens3 combined 4
wpg bench --backend afxdp -i ens3 --workers 4 --auto-pin
```

- The maximum frame length is the UMEM chunk size (2048 B when the MTU is 2030 or less, else 4096 B). Longer frames are counted as `dropped` (multi-buffer AF_XDP, needed for jumbo frames, is not used).

### Main options

| Option | Meaning |
|---|---|
| `--backend afxdp` | send with AF_XDP |
| `--xdp-mode auto\|copy\|zerocopy` | bind mode (above) |
| `--queue N` | first queue (worker i uses queue N+i) |
| `--workers N` | number of workers = number of queues used |
| `--auto-pin` / `--pin-cores 2,3` | worker core pinning ([Performance tuning](tuning.md)) |
| `--ring-depth N` | TX ring slots (rounded to a power of two, capped by the 16 MiB UMEM) |
| `--busy-poll` | sets `SO_PREFER_BUSY_POLL` / `SO_BUSY_POLL` so the sending thread runs the driver's NAPI itself (5.11+); `wpg send` also waits in `busy-poll` mode |

### Supported NICs (zero-copy)

AF_XDP copy mode works on every network device; zero-copy needs driver support. The table lists the main mainline Linux drivers with AF_XDP zero-copy and the approximate kernel version that added it. **None of them has been verified with wpg on real hardware.** Distribution kernels may differ because of backports, so always check that `wpg bench` reports zero-copy.

| Driver | NIC | Zero-copy since (approx.) | Verified with wpg |
|---|---|---|---|
| `i40e` | Intel X710 / XL710 / XXV710 | 4.20 | no |
| `ixgbe` | Intel 82599 / X520 / X540 / X550 | 4.20 | no |
| `mlx5_core` | NVIDIA (Mellanox) ConnectX-4 Lx and later | 5.3 | no |
| `ice` | Intel E810 | 5.5 | no |
| `stmmac` | Synopsys DWMAC (embedded SoCs) | 5.13 | no |
| `igc` | Intel I225 / I226 | 5.14 | no |
| `virtio_net` | virtio-net in KVM / CML2 | recent kernels (6.x); copy mode before | no |
| `veth` | containers / netns | none (native XDP yes, sending in copy mode) | **copy mode verified** (4 queues, 2 workers) |
| `lo` | loopback | none (copy mode) | **copy mode verified** |

Verified (2026-10, Linux 6.18 container, root): copy-mode transmission on `lo` and `veth` with the frames arriving at a receiver; the `auto` fallback (on veth the `XDP_PASS` program attaches in driver mode, the zero-copy bind is refused with `EOPNOTSUPP`, and copy mode is used); two workers on two veth queues. Unit tests: `cargo test -p wpg-tx --features afxdp` (the veth test runs only with `WPG_AFXDP_VETH=vxa,vxb`). For performance see `docs/perf.md`.

## DPDK

The `dpdk` backend sends through a DPDK poll-mode driver (PMD). The NIC is taken away from the kernel and driven by wpg's own threads, with no system call per batch. It is optional: build with `--features dpdk` (needs the DPDK development package and libclang, see [Building](#building-the-bindings-dpdk-sys)).

```sh
cargo build --release -p wpg-cli --features dpdk
```

**Verified so far** (2026-10, Ubuntu 24.04 with DPDK 23.11.4, in a container without hugepages or a NIC): EAL with `--no-huge`, the `net_null` (`wpg bench`, 2 workers), `net_ring` and `net_pcap` virtual devices. A `wpg send` of a template through `net_pcap` wrote the same frames as the `pcapfile` backend. The unit tests run in CI against DPDK 24.11 and 23.11. **Physical NICs (`vfio-pci`), hugepages and the virtio PMD on CML2 have not been tried.**

### How it works

- **EAL** (DPDK's environment layer) is started once per wpg process, by the first `dpdk` job, with your EAL arguments plus two defaults you can override: `-l <first CPU wpg may use>` (the main lcore only; wpg's TX workers are its own threads and register with DPDK as non-EAL lcores) and `--in-memory` (no files under `/var/run/dpdk`; `--no-shconf` instead when you pass `--no-huge`). EAL cannot be restarted inside a process: if it fails, or you need other arguments, restart `wpg serve` (`wpg send` / `wpg bench` start a new process every time).
- **The interface is a DPDK port**: its number (`0`), its device name (the PCI address of a bound NIC, `0000:00:04.0`), or a virtual device. Virtual devices may carry devargs and are added on first use: `net_tap0,iface=dtap0` (a kernel TAP interface, handy for testing), `net_pcap0,tx_pcap=/tmp/out.pcap` (writes a pcap file), `net_null0` (discards frames, for benchmarks). An unknown name fails with a list of the ports EAL found.
- **Workers** (`--workers N`) share one port: it is configured with `--queue` + N TX queues, worker `i` sends on TX queue `--queue` + `i`. Only one job can use a port at a time. The port is stopped when the job ends.
- **Buffers**: one mbuf pool per port on the NIC's NUMA socket, sized from `--ring-depth` (TX descriptors per queue, clamped to the driver's limits) and `--batch`. Each frame is copied into an mbuf (2048 octets of data, so longer frames are counted as `dropped`; jumbo frames are not supported). When the TX ring is full the backend retries for up to 10 ms, then counts the rest as `dropped`. `MBUF_FAST_FREE` is turned on when the driver offers it.
- The `backend` line of `wpg send` / `wpg bench` shows the DPDK version, port, driver, queue, descriptors, pool, NUMA socket, link state and the full EAL arguments.

### Host setup

`scripts/deploy.sh <host> --dpdk [--hugepages N]` does the first two steps (hugepages now and at boot, `hugetlbfs`, `vfio-pci`, an IOMMU check) but leaves binding the NIC to you; see [Deploying with scripts/deploy.sh](deploy.md#--dpdk-host-preparation-only).

1. **Hugepages** (`wpg doctor` check `hugepages`, a warning when none). The pool for one port with 4 workers and the default `--ring-depth 4096` takes about 70 MiB, plus what the driver allocates:

   ```sh
   echo 1024 | sudo tee /sys/kernel/mm/hugepages/hugepages-2048kB/nr_hugepages
   grep Huge /proc/meminfo
   ```

2. **IOMMU and vfio**: on bare metal enable the IOMMU (`intel_iommu=on iommu=pt` or `amd_iommu=on` on the kernel command line) and `sudo modprobe vfio-pci`.
3. **Bind the NIC.** The interface disappears from the kernel (and from `wpg ifaces`), so never bind the interface that carries your ssh session:

   ```sh
   dpdk-devbind.py --status                      # PCI addresses and current drivers
   sudo ip link set ens4 down
   sudo dpdk-devbind.py --bind=vfio-pci 0000:00:04.0
   # back to the kernel: sudo dpdk-devbind.py --bind=virtio-pci 0000:00:04.0 (or ixgbe, i40e, ...)
   ```

   NVIDIA / Mellanox ConnectX (`mlx5`) is the exception: it stays on its kernel driver and DPDK uses it through `rdma-core`; do not rebind it.
4. **Privileges**: run as root. vfio as an ordinary user needs `/dev/vfio/*` permissions, `CAP_IPC_LOCK` / a large `RLIMIT_MEMLOCK` and access to the hugepage mount, and is not covered here.

### EAL arguments

`wpg send` / `wpg bench` take them with `--eal-args` (one string, split on spaces); `wpg serve` takes them from `tx.dpdk_eal_args` in `wpg.toml` ([Configuration](../reference/config.md)) or `WPG_TX__DPDK_EAL_ARGS`. The useful ones:

| Argument | Meaning |
|---|---|
| `-a 0000:00:04.0` | Only probe this PCI device (repeatable). Without it, EAL takes every NIC bound to a DPDK driver |
| `-l 2` | Main lcore (default: first CPU wpg may use). Pin the TX workers with `--pin-cores` / `--auto-pin` as usual |
| `--socket-mem 512` / `-m 512` | Hugepage memory to reserve up front (MiB) |
| `--vdev net_tap0,iface=dtap0` | Create a virtual device at start (or name it as the interface, see above) |
| `--file-prefix wpg` | Use shared runtime files instead of `--in-memory` (for `dpdk-proc-info` and the telemetry socket) |
| `--log-level lib.eal:debug` | More EAL logging when something fails |
| `--no-huge -m 256` | Run without hugepages (virtual devices only; physical NICs need hugepages) |

```sh
sudo wpg bench --backend dpdk -i 0000:00:04.0 --eal-args "-a 0000:00:04.0" --workers 2 --auto-pin
sudo wpg send t.wpg.yaml --backend dpdk -i 0000:00:04.0 --eal-args "-a 0000:00:04.0" --rate 1Mpps

# Without a NIC: a TAP interface you can watch with tcpdump (needs root).
sudo wpg send t.wpg.yaml --backend dpdk -i net_tap0,iface=dtap0 --count 10 &
sudo tcpdump -ni dtap0
```

```toml
# wpg.toml
[tx]
default_backend = "dpdk"
allowed_ifaces = ["0000:00:04.0"]
dpdk_eal_args = ["-a", "0000:00:04.0", "-l", "1"]
```

### CML2 and virtio

On a CML2 Ubuntu node the data interfaces are virtio-net devices, which DPDK drives with the virtio PMD (`net_virtio`, part of the standard DPDK packages):

- The guest normally has no IOMMU, so `vfio-pci` refuses to bind. Use vfio's no-IOMMU mode (`echo 1 | sudo tee /sys/module/vfio/parameters/enable_unsafe_noiommu_mode` before binding; "unsafe" because DMA is not isolated, acceptable in a lab VM) or `uio_pci_generic`.
- Keep the management interface (usually `ens2`) on the kernel; bind only a data interface.
- The virtio PMD usually offers as many TX queues as the node has vCPUs / virtio queue pairs; `--workers` above that fails with the queue count in the message.
- Throughput is bounded by the host's vhost path and the virtual link, not by DPDK. Compare with `afpacket` on the same link before relying on DPDK; on CML2 the gain is often small.
- The NIC's MAC address does not matter for sending: wpg sends the source MAC in the template. Promiscuous mode is not needed to transmit.

### Limits

- Transmit only (no DPDK receive in `wpg capture`).
- Frames up to 2048 octets; no multi-segment mbufs, no TX checksum or segmentation offloads (wpg builds complete frames).
- One job per port at a time; EAL arguments are fixed for the life of the process.
- Shared DPDK libraries only (PMDs are loaded as plugins).

Expected performance (TODO section 2.2, not measured on hardware): AF_XDP about several Mpps per core, DPDK 10 Mpps or more per core on a physical NIC. Record your numbers with the checklist in `docs/perf.md`.

### Building the bindings (`dpdk-sys`)

The bindings live in the `dpdk-sys` crate (ADR-0004). With its `link` feature (turned on by `wpg-tx`'s `dpdk` feature) the build script finds DPDK with pkg-config (`libdpdk`, 23.11 or newer; other versions than 24.11 / 23.11 build with a warning), generates the bindings with bindgen (EAL, ethdev, mempool / mbuf, lcore, the `RTE_*` constants) and compiles a small C shim with DPDK's own cflags for the parts of the fast path that are `static inline` in the headers (`wpg_rte_eth_tx_burst`, `wpg_rte_pktmbuf_alloc`, `wpg_rte_errno`, ...). Without the feature the crate is empty, so ordinary builds need no DPDK.

```sh
# Debian 13 (DPDK 24.11) / Ubuntu 24.04 (DPDK 23.11)
sudo apt install dpdk-dev libclang-dev pkg-config
cargo test -p dpdk-sys --features link     # bindings + EAL smoke test on a net_ring port
cargo build -p wpg-tx --features dpdk      # wpg-tx with the bindings linked
```

For DPDK built from source (meson / ninja), point pkg-config at it and make the shared libraries findable:

```sh
export PKG_CONFIG_PATH=/opt/dpdk-24.11/lib/x86_64-linux-gnu/pkgconfig
export LD_LIBRARY_PATH=/opt/dpdk-24.11/lib/x86_64-linux-gnu
```

DPDK 24.11 refuses to load PMD plugins from a directory with a world-writable parent (such as `/tmp`), so install it under `/opt` or `/usr/local`. The smoke test runs EAL with `--no-huge --no-pci` on a `net_ring` virtual port, so it needs neither hugepages, a NIC nor root; CI runs it against both LTS releases (job `dpdk`). Only DPDK's shared libraries are supported (PMDs are loaded as plugins). The backend's own tests (`cargo test -p wpg-tx --features dpdk --lib dpdk`) also run without hugepages, a NIC or root, on `net_ring` / `net_pcap` / `net_null` devices.
