//! Notifications tab: the trap / inform receiver (start / stop, communities,
//! v3 users), the live list of received notifications decoded with MIB names,
//! and the trap / inform sender.

use leptos::prelude::*;
use serde_json::{json, Value};

use super::model::{parse_varbind_lines, AUTH_PROTOCOLS, PRIV_PROTOCOLS};
use super::ops::{use_suggestions, OidDatalist};
use super::use_snmp;
use crate::api::snmp::{self as api, ReceiverStatus, TrapOut};
use crate::api::ApiError;
use crate::components::{
    use_toasts, Empty, ErrorBox, Icon, IconName, Loading, BTN, BTN_DANGER, BTN_GHOST, BTN_PRIMARY,
    CARD, INPUT, LABEL, TD, TH,
};
use crate::fmt;
use crate::i18n::{tr, use_i18n};
use crate::ws::use_live;

/// A v3 user row of the receiver form.
#[derive(Debug, Clone, Copy)]
struct UserRow {
    id: u32,
    user: RwSignal<String>,
    auth_protocol: RwSignal<String>,
    auth_password: RwSignal<String>,
    priv_protocol: RwSignal<String>,
    priv_password: RwSignal<String>,
}

impl UserRow {
    fn new(id: u32) -> Self {
        Self {
            id,
            user: RwSignal::new(String::new()),
            auth_protocol: RwSignal::new("SHA".into()),
            auth_password: RwSignal::new(String::new()),
            priv_protocol: RwSignal::new("AES".into()),
            priv_password: RwSignal::new(String::new()),
        }
    }

    fn to_json(self) -> Value {
        let mut u = json!({ "user": self.user.get_untracked().trim() });
        let ap = self.auth_password.get_untracked();
        if !ap.is_empty() {
            u["auth_protocol"] = json!(self.auth_protocol.get_untracked());
            u["auth_password"] = json!(ap);
        }
        let pp = self.priv_password.get_untracked();
        if !pp.is_empty() {
            u["priv_protocol"] = json!(self.priv_protocol.get_untracked());
            u["priv_password"] = json!(pp);
        }
        u
    }
}

#[component]
pub(crate) fn TrapsPanel() -> impl IntoView {
    view! {
        <div class="space-y-6">
            <ReceiverCard />
            <TrapList />
            <SenderCard />
        </div>
    }
}

