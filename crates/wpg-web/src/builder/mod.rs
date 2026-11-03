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
//! | [`irregular`] | "Allow irregular packets" switch: strict by default, out-of-spec findings |
//!
//! Routes: `/builder` (new / restored draft), `/builder?template=<id>` (open).

mod dnd;
mod fields;
mod hex;
mod irregular;
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

use crate::api::ApiError;
use crate::components::{
    use_toasts, Confirm, Icon, IconName, Modal, BTN, BTN_GHOST, BTN_PRIMARY, INPUT,
};
use crate::i18n::{tr, use_i18n};
use dnd::{DndAnnounce, DragCtx};
use fields::{FieldEditor, SendEstimate};
use hex::HexView;
use irregular::Issue;
use palette::Palette;
use preview::{build_preview, Parity, PreviewCtx};
use stack::Stack;
use state::{blank_template, template_from_value, template_to_value, Builder};
use tree::TreeView;

/// Out-of-spec findings of the current template and its last build (context).
#[derive(Clone, Copy)]
pub(crate) struct Issues(pub(crate) Memo<Vec<Issue>>);

impl Issues {
    pub(crate) fn use_ctx() -> Issues {
        expect_context::<Issues>()
    }
}

/// Turns "allow irregular" on when `template` (just opened or restored) is already
/// out of spec, so opening a malformed-packet template does not flag it as broken.
fn allow_irregular_if_needed(builder: Builder, template: &wpg_template::Template) -> bool {
    if builder.allow_irregular.get_untracked() || template.layers.is_empty() {
        return false;
    }
    let found = irregular::issues(template, &build_preview(template, 0));
    if found.is_empty() {
        return false;
    }
    builder.set_irregular(true);
    true
}

