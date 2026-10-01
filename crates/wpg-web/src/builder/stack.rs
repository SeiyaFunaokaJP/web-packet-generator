//! Layer stack panel (TODO §2.5): add / remove / reorder / duplicate layers by
//! drag & drop or keyboard, select a layer to drive the field editor and hex/tree
//! highlight, and next-layer suggestions from the binding table (with a one-click
//! "fix parent next-protocol").

use leptos::prelude::*;
use wpg_template::{FieldInput, Template};

use super::dnd::{drop_target, Drag, DragCtx};
use super::proto;
use super::state::{edit, Builder};
use crate::components::{Icon, IconName, BTN_GHOST};
use crate::i18n::{tr, use_i18n};

/// A one-line summary of a layer's set fields / generators / auto fields.
fn layer_summary(t: &Template, layer: usize) -> String {
    let Some(l) = t.layers.get(layer) else {
        return String::new();
    };
    let manual = l.fields.len();
    let gens = l.generators.len();
    let auto = proto::def(&l.protocol).map_or(0, |d| {
        d.fields
            .iter()
            .filter(|f| f.auto.is_some() && !l.fields.contains_key(&f.name))
            .count()
    });
    let mut parts = Vec::new();
    if manual > 0 {
        parts.push(format!("{manual} set"));
    }
    if gens > 0 {
        parts.push(format!("{gens} gen"));
    }
    if auto > 0 {
        parts.push(format!("{auto} auto"));
    }
    parts.join(" · ")
}

