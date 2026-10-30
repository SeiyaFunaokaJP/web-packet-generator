//! MIB tab: upload (file picker or drag & drop), the module list with load
//! problems (missing imports named), deletion of uploaded modules, and the
//! tree browser (whole tree or one module) with node details and direct
//! Get / GetNext / Walk.

use leptos::html;
use leptos::prelude::*;

use super::model::request_oid;
use super::ops::Op;
use super::tree::{MibTree, ModuleSelect};
use super::{use_snmp, Tab};
use crate::api::snmp::{self as api, MibModule, NodeInfo, UploadedFile};
use crate::components::{
    use_toasts, Confirm, Empty, ErrorBox, Icon, IconName, Loading, BTN, BTN_GHOST, CARD, INPUT, TD,
    TH,
};
use crate::i18n::{tr, use_i18n};

#[component]
pub(crate) fn MibsPanel() -> impl IntoView {
    let reload = RwSignal::new(0u32);
    let selected = RwSignal::new(None::<String>);
    view! {
        <div class="space-y-6">
            <UploadCard on_done=move || reload.update(|r| *r += 1) />
            <div class="grid gap-6 xl:grid-cols-2">
                <BrowserCard selected=selected reload=reload />
                <NodeDetail selected=selected />
            </div>
            <ModuleList reload=reload />
        </div>
    }
}

/// Reads the text of the files (browser only).
#[cfg(target_arch = "wasm32")]
async fn read_files(list: web_sys::FileList) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for i in 0..list.length() {
        if let Some(f) = list.get(i) {
            if let Ok(v) = wasm_bindgen_futures::JsFuture::from(f.text()).await {
                if let Some(text) = v.as_string() {
                    out.push((f.name(), text));
                }
            }
        }
    }
    out
}

