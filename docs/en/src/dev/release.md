# Release process

## Versioning

The workspace has a single version (`[workspace.package] version` in `Cargo.toml`; each crate uses `version.workspace = true`). It follows [Semantic Versioning](https://semver.org/); breaking changes are possible until v1.0.

## Release gates

Before tagging, make sure all of the following are green. On top of the usual pre-push checks (CLAUDE.md) there are protocol, security and documentation gates.

```sh
# Pre-push checks
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
cargo check -p wpg-core -p wpg-protocols -p wpg-i18n --target wasm32-unknown-unknown

# Protocols (golden pcaps and the tshark cross-check)
cargo test -p wpg-golden-tests
WPG_TSHARK=1 cargo test -p wpg-golden-tests -- --ignored

# Security (just audit / just deny)
cargo audit
cargo deny check
```

The v1.0 release gate (TODO.md §4, M12) additionally requires:

- All P0 + P1 protocol golden pcaps and tshark cross-checks green.
- Build, start and pcap-backend transmission confirmed on all three OSes.
- The web UI end-to-end tests (`just e2e`) green, and zero ja / en i18n gaps (`cargo test -p wpg-i18n`).
- Documentation complete in ja and en, with `docs/build.sh` passing.
- The `CHANGELOG` updated.

## Steps

1. Pass every gate above.
2. Summarise the changes since the last tag into the `CHANGELOG`, from the Conventional Commits. Call out upgrade notes (template schema, database or configuration changes, see [Upgrading](../operations/upgrade.md)).
3. Bump the workspace version in `Cargo.toml` (also the `version` of the internal `wpg-*` entries in `[workspace.dependencies]`) and update `Cargo.lock` (`cargo check`).
4. Commit as `release: vX.Y.Z` and create an annotated tag `vX.Y.Z` (`git tag -a vX.Y.Z -m "wpg X.Y.Z"`).
5. Push the tag (`git push origin vX.Y.Z`). `.github/workflows/release.yml` builds the archives, writes `SHA256SUMS` and creates the GitHub Release with generated notes; edit the notes afterwards. Tags with a hyphen (`v1.0.0-rc.1`) become pre-releases. `scripts/release.sh` fails when the tag does not match the workspace version.

A manual run of the workflow (`workflow_dispatch`) builds the archives and keeps them as workflow artifacts without publishing anything; use it to test the pipeline.

## Binary distribution

Releases are built by `.github/workflows/release.yml`. Each target is built on its own runner by `scripts/release.sh`, and a final job publishes a GitHub Release with all archives and a `SHA256SUMS` file.

### Targets

| Archive | Runner | Notes |
|---|---|---|
| `wpg-<ver>-x86_64-unknown-linux-gnu.tar.gz` | `ubuntu-22.04` | Built against glibc 2.35, so it runs on Ubuntu 22.04 / Debian 12 and newer |
| `wpg-<ver>-aarch64-apple-darwin.tar.gz` | `macos-14` | Apple silicon |
| `wpg-<ver>-x86_64-apple-darwin.tar.gz` | `macos-14` (cross) | Intel Macs |
| `wpg-<ver>-x86_64-pc-windows-msvc.zip` | `windows-latest` | Needs Npcap at runtime |

Each archive holds `wpg` / `wpg.exe` (with the web UI embedded), `README.md`, `README.ja.md`, both licences, `templates/` and `packaging/` (systemd, launchd and Windows service samples). Binaries are built without optional features (no `pcap`), so they need no extra library to start. They are **not signed or notarised**; see the macOS and Windows install pages for what users have to do.

### Building an archive locally

```sh
scripts/release.sh                                    # host target, into dist/
scripts/release.sh --target x86_64-apple-darwin       # cross target (needs its linker / SDK)
scripts/release.sh --skip-frontend --features pcap    # reuse crates/wpg-web/dist
scripts/release.sh --checksums dist                   # SHA256SUMS of everything in dist/
```

It needs `trunk` (`cargo install trunk --locked`) unless `--skip-frontend` is given, and refuses to package without `crates/wpg-web/dist/index.html` so no release ships without the web UI. On Windows run it from Git Bash (it uses `7z` or `zip`).

### Why not cargo-dist

The roadmap planned cargo-dist. The pipeline uses a small script instead because wpg needs the Trunk front-end build before `cargo build` (the UI is embedded by `rust-embed`), and the archives carry templates and service files beside the binary. cargo-dist can do both through custom build-setup steps and `include`, but its generated workflow has to be regenerated whenever cargo-dist is upgraded. `release.sh` is the same script locally and in CI. Installers (shell / PowerShell / Homebrew / MSI) are not produced; revisit cargo-dist if they are wanted.

### Signing

Not done yet. macOS users have to remove the quarantine attribute ([Install on macOS](../getting-started/install-macos.md)); Windows SmartScreen may warn about the unknown publisher ([Install on Windows](../getting-started/install-windows.md)). Signing and notarisation need an Apple Developer ID and a code-signing certificate, which the project does not have.
