//! `/terminal`: the web terminal (ADR-0011). SSH / Telnet sessions in tabs that
//! stay open while browsing other pages, saved connection profiles (no
//! secrets), host key confirmation, keyboard-interactive prompts, session logs,
//! multi-line paste / file sending with confirmation, known hosts.

use leptos::html;
use leptos::prelude::*;

use crate::api::terminal::{self as tapi, KnownHost, LogInfo, TerminalStatus};
use crate::app::use_ui;
use crate::components::{
    use_toasts, Confirm, Empty, ErrorBox, Icon, IconName, Loading, Modal, PageHeader, BTN,
    BTN_DANGER, BTN_GHOST, BTN_PRIMARY, CARD, INPUT, LABEL, TD, TH,
};
use crate::i18n::{tr, use_i18n, I18n};
use crate::term::{
    closed_key, error_key, session, split_lines, state_key, upsert_profile, ConnectParams, Phase,
    Profile, Proto, TabState, TermSettings, TermStore,
};

/// Tera Term-like default: the terminal fills the rest of the window.
const TERM_AREA: &str = "relative h-[calc(100vh-17rem)] min-h-[18rem] overflow-hidden rounded-lg border border-slate-300 bg-white p-1 dark:border-slate-700 dark:bg-slate-950";

#[component]
pub(crate) fn TerminalPage() -> impl IntoView {
    let status = LocalResource::new(tapi::status);
    view! {
        <PageHeader title=tr("ui-nav-terminal") />
        {move || match status.get() {
            None => view! { <Loading /> }.into_any(),
            Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
            Some(Ok(s)) if !s.enabled => view! { <Disabled /> }.into_any(),
            Some(Ok(s)) => view! { <Workspace status=s /> }.into_any(),
        }}
    }
}

/// `terminal.enabled = false`: how to switch it on.
#[component]
fn Disabled() -> impl IntoView {
    view! {
        <section class=format!("{CARD} max-w-3xl space-y-3") aria-labelledby="term-disabled">
            <div class="flex items-center gap-2">
                <Icon name=IconName::Terminal class="h-6 w-6 text-slate-400" />
                <h2 id="term-disabled" class="font-semibold">{tr("ui-term-disabled-title")}</h2>
            </div>
            <p class="text-sm">{tr("ui-term-disabled-text")}</p>
            <pre class="overflow-x-auto rounded-md bg-slate-100 p-3 font-mono text-xs dark:bg-slate-800">
                "[terminal]\nenabled = true\nallowed_targets = [\"10.0.0.0/8\", \"*.lab.example\"]\nrecord_sessions = true\nidle_timeout_secs = 1800\nmax_sessions = 8"
            </pre>
            <p class="text-sm">{tr("ui-term-disabled-restart")}</p>
            <p class="rounded-md bg-amber-50 px-3 py-2 text-xs text-amber-800 dark:bg-amber-950/40 dark:text-amber-200">
                {tr("ui-term-disabled-security")}
            </p>
        </section>
    }
}

/// The tab of `id` (tracked).
fn tab_of(store: TermStore, id: u32) -> Option<TabState> {
    store.tabs.with(|v| v.iter().find(|t| t.id == id).cloned())
}

/// The active tab (tracked).
fn active_tab(store: TermStore) -> Option<TabState> {
    let id = store.active.get()?;
    tab_of(store, id)
}

#[component]
fn Workspace(status: TerminalStatus) -> impl IntoView {
    let store = crate::term::use_term();
    let ui = use_ui();
    let status = StoredValue::new(status);

    // Settings persist and apply to every tab; the colours follow the theme.
    Effect::new(move |_| {
        let s = store.settings.get();
        crate::term::save_settings(&s);
        session::apply_settings(&s);
    });
    Effect::new(move |_| session::apply_theme(crate::prefs::is_dark(ui.theme.get())));
    // A tab that waits for the user comes to the front.
    Effect::new(move |_| {
        let waiting = store.tabs.with(|v| {
            v.iter()
                .find(|t| matches!(t.phase, Phase::HostKey(_) | Phase::Auth(_)))
                .map(|t| t.id)
        });
        if let Some(id) = waiting {
            if store.active.get_untracked() != Some(id) {
                store.active.set(Some(id));
            }
            store.show_form.set(false);
        }
    });

    view! {
        <TabBar />
        <Show when=move || store.show_form.get()>
            <ConnectForm status=status.get_value() />
        </Show>
        <Show when=move || !store.show_form.get() && store.active.get().is_some()>
            <Toolbar record_allowed=status.with_value(|s| s.record_sessions) />
        </Show>
        <div class=move || if store.show_form.get() || store.tabs.with(Vec::is_empty) { "hidden" } else { TERM_AREA }>
            <For each=move || store.tabs.get() key=|t| t.id let:t>
                <TermHost id=t.id />
            </For>
        </div>
        <Show when=move || !store.show_form.get()>
            <StatusLine />
        </Show>
        <HostKeyDialog />
        <AuthDialog />
        <PasteDialog />
        <div class="mt-6 grid gap-4 xl:grid-cols-2">
            <SettingsPanel />
            <div class="space-y-4">
                <LogsPanel />
                <KnownHostsPanel />
            </div>
        </div>
    }
}

/// The element xterm.js renders into (kept per tab; inactive ones are hidden so
/// switching tabs keeps their state).
#[component]
fn TermHost(id: u32) -> impl IntoView {
    let store = crate::term::use_term();
    let visible = move || store.active.get() == Some(id) && !store.show_form.get();
    request_animation_frame(move || session::mount(id));
    Effect::new(move |_| {
        if visible() {
            request_animation_frame(move || {
                session::fit(id);
                session::focus(id);
            });
        }
    });
    view! {
        <div
            id=format!("term-host-{id}")
            role="tabpanel"
            aria-labelledby=format!("term-tab-{id}")
            class=move || if visible() { "h-full w-full" } else { "hidden" }
        ></div>
    }
}

