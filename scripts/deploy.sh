#!/usr/bin/env bash
# Deploy a wpg build to a Linux host over SSH (TODO §2.8; first target: Ubuntu on CML2).
#
# Usage: scripts/deploy.sh <user@host> [options]
#   --iface <name>          interface jobs may send on (repeatable; tx.allowed_ifaces)
#   --service               install / update the systemd unit and (re)start it
#   --bind <addr:port>      listen address of the service (default 0.0.0.0:8080)
#   --password-file <file>  local file with the admin password or its argon2 hash;
#                           installed as /etc/wpg/password (0640 root:wpg)
#   --allow-insecure        start without a password on a non-loopback address
#   --binary <file>         binary to deploy (default: target/<target>/release/wpg,
#                           built with scripts/build.sh --release when missing)
#   --target <triple>       cargo target of the binary (default: host target)
#   --prefix <dir>          install directory on the host (default /usr/local)
#   --port <n>              SSH port
#   --ssh-opt <opt>         extra ssh / scp -o option (repeatable)
#   --no-templates          do not copy the sample templates
#   --no-healthcheck        skip GET /api/health after starting the service
#   --rollback              restore the binary saved by the previous deploy and restart
#   --dpdk                  prepare the host for DPDK (hugepages, vfio-pci). The DPDK
#                           backend itself is NOT implemented yet: this only does the
#                           host preparation, see docs/*/src/operations/dpdk-afxdp.md
#   --hugepages <n>         number of 2 MiB hugepages for --dpdk (default 1024)
#   --dry-run               print the commands instead of running them
#   -h, --help              this help
#
# The remote user needs sudo. Re-running the script upgrades in place: the previous
# binary is kept as <prefix>/bin/wpg.prev, /etc/wpg/wpg.toml is never overwritten.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

usage() { sed -n '2,28p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }
die() { echo "error: $*" >&2; exit 1; }
info() { echo "==> $*"; }

host=""
ifaces=()
service=0
bind="0.0.0.0:8080"
password_file=""
allow_insecure=0
binary=""
target=""
prefix="/usr/local"
ssh_port=""
ssh_opts=()
templates=1
healthcheck=1
rollback=0
dpdk=0
hugepages=1024
dry_run=0

need_value() { [ "$2" -ge 2 ] || die "$1 needs a value"; }

while [ "$#" -gt 0 ]; do
    case "$1" in
        --iface) need_value "$1" "$#"; ifaces+=("$2"); shift ;;
        --service) service=1 ;;
        --bind) need_value "$1" "$#"; bind="$2"; shift ;;
        --password-file) need_value "$1" "$#"; password_file="$2"; shift ;;
        --allow-insecure) allow_insecure=1 ;;
        --binary) need_value "$1" "$#"; binary="$2"; shift ;;
        --target) need_value "$1" "$#"; target="$2"; shift ;;
        --prefix) need_value "$1" "$#"; prefix="$2"; shift ;;
        --port) need_value "$1" "$#"; ssh_port="$2"; shift ;;
        --ssh-opt) need_value "$1" "$#"; ssh_opts+=(-o "$2"); shift ;;
        --no-templates) templates=0 ;;
        --no-healthcheck) healthcheck=0 ;;
        --rollback) rollback=1 ;;
        --dpdk) dpdk=1 ;;
        --hugepages) need_value "$1" "$#"; hugepages="$2"; shift ;;
        --dry-run) dry_run=1 ;;
        -h | --help) usage; exit 0 ;;
        -*) die "unknown option: $1 (see --help)" ;;
        *)
            [ -z "$host" ] || die "unexpected argument: $1"
            host="$1"
            ;;
    esac
    shift
done

[ -n "$host" ] || { usage >&2; exit 2; }
case "$hugepages" in '' | *[!0-9]*) die "--hugepages must be a number" ;; esac
case "$bind" in *:*) ;; *) die "--bind must be <addr>:<port>" ;; esac
port="${bind##*:}"
# Address the health check connects to (on the host itself).
hc_addr="${bind%:*}"
case "$hc_addr" in 0.0.0.0 | "" | "[::]") hc_addr=127.0.0.1 ;; esac
case "$port" in '' | *[!0-9]*) die "--bind must end in a port number" ;; esac
for i in "${ifaces[@]}"; do
    case "$i" in '' | *[!A-Za-z0-9_.:@-]*) die "invalid interface name: $i" ;; esac
done
[ -z "$password_file" ] || [ -r "$password_file" ] || die "cannot read $password_file"

ssh_args=(-o BatchMode=no "${ssh_opts[@]}")
scp_args=("${ssh_opts[@]}")
if [ -n "$ssh_port" ]; then
    ssh_args+=(-p "$ssh_port")
    scp_args+=(-P "$ssh_port")
