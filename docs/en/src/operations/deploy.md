# Deploying with `scripts/deploy.sh`

Status: draft

`scripts/deploy.sh` copies a wpg build to a Linux host over SSH and installs it. It is written for an Ubuntu node on CML2 first, but works on any systemd-based distribution. One run does all of this:

1. builds `target/release/wpg` (with the web UI) when it is missing, or takes `--binary`;
2. uploads the binary, the sample templates and the systemd unit with `scp`;
3. installs the binary to `/usr/local/bin/wpg` (keeping the previous one as `wpg.prev`) and runs `setcap cap_net_raw,cap_net_admin+eip` on it (installs `libcap2-bin` first if `setcap` is missing and `apt-get` exists);
4. with `--service`: creates the `wpg` system user, `/etc/wpg/wpg.toml` (only if missing), `/etc/wpg/wpg.env`, installs `wpg.service` and (re)starts it;
5. prints `wpg doctor` from the host and, with `--service`, polls `GET /api/health` until the server answers (30 s), showing the last journal lines if it does not.

The remote account needs `sudo`; you are asked for its password when needed.

## Examples

```sh
# Build, upload, setcap only (run `wpg send` / `wpg serve` yourself on the host)
scripts/deploy.sh ubuntu@192.0.2.10

# Install as a service that listens on all addresses, allows sending only on ens3
scripts/deploy.sh ubuntu@192.0.2.10 --service --iface ens3 --password-file ./admin-password

# The same through the task runner
just deploy ubuntu@192.0.2.10

# Show what would run, without touching anything
scripts/deploy.sh ubuntu@192.0.2.10 --service --dry-run
```

`--password-file` takes a local file whose first line is the admin password in plain text or its argon2id hash. It is installed as `/etc/wpg/password` (mode `0640`, group `wpg`). Without it the script refuses to start a server on a non-loopback address unless the host already has a password (`/etc/wpg/password`, `password_hash` / `password_file` in `/etc/wpg/wpg.toml`, or an existing user database). Alternatives: `--bind 127.0.0.1:8080` and an SSH tunnel (`ssh -L 8080:127.0.0.1:8080 ubuntu@192.0.2.10`), or `--allow-insecure` for an isolated lab network only.

## Options

| Option | Meaning |
|---|---|
| `--iface <name>` | Interface jobs may send on (repeatable). Written as `WPG_TX__ALLOWED_IFACES`. Do not list the management interface. |
| `--service` | Install / update `wpg.service` and restart it. |
| `--bind <addr:port>` | Listen address of the service (default `0.0.0.0:8080`). |
| `--password-file <file>` | Admin password (or argon2 hash) for the service. |
| `--allow-insecure` | Start without a password on a non-loopback address (adds a drop-in that runs `wpg serve --allow-insecure`). |
| `--binary <file>` | Binary to deploy instead of `target/release/wpg`. |
| `--target <triple>` | Build for another target, e.g. `aarch64-unknown-linux-gnu` (needs a cross linker). |
| `--prefix <dir>` | Install prefix on the host (default `/usr/local`). |
| `--port <n>`, `--ssh-opt <opt>` | SSH port and extra `-o` options (e.g. `--ssh-opt IdentityFile=~/.ssh/lab`). |
| `--no-templates` | Do not copy the sample templates to `/usr/local/share/wpg/templates`. |
| `--no-healthcheck` | Skip the health check. |
| `--rollback` | Swap `/usr/local/bin/wpg` and `wpg.prev` and restart the service. Running it twice undoes it. |
| `--dpdk`, `--hugepages <n>` | Prepare the host for DPDK (below). |
| `--dry-run` | Print the commands only. |

The binary must run on the host: a binary built on a newer distribution than the host may fail with a glibc version error. The script checks this before installing (`wpg --version` on the host) and stops. Build on the oldest distribution you deploy to, or use a release archive (built on Ubuntu 22.04).

## What ends up on the host

| Path | Content | Rewritten by deploy.sh |
|---|---|---|
| `/usr/local/bin/wpg`, `wpg.prev` | Current and previous binary | yes |
| `/usr/local/share/wpg/templates/` | Sample templates (template library) | yes |
| `/etc/systemd/system/wpg.service` | Unit from `packaging/systemd/wpg.service` | yes |
| `/etc/wpg/wpg.env` | `WPG_SERVER__BIND`, `WPG_TX__ALLOWED_IFACES`, library path, password file | yes |
| `/etc/wpg/wpg.toml` | Full configuration (`wpg config init`) | **no**, created once |
| `/etc/wpg/password` | Admin password | only with `--password-file` |
| `/var/lib/wpg/` | Data directory: database, user templates, TLS certificate | no |

Environment variables win over `wpg.toml`, so change the keys in `wpg.env` with deploy.sh options and everything else in `wpg.toml`. See [Running as a service](services.md) for the unit itself.

## `--dpdk`: host preparation only

> `--dpdk` only prepares the host. The `dpdk` backend needs a binary built with `--features dpdk` (`scripts/build.sh --dpdk`), a NIC bound to `vfio-pci` and EAL arguments in `tx.dpdk_eal_args`; see [DPDK and AF_XDP setup](dpdk-afxdp.md#dpdk).

With `--dpdk` the script:

- reserves `--hugepages` 2 MiB pages (default 1024 = 2 GiB) now and at boot (`/etc/sysctl.d/80-wpg-hugepages.conf`), and mounts `hugetlbfs` on `/dev/hugepages` if nothing is mounted;
- loads `vfio-pci` now and at boot (`/etc/modules-load.d/wpg-dpdk.conf`);
- warns when there are no IOMMU groups (typical for CML2 VMs without a virtual IOMMU).

It does not bind any NIC to `vfio-pci`: that takes the interface away from the kernel and can cut your SSH session. Do it by hand with `dpdk-devbind.py`, see [DPDK and AF_XDP setup](dpdk-afxdp.md).

## Upgrading and rolling back

Re-running the same command is the upgrade: the new binary replaces the old one, the service restarts, the configuration and data are kept. See [Upgrading](upgrade.md). If the new version misbehaves:

```sh
scripts/deploy.sh ubuntu@192.0.2.10 --rollback
```

## Verification status

Tested against an Ubuntu 24.04 container running systemd and sshd: install, the password refusal, `--service` with the health check, `--rollback` and `--dpdk` (hugepages; `vfio-pci` is not available in a container). **Not yet tested on a CML2 node or real hardware.**