fn dot_class(phase: &Phase) -> &'static str {
    match phase {
        Phase::Connected => "bg-emerald-500",
        Phase::Closed { .. } => "bg-slate-400",
        Phase::HostKey(_) | Phase::Auth(_) => "bg-amber-500",
        Phase::Idle | Phase::Progress(_) => "bg-amber-400 animate-pulse",
    }
}

#[component]
fn TabBar() -> impl IntoView {
    let store = crate::term::use_term();
    let i18n = use_i18n();
    view! {
        <div class="mb-2 flex flex-wrap items-center gap-1" role="tablist" aria-label=tr("ui-term-tabs")>
            <For each=move || store.tabs.get() key=|t| t.id let:t>
                {
                    let id = t.id;
                    let selected = move || store.active.get() == Some(id) && !store.show_form.get();
                    let tab = move || tab_of(store, id);
                    view! {
                        <div class=move || {
                            if selected() {
                                "flex items-center rounded-md border border-sky-500 bg-sky-50 dark:bg-sky-500/10"
                            } else {
                                "flex items-center rounded-md border border-slate-300 bg-white dark:border-slate-700 dark:bg-slate-900"
                            }
                        }>
                            <button
                                type="button"
                                role="tab"
                                id=format!("term-tab-{id}")
                                aria-selected=move || selected().to_string()
                                aria-controls=format!("term-host-{id}")
                                class="flex items-center gap-2 px-2.5 py-1 text-sm"
                                on:click=move |_| {
                                    store.active.set(Some(id));
                                    store.show_form.set(false);
                                }
                            >
                                <span class=move || {
                                    format!("h-2 w-2 rounded-full {}", tab().map_or("bg-slate-400", |t| dot_class(&t.phase)))
                                }></span>
                                <span class="max-w-48 truncate">{move || tab().map(|t| t.title).unwrap_or_default()}</span>
                                <Show when=move || tab().is_some_and(|t| t.recording)>
                                    <span class="rounded bg-rose-600 px-1 text-[10px] font-bold text-white" title=tr("ui-term-recording")>
                                        "REC"
                                    </span>
                                </Show>
                            </button>
                            <button
                                type="button"
                                class=BTN_GHOST
                                aria-label=move || i18n.ta("ui-term-close-tab", &[("target", tab().map(|t| t.title).unwrap_or_default())])
                                title=tr("ui-term-close")
                                on:click=move |_| session::close(store, id)
                            >
                                <Icon name=IconName::Close class="h-3.5 w-3.5" />
                            </button>
                        </div>
                    }
                }
            </For>
            <button
                type="button"
                class=move || if store.show_form.get() { BTN_PRIMARY } else { BTN }
                aria-pressed=move || store.show_form.get().to_string()
                on:click=move |_| store.show_form.set(true)
            >
                <Icon name=IconName::Plus class="h-4 w-4" />
                {tr("ui-term-new")}
            </button>
        </div>
    }
}

