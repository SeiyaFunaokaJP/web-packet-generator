# Contributing to wpg

**English** | [日本語](CONTRIBUTING.ja.md)

Thank you for your interest! Please read the [Code of Conduct](CODE_OF_CONDUCT.md) first. The roadmap and
specification are in [TODO.md](TODO.md).

## Development setup

1. Install [rustup](https://rustup.rs). The pinned toolchain in `rust-toolchain.toml` is picked up automatically.
2. Add the WASM target: `rustup target add wasm32-unknown-unknown`
3. Install tools: `cargo install just trunk cargo-watch cargo-deny cargo-audit mdbook --locked`
   (only `just` and `trunk` are needed for a basic build).
4. Linux: `sudo apt install libpcap-dev`. Windows: Npcap SDK. (Optional until the pcap backend lands.)
5. `just build && ./target/debug/wpg serve`, then open <http://127.0.0.1:8080>.

## Common tasks (`just --list`)

| Recipe | What it does |
|---|---|
| `just build` / `just release` | Build the frontend (trunk), then the backend |
| `just run` | Dev server with hot reload (`trunk serve` + `cargo watch`) |
| `just test` | `cargo test --workspace` |
| `just e2e` | Playwright tests of the web UI against a real `wpg serve` ([e2e/README.md](e2e/README.md)) |
| `just fmt` / `just fmt-check` | Format / check formatting |
| `just lint` | `cargo clippy ... -D warnings` |
| `just check-wasm` | Ensure `wpg-core` and `wpg-i18n` build for `wasm32-unknown-unknown` |
| `just audit` / `just deny` | `cargo audit` / `cargo deny check` |
| `just docs-gen` | Regenerate the reference chapters of the docs from the code |
| `just docs` | Build the mdBook docs |
| `just clean` | Remove build artefacts |

Before opening a PR, run `just fmt lint test check-wasm`. CI runs the same checks (plus `cargo deny` and
`cargo audit`) on Linux, macOS and Windows.

## Commits and pull requests

- Use [Conventional Commits](https://www.conventionalcommits.org): `feat(core): ...`, `fix(tx): ...`,
  `docs: ...`, `ci: ...`, `chore: ...`. The scope is the crate name without the `wpg-` prefix.
- Keep PRs focused and small; one logical change per PR. Reference the TODO.md milestone or issue.
- Add tests. Protocol work needs a golden pcap and, where possible, a `tshark -V` cross-check.
- Update docs, and update TODO.md checkboxes only when the item is fully done.
- Do not force-push over review comments; squash happens on merge.
- Changes to a decision recorded in TODO.md section 3 need an ADR in `docs/adr/`.

## Adding a protocol

Protocols are declarative definitions in `wpg-protocols` (Rust macros for core protocols, YAML for
user-extensions), lowered to one shared IR. Follow the step-by-step guide in `docs/en/src/dev/add-protocol.md`
(developer guide: adding a protocol). At a minimum a protocol needs: a definition, Fluent strings for its
name and field descriptions, a sample template, a golden pcap and tests. Then run `just docs-gen` to
regenerate the reference pages (protocols, CLI, API, configuration, template schema) and commit them;
a test fails while they are stale.

## Internationalisation (i18n)

- **No hard-coded user-visible strings** in the server, CLI, or web UI (including error messages).
  Use Fluent message keys.
- Every key must exist in **both** `locales/ja/main.ftl` and `locales/en/main.ftl`. A missing key in either
  language fails CI (`cargo test -p wpg-i18n`).
- Adding a language: see the developer guide, `docs/en/src/dev/add-i18n.md`.

## Licensing and DCO

wpg is dual licensed under MIT OR Apache-2.0. Unless you explicitly state otherwise, any contribution
intentionally submitted for inclusion in this project, as defined in the Apache-2.0 license, is dual
licensed as above without additional terms or conditions.

By contributing you certify that you wrote the contribution or otherwise have the right to submit it under
these licences (the spirit of the [Developer Certificate of Origin](https://developercertificate.org/)).
Please sign off commits with `git commit -s` (`Signed-off-by: Your Name <you@example.com>`).

## Responsible use

wpg is for testing networks you own or are authorised to test. Contributions aimed at attack tooling will
not be accepted.
