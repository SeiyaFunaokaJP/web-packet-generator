# DPDK and AF_XDP setup

Status: draft

> **The `afxdp` and `dpdk` backends are not implemented yet (milestone M8).** `--backend afxdp` and `--backend dpdk` are listed in the CLI help but currently fail with "not implemented yet (planned for M8)". This page records the intended setup so an environment can be prepared in advance. Use the `afpacket` backend until then. Nothing on this page has been verified with wpg.

## AF_XDP

Requirements:

- Linux kernel 5.10 or later (`wpg doctor` check `kernel.af_xdp`). Zero-copy needs a driver that supports it; otherwise wpg will fall back to copy mode automatically.
- Drivers with AF_XDP zero-copy support include `i40e`, `ice`, `ixgbe`, `mlx5` and `igc`; `virtio_net` supports XDP but AF_XDP runs in copy mode there. Check the kernel documentation for your version.
- Root or `CAP_NET_RAW`, `CAP_NET_ADMIN` and `CAP_BPF` (the exact set is decided when the backend is written); raise the memlock limit (`ulimit -l unlimited`, or `LimitMEMLOCK=infinity` in systemd).
- One AF_XDP socket is bound to one RX/TX queue pair. Set the number of queues you want to use, and pin threads accordingly:

```sh
ethtool -l ens3               # show current and maximum channels
sudo ethtool -L ens3 combined 4
```

## DPDK

`scripts/deploy.sh <host> --dpdk [--hugepages N]` does the host-side part below (hugepages now and at boot, `hugetlbfs`, `vfio-pci`, an IOMMU check) but leaves binding the NIC to you; see [Deploying with scripts/deploy.sh](deploy.md#--dpdk-host-preparation-only).

Requirements (from the design decision in ADR 0004):

- DPDK **24.11 LTS** (23.11 LTS is also checked in CI). Build wpg with `--features dpdk`; the DPDK development packages must be installed.
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

Expected performance (TODO section 2.2, not measured): AF_XDP about several Mpps per core, DPDK 10 Mpps or more per core on a physical NIC.
