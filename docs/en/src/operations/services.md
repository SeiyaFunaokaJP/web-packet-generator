# Running as a service

Status: draft

Samples live under `packaging/` in the repository and in every release archive. All of them run `wpg serve` with the configuration in a fixed place (`WPG_CONFIG`) and a fixed data directory (`WPG_DATA_DIR`). Create the configuration first with `wpg config init --path <file>` and set a password with `wpg config set-password --config <file>`; a server that listens on a non-loopback address refuses to start without one.

## Linux: systemd

`packaging/systemd/wpg.service` is what [`scripts/deploy.sh --service`](deploy.md) installs. By hand:

```sh
sudo useradd --system --home-dir /var/lib/wpg --shell /usr/sbin/nologin wpg
sudo install -m 0755 wpg /usr/local/bin/wpg
sudo install -d -m 0750 -o root -g wpg /etc/wpg
sudo wpg config init --path /etc/wpg/wpg.toml          # then edit bind, allowed_ifaces, ...
sudo wpg config set-password --config /etc/wpg/wpg.toml
sudo chgrp wpg /etc/wpg/wpg.toml && sudo chmod 0640 /etc/wpg/wpg.toml
sudo install -m 0644 packaging/systemd/wpg.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now wpg
journalctl -u wpg -f
```

Points of the unit:

- Runs as the unprivileged user `wpg`. Raw sockets come from `AmbientCapabilities=CAP_NET_RAW CAP_NET_ADMIN`, so the binary needs no `setcap` for the service (file capabilities are ignored under `NoNewPrivileges=yes` anyway).
- `StateDirectory=wpg` creates `/var/lib/wpg` owned by `wpg`; `ProtectSystem=strict` makes the rest of the file system read-only for the server.
- `EnvironmentFile=-/etc/wpg/wpg.env` for per-host `WPG_*` overrides (deploy.sh writes it).
- `LimitMEMLOCK=infinity` for the AF_XDP backend (UMEM is charged to `RLIMIT_MEMLOCK` on kernels before 5.11).
- `RestrictAddressFamilies` allows `AF_PACKET` and `AF_XDP` besides IP and Unix sockets.

Change the listen address with `bind` in `wpg.toml` or `WPG_SERVER__BIND=` in `wpg.env`, then `sudo systemctl restart wpg`.

## macOS: launchd

`packaging/launchd/io.github.seiyafunaokajp.wpg.plist` is a system daemon (it runs as root, which `/dev/bpf*` needs unless ChmodBPF is set up, see [Permissions](../getting-started/permissions.md)):

```sh
sudo install -m 0755 wpg /usr/local/bin/wpg
sudo mkdir -p /usr/local/etc/wpg /usr/local/var/wpg /usr/local/var/log
sudo /usr/local/bin/wpg config init --path /usr/local/etc/wpg/wpg.toml
sudo /usr/local/bin/wpg config set-password --config /usr/local/etc/wpg/wpg.toml
sudo install -m 0644 packaging/launchd/io.github.seiyafunaokajp.wpg.plist /Library/LaunchDaemons/
sudo launchctl bootstrap system /Library/LaunchDaemons/io.github.seiyafunaokajp.wpg.plist
tail -f /usr/local/var/log/wpg.log
```

Stop and remove it with `sudo launchctl bootout system/io.github.seiyafunaokajp.wpg`. `KeepAlive` restarts the server when it exits with an error. Remove the quarantine attribute from a downloaded binary first, or launchd cannot start it (see [Install on macOS](../getting-started/install-macos.md#release-binaries-gatekeeper-and-quarantine)).

## Windows

`wpg.exe` is a console program and does not talk the Service Control Manager protocol. A plain `sc.exe create wpg binPath= "C:\...\wpg.exe serve"` therefore registers a service that fails to start with error 1053. Use one of these instead.

### Scheduled task (built in)

`packaging/windows/wpg-service.ps1` registers a task that runs `wpg.exe serve` as `SYSTEM` at boot and restarts it every minute if it stops. From an elevated PowerShell:

```powershell
.\packaging\windows\wpg-service.ps1 -Action Install -Exe "C:\Program Files\wpg\wpg.exe"
.\packaging\windows\wpg-service.ps1 -Action Status
.\packaging\windows\wpg-service.ps1 -Action Stop
.\packaging\windows\wpg-service.ps1 -Action Uninstall
```

It creates `C:\ProgramData\wpg\wpg.toml` (with `wpg config init`) when missing and keeps the data in `C:\ProgramData\wpg\data`. Set the password with `& "C:\Program Files\wpg\wpg.exe" config set-password --config C:\ProgramData\wpg\wpg.toml` before binding to a non-loopback address. The script warns when Npcap is missing.

### A real service through a wrapper

To manage wpg with `sc.exe` / `services.msc`, wrap it with [WinSW](https://github.com/winsw/winsw) or [NSSM](https://nssm.cc/). With NSSM:

```powershell
nssm install wpg "C:\Program Files\wpg\wpg.exe" serve
nssm set wpg AppEnvironmentExtra WPG_CONFIG=C:\ProgramData\wpg\wpg.toml WPG_DATA_DIR=C:\ProgramData\wpg\data
nssm set wpg AppDirectory C:\ProgramData\wpg\data
sc.exe start wpg
sc.exe query wpg
```

Npcap must be installed in either case (`wpg doctor` checks it). Allow `wpg.exe` through Windows Defender Firewall only on private networks, and only if it listens on a non-loopback address.

## Verification status

The systemd unit was started by `scripts/deploy.sh` in an Ubuntu 24.04 container running systemd (the server ran as `wpg` with only `CAP_NET_RAW` and `CAP_NET_ADMIN` and answered `/api/health`); `systemd-analyze verify` accepts it. The launchd plist was only checked to parse; the PowerShell script was only parsed and linted with PSScriptAnalyzer. **Not tested on real macOS or Windows yet.** The release workflow now has jobs that install the launchd daemon on a macOS 14 runner and the scheduled task on a Windows runner and check `/api/health` (see "What CI checks" in [Release process](../dev/release.md)); this page will be updated once they have run green.
