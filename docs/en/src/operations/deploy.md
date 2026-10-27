# Deploying (deploy.bat / deploy.ps1 / deploy.sh)

Status: draft

These scripts install wpg on a Linux host (Ubuntu 20.04 to 24.04, Debian; first target: an Ubuntu node on CML2) over SSH. From Windows use `scripts\deploy.bat` (a wrapper around `scripts\deploy.ps1`), from Linux / macOS `scripts/deploy.sh`. Both run the same host-side scripts (`scripts/lib/*.sh`), so the result is the same.

**There are no arguments to remember.** Started without arguments, a wizard asks for everything it needs, one question at a time, in English or Japanese (from the system language). Every question has a default in `[ ]` that Enter accepts.

## Using the wizard (recommended)

### Windows

Open the `scripts` folder of the repository in Explorer and **double-click `deploy.bat`**. From PowerShell:

```powershell
.\scripts\deploy.bat           # or: powershell -ExecutionPolicy Bypass -File .\scripts\deploy.ps1
```

Requirements: the OpenSSH client (`ssh`, `scp`, `ssh-keygen`) and `tar.exe` that ship with Windows 10 1809 and later / Windows 11. If the OpenSSH client is missing, add it under Settings > Apps > Optional features. Rust and Git are not needed on Windows (with Git, the source is packed with `git archive`).

### Linux / macOS

```sh
scripts/deploy.sh              # no arguments = the wizard (or: just deploy-wizard)
```

`sshpass` is used for the SSH password when it is installed (not required).

### What it looks like

A first run (Windows PowerShell 5.1, English system). Input at `********` is not shown.

```text
wpg deploy wizard (Enter accepts the default in [ ]; Ctrl+C aborts)
Target host (IP address or name): 192.0.2.10
SSH port [22]:
SSH user [ubuntu]: cisco
SSH authentication:
  1) Password (used once to install a deploy key)
  2) An existing private key file
Choose a number [1]:

[1] Connecting to cisco@192.0.2.10
SSH password (cisco@192.0.2.10): ********
  Created the deploy key C:\Users\you\.ssh\wpg_deploy_ed25519
  Installing the public key into ~/.ssh/authorized_keys on the host (the password is used this once).
  Key login works; no password needed next time.
  OS: Ubuntu 24.04.1 LTS (x86_64)   sudo: password   wpg: -
How to get the wpg binary:
  1) Build from source on the host (recommended; needs internet there; 10-30 min the first time)
  2) Upload a Linux binary file you already have
  3) Download from a GitHub Release (no release yet)
Choose a number [1]:
Source code:
  1) Upload this checkout (C:\work\web-packet-generator)
  2) git clone from GitHub
Choose a number [1]:
Install as a systemd service (starts at boot)? [Y/n]:
Web UI listen address:port (0.0.0.0 = all) [0.0.0.0:8080]:
Interfaces jobs may send on (tx.allowed_ifaces):
  1) ens2         UP       192.0.2.10/24  <- management (this SSH session)
  2) ens3         UP
  3) ens4         UP
Numbers, comma separated (e.g. 2,3; all = no restriction) [2,3]:
Enable the web terminal? (newer wpg only) [y/N]:
Enable the SNMP trap receiver (UDP 162)? (newer wpg only) [y/N]: y
sudo password (Enter = same as the SSH password):
wpg admin password (Web UI login): ********
Repeat it: ********

Summary
  Host : cisco@192.0.2.10 (port 22)
  Binary : build on the host (this checkout: C:\work\web-packet-generator)
  Service : systemd (wpg.service)
  Listen : 0.0.0.0:8080
  TX interfaces : ens3,ens4
  Admin password : set
  Web terminal : off
  SNMP traps : on (UDP 162)
Go ahead? [Y/n]:
Save these answers for next time? (no passwords) [Y/n]:
Saved the answers to C:\Users\you\AppData\Roaming\wpg\deploy.conf (no passwords).
log: C:\Users\you\AppData\Local\wpg\deploy-logs\deploy-20261027-101500.log

[2/6] Uploading to cisco@192.0.2.10:/tmp/wpg-deploy.Xa81Qe
[3/6] Installing the build dependencies (apt)
  -> apt-get install build-essential pkg-config libpcap-dev curl git ca-certificates tar
[4/6] Building on the host (10-30 minutes the first time)
  -> installing rustup (https://rustup.rs) into ~/.cargo
  -> installed trunk 0.22.1 (prebuilt)
  ...
  -> built wpg 0.1.0
[5/6] Installing (sudo)
  -> installed wpg 0.1.0 to /usr/local/bin/wpg
  -> setcap cap_net_raw,cap_net_admin,cap_net_bind_service+eip /usr/local/bin/wpg
  -> systemd unit wpg.service (re)started
[6/6] Health check (http://127.0.0.1:8080/api/health)
  {"status":"ok","version":"0.1.0",...}

Done. Open in a browser:
  http://192.0.2.10:8080/
```

