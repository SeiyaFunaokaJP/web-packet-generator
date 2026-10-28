# Install on Windows

Supported: Windows 10 21H2 or later. Release binaries are not published yet, so you build from source. Transmission uses Npcap only and is intended for GUI use at low to medium rates; use Linux for high rates. Commands below are for PowerShell.

> **Installing wpg on a Linux lab host from Windows?** You do not need to build anything on Windows. Get the repository (`git clone`, or "Download ZIP" on GitHub and unpack it), then double-click **`scripts\deploy.bat`**. A wizard asks for the host, the SSH user and password, the sudo password, the admin password of the web UI and the interfaces to send on, builds wpg on the host and installs it as a service. Only the OpenSSH client that ships with Windows 10 / 11 is needed. See [Deploying](../operations/deploy.md).

## 1. Install the prerequisites

1. Install [rustup](https://rustup.rs) (choose the default MSVC toolchain; it asks for the Visual Studio Build Tools "Desktop development with C++" workload if it is missing).
2. Add the WASM target and the frontend bundler:

   ```powershell
   rustup target add wasm32-unknown-unknown
   cargo install trunk --locked
   cargo install just --locked      # optional
   ```

3. Install [Npcap](https://npcap.com/#download) (the default options are fine; the WinPcap API-compatible mode is not needed). Sending needs it.
4. Download the [Npcap SDK](https://npcap.com/#download) and unpack it to `C:\Npcap-SDK`. `scripts\build.ps1` also finds it in `C:\npcap-sdk-<version>` (the name of the zip; the newest version wins), in the directory named by the environment variable `NPCAP_SDK`, or through `LIBPCAP_LIBDIR` set to its `Lib\x64` directory. It then builds the Npcap backend (`pcap` feature), the only way wpg can send on Windows. Without the SDK the build still succeeds, but the resulting `wpg.exe` cannot send and `build.ps1` warns about it.
5. Optional: Wireshark (for `tshark`).

## 2. Build

```powershell
git clone https://github.com/SeiyaFunaokaJP/web-packet-generator.git
cd web-packet-generator
.\scripts\build.ps1 -Release       # or: just release
```

It prints `==> Npcap SDK: LIBPCAP_LIBDIR=...` when it found the SDK and ends with `==> done: target\release\wpg.exe`. The variants:

| Command | Result |
|---|---|
| `.\scripts\build.ps1` | Debug build (`target\debug\wpg.exe`); adds the Npcap backend when the SDK is found |
| `.\scripts\build.ps1 -Release` | Optimised build (`target\release\wpg.exe`), same SDK detection |
| `.\scripts\build.ps1 -NoPcap` | Build without the Npcap backend even when the SDK is present (cannot send) |
| `.\scripts\build.ps1 -Features pcap` | Require the Npcap backend: stops with an error when the SDK is not found |
| `Get-Help .\scripts\build.ps1 -Detailed` | Every parameter |

Without trunk the frontend is skipped with a warning (the web UI then serves a stale or missing `crates\wpg-web\dist`).

## 3. Try it without any privileges

```powershell
.\target\release\wpg.exe doctor
.\target\release\wpg.exe render templates\examples\arp-sweep.wpg.yaml -o $env:TEMP\arp.pcapng
```

## 4. Allow sending

Check that `wpg doctor` reports Npcap as installed (`npcap`) and the Npcap backend as built in (`backend.pcap`). If `backend.pcap` fails, `build.ps1` did not find the Npcap SDK: install it as in step 1 and build again. With Npcap's default install an ordinary PowerShell can send (`caps.raw_socket` is ok); if Npcap was installed with "Restrict Npcap driver's access to Administrators only", `caps.raw_socket` fails and you must open **PowerShell as Administrator** for sending. See [Permissions](permissions.md).

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

Use the interface name printed by `wpg ifaces` (the bare `{GUID}`; `\Device\NPF_{GUID}` as printed by `tshark -D` works too). The Npcap loopback adapter (`\Device\NPF_Loopback`) does not accept Ethernet frames (`PacketSendPacket failed ... (50)`); to try sending without touching a real network, use a host-only virtual adapter such as Hyper-V's `vEthernet (Default Switch)` or a VMware host-only adapter, and watch it with `wpg capture -i <interface> --backend pcap --include-outgoing` or `tshark`. Only send on networks you own or are authorised to test. Continue with the [ARP sweep tutorial](../tutorials/arp-sweep.md).

## Release archive and Npcap check

Release archives (`wpg-<version>-x86_64-pc-windows-msvc.zip`) contain `wpg.exe` with the web UI and the Npcap backend, the sample templates and `packaging\windows\wpg-service.ps1`. They are not code-signed:

```powershell
# Verify, then clear the "downloaded from the Internet" mark before extracting
(Get-FileHash .\wpg-0.2.0-x86_64-pc-windows-msvc.zip -Algorithm SHA256).Hash   # compare with SHA256SUMS
Unblock-File .\wpg-0.2.0-x86_64-pc-windows-msvc.zip
Expand-Archive .\wpg-0.2.0-x86_64-pc-windows-msvc.zip -DestinationPath "C:\Program Files\wpg"
```

If SmartScreen still shows "Windows protected your PC", choose **More info → Run anyway** once.

Npcap is not bundled (its licence does not allow redistribution with the free edition). `wpg.exe` loads `wpcap.dll` only when the `pcap` backend is first used (it is linked with `/DELAYLOAD`) and adds `System32\Npcap` to the DLL search path itself, so it starts without Npcap and works with either Npcap install mode. Both `wpg doctor` and `scripts\build.ps1` check for it:

- `wpg doctor` fails the `npcap` check when Npcap is missing and warns when Npcap looks installed but `wpcap.dll` is in neither `System32\Npcap` nor `System32`. The fix line links to <https://npcap.com/#download>.
- Sending without Npcap stops with `Npcap is not installed; install it from https://npcap.com/#download and run wpg doctor` instead of crashing.
- `wpg doctor` also fails `backend.pcap` when this `wpg.exe` was built without the Npcap backend, and tells you to unpack the SDK and run `scripts\build.ps1` again.
- When Npcap was installed in administrator-only mode, `caps.raw_socket` fails in an unelevated terminal; the fix is an Administrator terminal or reinstalling Npcap without that option.
- `scripts\build.ps1` prints a warning before building, since a build without Npcap succeeds but cannot send, and another one when it builds without the Npcap SDK.

To run wpg in the background at boot, see [Running as a service](../operations/services.md#windows).
