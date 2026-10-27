#!/usr/bin/env bash
# Host side of scripts/deploy.sh / deploy.ps1: waits until wpg answers GET /api/health.
#
# Usage: remote-health.sh <addr> <port> [seconds]   (as the SSH user; sudo only with -n)
set -u

addr="${1:-127.0.0.1}"
port="${2:-8080}"
wait_s="${3:-30}"
case "$addr" in 0.0.0.0 | "" | "[::]" | "::") addr=127.0.0.1 ;; esac

i=0
while [ "$i" -lt "$wait_s" ]; do
    if command -v curl >/dev/null 2>&1; then
        out=$(curl -fsS "http://$addr:$port/api/health" 2>/dev/null \
            || curl -fsS -k "https://$addr:$port/api/health" 2>/dev/null) && { echo "$out"; exit 0; }
    elif command -v wget >/dev/null 2>&1; then
        out=$(wget -qO- "http://$addr:$port/api/health" 2>/dev/null \
            || wget -qO- --no-check-certificate "https://$addr:$port/api/health" 2>/dev/null) && { echo "$out"; exit 0; }
    else
        echo 'neither curl nor wget on the host; skipping the health check' >&2
        exit 0
    fi
    sleep 1
    i=$((i + 1))
done
echo "wpg did not become healthy on $addr:$port within ${wait_s}s; last log lines:" >&2
if [ "$(id -u)" -eq 0 ]; then
    journalctl -u wpg -n 30 --no-pager >&2 || true
else
    sudo -n journalctl -u wpg -n 30 --no-pager >&2 \
        || journalctl -u wpg -n 30 --no-pager >&2 \
        || echo "(run 'sudo journalctl -u wpg -n 50' on the host to see the log)" >&2
fi
exit 1
