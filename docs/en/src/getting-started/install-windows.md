# Install on Windows

Supported: Windows 10 21H2 or later. Release binaries are not published yet, so you build from source. Transmission uses Npcap only and is intended for GUI use at low to medium rates; use Linux for high rates. Commands below are for PowerShell.

## 1. Install the prerequisites

1. Install [rustup](https://rustup.rs) (choose the default MSVC toolchain; it asks for the Visual Studio Build Tools "Desktop development with C++" workload if it is missing).
2. Add the WASM target and the frontend bundler:

   ```powershell
   rustup target add wasm32-unknown-unknown
   cargo install trunk --locked
   cargo install just --locked      # optional
   ```

3. Install [Npcap](https://npcap.com/#download) and tick **Install Npcap in WinPcap API-compatible Mode**. Sending needs it.
4. Only if you build the optional libpcap backend (`--features pcap`): install the Npcap SDK and put its `Lib\x64` directory on the `LIB` environment variable.
5. Optional: Wireshark (for `tshark`).

## 2. Build

```powershell
git clone https://github.com/SeiyaFunaokaJP/web-packet-generator.git
cd web-packet-generator
.\scripts\build.ps1 -Release       # or: just release
```

It ends with `==> done: target\release\wpg.exe`.

## 3. Try it without any privileges

```powershell
.\target\release\wpg.exe doctor
.\target\release\wpg.exe render templates\examples\arp-sweep.wpg.yaml -o $env:TEMP\arp.pcapng
```

## 4. Allow sending

Open **PowerShell as Administrator** for sending, and check that `wpg doctor` reports Npcap as installed in WinPcap-compatible mode. See [Permissions](permissions.md).

## 5. Start the server

```powershell
.\scripts\run.ps1                  # or: .\target\release\wpg.exe serve
```

wpg listens on `127.0.0.1:8080` by default and loads the templates under `.\templates`. Open <http://127.0.0.1:8080> in a browser and continue with [First login](first-login.md). Windows Firewall may ask whether to allow `wpg.exe`; allow private networks only if you bind to a non-local address.

## Send your first frames

```powershell
.\target\release\wpg.exe ifaces
.\target\release\wpg.exe send templates\examples\arp-sweep.wpg.yaml -i <interface> --rate 1000pps
```

Use the interface name printed by `wpg ifaces`. Only send on networks you own or are authorised to test. Continue with the [ARP sweep tutorial](../tutorials/arp-sweep.md).
