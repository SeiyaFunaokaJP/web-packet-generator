# Introduction

wpg (web-packet-generator) is a Rust packet generator for network verification, driven from a web GUI. It lets you assemble arbitrary packets, including deep OSI-family protocols such as IS-IS and CLNP, and transmit them at high speed. Transmission backends range from pcap/Npcap up to AF_PACKET, AF_XDP and DPDK. It ships as a single binary for Windows, macOS and Linux. Use is limited to lab and verification environments.

![Building Ethernet / IPv4 / UDP in the packet builder, with a source-address range](images/builder.gif)

## Where to start

1. [Choose how to install](getting-started/overview.md): build from source on your OS, deploy to a Linux lab host over SSH, or run the Docker image.
2. Follow the install page for your choice, then [Permissions](getting-started/permissions.md) if `wpg doctor` reports that sending is not allowed.
3. [First login](getting-started/first-login.md): set the password and walk through the screens.
4. Work through the tutorials, starting with the [ARP sweep](tutorials/arp-sweep.md).

You can try everything except sending without any privileges: rendering a template to a pcapng file needs no raw socket.

## Concepts

| Term | Meaning |
|---|---|
| Template | A `.wpg.yaml` file describing a packet as a stack of layers (`ethernet2`, `ipv4`, `udp`, ...), their field values, variables and generators. Templates are what you save, share and send. See the [template schema](reference/template-schema.md). |
| Library | The template directories wpg loads, `./templates` by default. The bundled library has `examples`, `protocols`, `scenarios`, `malformed` and `interop`. |
| Field generator | A field value that changes from frame to frame: a range, a list, random values, a counter, values from a file or an expression. One template can expand to many frames (streams). |
| Builder | The GUI screen that edits a template layer by layer and previews the hex dump and field tree in the browser. |
| Job | One run of templates on an interface with a rate (pps, bps or % of line rate), a count or a duration. Jobs show live statistics. |
| Backend | How frames leave the machine: `afpacket`, `afxdp` or `dpdk` on Linux, `pcap` (libpcap / Npcap / BPF) everywhere, `pcapfile` to write a pcapng instead of sending (dry run), and `null` for benchmarks. |
| `wpg doctor` | The self-diagnosis that checks privileges, interfaces, kernel features and libraries, and prints the fix for each failed check. |

The same binary is both the server (`wpg serve`, the web UI and REST API) and a command-line tool (`wpg send`, `wpg render`, `wpg template lint`, ...); see the [CLI reference](reference/cli.md).

## How this manual is organised

- **Getting Started**: installation, permissions and the first login.
- **Tutorials**: end-to-end tasks such as an ARP sweep, rate control, pcap replay and an IS-IS adjacency.
- **Reference**: generated pages for every protocol, the template schema, the API, the CLI and `wpg.toml`.
- **Operations**: deployment, services, Docker, upgrades, fast backends, TLS, receive and verification.
- **Developer Guide**: the crate layout and how to add protocols and translations.

Only send on networks you own or are authorised to test; see the [legal notice](legal.md).
