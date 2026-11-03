# Install on Linux

Supported: Ubuntu 20.04 or later (glibc 2.31) and comparable distributions. Release binaries are not published yet, so you build from source; the result is a single `wpg` executable with the web UI embedded.

## 1. Install the prerequisites

```sh
# Rust toolchain installer (rustup.rs) and a C toolchain for linking
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
sudo apt install build-essential pkg-config git

git clone https://github.com/SeiyaFunaokaJP/web-packet-generator.git
cd web-packet-generator
./scripts/setup.sh          # add --just to also install the just task runner
```

`scripts/setup.sh` installs the Rust toolchain pinned in `rust-toolchain.toml` with the WASM target used by the web UI, and a prebuilt `trunk` (the web UI bundler) at the version pinned in `scripts/versions.env`. It ends by printing the next command.

Linux sends through AF_PACKET, which needs no extra library. Only the optional libpcap backend (`--features pcap`) needs `sudo apt install libpcap-dev`. `tshark` (package `tshark`) is optional; the tutorials use it to inspect generated captures.

## 2. Build

```sh
./scripts/build.sh --release        # or: just release
```

The script builds the web UI with `trunk`, then the `wpg` binary with `cargo`, and prints `==> done: target/release/wpg`. Add `--features pcap`, `--features afxdp` or `--dpdk` for optional backends. Without `trunk` the script still builds the binary but skips the web UI.

## 3. Try it without any privileges

Rendering to a file needs no special rights, so you can check the installation first:

```sh
./target/release/wpg doctor
./target/release/wpg template lint templates/examples/*.wpg.yaml
./target/release/wpg render templates/examples/arp-sweep.wpg.yaml -o /tmp/arp.pcapng
```

`wpg doctor` reports what is missing for sending (see [Permissions](permissions.md)); a failing `caps.raw_socket` check is expected until the next step.

## 4. Allow raw sending

Either run as root or grant capabilities to the binary once (it is lost when the file is replaced):

```sh
sudo setcap cap_net_raw,cap_net_admin+ep ./target/release/wpg
./target/release/wpg doctor
```

## 5. Start the server

```sh
./target/release/wpg serve            # or: just run for development with hot reload
```

By default wpg listens on `127.0.0.1:8080` and loads the templates under `./templates`. Open <http://127.0.0.1:8080> in a browser; the next page, [First login](first-login.md), covers the password and the first screens. To reach it from another machine, set a password and use `--bind 0.0.0.0:8080` (or `bind` in `wpg.toml`, written by `wpg config init`); see [Configuration](../reference/config.md). For a permanent installation on a lab host, `scripts/deploy.sh` copies the binary, applies `setcap` and installs a systemd unit; see [CML2 notes](../operations/cml2.md).

## Send your first frames

```sh
./target/release/wpg ifaces
./target/release/wpg send templates/examples/arp-sweep.wpg.yaml -i eth1 --rate 1000pps
```

Only send on networks you own or are authorised to test. Continue with the [ARP sweep tutorial](../tutorials/arp-sweep.md).