#[component]
fn ReceiverCard() -> impl IntoView {
    let i18n = use_i18n();
    let toasts = use_toasts();
    let live = use_live();
    let status = RwSignal::new(None::<ReceiverStatus>);
    let error = RwSignal::new(None::<ApiError>);
    let listen = RwSignal::new("0.0.0.0:162".to_owned());
    let communities = RwSignal::new(String::new());
    let users = RwSignal::new(Vec::<UserRow>::new());
    let next = StoredValue::new(0u32);
    let busy = RwSignal::new(false);
    Effect::new(move |_| {
        live.snmp_rev.track();
        leptos::task::spawn_local(async move {
            if let Ok(s) = api::receiver().await {
                if let (Some(l), true) = (s.listen.clone(), s.running) {
                    listen.set(l);
                }
                status.set(Some(s));
            }
        });
    });
    let start = move |_| {
        let body = json!({
            "listen": listen.get_untracked().trim(),
            "communities": communities
                .get_untracked()
                .split([',', ' '])
                .filter(|c| !c.is_empty())
                .collect::<Vec<_>>(),
            "users": users
                .get_untracked()
                .into_iter()
                .filter(|u| !u.user.get_untracked().trim().is_empty())
                .map(UserRow::to_json)
                .collect::<Vec<_>>(),
        });
        busy.set(true);
        leptos::task::spawn_local(async move {
            match api::receiver_start(&body).await {
                Ok(s) => {
                    toasts.ok(i18n.ta(
                        "ui-snmp-receiver-started",
                        &[("addr", s.listen.clone().unwrap_or_default())],
                    ));
                    status.set(Some(s));
                    error.set(None);
                }
                Err(e) => error.set(Some(e)),
            }
            busy.set(false);
        });
    };
    let stop = move |_| {
        busy.set(true);
        leptos::task::spawn_local(async move {
            match api::receiver_stop().await {
                Ok(s) => status.set(Some(s)),
                Err(e) => toasts.error(e.message(i18n)),
            }
            busy.set(false);
        });
    };
    let running = move || status.with(|s| s.as_ref().is_some_and(|s| s.running));
    view! {
        <section class=CARD aria-labelledby="snmp-rx-h">
            <div class="mb-3 flex flex-wrap items-center gap-3">
                <h2 id="snmp-rx-h" class="text-base font-semibold">{tr("ui-snmp-receiver")}</h2>
                <span
                    role="status"
                    class=move || if running() {
                        "rounded-full bg-emerald-100 px-2 py-0.5 text-xs font-medium text-emerald-800 dark:bg-emerald-500/15 dark:text-emerald-300"
                    } else {
                        "rounded-full bg-slate-100 px-2 py-0.5 text-xs font-medium text-slate-600 dark:bg-slate-800 dark:text-slate-300"
                    }
                >
                    {move || match status.get() {
                        Some(s) if s.running => i18n.ta("ui-snmp-receiver-running", &[("addr", s.listen.unwrap_or_default()), ("count", s.received.to_string())]),
                        _ => i18n.t("ui-snmp-receiver-stopped"),
                    }}
                </span>
            </div>
            <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
                <div>
                    <label for="snmp-rx-listen" class=LABEL>{tr("ui-snmp-listen")}</label>
                    <input id="snmp-rx-listen" class=format!("{INPUT} font-mono") spellcheck="false" aria-describedby="snmp-rx-listen-hint"
                        prop:value=move || listen.get() on:input=move |ev| listen.set(event_target_value(&ev)) />
                </div>
                <div class="lg:col-span-2">
                    <label for="snmp-rx-comm" class=LABEL>{tr("ui-snmp-accepted-communities")}</label>
                    <input id="snmp-rx-comm" class=INPUT spellcheck="false" placeholder=tr("ui-snmp-any-community")
                        prop:value=move || communities.get() on:input=move |ev| communities.set(event_target_value(&ev)) />
                </div>
                <div class="flex items-end gap-2">
                    <button type="button" class=BTN_PRIMARY disabled=move || busy.get() on:click=start>
                        <Icon name=IconName::Play class="h-4 w-4" />
                        {move || i18n.t(if running() { "ui-snmp-receiver-restart" } else { "ui-snmp-receiver-start" })}
                    </button>
                    <Show when=running>
                        <button type="button" class=BTN disabled=move || busy.get() on:click=stop>
                            <Icon name=IconName::Stop class="h-4 w-4" />
                            {tr("ui-snmp-receiver-stop")}
                        </button>
                    </Show>
                </div>
            </div>
            <p id="snmp-rx-listen-hint" class="mt-1 text-xs text-slate-500">{tr("ui-snmp-listen-hint")}</p>
            <div class="mt-3 space-y-2">
                <p class="text-xs font-medium text-slate-600 dark:text-slate-400">{tr("ui-snmp-receiver-users")}</p>
                <For each=move || users.get() key=|u| u.id let:u>
                    <div class="grid gap-2 sm:grid-cols-[1fr_7rem_1fr_7rem_1fr_auto]">
                        <input class=INPUT aria-label=tr("ui-snmp-user") placeholder=tr("ui-snmp-user")
                            prop:value=move || u.user.get() on:input=move |ev| u.user.set(event_target_value(&ev)) />
                        <select class=INPUT aria-label=tr("ui-snmp-auth-protocol") on:change=move |ev| u.auth_protocol.set(event_target_value(&ev))>
                            {AUTH_PROTOCOLS.iter().map(|p| { let p = *p; view! { <option value=p selected=move || u.auth_protocol.get() == p>{p}</option> } }).collect_view()}
                        </select>
                        <input class=INPUT type="password" autocomplete="new-password" aria-label=tr("ui-snmp-auth-password") placeholder=tr("ui-snmp-auth-password")
                            prop:value=move || u.auth_password.get() on:input=move |ev| u.auth_password.set(event_target_value(&ev)) />
                        <select class=INPUT aria-label=tr("ui-snmp-priv-protocol") on:change=move |ev| u.priv_protocol.set(event_target_value(&ev))>
                            {PRIV_PROTOCOLS.iter().map(|p| { let p = *p; view! { <option value=p selected=move || u.priv_protocol.get() == p>{p}</option> } }).collect_view()}
                        </select>
                        <input class=INPUT type="password" autocomplete="new-password" aria-label=tr("ui-snmp-priv-password") placeholder=tr("ui-snmp-priv-password")
                            prop:value=move || u.priv_password.get() on:input=move |ev| u.priv_password.set(event_target_value(&ev)) />
                        <button type="button" class=BTN_GHOST aria-label=tr("ui-snmp-remove-row") title=tr("ui-snmp-remove-row")
                            on:click=move |_| users.update(|l| l.retain(|x| x.id != u.id))>
                            <Icon name=IconName::Close class="h-4 w-4" />
                        </button>
                    </div>
                </For>
                <button type="button" class=BTN on:click=move |_| {
                    let id = next.get_value();
                    next.set_value(id + 1);
                    users.update(|l| l.push(UserRow::new(id)));
                }>
                    <Icon name=IconName::Plus class="h-4 w-4" />
                    {tr("ui-snmp-add-user")}
                </button>
                <p class="text-xs text-slate-500">
                    {move || i18n.ta("ui-snmp-receiver-engine", &[("id", status.get().map(|s| s.engine_id).unwrap_or_default())])}
                </p>
            </div>
            {move || error.get().map(|e| view! { <div class="mt-3"><ErrorBox error=e /></div> })}
        </section>
    }
}

