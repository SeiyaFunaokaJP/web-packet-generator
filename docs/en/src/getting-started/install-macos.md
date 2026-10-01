# Install on macOS

Supported: macOS 12 or later. Release binaries are not published yet, so you build from source; the result is a single `wpg` executable with the web UI embedded. macOS ships libpcap, so no extra library is needed.

## 1. Install the prerequisites

```sh
xcode-select --install                     # compiler and linker
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
cargo install just --locked                # optional task runner
brew install wireshark                     # optional: tshark, and ChmodBPF (see below)
```

## 2. Build

```sh
git clone https://github.com/SeiyaFunaokaJP/web-packet-generator.git
cd web-packet-generator
just release                # or: ./scripts/build.sh --release
```

The last line printed is `==> done: target/release/wpg`.

## 3. Try it without any privileges

```sh
./target/release/wpg doctor
./target/release/wpg template lint templates/examples/*.wpg.yaml
./target/release/wpg render templates/examples/arp-sweep.wpg.yaml -o /tmp/arp.pcapng
```

## 4. Allow sending

Sending goes through `/dev/bpf*`. Either run wpg with `sudo`, or make the devices accessible: installing Wireshark with its ChmodBPF helper does this for members of the `access_bpf` group. `wpg doctor` prints the exact hint for your machine; see [Permissions](permissions.md).

## 5. Start the server

```sh
./target/release/wpg serve
```

wpg listens on `127.0.0.1:8080` by default and loads the templates under `./templates`. Open <http://127.0.0.1:8080> in a browser and continue with [First login](first-login.md).

## Send your first frames

```sh
./target/release/wpg ifaces
sudo ./target/release/wpg send templates/examples/arp-sweep.wpg.yaml -i en0 --rate 1000pps
```

Only send on networks you own or are authorised to test. Continue with the [ARP sweep tutorial](../tutorials/arp-sweep.md).
