# Roadmap

**English** | [日本語](ROADMAP.ja.md)

Milestones towards wpg 1.0. A ticked item is complete and released on `main`; an unticked item is in
progress or not started. The detailed specification lives in [`docs/spec/`](docs/spec/README.md) (Japanese
primary), the design decisions in [`docs/adr/`](docs/adr/README.md), and the list of changes in
[CHANGELOG.md](CHANGELOG.md). Milestone numbers (M0 to M12) are referenced from the changelog, the
developer guide and code comments.

## Status

M0 to M11 are complete. The v1.0 release gate (M12) is in progress: some transmit paths (AF_XDP zero-copy
and DPDK on physical NICs, CML2 on real hardware) are not yet verified outside virtual environments, the pcap
backend is not yet confirmed on macOS, and the first release has not been published.

## M0: Project foundation

- [x] Cargo workspace and crate skeletons (`wpg-core`, `wpg-protocols`, `wpg-tx`, `wpg-template`, `wpg-server`, `wpg-cli`, `wpg-web`)
- [x] Toolchain, rustfmt, clippy and `cargo deny` configuration; MIT / Apache-2.0 dual licence
- [x] `justfile` and the build / run scripts for Linux, macOS and Windows
- [x] GitHub Actions: fmt / clippy / test on the three OSes, `cargo audit`
- [x] README (ja / en), CONTRIBUTING, Code of Conduct
- [x] mdBook skeleton (ja / en) and the split of the specification into `docs/spec/`
- [x] i18n foundation (Fluent) for server and web UI
- [x] Design decisions recorded as ADRs 0001 to 0009

## M1: Core engine MVP

- [x] Field type system and protocol definition IR (`wpg-core`)
- [x] Rust definition macros and the YAML loader, both lowered to the same IR
- [x] Builder: automatic lengths, checksums and next-header fields, Auto / Manual per field
- [x] Parser: bytes to layer tree (for import)
- [x] Constraint checks (warning / error)
- [x] P0 protocols: Ethernet II, 802.3 + LLC / SNAP, 802.1Q, QinQ, ARP, IPv4 (+options), IPv6 (+extension headers), ICMPv4, ICMPv6 + NDP, TCP (+options), UDP, payloads
- [x] Generators: range / list / random / counter / file / expr, zip / product / nested
- [x] Stream expansion (pre-render or on-the-fly, chosen automatically)
- [x] Golden pcap tests and `tshark -V` cross-checks (optional CI job)
- [x] `proptest` round-trip tests (build → parse → build)
- [x] `wpg-core` builds for `wasm32-unknown-unknown` in CI

## M2: Transmit engine

- [x] `TxBackend` trait, `pcapfile` and `null` backends
- [x] `pcap` backend (Linux libpcap / macOS BPF / Windows Npcap)
- [x] Linux `afpacket` (TPACKET_V3 + `sendmmsg`), core pinning, multiple queues
- [x] Rate control (pps / bps / %), bursts, inter-frame gap, pacing accuracy measurement
- [x] Statistics per backend and per stream, 100 ms snapshots
- [x] Interface enumeration on the three OSes
- [x] `wpg doctor` (privileges, Npcap, setcap, hugepages)
- [x] `wpg bench` and the performance baseline (`docs/perf.md`)
- [x] CML2 (Ubuntu on virtio-net) notes in the operations guide; measurements on real CML2 hosts are still open

## M3: Server and CLI

- [x] axum server with the frontend embedded through `rust-embed`
- [x] REST API: templates, protocol catalog, render / preview, jobs, interfaces, statistics, authentication
- [x] WebSocket for job progress, statistics and logs
- [x] OpenAPI (`utoipa`) and Swagger UI
- [x] Authentication: argon2id password, sessions, rate limiting, localhost-only mode without a password
- [x] TLS (`rustls`, generated self-signed certificate)
- [x] Configuration (`wpg.toml`, environment, CLI), `wpg config init`
- [x] Job manager (mixed streams, scheduling, safe cancellation)
- [x] Transmit policy (allowed interfaces, maximum rate)
- [x] CLI subcommands: serve / send / render / import / template / ifaces / doctor / bench
- [x] `tracing` logging (JSON, rotation)

## M4: GUI MVP

- [x] Leptos project, Tailwind, routing, i18n, themes
- [x] Login screen and session handling
- [x] Builder: protocol palette, layer stack drag and drop, per-type field editors, Auto / Manual toggles
- [x] Hex dump and protocol tree with two-way highlighting, direct hex editing
- [x] In-browser WASM preview (`wpg-core`) cross-checked against the server
- [x] Generator settings and the stream size / send-time estimates
- [x] Jobs: stream mixing, rate settings, start / stop, live statistics graphs
- [x] Templates: list, search, tags, import / export, duplicate
- [x] Settings screen
- [x] Undo / redo, keyboard shortcuts, draft autosave
- [x] Keyboard-only drag and drop, ARIA
- [x] End-to-end tests (Playwright, Chromium)

## M5: Template format and sharing

