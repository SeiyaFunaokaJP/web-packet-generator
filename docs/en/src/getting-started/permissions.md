# Permissions

Status: draft

Sending raw frames is a privileged operation and the requirement differs per OS. At startup and on demand (`wpg doctor`) wpg runs a self-diagnosis and shows OS-specific guidance. Administrators can also restrict allowed interfaces and the maximum rate.

## Linux

The `afpacket` backend needs `CAP_NET_RAW` (and `CAP_NET_ADMIN` is granted with it). Either run as root or grant the capabilities to the binary:

```sh
sudo setcap cap_net_raw,cap_net_admin+eip /path/to/wpg
```

This is the hint `wpg doctor` prints, with the real path of the running binary. If a backend is refused with EPERM/EACCES, the error says:

```text
AF_PACKET needs CAP_NET_RAW: run as root or grant it with `sudo setcap cap_net_raw,cap_net_admin+ep $(command -v wpg)`
```

File capabilities are lost when the binary is replaced and are ignored on `nosuid` mounts; under systemd use `AmbientCapabilities=CAP_NET_RAW CAP_NET_ADMIN`. See [CML2 notes](../operations/cml2.md).

## macOS

Needs read/write access to `/dev/bpf*`. Run with `sudo`, or make the devices accessible (for example with Wireshark's ChmodBPF). The hint printed by `wpg doctor` is `sudo /path/to/wpg …   (or make /dev/bpf* accessible, e.g. via Wireshark's ChmodBPF)`.

## Windows

Needs an Administrator terminal plus Npcap installed in WinPcap API-compatible mode. The `wpg doctor` hint is `Run from an elevated (Administrator) terminal`; a missing Npcap points to `https://npcap.com/#download`. An unelevated process can still use Npcap unless it was installed in administrator-only mode, so the privilege check is only a warning there.

## Checking with `wpg doctor`

`wpg doctor` prints one line per check with a stable id in brackets; `wpg doctor --json` gives the same data for scripts. The exit status is non-zero when a check fails.

| Check id | Status when unmet | Meaning |
|---|---|---|
| `caps.raw_socket` | fail (warn on Windows) | Raw-socket privilege: CAP_NET_RAW / root / BPF access / elevation |
| `iface.any_up` | warn | At least one non-loopback interface is up with carrier |
| `kernel.af_xdp` | warn | Kernel 5.10 or newer for the AF_XDP backend |
| `kernel.tpacket_v3` | warn | Kernel 3.2 or newer; otherwise afpacket falls back to `sendmmsg` |
| `hugepages` | warn | Hugepages configured; only needed for the DPDK backend |
| `npcap` | fail / warn (Windows only) | Npcap installed and in WinPcap-compatible mode; skipped elsewhere |
| `pcap_lib` | warn | libpcap found (needed by the `pcap` backend); skipped on Windows |

Each check reports `ok`, `warn`, `fail` or `skip` (not applicable on this OS).
