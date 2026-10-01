//! wpg web UI (Leptos CSR + Tailwind). Built for `wasm32-unknown-unknown` by Trunk;
//! the output in `dist/` is embedded into `wpg-server` (rust-embed) and served at `/`.
//!
//! The crate also compiles natively so that `cargo clippy/test --workspace` cover
//! it: the pure logic (rate validation, sparkline paths, shortcut parsing, number
//! formatting, WebSocket back-off, ...) is unit tested on the host, while every
//! browser API is only *called* on `wasm32`.
//!
//! Module layout (see `README.md`):
//!
//! * [`app`]: router, shell (sidebar, top bar, logs drawer, help overlay), auth guard;
//! * [`api`]: typed REST client (`gloo-net`), CSRF header, 401 handling, errors;
//! * [`ws`]: `/api/ws` client with reconnect / back-off, live signals;
//! * [`auth`], [`i18n`], [`prefs`]: session, translations (Fluent), theme / locale;
//! * [`components`]: icons, dialogs, badges, toasts, logs panel, sparkline;
//! * [`pages`]: dashboard, jobs, templates, settings, login / setup, placeholders;
//! * [`builder`]: the packet builder (phase 2; placeholder route for now);
//! * [`fmt`], [`validate`], [`shortcuts`], [`sparkline`], [`dom`]: helpers.

// `#[component]` expands to items the lint misreads in a binary crate.
#![allow(unreachable_pub)]
// Natively only the pure logic is exercised (tests, clippy); the browser glue
// (`#[cfg(target_arch = "wasm32")]`) is what uses the rest.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

mod api;
mod app;
mod auth;
mod builder;
mod components;
mod dom;
mod fmt;
mod i18n;
mod pages;
mod prefs;
mod shortcuts;
mod sparkline;
mod validate;
mod ws;

fn main() {
    if cfg!(not(target_arch = "wasm32")) {
        eprintln!("wpg-web is a browser application: build it with `trunk build` (see README.md).");
        return;
    }
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);
}