/// The connection form with saved profiles.
#[component]
fn ConnectForm(status: TerminalStatus) -> impl IntoView {
    let store = crate::term::use_term();
    let i18n = use_i18n();
    let toasts = use_toasts();
    let profile = RwSignal::new(Profile::blank());
    let password = RwSignal::new(String::new());
    let private_key = RwSignal::new(String::new());
    let passphrase = RwSignal::new(String::new());
    let tried = RwSignal::new(false);
    let record_allowed = status.record_sessions;
    let allowed = status.allowed_targets.clone();

    let params = move || ConnectParams {
        profile: profile.get(),
        password: password.get(),
        private_key: private_key.get(),
        passphrase: passphrase.get(),
    };
    let problems = move || params().problems();
    let connect = move || {
        tried.set(true);
        let mut p = params();
        if !p.problems().is_empty() {
            return;
        }
        if !record_allowed {
            p.profile.record = false;
        }
        session::open(store, i18n, p);
        // The tab keeps its own copy for reconnecting; the form forgets them.
        password.set(String::new());
        passphrase.set(String::new());
        tried.set(false);
    };
    let save_profile = move |_| {
        let p = profile.get_untracked();
        if p.name.trim().is_empty() {
            toasts.error(i18n.t("ui-term-profile-name-needed"));
            return;
        }
        let name = p.name.trim().to_owned();
        store.profiles.update(|l| {
            upsert_profile(
                l,
                Profile {
                    name: name.clone(),
                    ..p
                },
            );
        });
        store
            .profiles
            .with_untracked(|l| crate::term::save_profiles(l));
        toasts.ok(i18n.ta("ui-term-profile-saved", &[("target", name)]));
    };
    let delete_profile = move |_| {
        let name = profile.get_untracked().name;
        store.profiles.update(|l| l.retain(|p| p.name != name));
        store
            .profiles
            .with_untracked(|l| crate::term::save_profiles(l));
        profile.set(Profile::blank());
    };
    let key_file = NodeRef::<html::Input>::new();
    let on_key_file = move |_| {
        #[cfg(target_arch = "wasm32")]
        if let Some(input) = key_file.get() {
            leptos::task::spawn_local(async move {
                if let Some((_, text)) = crate::dom::read_first_file(&input).await {
                    private_key.set(text);
                }
                input.set_value("");
            });
        }
    };
    let set = move |f: fn(&mut Profile, String)| {
        move |ev: leptos::ev::Event| {
            let v = event_target_value(&ev);
            profile.update(|p| f(p, v));
        }
    };
    let ssh = move || profile.with(|p| p.protocol == Proto::Ssh);
    let use_key = move || profile.with(|p| p.use_key);
    let invalid = move |key: &'static str| tried.get() && problems().contains(&key);

    view! {
        <form
            class=format!("{CARD} max-w-3xl space-y-4")
            aria-labelledby="term-form-title"
            on:submit=move |ev| {
                ev.prevent_default();
                connect();
            }
        >
            <div class="flex flex-wrap items-end justify-between gap-3">
                <h2 id="term-form-title" class="font-semibold">{tr("ui-term-form-title")}</h2>
                <div class="flex flex-wrap items-end gap-2">
                    <div>
                        <label for="term-profile" class=LABEL>{tr("ui-term-profile")}</label>
                        <select
                            id="term-profile"
                            class=INPUT
                            on:change=move |ev| {
                                let name = event_target_value(&ev);
                                if let Some(p) = store.profiles.with(|l| l.iter().find(|p| p.name == name).cloned()) {
                                    profile.set(p);
                                    private_key.set(String::new());
                                }
                            }
                        >
                            <option value="" selected=move || profile.with(|p| p.name.is_empty())>{tr("ui-term-profile-none")}</option>
                            {move || store.profiles.get().into_iter().map(|p| {
                                let name = p.name.clone();
                                let sel = name.clone();
                                let label = name.clone();
                                view! { <option value=name selected=move || profile.with(|x| x.name == sel)>{label}</option> }
                            }).collect_view()}
                        </select>
                    </div>
                    <Show when=move || { let n = profile.with(|p| p.name.clone()); store.profiles.with(|l| l.iter().any(|p| p.name == n)) }>
                        <button type="button" class=BTN on:click=delete_profile>
                            <Icon name=IconName::Trash class="h-4 w-4" />
                            {tr("ui-term-profile-delete")}
                        </button>
                    </Show>
                </div>
            </div>

            <div class="grid gap-3 sm:grid-cols-6">
                <div class="sm:col-span-2">
                    <label for="term-proto" class=LABEL>{tr("ui-term-protocol")}</label>
                    <select
                        id="term-proto"
                        class=INPUT
                        on:change=move |ev| {
                            let p = Proto::parse(&event_target_value(&ev));
                            profile.update(|x| x.protocol = p);
                        }
                    >
                        {[Proto::Ssh, Proto::Telnet].into_iter().map(|p| view! {
                            <option value=p.as_str() selected=move || profile.with(|x| x.protocol == p)>{move || i18n.t(p.label_key())}</option>
                        }).collect_view()}
                    </select>
                </div>
                <div class="sm:col-span-3">
                    <label for="term-host" class=LABEL>{tr("ui-term-host")}</label>
                    <input
                        id="term-host"
                        class=INPUT
                        required
                        autocomplete="off"
                        spellcheck="false"
                        placeholder="192.0.2.1"
                        aria-invalid=move || invalid("ui-term-err-host").to_string()
                        prop:value=move || profile.with(|p| p.host.clone())
                        on:input=set(|p, v| p.host = v)
                    />
                </div>
                <div class="sm:col-span-1">
                    <label for="term-port" class=LABEL>{tr("ui-term-port")}</label>
                    <input
                        id="term-port"
                        class=INPUT
                        type="number"
                        min="1"
                        max="65535"
                        placeholder=move || profile.with(|p| p.protocol.default_port().to_string())
                        prop:value=move || profile.with(|p| p.port.map(|n| n.to_string()).unwrap_or_default())
                        on:input=set(|p, v| p.port = v.trim().parse().ok())
                    />
                </div>
                <Show when=ssh>
                    <div class="sm:col-span-3">
                        <label for="term-user" class=LABEL>{tr("ui-term-username")}</label>
                        <input
                            id="term-user"
                            class=INPUT
                            autocomplete="username"
                            spellcheck="false"
                            aria-invalid=move || invalid("ui-term-err-username").to_string()
                            prop:value=move || profile.with(|p| p.username.clone())
                            on:input=set(|p, v| p.username = v)
                        />
                    </div>
                    <fieldset class="sm:col-span-3">
                        <legend class=LABEL>{tr("ui-term-auth")}</legend>
                        <div class="flex gap-4 py-1.5 text-sm">
                            <label class="flex items-center gap-1.5">
                                <input type="radio" name="term-auth" prop:checked=move || !use_key() on:change=move |_| profile.update(|p| p.use_key = false) />
                                {tr("ui-term-auth-password")}
                            </label>
                            <label class="flex items-center gap-1.5">
                                <input type="radio" name="term-auth" prop:checked=use_key on:change=move |_| profile.update(|p| p.use_key = true) />
                                {tr("ui-term-auth-key")}
                            </label>
                        </div>
                    </fieldset>
                    <Show
                        when=use_key
                        fallback=move || view! {
                            <div class="sm:col-span-3">
                                <label for="term-pass" class=LABEL>{tr("ui-term-password")}</label>
                                <input
                                    id="term-pass"
                                    class=INPUT
                                    type="password"
                                    autocomplete="off"
                                    prop:value=move || password.get()
                                    on:input=move |ev| password.set(event_target_value(&ev))
                                />
                                <p class="mt-1 text-xs text-slate-500">{tr("ui-term-password-hint")}</p>
                            </div>
                        }
                    >
                        <div class="sm:col-span-6">
                            <div class="flex items-end justify-between gap-2">
                                <label for="term-key" class=LABEL>{tr("ui-term-key")}</label>
                                <button type="button" class=format!("{BTN} mb-1") on:click=move |_| { if let Some(i) = key_file.get() { i.click(); } }>
                                    <Icon name=IconName::Upload class="h-4 w-4" />
                                    {tr("ui-term-key-file")}
                                </button>
                                <input node_ref=key_file type="file" class="hidden" on:change=on_key_file />
                            </div>
                            <textarea
                                id="term-key"
                                class=format!("{INPUT} font-mono text-xs")
                                rows="4"
                                spellcheck="false"
                                placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"
                                aria-invalid=move || invalid("ui-term-err-key").to_string()
                                prop:value=move || private_key.get()
                                on:input=move |ev| private_key.set(event_target_value(&ev))
                            ></textarea>
                        </div>
                        <div class="sm:col-span-3">
                            <label for="term-passphrase" class=LABEL>{tr("ui-term-passphrase")}</label>
                            <input
                                id="term-passphrase"
                                class=INPUT
                                type="password"
                                autocomplete="off"
                                prop:value=move || passphrase.get()
                                on:input=move |ev| passphrase.set(event_target_value(&ev))
                            />
                        </div>
                    </Show>
                </Show>
            </div>

            <fieldset class="flex flex-wrap gap-x-6 gap-y-2 text-sm">
                <legend class="sr-only">{tr("ui-term-log")}</legend>
                <label class="flex items-center gap-1.5">
                    <input
                        type="checkbox"
                        disabled=!record_allowed
                        prop:checked=move || record_allowed && profile.with(|p| p.record)
                        on:change=move |ev| { let c = event_target_checked(&ev); profile.update(|p| p.record = c) }
                    />
                    {tr("ui-term-record")}
                </label>
                <label class="flex items-center gap-1.5">
                    <input
                        type="checkbox"
                        disabled=!record_allowed
                        prop:checked=move || profile.with(|p| p.strip_ansi)
                        on:change=move |ev| { let c = event_target_checked(&ev); profile.update(|p| p.strip_ansi = c) }
                    />
                    {tr("ui-term-strip-ansi")}
                </label>
                {(!record_allowed).then(|| view! { <span class="text-xs text-slate-500">{tr("ui-term-record-off")}</span> })}
            </fieldset>

            <div>
                <label for="term-name" class=LABEL>{tr("ui-term-profile-name")}</label>
                <div class="flex flex-wrap gap-2">
                    <input
                        id="term-name"
                        class=format!("{INPUT} max-w-xs")
                        autocomplete="off"
                        prop:value=move || profile.with(|p| p.name.clone())
                        on:input=set(|p, v| p.name = v)
                    />
                    <button type="button" class=BTN on:click=save_profile>
                        <Icon name=IconName::Star class="h-4 w-4" />
                        {tr("ui-term-profile-save")}
                    </button>
                </div>
                <p class="mt-1 text-xs text-slate-500">{tr("ui-term-profile-hint")}</p>
            </div>

            <Show when=move || tried.get() && !problems().is_empty()>
                <ul role="alert" class="list-disc space-y-0.5 pl-5 text-sm text-rose-700 dark:text-rose-300">
                    {move || problems().into_iter().map(|k| view! { <li>{move || i18n.t(k)}</li> }).collect_view()}
                </ul>
            </Show>
            {(!allowed.is_empty()).then(|| view! {
                <p class="text-xs text-slate-500">
                    {tr("ui-term-allowed")} " "
                    <span class="font-mono">{allowed.join(", ")}</span>
                </p>
            })}

            <div class="flex flex-wrap justify-end gap-2">
                <Show when=move || !store.tabs.with(Vec::is_empty)>
                    <button type="button" class=BTN on:click=move |_| store.show_form.set(false)>{tr("ui-cancel")}</button>
                </Show>
                <button type="submit" class=BTN_PRIMARY>
                    <Icon name=IconName::Play class="h-4 w-4" />
                    {tr("ui-term-connect")}
                </button>
            </div>
        </form>
    }
}

