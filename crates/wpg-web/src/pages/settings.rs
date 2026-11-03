//! `/settings`: language, theme, account, client settings, about, user-defined
//! protocols, doctor report.

use leptos::prelude::*;

use crate::api;
use crate::api::tokens::TokenInfo;
use crate::app::use_ui;
use crate::auth::{use_auth, AuthPhase};
use crate::components::{
    use_toasts, Confirm, ErrorBox, Icon, IconName, Loading, PageHeader, BTN, BTN_GHOST,
    BTN_PRIMARY, CARD, INPUT, LABEL, TD, TH,
};
use crate::i18n::{locale_label_key, tr, use_i18n};
use crate::prefs::Theme;
use crate::ws::use_live;

#[component]
pub(crate) fn SettingsPage() -> impl IntoView {
    let i18n = use_i18n();
    let ui = use_ui();
    let auth = use_auth();
    let live = use_live();
    let toasts = use_toasts();
    let version = LocalResource::new(api::version);
    let doctor = LocalResource::new(api::doctor);

    let logout = move |_| {
        leptos::task::spawn_local(async move {
            if let Err(e) = auth.logout().await {
                toasts.error(e.message(i18n));
            }
        });
    };

    let section = "space-y-3";
    view! {
        <PageHeader title=tr("ui-nav-settings") />
        <div class="grid gap-6 lg:grid-cols-2">
            <section aria-labelledby="set-appearance" class=format!("{CARD} {section}")>
                <h2 id="set-appearance" class="font-semibold">{tr("ui-set-appearance")}</h2>
                <div>
                    <label for="set-lang" class="mb-1 block text-sm">{tr("ui-language")}</label>
                    <select
                        id="set-lang"
                        class="rounded-md border border-slate-300 bg-white px-2 py-1 text-sm dark:border-slate-700 dark:bg-slate-900"
                        on:change=move |ev| {
                            if let Ok(l) = event_target_value(&ev).parse() {
                                i18n.locale.set(l);
                            }
                        }
                    >
                        {wpg_i18n::Locale::ALL
                            .into_iter()
                            .map(|l| {
                                view! {
                                    <option value=l.as_str() selected=move || i18n.locale.get() == l>
                                        {move || i18n.t(locale_label_key(l))}
                                    </option>
                                }
                            })
                            .collect_view()}
                    </select>
                </div>
                <fieldset>
                    <legend class="mb-1 text-sm">{tr("ui-theme")}</legend>
                    <div class="flex gap-4">
                        {Theme::ALL
                            .into_iter()
                            .map(|t| {
                                view! {
                                    <label class="flex items-center gap-1.5 text-sm">
                                        <input
                                            type="radio"
                                            name="theme"
                                            value=t.as_str()
                                            prop:checked=move || ui.theme.get() == t
                                            on:change=move |_| ui.theme.set(t)
                                        />
                                        {move || i18n.t(t.label_key())}
                                    </label>
                                }
                            })
                            .collect_view()}
                    </div>
                </fieldset>
            </section>

            <section aria-labelledby="set-account" class=format!("{CARD} {section}")>
                <h2 id="set-account" class="font-semibold">{tr("ui-set-account")}</h2>
                {move || match auth.phase.get() {
                    AuthPhase::Authenticated { user } => view! {
                        <p class="text-sm">
                            {move || i18n.ta("ui-set-logged-in", &[("id", user.clone().unwrap_or_else(|| "admin".to_owned()))])}
                        </p>
                        <button type="button" class=BTN on:click=logout>
                            <Icon name=IconName::Logout class="h-4 w-4" />
                            {tr("ui-logout")}
                        </button>
                    }.into_any(),
                    _ => view! { <p class="text-sm text-slate-600 dark:text-slate-400">{tr("ui-set-auth-disabled")}</p> }.into_any(),
                }}
            </section>

            <section aria-labelledby="set-client" class=format!("{CARD} {section}")>
                <h2 id="set-client" class="font-semibold">{tr("ui-set-client")}</h2>
                <p class="text-xs text-slate-500">{tr("ui-set-client-note")}</p>
                <dl class="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1 text-sm">
                    <dt class="text-slate-500">{tr("ui-set-api")}</dt>
                    <dd class="font-mono text-xs break-all">{crate::dom::ws_url("/api").replacen("ws", "http", 1)}</dd>
                    <dt class="text-slate-500">{tr("ui-set-ws")}</dt>
                    <dd class="font-mono text-xs break-all">
                        {crate::dom::ws_url("/api/ws")} " · " {move || i18n.t(live.status.get().label_key())}
                    </dd>
                    <dt class="text-slate-500">{tr("ui-language")}</dt>
                    <dd>{move || i18n.locale.get().as_str()}</dd>
                    <dt class="text-slate-500">{tr("ui-theme")}</dt>
                    <dd>{move || ui.theme.get().as_str()}</dd>
                    <dt class="text-slate-500">{tr("ui-set-ui-version")}</dt>
                    <dd>{env!("CARGO_PKG_VERSION")}</dd>
                </dl>
                <a href="/api/docs/" rel="external" target="_blank" class="inline-flex items-center gap-1 text-sm text-sky-700 underline dark:text-sky-400">
                    <Icon name=IconName::External class="h-4 w-4" />
                    {tr("ui-set-api-docs")}
                </a>
            </section>

            <section aria-labelledby="set-about" class=format!("{CARD} {section}")>
                <h2 id="set-about" class="font-semibold">{tr("ui-set-about")}</h2>
                {move || match version.get() {
                    None => view! { <Loading /> }.into_any(),
                    Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                    Some(Ok(v)) => view! {
                        <dl class="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1 text-sm">
                            <dt class="text-slate-500">{tr("ui-set-server-version")}</dt><dd class="font-mono">{v.version}</dd>
                            <dt class="text-slate-500">{tr("ui-set-api-version")}</dt><dd class="font-mono">{v.api_version}</dd>
                            <dt class="text-slate-500">{tr("ui-set-platform")}</dt><dd class="font-mono">{format!("{}/{}", v.os, v.arch)}</dd>
                            <dt class="text-slate-500">{tr("ui-set-features")}</dt>
                            <dd class="font-mono">{if v.features.is_empty() { "–".to_owned() } else { v.features.join(", ") }}</dd>
                            <dt class="text-slate-500">{tr("ui-set-locales")}</dt><dd class="font-mono">{v.locales.join(", ")}</dd>
                        </dl>
                        <p class="text-xs text-slate-500">{tr("ui-set-license")}</p>
                    }.into_any(),
                }}
            </section>

            <UserProtocols />
        </div>

        <Show when=move || matches!(auth.phase.get(), AuthPhase::Authenticated { .. })>
            <ApiTokens />
        </Show>

        <section id="doctor" aria-labelledby="set-doctor" class="mt-6">
            <div class="mb-3 flex items-center justify-between">
                <h2 id="set-doctor" class="font-semibold">{tr("ui-dash-doctor")}</h2>
                <button type="button" class=BTN on:click=move |_| doctor.refetch()>
                    <Icon name=IconName::Refresh class="h-4 w-4" />
                    {tr("ui-refresh")}
                </button>
            </div>
            {move || match doctor.get() {
                None => view! { <Loading /> }.into_any(),
                Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                Some(Ok(r)) => view! {
                    <div class="overflow-x-auto rounded-lg border border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
                        <table class="min-w-full divide-y divide-slate-200 dark:divide-slate-800">
                            <caption class="sr-only">{tr("ui-dash-doctor")}</caption>
                            <thead>
                                <tr>
                                    <th scope="col" class=TH>{tr("ui-doctor-status")}</th>
                                    <th scope="col" class=TH>{tr("ui-doctor-check")}</th>
                                    <th scope="col" class=TH>{tr("ui-doctor-detail")}</th>
                                </tr>
                            </thead>
                            <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                                {r.checks.into_iter().map(|c| {
                                    let (cls, key) = match c.status.as_str() {
                                        "ok" => ("text-emerald-600 dark:text-emerald-400", "ui-doctor-status-ok"),
                                        "warn" => ("text-amber-600 dark:text-amber-400", "ui-doctor-status-warn"),
                                        "fail" => ("text-rose-600 dark:text-rose-400", "ui-doctor-status-fail"),
                                        _ => ("text-slate-500", "ui-doctor-status-skip"),
                                    };
                                    view! {
                                        <tr>
                                            <td class=format!("{TD} font-semibold whitespace-nowrap {cls}")>{move || i18n.t(key)}</td>
                                            <td class=TD>
                                                <p>{c.summary.clone()}</p>
                                                <p class="font-mono text-xs text-slate-400">{c.id.clone()}</p>
                                            </td>
                                            <td class=format!("{TD} text-xs")>
                                                <p class="font-mono break-all text-slate-600 dark:text-slate-400">{c.detail.clone()}</p>
                                                {c.fix.clone().map(|f| view! {
                                                    <p class="mt-1 rounded bg-slate-50 p-1.5 font-mono break-all dark:bg-slate-800">{f}</p>
                                                })}
                                            </td>
                                        </tr>
                                    }
                                }).collect_view()}
                            </tbody>
                        </table>
                    </div>
                }.into_any(),
            }}
        </section>
    }
}

