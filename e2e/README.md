# Web UI end-to-end tests

[Playwright](https://playwright.dev) tests that drive the real web UI against a
real `wpg serve` (TODO.md M4). They cover login / logout, the packet builder
(layer stack, hex / tree preview, browser-vs-server parity, generators, the
stream and send-time estimates, undo / redo, saving and loading templates), job
creation on the `null` backend, the receive page (a signed dry-run job replayed
by a `pcapfile` capture with verification: counters, the per-stream loss table,
the first frames with a decoded frame, live filter validation), and the
language switch.

## Running

```sh
just e2e
```

or step by step, from the repository root:

```sh
(cd crates/wpg-web && trunk build --release)   # the UI; a debug build is ~100 MB of WASM and too slow
touch crates/wpg-server/src/lib.rs             # make rust-embed pick up a dist/ that did not exist before
cargo build -p wpg-cli
cd e2e
npm ci
npx playwright install chromium                # once
npx playwright test
```

`playwright.config.ts` starts `target/debug/wpg serve` on `127.0.0.1:18765`
with a fixed admin password, the repository's `templates/` as the library and
a fresh `e2e/.data/` data directory, and stops it afterwards. Environment
variables:

| Variable | Default | Meaning |
|---|---|---|
| `WPG_BIN` | `../target/debug/wpg` | Server binary to start |
| `WPG_E2E_PORT` | `18765` | Port it listens on |

The specs share one server and run one after another (`workers: 1`). Use
unique names (`helpers.unique`) for anything a spec creates. Prefer role and
accessible-name locators (`getByRole`) over CSS classes; if an element has no
accessible name, add one in the UI rather than reaching for a selector.

On failure, traces are kept under `test-results/`
(`npx playwright show-trace <trace.zip>`); CI uploads them with the HTML report.