/// Actions of the active tab.
#[component]
fn Toolbar(record_allowed: bool) -> impl IntoView {
    let store = crate::term::use_term();
    let i18n = use_i18n();
    let find = RwSignal::new(String::new());
    let tab = move || active_tab(store);
    let id = move || store.active.get().unwrap_or(0);
    let live = move || tab().is_some_and(|t| t.phase.is_live());
    let connected = move || tab().is_some_and(|t| t.phase == Phase::Connected);
    let recording = move || tab().is_some_and(|t| t.recording);
    let send_file = NodeRef::<html::Input>::new();
    let on_send_file = move |_| {
        #[cfg(target_arch = "wasm32")]
        if let Some(input) = send_file.get() {
            let tab_id = store.active.get_untracked().unwrap_or(0);
            leptos::task::spawn_local(async move {
                if let Some((name, text)) = crate::dom::read_first_file(&input).await {
                    session::request_paste(store, tab_id, text, Some(name));
                }
                input.set_value("");
            });
        }
    };
    let save_scrollback = move |_| {
        let Some(t) = active_tab(store) else {
            return;
        };
        let text = session::scrollback_text(t.id);
        let name = format!(
            "{}.txt",
            t.title.replace(
                |c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '-',
                "_"
            )
        );
        crate::dom::download_text(&name, "text/plain;charset=utf-8", &text);
    };
    let font = move |d: i16| {
        store.settings.update(|s| {
            let v = i16::from(s.font_size) + d;
            s.font_size = u8::try_from(v).unwrap_or(s.font_size);
            *s = s.clone().clamped();
        });
    };
    let toasts = use_toasts();
    let do_find = move |forward: bool| {
        let text = find.get_untracked();
        if !session::find(id(), &text, forward) && !text.is_empty() {
            toasts.error(i18n.t("ui-term-find-none"));
        }
    };

    view! {
        <div role="toolbar" aria-label=tr("ui-term-toolbar") class="mb-2 flex flex-wrap items-center gap-1.5">
            <Show
                when=live
                fallback=move || view! {
                    <button type="button" class=BTN on:click=move |_| session::reconnect(id())>
                        <Icon name=IconName::Refresh class="h-4 w-4" />
                        {tr("ui-term-reconnect")}
                    </button>
                }
            >
                <button type="button" class=BTN on:click=move |_| session::disconnect(id())>
                    <Icon name=IconName::Stop class="h-4 w-4" />
                    {tr("ui-term-disconnect")}
                </button>
            </Show>
            <button
                type="button"
                class=move || if recording() { BTN_DANGER } else { BTN }
                disabled=move || !record_allowed || !connected()
                title=move || i18n.t(if record_allowed { "ui-term-log-hint" } else { "ui-term-record-off" })
                aria-pressed=move || recording().to_string()
                on:click=move |_| {
                    let on = !recording();
                    session::send_json(id(), &serde_json::json!({ "type": "record", "enabled": on }));
                }
            >
                <Icon name=IconName::Logs class="h-4 w-4" />
                {move || i18n.t(if recording() { "ui-term-log-stop" } else { "ui-term-log-start" })}
            </button>
            {move || tab().and_then(|t| t.log_file).map(|f| view! {
                <a class=BTN href=tapi::log_url(&f) download=f.clone() title=f.clone()>
                    <Icon name=IconName::Download class="h-4 w-4" />
                    {tr("ui-term-log-download")}
                </a>
            })}
            <button type="button" class=BTN on:click=save_scrollback title=tr("ui-term-save-scrollback-hint")>
                <Icon name=IconName::Download class="h-4 w-4" />
                {tr("ui-term-save-scrollback")}
            </button>
            <button
                type="button"
                class=BTN
                disabled=move || !connected()
                on:click=move |_| { if let Some(i) = send_file.get() { i.click(); } }
            >
                <Icon name=IconName::Upload class="h-4 w-4" />
                {tr("ui-term-send-file")}
            </button>
            <input node_ref=send_file type="file" class="hidden" on:change=on_send_file />
            <button type="button" class=BTN disabled=move || !connected() on:click=move |_| session::paste_clipboard(store, id())>
                <Icon name=IconName::Copy class="h-4 w-4" />
                {tr("ui-term-paste")}
            </button>
            <button type="button" class=BTN_GHOST title=tr("ui-term-clear") aria-label=tr("ui-term-clear") on:click=move |_| session::clear(id())>
                <Icon name=IconName::Trash class="h-4 w-4" />
            </button>
            <span class="mx-1 h-6 w-px bg-slate-300 dark:bg-slate-700" aria-hidden="true"></span>
            <button type="button" class=BTN_GHOST aria-label=tr("ui-term-font-smaller") title=tr("ui-term-font-smaller") on:click=move |_| font(-1)>
                <span class="text-xs font-semibold">"A-"</span>
            </button>
            <span class="w-8 text-center text-xs tabular-nums">{move || store.settings.with(|s| s.font_size)}</span>
            <button type="button" class=BTN_GHOST aria-label=tr("ui-term-font-larger") title=tr("ui-term-font-larger") on:click=move |_| font(1)>
                <span class="text-sm font-semibold">"A+"</span>
            </button>
            <form
                class="ml-auto flex items-center gap-1"
                role="search"
                on:submit=move |ev| {
                    ev.prevent_default();
                    do_find(true);
                }
            >
                <label for="term-find" class="sr-only">{tr("ui-term-find")}</label>
                <input
                    id="term-find"
                    type="search"
                    class=format!("{INPUT} w-40")
                    placeholder=tr("ui-term-find")
                    prop:value=move || find.get()
                    on:input=move |ev| find.set(event_target_value(&ev))
                    on:keydown=move |ev| {
                        if ev.key() == "Enter" && ev.shift_key() {
                            ev.prevent_default();
                            do_find(false);
                        }
                    }
                />
                <button type="button" class=BTN_GHOST aria-label=tr("ui-term-find-prev") title=tr("ui-term-find-prev") on:click=move |_| do_find(false)>
                    <span aria-hidden="true">"↑"</span>
                </button>
                <button type="submit" class=BTN_GHOST aria-label=tr("ui-term-find-next") title=tr("ui-term-find-next")>
                    <span aria-hidden="true">"↓"</span>
                </button>
            </form>
        </div>
    }
}

