# Deploying (deploy.bat / deploy.ps1 / deploy.sh)

Status: draft

These scripts install wpg on a Linux host (Ubuntu 20.04 to 24.04, Debian; first target: an Ubuntu node on CML2) over SSH. From Windows use `scripts\deploy.bat` (a wrapper around `scripts\deploy.ps1`), from Linux / macOS `scripts/deploy.sh`. Both run the same host-side scripts (`scripts/lib/*.sh`), so the result is the same.

**There are no arguments to remember.** Started without arguments, a wizard asks for everything it needs, one question at a time, in English or Japanese (from the system language). Every question has a default in `[ ]` that Enter accepts.

**It can run unattended too.** With the answer file the wizard offers to save at the end, the same deploy runs without any question (scheduled upgrades, several hosts, CI); see [Unattended deploy](#unattended-deploy).

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

The answers (never the passwords) are saved to `%APPDATA%\wpg\deploy.conf` on Windows and `~/.config/wpg/deploy.conf` on Linux / macOS. The next run starts with "Use the previous settings?"; Enter repeats the last deployment and only the passwords are asked (the SSH password is not needed any more, the key works). After a successful deploy the wizard offers to save a file that runs the same deploy without questions ([Unattended deploy](#unattended-deploy)).

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
| `--no-service` | Do not install the service (overrides `service=1` of `--config`). |
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
| `--dhcp-iface <name>` | Pin the interface the DHCP server answers on (`WPG_DHCP__IFACE` in `/etc/wpg/wpg.env`; with `--service`). See [DHCP server](dhcp.md). |
| `--dhcp-iface-addr <addr/len>` | Static addresses for `--dhcp-iface` (repeatable or comma separated, e.g. `192.168.100.1/24,2001:db8:1::1/64`): written to `/etc/netplan/90-wpg-<iface>.yaml` and applied with `netplan apply`. Refused for an interface carrying the default route. |
| `--no-templates` | Do not copy the sample templates to `/usr/local/share/wpg/templates`. |
| `--no-healthcheck` | Skip the health check. |
| `--rollback` | Swap `/usr/local/bin/wpg` with `wpg.prev` and restart the service. Running it twice undoes it. |
| `--dpdk`, `--hugepages <n>` | Prepare the host for DPDK (see below). |
| `--profile <file>`, `--lang <ja\|en>`, `--log <file>` | Wizard answers file, message language, log file. |
| `--dry-run` | Print the commands instead of running them. |
| `--config`, `--yes`, `--json`, `--skip-if-current`, `--check-config`, `--targets`, `--section`, `--parallel`, `--set` | Unattended runs ([Unattended deploy](#unattended-deploy)). |

### deploy.ps1

Everything the wizard asks can also be passed as parameters. With `-HostName` the wizard does not start (only missing passwords are asked; with `-Yes` nothing is asked and a missing one fails at once).

```powershell
# Build on the host and install the service, without questions (sudo password from WPG_DEPLOY_SUDO_PASSWORD, ...)
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
| `-DhcpIface`, `-DhcpIfaceAddr` | DHCP server interface and static addresses (as `--dhcp-iface` / `--dhcp-iface-addr`). |
| `-Rollback`, `-NoTemplates`, `-NoHealthcheck` | As in deploy.sh. |
| `-ProfilePath`, `-Lang ja\|en`, `-LogFile` | Answers file, message language, log file. |
| `-Interactive`, `-Yes`, `-DryRun` | Start the wizard with the parameters as defaults / never ask anything (no confirmation, no password) / only connect and check, without building or installing. |
| `-Prefix`, `-AllowInsecure` | Install prefix on the host, start without a password (as in deploy.sh). |
| `-ConfigFile`, `-Json`, `-SkipIfCurrent`, `-CheckConfig`, `-Targets`, `-Section`, `-Parallel`, `-Set` | Unattended runs ([Unattended deploy](#unattended-deploy)). |

## Unattended deploy

Use the wizard when you deploy by hand. To repeat a known setup (scheduled upgrades, several hosts, CI), use **an answer file plus `--yes`**: the scripts then run to the end without a single question.

### The easy way: let the wizard write it

At the end of a successful wizard run the scripts ask:

```text
Save a file to run this deploy unattended (automatically) from now on? (no passwords) [Y/n]:
File [C:\Users\you\AppData\Roaming\wpg\deploy-192.0.2.10.conf]:
Save the sudo password encrypted? (Windows DPAPI: only this Windows user on this PC can decrypt it) [Y/n]:
Saved C:\Users\you\AppData\Roaming\wpg\deploy-192.0.2.10.conf (no plain-text passwords).
From now on this one line runs the same deploy without questions:
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File "C:\work\web-packet-generator\scripts\deploy.ps1" -ConfigFile "C:\Users\you\AppData\Roaming\wpg\deploy-192.0.2.10.conf" -Yes -SkipIfCurrent
Scheduled run, e.g. Task Scheduler every Sunday at 3:17 (logs stay in %LOCALAPPDATA%\wpg\deploy-logs):
  schtasks /Create /TN "wpg deploy 192.0.2.10" /SC WEEKLY /D SUN /ST 03:17 /TR "..."
```

Copy the printed line as it is (deploy.sh prints `scripts/deploy.sh --config ~/.config/wpg/deploy-192.0.2.10.conf --yes --skip-if-current` and a cron line). SSH uses the deploy key the wizard installed, so no SSH password is needed. When sudo needs a password, Windows can keep it DPAPI-encrypted (the question above); on Linux / macOS use an environment variable or a password file (see below).

### The answer file

Same format as the wizard's `deploy.conf` (`key=value` lines, `#` starts a comment); you can also write it by hand. Both scripts read the same file.

```ini
# deploy.wpg.conf
host=192.0.2.10
user=cisco
binary_source=build
source=git
git_ref=main
bind=0.0.0.0:8080
ifaces=ens3,ens4
service=1
sudo_password_file=/home/you/.config/wpg/sudo.pw
```

```sh
scripts/deploy.sh --config deploy.wpg.conf --yes                       # Linux / macOS
scripts/deploy.sh --config deploy.wpg.conf --check-config              # only show the settings (no connection)
```

```powershell
.\scripts\deploy.ps1 -ConfigFile .\deploy.wpg.conf -Yes                 # Windows
```

Relative paths in the file (`binary_file`, `identity`, `*_password_file`, ...) are relative to the answer file. Command-line options win over the file (`--set key=value` / `-Set "key=value"` overrides any key). An unknown key is an error (exit code 1), so typos do not go unnoticed.

| Key | Default | Meaning |
|---|---|---|
| `host` | (required) | Target host; `user@host` works too. |
| `user` | `ubuntu` | SSH user. |
| `ssh_port` (`port`) | `22` | SSH port. |
| `identity` | (none) | SSH private key. Without it: the deploy key (`wpg_deploy_ed25519`), then ssh-agent / your default keys. |
| `binary_source` | `build` | `build` (on the host), `file` (upload `binary_file`), `release` (GitHub Release); deploy.sh also has `local` (build on this machine). |
| `binary_file` | | Linux binary for `binary_source=file`. |
| `source` | `local` | Source for `build`: `local` (upload this checkout) or `git`. |
| `git_url`, `git_ref` | GitHub, `main` | Repository and branch / tag / commit for `source=git`. |
| `release_tag` | `latest` | Tag for `binary_source=release`. |
| `bind` | `0.0.0.0:8080` | Listen address. |
| `ifaces` | (all) | Interfaces jobs may send on (comma separated). |
| `service` | `1` | Install the systemd service. |
| `terminal`, `snmp_trap` | `0` | Web terminal / SNMP trap receiver. |
| `dhcp_iface`, `dhcp_iface_addr` | empty | DHCP server interface and static addresses (comma separated). |
| `templates`, `healthcheck` | `1` | Copy the sample templates / run the health check. |
| `allow_insecure` | `0` | Listen on a non-loopback address without a password. |
| `prefix` | `/usr/local` | Install prefix on the host. |
| `skip_if_current` | `0` | Same as `--skip-if-current`. |
| `dpdk`, `hugepages` | `0`, `1024` | DPDK host preparation (deploy.sh only). |
| `admin_password_file`, `ssh_password_file`, `sudo_password_file` | | A file you create with the password on its first line. `admin_password_file` may hold an argon2 hash instead. |
| `admin_password_dpapi`, `ssh_password_dpapi`, `sudo_password_dpapi` | | DPAPI-encrypted file (deploy.ps1 on Windows only; written by the wizard). |
| `name` | section / host | Name in the result table and the JSON. |
| `parallel` | `1` | Several hosts: how many at a time. |
| `lang` | from the environment | `ja` / `en`. |

Booleans are `1` / `0` (`yes` / `no`, `true` / `false` work too).

### Secrets without questions

| Source | SSH | sudo | Admin (web UI) |
|---|---|---|---|
| SSH key (recommended) | deploy key / `identity=` / ssh-agent | - | - |
| Environment | `WPG_DEPLOY_SSH_PASSWORD` | `WPG_DEPLOY_SUDO_PASSWORD` | `WPG_DEPLOY_ADMIN_PASSWORD` |
| File you create (`chmod 600`) | `ssh_password_file=` | `sudo_password_file=` | `admin_password_file=` |
| DPAPI file (Windows) | `ssh_password_dpapi=` | `sudo_password_dpapi=` | `admin_password_dpapi=` |

- Searched top to bottom. In deploy.ps1 the `SecureString` parameters (`-SshPassword`, ...) come first.
- The SSH password is only used when no key works, and then installs the deploy key (not needed next time). Without a sudo password the SSH password is tried. `NOPASSWD` sudo or a root login need none.
- The admin password is only required for a first install that listens on a non-loopback address (`0.0.0.0:8080`, ...). On later runs leaving it out keeps the current one (setting it rewrites it every time).
- **The scripts never write a password to a file in plain text.** A DPAPI file is the output of `ConvertFrom-SecureString` (no key): only the Windows user who made it can decrypt it, on that PC. Run the scheduled task as that user.
- A missing secret fails at once without asking: the message names the variable / key to set, and the exit code is 2 (SSH / sudo) or 1 (admin password). Even without `--yes` / `-Yes` nothing is asked when the standard input is not a terminal (cron, a pipe, `-NonInteractive`): the scripts say to add `--yes` and exit with 1.

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Success; also nothing to do with `--skip-if-current`, `--dry-run` and `--check-config`. |
| 1 | Usage error, bad answer file, missing setting or admin password, a question needed without `--yes`. |
| 2 | Connection / SSH authentication / sudo failed (including a missing SSH or sudo password). |
| 3 | Preparing the binary failed (local build, Release download, build dependencies or build on the host). |
| 4 | Upload or installation failed. |
| 5 | Health check failed (the installation itself is done). |
| 6 | Several hosts: at least one failed (each host's code is in the table and the JSON). |
| 130 | Interrupted, e.g. Ctrl+C (deploy.sh). |

### `--json` / `-Json`

Only the result goes to the standard output, as JSON (the log goes to the standard error and the log file). One host or several, the shape is the same:

```json
{"tool":"wpg-deploy","format":1,"status":"ok","exit_code":0,"dry_run":false,"message":"",
 "hosts":[{"name":"lab1","host":"192.0.2.10","user":"cisco","port":22,"status":"ok","exit_code":0,
  "failed_step":"","message":"","version_before":"wpg 0.1.0","version_after":"wpg 0.1.0",
  "source_id":"git:24d946f6614eecbff02ca481df380aafeb63f368","url":"http://192.0.2.10:8080/",
  "dry_run":false,"log":"/home/you/.local/state/wpg/deploy-20261027-031700-192.0.2.10.log",
  "started_at":"2026-10-27T03:17:00Z","duration_s":41}]}
```

A host's `status` is `ok`, `skipped` (already current, nothing done), `dry-run`, `checked`, `aborted` or `failed`. On a failure `failed_step` (`config`, `connect`, `sudo`, `binary`, `deps`, `build`, `upload`, `install`, `health`) and `message` say what went wrong.

### `--skip-if-current` / `-SkipIfCurrent`: nothing to do, nothing done

Every deploy records what it installed with which settings in `/usr/local/share/wpg/deploy.stamp` on the host (source id, settings, SHA-256 of the binary, time; no passwords). With `--skip-if-current` the scripts connect, and when all of the following match they stop without building or installing anything (exit code 0, `status` `skipped`), without even using the sudo password:

- The same source id: for `source=git` the commit the branch / tag points to (resolved with `git ls-remote`), for `source=local` the commit of this checkout (plus a hash of uncommitted changes), for `binary_source=file` the file's SHA-256, for `release` the URL of the release archive.
- The same settings (`bind`, `ifaces`, `service`, `terminal`, `snmp_trap`, `templates`, `allow_insecure`, `prefix`).
- The installed binary is the recorded one (after a rollback or a manual replacement the scripts deploy again).
- With a service: `wpg.service` runs and answers the health check.

`wpg --version` carries no commit, hence the record file. When the source cannot be identified (no Git, the ref does not resolve) the scripts warn and deploy as usual. Re-installing an identical binary no longer overwrites `wpg.prev`, so the previous version stays available for a rollback.

### `--dry-run` / `-DryRun` without questions

Together with `--yes` the scripts connect, read the host facts, check that every needed secret is there (the sudo password is really tried) and print the commands they would run. Nothing on the host changes (not even the key installation). Good for checking a setup from CI. To check only the answer file, without connecting, use `--check-config` / `-CheckConfig`.

### Several hosts

`[host <name>]` sections in the answer file deploy to one host each. Keys before the first section apply to all hosts; a section's own values win (without `host=` the section name is the host name).

```ini
# lab.conf
user=cisco
binary_source=build
source=git
git_ref=v0.2.0
sudo_password_file=sudo.pw

[host r1]
host=192.0.2.11
ifaces=ens3

[host r2]
host=192.0.2.12
ifaces=ens3,ens4
bind=127.0.0.1:8080
```

The hosts can also come from a separate list (`--targets` / `-Targets`: one host per line, `[user@]host[:port]` followed by optional `key=value` pairs; relative paths are relative to that file):

```text
# hosts.txt
cisco@192.0.2.11 ifaces=ens3
cisco@192.0.2.12:2222 ifaces=ens3,ens4
```

```sh
scripts/deploy.sh --config lab.conf --yes --parallel 2
scripts/deploy.sh --config common.conf --targets hosts.txt --yes --json > result.json
```

```powershell
.\scripts\deploy.ps1 -ConfigFile .\lab.conf -Yes -Parallel 2
```

Each host runs in its own process (`--parallel` / `-Parallel` sets how many at a time, default 1); per-host logs stay in `~/.local/state/wpg/deploy-<time>/` (Windows: `%LOCALAPPDATA%\wpg\deploy-logs\deploy-<time>\`). At the end a table is printed, and the exit code is 6 if any host failed.

```text
Result
  NAME HOST                     STATUS    EXIT VERSION      DETAIL
  r1   cisco@192.0.2.11         ok        0    0.2.0        git:5d0c1e2...
  r2   cisco@192.0.2.12         skipped   0    0.2.0        git:5d0c1e2...
  r3   cisco@192.0.2.13         failed    2                 cannot connect to cisco@192.0.2.13 (port 22): ssh: connect to host ...
2 succeeded, 1 failed
```

### Scheduled runs

#### Windows (Task Scheduler)

Paste the `schtasks` line the wizard prints into a command prompt. By hand it looks like this (add `/RU <user> /RP` to run while logged off; with a DPAPI file, run the task as the user who created the file):

```bat
schtasks /Create /TN "wpg deploy lab" /SC WEEKLY /D SUN /ST 03:17 ^
  /TR "powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\work\web-packet-generator\scripts\deploy.ps1 -ConfigFile C:\Users\you\AppData\Roaming\wpg\lab.conf -Yes -SkipIfCurrent"
```

Task Scheduler's "Last Run Result" shows the exit code; the logs are in `%LOCALAPPDATA%\wpg\deploy-logs\`.

#### Linux / macOS (cron)

```sh
# crontab -e
17 3 * * 0 /home/you/web-packet-generator/scripts/deploy.sh --config /home/you/.config/wpg/lab.conf --yes --skip-if-current >/dev/null 2>&1
```

When sudo needs a password, put `sudo_password_file=` (a file you create, `chmod 600`) into the answer file, or give the deploy user `NOPASSWD` on the host (environment variables are awkward in cron).

#### Linux (systemd user timer)

```ini
# ~/.config/systemd/user/wpg-deploy.service
[Unit]
Description=wpg unattended deploy

[Service]
Type=oneshot
ExecStart=%h/web-packet-generator/scripts/deploy.sh --config %h/.config/wpg/lab.conf --yes --skip-if-current
```

```ini
# ~/.config/systemd/user/wpg-deploy.timer
[Timer]
OnCalendar=Sun 03:17
Persistent=true

[Install]
WantedBy=timers.target
```

```sh
systemctl --user daemon-reload && systemctl --user enable --now wpg-deploy.timer
journalctl --user -u wpg-deploy      # results
```

### From CI (GitHub Actions example)

Keep the SSH key and the sudo password in repository secrets. Putting the host's public key into `known_hosts` avoids accepting it blindly on the first connection.

```yaml
jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: deploy to the lab
        env:
          WPG_DEPLOY_SUDO_PASSWORD: ${{ secrets.LAB_SUDO_PASSWORD }}
          WPG_DEPLOY_ADMIN_PASSWORD: ${{ secrets.LAB_ADMIN_PASSWORD }}
        run: |
          install -m 700 -d ~/.ssh
          printf '%s\n' "${{ secrets.LAB_SSH_KEY }}" > ~/.ssh/lab_ed25519 && chmod 600 ~/.ssh/lab_ed25519
          printf '%s\n' "${{ secrets.LAB_KNOWN_HOSTS }}" >> ~/.ssh/known_hosts
          scripts/deploy.sh --config deploy/lab.conf --yes --skip-if-current --json \
            --set identity=$HOME/.ssh/lab_ed25519 --set source=git --set git_ref=${{ github.sha }} > deploy-result.json
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: deploy-result
          path: deploy-result.json
```

### Options for unattended runs

| deploy.sh | deploy.ps1 | Meaning |
|---|---|---|
| `--config <file>` | `-ConfigFile <file>` | Answer file. |
| `--yes` | `-Yes` | Never ask anything. |
| `--json` | `-Json` | JSON result on the standard output. |
| `--skip-if-current` | `-SkipIfCurrent` | Do nothing when the host is current. |
| `--dry-run` | `-DryRun` | Check and print the commands, change nothing. |
| `--check-config` | `-CheckConfig` | Check the answer file and print the settings (no connection). |
| `--targets <file>` | `-Targets <file>` | File with the hosts. |
| `--section <name>` | `-Section <name>` | Only one section of the answer file. |
| `--parallel <n>` | `-Parallel <n>` | Hosts deployed at a time. |
| `--set <key=value>` | `-Set "key=value"` | Override an answer-file key (repeatable). |
| `--log <file>` | `-LogFile <file>` | Log file (default: `~/.local/state/wpg/`, `%LOCALAPPDATA%\wpg\deploy-logs\`). |

## Security

- **The SSH password is used once, to install a key.** The scripts create a deploy key (`~/.ssh/wpg_deploy_ed25519`, on Windows `%USERPROFILE%\.ssh\wpg_deploy_ed25519`, no passphrase) and append its public key to `~/.ssh/authorized_keys` on the host. The password is passed by `sshpass` (deploy.sh, when installed) or a temporary `SSH_ASKPASS` helper (OpenSSH 8.4 and later; the password travels in an environment variable, never in a file, and the helper is deleted right away); if neither works, ssh itself asks for the password once more. To revoke the key, delete the `wpg-deploy@...` line from `~/.ssh/authorized_keys` on the host.
- **The sudo and admin passwords go through the standard input of ssh** (`sudo -S`, `cat > /tmp/wpg-deploy.*/password`): they never appear on a command line, in the process list or on the local disk. The saved answers file contains no passwords.
- The host key of a new host is accepted automatically (`StrictHostKeyChecking=accept-new`; a changed key is still refused). Outside a lab, connect once with `ssh` first and check the fingerprint.
- The admin password is stored on the host as `/etc/wpg/password` (`0640 root:wpg`) and hashed by wpg at startup.
- Unattended runs read passwords from the environment, from files you create, or on Windows from DPAPI-encrypted files. The scripts never store a password in plain text ([Unattended deploy](#unattended-deploy)).
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
| `/usr/local/share/wpg/deploy.stamp` | Record of the last deployed source and settings (for `--skip-if-current`; no passwords) | yes |
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

`deploy.sh` was run against an Ubuntu 24.04 container (sshd, a minimal `systemctl` stand-in instead of systemd): the wizard (key installation with the password through both `sshpass` and `SSH_ASKPASS`, the sudo password, the interface selection, saving and reusing the answers, the rollback menu), building on the host (this checkout uploaded, rustup and trunk installed, through to the health check), and writing `[terminal]` / `[snmp]` into a new `wpg.toml` (with a stub standing in for a binary that knows the keys) while leaving an existing `wpg.toml` alone. The earlier checks in a container running systemd (installation, refusal without a password, `--service` with the health check, `--rollback`, `--dpdk`) still stand. The unattended mode was checked with `deploy.sh` against the same Ubuntu 24.04 container (two sshds, each in its own mount namespace separating `/usr/local`, `/etc/wpg` and `/var/lib/wpg`): a first install from an answer file with `--yes` (the deploy key installed with the SSH password from the environment, sudo and admin passwords from the environment, the sudo password also from a file), failing at once without asking when a secret is missing (no key and no SSH password: 2, sudo password missing / wrong: 2, no admin password on a first install: 1, no `--yes` with a non-terminal stdin: 1), `--skip-if-current` (skipped when nothing changed, even without a sudo password; deployed again after a new binary, changed settings or a rollback), `--dry-run`, `--check-config`, the `--json` output (validated with `python3 -m json.tool`), several hosts (three sections, one unreachable: per-host results in the table and the JSON, exit code 6; `--parallel 2`; `--targets`), and saving the unattended file at the end of the wizard plus a rerun with it. **`deploy.ps1` / `deploy.bat` have not been run on Windows** (CI only parses them and checks the PowerShell 5.1 syntax; the unattended parts are untested on Windows too). Not tested on CML2 nodes or real hardware yet.
