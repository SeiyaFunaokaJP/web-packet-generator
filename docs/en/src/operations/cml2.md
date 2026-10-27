# CML2 notes

Status: draft

The first deployment target is an Ubuntu node in Cisco CML2 with a virtio-net interface, where the `afpacket` backend is the main path. This page covers what to set up and what to watch for.

> **Verification status.** Everything about wpg's own behaviour (doctor checks, error messages, flags) is taken from the code and from `wpg` run in a container. **No measurement has been made on CML2 yet.** All performance numbers in `docs/perf.md` come from the `lo` interface and the `null` backend, so they only bound the afpacket path from above. The CML2-specific statements below (bridge behaviour, MAC filtering) are general Linux/KVM knowledge to be confirmed on your CML2 version; the M2 on-CML2 measurement is still open.

## Prerequisites

- An Ubuntu node (22.04 or later, kernel 5.15+) with at least one connected virtio-net interface. `wpg doctor` requires kernel 3.2 or newer for TPACKET_V3 and 5.10 or newer for AF_XDP.
- The `wpg` binary built with `--features pcap` if you also want the `pcap` backend (needs `libpcap0.8`). `afpacket` needs no library.
- Use two or more vCPUs. Sending is one busy thread; leave a core for the OS.

## Building the lab

The smallest setup is one Ubuntu node running wpg and the node under test. To receive and verify as well ([Receive and verification](rx-verify.md)), add a second Ubuntu node on the far side.

```text
External Connector (NAT / bridge) ── ens2 [Ubuntu: wpg] ens3 ── device under test (IOSv, CSR1000v, XRv9k, FRR, ...) ── ens3 [Ubuntu: receiver (optional)]
```

1. **Management link**: connect the Ubuntu node's first interface (`ens2`) to an External Connector. Use it only for copying the binary (`scripts/deploy.sh`), ssh and the web UI, never for test traffic.
2. **Data links**: connect the next interfaces (`ens3`, `ens4`, ...) to the device under test. An interface has no carrier until the nodes on both ends of the link are started.
3. **Node size**: 2 or more vCPUs (N + 1 or more for `--workers N`) and 2 GiB of memory or more. Add the hugepages for DPDK (2 GiB for the default 1024 pages).
4. **Initial configuration**: in the Ubuntu node's Config (cloud-init user-data), add your ssh key and, if you want them, `ethtool`, `tcpdump` and `libpcap0.8`:

   ```yaml
   #cloud-config
   ssh_authorized_keys:
     - ssh-ed25519 AAAA... you@workstation
   packages: [ethtool, tcpdump]
   ```

5. **Deploy**: run `scripts/deploy.sh` from your workstation against the management address ([Deploying with `scripts/deploy.sh`](deploy.md)). Restrict sending to the data interfaces with `--iface`:

   ```sh
   scripts/deploy.sh ubuntu@192.0.2.10 --service --iface ens3 --password-file ./wpg-password
   ```

6. **Web UI**: either set a password and listen on `0.0.0.0:8080`, or install the service with `--bind 127.0.0.1:8080` and open it through an ssh tunnel (`ssh -L 8080:127.0.0.1:8080 ubuntu@192.0.2.10`) ([Authentication and TLS](security.md)).

## Privileges: setcap or root

AF_PACKET sockets need `CAP_NET_RAW`. Either run as root, or grant the capabilities to the binary once:

```sh
sudo setcap cap_net_raw,cap_net_admin+eip /usr/local/bin/wpg
```

This is the hint `wpg doctor` prints (with the real path of the running binary). When the backend itself is refused it prints: ``AF_PACKET needs CAP_NET_RAW: run as root or grant it with `sudo setcap cap_net_raw,cap_net_admin+ep $(command -v wpg)` ``. The capability is lost when the file is replaced, so re-run `setcap` after every copy (`scripts/deploy.sh` does this). For systemd, use `AmbientCapabilities=CAP_NET_RAW CAP_NET_ADMIN` instead of file capabilities. Capabilities are ignored on filesystems mounted `nosuid`.

## Choosing the interface

```console
$ wpg ifaces
NAME  IDX  MAC                IPV4       IPV6  MTU    STATE         SPEED  DRIVER
lo    1    -                  127.0.0.1  -     65536  up, loopback  -      -
ifb0  2    66:00:97:e0:50:14  -          -     1500   down          -      -
ifb1  3    62:7f:e1:7b:e9:80  -          -     1500   down          -      -
eth0  4    02:fc:00:00:00:01  192.0.2.2  -     1400   up            -      virtio_net
```