/// Error text of a tab (with the stored / presented keys for a changed host key).
fn error_view(i18n: I18n, e: &crate::term::TermError) -> impl IntoView {
    let text = i18n.ta(
        if e.code == "xterm_load" {
            "ui-term-error-xterm"
        } else {
            error_key(&e.code)
        },
        &[("detail", e.detail.clone().unwrap_or_default())],
    );
    let changed = (e.code == "host_key_changed").then(|| {
        let s = |k: &str| {
            e.info
                .as_ref()
                .and_then(|i| i.get(k))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_owned()
        };
        view! {
            <dl class="mt-2 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-xs">
                <dt>{i18n.t("ui-term-hostkey-presented")}</dt>
                <dd class="font-mono break-all">{format!("{} {}", s("algorithm"), s("fingerprint"))}</dd>
                <dt>{i18n.t("ui-term-hostkey-known")}</dt>
                <dd class="font-mono break-all">{format!("{} {}", s("known_algorithm"), s("known_fingerprint"))}</dd>
            </dl>
            <p class="mt-2 text-xs">{i18n.t("ui-term-hostkey-changed-help")}</p>
        }
    });
    view! {
        <div role="alert" class="mt-2 rounded-md border border-rose-300 bg-rose-50 px-3 py-2 text-sm text-rose-800 dark:border-rose-800 dark:bg-rose-950/50 dark:text-rose-200">
            <p>{text}</p>
            {changed}
        </div>
    }
}

/// Connection state of the active tab (`aria-live`).
#[component]
fn StatusLine() -> impl IntoView {
    let store = crate::term::use_term();
    let i18n = use_i18n();
    let line = move || {
        let t = active_tab(store)?;
        let target = t.title.clone();
        Some(match &t.phase {
            Phase::Idle | Phase::Progress(_) | Phase::HostKey(_) | Phase::Auth(_) => {
                let state = match &t.phase {
                    Phase::Progress(s) => state_key(s),
                    Phase::HostKey(_) => "ui-term-state-hostkey",
                    Phase::Auth(_) => "ui-term-state-auth",
                    _ => "ui-term-state-opening",
                };
                i18n.ta(state, &[("target", target)])
            }
            Phase::Connected => {
                let mut s = i18n.ta(
                    "ui-term-status-connected",
                    &[
                        ("target", target),
                        ("detail", t.address.clone().unwrap_or_default()),
                    ],
                );
                if let Some(fp) = &t.fingerprint {
                    s.push_str(" · ");
                    s.push_str(&i18n.ta("ui-term-status-hostkey", &[("fingerprint", fp.clone())]));
                }
                if t.recording {
                    s.push_str(" · ");
                    s.push_str(&i18n.ta(
                        "ui-term-status-recording",
                        &[("file", t.log_file.clone().unwrap_or_default())],
                    ));
                }
                s
            }
            Phase::Closed {
                reason,
                bytes_in,
                bytes_out,
                duration_secs,
            } => i18n.ta(
                "ui-term-status-closed",
                &[
                    ("reason", i18n.t(closed_key(reason))),
                    (
                        "bytes",
                        format!(
                            "{} / {}",
                            crate::fmt::bytes(*bytes_in),
                            crate::fmt::bytes(*bytes_out)
                        ),
                    ),
                    ("duration", crate::fmt::duration(*duration_secs as f64)),
                ],
            ),
        })
    };
    view! {
        <p role="status" aria-live="polite" class="mt-2 text-xs text-slate-600 dark:text-slate-400">
            {line}
        </p>
        {move || active_tab(store).and_then(|t| t.error).map(|e| error_view(i18n, &e))}
    }
}