fn trap_title(t: &TrapOut) -> String {
    t.trap_name
        .clone()
        .or_else(|| t.trap_oid.clone())
        .unwrap_or_else(|| "?".into())
}

#[component]
fn TrapList() -> impl IntoView {
    let i18n = use_i18n();
    let toasts = use_toasts();
    let live = use_live();
    let reload = RwSignal::new(0u32);
    let filter = RwSignal::new(String::new());
    let list = LocalResource::new(move || {
        live.snmp_rev.track();
        reload.track();
        api::traps()
    });
    let clear = move |_| {
        leptos::task::spawn_local(async move {
            match api::clear_traps().await {
                Ok(()) => reload.update(|r| *r += 1),
                Err(e) => toasts.error(e.message(i18n)),
            }
        });
    };
    let matches = move |t: &TrapOut, f: &str| {
        if f.is_empty() {
            return true;
        }
        let f = f.to_lowercase();
        trap_title(t).to_lowercase().contains(&f)
            || t.source.contains(&f)
            || t.varbinds
                .iter()
                .any(|v| v.name.to_lowercase().contains(&f) || v.value.to_lowercase().contains(&f))
    };
    view! {
        <section aria-labelledby="snmp-traps-h">
            <div class="mb-2 flex flex-wrap items-center gap-3">
                <h2 id="snmp-traps-h" class="text-base font-semibold">{tr("ui-snmp-received")}</h2>
                <input class=format!("{INPUT} max-w-xs") type="search" placeholder=tr("ui-snmp-filter") aria-label=tr("ui-snmp-filter")
                    prop:value=move || filter.get() on:input=move |ev| filter.set(event_target_value(&ev)) />
                <button type="button" class=BTN_DANGER on:click=clear>
                    <Icon name=IconName::Trash class="h-4 w-4" />
                    {tr("ui-snmp-clear-traps")}
                </button>
            </div>
            {move || match list.get() {
                None => view! { <Loading /> }.into_any(),
                Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                Some(Ok(l)) if l.is_empty() => view! { <Empty text=tr("ui-snmp-no-traps") /> }.into_any(),
                Some(Ok(l)) => {
                    let f = filter.get();
                    view! {
                        <div class="overflow-x-auto rounded-lg border border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
                            <table class="min-w-full divide-y divide-slate-200 dark:divide-slate-800">
                                <caption class="sr-only">{tr("ui-snmp-received")}</caption>
                                <thead>
                                    <tr>
                                        <th scope="col" class=TH>{tr("ui-snmp-time")}</th>
                                        <th scope="col" class=TH>{tr("ui-snmp-source")}</th>
                                        <th scope="col" class=TH>{tr("ui-snmp-notification")}</th>
                                        <th scope="col" class=TH>{tr("ui-snmp-varbinds")}</th>
                                    </tr>
                                </thead>
                                <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                                    {l.into_iter().filter(|t| matches(t, &f)).map(|t| view! { <TrapRow trap=t /> }).collect_view()}
                                </tbody>
                            </table>
                        </div>
                    }
                    .into_any()
                }
            }}
        </section>
    }
}