fi

run() {
    if [ "$dry_run" -eq 1 ]; then
        printf '+'
        printf ' %q' "$@"
        printf '\n'
    else
        "$@"
    fi
}

# ---- binary -----------------------------------------------------------------
if [ "$rollback" -eq 0 ]; then
    if [ -z "$binary" ]; then
        if [ -n "$target" ]; then
            binary="target/$target/release/wpg"
        else
            binary="target/release/wpg"
        fi
        if [ ! -x "$binary" ]; then
            info "$binary missing, building"
            if [ -n "$target" ]; then
                # The front-end is embedded at build time, so build it first.
                run "$ROOT/scripts/build.sh" --release
                run cargo build --release --locked -p wpg-cli --target "$target"
            else
                run "$ROOT/scripts/build.sh" --release
            fi
        fi
    fi
    if [ "$dry_run" -eq 0 ]; then
        [ -f "$binary" ] || die "binary not found: $binary"
        if command -v file >/dev/null 2>&1 && ! file -b "$binary" | grep -q '^ELF'; then
            die "$binary is not a Linux (ELF) executable; build for the host's target with --target"
        fi
    fi
fi

# ---- upload -----------------------------------------------------------------
stage="/tmp/wpg-deploy.$$"
info "uploading to $host:$stage"
run ssh "${ssh_args[@]}" "$host" "mkdir -p '$stage'"
upload=("$ROOT/scripts/lib/deploy-remote.sh" "$ROOT/packaging/systemd/wpg.service")
[ "$rollback" -eq 1 ] || upload+=("$binary")
run scp "${scp_args[@]}" "${upload[@]}" "$host:$stage/"
[ -z "$password_file" ] || run scp "${scp_args[@]}" "$password_file" "$host:$stage/password"
if [ "$templates" -eq 1 ] && [ "$rollback" -eq 0 ]; then
    run scp "${scp_args[@]}" -r "$ROOT/templates" "$host:$stage/templates"
fi

# ---- install ----------------------------------------------------------------
remote_args=(--stage "$stage" --prefix "$prefix" --bind "$bind" --hugepages "$hugepages")
[ "$rollback" -eq 1 ] || remote_args+=(--binary "$stage/$(basename "$binary")")
[ "$service" -eq 1 ] && remote_args+=(--service)
[ "$rollback" -eq 1 ] && remote_args+=(--rollback)
[ "$dpdk" -eq 1 ] && remote_args+=(--dpdk)
[ "$allow_insecure" -eq 1 ] && remote_args+=(--allow-insecure)
[ "$templates" -eq 1 ] && [ "$rollback" -eq 0 ] && remote_args+=(--templates "$stage/templates")
[ -z "$password_file" ] || remote_args+=(--password-file "$stage/password")
for i in "${ifaces[@]}"; do remote_args+=(--iface "$i"); done

quoted=""
for a in "${remote_args[@]}"; do quoted+=" $(printf '%q' "$a")"; done
info "installing on $host (sudo)"
# -t: sudo may need to ask for a password.
run ssh -t "${ssh_args[@]}" "$host" "sudo bash '$stage/deploy-remote.sh'$quoted; rc=\$?; rm -rf '$stage'; exit \$rc"

# ---- healthcheck ------------------------------------------------------------
if [ "$service" -eq 1 ] && [ "$healthcheck" -eq 1 ]; then
    info "health check: http://$hc_addr:$port/api/health on $host"
    check="for i in \$(seq 1 30); do
        if command -v curl >/dev/null 2>&1; then
            out=\$(curl -fsS http://$hc_addr:$port/api/health 2>/dev/null || curl -fsS -k https://$hc_addr:$port/api/health 2>/dev/null) && { echo \"\$out\"; exit 0; }
        elif command -v wget >/dev/null 2>&1; then
            out=\$(wget -qO- http://$hc_addr:$port/api/health 2>/dev/null) && { echo \"\$out\"; exit 0; }
        else
            echo 'neither curl nor wget on the host; skipping' >&2; exit 0
        fi
        sleep 1
    done
    echo 'wpg did not become healthy; last log lines:' >&2
    sudo journalctl -u wpg -n 30 --no-pager >&2 || true
    exit 1"
    run ssh "${ssh_args[@]}" "$host" "$check"
    info "deployed: http://${host#*@}:$port/"
elif [ "$rollback" -eq 1 ]; then
    info "rolled back (wpg.service was restarted if it is enabled)"
else
    info "done (no service started; run 'wpg serve' on $host or pass --service)"
fi