(The listing above is from a development container, not CML2.) Pick the interface that is `up`, has driver `virtio_net`, and is wired to the link you want to test. Do not send on the management interface that carries your ssh session. Interface names on CML2 Ubuntu nodes are usually `ens2`, `ens3` and so on, one per node interface in the order of the CML2 topology; confirm with the MAC shown in the CML2 UI. Note the MTU column, it decides the maximum frame size (see below). An interface must be `up` with carrier; an unconnected CML2 interface stays without carrier.

## Walkthrough with `wpg doctor`

```console
$ wpg doctor
wpg doctor: environment check

✓ Raw sockets are available  [caps.raw_socket]
    euid/elevated=true CAP_NET_RAW=yes CAP_NET_ADMIN=yes bpf_writable=n/a
✓ At least one usable network interface is up  [iface.any_up]
    eth0
✓ Kernel supports AF_XDP (5.10 or newer)  [kernel.af_xdp]
    kernel 6.18.44-fc-v49 (need >= 5.10.0)
✓ Kernel supports TPACKET_V3  [kernel.tpacket_v3]
    kernel 6.18.44-fc-v49 (need >= 3.2.0)
! No hugepages configured (only needed for the DPDK backend)  [hugepages]
    0 x 2048 kB (0 free), 0 x 1048576 kB (0 free)
    → Reserve hugepages and bind the NIC to DPDK: echo 1024 | sudo tee /sys/kernel/mm/hugepages/hugepages-2048kB/nr_hugepages ; sudo dpdk-devbind.py --status (then: --bind=vfio-pci <PCI address>)
- Not applicable on this operating system  [npcap]
- Not applicable on this operating system  [backend.pcap]
✓ libpcap is available  [pcap_lib]
    /usr/lib/x86_64-linux-gnu/libpcap.so

5 ok, 1 warning(s), 0 failure(s)
```

For afpacket only `caps.raw_socket` (fail if missing), `iface.any_up` and `kernel.tpacket_v3` matter. The `hugepages` warning is expected unless you use DPDK. `wpg doctor --json` gives the same list for scripts. Then run a smoke test that leaves the host untouched:

```sh
wpg send tpl.wpg.yaml --backend pcapfile -i /tmp/out.pcapng   # dry run: writes a pcapng, sends nothing
wpg bench --backend afpacket -i eth0 --duration 5       # unlimited 64-byte frames on the wire!
wpg send tpl.wpg.yaml -i eth0 --backend afpacket --rate 10kpps --count 1000
```

Start with a low `--rate`; `wpg bench` sends at line rate into the CML2 link.

## Promiscuous mode and MAC restrictions

wpg only transmits, and its afpacket socket is created with protocol 0, so it receives nothing and **does not need promiscuous mode** on the sending node. The restrictions are on the path:

- **Internal links between CML2 nodes** are virtual links implemented by the CML2 host. Frames whose source MAC is not the one the node interface was created with may be filtered or the link may learn it inconsistently. If you generate frames with spoofed source MACs (ARP sweeps, DHCP load with many client MACs) and the peer never sees them, test first with the interface's real MAC as source, then consider that the CML2 link may be dropping the rest. This has to be confirmed on your CML2 version.
- **External connector** (bridge to the CML2 host network or to a physical NIC): the host bridge and any upstream switch may apply port security or MAC learning limits. High-rate multi-MAC traffic can flood the CAM table of a physical switch; use only on an isolated segment.
- The receiving side (a CML2 router, or a second Ubuntu node running `tcpdump`) needs promiscuous mode only if the destination MAC is not its own: `sudo ip link set ens3 promisc on`.
- Use of wpg is limited to lab networks you are authorised to test; see the [legal notice](../legal.md).

## MTU and jumbo frames

The afpacket ring sizes its slots from the interface MTU (capped at 9216); frames larger than a slot go out through a second plain socket. The frame length you send is Ethernet header plus payload without FCS, so a 1514-byte frame needs MTU 1500. The example above shows an MTU of 1400 on a virtio interface: a 1514-byte frame will then be rejected by the kernel (`EMSGSIZE`) and counted as `dropped`. Check `wpg ifaces`, keep template frames at MTU + 14, or raise the MTU on both ends (`sudo ip link set ens3 mtu 9000`). Jumbo frames also need every CML2 link and the peer node to allow them, which depends on the CML2 version and node type.

## Expected afpacket performance on virtio