#[component]
fn UploadCard(#[prop(into)] on_done: Callback<()>) -> impl IntoView {
    let i18n = use_i18n();
    let toasts = use_toasts();
    let results = RwSignal::new(Vec::<UploadedFile>::new());
    let modules = RwSignal::new(Vec::<MibModule>::new());
    let busy = RwSignal::new(false);
    let over = RwSignal::new(false);
    let input = NodeRef::<html::Input>::new();

    let upload = move |files: Vec<(String, String)>| {
        if files.is_empty() {
            return;
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            let sized: Vec<((String, String), usize)> = files
                .into_iter()
                .map(|f| {
                    let n = f.1.len();
                    (f, n)
                })
                .collect();
            let mut all_files = Vec::new();
            let mut all_modules = Vec::new();
            for batch in super::model::batches(&sized) {
                let req: Vec<api::MibFileIn> = batch
                    .into_iter()
                    .map(|(name, content)| api::MibFileIn { name, content })
                    .collect();
                match api::upload(&req).await {
                    Ok(r) => {
                        all_files.extend(r.files);
                        all_modules.extend(r.modules);
                    }
                    Err(e) => toasts.error(e.message(i18n)),
                }
            }
            let stored = all_files.iter().filter(|f| f.stored).count();
            toasts.ok(i18n.ta("ui-snmp-mib-uploaded", &[("count", stored.to_string())]));
            results.set(all_files);
            modules.set(all_modules);
            busy.set(false);
            on_done.run(());
        });
    };
    let on_pick = move |_| {
        #[cfg(target_arch = "wasm32")]
        if let Some(el) = input.get() {
            if let Some(list) = el.files() {
                leptos::task::spawn_local(async move {
                    upload(read_files(list).await);
                    el.set_value("");
                });
            }
        }
    };
    let on_drop = move |ev: leptos::ev::DragEvent| {
        ev.prevent_default();
        over.set(false);
        #[cfg(target_arch = "wasm32")]
        if let Some(list) = ev.data_transfer().and_then(|d| d.files()) {
            leptos::task::spawn_local(async move { upload(read_files(list).await) });
        }
    };
    let _ = &upload;
    view! {
        <section class=CARD aria-labelledby="snmp-mib-up-h">
            <h2 id="snmp-mib-up-h" class="mb-3 text-base font-semibold">{tr("ui-snmp-mib-upload")}</h2>
            <div
                class=move || if over.get() {
                    "rounded-lg border-2 border-dashed border-sky-500 bg-sky-50 p-6 text-center dark:bg-sky-500/10"
                } else {
                    "rounded-lg border-2 border-dashed border-slate-300 p-6 text-center dark:border-slate-700"
                }
                on:dragover=move |ev: leptos::ev::DragEvent| { ev.prevent_default(); over.set(true); }
                on:dragleave=move |_| over.set(false)
                on:drop=on_drop
            >
                <Icon name=IconName::Upload class="mx-auto h-8 w-8 text-slate-400" />
                <p class="mt-2 text-sm">{tr("ui-snmp-mib-drop")}</p>
                <label class=format!("{BTN} mt-3 cursor-pointer")>
                    {tr("ui-snmp-mib-choose")}
                    <input
                        node_ref=input
                        type="file"
                        multiple=true
                        accept=".mib,.txt,.my,.smi,.asn,.asn1"
                        class="sr-only"
                        on:change=on_pick
                    />
                </label>
                <p class="mt-2 text-xs text-slate-500">{tr("ui-snmp-mib-upload-hint")}</p>
            </div>
            <Show when=move || busy.get()><Loading /></Show>
            <ul class="mt-3 space-y-1 text-sm" aria-live="polite">
                {move || results.get().into_iter().map(|f| {
                    let mods = modules.get();
                    let problems: Vec<String> = mods.iter()
                        .filter(|m| f.modules.contains(&m.name) && !m.missing_imports.is_empty())
                        .map(|m| i18n.ta("ui-snmp-mib-missing", &[("what", m.name.clone()), ("list", m.missing_imports.join(", "))]))
                        .collect();
                    view! {
                        <li class=if f.stored { "" } else { "text-rose-700 dark:text-rose-300" }>
                            <span class="font-mono">{f.file.clone()}</span>
                            ": "
                            {if f.stored { f.modules.join(", ") } else { i18n.t("ui-snmp-mib-no-module") }}
                            {f.errors.iter().map(|e| view! { <span class="block text-xs">{e.clone()}</span> }).collect_view()}
                            {problems.into_iter().map(|p| view! { <span class="block text-xs text-amber-700 dark:text-amber-400">{p}</span> }).collect_view()}
                        </li>
                    }
                }).collect_view()}
            </ul>
        </section>
    }
}

fn status_class(s: &str) -> &'static str {
    match s {
        "ok" => "rounded-full bg-emerald-100 px-2 py-0.5 text-xs text-emerald-800 dark:bg-emerald-500/15 dark:text-emerald-300",
        "partial" => "rounded-full bg-amber-100 px-2 py-0.5 text-xs text-amber-800 dark:bg-amber-500/15 dark:text-amber-300",
        _ => "rounded-full bg-rose-100 px-2 py-0.5 text-xs text-rose-800 dark:bg-rose-500/15 dark:text-rose-300",
    }
}

fn status_key(s: &str) -> &'static str {
    match s {
        "ok" => "ui-snmp-mib-ok",
        "partial" => "ui-snmp-mib-partial",
        _ => "ui-snmp-mib-error",
    }
}

