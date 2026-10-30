//! Operations tab: OID input with MIB name completion and a MIB tree
//! picker, Get / GetNext /
//! GetBulk / Walk / Set, readable results (list or table view, raw toggle),
//! auto refresh with counter rates and sparklines.

use std::time::Duration;

use leptos::prelude::*;
use serde_json::{json, Value};

use super::model::{parse_oid_list, rate_text, split_last_token, Rates, VALUE_TYPES};
use super::target::TargetForm;
use super::tree::{MibTree, ModuleSelect};
use super::use_snmp;
use crate::api::snmp::{self as api, SearchHit, SnmpResult, TableOut, VarBindOut};
use crate::api::ApiError;
use crate::components::{
    Empty, ErrorBox, Icon, IconName, BTN, BTN_GHOST, BTN_PRIMARY, CARD, INPUT, LABEL, TD, TH,
};
use crate::i18n::{tr, use_i18n};
use crate::sparkline::Sparkline;

/// Delay before the OID field asks for completions.
const SUGGEST_DEBOUNCE_MS: u64 = 200;
/// Auto refresh choices in seconds (0 = off).
const POLL_CHOICES: [u32; 6] = [0, 1, 2, 5, 10, 30];

/// The operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Op {
    Get,
    GetNext,
    GetBulk,
    Walk,
    Set,
}

impl Op {
    pub(crate) const fn path(self) -> &'static str {
        match self {
            Op::Get => "get",
            Op::GetNext => "getnext",
            Op::GetBulk => "getbulk",
            Op::Walk => "walk",
            Op::Set => "set",
        }
    }

    const fn key(self) -> &'static str {
        match self {
            Op::Get => "ui-snmp-op-get",
            Op::GetNext => "ui-snmp-op-getnext",
            Op::GetBulk => "ui-snmp-op-getbulk",
            Op::Walk => "ui-snmp-op-walk",
            Op::Set => "ui-snmp-op-set",
        }
    }
}

/// One row of the Set editor.
#[derive(Debug, Clone, Copy)]
struct SetRow {
    id: u32,
    oid: RwSignal<String>,
    ty: RwSignal<String>,
    value: RwSignal<String>,
}

impl SetRow {
    fn new(id: u32) -> Self {
        Self {
            id,
            oid: RwSignal::new(String::new()),
            ty: RwSignal::new("auto".into()),
            value: RwSignal::new(String::new()),
        }
    }
}

/// Feeds the counter bindings of a result into the rates.
fn record_rates(rates: &mut Rates, r: &SnmpResult, now: f64) {
    for vb in r.varbinds.iter().filter(|v| v.is_counter()) {
        if let Some(n) = vb.numeric {
            rates.update(&vb.oid, n, now, vb.value_type == "Counter32");
        }
    }
}

/// Live completions for an OID field: a `<datalist>` filled from
/// `/api/snmp/mibs/search` with the last word of `text`.
pub(crate) fn use_suggestions(text: RwSignal<String>) -> RwSignal<Vec<(String, SearchHit)>> {
    let out = RwSignal::new(Vec::new());
    let gen = StoredValue::new(0u32);
    Effect::new(move |_| {
        let s = text.get();
        let g = gen.get_value().wrapping_add(1);
        gen.set_value(g);
        let (prefix, token) = split_last_token(&s);
        let (prefix, token) = (prefix.to_owned(), token.to_owned());
        if token.chars().count() < 2 || token.starts_with(|c: char| c.is_ascii_digit() || c == '.')
        {
            out.set(Vec::new());
            return;
        }
        set_timeout(
            move || {
                if gen.get_value() != g {
                    return;
                }
                leptos::task::spawn_local(async move {
                    if let Ok(hits) = api::search(&token, 15).await {
                        if gen.get_value() == g {
                            out.set(
                                hits.into_iter()
                                    .map(|h| (format!("{prefix}{}", h.name), h))
                                    .collect(),
                            );
                        }
                    }
                });
            },
            Duration::from_millis(SUGGEST_DEBOUNCE_MS),
        );
    });
    out
}

