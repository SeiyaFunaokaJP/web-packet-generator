# web-packet-generator task runner. Run `just --list` to see all recipes.
# Install just: https://github.com/casey/just  (cargo install just --locked, or scripts/setup.sh --just)
#
# Recipes that differ by OS have a [unix] and a [windows] variant; the Windows ones run in
# Windows PowerShell and call the scripts/*.ps1 counterparts.

set shell := ["bash", "-euo", "pipefail", "-c"]
# -ExecutionPolicy Bypass only applies to this process, so the repository's .ps1 scripts
# run even where the machine policy is Restricted (the Windows PowerShell default).
set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command"]

default:
    @just --list

# Install the build tools: pinned toolchain + wasm target, trunk; lists system packages (--just: also just).
[unix]
setup *args:
    ./scripts/setup.sh {{args}}

# Install the build tools: pinned toolchain + wasm target, trunk, Npcap SDK; lists what else to install.
[windows]
setup *args:
    & ./scripts/setup.ps1 {{args}}

# Build frontend (trunk) then the wpg binary (debug). See `release` for the optimised build.
[unix]
build:
    ./scripts/build.sh

# Build frontend (trunk) then the wpg binary (debug). See `release` for the optimised build.
[windows]
build:
    & ./scripts/build.ps1

# Optimised build: frontend then the wpg binary with --release (target/release/wpg).
[unix]
release:
    ./scripts/build.sh --release

# Optimised build: frontend then the wpg binary with -Release (target/release/wpg.exe).
[windows]
release:
    & ./scripts/build.ps1 -Release

# Development server: trunk serve (frontend) + cargo watch (backend).
[windows]
run:
    & ./scripts/run.ps1 -Dev

# Development server: trunk serve (frontend) + cargo watch (backend).
[unix]
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
    cargo test --workspace --no-fail-fast

# Fast check while working: formatting, WASM build, clippy on the default members, core crate tests.
check: fmt-check check-wasm
    cargo clippy --all-targets -- -D warnings
    cargo test -p wpg-core -p wpg-protocols -p wpg-template -p wpg-i18n

# Everything a PR must pass (the checks in CLAUDE.md / CONTRIBUTING.md): run before opening a PR.
ci: fmt-check lint check-wasm test

# Browser end-to-end tests of the web UI (requires: trunk, Node.js 20+; see e2e/README.md).
e2e:
    cd crates/wpg-web && trunk build --release
    touch crates/wpg-server/src/lib.rs
    cargo build -p wpg-cli
    cd e2e && npm ci && npx playwright install chromium && npx playwright test

# Regenerate the documentation screenshots and the builder GIF in docs/{en,ja}/src/images/
# (requires what `e2e` does, plus ffmpeg for the GIF; see e2e/README.md).
screenshots:
    cd crates/wpg-web && trunk build --release
    touch crates/wpg-server/src/lib.rs
    cargo build -p wpg-cli
    cd e2e && npm ci && npx playwright install chromium && npx playwright test -c screenshots.config.ts

# Regenerate tests/golden/*.hex and *.pcap (review the diff before committing).
[unix]
golden-update:
    WPG_UPDATE_GOLDEN=1 cargo test -p wpg-golden-tests --test golden

# Regenerate tests/golden/*.hex and *.pcap (review the diff before committing).
[windows]
golden-update:
    $env:WPG_UPDATE_GOLDEN = '1'; cargo test -p wpg-golden-tests --test golden

# Cross-check golden pcaps with Wireshark (requires tshark).
[unix]
tshark:
    WPG_TSHARK=1 cargo test -p wpg-golden-tests --test tshark -- --ignored

# Cross-check golden pcaps with Wireshark (requires tshark).
[windows]
tshark:
    $env:WPG_TSHARK = '1'; cargo test -p wpg-golden-tests --test tshark -- --ignored

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

# DPDK bindings, EAL smoke test and the dpdk backend on virtual devices (requires: dpdk-dev, libclang-dev; see docs operations/dpdk-afxdp).
check-dpdk:
    cargo clippy -p dpdk-sys -p wpg-tx -p wpg-cli --all-targets --features dpdk-sys/link,wpg-tx/dpdk,wpg-cli/dpdk -- -D warnings
    cargo test -p dpdk-sys --features link
    cargo test -p wpg-tx --features dpdk --lib dpdk

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

# Deploy wizard: asks host, user, passwords, interfaces... (Windows: scripts\deploy.bat).
deploy-wizard:
    ./scripts/deploy.sh --interactive

# Release archive for the host target in dist/ (see docs: Release process).
dist *args:
    ./scripts/release.sh {{args}}

# Smoke-test a release archive: contents, --version, doctor, render, serve (see docs: Release process).
smoke-archive archive *args:
    ./scripts/smoke-archive.sh {{args}} {{archive}}

# Lint the shell scripts (requires shellcheck).
shellcheck:
    shellcheck scripts/*.sh scripts/lib/*.sh docs/build.sh

# Remove build artefacts.
[unix]
clean:
    cargo clean
    rm -rf crates/wpg-web/dist

# Remove build artefacts.
[windows]
clean:
    cargo clean
    if (Test-Path crates/wpg-web/dist) { Remove-Item -Recurse -Force crates/wpg-web/dist }
