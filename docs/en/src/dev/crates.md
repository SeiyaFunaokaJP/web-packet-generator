# Crate layout

The repository is a Cargo workspace under `crates/`. User-facing strings live in [`locales/`](add-i18n.md), sample templates in `templates/`, documentation in `docs/` and the end-to-end tests in `e2e/`, all beside `crates/`.

## The crates

| Crate | Role | wasm32 |
|---|---|---|
| `wpg-core` | Protocol definition IR, packet build/parse, field generators and lint diagnostics (`diag::keys`). OS-independent | ✓ |
| `wpg-protocols` | Built-in protocol definitions (Ethernet through IS-IS / CLNP). A Rust macro compiles them to the `wpg-core` IR | ✓ |
| `wpg-i18n` | Fluent-based i18n (ja / en / zh-CN / zh-TW / ko / de / fr / es); embeds `locales/<lang>/main.ftl` with `include_str!` | ✓ |
| `wpg-template` | Template file format (`.wpg.yaml` / JSON), schema versioning and (de)serialisation | |
| `wpg-tx` | Transmit backends (`null`, `pcap`, AF_PACKET, AF_XDP, DPDK) behind the `TxBackend` trait | |
| `wpg-rx` | Receive-side counters and simple verification (v1: minimal) | |
| `wpg-sys` | OS facts: network interfaces, privileges, kernel, hugepages, Npcap / libpcap detection, the `doctor` checks | |
| `wpg-log` | Logging setup: a tracing subscriber (pretty / JSON), rolling log files and a broadcast sink for live streaming | |
| `wpg-server` | axum HTTP / WebSocket server with the web UI embedded | |
| `wpg-cli` | The command line itself (`serve`, `send`, `render`, `import`, `template`, `doctor`, …); builds the `wpg` binary | |
| `wpg-web` | Leptos (CSR) web UI, compiled to WASM with Trunk | ✓ |
| `dpdk-sys` | Raw FFI to DPDK (24.11 / 23.11 LTS), generated with bindgen; the inline fast path (`rte_eth_tx_burst`, `rte_pktmbuf_alloc`, ...) and `rte_errno` are wrapped by a C shim (`wpg_` prefix). Empty unless the `link` feature is on (enabled by `wpg-tx`'s `dpdk`). ADR-0004 | |
| `wpg-testutil` | Test helpers: pcap / pcapng reader and writer, hex helpers, golden-file assertions (not published) | |

A further `wpg-golden-tests` crate holds the golden-pcap byte checks and the tshark cross-check (`cargo test -p wpg-golden-tests`).

## The wasm32 constraint

`wpg-core`, `wpg-protocols`, `wpg-i18n` and `wpg-web` must also build for the browser (`wasm32-unknown-unknown`) for the web UI. Do not use file I/O, threads or OS-specific crates in these four. CI and `just check-wasm` enforce it with:

```sh
cargo check -p wpg-core -p wpg-protocols -p wpg-i18n --target wasm32-unknown-unknown
```

Transmission and OS facts (`wpg-tx`, `wpg-sys`) are native-only. When you add build/parse logic, first check whether it belongs in the OS-independent `wpg-core`.
