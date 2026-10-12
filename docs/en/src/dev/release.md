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
2. Summarise the changes since the last tag into the `CHANGELOG`, from the Conventional Commits.
3. Bump the workspace version in `Cargo.toml` and update `Cargo.lock`.
4. Commit as `release: vX.Y.Z` and create an annotated tag `vX.Y.Z`.
5. Push the tag.

## Binary distribution (planned)

For v1.0, a GitHub Actions matrix is planned to produce Windows, macOS and Linux binaries with checksums automatically (cargo-dist is the intended tooling; TODO.md M10 / M12). Until then, build from source with the [Getting Started](../getting-started/install-linux.md) steps.
