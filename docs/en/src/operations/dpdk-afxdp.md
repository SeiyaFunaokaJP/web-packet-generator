# DPDK and AF_XDP setup

Status: draft

> **The `afxdp` backend is implemented (M8) but optional:** it is only available in builds with `--features afxdp` (`cargo build --release -p wpg-cli --features afxdp`). **Copy mode has been verified on `lo` and `veth`; zero-copy has not been verified on real (physical) NICs.** The `dpdk` backend is not implemented yet (it fails with "not implemented yet"); the DPDK section records the intended setup and has not been verified with wpg.

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

`scripts/deploy.sh <host> --dpdk [--hugepages N]` does the host-side part below (hugepages now and at boot, `hugetlbfs`, `vfio-pci`, an IOMMU check) but leaves binding the NIC to you; see [Deploying with scripts/deploy.sh](deploy.md#--dpdk-host-preparation-only).

Requirements (from the design decision in ADR 0004):

- DPDK **24.11 LTS** (23.11 LTS is also checked in CI). The DPDK development package and libclang must be installed at build time (see [Building the bindings](#building-the-bindings-dpdk-sys)).
- Hugepages (`wpg doctor` check `hugepages`, warning when none):

```sh
echo 1024 | sudo tee /sys/kernel/mm/hugepages/hugepages-2048kB/nr_hugepages
grep Huge /proc/meminfo
```

- Bind the NIC to a DPDK-capable driver. The NIC is taken away from the kernel, so keep a separate management interface:

```sh
dpdk-devbind.py --status
sudo modprobe vfio-pci        # or uio_pci_generic
sudo dpdk-devbind.py --bind=vfio-pci 0000:00:04.0
```

- EAL arguments (cores, memory channels, allow-list) come from the configuration file; see [Configuration](../reference/config.md).
- **CML2**: the Ubuntu node exposes virtio-net devices, which DPDK drives with the virtio PMD. Without an IOMMU in the guest use `uio_pci_generic` or vfio in no-IOMMU mode (`enable_unsafe_noiommu_mode`). Performance is bounded by the host-side vhost path, not by DPDK.

Expected performance (TODO section 2.2, not measured): AF_XDP about several Mpps per core, DPDK 10 Mpps or more per core on a physical NIC. AF_XDP copy-mode figures measured on `lo` / `veth` are in `docs/perf.md`.

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

DPDK 24.11 refuses to load PMD plugins from a directory with a world-writable parent (such as `/tmp`), so install it under `/opt` or `/usr/local`. The smoke test runs EAL with `--no-huge --no-pci` on a `net_ring` virtual port, so it needs neither hugepages, a NIC nor root; CI runs it against both LTS releases (job `dpdk`). Only DPDK's shared libraries are supported (PMDs are loaded as plugins). The `dpdk` transmit backend itself is still to come: `wpg` does not have a `dpdk` build feature yet.
