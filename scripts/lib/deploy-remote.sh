#!/usr/bin/env bash
# Host side of scripts/deploy.sh: runs as root on the target host from the staging
# directory the files were copied to. Not meant to be run by hand (see deploy.sh).
set -euo pipefail

stage=""
binary=""
prefix="/usr/local"
bind="0.0.0.0:8080"
service=0
rollback=0
dpdk=0
hugepages=1024
allow_insecure=0
templates=""
password_file=""
ifaces=()

while [ "$#" -gt 0 ]; do
    case "$1" in
        --stage) stage="$2"; shift ;;
        --binary) binary="$2"; shift ;;
        --prefix) prefix="$2"; shift ;;
        --bind) bind="$2"; shift ;;
        --service) service=1 ;;
        --rollback) rollback=1 ;;
        --dpdk) dpdk=1 ;;
        --hugepages) hugepages="$2"; shift ;;
        --allow-insecure) allow_insecure=1 ;;
        --templates) templates="$2"; shift ;;
        --password-file) password_file="$2"; shift ;;
        --iface) ifaces+=("$2"); shift ;;
        *) echo "deploy-remote: unknown argument: $1" >&2; exit 2 ;;
    esac
    shift
done

info() { echo "  -> $*"; }
warn() { echo "  warning: $*" >&2; }
die() { echo "  error: $*" >&2; exit 1; }

[ "$(id -u)" -eq 0 ] || die "must run as root"
[ "$(uname -s)" = Linux ] || die "only Linux hosts are supported"

bin_dir="$prefix/bin"
exe="$bin_dir/wpg"
share="$prefix/share/wpg"
etc=/etc/wpg
state=/var/lib/wpg
unit=/etc/systemd/system/wpg.service

have_systemd() { [ -d /run/systemd/system ] && command -v systemctl >/dev/null 2>&1; }

# ---- binary -----------------------------------------------------------------
install -d -m 0755 "$bin_dir"
if [ "$rollback" -eq 1 ]; then
    [ -x "$exe.prev" ] || die "no previous binary ($exe.prev) to roll back to"
    info "rolling back: $exe.prev -> $exe (the replaced binary becomes $exe.prev)"
    # Swap, so a second rollback undoes the first.
    mv -f "$exe.prev" "$exe.swap"
    [ ! -e "$exe" ] || mv -f "$exe" "$exe.prev"
    mv -f "$exe.swap" "$exe"
else
    [ -n "$binary" ] && [ -f "$binary" ] || die "uploaded binary missing"
    "$binary" --version >/dev/null 2>&1 || die "the uploaded binary does not run on this host (wrong architecture or libc?)"
    if [ -x "$exe" ]; then
        info "keeping the current binary as $exe.prev ($("$exe" --version 2>/dev/null || echo unknown))"
        cp -p "$exe" "$exe.prev"
    fi
    # install(1) writes a new inode, so a running server keeps its old image until restart.
    install -m 0755 "$binary" "$exe"
    info "installed $("$exe" --version) to $exe"
fi

# ---- capabilities -----------------------------------------------------------
# Lets `wpg send` / `wpg serve` run by a normal user open raw sockets. The systemd
# unit does not need it (it uses AmbientCapabilities).
if ! command -v setcap >/dev/null 2>&1 && command -v apt-get >/dev/null 2>&1; then
    info "installing libcap2-bin (setcap)"
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq libcap2-bin >/dev/null || true
fi
if command -v setcap >/dev/null 2>&1; then
    setcap cap_net_raw,cap_net_admin+eip "$exe"
    info "setcap cap_net_raw,cap_net_admin+eip $exe"
else
    warn "setcap not found (package libcap2-bin / libcap); 'wpg send' will need sudo"
fi

# ---- templates --------------------------------------------------------------
if [ -n "$templates" ] && [ -d "$templates" ]; then
    rm -rf "$share/templates.new"
    install -d -m 0755 "$share"
    cp -r "$templates" "$share/templates.new"
    rm -rf "$share/templates"
    mv "$share/templates.new" "$share/templates"
    chmod -R a+rX "$share/templates"
    info "sample templates -> $share/templates"
fi