/// The `<datalist>` of [`use_suggestions`].
#[component]
pub(crate) fn OidDatalist(
    id: &'static str,
    items: RwSignal<Vec<(String, SearchHit)>>,
) -> impl IntoView {
    view! {
        <datalist id=id>
            {move || {
                items
                    .get()
                    .into_iter()
                    .map(|(value, h)| {
                        let label = format!(
                            "{}::{} {}",
                            h.module,
                            h.name,
                            h.syntax.unwrap_or_default()
                        );
                        view! { <option value=value label=label></option> }
                    })
                    .collect_view()
            }}
        </datalist>
    }
}

#[component]
pub(crate) fn OpsPanel() -> impl IntoView {
    let i18n = use_i18n();
    let ctx = use_snmp();
    let max_rep = RwSignal::new("20".to_owned());
    let limit = RwSignal::new("10000".to_owned());
    let result = RwSignal::new(None::<SnmpResult>);
    let error = RwSignal::new(None::<ApiError>);
    let form_error = RwSignal::new(None::<&'static str>);
    let busy = RwSignal::new(false);
    let running = RwSignal::new(None::<Op>);
    let last = StoredValue::new(None::<(Op, Value)>);
    let rates = RwSignal::new(Rates::default());
    let raw = RwSignal::new(false);
    let table_view = RwSignal::new(true);
    let poll = RwSignal::new(0u32);
    let next_row = StoredValue::new(1u32);
    let rows = RwSignal::new(vec![SetRow::new(0)]);
    let suggestions = use_suggestions(ctx.oids);
    let picker = RwSignal::new(false);
    // The picker only checks OIDs; nothing is "selected" there.
    let pick_sel = RwSignal::new(None::<String>);

    let build = move |op: Op| -> Result<Value, &'static str> {
        let t = ctx.target.get_untracked();
        t.check()?;
        let target = t.to_json(None);
        let oids = parse_oid_list(&ctx.oids.get_untracked());
        let reps = max_rep.get_untracked().trim().parse::<i32>().unwrap_or(20);
        match op {
            Op::Get | Op::GetNext | Op::GetBulk | Op::Walk if oids.is_empty() => {
                Err("ui-snmp-need-oid")
            }
            Op::Get | Op::GetNext => Ok(json!({ "target": target, "oids": oids })),
            Op::GetBulk => Ok(json!({ "target": target, "oids": oids, "max_repetitions": reps })),
            Op::Walk => Ok(json!({
                "target": target,
                "oid": oids[0],
                "max_repetitions": reps,
                "limit": limit.get_untracked().trim().parse::<usize>().unwrap_or(10_000),
            })),
            Op::Set => {
                let vbs: Vec<Value> = rows
                    .get_untracked()
                    .iter()
                    .filter(|r| !r.oid.get_untracked().trim().is_empty())
                    .map(|r| {
                        json!({
                            "oid": r.oid.get_untracked().trim(),
                            "type": r.ty.get_untracked(),
                            "value": r.value.get_untracked(),
                        })
                    })
                    .collect();
                if vbs.is_empty() {
                    Err("ui-snmp-need-oid")
                } else {
                    Ok(json!({ "target": target, "varbinds": vbs }))
                }
            }
        }
    };

    let run = move |op: Op, body: Value, from_poll: bool| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        running.set(Some(op));
        leptos::task::spawn_local(async move {
            match api::operation(op.path(), &body).await {
                Ok(r) => {
                    rates.update(|rt| record_rates(rt, &r, crate::dom::now_ms()));
                    result.set(Some(r));
                    error.set(None);
                }
                Err(e) => {
                    error.set(Some(e));
                    if from_poll {
                        poll.set(0);
                    }
                }
            }
            busy.set(false);
            running.set(None);
            ctx.capture_rev.update(|r| *r += 1);
        });
    };
    let start = move |op: Op| match build(op) {
        Ok(body) => {
            form_error.set(None);
            rates.update(Rates::clear);
            last.set_value((op != Op::Set).then(|| (op, body.clone())));
            if op == Op::Walk || op == Op::GetBulk {
                table_view.set(true);
            }
            run(op, body, false);
        }
        Err(k) => form_error.set(Some(k)),
    };
    // Runs what the MIB tab asked for (its node details' Get / Walk buttons).
    Effect::new(move |_| {
        if let Some(op) = ctx.run.get() {
            ctx.run.set(None);
            start(op);
        }
    });

    // Auto refresh: repeats the last read operation.
    let handle = StoredValue::new(None::<IntervalHandle>);
    Effect::new(move |_| {
        let secs = poll.get();
        if let Some(h) = handle.get_value() {
            h.clear();
        }
        handle.set_value(None);
        if secs > 0 {
            if let Ok(h) = set_interval_with_handle(
                move || {
                    if let Some((op, body)) = last.get_value() {
                        run(op, body, true);
                    }
                },
                Duration::from_secs(u64::from(secs)),
            ) {
                handle.set_value(Some(h));
            }
        }
    });
    on_cleanup(move || {
        if let Some(h) = handle.get_value() {
            h.clear();
        }
    });

    let op_button = move |op: Op, primary: bool| {
        view! {
            <button
                type="button"
                class=if primary { BTN_PRIMARY } else { BTN }
                disabled=move || busy.get()
                aria-busy=move || (running.get() == Some(op)).to_string()
                on:click=move |_| start(op)
            >
                {move || i18n.t(op.key())}
            </button>
        }
    };
    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        start(Op::Get);
    };

    view! {
        <div class="space-y-6">
            <section class=CARD>
                <TargetForm />
            </section>

            <section class=CARD aria-labelledby="snmp-ops-h">
                <h2 id="snmp-ops-h" class="mb-3 text-base font-semibold">{tr("ui-snmp-operations")}</h2>
                <form on:submit=on_submit novalidate class="space-y-3">
                    <div>
                        <label for="snmp-oids" class=LABEL>{tr("ui-snmp-oids")}</label>
                        <input
                            id="snmp-oids"
                            class=format!("{INPUT} font-mono")
                            list="snmp-oid-suggest"
                            spellcheck="false"
                            autocomplete="off"
                            placeholder="sysDescr.0 IF-MIB::ifTable 1.3.6.1.2.1.1"
                            aria-describedby="snmp-oids-hint"
                            prop:value=move || ctx.oids.get()
                            on:input=move |ev| ctx.oids.set(event_target_value(&ev))
                        />
                        <OidDatalist id="snmp-oid-suggest" items=suggestions />
                        <p id="snmp-oids-hint" class="mt-1 text-xs text-slate-500">{tr("ui-snmp-oids-hint")}</p>
                        <button
                            type="button"
                            class=format!("{BTN_GHOST} mt-2")
                            aria-expanded=move || picker.get().to_string()
                            aria-controls="snmp-oid-picker"
                            on:click=move |_| picker.update(|p| *p = !*p)
                        >
                            {move || if picker.get() { "▾ " } else { "▸ " }}
                            {tr("ui-snmp-mib-pick-tree")}
                        </button>
                        // Mounted on opening, so the module list is current.
                        <Show when=move || picker.get()>
                            <div id="snmp-oid-picker" class="mt-2 rounded-lg border border-slate-200 p-3 dark:border-slate-800">
                                <ModuleSelect id="snmp-ops-mib-scope" module=ctx.mib_module />
                                <p id="snmp-oid-picker-h" class="mt-2 text-xs text-slate-500">{tr("ui-snmp-mib-pick-hint")}</p>
                                <div class="mt-2 max-h-80 overflow-auto">
                                    <MibTree module=ctx.mib_module selected=pick_sel labelledby="snmp-oid-picker-h" picked=ctx.oids />
                                </div>
                            </div>
                        </Show>
                    </div>
                    <div class="flex flex-wrap items-end gap-3">
                        {op_button(Op::Get, true)}
                        {op_button(Op::GetNext, false)}
                        {op_button(Op::GetBulk, false)}
                        {op_button(Op::Walk, false)}
                        <div class="w-28">
                            <label for="snmp-maxrep" class=LABEL>{tr("ui-snmp-max-repetitions")}</label>
                            <input id="snmp-maxrep" class=INPUT inputmode="numeric"
                                prop:value=move || max_rep.get()
                                on:input=move |ev| max_rep.set(event_target_value(&ev)) />
                        </div>
                        <div class="w-28">
                            <label for="snmp-limit" class=LABEL>{tr("ui-snmp-walk-limit")}</label>
                            <input id="snmp-limit" class=INPUT inputmode="numeric"
                                prop:value=move || limit.get()
                                on:input=move |ev| limit.set(event_target_value(&ev)) />
                        </div>
                        <div class="w-36">
                            <label for="snmp-poll" class=LABEL>{tr("ui-snmp-auto-refresh")}</label>
                            <select id="snmp-poll" class=INPUT
                                on:change=move |ev| poll.set(event_target_value(&ev).parse().unwrap_or(0))>
                                {POLL_CHOICES
                                    .into_iter()
                                    .map(|s| {
                                        let label = move || {
                                            if s == 0 {
                                                i18n.t("ui-snmp-refresh-off")
                                            } else {
                                                i18n.ta("ui-snmp-refresh-every", &[("secs", s.to_string())])
                                            }
                                        };
                                        view! { <option value=s.to_string() selected=move || poll.get() == s>{label}</option> }
                                    })
                                    .collect_view()}
                            </select>
                        </div>
                    </div>
                    {move || form_error.get().map(|k| view! {
                        <p class="text-sm text-rose-600 dark:text-rose-400" role="alert">{move || i18n.t(k)}</p>
                    })}
                </form>
            </section>

            <SetEditor rows=rows next_row=next_row busy=busy on_set=move || start(Op::Set) />

            <section aria-labelledby="snmp-result-h" aria-busy=move || busy.get().to_string()>
                <div class="mb-2 flex flex-wrap items-center gap-3">
                    <h2 id="snmp-result-h" class="text-base font-semibold">{tr("ui-snmp-result")}</h2>
                    <label class="flex items-center gap-1.5 text-sm">
                        <input type="checkbox" prop:checked=move || raw.get() on:change=move |ev| raw.set(event_target_checked(&ev)) />
                        {tr("ui-snmp-raw")}
                    </label>
                    <Show when=move || result.with(|r| r.as_ref().is_some_and(|r| !r.tables.is_empty()))>
                        <label class="flex items-center gap-1.5 text-sm">
                            <input type="checkbox" prop:checked=move || table_view.get() on:change=move |ev| table_view.set(event_target_checked(&ev)) />
                            {tr("ui-snmp-table-view")}
                        </label>
                    </Show>
                    <Show when=move || poll.get() != 0>
                        <span class="flex items-center gap-1 text-xs text-emerald-700 dark:text-emerald-400" role="status">
                            <span class="h-2 w-2 animate-pulse rounded-full bg-current"></span>
                            {move || i18n.ta("ui-snmp-refresh-every", &[("secs", poll.get().to_string())])}
                        </span>
                    </Show>
                </div>
                {move || error.get().map(|e| view! { <ErrorBox error=e /> })}
                {move || match result.get() {
                    None => view! { <Empty text=tr("ui-snmp-no-result") /> }.into_any(),
                    Some(r) => view! { <ResultView result=r raw=raw table_view=table_view rates=rates /> }.into_any(),
                }}
            </section>
        </div>
    }
}