Unmeasured on CML2. What we have is the `lo` baseline in `docs/perf.md`: about 1.4–1.9 Mpps for 64-byte frames from one thread, bounded by the loopback receive path. On virtio-net the limit is the guest-to-host path: the virtio TX queue, the vhost thread on the CML2 host and the virtual link, so expect a value below the `lo` figure and strongly dependent on the host. Tuning switches worth trying, in order: `--qdisc-bypass`, larger `--batch`, `--pin-core`, and comparing `--no-tx-ring`. Record your results using the checklist in `docs/perf.md`.

## `--qdisc-bypass`

`--qdisc-bypass` sets `PACKET_QDISC_BYPASS` so frames skip the qdisc layer. On `lo` it improved 64-byte throughput by about 30%. The consequences: no tc shaping, priority or `netem` on that interface applies to the traffic, packets are not counted by the qdisc statistics (`tc -s qdisc`), and if the device queue is full the frame is dropped (the kernel returns `ENOBUFS`) instead of waiting in the qdisc. wpg retries for a short time and then counts the frames as `dropped`. Leave it off when you rely on `netem` for impairments.

## Receiving side: GRO, TSO and checksum offload

Offloads change what the receiver *shows*, not what wpg sent. With GRO or LRO enabled, `tcpdump` on a Linux receiver can display merged super-frames larger than the MTU for TCP streams; disable it for accurate captures: `sudo ethtool -K ens3 gro off lro off`. Checksum offload on the sending node does not matter for wpg, because wpg builds the full frame including checksums itself (bad checksums you asked for are sent as is). On virtio guests, captures on the sender may show "incorrect checksum" only for packets the kernel's own stack produced, not for wpg frames. Use `ethtool -k` to see the current settings.

## Choosing a backend on CML2

| Backend | When to use it on CML2 (virtio-net) | Preparation |
|---|---|---|
| `afpacket` | **The default; start here.** Needs nothing extra and coexists with the management interface | `CAP_NET_RAW` (above) |
| `afxdp` | Worth comparing when `afpacket` tops out. virtio-net zero-copy needs a recent kernel; mostly you get copy mode, and the gap to `afpacket` is small | built with `--features afxdp`, kernel 5.10+, enough queues (`ethtool -L`) ([AF_XDP](dpdk-afxdp.md#af_xdp)) |
| `dpdk` | Takes the interface away from the kernel with the virtio PMD. The host's vhost path is the limit, so the gain depends on the host | built with `--features dpdk`, hugepages, vfio no-IOMMU mode or `uio_pci_generic` ([CML2 and virtio](dpdk-afxdp.md#cml2-and-virtio)) |
| `pcap` | Only to follow the same procedure as on other OSes | built with `--features pcap`, `libpcap0.8` |

With any backend, check the frames with `--backend pcapfile` first and start with a low `--rate`. To compare performance, run `wpg bench` with each backend on the same link ([Performance tuning](tuning.md)) and record the results with the checklist in `docs/perf.md`.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `AF_PACKET needs CAP_NET_RAW ...` / EPERM / `caps.raw_socket` fails | No privilege | Run as root or `sudo setcap cap_net_raw,cap_net_admin+eip $(command -v wpg)`; re-run after replacing the binary; not on `nosuid` mounts |
| ENOBUFS or a non-zero `dropped` counter | TX queue full: the virtual link or vhost is slower than the send rate; frames retried ~100 ms then dropped | Lower `--rate`, reduce `--batch`, try without `--qdisc-bypass`, check the peer and the link speed; compare with `--no-tx-ring` |
| `dropped` equals the frames of one size class | Frame larger than MTU (`EMSGSIZE`) | Check `wpg ifaces` MTU, shrink the frame or raise MTU on both ends |
| "interface not found" / `ENODEV` | Wrong name or interface renamed | `wpg ifaces`; names differ between `eth0` and `ens3` style |
| `iface.any_up` warns "none up with carrier" | Interface down, or CML2 link not connected/started | `sudo ip link set ens3 up`; start the node and check the link in the CML2 topology |
| Sender counts packets but the peer sees none | Wrong link, MAC filtering on the CML2 link, VLAN mismatch, receiver not in promisc | Capture on the sending node interface first, then on the peer; try the real source MAC; `ip link set ... promisc on` |
| Throughput far below the `lo` baseline | virtio/vhost limits, one vCPU shared with other work | Give the node 2+ vCPUs, `--pin-core`, see [Performance tuning](tuning.md) |
| Rate lower than requested | Above what the path sustains; see rate accuracy in `wpg send` output | Lower the rate; see [Performance tuning](tuning.md) |