#[component]
fn ModuleList(reload: RwSignal<u32>) -> impl IntoView {
    let i18n = use_i18n();
    let ctx = use_snmp();
    let toasts = use_toasts();
    let show_bundled = RwSignal::new(false);
    let deleting = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);
    let list = LocalResource::new(move || {
        reload.track();
        api::mibs()
    });
    let delete = move || {
        let Some(name) = deleting.get_untracked() else {
            return;
        };
        busy.set(true);
        leptos::task::spawn_local(async move {
            match api::delete_mib(&name).await {
                Ok(()) => {
                    toasts.ok(i18n.ta("ui-snmp-mib-deleted", &[("what", name)]));
                    reload.update(|r| *r += 1);
                }
                Err(e) => toasts.error(e.message(i18n)),
            }
            busy.set(false);
            deleting.set(None);
        });
    };
    view! {
        <section aria-labelledby="snmp-mods-h">
            <div class="mb-2 flex flex-wrap items-center gap-3">
                <h2 id="snmp-mods-h" class="text-base font-semibold">{tr("ui-snmp-mib-modules")}</h2>
                <label class="flex items-center gap-1.5 text-sm">
                    <input type="checkbox" prop:checked=move || show_bundled.get() on:change=move |ev| show_bundled.set(event_target_checked(&ev)) />
                    {tr("ui-snmp-mib-show-bundled")}
                </label>
            </div>
            {move || match list.get() {
                None => view! { <Loading /> }.into_any(),
                Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                Some(Ok(l)) => {
                    let all = show_bundled.get();
                    let rows: Vec<MibModule> = l.into_iter().filter(|m| all || !m.bundled).collect();
                    if rows.is_empty() {
                        return view! { <Empty text=tr("ui-snmp-mib-none") /> }.into_any();
                    }
                    view! {
                        <div class="overflow-x-auto rounded-lg border border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
                            <table class="min-w-full divide-y divide-slate-200 dark:divide-slate-800">
                                <caption class="sr-only">{tr("ui-snmp-mib-modules")}</caption>
                                <thead>
                                    <tr>
                                        <th scope="col" class=TH>{tr("ui-snmp-mib-module")}</th>
                                        <th scope="col" class=TH>{tr("ui-snmp-mib-source")}</th>
                                        <th scope="col" class=TH>{tr("ui-snmp-mib-status")}</th>
                                        <th scope="col" class=TH>{tr("ui-snmp-mib-problems")}</th>
                                        <th scope="col" class=TH><span class="sr-only">{tr("ui-rx-actions")}</span></th>
                                    </tr>
                                </thead>
                                <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                                    {rows.into_iter().map(|m| {
                                        let name = m.name.clone();
                                        let tree_name = m.name.clone();
                                        let st = m.status.clone();
                                        let nodes = m.nodes;
                                        view! {
                                            <tr class="align-top">
                                                <td class=format!("{TD} font-medium")>
                                                    {m.name.clone()}
                                                    <span class="block text-xs text-slate-500">{move || i18n.ta("ui-snmp-mib-nodes", &[("count", nodes.to_string())])}</span>
                                                </td>
                                                <td class=format!("{TD} text-xs")>
                                                    {m.file.clone().unwrap_or_else(|| i18n.t("ui-snmp-mib-bundled"))}
                                                </td>
                                                <td class=TD><span class=status_class(&st)>{let k = status_key(&st); move || i18n.t(k)}</span></td>
                                                <td class=format!("{TD} text-xs")>
                                                    {(!m.missing_imports.is_empty()).then(|| {
                                                        let list = m.missing_imports.join(", ");
                                                        view! { <p class="text-amber-700 dark:text-amber-400">{move || i18n.ta("ui-snmp-mib-missing-short", &[("list", list.clone())])}</p> }
                                                    })}
                                                    {(!m.errors.is_empty()).then(|| {
                                                        let n = m.errors.len();
                                                        view! {
                                                            <details>
                                                                <summary class="cursor-pointer">{move || i18n.ta("ui-snmp-mib-error-count", &[("count", n.to_string())])}</summary>
                                                                <ul class="mt-1 list-disc pl-4">{m.errors.iter().map(|e| view! { <li class="break-all">{e.clone()}</li> }).collect_view()}</ul>
                                                            </details>
                                                        }
                                                    })}
                                                </td>
                                                <td class=format!("{TD} whitespace-nowrap text-right")>
                                                    <button type="button" class=BTN_GHOST
                                                        title=tr("ui-snmp-mib-show-tree")
                                                        aria-label=move || i18n.ta("ui-snmp-mib-show-tree-named", &[("what", tree_name.clone())])
                                                        on:click={
                                                            let n = m.name.clone();
                                                            move |_| {
                                                                ctx.mib_module.set(Some(n.clone()));
                                                                crate::dom::focus_id(SCOPE_ID);
                                                            }
                                                        }>
                                                        <Icon name=IconName::Search class="h-4 w-4" />
                                                    </button>
                                                    {(!m.bundled).then(|| view! {
                                                        <button type="button" class=BTN_GHOST
                                                            title=tr("ui-delete")
                                                            aria-label=move || i18n.ta("ui-snmp-mib-delete-named", &[("what", name.clone())])
                                                            on:click={
                                                                let n = m.name.clone();
                                                                move |_| deleting.set(Some(n.clone()))
                                                            }>
                                                            <Icon name=IconName::Trash class="h-4 w-4" />
                                                        </button>
                                                    })}
                                                </td>
                                            </tr>
                                        }
                                    }).collect_view()}
                                </tbody>
                            </table>
                        </div>
                    }.into_any()
                }
            }}
            <Show when=move || deleting.with(Option::is_some)>
                <Confirm
                    title=tr("ui-snmp-mib-delete-title")
                    confirm_label=tr("ui-delete")
                    danger=true
                    busy=busy
                    on_confirm=move || delete()
                    on_cancel=move || deleting.set(None)
                >
                    <p>{tr("ui-snmp-mib-delete-body")}</p>
                    <p class="font-mono text-xs">{move || deleting.get()}</p>
                </Confirm>
            </Show>
        </section>
    }
}