/// The Set editor (rows of OID / type / value).
#[component]
fn SetEditor(
    rows: RwSignal<Vec<SetRow>>,
    next_row: StoredValue<u32>,
    busy: RwSignal<bool>,
    #[prop(into)] on_set: Callback<()>,
) -> impl IntoView {
    let i18n = use_i18n();
    let add = move |_| {
        let id = next_row.get_value();
        next_row.set_value(id + 1);
        rows.update(|r| r.push(SetRow::new(id)));
    };
    view! {
        <section class=CARD aria-labelledby="snmp-set-h">
            <h2 id="snmp-set-h" class="mb-1 text-base font-semibold">{tr("ui-snmp-op-set")}</h2>
            <p class="mb-3 text-xs text-slate-500">{tr("ui-snmp-set-hint")}</p>
            <div class="space-y-2">
                <For each=move || rows.get() key=|r| r.id let:row>
                    {
                        let sugg = use_suggestions(row.oid);
                        let list_id = format!("snmp-set-suggest-{}", row.id);
                        view! {
                            <div class="grid gap-2 sm:grid-cols-[2fr_8rem_2fr_auto]">
                                <input
                                    class=format!("{INPUT} font-mono")
                                    list=list_id.clone()
                                    spellcheck="false"
                                    autocomplete="off"
                                    placeholder="IF-MIB::ifAdminStatus.2"
                                    aria-label=tr("ui-snmp-oid")
                                    prop:value=move || row.oid.get()
                                    on:input=move |ev| row.oid.set(event_target_value(&ev))
                                />
                                <datalist id=list_id>
                                    {move || sugg.get().into_iter().map(|(v, _)| view! { <option value=v></option> }).collect_view()}
                                </datalist>
                                <select class=INPUT aria-label=tr("ui-snmp-type") on:change=move |ev| row.ty.set(event_target_value(&ev))>
                                    {VALUE_TYPES
                                        .iter()
                                        .map(|(code, key)| {
                                            let code = *code;
                                            let key = *key;
                                            view! { <option value=code selected=move || row.ty.get() == code>{move || i18n.t(key)}</option> }
                                        })
                                        .collect_view()}
                                </select>
                                <input
                                    class=INPUT
                                    spellcheck="false"
                                    placeholder=tr("ui-snmp-value-placeholder")
                                    aria-label=tr("ui-snmp-value")
                                    prop:value=move || row.value.get()
                                    on:input=move |ev| row.value.set(event_target_value(&ev))
                                />
                                <button
                                    type="button"
                                    class=BTN_GHOST
                                    aria-label=tr("ui-snmp-remove-row")
                                    title=tr("ui-snmp-remove-row")
                                    on:click=move |_| rows.update(|r| r.retain(|x| x.id != row.id))
                                >
                                    <Icon name=IconName::Close class="h-4 w-4" />
                                </button>
                            </div>
                        }
                    }
                </For>
            </div>
            <div class="mt-3 flex flex-wrap gap-2">
                <button type="button" class=BTN on:click=add>
                    <Icon name=IconName::Plus class="h-4 w-4" />
                    {tr("ui-snmp-add-row")}
                </button>
                <button type="button" class=BTN_PRIMARY disabled=move || busy.get() on:click=move |_| on_set.run(())>
                    {tr("ui-snmp-op-set")}
                </button>
            </div>
        </section>
    }
}

