# DPDK and AF_XDP setup

Status: draft

AF_XDP requires Linux kernel 5.10 or later and falls back from zero-copy to copy mode when the NIC driver lacks support. DPDK is enabled by building with `--features dpdk` against DPDK 24.11 LTS (23.11 LTS is also checked in CI). It needs hugepages, NIC binding with dpdk-devbind and EAL arguments from the config file. On CML2 the virtio-net PMD is used.