The `deploy.sh` wizard asks the same questions (its binary menu also offers "Build on this machine and upload", and it asks for the sudo password right after connecting).

### The questions

| Question | Default | Notes |
|---|---|---|
| Target host | last value | IP address or name. |
| SSH port | 22 | |
| SSH user | ubuntu | A user that may use `sudo`. |
| Authentication | password | Used once to install a deploy key (see below); an existing key file works too. Not asked when a key login already works. |
| sudo password | same as the SSH password | Not asked with `NOPASSWD` sudo or a root login. Asked again up to 3 times when wrong. |
| Binary | build on the host | The default changes when the host cannot reach the internet; see "Getting the binary". |
| Install as a service | yes | systemd `wpg.service`. |
| Listen address | `0.0.0.0:8080` | With `127.0.0.1:8080` the UI is only reachable through an SSH tunnel. |
| Admin password | (required) | Web UI login (user `admin`), typed twice. Enter keeps it when wpg is already installed. |
| TX interfaces | all but the management one | The host's `ip -br addr`, numbered; the interface carrying the SSH session is marked "management". `all` = no restriction. |
| Web terminal / SNMP traps | no | Only enabled when the installed wpg supports them (see below). |

### Later runs

The answers (never the passwords) are saved to `%APPDATA%\wpg\deploy.conf` on Windows and `~/.config/wpg/deploy.conf` on Linux / macOS. The next run starts with "Use the previous settings?"; Enter repeats the last deployment and only the passwords are asked (the SSH password is not needed any more, the key works).

When wpg is installed and a `wpg.prev` exists, a menu comes first:

```text
wpg is installed (wpg 0.1.0). What do you want to do?
  1) Install / upgrade
  2) Roll back (restore the previous binary wpg.prev)
  3) Quit
```

## Getting the binary

