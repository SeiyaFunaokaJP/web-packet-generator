//! Packet builder (spec §2.5 画面構成 2, M4 phase 2).
//!
//! | module | role |
//! |---|---|
//! | [`state`] | Document model: a `wpg_template::Template` in a signal, selection, Undo/Redo, draft autosave |
//! | [`palette`] | Left: protocol palette from `GET /api/protocols` (search, favourites) |
//! | [`dnd`] | HTML5 drag & drop + keyboard equivalents |
//! | [`stack`] | Centre: layer stack, next-header suggestions |
//! | [`fields`] | Right: per-`FieldType` editor with Auto/Manual + generator settings |
//! | [`hex`] | Bottom: virtualised hex view, byte ⇄ field highlight, hex edit |
//! | [`tree`] | Bottom: field tree from the built layout |
//! | [`preview`] | In-browser build (`wpg-core` WASM), cross-checked against `POST /api/render` |
//! | [`proto`] | Catalog / binding helpers |
//!
//! Routes: `/builder` (new / restored draft), `/builder?template=<id>` (open).

mod dnd;
mod fields;
mod hex;
mod palette;
pub(crate) mod preview;
pub(crate) mod proto;
mod rate;
mod stack;
mod state;
mod tree;

use std::time::Duration;

use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_query_map};
use leptos_router::NavigateOptions;

use crate::components::{use_toasts, Icon, IconName, Modal, BTN, BTN_GHOST, BTN_PRIMARY, INPUT};
use crate::i18n::{tr, use_i18n};
use dnd::{DndAnnounce, DragCtx};
use fields::{FieldEditor, SendEstimate};
use hex::HexView;
use palette::Palette;
use preview::{build_preview, PreviewCtx};
use stack::Stack;
use state::{template_from_value, template_to_value, Builder};
use tree::TreeView;

/// `/builder`.
#[component]
pub(crate) fn BuilderPage() -> impl IntoView {
    let i18n = use_i18n();
    let toasts = use_toasts();
    let query = use_query_map();
    let id = query.with_untracked(|q| q.get("template").map(|s| s.to_string()));

    let builder = Builder::new(id.clone());
    let dnd = DragCtx::new();
    let pv = PreviewCtx::new();
    provide_context(builder);
    provide_context(dnd);
    provide_context(pv);

    // Load from the server when opening ?template=<id> and no local draft was restored.
    if let Some(id) = id.clone() {
        if builder.template.with_untracked(|t| t.layers.is_empty()) {
            leptos::task::spawn_local(async move {
                match crate::api::template(&id).await {
                    Ok(d) => {
                        if let Ok(t) = template_from_value(&d.template) {
                            builder.replace(t, Some(id));
                        }
                    }
                    Err(e) => toasts.error(e.message(i18n)),
                }
            });
        }
    }

    // Live, debounced preview (~50ms) + server parity check.
    let gen = StoredValue::new(0u32);
    let recompute = RwSignal::new(0u32);
    let run_build = move || {
        let template = builder.template.get_untracked();
        let idx = builder.index.get_untracked();
        let g = gen.get_value() + 1;
        gen.set_value(g);
        set_timeout(
            move || {
                if gen.get_value() != g {
                    return;
                }
                let p = build_preview(&template, idx);
                let local_hex = p.hex();
                let ok = p.ok();
                pv.preview.set(p);
                // Compare against the server render of the same packet.
                pv.parity.set(None);
                if ok {
                    let value = template_to_value(&template);
                    leptos::task::spawn_local(async move {
                        match crate::api::render(&value, 1, idx).await {
                            Ok(r) => {
                                let server = r.packets.first().map(|p| p.bytes_hex.clone());
                                pv.parity
                                    .set(Some(server.as_deref() == Some(local_hex.as_str())));
                            }
                            Err(_) => pv.parity.set(None),
                        }
                    });
                }
            },
            Duration::from_millis(50),
        );
    };
    Effect::new(move |_| {
        builder.template.track();
        builder.index.track();
        recompute.track();
        if builder.auto_recompute.get() || recompute.get_untracked() > 0 {
            run_build();
        }
    });

    // Undo / redo keyboard (Ctrl/⌘+Z, Ctrl+Shift+Z / Ctrl+Y).
    let handle = window_event_listener(leptos::ev::keydown, move |ev| {
        if !(ev.ctrl_key() || ev.meta_key()) {
            return;
        }
        match ev.key().as_str() {
            "z" | "Z" if ev.shift_key() => {
                ev.prevent_default();
                builder.redo();
            }
            "z" | "Z" => {
                ev.prevent_default();
                builder.undo();
            }
            "y" | "Y" => {
                ev.prevent_default();
                builder.redo();
            }
            _ => {}
        }
    });
    on_cleanup(move || handle.remove());

    view! {
        <div class="flex min-h-0 flex-col gap-3">
            <Toolbar recompute=recompute />
            <DndAnnounce />
            <div class="grid min-h-0 gap-3 lg:grid-cols-[14rem_minmax(0,1fr)_20rem]" style="height:min(62vh,40rem)">
                <div class="min-h-0 rounded-lg border border-slate-200 bg-slate-50 p-2 max-lg:hidden dark:border-slate-800 dark:bg-slate-900/40">
                    <Palette />
                </div>
                <div class="min-h-0 rounded-lg border border-slate-200 bg-white p-3 dark:border-slate-800 dark:bg-slate-900">
                    <Stack />
                </div>
                <div class="flex min-h-0 flex-col rounded-lg border border-slate-200 bg-white p-3 dark:border-slate-800 dark:bg-slate-900">
                    <div class="min-h-0 flex-1">
                        <FieldEditor />
                    </div>
                    <SendEstimate />
                </div>
            </div>
            <div class="grid min-h-0 gap-3 lg:grid-cols-2" style="min-height:22rem">
                <div class="min-h-0 rounded-lg border border-slate-200 bg-white p-3 dark:border-slate-800 dark:bg-slate-900">
                    <HexView />
                </div>
                <div class="min-h-0 rounded-lg border border-slate-200 bg-white p-3 dark:border-slate-800 dark:bg-slate-900">
                    <TreeView />
                </div>
            </div>
        </div>
    }
}

