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

# Browser end-to-end tests of the web UI (requires: trunk, Node.js 20+; see e2e/README.md).
e2e:
    cd crates/wpg-web && trunk build --release
    touch crates/wpg-server/src/lib.rs
    cargo build -p wpg-cli
    cd e2e && npm ci && npx playwright install chromium && npx playwright test

# Regenerate tests/golden/*.hex and *.pcap (review the diff before committing).
golden-update:
    WPG_UPDATE_GOLDEN=1 cargo test -p wpg-golden-tests --test golden

# Cross-check golden pcaps with Wireshark (requires tshark).
tshark:
    WPG_TSHARK=1 cargo test -p wpg-golden-tests --test tshark -- --ignored

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
    cargo check -p wpg-core -p wpg-protocols -p wpg-i18n --target wasm32-unknown-unknown

# Security advisories (requires: cargo install cargo-audit --locked).
audit:
    cargo audit

# Licence / ban / source policy (requires: cargo install cargo-deny --locked).
deny:
    cargo deny check

# Regenerate the reference chapters of the docs (protocols, CLI, API, config, template schema)
# from the code; `just docs-gen --check` only reports stale files.
docs-gen *args:
    cargo run -q -p wpg-cli -- docs gen {{args}}

# Build the mdBook documentation into docs/book (requires: cargo install mdbook --locked).
docs:
    ./docs/build.sh

# Deploy over SSH, e.g. `just deploy user@192.0.2.10 --service --iface ens3` (scripts/deploy.sh --help).
deploy host *args:
    #!/usr/bin/env bash
    set -euo pipefail
    h='{{host}}'
    ./scripts/deploy.sh "${h#host=}" {{args}}

# Release archive for the host target in dist/ (see docs: Release process).
dist *args:
    ./scripts/release.sh {{args}}

# Lint the shell scripts (requires shellcheck).
shellcheck:
    shellcheck scripts/*.sh scripts/lib/*.sh docs/build.sh

# Remove build artefacts.
clean:
    cargo clean
    rm -rf crates/wpg-web/dist