/// Id of the module chooser (the module list's "show in tree" focuses it).
const SCOPE_ID: &str = "snmp-mib-scope";

#[component]
fn BrowserCard(selected: RwSignal<Option<String>>, reload: RwSignal<u32>) -> impl IntoView {
    let ctx = use_snmp();
    let query = RwSignal::new(String::new());
    let hits = super::ops::use_suggestions(query);
    view! {
        <section class=CARD aria-labelledby="snmp-tree-h">
            <h2 id="snmp-tree-h" class="mb-3 text-base font-semibold">{tr("ui-snmp-mib-browser")}</h2>
            <div class="mb-3">
                <ModuleSelect id=SCOPE_ID module=ctx.mib_module reload=reload />
            </div>
            <input class=INPUT type="search" placeholder=tr("ui-snmp-mib-search") aria-label=tr("ui-snmp-mib-search")
                prop:value=move || query.get() on:input=move |ev| query.set(event_target_value(&ev)) />
            <ul class="mt-1 max-h-48 overflow-y-auto text-sm">
                {move || hits.get().into_iter().map(|(_, h)| {
                    let id = format!("{}::{}", h.module, h.name);
                    let id2 = id.clone();
                    view! {
                        <li>
                            <button type="button" class="w-full rounded px-2 py-1 text-left hover:bg-slate-100 dark:hover:bg-slate-800"
                                on:click=move |_| selected.set(Some(id2.clone()))>
                                <span class="font-medium">{h.name.clone()}</span>
                                <span class="ml-2 text-xs text-slate-500">{format!("{} {}", h.module, h.oid)}</span>
                            </button>
                        </li>
                    }
                }).collect_view()}
            </ul>
            <div class="mt-3 max-h-[32rem] overflow-auto">
                <MibTree module=ctx.mib_module selected=selected labelledby="snmp-tree-h" reload=reload />
            </div>
        </section>
    }
}

#[component]
fn NodeDetail(selected: RwSignal<Option<String>>) -> impl IntoView {
    let node = LocalResource::new(move || {
        let id = selected.get();
        async move {
            match id {
                Some(id) => api::node(&id).await.map(Some),
                None => Ok(None),
            }
        }
    });
    view! {
        <section class=CARD aria-labelledby="snmp-node-h" aria-live="polite">
            <h2 id="snmp-node-h" class="mb-3 text-base font-semibold">{tr("ui-snmp-mib-node")}</h2>
            {move || match node.get() {
                None => view! { <Loading /> }.into_any(),
                Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                Some(Ok(None)) => view! { <Empty text=tr("ui-snmp-mib-pick") /> }.into_any(),
                Some(Ok(Some(n))) => view! { <NodeBody node=n /> }.into_any(),
            }}
        </section>
    }
}

