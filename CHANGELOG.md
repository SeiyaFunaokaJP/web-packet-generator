# Changelog

All notable changes to wpg are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/). Entries are written in English; the milestone numbers
(M0 to M12) refer to [ROADMAP.md](ROADMAP.md).

When you cut a release, move the entries under **Unreleased** into a new version section, add the upgrade
notes (template schema, database, configuration; see the "Upgrading" page of the manual) and follow the
release procedure in the developer guide.

## [Unreleased]

Everything below is the initial development towards 1.0. No version has been tagged yet.

### Added

#### Packet engine and protocols

- Protocol definition IR in `wpg-core`, written with Rust macros or YAML, driving build, parse, constraint
  checks, the GUI field editors and the generated documentation (M1).
- Automatic lengths, checksums and next-header fields with Auto / Manual per field, and deliberate
  malformed packets (wrong checksums and lengths, truncation, bad FCS) (M1).
- User-defined protocols (`protocols.d/`, spec §2.1 P3): YAML files with new protocol definitions and
  bindings under existing protocols (`bind: [{under: udp, value: 47000, protocol: ...}]`), loaded by
  `[protocols] dirs` in `wpg.toml`, the global `--protocols-dir` option or `WPG_PROTOCOLS_DIR`, served to the
  web UI by `GET /api/protocols/extensions`, with an example in `examples/protocols.d/`.
- IR building blocks for demultiplexing by peeked nibble, remaining length and source port, list counts,
  NUL-terminated strings, checksum exclusions, TLV fill and head octets, alignment padding, and lengths
  counted from fields or offsets (M6, M7).
- P0 protocols: Ethernet II, 802.3 with LLC / SNAP, 802.1Q, QinQ, ARP, IPv4 with options, IPv6 with
  extension headers, ICMPv4, ICMPv6 with NDP, TCP with options, UDP and payload layers (M1).
- P1 protocols: LLDP, CDP, STP / RSTP / MSTP, LACP, EAPOL, PPPoE, PPP; VXLAN, GRE, NVGRE, MPLS,
  MPLS-in-UDP, L2TP, GTP-U, Geneve, IP-in-IP; OSPFv2 / v3, BGP, RIP, RIPng, EIGRP; VRRP, HSRP, GLBP;
  IGMP, MLD, PIM; DHCPv4 / v6, DNS, NTP, SNMP, Syslog, RADIUS, TFTP (M6).
- P2 protocols: OSI encapsulation over LLC `0xfe` and PPP with the NSAP type; IS-IS (Hellos, LSP, CSNP,
  PSNP, main TLVs and sub-TLVs, Fletcher checksum, Hello padding); CLNP and ES-IS; SCTP with CRC32c;
  OSPF opaque LSAs and segment routing, BGP multiprotocol, VPN and EVPN NLRI; LDP, RSVP-TE, BFD and
  Micro-BFD; SRv6 SRH and SR-MPLS; PTP, ESMC, Ethernet CFM / Y.1731, ERPS R-APS and 802.3ah OAM;
  MACsec, GTPv2-C, Diameter, SIP and RTP / RTCP (M7).
- Golden byte tests and `tshark -V` cross-checks for the protocols, and `proptest` round trips (M1, M6, M7).
- Field generators (range, list, random, random MAC, counter, file, expr) combined by zip, product or
  nested order, and a stream expander that pre-renders or builds on the fly (M1).

#### Transmit and receive

- `TxBackend` with the `null`, `pcapfile`, `pcap` (libpcap / Npcap / BPF), `afpacket` (TPACKET_V3 ring and
  `sendmmsg`), `afxdp` (copy / zero-copy, optional feature) and `dpdk` (DPDK 24.11 / 23.11 LTS through the
  `dpdk-sys` bindings, optional feature) backends (M2, M8).
- Rate control in pps, bps, line-rate percent and inter-frame gap, bursts, ramp-up, scheduled start,
  TSC clock, busy-poll waiting and accuracy reporting (M2, M8).
- Multi-queue, multi-core sending with `--workers`, core pinning and NUMA-aware placement (`--auto-pin`) (M8).
- `wpg bench`, the performance baseline in `docs/perf.md` and a nightly benchmark job with thresholds (M2, M8).
- Receive counters with a tcpdump-style filter evaluated in user space, `wpg capture`, the first frames
  and pcapng output, and send-signature verification for loss, ordering, duplicates and latency (M9).

#### Server, CLI and GUI

- `wpg serve`: axum server with embedded assets, REST API with OpenAPI and Swagger UI, WebSocket
  statistics and logs, argon2id password authentication, sessions and rate limiting, localhost-only
  mode without a password, TLS with a generated self-signed certificate, `wpg.toml` with environment
  and CLI overrides, a job manager and transmit limits (allowed interfaces, maximum rate) (M3).