#[component]
fn TrapRow(trap: TrapOut) -> impl IntoView {
    let i18n = use_i18n();
    let when = trap.received_ms;
    let sec = match (&trap.community, &trap.user) {
        (Some(c), _) => format!("{} · {c}", trap.version),
        (_, Some(u)) => format!(
            "{} · {u} · {}",
            trap.version,
            trap.level.clone().unwrap_or_default()
        ),
        _ => trap.version.clone(),
    };
    let kind = trap.pdu_type.clone().unwrap_or_default();
    let n = trap.varbinds.len();
    view! {
        <tr class=if trap.error.is_some() { "bg-rose-50 align-top dark:bg-rose-950/30" } else { "align-top" }>
            <td class=format!("{TD} whitespace-nowrap text-xs")>{move || fmt::datetime(i18n.locale.get(), when)}</td>
            <td class=format!("{TD} text-xs")>
                <span class="font-mono">{trap.source.clone()}</span>
                <span class="block text-slate-500">{sec}</span>
            </td>
            <td class=TD>
                <span class="font-medium break-all">{trap_title(&trap)}</span>
                <span class="block text-xs text-slate-500">
                    {kind}
                    {trap.uptime_text.clone().map(|u| format!(" · {u}"))}
                    {trap.acknowledged.then(|| view! { " · " {tr("ui-snmp-acknowledged")} })}
                </span>
                {trap.v1.clone().map(|h| view! {
                    <span class="block font-mono text-xs text-slate-500">
                        {format!("{} {} {}/{}", h.enterprise, h.agent_addr, h.generic, h.specific)}
                    </span>
                })}
                {trap.error.clone().map(|e| view! { <span class="block text-xs text-rose-700 dark:text-rose-300" role="note">{e}</span> })}
            </td>
            <td class=TD>
                <details>
                    <summary class="cursor-pointer text-xs text-sky-700 dark:text-sky-400">
                        {move || i18n.ta("ui-snmp-varbind-count", &[("count", n.to_string())])}
                    </summary>
                    <dl class="mt-1 grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 text-xs">
                        {trap.varbinds.into_iter().map(|v| view! {
                            <dt class="break-all text-slate-500" title=v.oid.clone()>{v.name.clone()}</dt>
                            <dd class="font-mono break-all">{v.value.clone()}</dd>
                        }).collect_view()}
                    </dl>
                </details>
            </td>
        </tr>
    }
}

