# Release process

Status: draft

Releases are built by `.github/workflows/release.yml`. Each target is built on its own runner by `scripts/release.sh`, and a final job publishes a GitHub Release with all archives and a `SHA256SUMS` file.

## Targets

| Archive | Runner | Notes |
|---|---|---|
| `wpg-<ver>-x86_64-unknown-linux-gnu.tar.gz` | `ubuntu-22.04` | Built against glibc 2.35, so it runs on Ubuntu 22.04 / Debian 12 and newer |
| `wpg-<ver>-aarch64-apple-darwin.tar.gz` | `macos-14` | Apple silicon |
| `wpg-<ver>-x86_64-apple-darwin.tar.gz` | `macos-14` (cross) | Intel Macs |
| `wpg-<ver>-x86_64-pc-windows-msvc.zip` | `windows-latest` | Needs Npcap at runtime |

Each archive holds `wpg` / `wpg.exe` (with the web UI embedded), `README.md`, `README.ja.md`, both licences, `templates/` and `packaging/` (systemd, launchd and Windows service samples). Binaries are built without optional features (no `pcap`), so they need no extra library to start. They are **not signed or notarised**; see the macOS and Windows install pages for what users have to do.

## Cutting a release

1. Make sure `main` is green, including `cargo audit`, `cargo deny`, the golden / tshark checks and the i18n lint. Before v1.0 the CHANGELOG and both language docs must be complete as well.
2. Bump `version` in `[workspace.package]` of the root `Cargo.toml` (and the internal `wpg-*` entries in `[workspace.dependencies]`), run `cargo check` to update `Cargo.lock`, and merge that change.
3. Tag the merge commit and push the tag:

   ```sh
   git tag -a v0.2.0 -m "wpg 0.2.0"
   git push origin v0.2.0
   ```

4. The workflow builds the four archives, writes `SHA256SUMS` and creates the release with generated notes. Tags with a hyphen (`v1.0.0-rc.1`) become pre-releases. `release.sh` fails when the tag does not match the workspace version.
5. Edit the release notes: list upgrade notes (template schema, database or configuration changes, see [Upgrading](../operations/upgrade.md)).

A manual run of the workflow (`workflow_dispatch`) builds the archives and keeps them as workflow artifacts without publishing anything; use it to test the pipeline.

## Building an archive locally

```sh
scripts/release.sh                                    # host target, into dist/
scripts/release.sh --target x86_64-apple-darwin       # cross target (needs its linker / SDK)
scripts/release.sh --skip-frontend --features pcap    # reuse crates/wpg-web/dist
scripts/release.sh --checksums dist                   # SHA256SUMS of everything in dist/
```

It needs `trunk` (`cargo install trunk --locked`) unless `--skip-frontend` is given, and refuses to package without `crates/wpg-web/dist/index.html` so no release ships without the web UI. On Windows run it from Git Bash (it uses `7z` or `zip`).

## Why not cargo-dist

The roadmap planned cargo-dist. The pipeline uses a small script instead because wpg needs the Trunk front-end build before `cargo build` (the UI is embedded by `rust-embed`), and the archives carry templates and service files beside the binary. cargo-dist can do both through custom build-setup steps and `include`, but its generated workflow has to be regenerated whenever cargo-dist is upgraded. `release.sh` is the same script locally and in CI. Installers (shell / PowerShell / Homebrew / MSI) are not produced; revisit cargo-dist if they are wanted.

## Signing

Not done yet. macOS users have to remove the quarantine attribute ([Install on macOS](../getting-started/install-macos.md)); Windows SmartScreen may warn about the unknown publisher ([Install on Windows](../getting-started/install-windows.md)). Signing and notarisation need an Apple Developer ID and a code-signing certificate, which the project does not have.