- CLI subcommands `serve`, `send`, `render`, `import`, `template`, `ifaces`, `doctor`, `bench`,
  `capture` and `config` (M3, M9, M10).
- Leptos web UI: packet builder with drag-and-drop layers, typed field editors, Auto / Manual toggles,
  linked hex and tree views with hex editing, in-browser WASM preview checked against the server,
  generator settings with count and duration estimates, jobs with live charts, template library,
  receive page, settings, undo / redo, keyboard operation and Japanese / English UI (M4, M9).
- Playwright end-to-end tests of the UI (M4, M9).
- Field descriptions of the P0 protocols (Ethernet II, 802.3, LLC, SNAP, 802.1Q / 802.1ad, IPv4, IPv6 and
  its extension headers, ICMPv4 / ICMPv6, NDP, TCP, UDP, payload) in all eight languages; the packet
  builder shows every field's description as a tooltip on the field name, and the generated protocol
  reference lists them (M11).
- UI, CLI, API and error messages in Simplified Chinese, Traditional Chinese, Korean, German, French and
  Spanish besides Japanese and English, picked from the browser language, `Accept-Language` or `LANG`
  (Chinese by script and region), with a language menu in the header; the manual and the bundled
  template descriptions stay Japanese / English, falling back to English.

#### Templates

- YAML / JSON template format (`schema: 1`) with a published JSON Schema, `wpg template lint`, variables
  overridable at run time, `.wpgbundle` export / import, multiple library paths with hot reload, and
  pcap / pcapng and hex-dump import (M5).
- Bundled library: samples for every protocol, scenarios (ARP sweeps, SYN flood, port scan, QinQ, NDP and
  RA, ping and TTL sweeps, QoS matrix, MTU probe, frame-size sweep, MAC flood, SR-MPLS sweep, DHCP
  DISCOVER load), interop checks against FRR and real routers, and malformed packets (M5 to M11).
- `wpg template migrate` and the schema migration mechanism (M10).

#### Deployment and operations

- `scripts/build.sh` / `build.ps1`, `scripts/deploy.sh` (copy, `setcap`, systemd, health check,
  rollback, `--dpdk` host preparation), release archives for Linux, macOS and Windows built by
  `scripts/release.sh` and the release workflow, Dockerfile and compose file, systemd, launchd and
  Windows service samples (M0, M10).
- `wpg doctor` environment checks (privileges, kernel features, Npcap, libpcap, hugepages) (M2). On
  Windows the privilege check follows Npcap's administrator-only setting: ok when unelevated with the
  default install, a failure in administrator-only mode (M10).
- The macOS and Windows release archives include the libpcap / Npcap backend, the only way to send on
  those systems. `wpg.exe` delay-loads `wpcap.dll` and finds it in `System32\Npcap` itself, so it starts
  without Npcap, works without the WinPcap-compatible mode, and reports a missing Npcap instead of
  crashing. `scripts/build.sh` adds the backend on macOS, `scripts/build.ps1` when the Npcap SDK is found,
  and `scripts/release.sh` fetches a pinned Npcap SDK for Windows targets (M10).
- `scripts/smoke-archive.sh` (`just smoke-archive`) and release-workflow jobs, also run on pull requests
  touching the packaging, that test each archive, the launchd daemon with pcap send / receive on macOS,
  the Windows scheduled task and the no-Npcap path, `build.ps1`, and the Docker compose setup (M10).

#### Documentation

- Japanese and English manuals (mdBook): installation on three OSes, permissions and first login,
  seven tutorials (ARP sweep, VLAN SYN flood, IPv6 NDP, rate control, pcap replay, DHCP server load,
  IS-IS adjacency), operations (deployment, services, Docker, upgrades, DPDK / AF_XDP, CML2, interop,
  tuning, security, receive and verification), developer guide, FAQ and troubleshooting, legal notice,
  and screenshots (M11).
- Protocol, CLI, API, configuration and template schema references generated from the code with
  `wpg docs gen` (M11).
- Architecture decision records `docs/adr/0001` to `0009` and the split specification in `docs/spec/` (M0).

### Security

- `cargo audit` and `cargo deny` run in CI; the deprecated `serde_yaml` was replaced with the maintained
  `serde_norway` fork (RUSTSEC-2024-0449) (M12).

### Known limitations

- AF_XDP zero-copy, DPDK on physical NICs and the virtio PMD, and the CML2 deployment are not yet
  verified on real hardware.
- Binaries are not signed or notarised on macOS and Windows.
- Sending through Npcap on Windows is not covered by CI (Npcap cannot be installed unattended on the
  runners); it is checked by hand on Windows 11.
- wpg is stateless: no TCP handshakes, DHCP leases, LSP flooding or other protocol state machines.
