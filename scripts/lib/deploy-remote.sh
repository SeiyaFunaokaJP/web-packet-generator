#!/usr/bin/env bash
# Host side of scripts/deploy.sh and scripts/deploy.ps1: runs as root on the target host
# from the staging directory the files were copied to. Not meant to be run by hand (see
# deploy.sh / deploy.ps1).
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
terminal=0
snmp_trap=0
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
        --terminal) terminal=1 ;;
        --snmp-trap) snmp_trap=1 ;;
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

# enable_feature <section> "<key> = <value>"...: sets the keys of [section] in a copy of
# wpg.toml, checks the copy with `wpg config validate` and installs it. Only a wpg.toml
# created by this run is changed. Keys the installed binary does not know would make it
# refuse to start (deny_unknown_fields), so they are first tried on a scratch file.
enable_feature() {
    local section="$1"
    shift
    local toml="$etc/wpg.toml" probe tmp line
    probe="$(mktemp)"
    { echo "[$section]"; printf '%s\n' "$@"; } >"$probe"
    if ! "$exe" config validate --config "$probe" >/dev/null 2>&1; then
        rm -f "$probe"
        warn "this wpg build ($("$exe" --version 2>/dev/null || echo unknown)) does not support [$section]; not enabled. Upgrade wpg and deploy again."
        return 0
    fi
    rm -f "$probe"
    if [ "$toml_created" -eq 0 ]; then
        warn "$toml already exists and is never changed by deploy. To enable [$section], add these lines to it and run 'sudo systemctl restart wpg':"
        { echo "[$section]"; printf '%s\n' "$@"; } | sed 's/^/       /' >&2
        return 0
    fi
    tmp="$(mktemp)"
    cp "$toml" "$tmp"
    for line in "$@"; do
        toml_set "$tmp" "$section" "$line"
    done
    if "$exe" config validate --config "$tmp" >/dev/null 2>&1; then
        cat "$tmp" >"$toml"
        info "enabled [$section] in $toml"
    else
        warn "could not enable [$section] in $toml automatically; add these lines by hand:"
        { echo "[$section]"; printf '%s\n' "$@"; } | sed 's/^/       /' >&2
    fi
    rm -f "$tmp"
}

