#!/usr/bin/env bash
# Build wpg: frontend (trunk, WASM) first, then the backend workspace.
#
# Usage: scripts/build.sh [--release] [--features <list>] [--dpdk] [--no-pcap]
#   --release          optimised build
#   --features <list>  extra cargo features for wpg-cli (e.g. "dpdk")
#   --dpdk             shorthand for --features dpdk
#   --no-pcap          macOS: leave out the pcap backend, which is otherwise added
#                      because it is the only way to send there (libpcap ships with macOS)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

release=0
features=""
pcap_default=1

while [ "$#" -gt 0 ]; do
    case "$1" in
        --release)
            release=1
            ;;
        --features)
            [ "$#" -ge 2 ] || { echo "error: --features needs a value" >&2; exit 2; }
            features="${features:+$features,}$2"
            shift
            ;;
        --features=*)
            features="${features:+$features,}${1#--features=}"
            ;;
        --dpdk)
            features="${features:+$features,}dpdk"
            ;;
        --no-pcap)
            pcap_default=0
            ;;
        -h | --help)
            sed -n '2,10p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "error: unknown argument: $1" >&2
            exit 2
            ;;
    esac
    shift
done

warn() { echo "warning: $*" >&2; }
die() { echo "error: $*" >&2; exit 1; }

# ---- dependency check -------------------------------------------------------
have_trunk=1

command -v rustup >/dev/null 2>&1 || die "rustup not found. Install from https://rustup.rs"
command -v cargo >/dev/null 2>&1 || die "cargo not found. Install from https://rustup.rs"

if ! rustup target list --installed 2>/dev/null | grep -qx 'wasm32-unknown-unknown'; then
    die "wasm32-unknown-unknown target missing. Run: rustup target add wasm32-unknown-unknown"
fi

if ! command -v trunk >/dev/null 2>&1; then
    have_trunk=0
    warn "trunk not found; the frontend build will be skipped. Install: cargo install trunk --locked"
fi

case "$(uname -s)" in
    Linux)
        if ! { command -v pkg-config >/dev/null 2>&1 && pkg-config --exists libpcap; } \
            && [ ! -e /usr/include/pcap/pcap.h ]; then
            warn "libpcap headers not found (Debian/Ubuntu: sudo apt install libpcap-dev). Not required yet: the pcap feature is unused."
        fi
        ;;
    Darwin)
        # libpcap ships with macOS, and BPF through libpcap is the only way to send there.
        case ",$features," in
            *,pcap,*) ;;
            *) [ "$pcap_default" -eq 0 ] || features="${features:+$features,}pcap" ;;
        esac
        ;;
    *)
        warn "unrecognised OS; skipping pcap check (Windows users: use scripts/build.ps1)"
        ;;
esac

# ---- frontend ---------------------------------------------------------------
if [ "$have_trunk" -eq 1 ]; then
    trunk_args=(build)
    [ "$release" -eq 1 ] && trunk_args+=(--release)
    echo "==> building frontend (trunk ${trunk_args[*]})"
    # trunk reads NO_COLOR as a boolean and rejects the usual NO_COLOR=1 (no-color.org).
    (cd crates/wpg-web && NO_COLOR="${NO_COLOR:+true}" trunk "${trunk_args[@]}")
else
    warn "skipping frontend build (crates/wpg-web/dist will be missing or stale)"
fi

# ---- backend ----------------------------------------------------------------
cargo_args=(build --workspace)
[ "$release" -eq 1 ] && cargo_args+=(--release)
[ -n "$features" ] && cargo_args+=(--features "$features")
echo "==> building backend (cargo ${cargo_args[*]})"
cargo "${cargo_args[@]}"

profile=debug
[ "$release" -eq 1 ] && profile=release
echo "==> done: target/$profile/wpg"