# ---- DPDK host preparation --------------------------------------------------
if [ "$dpdk" -eq 1 ]; then
    info "DPDK: preparing hugepages and vfio-pci (the dpdk backend needs a build with --features dpdk)"
    echo "vm.nr_hugepages = $hugepages" >/etc/sysctl.d/80-wpg-hugepages.conf
    sysctl -q -w "vm.nr_hugepages=$hugepages" || warn "could not reserve $hugepages hugepages now"
    got="$(cat /proc/sys/vm/nr_hugepages 2>/dev/null || echo 0)"
    [ "$got" -ge "$hugepages" ] || warn "only $got of $hugepages hugepages reserved (memory fragmented? reboot applies /etc/sysctl.d/80-wpg-hugepages.conf)"
    if ! grep -qs ' hugetlbfs ' /proc/mounts; then
        mkdir -p /dev/hugepages
        mount -t hugetlbfs nodev /dev/hugepages || warn "could not mount hugetlbfs on /dev/hugepages"
    fi
    if modprobe vfio-pci 2>/dev/null; then
        echo vfio-pci >/etc/modules-load.d/wpg-dpdk.conf
        info "vfio-pci loaded (persisted in /etc/modules-load.d/wpg-dpdk.conf)"
    else
        warn "modprobe vfio-pci failed (linux-modules-extra-$(uname -r) missing?)"
    fi
    if [ -z "$(ls -A /sys/kernel/iommu_groups 2>/dev/null)" ]; then
        warn "no IOMMU groups: enable VT-d/AMD-Vi and boot with intel_iommu=on iommu=pt, or (VMs without vIOMMU, e.g. CML2) use vfio's enable_unsafe_noiommu_mode"
    fi
    info "binding NICs to vfio-pci is left to you: dpdk-devbind.py --bind=vfio-pci <pci-addr> (see 'wpg doctor')"
fi

# ---- systemd service --------------------------------------------------------
if [ "$service" -eq 1 ]; then
    have_systemd || die "systemd is not running on this host; start 'wpg serve' another way"
    if ! id wpg >/dev/null 2>&1; then
        nologin="$(command -v nologin || echo /usr/sbin/nologin)"
        useradd --system --home-dir "$state" --no-create-home --shell "$nologin" wpg
        info "created system user wpg"
    fi
    install -d -m 0750 -o root -g wpg "$etc"
    install -d -m 0750 -o wpg -g wpg "$state"

    if [ ! -f "$etc/wpg.toml" ]; then
        "$exe" config init --path "$etc/wpg.toml" >/dev/null
        chown root:wpg "$etc/wpg.toml"
        chmod 0640 "$etc/wpg.toml"
        info "wrote $etc/wpg.toml (edit it for settings not covered by deploy.sh)"
    fi
    if [ -n "$password_file" ]; then
        install -m 0640 -o root -g wpg "$password_file" "$etc/password"
        info "admin password -> $etc/password"
    fi

    # Settings owned by deploy.sh; rewritten on every deploy. Override them in
    # wpg.toml only for keys not listed here (the environment wins over the file).
    {
        echo "# Managed by scripts/deploy.sh; rewritten on every deploy."
        echo "WPG_SERVER__BIND=$bind"
        if [ "${#ifaces[@]}" -gt 0 ]; then
            list="$(printf '"%s",' "${ifaces[@]}")"
            echo "WPG_TX__ALLOWED_IFACES=[${list%,}]"
        fi
        if [ -d "$share/templates" ]; then
            echo "WPG_TEMPLATES__LIBRARY_PATHS=[\"$share/templates\"]"
        fi
        if [ -s "$etc/password" ]; then
            echo "WPG_AUTH__PASSWORD_FILE=$etc/password"
        fi
        echo "WPG_LOG__ANSI=false"
    } >"$etc/wpg.env.new"
    chmod 0640 "$etc/wpg.env.new"
    chown root:wpg "$etc/wpg.env.new"
    mv -f "$etc/wpg.env.new" "$etc/wpg.env"

    host_part="${bind%:*}"
    case "$host_part" in 127.* | localhost | "[::1]") loopback=1 ;; *) loopback=0 ;; esac
    if [ "$loopback" -eq 0 ] && [ "$allow_insecure" -eq 0 ] \
        && [ ! -s "$etc/password" ] \
        && ! grep -Eq '^[[:space:]]*password_(hash|file)[[:space:]]*=' "$etc/wpg.toml" \
        && [ ! -f "$state/wpg.sqlite" ]; then
        die "no admin password for a server on $bind. Pass --password-file, run 'sudo $exe config set-password --config $etc/wpg.toml', use --bind 127.0.0.1:8080, or --allow-insecure"
    fi
    if [ "$allow_insecure" -eq 1 ]; then
        mkdir -p /etc/systemd/system/wpg.service.d
        printf '[Service]\nExecStart=\nExecStart=%s serve --allow-insecure\n' "$exe" \
            >/etc/systemd/system/wpg.service.d/allow-insecure.conf
    else
        rm -f /etc/systemd/system/wpg.service.d/allow-insecure.conf
    fi

    sed "s#/usr/local/bin/wpg#$exe#g" "$stage/wpg.service" >"$unit.new"
    chmod 0644 "$unit.new"
    mv -f "$unit.new" "$unit"
    systemctl daemon-reload
    systemctl enable --quiet wpg
    systemctl restart wpg
    info "systemd unit wpg.service (re)started"
elif [ "$rollback" -eq 1 ] && have_systemd && systemctl is-enabled --quiet wpg 2>/dev/null; then
    systemctl restart wpg
    info "wpg.service restarted"
fi

info "doctor:"
"$exe" doctor 2>&1 | sed 's/^/     /' || true
