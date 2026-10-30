# Choose how to install

wpg is a single `wpg` executable with the web UI embedded. Release binaries are not published yet, so every way below builds it from source; they differ in where it is built and how it runs.

## What you need

- A machine to run wpg on: Linux (Ubuntu 20.04 or later and comparable), macOS 12 or later, or Windows 10 21H2 or later.
- A network interface connected to the lab network you want to test. Sending on a network you do not own or are not authorised to test is not allowed.
- A browser on the same machine, or on another one if you set a password and bind to a non-local address.

## Pick a way

| You want to | Use | Needs |
|---|---|---|
| Run wpg on a Linux lab host (CML2, EVE-NG, a VM), at high rates, as a service | [Deploying with scripts/deploy.sh](../operations/deploy.md): a wizard that builds wpg on the host over SSH and installs a systemd unit. Start it from Windows (`scripts\deploy.bat`), Linux or macOS. | SSH access with sudo to the host |
| Run wpg on a Linux host without installing a Rust toolchain | [Run with Docker](docker.md) | Docker on a Linux host |
| Use wpg on your own PC, or develop it | Build from source: [Linux](install-linux.md), [macOS](install-macos.md), [Windows](install-windows.md) | rustup, trunk; Npcap and its SDK on Windows |

Notes for the choice:

- **Rates.** Windows sends through Npcap only and is meant for the GUI at low to medium rates. macOS sends through BPF. For high rates (AF_PACKET, AF_XDP, DPDK) use Linux.
- **Docker Desktop** on macOS and Windows runs containers in a VM, so frames never reach your physical NIC. Use Docker on Linux hosts only.
- **No privileges yet?** Every path can render templates to pcapng files and run the web UI without raw-socket rights; only sending needs [Permissions](permissions.md).

## After installing

Whichever way you chose, the next steps are the same:

1. Run `wpg doctor` and fix what it reports for sending ([Permissions](permissions.md)).
2. Start the server with `wpg serve` (the deploy script and Docker start it for you) and open <http://127.0.0.1:8080>.
3. Set the password and look around: [First login](first-login.md).
4. Try a dry run (the `pcapfile` backend writes a pcapng file instead of sending), then send for real in the [ARP sweep tutorial](../tutorials/arp-sweep.md).

If something does not work, see [FAQ and troubleshooting](../faq/troubleshooting.md).