| Method | When | Needs |
|---|---|---|
| Build from source on the host (default) | Almost always: the binary matches the host's glibc. | Internet access from the host (apt, rustup, crates.io, GitHub); 2 GiB of memory or swap; about 5 GiB of free disk. |
| Upload a Linux binary | You already built one on another Linux machine. | A `wpg` ELF file (Windows' `wpg.exe` does not work). A binary built against a newer glibc than the host's does not run. |
| Download a GitHub Release | Once releases are published. | Internet access from your machine. **There are no releases yet** (the wizard says "no release yet"). |
| Build on this machine (deploy.sh only) | Your machine is Linux on the same CPU architecture. | Rust and trunk (the original `deploy.sh` behaviour). |

Building on the host:

1. As root, installs build-essential, pkg-config, libpcap-dev, curl, git and ca-certificates with `apt-get` (on dnf-based distributions the equivalent packages, best effort).
2. As the SSH user, installs rustup (stable, minimal profile, `~/.cargo`), the `wasm32-unknown-unknown` target and trunk (prebuilt; `cargo install trunk` when that fails).
3. Puts the source into `~/wpg-build/src`: by default this checkout, uploaded (`git archive` when Git is available, including uncommitted changes of tracked files but not untracked files; otherwise `tar.exe` without `target` and the like), or a `git clone` from GitHub at a branch / tag.
4. Runs `scripts/build.sh --release`. `~/wpg-build/target` is kept, so later builds are incremental. The number of parallel jobs is lowered on hosts with little memory.

## What one run does

1. Connects over SSH (installing the key if needed) and reads facts about the host (OS, CPU, sudo, interfaces, installed wpg, internet access).
2. Uploads to a temporary directory on the host (`/tmp/wpg-deploy.*`, removed at the end).
3. (When building on the host) installs the dependencies and builds.
4. Installs (`scripts/lib/deploy-remote.sh` as root): the binary goes to `/usr/local/bin/wpg` (the previous one stays as `wpg.prev`) with `setcap cap_net_raw,cap_net_admin,cap_net_bind_service+eip` (installing `libcap2-bin` first if `setcap` is missing and `apt-get` exists). For the service it creates the `wpg` system user, `/etc/wpg/wpg.toml` (only if missing), `/etc/wpg/wpg.env` and `/etc/wpg/password`, and (re)starts `wpg.service`. If ufw / firewalld is active and the port is not open, it prints how to open it (the firewall is never changed).
5. Prints `wpg doctor` from the host and waits for `GET /api/health` (30 s; shows the last journal lines if the server does not answer).
6. Prints the URL to open.

Steps are numbered (`[3/6]`) and coloured, and everything also goes to a log file (Windows: `%LOCALAPPDATA%\wpg\deploy-logs\`, deploy.sh wizard: `~/.local/state/wpg/`). On a failure the scripts name the failing step, show the last lines of output and suggest a fix.

## From the command line

### deploy.sh

All the original options still work.

```sh
# Build, upload, setcap only (run `wpg send` / `wpg serve` yourself on the host)
scripts/deploy.sh ubuntu@192.0.2.10

# Install as a service that listens on all addresses, allows sending only on ens3
scripts/deploy.sh ubuntu@192.0.2.10 --service --iface ens3 --password-file ./admin-password

# Build from source on the host and install (the password is asked once, a key is installed)
scripts/deploy.sh ubuntu@192.0.2.10 --ask-password --sudo-password --build-on-host --service --iface ens3

# The same through the task runner
just deploy ubuntu@192.0.2.10

# Show what would run, without touching anything
scripts/deploy.sh ubuntu@192.0.2.10 --service --dry-run
```

`--password-file` takes a local file whose first line is the admin password (plain text) or its argon2id hash; it lands on the host as `/etc/wpg/password` (mode `0640`, group `wpg`). Without it, and without a password already on the host (`/etc/wpg/password`, `password_hash` / `password_file` in `/etc/wpg/wpg.toml`, or an existing user database), the script refuses to start the server on a non-loopback address: use `--bind 127.0.0.1:8080` plus an SSH tunnel (`ssh -L 8080:127.0.0.1:8080 ubuntu@192.0.2.10`), or `--allow-insecure` on an isolated lab network.

| Option | Meaning |
|---|---|
| (none), `--interactive` | The wizard. |
| `--iface <name>` | Interface jobs may send on (repeatable); written as `WPG_TX__ALLOWED_IFACES`. Leave the management interface out. |
| `--service` | Install / update `wpg.service` and restart it. |
| `--bind <addr:port>` | Listen address of the service (default `0.0.0.0:8080`). |
| `--password-file <file>` | Admin password (or argon2 hash) for the service. |
| `--allow-insecure` | Start without a password on a non-loopback address (adds a drop-in running `wpg serve --allow-insecure`). |
| `--binary <file>` | Deploy this binary instead of `target/release/wpg`. |
| `--target <triple>` | Build for another target, e.g. `aarch64-unknown-linux-gnu` (needs a cross linker). |
| `--build-on-host` | Build from source on the host: `--source local` (default: upload this checkout) or `--source git` (`--git-url`, `--git-ref`; default: GitHub, `main`). |
| `--release-tag <tag>` | Use the Linux binary of a GitHub Release (`latest` for the newest). |
| `--prefix <dir>` | Install prefix on the host (default `/usr/local`). |
| `--port <n>`, `--ssh-opt <opt>` | SSH port and extra `-o` options (e.g. `--ssh-opt IdentityFile=~/.ssh/lab`). |
| `--identity <file>` | SSH private key to use. |
| `--ask-password` | Ask for the SSH password once and install the deploy key `~/.ssh/wpg_deploy_ed25519` on the host. |
| `--sudo-password` | Ask for the sudo password up front and pass it to `sudo -S` on stdin (default: sudo asks on the SSH terminal). |
| `--terminal`, `--snmp-trap` | Enable the web terminal / the SNMP trap receiver (see below). |
| `--no-templates` | Do not copy the sample templates to `/usr/local/share/wpg/templates`. |
| `--no-healthcheck` | Skip the health check. |
| `--rollback` | Swap `/usr/local/bin/wpg` with `wpg.prev` and restart the service. Running it twice undoes it. |
| `--dpdk`, `--hugepages <n>` | Prepare the host for DPDK (see below). |
| `--profile <file>`, `--lang <ja\|en>`, `--log <file>` | Wizard answers file, message language, log file. |
| `--dry-run` | Print the commands instead of running them. |

### deploy.ps1

Everything the wizard asks can also be passed as parameters. With `-HostName` the wizard does not start (only missing passwords are asked).

```powershell
# Build on the host and install the service, without confirmation
.\scripts\deploy.ps1 -HostName 192.0.2.10 -User cisco -Iface ens3,ens4 -Yes

# Use a Linux binary you already have
.\scripts\deploy.ps1 -HostName 192.0.2.10 -User cisco -BinarySource file -Binary .\wpg

# Build the v0.2.0 tag from GitHub
.\scripts\deploy.ps1 -HostName 192.0.2.10 -Source git -GitRef v0.2.0

# Roll back
.\scripts\deploy.ps1 -HostName 192.0.2.10 -User cisco -Rollback
```

| Parameter | Meaning |
|---|---|
| `-HostName`, `-User`, `-Port`, `-Identity` | Host, SSH user (default `ubuntu`), port, existing private key. |
| `-SshPassword`, `-SudoPassword`, `-AdminPassword` | `SecureString`s (e.g. `(Read-Host -AsSecureString)`); asked when needed if omitted. |
| `-BinarySource build\|file\|release` | Build on the host (default), the file `-Binary`, or a GitHub Release (`-ReleaseTag`). |
| `-Source local\|git`, `-GitUrl`, `-GitRef` | Source to build. |
| `-Bind`, `-Iface` | Listen address, interfaces jobs may send on. |
| `-NoService` | Do not install the service (installed by default). |
| `-Terminal`, `-SnmpTrap` | Web terminal / SNMP trap receiver. |
| `-Rollback`, `-NoTemplates`, `-NoHealthcheck` | As in deploy.sh. |
| `-ProfilePath`, `-Lang ja\|en`, `-LogFile` | Answers file, message language, log file. |
| `-Interactive`, `-Yes`, `-DryRun` | Start the wizard with the parameters as defaults / skip confirmations / only connect and ask, without building or installing. |

## Security

- **The SSH password is used once, to install a key.** The scripts create a deploy key (`~/.ssh/wpg_deploy_ed25519`, on Windows `%USERPROFILE%\.ssh\wpg_deploy_ed25519`, no passphrase) and append its public key to `~/.ssh/authorized_keys` on the host. The password is passed by `sshpass` (deploy.sh, when installed) or a temporary `SSH_ASKPASS` helper (OpenSSH 8.4 and later; the password travels in an environment variable, never in a file, and the helper is deleted right away); if neither works, ssh itself asks for the password once more. To revoke the key, delete the `wpg-deploy@...` line from `~/.ssh/authorized_keys` on the host.
- **The sudo and admin passwords go through the standard input of ssh** (`sudo -S`, `cat > /tmp/wpg-deploy.*/password`): they never appear on a command line, in the process list or on the local disk. The saved answers file contains no passwords.
- The host key of a new host is accepted automatically (`StrictHostKeyChecking=accept-new`; a changed key is still refused). Outside a lab, connect once with `ssh` first and check the fingerprint.
- The admin password is stored on the host as `/etc/wpg/password` (`0640 root:wpg`) and hashed by wpg at startup.
- Keep the management interface out of the TX interfaces (the wizard's default does): test traffic on it can cut your SSH session.

## Web terminal and SNMP trap receiver

When enabled in the wizard (`--terminal` / `--snmp-trap`, `-Terminal` / `-SnmpTrap`), these lines are written into `/etc/wpg/wpg.toml`:

```toml
[terminal]
enabled = true
allowed_targets = []

[snmp]
trap_listen = "0.0.0.0:162"
```

- Only into a `wpg.toml` **created by this deployment**. An existing `wpg.toml` is never changed; the lines to add and `sudo systemctl restart wpg` are printed instead.
- An older wpg refuses to start with keys it does not know, so they are only written when `wpg config validate` of the installed binary accepts them; otherwise a warning is printed and they are skipped.
- With `trap_listen` set, a drop-in (`/etc/systemd/system/wpg.service.d/bind-service.conf`, `CAP_NET_BIND_SERVICE`) lets the service bind the privileged UDP port 162. With ufw active, `sudo ufw allow 162/udp` is suggested.

## What ends up on the host

| Path | What | Rewritten by a deploy |
|---|---|---|
| `/usr/local/bin/wpg`, `wpg.prev` | Current and previous binary | yes |
| `/usr/local/share/wpg/templates/` | Sample templates (template library) | yes |
| `/etc/systemd/system/wpg.service` | The unit from `packaging/systemd/wpg.service` | yes |
| `/etc/systemd/system/wpg.service.d/` | `allow-insecure.conf` (`--allow-insecure`), `bind-service.conf` (SNMP traps) | yes |
| `/etc/wpg/wpg.env` | `WPG_SERVER__BIND`, `WPG_TX__ALLOWED_IFACES`, library path, password file | yes |
| `/etc/wpg/wpg.toml` | Full configuration (`wpg config init`) | **no** (created once) |
| `/etc/wpg/password` | Admin password | only when given |
| `/var/lib/wpg/` | Data directory: database, user templates, TLS certificate | no |
| `~/wpg-build/`, `~/.cargo/`, `~/.rustup/` (SSH user) | Source, build cache and Rust toolchain when building on the host | when building |

Environment variables win over `wpg.toml`: change the `wpg.env` keys through the deploy options and everything else in `wpg.toml`. See [Running as a service](services.md) for the unit itself.

## `--dpdk`: host preparation only

> `--dpdk` only prepares the host. The `dpdk` backend also needs a binary built with `--features dpdk` (`scripts/build.sh --dpdk`; together with `--build-on-host` the host builds with `--features dpdk`), the NIC bound to `vfio-pci`, and EAL arguments in `tx.dpdk_eal_args`. See [DPDK / AF_XDP setup](dpdk-afxdp.md#dpdk).

With `--dpdk` the script:

- reserves `--hugepages` 2 MiB hugepages (default 1024 = 2 GiB) now and at boot (`/etc/sysctl.d/80-wpg-hugepages.conf`), and mounts `hugetlbfs` on `/dev/hugepages` if nothing is mounted;
- loads `vfio-pci` now and at boot (`/etc/modules-load.d/wpg-dpdk.conf`);
- warns when there are no IOMMU groups (typical for CML2 VMs without a virtual IOMMU).

It does not bind NICs to `vfio-pci`: that takes the interface away from the kernel and can cut your SSH session. Use `dpdk-devbind.py` by hand; see [DPDK / AF_XDP setup](dpdk-afxdp.md).

## Upgrades and rollback

Running the same deployment again (in the wizard: Enter at "Use the previous settings?") upgrades: the new binary replaces the old one, the service restarts, configuration and data stay. See [Upgrading](upgrade.md). If the new version misbehaves, choose "Roll back" in the wizard's menu, or:

```sh
scripts/deploy.sh ubuntu@192.0.2.10 --rollback
```

## Troubleshooting

| Symptom | Fix |
|---|---|
| `ssh / scp not found` (Windows) | Add the "OpenSSH Client" optional feature (elevated PowerShell: `Add-WindowsCapability -Online -Name OpenSSH.Client~~~~0.0.1.0`). |
| Cannot connect / timeout | Check address, port and user; try `ssh -p 22 user@host` by hand. On CML2 the node needs the management link (External Connector). |
| `could not install the public key` | Wrong password, or password authentication is disabled on the server (`PasswordAuthentication no`). Choose an existing key file, or install a key on the host yourself. |
| `key login fails` | Public key authentication is disabled on the server, or `~/.ssh` is too permissive (`chmod 700 ~/.ssh; chmod 600 ~/.ssh/authorized_keys`). |
| `sudo failed` | Wrong sudo password, or the user is not in the sudo group (`sudo usermod -aG sudo <user>`). |
| Failure in the dependency step | Check that the host reaches the internet (behind a proxy: configure it in `/etc/apt/apt.conf.d/` and `~/.bashrc`) and that apt is not locked by automatic updates; retry after a few minutes. |
| `killed` / `signal: 9` in the build step | Out of memory: add swap (`sudo fallocate -l 4G /swapfile && sudo chmod 600 /swapfile && sudo mkswap /swapfile && sudo swapon /swapfile`) and run again. |
| The build keeps failing | Run `rm -rf ~/wpg-build` on the host and deploy again. |
| `the uploaded binary does not run on this host` | The binary does not match the host's glibc / CPU; build on the host instead. |
| Health check fails | `sudo journalctl -u wpg -n 50` on the host; `sudo ss -ltnp` for a port already in use. Without an admin password the server does not start on a non-loopback address. |
| The browser cannot connect | Check the firewall (`sudo ufw allow 8080/tcp`), the CML2 external connection, and the listen address (`127.0.0.1` needs an SSH tunnel). |
| `does not support [terminal]` / `[snmp]` | The installed wpg predates the feature; deploy a newer source. |
| Garbled Japanese (Windows) | If you edited `deploy.ps1`, save it as UTF-8 with BOM. |

## Verification status

`deploy.sh` was run against an Ubuntu 24.04 container (sshd, a minimal `systemctl` stand-in instead of systemd): the wizard (key installation with the password through both `sshpass` and `SSH_ASKPASS`, the sudo password, the interface selection, saving and reusing the answers, the rollback menu), building on the host (this checkout uploaded, rustup and trunk installed, through to the health check), and writing `[terminal]` / `[snmp]` into a new `wpg.toml` (with a stub standing in for a binary that knows the keys) while leaving an existing `wpg.toml` alone. The earlier checks in a container running systemd (installation, refusal without a password, `--service` with the health check, `--rollback`, `--dpdk`) still stand. **`deploy.ps1` / `deploy.bat` have not been run on Windows** (CI only parses them and checks the PowerShell 5.1 syntax). Not tested on CML2 nodes or real hardware yet.
