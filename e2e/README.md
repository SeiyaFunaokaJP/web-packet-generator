# End-to-end tests (Playwright)

Browser tests of the web UI against a real `wpg serve` (TODO M4). Playwright's
`webServer` starts the debug `wpg` on `127.0.0.1:18080` with a fresh temporary data
directory, the bundled `templates/` library and the admin password from
`playwright.config.ts`; nothing touches a real interface (jobs use the `null` backend).

```sh
just e2e                       # build the front-end + wpg, then run everything

# or step by step
(cd crates/wpg-web && trunk build --release)   # wpg-server embeds crates/wpg-web/dist
cargo build -p wpg-cli
cd e2e
npm ci
npx playwright install chromium   # once
npx playwright test               # --ui / --headed / -g "builder" as usual
```

`WPG_E2E_PORT` and `WPG_E2E_DATA_DIR` override the port and data directory.
Failures keep a trace under `test-results/` (`npx playwright show-trace …`).

| Spec | Covers |
|---|---|
| `auth.spec.ts` | login redirect with `next`, wrong password, logout |
| `builder.spec.ts` | palette → layer stack, WASM/server parity badge, generator expansion, send-time estimate, undo/redo, save |
| `templates.spec.ts` | library search, open in the builder |
| `jobs.spec.ts` | new job on the `null` backend, live statistics, history row |
| `ui.spec.ts` | ja/en switch, narrow-screen view-only note, shortcut help |