/// Unknown SSH host key: show the fingerprint, trust once / save / cancel.
#[component]
fn HostKeyDialog() -> impl IntoView {
    let store = crate::term::use_term();
    let i18n = use_i18n();
    let prompt = move || {
        active_tab(store).and_then(|t| match t.phase {
            Phase::HostKey(h) => Some((t.id, h)),
            _ => None,
        })
    };
    let prompt = Memo::new(move |_| prompt());
    let answer = move |accept: bool, save: bool| {
        if let Some((id, _)) = prompt.get_untracked() {
            session::send_json(
                id,
                &serde_json::json!({ "type": "host_key", "accept": accept, "save": save }),
            );
            store.update(id, |t| t.phase = Phase::Progress("connecting".into()));
        }
    };
    view! {
        <Show when=move || prompt.with(Option::is_some)>
            <Modal title=tr("ui-term-hostkey-title") on_close=move || answer(false, false) focus="term-hk-cancel".to_owned()>
                {move || prompt.get().map(|(_, h)| view! {
                    <div class="space-y-3 text-sm">
                        <p>{i18n.ta("ui-term-hostkey-intro", &[("target", format!("{}:{}", h.host, h.port))])}</p>
                        <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 rounded-md bg-slate-50 p-3 dark:bg-slate-800">
                            <dt class="text-slate-500">{tr("ui-term-hostkey-algorithm")}</dt>
                            <dd class="font-mono">{h.algorithm.clone()}</dd>
                            <dt class="text-slate-500">{tr("ui-term-hostkey-fingerprint")}</dt>
                            <dd class="font-mono break-all">{h.fingerprint.clone()}</dd>
                        </dl>
                        <p class="text-xs text-slate-500">{tr("ui-term-hostkey-hint")}</p>
                    </div>
                })}
                <div class="mt-5 flex flex-wrap justify-end gap-2">
                    <button id="term-hk-cancel" type="button" class=BTN on:click=move |_| answer(false, false)>
                        {tr("ui-cancel")}
                    </button>
                    <button type="button" class=BTN on:click=move |_| answer(true, false)>
                        {tr("ui-term-hostkey-once")}
                    </button>
                    <button type="button" class=BTN_PRIMARY on:click=move |_| answer(true, true)>
                        {tr("ui-term-hostkey-save")}
                    </button>
                </div>
            </Modal>
        </Show>
    }
}

/// Keyboard-interactive / password questions.
#[component]
fn AuthDialog() -> impl IntoView {
    let store = crate::term::use_term();
    let answers = RwSignal::new(Vec::<String>::new());
    let prompt = move || {
        active_tab(store).and_then(|t| match t.phase {
            Phase::Auth(a) => Some((t.id, a)),
            _ => None,
        })
    };
    // Only a new question resets the answers (other tabs update the store too).
    let prompt = Memo::new(move |_| prompt());
    Effect::new(move |_| {
        if let Some((_, a)) = prompt.get() {
            answers.set(vec![String::new(); a.prompts.len()]);
        }
    });
    let submit = move || {
        if let Some((id, _)) = prompt.get_untracked() {
            session::send_json(
                id,
                &serde_json::json!({ "type": "auth_response", "responses": answers.get_untracked() }),
            );
            answers.set(Vec::new());
            store.update(id, |t| t.phase = Phase::Progress("authenticating".into()));
        }
    };
    let cancel = move || {
        if let Some((id, _)) = prompt.get_untracked() {
            session::disconnect(id);
            store.update(id, |t| t.phase = Phase::Progress("authenticating".into()));
        }
    };
    view! {
        <Show when=move || prompt.with(Option::is_some)>
            <Modal title=tr("ui-term-auth-title") on_close=move || cancel() focus="term-auth-0".to_owned()>
                <form
                    class="space-y-3 text-sm"
                    on:submit=move |ev| {
                        ev.prevent_default();
                        submit();
                    }
                >
                    {move || prompt.get().map(|(_, a)| {
                        let header = [a.name.clone(), a.instructions.clone()]
                            .into_iter()
                            .filter(|s| !s.trim().is_empty())
                            .collect::<Vec<_>>();
                        view! {
                            {header.into_iter().map(|s| view! { <p class="whitespace-pre-wrap">{s}</p> }).collect_view()}
                            {a.prompts.into_iter().enumerate().map(|(i, p)| view! {
                                <div>
                                    <label for=format!("term-auth-{i}") class=LABEL>{p.prompt.clone()}</label>
                                    <input
                                        id=format!("term-auth-{i}")
                                        class=INPUT
                                        type=if p.echo { "text" } else { "password" }
                                        autocomplete="off"
                                        on:input=move |ev| {
                                            let v = event_target_value(&ev);
                                            answers.update(|a| { if let Some(x) = a.get_mut(i) { *x = v; } });
                                        }
                                    />
                                </div>
                            }).collect_view()}
                        }
                    })}
                    <div class="mt-5 flex justify-end gap-2">
                        <button type="button" class=BTN on:click=move |_| cancel()>{tr("ui-term-disconnect")}</button>
                        <button type="submit" class=BTN_PRIMARY>{tr("ui-term-auth-submit")}</button>
                    </div>
                </form>
            </Modal>
        </Show>
    }
}

