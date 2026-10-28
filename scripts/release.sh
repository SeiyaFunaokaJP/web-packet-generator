#!/usr/bin/env bash
# Build and package a wpg release archive for one target (spec §2.8). Used by
# .github/workflows/release.yml on Linux, macOS and Windows (Git Bash), and locally.
#
# Usage: scripts/release.sh [options]
#   --target <triple>    cargo target (default: the host, e.g. x86_64-unknown-linux-gnu)
#   --features <list>    extra cargo features of wpg-cli (e.g. "pcap"); for a Windows target
#                        with "pcap" the Npcap SDK is downloaded (pinned, checksummed) into
#                        target/ unless LIBPCAP_LIBDIR points at its Lib\x64 directory
#   --out <dir>          output directory (default: dist)
#   --skip-frontend      reuse crates/wpg-web/dist instead of running `trunk build`
#   --checksums <dir>    only write <dir>/SHA256SUMS for the archives in <dir>
#   -h, --help           this help
#
# Output: <out>/wpg-<version>-<target>.tar.gz (.zip on Windows) and a .sha256 file
# next to it. The archive holds the binary, README, CHANGELOG, licences, the sample templates
# and the service samples under packaging/.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

usage() { sed -n '2,18p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }
die() { echo "error: $*" >&2; exit 1; }
info() { echo "==> $*"; }

target=""
features=""
out="dist"
frontend=1
checksums_dir=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --target) [ "$#" -ge 2 ] || die "--target needs a value"; target="$2"; shift ;;
        --features) [ "$#" -ge 2 ] || die "--features needs a value"; features="$2"; shift ;;
        --out) [ "$#" -ge 2 ] || die "--out needs a value"; out="$2"; shift ;;
        --skip-frontend) frontend=0 ;;
        --checksums) [ "$#" -ge 2 ] || die "--checksums needs a value"; checksums_dir="$2"; shift ;;
        -h | --help) usage; exit 0 ;;
        *) die "unknown argument: $1 (see --help)" ;;
    esac
    shift
done

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$@"
    else
        shasum -a 256 "$@"
    fi
}

# ---- SHA256SUMS of all archives (release job) -------------------------------
if [ -n "$checksums_dir" ]; then
    cd "$checksums_dir"
    shopt -s nullglob
    archives=(wpg-*.tar.gz wpg-*.zip)
    [ "${#archives[@]}" -gt 0 ] || die "no wpg-* archives in $checksums_dir"
    sha256 "${archives[@]}" >SHA256SUMS
    info "wrote $checksums_dir/SHA256SUMS"
    cat SHA256SUMS
    exit 0
fi

# ---- version / target -------------------------------------------------------
version="$(sed -n '/^\[workspace.package\]/,/^\[/ s/^version = "\(.*\)"/\1/p' Cargo.toml)"
[ -n "$version" ] || die "cannot read the workspace version from Cargo.toml"
# On a tag build the tag must match the crate version (v0.2.0 <-> 0.2.0).
if [ "${GITHUB_REF_TYPE:-}" = tag ] && [ "${GITHUB_REF_NAME:-}" != "v$version" ]; then
    die "tag ${GITHUB_REF_NAME} does not match the workspace version $version (expected v$version)"
fi

host="$(rustc -vV | sed -n 's/^host: //p')"
[ -n "$target" ] || target="$host"
case "$target" in
    *windows*) exe=wpg.exe ;;
    *) exe=wpg ;;
esac
name="wpg-$version-$target"

