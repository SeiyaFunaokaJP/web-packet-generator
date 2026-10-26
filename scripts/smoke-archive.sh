#!/usr/bin/env bash
# Smoke-test a release archive made by scripts/release.sh (spec §2.8 / M10). Used by
# .github/workflows/packaging.yml on Linux, macOS and Windows (Git Bash), and locally.
#
# Usage: scripts/smoke-archive.sh [options] <archive>
#   --no-run       only check the archive contents (e.g. a cross-built target)
#   --port <n>     port for the `wpg serve` check (default: 18080)
#   -h, --help     this help
#
# Checks: the .sha256 file next to the archive (when present), the files every archive
# must hold, and, unless --no-run: `wpg --version`, `wpg doctor --json`, `wpg render` of a
# sample template, and `wpg serve` answering /api/health and serving the web UI.
set -euo pipefail

usage() { sed -n '2,12p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }
die() { echo "error: $*" >&2; exit 1; }
info() { echo "==> $*"; }

run=1
port=18080
archive=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --no-run) run=0 ;;
        --port) [ "$#" -ge 2 ] || die "--port needs a value"; port="$2"; shift ;;
        -h | --help) usage; exit 0 ;;
        -*) die "unknown argument: $1 (see --help)" ;;
        *) [ -z "$archive" ] || die "only one archive"; archive="$1" ;;
    esac
    shift
done
[ -n "$archive" ] || die "no archive given (see --help)"
[ -f "$archive" ] || die "not found: $archive"

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$@"
    else
        shasum -a 256 "$@"
    fi
}

dir="$(cd "$(dirname "$archive")" && pwd)"
file="$(basename "$archive")"
if [ -f "$dir/$file.sha256" ]; then
    info "checking $file.sha256"
    (cd "$dir" && sha256 -c "$file.sha256")
fi

work="$(mktemp -d)"
server_pid=""
cleanup() {
    if [ -n "$server_pid" ]; then
        kill "$server_pid" 2>/dev/null || true
        wait "$server_pid" 2>/dev/null || true
    fi
    rm -rf "$work"
}
trap cleanup EXIT

case "$file" in
    *.tar.gz) name="${file%.tar.gz}"; tar -C "$work" -xzf "$dir/$file" ;;
    *.zip)
        name="${file%.zip}"
        if command -v unzip >/dev/null 2>&1; then
            unzip -q "$dir/$file" -d "$work"
        else
            7z x -bso0 "-o$work" "$dir/$file"
        fi
        ;;
    *) die "unknown archive type: $file" ;;
esac
pkg="$work/$name"
[ -d "$pkg" ] || die "the archive has no top-level directory $name/"

case "$name" in
    *windows*) exe="$pkg/wpg.exe" ;;
    *) exe="$pkg/wpg" ;;
esac

info "checking the contents of $file"
missing=0
for f in "$exe" README.md README.ja.md CHANGELOG.md LICENSE-MIT LICENSE-APACHE \
    templates/examples/arp-sweep.wpg.yaml packaging/systemd/wpg.service \
    packaging/launchd/io.github.seiyafunaokajp.wpg.plist packaging/windows/wpg-service.ps1; do
    case "$f" in /*) p="$f" ;; *) p="$pkg/$f" ;; esac
    if [ ! -f "$p" ]; then
        echo "missing: ${p#"$work"/}" >&2
        missing=1
    fi
done
[ "$missing" -eq 0 ] || die "the archive is incomplete"
case "$name" in
    *windows*) ;;
    *) [ -x "$exe" ] || die "wpg is not executable in the archive" ;;
esac

if [ "$run" -eq 0 ]; then
    info "contents ok (--no-run)"
    exit 0
fi

info "wpg --version"
"$exe" --version

# The exit status reflects the host (missing privileges, Npcap, ...), not the archive.
info "wpg doctor --json"
"$exe" doctor --json >"$work/doctor.json" || true
grep -q '"checks"' "$work/doctor.json" || die "wpg doctor printed no report"

info "wpg render"
"$exe" render "$pkg/templates/examples/arp-sweep.wpg.yaml" --count 16 -o "$work/arp.pcapng"
[ -s "$work/arp.pcapng" ] || die "wpg render wrote nothing"

info "wpg serve on 127.0.0.1:$port"
mkdir -p "$work/data"
(cd "$work" && exec "$exe" serve --bind "127.0.0.1:$port" --data-dir "$work/data" \
    --library-path "$pkg/templates") >"$work/serve.log" 2>&1 &
server_pid=$!
ok=0
for _ in $(seq 1 60); do
    if curl -fsS "http://127.0.0.1:$port/api/health" >"$work/health.json" 2>/dev/null; then
        ok=1
        break
    fi
    kill -0 "$server_pid" 2>/dev/null || break
    sleep 1
done
if [ "$ok" -ne 1 ]; then
    cat "$work/serve.log" >&2
    die "wpg serve did not answer /api/health"
fi
cat "$work/health.json"
echo
# The web UI is embedded: the index page must be HTML, not the "UI not built" fallback.
curl -fsS "http://127.0.0.1:$port/" >"$work/index.html"
grep -qi '<script' "$work/index.html" || die "the embedded web UI is missing"
info "smoke test passed: $file"