#[component]
fn NodeBody(node: NodeInfo) -> impl IntoView {
    let i18n = use_i18n();
    let ctx = use_snmp();
    // `MODULE::name.0` for a scalar (Get works), `MODULE::name` otherwise.
    let use_name = request_oid(
        &node.oid,
        Some(&node.name),
        Some(&node.module),
        node.role.as_deref(),
    );
    let base = format!("{}::{}", node.module, node.name);
    let scalar = node.role.as_deref() == Some("scalar");
    // Fills the operations tab's OID field and runs `op` there.
    let run = move |op: Op, oid: String| {
        ctx.oids.set(oid);
        ctx.run.set(Some(op));
        ctx.tab.set(Tab::Ops);
    };
    let run_button = move |op: Op, key: &'static str, oid: String| {
        view! {
            <button type="button" class=BTN on:click=move |_| run(op, oid.clone())>
                {move || i18n.t(key)}
            </button>
        }
    };
    let row = |k: &'static str, v: Option<String>| {
        v.filter(|s| !s.is_empty()).map(|v| {
            view! {
                <dt class="text-slate-500">{tr(k)}</dt>
                <dd class="break-all">{v}</dd>
            }
        })
    };
    let path = node
        .path
        .iter()
        .map(|p| p.name.clone())
        .collect::<Vec<_>>()
        .join(".");
    view! {
        <div class="space-y-3">
            <div class="flex flex-wrap items-center gap-2">
                <span class="text-lg font-semibold">{node.name.clone()}</span>
                <span class="text-xs text-slate-500">{node.module.clone()}</span>
                <button type="button" class=format!("{BTN} ml-auto") on:click={
                    let n = use_name.clone();
                    move |_| { ctx.oids.set(n.clone()); ctx.tab.set(Tab::Ops); }
                }>
                    {tr("ui-snmp-mib-use")}
                </button>
            </div>
            <div class="flex flex-wrap gap-2" role="group" aria-label=tr("ui-snmp-mib-run")>
                {scalar.then(|| run_button(Op::Get, "ui-snmp-op-get", use_name.clone()))}
                {run_button(Op::GetNext, "ui-snmp-op-getnext", base.clone())}
                {run_button(Op::Walk, "ui-snmp-op-walk", base.clone())}
            </div>
            <p class="font-mono text-xs break-all text-slate-500">{format!("{} ({path})", node.oid)}</p>
            <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
                {row("ui-snmp-mib-kind", Some(node.role.clone().map_or_else(|| node.kind.clone(), |r| format!("{} · {r}", node.kind))))}
                {row("ui-snmp-mib-syntax", node.syntax.clone())}
                {row("ui-snmp-mib-hint", node.display_hint.clone())}
                {row("ui-snmp-mib-access", node.access.clone())}
                {row("ui-snmp-mib-status-label", node.status.clone())}
                {row("ui-snmp-mib-units", node.units.clone())}
                {row("ui-snmp-mib-index", (!node.index.is_empty()).then(|| node.index.join(", ")))}
                {row("ui-snmp-mib-augments", node.augments.clone())}
                {row("ui-snmp-mib-objects", (!node.objects.is_empty()).then(|| node.objects.join(", ")))}
                {row("ui-snmp-mib-defval", node.defval.clone())}
                {row("ui-snmp-mib-reference", node.reference.clone())}
            </dl>
            {(!node.enums.is_empty()).then(|| view! {
                <div>
                    <p class="text-xs font-medium text-slate-500">{tr("ui-snmp-mib-enums")}</p>
                    <ul class="mt-1 flex flex-wrap gap-1 text-xs">
                        {node.enums.iter().map(|e| view! {
                            <li class="rounded bg-slate-100 px-1.5 py-0.5 font-mono dark:bg-slate-800">{format!("{}({})", e.label, e.value)}</li>
                        }).collect_view()}
                    </ul>
                </div>
            })}
            {node.description.clone().map(|d| view! {
                <div>
                    <p class="text-xs font-medium text-slate-500">{tr("ui-snmp-mib-description")}</p>
                    <p class="mt-1 whitespace-pre-wrap text-sm">{d}</p>
                </div>
            })}
            <p class="sr-only">{move || i18n.t("ui-snmp-mib-node")}</p>
        </div>
    }
}