- [x] YAML / JSON schema (`schema: 1`), published JSON Schema, `wpg template lint`
- [x] Template variables and run-time overrides
- [x] `.wpgbundle` export / import
- [x] Multiple library paths (Git-managed directories), hot reload
- [x] Bundled sample templates (P0 protocols and representative scenarios)
- [x] pcap / pcapng import and hex-paste import

## M6: P1 protocols

- [x] LLDP / CDP / STP, RSTP, MSTP / LACP / EAPOL / PPPoE / PPP
- [x] VXLAN / GRE / NVGRE / MPLS / MPLS-in-UDP / L2TP / GTP-U / Geneve / IP-in-IP
- [x] OSPFv2 / v3 / BGP / RIP / RIPng / EIGRP
- [x] VRRP / HSRP / GLBP
- [x] IGMP / MLD / PIM
- [x] DHCPv4 / v6 / DNS / NTP / SNMP / Syslog / RADIUS / TFTP
- [x] Sample templates, golden pcaps and documentation for each protocol
- [x] IR extensions that removed the early approximations (peek-based demultiplexing, list counts, NUL-terminated strings, checksum exclusions, source-port demultiplexing)

## M7: P2 OSI and specialist protocols

- [x] 802.2 LLC (0xFE) OSI encapsulation, NSAP type and editor
- [x] IS-IS: IIH (LAN L1 / L2, P2P), LSP, CSNP, PSNP, main TLVs and sub-TLVs, Fletcher checksum, padding
- [x] CLNP / ES-IS
- [x] SCTP (CRC-32C)
- [x] OSPF Opaque LSAs and SR, BGP MP / EVPN / VPNv4 NLRI
- [x] LDP / RSVP-TE / BFD
- [x] SRv6 SRH and SR-MPLS helpers
- [x] PTP / ESMC / ERPS / Y.1731 CFM / 802.3ah OAM
- [x] MACsec (structure only) / GTP-C / Diameter / SIP / RTP
- [x] Interoperability scenarios against FRR (`templates/interop/`, `scripts/interop/`)

## M8: High-speed backends

- [x] AF_XDP backend (`--features afxdp`, copy / zero-copy automatic, kernel 5.10+)
- [x] `dpdk-sys` crate (bindgen, DPDK 24.11 / 23.11 LTS)
- [x] DPDK backend (`--features dpdk`), EAL settings, hugepages and `dpdk-devbind` procedure, virtio PMD
- [x] Multi-queue / multi-core transmission with NUMA awareness
- [x] Rate control accuracy (busy-poll, TSC clock)
- [x] Performance regression test (`wpg bench` nightly in CI)

## M9: Receive and verification

- [x] Receive counters (pps / bps, filter matches) with tcpdump-style filters
- [x] Transmit signatures and loss / order / latency verification
- [x] First N packets and pcapng capture
- [x] Receive screen in the GUI

## M10: Deployment, distribution and operations

- [x] `scripts/deploy.sh` (scp + setcap + systemd + health check), `--dpdk` option
- [x] `scripts/release.sh` and GitHub Releases for the three OSes (built and smoke-tested in CI; the first published release is part of M12)
- [x] Dockerfile / compose, systemd unit, launchd plist, Windows service procedure (checked in CI)
- [x] Windows: Npcap detection and installation guidance, `build.ps1` verification
- [x] macOS: Gatekeeper / quarantine workaround documented (signing and notarisation deferred)
- [x] Upgrade procedure and template migration (`wpg template migrate`)

## M11: Documentation

- [x] Getting started for the three OSes, five tutorials (ja / en)
- [x] Generated protocol reference (definition IR → Markdown)
- [x] Generated API / CLI / configuration / template schema references
- [x] DPDK / AF_XDP / CML2 setup guides and performance tuning
- [x] Developer guide (adding a protocol, adding a language, releasing)
- [x] FAQ, troubleshooting, legal notice
- [x] Screenshots and the builder GIF, generated from the real UI

## M12: v1.0 release gate

- [x] All P0 + P1 golden pcaps and tshark cross-checks green
- [ ] Build, start and pcap-backend transmission confirmed on the three OSes (Windows done; macOS and Linux pcap backend open)
- [ ] Linux `afpacket` at 1 Mpps/core or more (reference value); DPDK recorded as reference only
- [x] GUI end-to-end tests green, zero i18n gaps (lint)
- [x] `cargo audit` / `cargo deny` clean
- [x] Documentation complete in ja / en, CHANGELOG

## Added beyond the 1.0 plan

- [x] SNMP manager (Get / Walk / Set / traps, v1 / v2c / v3, MIB upload, pcap recording; ADR-0010)
- [x] Web terminal (SSH / Telnet, jump hosts (ProxyJump); ADR-0011)
- [x] Interactive deploy wizard and unattended deploys (Windows `deploy.bat` / `deploy.ps1`, `deploy.sh`; answer files, several hosts, scheduled runs)

## After 1.0

Candidates, not commitments: stateful sessions (TCP handshakes, HTTP clients), 802.11, further P3
protocols through user-defined definitions, and signed macOS builds.
