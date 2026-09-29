#!/usr/bin/env bash
# Run wpg.
#
# Usage: scripts/run.sh [--dev] [-- <extra args for `wpg serve`>]
#   --dev   hot reload: `trunk serve` (frontend) + `cargo watch` (backend)
#   default: run the release binary `wpg serve` (builds it first if missing)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

dev=0
while [ "$#" -gt 0 ]; do
    case "$1" in
        --dev)
            dev=1
            ;;
        -h | --help)
            sed -n '2,6p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        --)
            shift
            break
            ;;
        *)
            echo "error: unknown argument: $1" >&2
            exit 2
            ;;
    esac
    shift
done

if [ "$dev" -eq 1 ]; then
    command -v trunk >/dev/null 2>&1 || { echo "error: trunk not found. Install: cargo install trunk --locked" >&2; exit 1; }
    cargo watch --version >/dev/null 2>&1 || { echo "error: cargo-watch not found. Install: cargo install cargo-watch --locked" >&2; exit 1; }

    trunk_pid=""
    cleanup() {
        if [ -n "$trunk_pid" ]; then
            kill "$trunk_pid" 2>/dev/null || true
        fi
    }
    trap cleanup EXIT INT TERM

    (cd crates/wpg-web && trunk serve) &
    trunk_pid=$!
    cargo watch -x "run -p wpg-cli -- serve $*"
else
    bin="target/release/wpg"
    if [ ! -x "$bin" ]; then
        echo "==> release binary missing, building"
        "$ROOT/scripts/build.sh" --release
    fi
    exec "$bin" serve "$@"
fi