/// Top toolbar: name, save / save-as, lint, export, new job, index stepper, parity
/// badge, undo / redo, auto-recompute, help.
#[component]
fn Toolbar(recompute: RwSignal<u32>) -> impl IntoView {
    let i18n = use_i18n();
    let toasts = use_toasts();
    let navigate = use_navigate();
    let builder = Builder::use_ctx();
    let pv = PreviewCtx::use_ctx();
    let help_open = RwSignal::new(false);
    let lint_open = RwSignal::new(None::<Vec<crate::api::LintIssue>>);
    let busy = RwSignal::new(false);

    let name = move || builder.template.with(|t| t.meta.name.clone());
    let set_name = move |ev: leptos::ev::Event| {
        let v = event_target_value(&ev);
        builder.edit(true, move |t| t.meta.name = v);
    };

    // Save (create or update) / Save As (always create).
    let save = move |as_new: bool| {
        if builder
            .template
            .with_untracked(|t| t.meta.name.trim().is_empty())
        {
            toasts.error(i18n.t("ui-bld-need-name"));
            return;
        }
        busy.set(true);
        let value = builder.template.with_untracked(template_to_value);
        let existing = builder.server_id.get_untracked();
        leptos::task::spawn_local(async move {
            let result = match (&existing, as_new) {
                (Some(id), false) => crate::api::update_template_json(id, &value).await,
                _ => crate::api::create_template(&value).await,
            };
            match result {
                Ok(d) => {
                    builder.server_id.set(Some(d.summary.id.clone()));
                    builder.dirty.set(false);
                    builder.save_draft();
                    toasts.ok(i18n.t("ui-tpl-saved"));
                }
                Err(e) => toasts.error(e.message(i18n)),
            }
            busy.set(false);
        });
    };

    let lint = move || {
        busy.set(true);
        let value = builder.template.with_untracked(template_to_value);
        leptos::task::spawn_local(async move {
            match crate::api::lint_json(&value).await {
                Ok(r) => lint_open.set(Some(r.issues)),
                Err(e) => toasts.error(e.message(i18n)),
            }
            busy.set(false);
        });
    };

    let export = move || {
        let yaml = builder
            .template
            .with_untracked(|t| t.to_yaml().unwrap_or_default());
        let name = builder.template.with_untracked(|t| {
            let n = t.meta.name.trim();
            if n.is_empty() {
                "packet".to_owned()
            } else {
                n.replace(' ', "_")
            }
        });
        crate::dom::download_text(&format!("{name}.wpg.yaml"), "text/yaml", &yaml);
    };

    let new_job = move || match builder.server_id.get_untracked() {
        Some(id) => navigate(
            &format!("/jobs?new={}", crate::api::enc(&id)),
            NavigateOptions::default(),
        ),
        None => toasts.error(i18n.t("ui-bld-save-first")),
    };

    let index = move || builder.index.get();
    let card = move || pv.preview.with(|p| p.cardinality);
    let step = move |delta: i64| {
        let n = card();
        builder.index.update(|i| {
            let next = (*i as i64 + delta).max(0) as u64;
            *i = n.map_or(next, |c| if c == 0 { 0 } else { next.min(c - 1) });
        });
    };

    let parity_badge = move || {
        let ok = builder.template.with(|t| !t.layers.is_empty());
        if !ok {
            return ().into_any();
        }
        match pv.parity.get() {
            Some(true) => view! {
                <span class="flex items-center gap-1 rounded-full bg-emerald-100 px-2 py-0.5 text-xs font-medium text-emerald-800 dark:bg-emerald-500/15 dark:text-emerald-300" title=i18n.t("ui-bld-parity-ok")>
                    <Icon name=IconName::Check class="h-3.5 w-3.5" />{tr("ui-bld-parity-ok-short")}
                </span>
            }.into_any(),
            Some(false) => view! {
                <span class="flex items-center gap-1 rounded-full bg-rose-100 px-2 py-0.5 text-xs font-medium text-rose-800 dark:bg-rose-500/15 dark:text-rose-300" title=i18n.t("ui-bld-parity-bad")>
                    <Icon name=IconName::Warning class="h-3.5 w-3.5" />{tr("ui-bld-parity-bad-short")}
                </span>
            }.into_any(),
            None => view! {
                <span class="rounded-full bg-slate-100 px-2 py-0.5 text-xs text-slate-500 dark:bg-slate-800" title=i18n.t("ui-bld-parity-pending")>"…"</span>
            }.into_any(),
        }
    };

    let can_undo = move || builder.history.with(state::History::can_undo);
    let can_redo = move || builder.history.with(state::History::can_redo);

    view! {
        <div class="flex flex-wrap items-center gap-2 rounded-lg border border-slate-200 bg-white px-3 py-2 dark:border-slate-800 dark:bg-slate-900">
            <div class="w-56"><input
                type="text"
                class=INPUT
                placeholder=tr("ui-bld-name")
                aria-label=tr("ui-bld-name")
                prop:value=name
                on:input=set_name
            /></div>
            {move || builder.dirty.get().then(|| view! { <span class="text-xs text-amber-600" title=i18n.t("ui-bld-unsaved")>"●"</span> })}

            <div class="flex items-center gap-1">
                <button type="button" class=BTN_GHOST aria-label=tr("ui-bld-undo") title=format!("{} (Ctrl+Z)", i18n.t("ui-bld-undo")) disabled=move || !can_undo() on:click=move |_| builder.undo()>
                    <Icon name=IconName::Refresh class="h-4 w-4 -scale-x-100" />
                </button>
                <button type="button" class=BTN_GHOST aria-label=tr("ui-bld-redo") title=format!("{} (Ctrl+Shift+Z)", i18n.t("ui-bld-redo")) disabled=move || !can_redo() on:click=move |_| builder.redo()>
                    <Icon name=IconName::Refresh class="h-4 w-4" />
                </button>
            </div>

            <div class="ml-auto flex flex-wrap items-center gap-2">
                {parity_badge}
                // Index stepper.
                <div class="flex items-center gap-1 rounded-md border border-slate-300 px-1 dark:border-slate-700" role="group" aria-label=tr("ui-bld-index")>
                    <button type="button" class=BTN_GHOST aria-label=tr("ui-bld-index-prev") on:click=move |_| step(-1)>"‹"</button>
                    <span class="min-w-16 text-center text-xs tabular-nums" title=i18n.t("ui-bld-index")>
                        {move || format!("{} / {}", index(), preview::format_count(card()))}
                    </span>
                    <button type="button" class=BTN_GHOST aria-label=tr("ui-bld-index-next") on:click=move |_| step(1)>"›"</button>
                </div>

                <label class="flex items-center gap-1 text-xs" title=i18n.t("ui-bld-auto-recompute")>
                    <input type="checkbox" prop:checked=move || builder.auto_recompute.get() on:change=move |ev| builder.auto_recompute.set(event_target_checked(&ev)) />
                    {tr("ui-bld-auto")}
                </label>
                <button type="button" class=BTN on:click=move |_| recompute.update(|r| *r += 1)>
                    <Icon name=IconName::Refresh class="h-4 w-4" />{tr("ui-bld-recompute")}
                </button>

                <button type="button" class=BTN disabled=move || busy.get() on:click=move |_| lint()>
                    <Icon name=IconName::Check class="h-4 w-4" />{tr("ui-lint-run")}
                </button>
                <button type="button" class=BTN on:click=move |_| export()>
                    <Icon name=IconName::Download class="h-4 w-4" />{tr("ui-tpl-export")}
                </button>
                <button type="button" class=BTN on:click=move |_| new_job()>
                    <Icon name=IconName::Play class="h-4 w-4" />{tr("ui-tpl-new-job")}
                </button>
                <button type="button" class=BTN disabled=move || busy.get() on:click=move |_| save(true)>{tr("ui-bld-save-as")}</button>
                <button type="button" class=BTN_PRIMARY disabled=move || busy.get() on:click=move |_| save(false)>{tr("ui-save")}</button>
                <button type="button" class=BTN_GHOST aria-label=tr("ui-help") title=tr("ui-help") on:click=move |_| help_open.set(true)>
                    <Icon name=IconName::Help class="h-4 w-4" />
                </button>
            </div>
        </div>

        <Show when=move || help_open.get()>
            <BuilderHelp on_close=move || help_open.set(false) />
        </Show>
        {move || lint_open.get().map(|issues| view! { <LintModal issues=issues on_close=move || lint_open.set(None) /> })}
    }
}

