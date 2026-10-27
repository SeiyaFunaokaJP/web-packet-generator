#!/usr/bin/env bash
# Host side of the deploy wizards: builds wpg from source on the target host.
#
#   remote-build.sh --deps                      (as root) install the build dependencies
#   remote-build.sh --build --out <file> ...    (as the SSH user) build and copy the binary
#
# Build options:
#   --tarball <file>     source tree as .tar.gz (uploaded from the deploying machine)
#   --git-url <url>      or: fetch the source with git (default ref: main)
#   --git-ref <ref>      branch, tag or commit for --git-url
#   --workdir <dir>      build directory, kept between runs so rebuilds are incremental
#                        (default ~/wpg-build: src/ is replaced, target/ is reused)
#   --features <list>    extra cargo features (e.g. dpdk)
#   --out <file>         where to copy the built binary
#
# Installs rustup (stable, minimal profile, into ~/.cargo of the user), the
# wasm32-unknown-unknown target and trunk when they are missing.
set -euo pipefail

mode=""
tarball=""
git_url=""
git_ref="main"
workdir="${HOME:-/tmp}/wpg-build"
features=""
out=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --deps) mode=deps ;;
        --build) mode=build ;;
        --tarball) tarball="$2"; shift ;;
        --git-url) git_url="$2"; shift ;;
        --git-ref) git_ref="$2"; shift ;;
        --workdir) workdir="$2"; shift ;;
        --features) features="$2"; shift ;;
        --out) out="$2"; shift ;;
        *) echo "remote-build: unknown argument: $1" >&2; exit 2 ;;
    esac
    shift
done

info() { echo "  -> $*"; }
warn() { echo "  warning: $*" >&2; }
die() { echo "  error: $*" >&2; exit 1; }

# ---- dependencies (root) ----------------------------------------------------
if [ "$mode" = deps ]; then
    [ "$(id -u)" -eq 0 ] || die "--deps must run as root"
    if command -v apt-get >/dev/null 2>&1; then
        export DEBIAN_FRONTEND=noninteractive
        info "apt-get update"
        apt-get update -q
        info "apt-get install build-essential pkg-config libpcap-dev curl git ca-certificates tar"
        apt-get install -y -q --no-install-recommends \
            build-essential pkg-config libpcap-dev curl git ca-certificates tar gzip libcap2-bin
    elif command -v dnf >/dev/null 2>&1 || command -v yum >/dev/null 2>&1; then
        pm="$(command -v dnf || command -v yum)"
        info "$pm install gcc gcc-c++ make pkgconf-pkg-config libpcap-devel curl git tar (best effort)"
        "$pm" install -y gcc gcc-c++ make pkgconf-pkg-config libpcap-devel curl git ca-certificates tar gzip libcap \
            || die "installing the build dependencies with $pm failed; install a C compiler, pkg-config, libpcap headers, curl and git, then run again"
    else
        die "no apt-get or dnf on this host: install a C compiler (gcc), pkg-config, libpcap headers, curl and git by hand, then run again"
    fi
    info "build dependencies installed"
    exit 0
fi

[ "$mode" = build ] || die "pass --deps or --build"
[ -n "$out" ] || die "--out is required"
[ "$(id -u)" -ne 0 ] || warn "building as root; the toolchain goes to /root/.cargo"

# ---- toolchain (user) -------------------------------------------------------
export PATH="$HOME/.cargo/bin:$PATH"
if ! command -v rustup >/dev/null 2>&1; then
    info "installing rustup (https://rustup.rs) into ~/.cargo"
    command -v curl >/dev/null 2>&1 || die "curl missing (run the dependency step first)"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --default-toolchain stable --no-modify-path
fi
info "rust toolchain: stable + wasm32-unknown-unknown"
rustup toolchain install stable --profile minimal >/dev/null 2>&1 \
    || rustup toolchain install stable --profile minimal
rustup target add --toolchain stable wasm32-unknown-unknown

