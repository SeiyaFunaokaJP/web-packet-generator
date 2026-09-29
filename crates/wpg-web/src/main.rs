//! wpg web UI (Leptos CSR). Built for `wasm32-unknown-unknown` by Trunk; the output in
//! `dist/` is embedded into `wpg-server`.
//!
//! The crate also compiles natively so `cargo clippy --workspace` covers it, but it is
//! only meaningful in a browser.

use leptos::prelude::*;
use wpg_i18n::{negotiate, t, Locale};

/// Initial locale from the browser (`navigator.language`), falling back to English.
fn browser_locale() -> Locale {
    web_sys::window()
        .and_then(|w| w.navigator().language())
        .map(|lang| negotiate(&lang))
        .unwrap_or_default()
}

/// Root component.
#[component]
fn App() -> impl IntoView {
    let (locale, set_locale) = signal(browser_locale());

    // Keep <html lang> in sync for screen readers / fonts.
    Effect::new(move |_| {
        if let Some(root) = document().document_element() {
            let _ = root.set_attribute("lang", locale.get().as_str());
        }
    });

    view! {
        <header>
            <h1>{move || t(locale.get(), "app-name")}</h1>
            <p>{move || t(locale.get(), "app-tagline")}</p>
        </header>
        <nav aria-label=move || t(locale.get(), "ui-language")>
            {Locale::ALL
                .into_iter()
                .map(|l| {
                    view! {
                        <button
                            type="button"
                            aria-pressed=move || (locale.get() == l).to_string()
                            on:click=move |_| set_locale.set(l)
                        >
                            {l.as_str()}
                        </button>
                    }
                })
                .collect_view()}
        </nav>
    }
}

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}