/// Exception values (`noSuchObject`, ...) are highlighted.
fn is_exception(vb: &VarBindOut) -> bool {
    matches!(
        vb.value_type.as_str(),
        "noSuchObject" | "noSuchInstance" | "endOfMibView"
    )
}

/// A result: summary, bindings (list) or tables.
#[component]
pub(crate) fn ResultView(
    result: SnmpResult,
    raw: RwSignal<bool>,
    table_view: RwSignal<bool>,
    rates: RwSignal<Rates>,
) -> impl IntoView {
    let i18n = use_i18n();
    let summary = {
        let r = result.clone();
        move || {
            i18n.ta(
                "ui-snmp-summary",
                &[
                    ("action", r.op.clone()),
                    ("target", r.target.clone()),
                    ("version", r.version.clone()),
                    ("count", r.varbinds.len().to_string()),
                    ("time", format!("{:.1}", r.elapsed_ms)),
                    ("n", r.requests.to_string()),
                ],
            )
        }
    };
    let status = (result.error_status != 0).then(|| {
        let name = result.error_status_name.clone().unwrap_or_default();
        let index = result.error_index;
        view! {
            <p class="text-sm font-medium text-rose-700 dark:text-rose-300" role="alert">
                {move || i18n.ta("ui-snmp-error-status", &[("error", name.clone()), ("index", index.to_string())])}
            </p>
        }
    });
    let truncated = result.truncated;
    let engine = result.engine_id.clone();
    let tables = StoredValue::new(result.tables.clone());
    let has_tables = !result.tables.is_empty();
    let error_index = usize::try_from(result.error_index).unwrap_or(0);
    let varbinds = result.varbinds;
    view! {
        <div class="space-y-3">
            <p class="text-xs text-slate-500" aria-live="polite">{summary}</p>
            {status}
            {truncated.then(|| view! { <p class="text-xs text-amber-700 dark:text-amber-400">{tr("ui-snmp-truncated")}</p> })}
            {engine.map(|e| view! { <p class="font-mono text-xs text-slate-500">{move || i18n.ta("ui-snmp-engine-id", &[("id", e.clone())])}</p> })}
            {move || if has_tables && table_view.get() {
                tables
                    .get_value()
                    .into_iter()
                    .map(|t| view! { <TableBlock table=t raw=raw rates=rates /> })
                    .collect_view()
                    .into_any()
            } else {
                view! { <VarBindTable varbinds=varbinds.clone() raw=raw rates=rates error_index=error_index /> }.into_any()
            }}
        </div>
    }
}