#[component]
fn SenderCard() -> impl IntoView {
    let i18n = use_i18n();
    let toasts = use_toasts();
    let ctx = use_snmp();
    let port = RwSignal::new("162".to_owned());
    let trap_oid = RwSignal::new("linkDown".to_owned());
    let uptime = RwSignal::new(String::new());
    let lines = RwSignal::new("ifIndex.2 2\nifAdminStatus.2 up\nifOperStatus.2 down".to_owned());
    let inform = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<ApiError>);
    let form_error = RwSignal::new(None::<String>);
    let suggestions = use_suggestions(trap_oid);
    let send = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let t = ctx.target.get_untracked();
        if let Err(k) = t.check() {
            form_error.set(Some(i18n.t(k)));
            return;
        }
        let vbs = match parse_varbind_lines(&lines.get_untracked()) {
            Ok(v) => v,
            Err(line) => {
                form_error.set(Some(
                    i18n.ta("ui-snmp-bad-line", &[("index", line.to_string())]),
                ));
                return;
            }
        };
        form_error.set(None);
        let mut body = json!({
            "target": t.to_json(port.get_untracked().trim().parse::<u16>().ok()),
            "trap_oid": trap_oid.get_untracked().trim(),
            "varbinds": vbs.iter().map(super::model::VarBindLine::to_json).collect::<Vec<_>>(),
        });
        if let Ok(u) = uptime.get_untracked().trim().parse::<u32>() {
            body["uptime"] = json!(u);
        }
        let op = if inform.get_untracked() {
            "inform"
        } else {
            "trap"
        };
        busy.set(true);
        leptos::task::spawn_local(async move {
            match api::operation(op, &body).await {
                Ok(r) => {
                    toasts.ok(i18n.ta(
                        if op == "inform" {
                            "ui-snmp-inform-acked"
                        } else {
                            "ui-snmp-trap-sent"
                        },
                        &[("target", r.target)],
                    ));
                    error.set(None);
                }
                Err(e) => error.set(Some(e)),
            }
            busy.set(false);
            ctx.capture_rev.update(|r| *r += 1);
        });
    };
    view! {
        <section class=CARD aria-labelledby="snmp-send-h">
            <h2 id="snmp-send-h" class="mb-1 text-base font-semibold">{tr("ui-snmp-send")}</h2>
            <p class="mb-3 text-xs text-slate-500">{tr("ui-snmp-send-hint")}</p>
            <p class="mb-3 text-sm">
                {move || ctx.target.with(|t| i18n.ta("ui-snmp-send-to", &[("target", if t.host.is_empty() { "—".to_owned() } else { t.host.clone() }), ("version", t.version.clone())]))}
                " "
                <button type="button" class="text-sky-700 underline dark:text-sky-400" on:click=move |_| ctx.tab.set(super::Tab::Ops)>
                    {tr("ui-snmp-edit-target")}
                </button>
            </p>
            <form on:submit=send novalidate class="space-y-3">
                <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
                    <div class="sm:col-span-2">
                        <label for="snmp-trap-oid" class=LABEL>{tr("ui-snmp-trap-oid")}</label>
                        <input id="snmp-trap-oid" class=format!("{INPUT} font-mono") list="snmp-trap-suggest" spellcheck="false" autocomplete="off"
                            prop:value=move || trap_oid.get() on:input=move |ev| trap_oid.set(event_target_value(&ev)) />
                        <OidDatalist id="snmp-trap-suggest" items=suggestions />
                    </div>
                    <div>
                        <label for="snmp-trap-port" class=LABEL>{tr("ui-snmp-port")}</label>
                        <input id="snmp-trap-port" class=INPUT inputmode="numeric"
                            prop:value=move || port.get() on:input=move |ev| port.set(event_target_value(&ev)) />
                    </div>
                    <div>
                        <label for="snmp-trap-uptime" class=LABEL>{tr("ui-snmp-uptime")}</label>
                        <input id="snmp-trap-uptime" class=INPUT inputmode="numeric" placeholder=tr("ui-snmp-uptime-auto")
                            prop:value=move || uptime.get() on:input=move |ev| uptime.set(event_target_value(&ev)) />
                    </div>
                </div>
                <div>
                    <label for="snmp-trap-vbs" class=LABEL>{tr("ui-snmp-varbinds")}</label>
                    <textarea id="snmp-trap-vbs" rows="4" class=format!("{INPUT} font-mono") spellcheck="false" aria-describedby="snmp-trap-vbs-hint"
                        prop:value=move || lines.get() on:input=move |ev| lines.set(event_target_value(&ev))></textarea>
                    <p id="snmp-trap-vbs-hint" class="mt-1 text-xs text-slate-500">{tr("ui-snmp-varbind-lines-hint")}</p>
                </div>
                <div class="flex flex-wrap items-center gap-3">
                    <label class="flex items-center gap-2 text-sm">
                        <input type="checkbox" prop:checked=move || inform.get() on:change=move |ev| inform.set(event_target_checked(&ev)) />
                        {tr("ui-snmp-inform")}
                    </label>
                    <button type="submit" class=BTN_PRIMARY disabled=move || busy.get()>
                        <Icon name=IconName::Play class="h-4 w-4" />
                        {tr("ui-snmp-send-button")}
                    </button>
                </div>
                {move || form_error.get().map(|m| view! { <p class="text-sm text-rose-600 dark:text-rose-400" role="alert">{m}</p> })}
                {move || error.get().map(|e| view! { <ErrorBox error=e /> })}
            </form>
        </section>
    }
}