if ! command -v trunk >/dev/null 2>&1; then
    arch="$(uname -m)"
    url=""
    case "$arch" in
        x86_64 | aarch64) url="https://github.com/trunk-rs/trunk/releases/latest/download/trunk-$arch-unknown-linux-gnu.tar.gz" ;;
    esac
    tmp="$(mktemp -d)"
    if [ -n "$url" ] && curl -fsSL --max-time 300 "$url" -o "$tmp/trunk.tar.gz" \
        && tar -xzf "$tmp/trunk.tar.gz" -C "$tmp" && [ -f "$tmp/trunk" ] \
        && "$tmp/trunk" --version >/dev/null 2>&1; then
        install -m 0755 "$tmp/trunk" "$HOME/.cargo/bin/trunk"
        info "installed $(trunk --version) (prebuilt)"
    else
        info "building trunk with cargo install trunk --locked (takes a few minutes)"
        cargo install trunk --locked
    fi
    rm -rf "$tmp"
fi

# ---- source -----------------------------------------------------------------
mkdir -p "$workdir"
src="$workdir/src"
if [ -n "$tarball" ]; then
    [ -f "$tarball" ] || die "source tarball missing: $tarball"
    info "unpacking the uploaded source into $src"
    rm -rf "$src"
    mkdir -p "$src"
    tar -xzf "$tarball" -C "$src"
elif [ -n "$git_url" ]; then
    command -v git >/dev/null 2>&1 || die "git missing (run the dependency step first)"
    info "fetching $git_url ($git_ref)"
    if [ ! -d "$src/.git" ]; then
        rm -rf "$src"
        git init -q "$src"
        git -C "$src" remote add origin "$git_url"
    else
        git -C "$src" remote set-url origin "$git_url"
    fi
    git -C "$src" fetch -q --depth 1 origin "$git_ref" \
        || die "git fetch of '$git_ref' from $git_url failed (wrong branch / tag name, or no access to GitHub?)"
    git -C "$src" checkout -q -f FETCH_HEAD
    git -C "$src" clean -q -fdx
    info "source at $(git -C "$src" rev-parse --short HEAD)"
else
    die "pass --tarball or --git-url"
fi
[ -f "$src/scripts/build.sh" ] || die "$src does not look like the wpg repository (scripts/build.sh missing)"
# A checkout made on Windows may carry CRLF line endings, which bash does not accept.
find "$src/scripts" -name '*.sh' -exec sed -i 's/\r$//' {} +

# ---- build ------------------------------------------------------------------
mem_mb="$(awk '/^MemTotal:/ {print int($2 / 1024)}' /proc/meminfo 2>/dev/null || echo 4096)"
swap_mb="$(awk '/^SwapTotal:/ {print int($2 / 1024)}' /proc/meminfo 2>/dev/null || echo 0)"
cpus="$(nproc 2>/dev/null || echo 1)"
# About 1.5 GiB per rustc job in release mode; fewer jobs instead of running out of memory.
jobs=$(((mem_mb + swap_mb) / 1536))
[ "$jobs" -ge 1 ] || jobs=1
[ "$jobs" -le "$cpus" ] || jobs="$cpus"
if [ $((mem_mb + swap_mb)) -lt 2000 ]; then
    warn "only $((mem_mb + swap_mb)) MiB of memory + swap: the build may be killed. Add swap (e.g. sudo fallocate -l 4G /swapfile && sudo chmod 600 /swapfile && sudo mkswap /swapfile && sudo swapon /swapfile) or more memory."
fi
export CARGO_BUILD_JOBS="$jobs"
export CARGO_TARGET_DIR="${WPG_BUILD_TARGET_DIR:-$workdir/target}"
export CARGO_TERM_COLOR=never NO_COLOR=1

build_args=(--release)
[ -z "$features" ] || build_args+=(--features "$features")
info "building in $src with $jobs job(s) (first build: 10-30 minutes)"
(cd "$src" && rustup target add wasm32-unknown-unknown >/dev/null 2>&1; bash scripts/build.sh "${build_args[@]}")

bin="$CARGO_TARGET_DIR/release/wpg"
[ -x "$bin" ] || die "build finished but $bin is missing"
cp -f "$bin" "$out"
chmod 0755 "$out"
info "built $("$out" --version)"