/// The `?` help overlay for builder shortcuts.
#[component]
fn BuilderHelp(#[prop(into)] on_close: Callback<()>) -> impl IntoView {
    let i18n = use_i18n();
    let rows = [
        ("Enter", "ui-bld-help-add"),
        ("Alt + ↑ / ↓", "ui-bld-help-move"),
        ("Delete", "ui-bld-help-del"),
        ("Ctrl + Z", "ui-bld-help-undo"),
        ("Ctrl + Shift + Z", "ui-bld-help-redo"),
        ("0-9 a-f", "ui-bld-help-hex"),
        ("← → ↑ ↓", "ui-bld-help-caret"),
    ];
    view! {
        <Modal title=tr("ui-bld-help-title") on_close=on_close>
            <table class="w-full text-sm">
                <tbody>
                    {rows.into_iter().map(|(keys, key)| view! {
                        <tr class="border-b border-slate-100 last:border-0 dark:border-slate-800">
                            <td class="py-1.5 pr-4 whitespace-nowrap"><kbd class="rounded border border-slate-300 bg-slate-50 px-1.5 py-0.5 font-mono text-xs dark:border-slate-700 dark:bg-slate-800">{keys}</kbd></td>
                            <td class="py-1.5">{move || i18n.t(key)}</td>
                        </tr>
                    }).collect_view()}
                </tbody>
            </table>
        </Modal>
    }
}

/// Lint results modal.
#[component]
fn LintModal(
    issues: Vec<crate::api::LintIssue>,
    #[prop(into)] on_close: Callback<()>,
) -> impl IntoView {
    view! {
        <Modal title=tr("ui-lint-run") on_close=on_close>
            {if issues.is_empty() {
                view! { <p class="flex items-center gap-1 text-sm text-emerald-700 dark:text-emerald-400"><Icon name=IconName::Check class="h-4 w-4" />{tr("ui-lint-clean")}</p> }.into_any()
            } else {
                view! {
                    <ul class="space-y-1 text-xs">
                        {issues.into_iter().map(|i| {
                            let cls = if i.level == "error" { "text-rose-600" } else { "text-amber-600" };
                            view! {
                                <li class="flex gap-2">
                                    <span class=format!("w-14 shrink-0 font-semibold uppercase {cls}")>{i.level.clone()}</span>
                                    <code class="shrink-0 font-mono text-slate-500">{i.path.clone()}</code>
                                    <span>{i.message.clone()}</span>
                                </li>
                            }
                        }).collect_view()}
                    </ul>
                }.into_any()
            }}
        </Modal>
    }
}
