//! `/login` and `/setup` (first run): full-screen cards outside the shell.

use std::time::Duration;

use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_query_map};
use leptos_router::NavigateOptions;

use crate::api::{self, ApiError};
use crate::auth::{safe_next, use_auth, AuthPhase};
use crate::components::{ErrorBox, BTN_PRIMARY, INPUT, LABEL};
use crate::i18n::{locale_label_key, tr, use_i18n};

/// Minimum password length suggested by the setup form (the server decides).
pub(crate) const MIN_PASSWORD: usize = 8;

/// Client-side check of the setup form: an i18n key when invalid.
pub(crate) fn check_setup(password: &str, confirm: &str) -> Result<(), &'static str> {
    if password.chars().count() < MIN_PASSWORD {
        Err("ui-setup-too-short")
    } else if password != confirm {
        Err("ui-setup-mismatch")
    } else {
        Ok(())
    }
}

/// Centred card with the app name and a language switch.
#[component]
fn AuthCard(#[prop(into)] title: Signal<String>, children: Children) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <main class="flex min-h-full items-center justify-center p-4">
            <div class="w-full max-w-sm">
                <div class="mb-6 flex items-center justify-center gap-2">
                    <span class="grid h-9 w-9 place-items-center rounded-md bg-sky-600 text-sm font-bold text-white">
                        "wpg"
                    </span>
                    <span class="text-lg font-semibold">{tr("app-name")}</span>
                </div>
                <section class="rounded-lg border border-slate-200 bg-white p-6 shadow-sm dark:border-slate-800 dark:bg-slate-900">
                    <h1 class="mb-4 text-lg font-semibold">{move || title.get()}</h1>
                    {children()}
                </section>
                <div class="mt-4 flex justify-center gap-3 text-xs" role="group" aria-label=tr("ui-language")>
                    {wpg_i18n::Locale::ALL
                        .into_iter()
                        .map(|l| {
                            view! {
                                <button
                                    type="button"
                                    lang=l.as_str()
                                    aria-pressed=move || (i18n.locale.get() == l).to_string()
                                    class="text-slate-500 underline-offset-2 hover:underline aria-pressed:font-semibold aria-pressed:text-slate-900 dark:aria-pressed:text-slate-100"
                                    on:click=move |_| i18n.locale.set(l)
                                >
                                    {move || i18n.t(locale_label_key(l))}
                                </button>
                            }
                        })
                        .collect_view()}
                </div>
            </div>
        </main>
    }
}

/// `/login`.
#[component]
pub(crate) fn LoginPage() -> impl IntoView {
    let i18n = use_i18n();
    let auth = use_auth();
    let navigate = use_navigate();
    let query = use_query_map();
    let password = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<ApiError>);
    // Seconds left of a 429 lock-out.
    let wait = RwSignal::new(0u32);

    let tick = move || {
        set_interval_with_handle(
            move || wait.update(|w| *w = w.saturating_sub(1)),
            Duration::from_secs(1),
        )
        .ok()
    };
    let timer = StoredValue::new(tick());
    on_cleanup(move || {
        if let Some(h) = timer.get_value() {
            h.clear();
        }
    });

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() || wait.get_untracked() > 0 {
            return;
        }
        busy.set(true);
        error.set(None);
        let pw = password.get_untracked();
        let navigate = navigate.clone();
        let next = safe_next(query.with_untracked(|q| q.get("next")).as_deref());
        leptos::task::spawn_local(async move {
            match api::auth_login(&pw).await {
                Ok(()) => {
                    password.set(String::new());
                    auth.refresh().await;
                    navigate(&next, NavigateOptions::default());
                }
                Err(e) => {
                    if e.status() == Some(429) {
                        wait.set(e.retry_after().unwrap_or(30));
                    }
                    error.set(Some(e));
                }
            }
            busy.set(false);
        });
    };

    view! {
        <AuthCard title=tr("ui-login-title")>
            <form class="space-y-4" on:submit=submit>
                <div>
                    <label for="login-password" class=LABEL>
                        {tr("ui-password")}
                    </label>
                    <input
                        id="login-password"
                        type="password"
                        class=INPUT
                        autocomplete="current-password"
                        required
                        autofocus
                        prop:value=move || password.get()
                        on:input=move |ev| password.set(event_target_value(&ev))
                    />
                </div>
                {move || {
                    error
                        .get()
                        .map(|e| {
                            if e.status() == Some(401) {
                                view! {
                                    <p role="alert" class="text-sm text-rose-600 dark:text-rose-400">
                                        {tr("ui-login-wrong")}
                                    </p>
                                }
                                    .into_any()
                            } else if e.status() == Some(429) {
                                view! {
                                    <p role="alert" class="text-sm text-amber-700 dark:text-amber-300">
                                        {move || {
                                            i18n.ta("ui-login-rate-limited", &[("secs", wait.get().to_string())])
                                        }}
                                    </p>
                                }
                                    .into_any()
                            } else {
                                view! { <ErrorBox error=e /> }.into_any()
                            }
                        })
                }}
                <button
                    type="submit"
                    class=format!("{BTN_PRIMARY} w-full")
                    disabled=move || { busy.get() || wait.get() > 0 }
                >
                    {move || {
                        if busy.get() {
                            i18n.t("ui-login-busy")
                        } else if wait.get() > 0 {
                            i18n.ta("ui-login-wait", &[("secs", wait.get().to_string())])
                        } else {
                            i18n.t("ui-login-submit")
                        }
                    }}
                </button>
            </form>
            <Show when=move || matches!(auth.phase.get(), AuthPhase::Unreachable(_))>
                <p class="mt-3 text-xs text-rose-600">{tr("ui-server-unreachable")}</p>
            </Show>
        </AuthCard>
    }
}