/// Count of the server's user-defined protocols and a button that makes the
/// server read its protocol directories again.
#[component]
fn UserProtocols() -> impl IntoView {
    let i18n = use_i18n();
    let live = use_live();
    let toasts = use_toasts();
    let busy = RwSignal::new(false);
    let loaded = LocalResource::new(move || {
        live.protocols_rev.track();
        async move {
            api::protocol_extensions()
                .await
                .map(|exts| exts.iter().map(|e| e.protocols.len()).sum::<usize>())
        }
    });
    let reload = move |_| {
        busy.set(true);
        leptos::task::spawn_local(async move {
            match api::reload_protocols().await {
                Ok(count) => {
                    toasts
                        .ok(i18n.ta("ui-set-protocols-reloaded", &[("count", count.to_string())]));
                }
                Err(e) => toasts.error(e.message(i18n)),
            }
            busy.set(false);
        });
    };
    view! {
        <section aria-labelledby="set-protocols" class=format!("{CARD} space-y-3")>
            <h2 id="set-protocols" class="font-semibold">{tr("ui-set-protocols")}</h2>
            {move || match loaded.get() {
                None => view! { <Loading /> }.into_any(),
                Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                Some(Ok(n)) => view! {
                    <p class="text-sm">{move || i18n.ta("ui-set-protocols-count", &[("count", n.to_string())])}</p>
                }.into_any(),
            }}
            <button type="button" class=BTN on:click=reload disabled=move || busy.get()>
                <Icon name=IconName::Refresh class="h-4 w-4" />
                {tr("ui-set-protocols-reload")}
            </button>
        </section>
    }
}

