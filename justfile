# web-packet-generator task runner. Run `just --list` to see all recipes.
# Install just: https://github.com/casey/just  (cargo install just)

set shell := ["bash", "-euo", "pipefail", "-c"]
set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

default:
    @just --list

# Build frontend (trunk) then backend (debug). See `release` for the optimised build.
build:
    ./scripts/build.sh

# Optimised build: frontend then backend with --release.
release:
    ./scripts/build.sh --release

# Development server: trunk serve (frontend) + cargo watch (backend).
run:
    #!/usr/bin/env bash
    set -euo pipefail
    missing=0
    if ! command -v trunk >/dev/null 2>&1; then
        echo "error: trunk not found. Install it with: cargo install trunk --locked" >&2
        missing=1
    fi
    if ! cargo watch --version >/dev/null 2>&1; then
        echo "error: cargo-watch not found. Install it with: cargo install cargo-watch --locked" >&2
        missing=1
    fi
    if [ "$missing" -ne 0 ]; then
        echo "hint: without them you can still run: just build && ./target/debug/wpg serve" >&2
        exit 1
    fi
    ./scripts/run.sh --dev

# Run the whole test suite.
test:
    cargo test --workspace

# Format all code.
fmt:
    cargo fmt --all

# Check formatting without modifying files.
fmt-check:
    cargo fmt --all -- --check

# Clippy with warnings denied.
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Ensure the WASM-shared crates still compile for the browser.
check-wasm:
    cargo check -p wpg-core -p wpg-i18n --target wasm32-unknown-unknown

# Security advisories (requires: cargo install cargo-audit --locked).
audit:
    cargo audit

# Licence / ban / source policy (requires: cargo install cargo-deny --locked).
deny:
    cargo deny check

# Build the mdBook documentation (requires: cargo install mdbook --locked).
docs:
    mdbook build docs

# Deploy to a remote host: `just deploy user@192.0.2.10` or `just deploy host=user@192.0.2.10`.
deploy host:
    #!/usr/bin/env bash
    set -euo pipefail
    h='{{host}}'
    ./scripts/deploy.sh "${h#host=}"

# Remove build artefacts.
clean:
    cargo clean
    rm -rf crates/wpg-web/dist