#[component]
pub(crate) fn Stack() -> impl IntoView {
    let i18n = use_i18n();
    let builder = Builder::use_ctx();
    let dnd = DragCtx::use_ctx();

    let rows = move || {
        let t = builder.template.get();
        let sel = builder.sel_layer.get();
        let over = dnd.over.get();
        let len = t.layers.len();
        (0..len)
            .map(|i| {
                let proto_id = t.layers[i].protocol.clone();
                let name = proto::def(&proto_id).map_or_else(|| proto_id.clone(), |d| d.name.clone());
                let summary = layer_summary(&t, i);
                let selected = sel == Some(i);
                let unknown = proto::def(&proto_id).is_none();
                // Warn when this layer is not announced by its parent's binding.
                let mismatch = i > 0 && {
                    let parent = t.layers[i - 1].protocol.clone();
                    !proto::binding_ok(&parent, &proto_id)
                };
                let parent_id = (i > 0).then(|| t.layers[i - 1].protocol.clone());
                let child_id = proto_id.clone();

                let row_class = move || {
                    let base = "flex w-full items-start gap-2 rounded-md border px-2.5 py-2 text-left focus:outline-none focus:ring-2 focus:ring-sky-500";
                    if selected {
                        format!("{base} border-sky-500 bg-sky-50 dark:border-sky-500 dark:bg-sky-500/10")
                    } else {
                        format!("{base} border-slate-200 bg-white hover:border-slate-300 dark:border-slate-700 dark:bg-slate-800")
                    }
                };

                let drop_slot = move |slot: usize, ev: leptos::ev::DragEvent| {
                    ev.prevent_default();
                    if let Some(d) = dnd.dragging.get_untracked() {
                        match d {
                            Drag::New(id) => {
                                builder.edit(false, |t| { edit::add_layer(t, &id, Some(slot)); });
                                builder.select_layer(Some(slot));
                                dnd.say(i18n.ta("ui-bld-added", &[("id", id)]));
                            }
                            Drag::Move(from) => {
                                if let Some(to) = drop_target(&Drag::Move(from), slot, len) {
                                    builder.edit(false, |t| edit::move_layer(t, from, to));
                                    builder.select_layer(Some(to));
                                    dnd.say(i18n.ta("ui-bld-moved", &[("index", (to + 1).to_string())]));
                                }
                            }
                        }
                    }
                    dnd.clear();
                };

                let indicator = move |slot: usize| {
                    let show = move || over.map(|o| o == slot).unwrap_or(false) && dnd.dragging.with(Option::is_some);
                    view! {
                        <div
                            class=move || if show() { "h-1.5 rounded bg-sky-500" } else { "h-1.5" }
                            on:dragover=move |ev: leptos::ev::DragEvent| { ev.prevent_default(); dnd.over.set(Some(slot)); }
                            on:drop=move |ev| drop_slot(slot, ev)
                        ></div>
                    }
                };

                view! {
                    <li>
                        {indicator(i)}
                        <div
                            role="button"
                            tabindex="0"
                            aria-pressed=selected.to_string()
                            draggable="true"
                            class=row_class
                            on:click=move |_| builder.select_layer(Some(i))
                            on:dragstart=move |_| dnd.dragging.set(Some(Drag::Move(i)))
                            on:dragend=move |_| dnd.clear()
                            on:dragover=move |ev: leptos::ev::DragEvent| { ev.prevent_default(); dnd.over.set(Some(i + 1)); }
                            on:keydown=move |ev| {
                                match (ev.alt_key(), ev.key().as_str()) {
                                    (true, "ArrowUp") => { ev.prevent_default(); if i > 0 { builder.edit(false, |t| edit::move_layer(t, i, i - 1)); builder.select_layer(Some(i - 1)); dnd.say(i18n.ta("ui-bld-moved", &[("index", i.to_string())])); } }
                                    (true, "ArrowDown") => { ev.prevent_default(); builder.edit(false, |t| edit::move_layer(t, i, i + 1)); builder.select_layer(Some((i + 1).min(len - 1))); dnd.say(i18n.ta("ui-bld-moved", &[("index", (i + 2).to_string())])); }
                                    (_, "Delete" | "Backspace") => { ev.prevent_default(); builder.edit(false, |t| edit::remove_layer(t, i)); builder.select_layer(None); dnd.say(i18n.t("ui-bld-removed")); }
                                    (_, "Enter" | " ") => { ev.prevent_default(); builder.select_layer(Some(i)); }
                                    _ => {}
                                }
                            }
                        >
                            <span class="mt-0.5 grid h-6 w-6 shrink-0 place-items-center rounded bg-slate-200 text-[11px] font-bold text-slate-600 dark:bg-slate-700 dark:text-slate-300">{i + 1}</span>
                            <div class="min-w-0 flex-1">
                                <p class="truncate text-sm font-medium">
                                    {name}
                                    {unknown.then(|| view! { <span class="ml-1 text-xs text-rose-600">{tr("ui-bld-unknown")}</span> })}
                                </p>
                                <p class="truncate font-mono text-[11px] text-slate-500">{proto_id.clone()}</p>
                                {(!summary.is_empty()).then(|| view! { <p class="truncate text-[11px] text-slate-400">{summary}</p> })}
                                {mismatch.then(|| {
                                    let parent = parent_id.clone().unwrap_or_default();
                                    let child = child_id.clone();
                                    let fixable = proto::binding_fix(&parent, &child).is_some();
                                    view! {
                                        <div class="mt-1 flex items-center gap-1 text-[11px] text-amber-600">
                                            <Icon name=IconName::Warning class="h-3 w-3" />
                                            <span>{tr("ui-bld-binding-warn")}</span>
                                            {fixable.then(|| {
                                                let parent = parent.clone(); let child = child.clone();
                                                view! {
                                                    <button type="button" class="ml-1 rounded border border-amber-400 px-1 text-amber-700 hover:bg-amber-50"
                                                        on:click=move |ev| {
                                                            ev.stop_propagation();
                                                            if let Some((field, value)) = proto::binding_fix(&parent, &child) {
                                                                builder.edit(false, |t| edit::set_field(t, i - 1, &field, FieldInput::Uint(value)));
                                                            }
                                                        }
                                                    >{tr("ui-bld-fix")}</button>
                                                }
                                            })}
                                        </div>
                                    }
                                })}
                            </div>
                            <div class="flex shrink-0 flex-col gap-0.5" on:click=|ev| ev.stop_propagation()>
                                <button type="button" class=BTN_GHOST aria-label=tr("ui-bld-dup") title=tr("ui-bld-dup")
                                    on:click=move |_| { builder.edit(false, |t| { edit::duplicate_layer(t, i); }); builder.select_layer(Some(i + 1)); }
                                ><Icon name=IconName::Copy class="h-4 w-4" /></button>
                                <button type="button" class=format!("{BTN_GHOST} hover:text-rose-600") aria-label=tr("ui-delete") title=tr("ui-delete")
                                    on:click=move |_| { builder.edit(false, |t| edit::remove_layer(t, i)); builder.select_layer(None); }
                                ><Icon name=IconName::Trash class="h-4 w-4" /></button>
                            </div>
                        </div>
                        {(i + 1 == len).then(|| indicator(len))}
                    </li>
                }
            })
            .collect_view()
    };

    // Suggested next layer (from the last layer's binding) and previous layer (a
    // protocol whose binding can carry the first layer), from the binding table.
    let suggestions = move || {
        let t = builder.template.get();
        let Some(last) = t.layers.last() else {
            return ().into_any();
        };
        let first = t
            .layers
            .first()
            .map(|l| l.protocol.clone())
            .unwrap_or_default();
        let next = proto::next_protocols(&last.protocol);
        let prev = proto::parent_protocols(&first);
        if next.is_empty() && prev.is_empty() {
            return ().into_any();
        }
        let chip = move |id: String, at: Option<usize>| {
            let name = proto::def(&id).map_or_else(|| id.clone(), |d| d.name.clone());
            view! {
                <button type="button" class="rounded border border-slate-300 px-1.5 py-0.5 text-xs hover:border-sky-500 hover:bg-sky-50 dark:border-slate-700"
                    on:click=move |_| {
                        builder.edit(false, |t| { edit::add_layer(t, &id, at); });
                        let n = at.unwrap_or_else(|| builder.template.with_untracked(|t| t.layers.len().saturating_sub(1)));
                        builder.select_layer(Some(n));
                    }
                >{"+ "}{name}</button>
            }
        };
        view! {
            <div class="mt-3 space-y-2 border-t border-slate-200 pt-2 dark:border-slate-800">
                {(!next.is_empty()).then(|| view! {
                    <div>
                        <p class="mb-1 text-[11px] font-semibold text-slate-500">{tr("ui-bld-suggest")}</p>
                        <div class="flex flex-wrap gap-1">{next.into_iter().take(8).map(|id| chip(id, None)).collect_view()}</div>
                    </div>
                })}
                {(!prev.is_empty()).then(|| view! {
                    <div>
                        <p class="mb-1 text-[11px] font-semibold text-slate-500">{tr("ui-bld-suggest-prev")}</p>
                        <div class="flex flex-wrap gap-1">{prev.into_iter().take(8).map(|id| chip(id, Some(0))).collect_view()}</div>
                    </div>
                })}
            </div>
        }.into_any()
    };

    view! {
        <section aria-label=tr("ui-bld-stack") class="flex h-full min-h-0 flex-col">
            <h2 class="mb-2 text-sm font-semibold">{tr("ui-bld-stack")}</h2>
            <div class="min-h-0 flex-1 overflow-y-auto">
                <Show
                    when=move || builder.template.with(|t| !t.layers.is_empty())
                    fallback=move || view! {
                        <div class="rounded-md border border-dashed border-slate-300 px-3 py-8 text-center text-sm text-slate-500 dark:border-slate-700"
                            on:dragover=move |ev: leptos::ev::DragEvent| { ev.prevent_default(); dnd.over.set(Some(0)); }
                            on:drop=move |ev: leptos::ev::DragEvent| {
                                ev.prevent_default();
                                if let Some(Drag::New(id)) = dnd.dragging.get_untracked() {
                                    builder.edit(false, |t| { edit::add_layer(t, &id, None); });
                                    builder.select_layer(Some(0));
                                }
                                dnd.clear();
                            }
                        >{tr("ui-bld-empty")}</div>
                    }
                >
                    <ul class="space-y-0.5">{rows}</ul>
                </Show>
                {suggestions}
            </div>
        </section>
    }
}
