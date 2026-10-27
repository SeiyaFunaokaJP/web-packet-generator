#!/usr/bin/env bash
# Host side of the deploy wizards (scripts/deploy.sh --interactive, scripts/deploy.ps1):
# runs as the SSH user on the target and prints facts about it as KEY=value lines, one
# WPG_PROBE_IFACE line per interface. Read-only; uses sudo only with -n (never prompts).
set -u

kv() { printf 'WPG_PROBE_%s=%s\n' "$1" "$2"; }

os_id=unknown
os_version=""
os_name="$(uname -s)"
if [ -r /etc/os-release ]; then
    # shellcheck disable=SC1091
    os_id="$(. /etc/os-release && echo "${ID:-unknown}")"
    # shellcheck disable=SC1091
    os_version="$(. /etc/os-release && echo "${VERSION_ID:-}")"
    # shellcheck disable=SC1091
    os_name="$(. /etc/os-release && echo "${PRETTY_NAME:-$os_id}")"
fi
kv OS_ID "$os_id"
kv OS_VERSION "$os_version"
kv OS_NAME "$os_name"
kv KERNEL "$(uname -s)"
kv ARCH "$(uname -m)"

if command -v apt-get >/dev/null 2>&1; then
    kv PKG apt
elif command -v dnf >/dev/null 2>&1; then
    kv PKG dnf
elif command -v yum >/dev/null 2>&1; then
    kv PKG yum
else
    kv PKG none
fi

if [ -d /run/systemd/system ] && command -v systemctl >/dev/null 2>&1; then
    kv SYSTEMD 1
else
    kv SYSTEMD 0
fi

if [ "$(id -u)" -eq 0 ]; then
    kv SUDO root
elif ! command -v sudo >/dev/null 2>&1; then
    kv SUDO none
elif sudo -n true >/dev/null 2>&1; then
    kv SUDO nopasswd
else
    kv SUDO password
fi

exe=/usr/local/bin/wpg
if [ -x "$exe" ]; then
    kv WPG_VERSION "$("$exe" --version 2>/dev/null | head -n 1)"
else
    kv WPG_VERSION ""
fi
if [ -x "$exe.prev" ]; then kv WPG_PREV 1; else kv WPG_PREV 0; fi
if command -v systemctl >/dev/null 2>&1 && systemctl is-enabled --quiet wpg 2>/dev/null; then
    kv SERVICE "$(systemctl is-active wpg 2>/dev/null || true)"
else
    kv SERVICE absent
fi

# Interfaces: name|state|addresses. The one carrying this SSH session is the management
# interface (it should not be in tx.allowed_ifaces).
ssh_ip="$(echo "${SSH_CONNECTION:-}" | awk '{print $3}')"
mgmt=""
if command -v ip >/dev/null 2>&1; then
    while read -r name state addrs; do
        name="${name%%@*}"
        [ "$name" = lo ] && continue
        case " $addrs " in *" $ssh_ip/"*) [ -n "$ssh_ip" ] && mgmt="$name" ;; esac
        kv IFACE "$name|$state|$addrs"
    done < <(ip -br addr show 2>/dev/null)
    if [ -z "$mgmt" ]; then
        mgmt="$(ip route show default 2>/dev/null | awk '{for (i = 1; i < NF; i++) if ($i == "dev") { print $(i + 1); exit }}')"
    fi
else
    for d in /sys/class/net/*; do
        name="${d##*/}"
        [ "$name" = lo ] && continue
        kv IFACE "$name|$(cat "$d/operstate" 2>/dev/null || echo unknown)|"
    done
fi
kv MGMT_IFACE "$mgmt"

kv MEM_MB "$(awk '/^MemTotal:/ {print int($2 / 1024)}' /proc/meminfo 2>/dev/null || echo 0)"
kv SWAP_MB "$(awk '/^SwapTotal:/ {print int($2 / 1024)}' /proc/meminfo 2>/dev/null || echo 0)"
kv DISK_MB "$(df -Pm "$HOME" 2>/dev/null | awk 'NR == 2 {print $4}')"
kv CPUS "$(nproc 2>/dev/null || echo 1)"

if [ -x "$HOME/.cargo/bin/cargo" ] || command -v cargo >/dev/null 2>&1; then
    kv CARGO 1
else
    kv CARGO 0
fi

# Internet access (needed to build on the host: apt, rustup, crates.io, trunk).
if command -v curl >/dev/null 2>&1; then
    if curl -fsS -o /dev/null --max-time 8 https://static.rust-lang.org/rustup/release-stable.toml 2>/dev/null; then
        kv INTERNET 1
    else
        kv INTERNET 0
    fi
elif command -v wget >/dev/null 2>&1; then
    if wget -q -O /dev/null -T 8 https://static.rust-lang.org/rustup/release-stable.toml 2>/dev/null; then
        kv INTERNET 1
    else
        kv INTERNET 0
    fi
else
    kv INTERNET unknown
fi
exit 0