# ---- Npcap SDK (Windows + pcap) ----------------------------------------------
# The SDK only provides wpcap.lib for linking; it is not shipped. wpg.exe delay-loads
# wpcap.dll (crates/wpg-cli/build.rs), so it starts without Npcap installed.
NPCAP_SDK_VERSION=1.15
NPCAP_SDK_SHA256=52c7b9fb4abee3ad9fe739bb545c3efe77b731c8e127122bdf328eafdae3ed4f
case "$target,$features" in
    *windows*,*pcap*)
        if [ -z "${LIBPCAP_LIBDIR:-}" ]; then
            sdk="$ROOT/target/npcap-sdk-$NPCAP_SDK_VERSION"
            if [ ! -f "$sdk/Lib/x64/wpcap.lib" ]; then
                info "downloading the Npcap SDK $NPCAP_SDK_VERSION"
                mkdir -p "$sdk"
                zip_file="$sdk.zip"
                curl -fsSL -o "$zip_file" "https://npcap.com/dist/npcap-sdk-$NPCAP_SDK_VERSION.zip"
                echo "$NPCAP_SDK_SHA256  $zip_file" | sha256 -c - >/dev/null \
                    || die "checksum mismatch for npcap-sdk-$NPCAP_SDK_VERSION.zip"
                if command -v unzip >/dev/null 2>&1; then
                    unzip -q -o "$zip_file" -d "$sdk"
                else
                    7z x -y -bso0 "-o$sdk" "$zip_file"
                fi
                rm -f "$zip_file"
            fi
            case "$target" in
                aarch64-*) libdir="$sdk/Lib/ARM64" ;;
                *) libdir="$sdk/Lib/x64" ;;
            esac
            if command -v cygpath >/dev/null 2>&1; then
                libdir="$(cygpath -w "$libdir")"
            fi
            export LIBPCAP_LIBDIR="$libdir"
        fi
        # The pcap crate cannot read the version from the SDK (no DLL in it); Npcap 1.x is
        # based on libpcap 1.10.
        export LIBPCAP_VER="${LIBPCAP_VER:-1.10.0}"
        info "LIBPCAP_LIBDIR=$LIBPCAP_LIBDIR LIBPCAP_VER=$LIBPCAP_VER"
        ;;
esac

# ---- build ------------------------------------------------------------------
if [ "$frontend" -eq 1 ]; then
    command -v trunk >/dev/null 2>&1 || die "trunk not found (cargo install trunk --locked), or pass --skip-frontend"
    info "building the web UI (trunk build --release)"
    (
        cd crates/wpg-web
        # trunk reads NO_COLOR as a boolean: it rejects the usual NO_COLOR=1 (no-color.org)
        # and, in 0.21, an empty value too, so pass "true" or leave it unset.
        if [ -n "${NO_COLOR:-}" ]; then export NO_COLOR=true; else unset NO_COLOR; fi
        trunk build --release
    )
fi
[ -f crates/wpg-web/dist/index.html ] || die "crates/wpg-web/dist/index.html missing: the release would ship without the web UI"

cargo_args=(build --release --locked -p wpg-cli --target "$target")
[ -z "$features" ] || cargo_args+=(--features "$features")
info "cargo ${cargo_args[*]}"
cargo "${cargo_args[@]}"
bin="target/$target/release/$exe"
[ -f "$bin" ] || die "build produced no $bin"
if [ "$target" = "$host" ]; then
    "$bin" --version
fi

# ---- package ----------------------------------------------------------------
mkdir -p "$out"
out="$(cd "$out" && pwd)"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
pkg="$stage/$name"
mkdir -p "$pkg"
cp "$bin" "$pkg/"
cp README.md README.ja.md CHANGELOG.md LICENSE-MIT LICENSE-APACHE "$pkg/"
cp -R templates "$pkg/templates"
cp -R packaging "$pkg/packaging"

case "$target" in
    *windows*)
        archive="$out/$name.zip"
        rm -f "$archive"
        if command -v 7z >/dev/null 2>&1; then
            (cd "$stage" && 7z a -tzip -bso0 "$archive" "$name")
        elif command -v zip >/dev/null 2>&1; then
            (cd "$stage" && zip -qr "$archive" "$name")
        else
            die "neither 7z nor zip found"
        fi
        ;;
    *)
        archive="$out/$name.tar.gz"
        tar -C "$stage" -czf "$archive" "$name"
        ;;
esac
(cd "$out" && sha256 "$(basename "$archive")" >"$(basename "$archive").sha256")
info "wrote $archive"
cat "$archive.sha256"