/// `/setup`: first-run password.
#[component]
pub(crate) fn SetupPage() -> impl IntoView {
    let i18n = use_i18n();
    let auth = use_auth();
    let navigate = use_navigate();
    let password = RwSignal::new(String::new());
    let confirm = RwSignal::new(String::new());
    let touched = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<ApiError>);
    let local = move || check_setup(&password.get(), &confirm.get());

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        touched.set(true);
        if busy.get_untracked() || local().is_err() {
            return;
        }
        busy.set(true);
        error.set(None);
        let (pw, cf) = (password.get_untracked(), confirm.get_untracked());
        let navigate = navigate.clone();
        leptos::task::spawn_local(async move {
            match api::auth_setup(&pw, &cf).await {
                Ok(()) => {
                    auth.refresh().await;
                    let to = if auth.phase.get_untracked().allows_app() {
                        "/"
                    } else {
                        "/login"
                    };
                    navigate(to, NavigateOptions::default());
                }
                Err(e) => error.set(Some(e)),
            }
            busy.set(false);
        });
    };

    view! {
        <AuthCard title=tr("ui-setup-title")>
            <p class="mb-4 text-sm text-slate-600 dark:text-slate-400">{tr("ui-setup-intro")}</p>
            <form class="space-y-4" on:submit=submit novalidate>
                <div>
                    <label for="setup-password" class=LABEL>
                        {tr("ui-password")}
                    </label>
                    <input
                        id="setup-password"
                        type="password"
                        class=INPUT
                        autocomplete="new-password"
                        required
                        autofocus
                        aria-describedby="setup-hint"
                        prop:value=move || password.get()
                        on:input=move |ev| password.set(event_target_value(&ev))
                    />
                    <p id="setup-hint" class="mt-1 text-xs text-slate-500">
                        {move || i18n.ta("ui-setup-hint", &[("min", MIN_PASSWORD.to_string())])}
                    </p>
                </div>
                <div>
                    <label for="setup-confirm" class=LABEL>
                        {tr("ui-password-confirm")}
                    </label>
                    <input
                        id="setup-confirm"
                        type="password"
                        class=INPUT
                        autocomplete="new-password"
                        required
                        prop:value=move || confirm.get()
                        on:input=move |ev| confirm.set(event_target_value(&ev))
                    />
                </div>
                {move || {
                    (touched.get())
                        .then(|| local().err())
                        .flatten()
                        .map(|key| {
                            view! {
                                <p role="alert" class="text-sm text-rose-600 dark:text-rose-400">
                                    {move || {
                                        i18n.ta(key, &[("min", MIN_PASSWORD.to_string())])
                                    }}
                                </p>
                            }
                        })
                }}
                {move || error.get().map(|e| view! { <ErrorBox error=e /> })}
                <button type="submit" class=format!("{BTN_PRIMARY} w-full") disabled=move || busy.get()>
                    {tr("ui-setup-submit")}
                </button>
            </form>
        </AuthCard>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_checks() {
        assert_eq!(check_setup("short", "short"), Err("ui-setup-too-short"));
        assert_eq!(
            check_setup("longenough", "longenougx"),
            Err("ui-setup-mismatch")
        );
        assert_eq!(check_setup("longenough", "longenough"), Ok(()));
        assert_eq!(
            check_setup("パスワード長いです", "パスワード長いです"),
            Ok(())
        );
    }
}