# toml_set <file> <section> "<key> = <value>": replaces the key (also a commented-out
# one) inside an uncommented [section], or adds the line, or appends the section.
toml_set() {
    local file="$1" section="$2" line="$3" key
    key="${line%%[ =]*}"
    awk -v sec="[$section]" -v key="$key" -v line="$line" '
        function flush() { if (insec && !done) { print line; done = 1 } }
        /^[[:space:]]*\[/ {
            t = $0; gsub(/[[:space:]]/, "", t); sub(/#.*/, "", t)
            if (insec) flush()
            insec = (t == sec)
            if (insec) seen = 1
            print; next
        }
        insec && !done && $0 ~ ("^[[:space:]]*#?[[:space:]]*" key "[[:space:]]*=") { print line; done = 1; next }
        { print }
        END {
            if (insec) flush()
            if (!seen) { print ""; print sec; print line }
        }' "$file" >"$file.new"
    mv -f "$file.new" "$file"
}

# Hints only: the firewall is never changed.
firewall_hint() {
    local port="${bind##*:}"
    if command -v ufw >/dev/null 2>&1 && ufw status 2>/dev/null | grep -q '^Status: active'; then
        if ! ufw status 2>/dev/null | grep -Eq "^$port(/tcp)?[[:space:]].*ALLOW"; then
            warn "ufw is active and does not allow port $port: sudo ufw allow $port/tcp"
        fi
        if [ "$snmp_trap" -eq 1 ] && ! ufw status 2>/dev/null | grep -Eq '^162(/udp)?[[:space:]].*ALLOW'; then
            warn "ufw: allow the SNMP traps with: sudo ufw allow 162/udp"
        fi
    elif command -v firewall-cmd >/dev/null 2>&1 && firewall-cmd --state >/dev/null 2>&1; then
        warn "firewalld is running; open the port if needed: sudo firewall-cmd --permanent --add-port=$port/tcp && sudo firewall-cmd --reload"
    fi
}

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
    if [ -z "$binary" ] || [ ! -f "$binary" ]; then die "uploaded binary missing"; fi
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
# Lets `wpg send` / `wpg serve` run by a normal user open raw sockets (and bind the
# SNMP trap port 162). The systemd
# unit does not need it (it uses AmbientCapabilities).
if ! command -v setcap >/dev/null 2>&1 && command -v apt-get >/dev/null 2>&1; then
    info "installing libcap2-bin (setcap)"
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq libcap2-bin >/dev/null || true
fi
if command -v setcap >/dev/null 2>&1; then
    setcap cap_net_raw,cap_net_admin,cap_net_bind_service+eip "$exe"
    info "setcap cap_net_raw,cap_net_admin,cap_net_bind_service+eip $exe"
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
if [ "$service" -eq 0 ] && [ "$((terminal + snmp_trap))" -gt 0 ]; then
    warn "--terminal / --snmp-trap only configure the service (/etc/wpg/wpg.toml); ignored without --service"
fi
if [ "$service" -eq 1 ]; then
    have_systemd || die "systemd is not running on this host; start 'wpg serve' another way"
    if ! id wpg >/dev/null 2>&1; then
        nologin="$(command -v nologin || echo /usr/sbin/nologin)"
        useradd --system --home-dir "$state" --no-create-home --shell "$nologin" wpg
        info "created system user wpg"
    fi
    install -d -m 0750 -o root -g wpg "$etc"
    install -d -m 0750 -o wpg -g wpg "$state"

    toml_created=0
    if [ ! -f "$etc/wpg.toml" ]; then
        "$exe" config init --path "$etc/wpg.toml" >/dev/null
        chown root:wpg "$etc/wpg.toml"
        chmod 0640 "$etc/wpg.toml"
        toml_created=1
        info "wrote $etc/wpg.toml (edit it for settings not covered by deploy.sh)"
    fi
    # Optional features (web terminal, SNMP trap receiver).
    [ "$terminal" -eq 0 ] || enable_feature terminal "enabled = true" "allowed_targets = []"
    [ "$snmp_trap" -eq 0 ] || enable_feature snmp 'trap_listen = "0.0.0.0:162"'
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

    # Port 162 (SNMP traps) is privileged: give the service CAP_NET_BIND_SERVICE when a
    # trap listener is configured, as a drop-in so that wpg.service stays as shipped.
    if grep -Eq '^[[:space:]]*trap_listen[[:space:]]*=' "$etc/wpg.toml" 2>/dev/null; then
        mkdir -p /etc/systemd/system/wpg.service.d
        printf '[Service]\nAmbientCapabilities=CAP_NET_BIND_SERVICE\nCapabilityBoundingSet=CAP_NET_BIND_SERVICE\n' \
            >/etc/systemd/system/wpg.service.d/bind-service.conf
        info "drop-in wpg.service.d/bind-service.conf: CAP_NET_BIND_SERVICE for the SNMP trap port"
    else
        rm -f /etc/systemd/system/wpg.service.d/bind-service.conf
    fi

    sed "s#/usr/local/bin/wpg#$exe#g" "$stage/wpg.service" >"$unit.new"
    chmod 0644 "$unit.new"
    mv -f "$unit.new" "$unit"
    systemctl daemon-reload
    systemctl enable --quiet wpg
    systemctl restart wpg
    info "systemd unit wpg.service (re)started"
    firewall_hint
elif [ "$rollback" -eq 1 ] && have_systemd && systemctl is-enabled --quiet wpg 2>/dev/null; then
    systemctl restart wpg
    info "wpg.service restarted"
fi

info "doctor:"
"$exe" doctor 2>&1 | sed 's/^/     /' || true
