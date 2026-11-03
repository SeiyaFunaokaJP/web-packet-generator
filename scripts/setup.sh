#!/usr/bin/env bash
# Prepare this machine (Linux / macOS) for building wpg from source.
#
# Usage: scripts/setup.sh [--just] [--no-trunk]
#   --just       also install the just task runner (cargo install just --locked)
#   --no-trunk   do not install trunk (backend-only builds, no web UI)
#
# Needs rustup (https://rustup.rs). Installs the toolchain pinned in rust-toolchain.toml
# (with the wasm32 target) and a prebuilt trunk at the version pinned in
# scripts/versions.env, then lists the system packages to install yourself.
# Windows: use scripts\setup.ps1.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

want_just=0
want_trunk=1
while [ "$#" -gt 0 ]; do
    case "$1" in
        --just) want_just=1 ;;
        --no-trunk) want_trunk=0 ;;
        -h | --help)
            sed -n '2,11p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "error: unknown argument: $1" >&2
            exit 2
            ;;
    esac
    shift
done

info() { echo "==> $*"; }
warn() { echo "warning: $*" >&2; }
die() { echo "error: $*" >&2; exit 1; }

# KEY=value from scripts/versions.env (tolerates CRLF line endings).
pinned() { sed -n "s/^$1=//p" "$ROOT/scripts/versions.env" | tr -d '\r' | head -n 1; }

TRUNK_VERSION="$(pinned TRUNK_VERSION)"
[ -n "$TRUNK_VERSION" ] || die "TRUNK_VERSION missing in scripts/versions.env"

# ---- Rust toolchain ---------------------------------------------------------
command -v rustup >/dev/null 2>&1 \
    || die "rustup not found. Install it from https://rustup.rs, open a new terminal, then run this script again."
info "Rust toolchain from rust-toolchain.toml"
# rustup 1.28+ installs the pinned toolchain with a bare `toolchain install`;
# older versions do it on `rustup show`.
rustup toolchain install --no-self-update 2>/dev/null || rustup show active-toolchain >/dev/null
rustup target add wasm32-unknown-unknown
cargo --version

cargo_bin="${CARGO_HOME:-$HOME/.cargo}/bin"

# ---- trunk ------------------------------------------------------------------
sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

install_trunk() {
    local os arch asset base tmp expected actual
    case "$(uname -s)" in
        Linux) os=unknown-linux-gnu ;;
        Darwin) os=apple-darwin ;;
        *) os="" ;;
    esac
    arch="$(uname -m)"
    [ "$arch" = arm64 ] && arch=aarch64
    case "$arch" in x86_64 | aarch64) ;; *) os="" ;; esac

    if [ -n "$os" ] && command -v curl >/dev/null 2>&1; then
        asset="trunk-$arch-$os.tar.gz"
        base="https://github.com/trunk-rs/trunk/releases/download/v$TRUNK_VERSION"
        tmp="$(mktemp -d)"
        if curl -fsSL --max-time 300 -o "$tmp/$asset" "$base/$asset" \
            && curl -fsSL --max-time 60 -o "$tmp/$asset.sha256" "$base/$asset.sha256"; then
            expected="$(cut -d' ' -f1 <"$tmp/$asset.sha256" | tr -d '\r\n')"
            actual="$(sha256_of "$tmp/$asset")"
            if [ "$expected" = "$actual" ] && tar -xzf "$tmp/$asset" -C "$tmp" trunk \
                && "$tmp/trunk" --version >/dev/null 2>&1; then
                mkdir -p "$cargo_bin"
                install -m 0755 "$tmp/trunk" "$cargo_bin/trunk"
                rm -rf "$tmp"
                return 0
            fi
            warn "the downloaded $asset did not verify; building trunk instead"
        fi
        rm -rf "$tmp"
    fi
    info "cargo install trunk --version $TRUNK_VERSION --locked (takes a few minutes)"
    cargo install trunk --version "$TRUNK_VERSION" --locked
}

if [ "$want_trunk" -eq 1 ]; then
    current="$(trunk --version 2>/dev/null | awk '{print $2}' || true)"
    if [ "$current" = "$TRUNK_VERSION" ]; then
        info "trunk $TRUNK_VERSION already installed"
    else
        info "installing trunk $TRUNK_VERSION${current:+ (found $current)}"
        install_trunk
        case ":$PATH:" in
            *":$cargo_bin:"*) ;;
            *) warn "$cargo_bin is not on PATH; add it (or open a new terminal) before building" ;;
        esac
    fi
fi

# ---- just -------------------------------------------------------------------
if [ "$want_just" -eq 1 ]; then
    if command -v just >/dev/null 2>&1; then
        info "$(just --version) already installed"
    else
        info "cargo install just --locked"
        cargo install just --locked
    fi
fi

# ---- system packages (printed, not installed) -------------------------------
echo
case "$(uname -s)" in
    Linux)
        missing=""
        command -v cc >/dev/null 2>&1 || missing="$missing build-essential"
        command -v pkg-config >/dev/null 2>&1 || missing="$missing pkg-config"
        if [ -n "$missing" ]; then
            echo "Install a C toolchain first (Debian / Ubuntu):"
            echo "    sudo apt install$missing"
            echo "Fedora / RHEL: sudo dnf install gcc pkgconf-pkg-config"
            echo
        fi
        echo "Linux sends through AF_PACKET with no extra library. Only for the optional"
        echo "libpcap backend (--features pcap):"
        echo "    sudo apt install libpcap-dev        # Fedora / RHEL: sudo dnf install libpcap-devel"
        ;;
    Darwin)
        if ! xcode-select -p >/dev/null 2>&1; then
            echo "Install the Xcode command line tools (compiler and linker):"
            echo "    xcode-select --install"
            echo
        fi
        echo "macOS ships libpcap; the build script adds the pcap backend automatically."
        ;;
esac

echo
echo "Setup done. Next:"
if command -v just >/dev/null 2>&1; then
    echo "    just release && ./target/release/wpg serve"
else
    echo "    ./scripts/build.sh --release && ./target/release/wpg serve"
fi
