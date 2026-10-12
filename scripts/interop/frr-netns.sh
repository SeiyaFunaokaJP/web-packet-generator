#!/usr/bin/env bash
# Interop check of the templates in templates/interop/ against FRR, on one host.
#
# Creates a network namespace with FRR (zebra, ospfd, ripd, isisd, vrrpd, pimd,
# ldpd, bfdd) behind a veth pair, sends each interop template from the host side
# with the afpacket backend, and checks FRR's view with vtysh. Linux only; needs
# root, iproute2 and FRR (`apt install frr`; the daemons are started directly,
# the frr service stays stopped).
#
#   sudo scripts/interop/frr-netns.sh [path/to/wpg]     # run all checks
#   sudo scripts/interop/frr-netns.sh --keep [wpg]      # leave the lab up
#   sudo scripts/interop/frr-netns.sh --down            # tear the lab down
#
# Exit status: 0 when every check passes.
set -euo pipefail

NS=wpgfrr
PATHSPACE=wpg
HOST_IF=wpg0
PEER_IF=frr0
PEER_IP=10.99.0.2
FRR_LIB=/usr/lib/frr
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
KEEP=0

down() {
    if [[ -d /var/run/frr/$PATHSPACE ]]; then
        for pid in /var/run/frr/"$PATHSPACE"/*.pid; do
            if [[ -e $pid ]]; then
                kill "$(cat "$pid")" 2>/dev/null || true
            fi
        done
    fi
    ip netns del "$NS" 2>/dev/null || true
    ip link del "$HOST_IF" 2>/dev/null || true
}

case "${1:-}" in
    --down) down; exit 0 ;;
    --keep) KEEP=1; shift ;;
esac
WPG=${1:-$ROOT/target/debug/wpg}

[[ $(id -u) -eq 0 ]] || { echo "run as root" >&2; exit 2; }
command -v ip >/dev/null || { echo "iproute2 (ip) is required" >&2; exit 2; }
[[ -x $FRR_LIB/zebra ]] || { echo "FRR is required: apt install frr" >&2; exit 2; }
[[ -x $WPG ]] || { echo "wpg binary not found: $WPG (cargo build -p wpg-cli)" >&2; exit 2; }

in_ns() { ip netns exec "$NS" "$@"; }
vty() { in_ns vtysh -N "$PATHSPACE" "$@"; }

down
[[ $KEEP -eq 1 ]] || trap down EXIT

# --- topology -------------------------------------------------------------
ip netns add "$NS"
ip link add "$HOST_IF" type veth peer name "$PEER_IF"
ip link set "$PEER_IF" netns "$NS"
ip link set "$HOST_IF" up
in_ns ip link set lo up
in_ns ip link set "$PEER_IF" up
in_ns ip addr add "$PEER_IP/24" dev "$PEER_IF"
# VRRP (FRR vrrpd) needs a macvlan carrying the virtual MAC of VRID 5.
in_ns ip link add vrrp4-2-5 link "$PEER_IF" type macvlan mode bridge
in_ns ip link set vrrp4-2-5 address 00:00:5e:00:01:05
in_ns ip addr add 10.99.0.254/24 dev vrrp4-2-5
in_ns ip link set vrrp4-2-5 up

SRC_MAC=$(cat /sys/class/net/$HOST_IF/address)
PEER_MAC=$(in_ns cat /sys/class/net/$PEER_IF/address)

# --- FRR ------------------------------------------------------------------
mkdir -p /etc/frr/$PATHSPACE /var/run/frr/$PATHSPACE
touch /etc/frr/$PATHSPACE/vtysh.conf
chown -R frr:frr /etc/frr/$PATHSPACE /var/run/frr/$PATHSPACE
for d in zebra ospfd ripd isisd vrrpd pimd ldpd bfdd; do
    in_ns "$FRR_LIB/$d" -d -N "$PATHSPACE" -A 127.0.0.1 2>/dev/null
done
sleep 1
vty -c "conf t" \
    -c "router ospf" -c "ospf router-id $PEER_IP" -c "network 10.99.0.0/24 area 0" -c "exit" \
    -c "router rip" -c "version 2" -c "network 10.99.0.0/24" -c "exit" \
    -c "router isis 1" -c "net 49.0001.0000.0000.0002.00" -c "is-type level-2-only" -c "exit" \
    -c "interface $PEER_IF" -c "ip router isis 1" -c "ip pim" \
    -c "vrrp 5 version 3" -c "vrrp 5 priority 100" -c "vrrp 5 ip 10.99.0.254" -c "exit" \
    -c "mpls ldp" -c "router-id $PEER_IP" -c "address-family ipv4" \
    -c "discovery transport-address $PEER_IP" -c "interface $PEER_IF" -c "exit-address-family" -c "exit" \
    -c "bfd" -c "peer 10.99.0.1 interface $PEER_IF" -c "receive-interval 300" -c "transmit-interval 300" \
    -c "end" >/dev/null
sleep 4 # VRRP: become Master first

# --- checks ---------------------------------------------------------------
FAILED=0
LOG=$(mktemp)
send() { # template, extra args...
    local t=$1
    shift
    if ! "$WPG" send "$ROOT/templates/interop/$t.wpg.yaml" -i "$HOST_IF" --backend afpacket \
        "$@" >"$LOG" 2>&1; then
        echo "FAIL  wpg send $t"
        sed 's/^/      /' "$LOG"
        FAILED=1
    fi
}
check() { # name, command, expected regex
    local out
    out=$(eval "$2" 2>&1 || true)
    if grep -Eq "$3" <<<"$out"; then
        echo "PASS  $1"
    else
        echo "FAIL  $1"
        printf '%s\n' "$out" | sed 's/^/      /'
        FAILED=1
    fi
}

send ospfv2-hello-2way --var "src_mac=$SRC_MAC" --count 2 --rate 1pps
sleep 1
check "OSPFv2 neighbor 2-Way" 'vty -c "show ip ospf neighbor"' '10\.99\.0\.1 +0 +2-Way'

send ripv2-response-route --var "src_mac=$SRC_MAC" --count 1
sleep 2
check "RIPv2 routes installed" 'vty -c "show ip route rip"' '198\.51\.100\.0/24 \[120/2\] via 10\.99\.0\.1'

send isis-l2-lan-hello-up --var "src_mac=$SRC_MAC" --count 2 --rate 1pps --var "peer_mac=$PEER_MAC"
sleep 1
check "IS-IS L2 adjacency Up" 'vty -c "show isis neighbor"' '0000\.0000\.0001 +'"$PEER_IF"' +2 +Up'

send vrrpv3-preempt --count 4 --rate 1pps
check "VRRPv3 peer is Backup" 'vty -c "show vrrp 5"' 'Status \(v4\) +Backup'

send pim-hello-neighbor --var "src_mac=$SRC_MAC" --count 1
sleep 1
check "PIM neighbor" 'vty -c "show ip pim neighbor"' "$PEER_IF +10\.99\.0\.1"

send ldp-link-hello --var "src_mac=$SRC_MAC" --count 1
sleep 1
check "LDP link discovery" 'vty -c "show mpls ldp discovery"' '10\.99\.0\.1 +Link +'"$PEER_IF"

send bfd-session-up --var "src_mac=$SRC_MAC" --count 2 --rate 4pps --var state=down --var "peer_mac=$PEER_MAC" --var peer_discriminator=0
sleep 0.5
DISC=$(vty -c "show bfd peers json" | sed -n 's/.*"id":\([0-9]*\).*/\1/p' | head -n1)
send bfd-session-up --var "src_mac=$SRC_MAC" --count 8 --var "peer_mac=$PEER_MAC" --var "peer_discriminator=${DISC:-0}"
check "BFD session Up" 'vty -c "show bfd peers brief"' '10\.99\.0\.1 +up'

if [[ $KEEP -eq 1 ]]; then
    echo "lab kept: ip netns exec $NS vtysh -N $PATHSPACE; tear down with $0 --down"
fi
rm -f "$LOG"
exit $FAILED
