# wpg-web

The browser UI of wpg: **Leptos 0.8 (CSR) + Tailwind CSS v4**, compiled to
`wasm32-unknown-unknown` with [Trunk](https://trunkrs.dev). The build output
(`dist/`) is embedded into `wpg-server` with `rust-embed` and served at `/`
(SPA fallback), so the released `wpg` binary needs no separate web server.

## Build

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk --locked            # 0.21+

cd crates/wpg-web
trunk build --release                   # -> crates/wpg-web/dist (gitignored)
cd ../.. && cargo build --release -p wpg-cli   # embeds dist/ into the binary
```

* The release build uses the size-optimised `wasm-release` Cargo profile and
  `wasm-opt -Oz` (see `index.html`); about 1.5 MB of WASM, ~0.6 MB gzip.
* Tailwind is the **standalone CLI**, downloaded by Trunk on first use (version
  pinned in `Trunk.toml`); no Node.js is needed. The input is
  `style/tailwind.css`; class names must appear as complete literals in `src/`.
* In debug builds `rust-embed` reads `dist/` from disk at run time, so a
  `trunk build` is picked up by a running debug server on the next request.

### Development

```sh
wpg serve --dev                         # API on :8080, CORS for :8081
cd crates/wpg-web && trunk serve        # UI on :8081, /api and /api/ws proxied
```

### Tests and lints

The crate also compiles natively: pure logic is unit tested on the host, browser
calls are gated on `target_arch = "wasm32"`.

```sh
cargo test -p wpg-web
cargo clippy -p wpg-web --target wasm32-unknown-unknown -- -D warnings
```

`i18n::tests::ui_keys_exist` scans the sources for `"ui-…"` keys and fails when
one is missing from `locales/{en,ja}/main.ftl` (and `wpg-i18n` checks that both
locales have the same keys).

## Layout

| Path | What |
|---|---|
| `src/main.rs` | Entry point (`mount_to_body`) |
| `src/app.rs` | Contexts, router, shell (sidebar, top bar, logs drawer, help), auth guard, global shortcuts |
| `src/api/` | Typed REST client (`gloo-net`): `Accept-Language`, `X-CSRF-Token`, 401 → re-check session, `{error:{code,message,details}}` → `ApiError`; `types.rs` mirrors the server JSON |
| `src/ws.rs` | `/api/ws` client: subscribe, reconnect with back-off, live signals (per-job stats + 60-sample history, job states, logs, template changes) |
| `src/auth.rs` | Session phase from `GET /api/auth/status`, redirects to `/setup` / `/login` |
| `src/i18n.rs`, `src/prefs.rs`, `src/fmt.rs` | Fluent via `wpg-i18n`, theme / language in `localStorage`, `Intl` number / date formatting |
| `src/components/` | Icons, buttons, modal / confirm / drawer, badges, toasts, logs panel |
| `src/pages/` | Dashboard, jobs (list, form, detail drawer), templates, settings, login / setup, placeholders |
| `src/builder/` | Packet builder — phase 2 (placeholder route; planned module layout in `mod.rs`) |
| `src/validate.rs`, `src/shortcuts.rs`, `src/sparkline.rs`, `src/dom.rs` | Rate / duration validation, `g`-chord parser, SVG sparkline paths, browser helpers |

## Routes and shortcuts

| Route | Screen | Keys |
|---|---|---|
| `/` | Dashboard | `g d` |
| `/builder` (`?template=<id>`) | Packet builder (phase 2) | `g b` |
| `/jobs` (`?new=1\|<template>`, `?job=<id>`) | Jobs, new-job form, detail drawer | `g j`, `n` |
| `/templates` | Template library | `g t` |
| `/rx` | Receive (M9) | `g r` |
| `/settings` | Settings, about, doctor | `g s` |
| `/login`, `/setup` | Authentication | |

`l` toggles the server log, `?` shows the shortcuts, `Esc` closes dialogs.
Narrow screens (< 768 px) are view-only (TODO §2.5).

## End-to-end tests

Playwright tests of these screens against a real `wpg serve` live in [`e2e/`](../../e2e/README.md) (`just e2e`).
