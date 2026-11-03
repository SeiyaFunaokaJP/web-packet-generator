#!/usr/bin/env bash
# Run wpg.
#
# Usage: scripts/run.sh [--dev | --no-build] [-- <extra args for `wpg serve`>]
#   --dev        hot reload: `trunk serve` (frontend) + `cargo watch` (backend)
#   --no-build   run target/release/wpg as it is, without rebuilding it first
#   default: bring the release build up to date (scripts/build.sh --release), then
#            run `wpg serve`
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

dev=0
build=1
while [ "$#" -gt 0 ]; do
    case "$1" in
        --dev)
            dev=1
            ;;
        --no-build)
            build=0
            ;;
        -h | --help)
            sed -n '2,8p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
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
    if [ "$build" -eq 1 ]; then
        "$ROOT/scripts/build.sh" --release
    elif [ ! -x "$bin" ]; then
        echo "error: $bin missing; run without --no-build" >&2
        exit 1
    fi
    exec "$bin" serve "$@"
fi