/// Confirmation before pasting several lines / sending a file.
#[component]
fn PasteDialog() -> impl IntoView {
    let store = crate::term::use_term();
    let i18n = use_i18n();
    let pending = move || store.paste.get();
    let confirm = move || {
        if let Some(p) = store.paste.get_untracked() {
            store.paste.set(None);
            session::send_text(store, p.tab, &p.text);
            session::focus(p.tab);
        }
    };
    let cancel = move || {
        if let Some(p) = store.paste.get_untracked() {
            store.paste.set(None);
            session::focus(p.tab);
        }
    };
    view! {
        <Show when=move || pending().is_some()>
            <Confirm
                title=Signal::derive(move || i18n.t(if pending().is_some_and(|p| p.file.is_some()) { "ui-term-sendfile-title" } else { "ui-term-paste-title" }))
                confirm_label=tr("ui-term-paste-send")
                on_confirm=move || confirm()
                on_cancel=move || cancel()
            >
                {move || pending().map(|p| {
                    let lines = split_lines(&p.text);
                    let n = lines.len();
                    let preview = lines.into_iter().take(20).collect::<Vec<_>>().join("\n");
                    let delay = store.settings.with(|s| s.line_delay_ms);
                    view! {
                        <p>
                            {p.file.clone().map(|f| view! { <span class="font-mono">{f}" · "</span> })}
                            {i18n.ta("ui-term-paste-lines", &[("n", n.to_string())])}
                        </p>
                        <pre class="max-h-60 overflow-auto rounded-md bg-slate-50 p-2 font-mono text-xs whitespace-pre dark:bg-slate-800">{preview}{(n > 20).then_some("\n…")}</pre>
                        {(delay > 0).then(|| view! { <p class="text-xs text-slate-500">{i18n.ta("ui-term-paste-delay", &[("n", delay.to_string())])}</p> })}
                    }
                })}
            </Confirm>
        </Show>
    }
}

/// Terminal settings (this browser).
#[component]
fn SettingsPanel() -> impl IntoView {
    let store = crate::term::use_term();
    let num = move |f: fn(&mut TermSettings, u32)| {
        move |ev: leptos::ev::Event| {
            if let Ok(v) = event_target_value(&ev).trim().parse::<u32>() {
                store.settings.update(|s| {
                    f(s, v);
                    *s = s.clone().clamped();
                });
            }
        }
    };
    let check = move |f: fn(&mut TermSettings, bool)| {
        move |ev: leptos::ev::Event| {
            let c = event_target_checked(&ev);
            store.settings.update(|s| f(s, c));
        }
    };
    view! {
        <section class=format!("{CARD} space-y-3") aria-labelledby="term-settings">
            <h2 id="term-settings" class="font-semibold">{tr("ui-term-settings")}</h2>
            <div class="grid gap-3 sm:grid-cols-3">
                <div>
                    <label for="term-font" class=LABEL>{tr("ui-term-font-size")}</label>
                    <input id="term-font" class=INPUT type="number" min=TermSettings::FONT_MIN max=TermSettings::FONT_MAX
                        prop:value=move || store.settings.with(|s| s.font_size.to_string())
                        on:change=num(|s, v| s.font_size = u8::try_from(v).unwrap_or(u8::MAX)) />
                </div>
                <div>
                    <label for="term-scrollback" class=LABEL>{tr("ui-term-scrollback")}</label>
                    <input id="term-scrollback" class=INPUT type="number" min="100" max=TermSettings::SCROLLBACK_MAX step="1000"
                        prop:value=move || store.settings.with(|s| s.scrollback.to_string())
                        on:change=num(|s, v| s.scrollback = v) />
                </div>
                <div>
                    <label for="term-delay" class=LABEL>{tr("ui-term-line-delay")}</label>
                    <input id="term-delay" class=INPUT type="number" min="0" max=TermSettings::DELAY_MAX step="10"
                        prop:value=move || store.settings.with(|s| s.line_delay_ms.to_string())
                        on:change=num(|s, v| s.line_delay_ms = v) />
                </div>
            </div>
            <div class="flex flex-col gap-1.5 text-sm">
                <label class="flex items-center gap-1.5">
                    <input type="checkbox" prop:checked=move || store.settings.with(|s| s.copy_on_select) on:change=check(|s, c| s.copy_on_select = c) />
                    {tr("ui-term-copy-on-select")}
                </label>
                <label class="flex items-center gap-1.5">
                    <input type="checkbox" prop:checked=move || store.settings.with(|s| s.right_click_paste) on:change=check(|s, c| s.right_click_paste = c) />
                    {tr("ui-term-right-click-paste")}
                </label>
                <label class="flex items-center gap-1.5">
                    <input type="checkbox" prop:checked=move || store.settings.with(|s| s.confirm_paste) on:change=check(|s, c| s.confirm_paste = c) />
                    {tr("ui-term-confirm-paste")}
                </label>
            </div>
            <p class="text-xs text-slate-500">{tr("ui-term-keys-hint")}</p>
        </section>
    }
}