/// Saves the template (an update when it has a server id and `as_new` is false,
/// otherwise a new one) and returns its id. Name and strict-mode checks are the
/// caller's.
async fn save_template(builder: Builder, as_new: bool) -> Result<String, ApiError> {
    let sent = builder.template.get_untracked();
    let value = template_to_value(&sent);
    let existing = builder.server_id.get_untracked();
    let detail = match (&existing, as_new) {
        (Some(id), false) => crate::api::update_template_json(id, &value).await,
        _ => crate::api::create_template(&value).await,
    }?;
    let id = detail.summary.id;
    builder.mark_saved(id.clone());
    // Edits made while the request was in flight are still unsaved.
    if builder.template.with_untracked(|t| *t != sent) {
        builder.dirty.set(true);
    }
    Ok(id)
}

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
    let issues = Memo::new(move |_| {
        builder
            .template
            .with(|t| pv.preview.with(|p| irregular::issues(t, p)))
    });
    provide_context(Issues(issues));
    if builder
        .template
        .with_untracked(|t| allow_irregular_if_needed(builder, t))
    {
        toasts.ok(i18n.t("ui-bld-irr-auto-on"));
    }

    // Load ?template=<id> from the server. A restored local draft stays when it
    // differs from the server copy, with a banner offering to discard it.
    if let Some(id) = id.clone() {
        leptos::task::spawn_local(async move {
            match crate::api::template(&id).await {
                Ok(d) => match template_from_value(&d.template) {
                    Ok(t) if builder.dirty.get_untracked() => builder.compare_with_server(t),
                    Ok(t) => {
                        if allow_irregular_if_needed(builder, &t) {
                            toasts.ok(i18n.t("ui-bld-irr-auto-on"));
                        }
                        builder.replace(t, Some(id));
                    }
                    Err(e) => toasts.error(i18n.ta("ui-bld-load-error", &[("error", e)])),
                },
                Err(e) => toasts.error(e.message(i18n)),
            }
        });
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
                pv.parity.set(Parity::Idle);
                if ok {
                    let value = template_to_value(&template);
                    leptos::task::spawn_local(async move {
                        let parity = match crate::api::render(&value, 1, idx).await {
                            Ok(r) => {
                                let server = r.packets.first().map(|p| p.bytes_hex.as_str());
                                if server == Some(local_hex.as_str()) {
                                    Parity::Match
                                } else {
                                    Parity::Differs
                                }
                            }
                            Err(_) => Parity::Unknown,
                        };
                        // A newer build supersedes this answer.
                        if gen.get_value() == g {
                            pv.parity.set(parity);
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

    // Narrow screens hide the side palette; "Add layer" opens it in a dialog.
    let palette_open = RwSignal::new(false);
    let discard = move |_| {
        if let Some(t) = builder.discard_to.get_untracked() {
            builder.replace(t, builder.server_id.get_untracked());
        }
    };

    view! {
        <div class="flex min-h-0 flex-col gap-3">
            <Toolbar recompute=recompute />
            {move || builder.discard_to.with(Option::is_some).then(|| view! {
                <div role="status" class="flex flex-wrap items-center gap-3 rounded-lg border border-amber-300 bg-amber-50 px-3 py-1.5 text-sm text-amber-900 dark:border-amber-500/40 dark:bg-amber-500/10 dark:text-amber-200">
                    <span>{tr("ui-bld-draft-restored")}</span>
                    <button type="button" class="font-medium underline hover:no-underline" on:click=discard>{tr("ui-bld-discard")}</button>
                </div>
            })}
            <DndAnnounce />
            <div class="grid min-h-0 gap-3 lg:grid-cols-[13rem_15rem_minmax(0,1fr)]" style="height:min(78vh,56rem)">
                <div class="min-h-0 rounded-lg border border-slate-200 bg-slate-50 p-2 max-lg:hidden dark:border-slate-800 dark:bg-slate-900/40">
                    <Palette />
                </div>
                <div class="min-h-0 rounded-lg border border-slate-200 bg-white p-3 dark:border-slate-800 dark:bg-slate-900">
                    <Stack on_add=move || palette_open.set(true) />
                </div>
                <div class="@container flex min-h-0 flex-col rounded-lg border border-slate-200 bg-white p-3 dark:border-slate-800 dark:bg-slate-900">
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
        <Show when=move || palette_open.get()>
            <Modal title=tr("ui-bld-add-layer") on_close=move || palette_open.set(false)>
                <div class="h-[60vh]"><Palette /></div>
            </Modal>
        </Show>
    }
}

/// A toolbar action that saves first and may need a template name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    /// Save (`as_new`: always create a new template).
    Save { as_new: bool },
    /// Save unsaved changes, then open the new-job form for the template.
    NewJob,
}

/// Menu item style of the toolbar's "More" menu.
const MENU_ITEM: &str = "flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm hover:bg-slate-100 disabled:opacity-50 dark:hover:bg-slate-800";

/// Top toolbar: name, undo / redo, "allow irregular", parity badge, packet stepper,
/// recompute, the "More" menu (new, save as, validate, export), save, new job, help.
#[component]
fn Toolbar(recompute: RwSignal<u32>) -> impl IntoView {
    let i18n = use_i18n();
    let toasts = use_toasts();
    let navigate = StoredValue::new_local(use_navigate());
    let builder = Builder::use_ctx();
    let pv = PreviewCtx::use_ctx();
    let issues = Issues::use_ctx().0;
    let help_open = RwSignal::new(false);
    let more_open = RwSignal::new(false);
    let confirm_new = RwSignal::new(false);
    // An action waiting for the template name (the name dialog is open).
    let pending = RwSignal::new(None::<Action>);
    let pending_name = RwSignal::new(String::new());
    // In strict mode an out-of-spec packet cannot be saved or sent.
    let blocked = move || {
        let n = issues.with_untracked(Vec::len);
        let strict = !builder.allow_irregular.get_untracked();
        if strict && n > 0 {
            toasts.error(i18n.ta("ui-bld-irr-blocked", &[("count", n.to_string())]));
            true
        } else {
            false
        }
    };
    let lint_open = RwSignal::new(None::<Vec<crate::api::LintIssue>>);
    let busy = RwSignal::new(false);

    let go = move |path: String, replace: bool| {
        navigate.with_value(|n| {
            n(
                &path,
                NavigateOptions {
                    replace,
                    ..NavigateOptions::default()
                },
            );
        });
    };

    let name = move || builder.template.with(|t| t.meta.name.clone());
    let set_name = move |ev: leptos::ev::Event| {
        let v = event_target_value(&ev);
        builder.edit(true, move |t| t.meta.name = v);
    };

    // Save / Save as / New job. New job saves unsaved changes first; any save asks
    // for a name when the template has none.
    let run = move |action: Action| {
        if blocked() {
            return;
        }
        if builder
            .template
            .with_untracked(|t| t.meta.name.trim().is_empty())
        {
            pending_name.set(String::new());
            pending.set(Some(action));
            return;
        }
        let saved_id = builder.server_id.get_untracked();
        if action == Action::NewJob && !builder.dirty.get_untracked() {
            if let Some(id) = saved_id {
                go(format!("/jobs?new={}", crate::api::enc(&id)), false);
                return;
            }
        }
        busy.set(true);
        let as_new = action == (Action::Save { as_new: true });
        leptos::task::spawn_local(async move {
            match save_template(builder, as_new).await {
                Ok(id) => {
                    toasts.ok(i18n.t("ui-tpl-saved"));
                    if action == Action::NewJob {
                        go(format!("/jobs?new={}", crate::api::enc(&id)), false);
                    } else if saved_id.as_deref() != Some(id.as_str()) {
                        // Keep the URL on the saved template so a reload reopens it.
                        go(format!("/builder?template={}", crate::api::enc(&id)), true);
                    }
                }
                Err(e) => toasts.error(e.message(i18n)),
            }
            busy.set(false);
        });
    };
    let confirm_name = move || {
        let v = pending_name.get_untracked().trim().to_owned();
        if v.is_empty() {
            return;
        }
        let Some(action) = pending.get_untracked() else {
            return;
        };
        pending.set(None);
        builder.edit(false, move |t| t.meta.name = v);
        run(action);
    };

    // "New": a blank template (after confirming when there are unsaved changes).
    let start_new = move || {
        confirm_new.set(false);
        builder.replace(blank_template(), None);
        go("/builder".to_owned(), false);
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

    let index = move || builder.index.get();
    let card = move || pv.preview.with(|p| p.cardinality);
    let step = move |delta: i64| {
        let n = card();
        builder.index.update(|i| {
            let next = (*i as i64 + delta).max(0) as u64;
            *i = n.map_or(next, |c| if c == 0 { 0 } else { next.min(c - 1) });
        });
    };
    let position = move || {
        i18n.ta(
            "ui-bld-index-of",
            &[
                ("index", crate::fmt::group_thousands(index() + 1)),
                ("count", preview::format_count(card())),
            ],
        )
    };

    // Only a problem is worth a badge: a mismatch, or no answer from the server.
    let parity_badge = move || {
        if builder.template.with(|t| t.layers.is_empty()) {
            return ().into_any();
        }
        match pv.parity.get() {
            Parity::Differs => view! {
                <span role="status" class="flex items-center gap-1 rounded-full bg-rose-100 px-2 py-0.5 text-xs font-medium text-rose-800 dark:bg-rose-500/15 dark:text-rose-300" title=i18n.t("ui-bld-parity-differs-hint")>
                    <Icon name=IconName::Warning class="h-3.5 w-3.5" />{tr("ui-bld-parity-differs")}
                </span>
            }.into_any(),
            Parity::Unknown => view! {
                <span class="rounded-full bg-slate-100 px-2 py-0.5 text-xs text-slate-600 dark:bg-slate-800 dark:text-slate-300">{tr("ui-bld-parity-unknown")}</span>
            }.into_any(),
            Parity::Idle | Parity::Match => ().into_any(),
        }
    };
    let parity_attr = move || match pv.parity.get() {
        Parity::Idle => "idle",
        Parity::Match => "match",
        Parity::Differs => "differs",
        Parity::Unknown => "unknown",
    };

    // "Allow irregular" switch with the count of out-of-spec findings.
    let irregular_switch = move || {
        let on = builder.allow_irregular.get();
        let n = issues.with(Vec::len);
        let (track, knob) = if on {
            ("bg-amber-500", "translate-x-4")
        } else {
            ("bg-slate-300 dark:bg-slate-600", "translate-x-0")
        };
        let badge = (n > 0).then(|| {
            let cls = if on {
                "rounded-full bg-amber-100 px-2 py-0.5 text-xs font-medium text-amber-800 dark:bg-amber-500/15 dark:text-amber-300"
            } else {
                "rounded-full bg-rose-100 px-2 py-0.5 text-xs font-medium text-rose-800 dark:bg-rose-500/15 dark:text-rose-300"
            };
            view! { <span class=cls role="status">{i18n.ta("ui-bld-irr-count", &[("count", n.to_string())])}</span> }
        });
        view! {
            <button type="button" role="switch" aria-checked=on.to_string()
                class="flex items-center gap-1.5 rounded-md px-1.5 py-1 text-xs font-medium hover:bg-slate-100 dark:hover:bg-slate-800"
                title=i18n.t("ui-bld-irr-hint")
                on:click=move |_| builder.set_irregular(!on)
            >
                <span class=format!("relative inline-block h-4 w-8 rounded-full transition-colors {track}")>
                    <span class=format!("absolute top-0.5 left-0.5 h-3 w-3 rounded-full bg-white shadow transition-transform {knob}")></span>
                </span>
                {tr("ui-bld-irr-allow")}
            </button>
            {badge}
        }
    };

    let can_undo = move || builder.history.with(state::History::can_undo);
    let can_redo = move || builder.history.with(state::History::can_redo);

    view! {
        <div class="flex flex-wrap items-center gap-2 rounded-lg border border-slate-200 bg-white px-3 py-2 dark:border-slate-800 dark:bg-slate-900" data-parity=parity_attr>
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

            <div class="flex items-center gap-1">{irregular_switch}</div>

            <div class="ml-auto flex flex-wrap items-center gap-2">
                {parity_badge}
                // Packet stepper.
                <div class="flex items-center gap-1 rounded-md border border-slate-300 px-1 dark:border-slate-700" role="group" aria-label=tr("ui-bld-index")>
                    <button type="button" class=BTN_GHOST aria-label=tr("ui-bld-index-prev") on:click=move |_| step(-1)>"‹"</button>
                    <span class="min-w-16 text-center text-xs whitespace-nowrap tabular-nums">{position}</span>
                    <button type="button" class=BTN_GHOST aria-label=tr("ui-bld-index-next") on:click=move |_| step(1)>"›"</button>
                </div>

                <label class="flex items-center gap-1 text-xs" title=i18n.t("ui-bld-auto-recompute")>
                    <input type="checkbox" prop:checked=move || builder.auto_recompute.get() on:change=move |ev| builder.auto_recompute.set(event_target_checked(&ev)) />
                    {tr("ui-bld-auto")}
                </label>
                <button type="button" class=BTN on:click=move |_| recompute.update(|r| *r += 1)>
                    <Icon name=IconName::Refresh class="h-4 w-4" />{tr("ui-bld-recompute")}
                </button>

                <div class="relative">
                    <button type="button" class=BTN aria-haspopup="menu" aria-expanded=move || more_open.get().to_string() on:click=move |_| more_open.update(|o| *o = !*o)>
                        {tr("ui-bld-more")}<span aria-hidden="true">"▾"</span>
                    </button>
                    <Show when=move || more_open.get()>
                        <div class="fixed inset-0 z-30" on:click=move |_| more_open.set(false)></div>
                        <div role="menu" aria-label=tr("ui-bld-more")
                            class="absolute right-0 z-40 mt-1 w-52 rounded-md border border-slate-200 bg-white py-1 shadow-lg dark:border-slate-700 dark:bg-slate-900"
                            on:keydown=move |ev| if ev.key() == "Escape" { more_open.set(false); }
                        >
                            <button type="button" role="menuitem" class=MENU_ITEM on:click=move |_| {
                                more_open.set(false);
                                if builder.dirty.get_untracked() { confirm_new.set(true) } else { start_new() }
                            }>
                                <Icon name=IconName::Plus class="h-4 w-4" />{tr("ui-bld-new")}
                            </button>
                            <button type="button" role="menuitem" class=MENU_ITEM disabled=move || busy.get() on:click=move |_| {
                                more_open.set(false);
                                run(Action::Save { as_new: true });
                            }>
                                <Icon name=IconName::Copy class="h-4 w-4" />{tr("ui-bld-save-as")}
                            </button>
                            <button type="button" role="menuitem" class=MENU_ITEM disabled=move || busy.get() on:click=move |_| {
                                more_open.set(false);
                                lint();
                            }>
                                <Icon name=IconName::Check class="h-4 w-4" />{tr("ui-lint-run")}
                            </button>
                            <button type="button" role="menuitem" class=MENU_ITEM on:click=move |_| {
                                more_open.set(false);
                                export();
                            }>
                                <Icon name=IconName::Download class="h-4 w-4" />{tr("ui-tpl-export")}
                            </button>
                        </div>
                    </Show>
                </div>
                <button type="button" class=BTN disabled=move || busy.get() on:click=move |_| run(Action::Save { as_new: false })>{tr("ui-save")}</button>
                <button type="button" class=BTN_PRIMARY disabled=move || busy.get() on:click=move |_| run(Action::NewJob)>
                    <Icon name=IconName::Play class="h-4 w-4" />{tr("ui-bld-new-job")}
                </button>
                <button type="button" class=BTN_GHOST aria-label=tr("ui-help") title=tr("ui-help") on:click=move |_| help_open.set(true)>
                    <Icon name=IconName::Help class="h-4 w-4" />
                </button>
            </div>
        </div>

        <Show when=move || help_open.get()>
            <BuilderHelp on_close=move || help_open.set(false) />
        </Show>
        <Show when=move || confirm_new.get()>
            <Confirm
                title=tr("ui-bld-new-confirm")
                confirm_label=tr("ui-bld-discard")
                danger=true
                on_confirm=move || start_new()
                on_cancel=move || confirm_new.set(false)
            >
                <p>{tr("ui-bld-new-confirm-body")}</p>
            </Confirm>
        </Show>
        <Show when=move || pending.get().is_some()>
            <Modal title=tr("ui-bld-name-title") on_close=move || pending.set(None) focus="bld-name-prompt".to_owned()>
                <form class="space-y-4" on:submit=move |ev| { ev.prevent_default(); confirm_name(); }>
                    <input
                        id="bld-name-prompt"
                        type="text"
                        class=INPUT
                        aria-label=tr("ui-bld-name")
                        placeholder=tr("ui-bld-name")
                        prop:value=move || pending_name.get()
                        on:input=move |ev| pending_name.set(event_target_value(&ev))
                    />
                    <div class="flex justify-end gap-2">
                        <button type="button" class=BTN on:click=move |_| pending.set(None)>{tr("ui-cancel")}</button>
                        <button type="submit" class=BTN_PRIMARY disabled=move || pending_name.with(|n| n.trim().is_empty())>{tr("ui-save")}</button>
                    </div>
                </form>
            </Modal>
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