/// Bindings as a list.
#[component]
fn VarBindTable(
    varbinds: Vec<VarBindOut>,
    raw: RwSignal<bool>,
    rates: RwSignal<Rates>,
    error_index: usize,
) -> impl IntoView {
    let i18n = use_i18n();
    let any_counter = varbinds.iter().any(VarBindOut::is_counter);
    view! {
        <div class="overflow-x-auto rounded-lg border border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
            <table class="min-w-full divide-y divide-slate-200 dark:divide-slate-800">
                <caption class="sr-only">{tr("ui-snmp-result")}</caption>
                <thead>
                    <tr>
                        <th scope="col" class=TH>{tr("ui-snmp-name")}</th>
                        <th scope="col" class=TH>{tr("ui-snmp-type")}</th>
                        <th scope="col" class=TH>{tr("ui-snmp-value")}</th>
                        {any_counter.then(|| view! { <th scope="col" class=TH>{tr("ui-snmp-rate")}</th> })}
                    </tr>
                </thead>
                <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                    {varbinds
                        .into_iter()
                        .enumerate()
                        .map(|(i, vb)| {
                            let bad = i + 1 == error_index || is_exception(&vb);
                            let (value, rawv) = (vb.value.clone(), vb.raw.clone());
                            let oid = StoredValue::new(vb.oid.clone());
                            let counter = vb.is_counter();
                            let ty = match &vb.syntax {
                                Some(s) if s != &vb.value_type => format!("{} ({s})", vb.value_type),
                                _ => vb.value_type.clone(),
                            };
                            view! {
                                <tr class=if bad { "bg-rose-50 dark:bg-rose-950/30" } else { "" }>
                                    <td class=TD>
                                        <span class="font-medium break-all">{vb.name.clone()}</span>
                                        <span class="block font-mono text-xs text-slate-500 break-all">{vb.oid.clone()}</span>
                                    </td>
                                    <td class=format!("{TD} whitespace-nowrap text-xs text-slate-600 dark:text-slate-400")>{ty}</td>
                                    <td class=format!("{TD} font-mono break-all whitespace-pre-wrap")>
                                        {move || if raw.get() { rawv.clone() } else { value.clone() }}
                                    </td>
                                    {any_counter.then(|| view! {
                                        <td class=format!("{TD} whitespace-nowrap")>
                                            {counter.then(|| view! {
                                                <span class="tabular-nums text-xs">
                                                    {move || rates.with(|r| r.rate.get(&oid.get_value()).map_or_else(|| "–".to_owned(), |x| rate_text(*x)))}
                                                </span>
                                                <Sparkline
                                                    values=Signal::derive(move || rates.with(|r| r.history.get(&oid.get_value()).map(|h| h.iter().copied().collect()).unwrap_or_default()))
                                                    label=Signal::derive(move || i18n.ta("ui-snmp-rate-history", &[("target", oid.get_value())]))
                                                    class="h-6 w-24 text-sky-600"
                                                />
                                            })}
                                        </td>
                                    })}
                                </tr>
                            }
                        })
                        .collect_view()}
                </tbody>
            </table>
        </div>
    }
}