/// Expiry choices of a new token, in days (`None`: never).
const TOKEN_EXPIRY: [Option<u32>; 4] = [Some(30), Some(90), Some(365), None];

/// API tokens of the logged-in user: create (the secret is shown once), list,
/// revoke.
#[component]
fn ApiTokens() -> impl IntoView {
    let i18n = use_i18n();
    let toasts = use_toasts();
    let rev = RwSignal::new(0u32);
    let list = LocalResource::new(move || {
        rev.track();
        api::tokens::tokens()
    });
    let name = RwSignal::new(String::new());
    let expiry = RwSignal::new(Some(90u32));
    let busy = RwSignal::new(false);
    let secret = RwSignal::new(None::<String>);
    let revoking = RwSignal::new(None::<TokenInfo>);

    let create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let n = name.get_untracked();
        if n.trim().is_empty() {
            return;
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            match api::tokens::create_token(n.trim(), expiry.get_untracked()).await {
                Ok(c) => {
                    secret.set(Some(c.secret));
                    name.set(String::new());
                    rev.update(|r| *r += 1);
                }
                Err(e) => toasts.error(e.message(i18n)),
            }
            busy.set(false);
        });
    };
    let revoke = move || {
        let Some(t) = revoking.get_untracked() else {
            return;
        };
        busy.set(true);
        leptos::task::spawn_local(async move {
            match api::tokens::revoke_token(t.id).await {
                Ok(()) => {
                    toasts.ok(i18n.t("ui-set-token-revoked"));
                    rev.update(|r| *r += 1);
                }
                Err(e) => toasts.error(e.message(i18n)),
            }
            revoking.set(None);
            busy.set(false);
        });
    };
    let when = move |secs: Option<i64>| match secs.and_then(|s| u64::try_from(s).ok()) {
        Some(s) => crate::fmt::datetime(i18n.locale.get(), s.saturating_mul(1000)),
        None => i18n.t("ui-set-token-never"),
    };

    view! {
        <section aria-labelledby="set-tokens" class=format!("{CARD} mt-6 space-y-4")>
            <div>
                <h2 id="set-tokens" class="font-semibold">{tr("ui-set-tokens")}</h2>
                <p class="text-sm text-slate-600 dark:text-slate-400">{tr("ui-set-tokens-intro")}</p>
            </div>
            <form class="flex flex-wrap items-end gap-3" on:submit=create>
                <div class="min-w-48 flex-1">
                    <label for="token-name" class=LABEL>{tr("ui-set-token-name")}</label>
                    <input
                        id="token-name"
                        type="text"
                        class=INPUT
                        maxlength="100"
                        required
                        prop:value=move || name.get()
                        on:input=move |ev| name.set(event_target_value(&ev))
                    />
                </div>
                <div>
                    <label for="token-expiry" class=LABEL>{tr("ui-set-token-expires")}</label>
                    <select
                        id="token-expiry"
                        class=INPUT
                        on:change=move |ev| expiry.set(event_target_value(&ev).parse().ok())
                    >
                        {TOKEN_EXPIRY
                            .into_iter()
                            .map(|d| {
                                let value = d.map(|d| d.to_string()).unwrap_or_default();
                                view! {
                                    <option value=value selected=move || expiry.get() == d>
                                        {move || match d {
                                            Some(n) => i18n.ta("ui-set-token-days", &[("count", n.to_string())]),
                                            None => i18n.t("ui-set-token-never"),
                                        }}
                                    </option>
                                }
                            })
                            .collect_view()}
                    </select>
                </div>
                <button type="submit" class=BTN_PRIMARY disabled=move || busy.get() || name.get().trim().is_empty()>
                    <Icon name=IconName::Plus class="h-4 w-4" />
                    {tr("ui-set-token-create")}
                </button>
            </form>

            {move || secret.get().map(|s| {
                let copy = s.clone();
                view! {
                    <div role="status" class="space-y-2 rounded-md border border-amber-300 bg-amber-50 p-3 dark:border-amber-700 dark:bg-amber-950">
                        <p class="text-sm font-medium">{tr("ui-set-token-secret")}</p>
                        <div class="flex flex-wrap items-center gap-2">
                            <code class="flex-1 rounded bg-white px-2 py-1 font-mono text-xs break-all dark:bg-slate-900">{s}</code>
                            <button
                                type="button"
                                class=BTN
                                on:click=move |_| {
                                    crate::dom::copy_text(&copy);
                                    toasts.ok(i18n.t("ui-set-token-copied"));
                                }
                            >
                                <Icon name=IconName::Copy class="h-4 w-4" />
                                {tr("ui-set-token-copy")}
                            </button>
                            <button type="button" class=BTN on:click=move |_| secret.set(None)>
                                {tr("ui-set-token-done")}
                            </button>
                        </div>
                    </div>
                }
            })}

            {move || match list.get() {
                None => view! { <Loading /> }.into_any(),
                Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                Some(Ok(tokens)) if tokens.is_empty() => view! {
                    <p class="text-sm text-slate-500">{tr("ui-set-token-none")}</p>
                }.into_any(),
                Some(Ok(tokens)) => view! {
                    <div class="overflow-x-auto">
                        <table class="min-w-full divide-y divide-slate-200 dark:divide-slate-800">
                            <caption class="sr-only">{tr("ui-set-tokens")}</caption>
                            <thead>
                                <tr>
                                    <th scope="col" class=TH>{tr("ui-set-token-name")}</th>
                                    <th scope="col" class=TH>{tr("ui-set-token-col-token")}</th>
                                    <th scope="col" class=TH>{tr("ui-set-token-col-user")}</th>
                                    <th scope="col" class=TH>{tr("ui-set-token-col-created")}</th>
                                    <th scope="col" class=TH>{tr("ui-set-token-col-used")}</th>
                                    <th scope="col" class=TH>{tr("ui-set-token-expires")}</th>
                                    <th scope="col" class=TH><span class="sr-only">{tr("ui-set-token-revoke")}</span></th>
                                </tr>
                            </thead>
                            <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                                {tokens.into_iter().map(|t| {
                                    let (created, used, expires) = (t.created, t.last_used, t.expires);
                                    let label = t.name.clone();
                                    let row = t.clone();
                                    view! {
                                        <tr>
                                            <td class=TD>{t.name}</td>
                                            <td class=format!("{TD} font-mono text-xs")>{format!("{}…", t.prefix)}</td>
                                            <td class=TD>{t.user}</td>
                                            <td class=format!("{TD} whitespace-nowrap")>{move || when(Some(created))}</td>
                                            <td class=format!("{TD} whitespace-nowrap")>{move || when(used)}</td>
                                            <td class=format!("{TD} whitespace-nowrap")>{move || when(expires)}</td>
                                            <td class=format!("{TD} text-right")>
                                                <button
                                                    type="button"
                                                    class=BTN_GHOST
                                                    aria-label=move || i18n.ta("ui-set-token-revoke-named", &[("name", label.clone())])
                                                    on:click=move |_| revoking.set(Some(row.clone()))
                                                >
                                                    <Icon name=IconName::Trash class="h-4 w-4" />
                                                </button>
                                            </td>
                                        </tr>
                                    }
                                }).collect_view()}
                            </tbody>
                        </table>
                    </div>
                }.into_any(),
            }}

            <Show when=move || revoking.get().is_some()>
                <Confirm
                    title=Signal::derive(move || i18n.ta("ui-set-token-revoke-title",&[("name", revoking.get().map(|t| t.name).unwrap_or_default())]))
                    confirm_label=tr("ui-set-token-revoke")
                    danger=true
                    busy=busy
                    on_confirm=move || revoke()
                    on_cancel=move || revoking.set(None)
                >
                    <p>{tr("ui-set-token-revoke-body")}</p>
                </Confirm>
            </Show>
        </section>
    }
}