/// Session logs on the server.
#[component]
fn LogsPanel() -> impl IntoView {
    let i18n = use_i18n();
    let toasts = use_toasts();
    let store = crate::term::use_term();
    let reload = RwSignal::new(0u32);
    let logs = LocalResource::new(move || {
        reload.track();
        // a log appears / stops growing with these
        store
            .tabs
            .with(|v| v.iter().filter(|t| t.recording).count());
        tapi::logs()
    });
    let deleting = RwSignal::new(None::<LogInfo>);
    let busy = RwSignal::new(false);
    let do_delete = move || {
        let Some(l) = deleting.get_untracked() else {
            return;
        };
        busy.set(true);
        leptos::task::spawn_local(async move {
            match tapi::delete_log(&l.name).await {
                Ok(()) => reload.update(|r| *r += 1),
                Err(e) => toasts.error(e.message(i18n)),
            }
            busy.set(false);
            deleting.set(None);
        });
    };
    view! {
        <section class=format!("{CARD} space-y-3") aria-labelledby="term-logs">
            <div class="flex items-center justify-between">
                <h2 id="term-logs" class="font-semibold">{tr("ui-term-logs")}</h2>
                <button type="button" class=BTN_GHOST aria-label=tr("ui-refresh") title=tr("ui-refresh") on:click=move |_| reload.update(|r| *r += 1)>
                    <Icon name=IconName::Refresh class="h-4 w-4" />
                </button>
            </div>
            {move || match logs.get() {
                None => view! { <Loading /> }.into_any(),
                Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                Some(Ok(l)) if l.is_empty() => view! { <Empty text=tr("ui-term-logs-empty") /> }.into_any(),
                Some(Ok(l)) => view! {
                    <div class="max-h-72 overflow-auto">
                        <table class="min-w-full divide-y divide-slate-200 dark:divide-slate-800">
                            <caption class="sr-only">{tr("ui-term-logs")}</caption>
                            <thead>
                                <tr>
                                    <th scope="col" class=TH>{tr("ui-term-log-file")}</th>
                                    <th scope="col" class=TH>{tr("ui-term-log-size")}</th>
                                    <th scope="col" class=TH><span class="sr-only">{tr("ui-term-actions")}</span></th>
                                </tr>
                            </thead>
                            <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                                {l.into_iter().map(|log| {
                                    let del = log.clone();
                                    view! {
                                        <tr>
                                            <td class=format!("{TD} font-mono text-xs break-all")>
                                                {log.name.clone()}
                                                {log.active.then(|| view! { <span class="ml-2 rounded bg-rose-600 px-1 text-[10px] font-bold text-white">"REC"</span> })}
                                                <div class="text-slate-400">{crate::fmt::datetime(i18n.current(), log.modified * 1000)}</div>
                                            </td>
                                            <td class=format!("{TD} whitespace-nowrap")>{crate::fmt::bytes(log.size)}</td>
                                            <td class=format!("{TD} whitespace-nowrap text-right")>
                                                <a class=BTN_GHOST href=tapi::log_url(&log.name) download=log.name.clone()
                                                    aria-label=move || i18n.ta("ui-term-log-download-named", &[("file", log.name.clone())])
                                                    title=tr("ui-term-log-download")>
                                                    <Icon name=IconName::Download class="h-4 w-4" />
                                                </a>
                                                <button type="button" class=BTN_GHOST disabled=log.active
                                                    aria-label=tr("ui-delete") title=tr("ui-delete")
                                                    on:click=move |_| deleting.set(Some(del.clone()))>
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
        </section>
        <Show when=move || deleting.get().is_some()>
            <Confirm
                title=tr("ui-term-log-delete-title")
                confirm_label=tr("ui-delete")
                danger=true
                busy=busy
                on_confirm=move || do_delete()
                on_cancel=move || deleting.set(None)
            >
                <p class="font-mono break-all">{move || deleting.get().map(|l| l.name).unwrap_or_default()}</p>
            </Confirm>
        </Show>
    }
}

/// Trusted SSH host keys; removing one lets a legitimately changed key be
/// accepted again.
#[component]
fn KnownHostsPanel() -> impl IntoView {
    let i18n = use_i18n();
    let toasts = use_toasts();
    let reload = RwSignal::new(0u32);
    let store = crate::term::use_term();
    let hosts = LocalResource::new(move || {
        reload.track();
        store
            .tabs
            .with(|v| v.iter().filter(|t| t.phase == Phase::Connected).count());
        tapi::known_hosts()
    });
    let deleting = RwSignal::new(None::<KnownHost>);
    let busy = RwSignal::new(false);
    let do_delete = move || {
        let Some(h) = deleting.get_untracked() else {
            return;
        };
        busy.set(true);
        leptos::task::spawn_local(async move {
            match tapi::delete_known_host(&h.host, h.port).await {
                Ok(()) => reload.update(|r| *r += 1),
                Err(e) => toasts.error(e.message(i18n)),
            }
            busy.set(false);
            deleting.set(None);
        });
    };
    view! {
        <section class=format!("{CARD} space-y-3") aria-labelledby="term-known">
            <div class="flex items-center justify-between">
                <h2 id="term-known" class="font-semibold">{tr("ui-term-known-hosts")}</h2>
                <button type="button" class=BTN_GHOST aria-label=tr("ui-refresh") title=tr("ui-refresh") on:click=move |_| reload.update(|r| *r += 1)>
                    <Icon name=IconName::Refresh class="h-4 w-4" />
                </button>
            </div>
            {move || match hosts.get() {
                None => view! { <Loading /> }.into_any(),
                Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                Some(Ok(l)) if l.is_empty() => view! { <Empty text=tr("ui-term-known-hosts-empty") /> }.into_any(),
                Some(Ok(l)) => view! {
                    <ul class="max-h-72 divide-y divide-slate-100 overflow-auto text-sm dark:divide-slate-800">
                        {l.into_iter().map(|h| {
                            let del = h.clone();
                            view! {
                                <li class="flex items-start justify-between gap-2 py-2">
                                    <div class="min-w-0">
                                        <p class="font-mono">{format!("{}:{}", h.host, h.port)}</p>
                                        <p class="font-mono text-xs break-all text-slate-500">{format!("{} {}", h.algorithm, h.fingerprint)}</p>
                                        {(!h.comment.is_empty()).then(|| view! { <p class="text-xs text-slate-400">{h.comment.clone()}</p> })}
                                    </div>
                                    <button type="button" class=BTN_GHOST
                                        aria-label=tr("ui-term-known-host-forget") title=tr("ui-term-known-host-forget")
                                        on:click=move |_| deleting.set(Some(del.clone()))>
                                        <Icon name=IconName::Trash class="h-4 w-4" />
                                    </button>
                                </li>
                            }
                        }).collect_view()}
                    </ul>
                }.into_any(),
            }}
        </section>
        <Show when=move || deleting.get().is_some()>
            <Confirm
                title=tr("ui-term-known-host-forget")
                confirm_label=tr("ui-delete")
                danger=true
                busy=busy
                on_confirm=move || do_delete()
                on_cancel=move || deleting.set(None)
            >
                <p>{move || i18n.ta("ui-term-known-host-forget-body", &[("target", deleting.get().map(|h| format!("{}:{}", h.host, h.port)).unwrap_or_default())])}</p>
            </Confirm>
        </Show>
    }
}