/// A conceptual table: rows keyed by INDEX, one column per object.
#[component]
fn TableBlock(table: TableOut, raw: RwSignal<bool>, rates: RwSignal<Rates>) -> impl IntoView {
    let i18n = use_i18n();
    let title = table.table.clone().unwrap_or_else(|| table.entry.clone());
    let index_head = if table.index_names.is_empty() {
        i18n.t("ui-snmp-index")
    } else {
        table.index_names.join(", ")
    };
    let cols = StoredValue::new(table.columns.clone());
    view! {
        <div class="overflow-x-auto rounded-lg border border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
            <table class="min-w-full divide-y divide-slate-200 text-sm dark:divide-slate-800">
                <caption class="px-3 py-2 text-left text-sm font-semibold">
                    {title}
                    <span class="ml-2 text-xs font-normal text-slate-500">
                        {let n = table.rows.len(); move || i18n.ta("ui-snmp-rows", &[("count", n.to_string())])}
                    </span>
                </caption>
                <thead>
                    <tr>
                        <th scope="col" class=TH>{index_head}</th>
                        {table
                            .columns
                            .iter()
                            .map(|c| view! { <th scope="col" class=TH title=format!("{} {}", c.oid, c.syntax)>{c.name.clone()}</th> })
                            .collect_view()}
                    </tr>
                </thead>
                <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                    {table
                        .rows
                        .iter()
                        .map(|row| {
                            let idx = if row.index.is_empty() { row.instance.clone() } else { row.index.join(", ") };
                            let instance = row.instance.clone();
                            let cells = row.cells.clone();
                            let nums = row.numeric.clone();
                            view! {
                                <tr>
                                    <th scope="row" class=format!("{TD} font-mono text-xs whitespace-nowrap")>{idx}</th>
                                    {cells
                                        .into_iter()
                                        .zip(nums)
                                        .enumerate()
                                        .map(|(ci, (cell, num))| {
                                            let col = cols.with_value(|c| c.get(ci).cloned()).unwrap_or_default();
                                            let key = format!("{}.{instance}", col.oid);
                                            let text = cell.unwrap_or_default();
                                            let rawv = num.map_or_else(|| text.clone(), |n| n.to_string());
                                            view! {
                                                <td class=format!("{TD} whitespace-nowrap")>
                                                    <span class="font-mono">{move || if raw.get() { rawv.clone() } else { text.clone() }}</span>
                                                    {col.counter.then(|| view! {
                                                        <span class="block text-xs tabular-nums text-sky-700 dark:text-sky-400">
                                                            {move || rates.with(|r| r.rate.get(&key).map(|x| rate_text(*x)))}
                                                        </span>
                                                    })}
                                                </td>
                                            }
                                        })
                                        .collect_view()}
                                </tr>
                            }
                        })
                        .collect_view()}
                </tbody>
            </table>
        </div>
    }
}
