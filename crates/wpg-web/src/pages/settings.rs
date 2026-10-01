//! `/settings`: language, theme, account, client settings, about, doctor report.

use leptos::prelude::*;

use crate::api;
use crate::app::use_ui;
use crate::auth::{use_auth, AuthPhase};
use crate::components::{
    use_toasts, ErrorBox, Icon, IconName, Loading, PageHeader, BTN, CARD, TD, TH,
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
        </div>

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
