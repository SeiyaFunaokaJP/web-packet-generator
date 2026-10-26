//! Protocol palette (spec §2.5): the catalog from `GET /api/protocols`, searchable
//! and grouped by layer, with favourites. Each entry is a focusable button that adds
//! a layer on `Enter` / click and is also draggable onto the stack.

use leptos::prelude::*;

use super::dnd::{Drag, DragCtx};
use super::state::{edit, Builder};
use crate::api::{self, ProtocolSummary};
use crate::components::{Icon, IconName, INPUT};
use crate::i18n::{tr, use_i18n};

/// Layer groups in palette order.
const GROUPS: [(&str, &str); 5] = [
    ("l2", "ui-bld-layer-l2"),
    ("l3", "ui-bld-layer-l3"),
    ("l4", "ui-bld-layer-l4"),
    ("l7", "ui-bld-layer-l7"),
    ("payload", "ui-bld-layer-payload"),
];

/// Whether a protocol matches the search query (id or name, case-insensitive).
fn matches(p: &ProtocolSummary, q: &str) -> bool {
    let q = q.trim().to_lowercase();
    q.is_empty() || p.id.to_lowercase().contains(&q) || p.name.to_lowercase().contains(&q)
}

#[component]
pub(crate) fn Palette() -> impl IntoView {
    let i18n = use_i18n();
    let builder = Builder::use_ctx();
    let dnd = DragCtx::use_ctx();
    let query = RwSignal::new(String::new());

    let protocols = LocalResource::new(|| async move { api::protocols().await });

    // Appends a layer after the current selection and selects it.
    let add = move |id: String| {
        let at = builder.sel_layer.get_untracked().map(|i| i + 1);
        builder.edit(false, |t| {
            let idx = edit::add_layer(t, &id, at);
            let _ = idx;
        });
        let new = at
            .map(|a| {
                a.min(
                    builder
                        .template
                        .with_untracked(|t| t.layers.len().saturating_sub(1)),
                )
            })
            .unwrap_or_else(|| {
                builder
                    .template
                    .with_untracked(|t| t.layers.len().saturating_sub(1))
            });
        builder.select_layer(Some(new));
        dnd.say(i18n.ta("ui-bld-added", &[("id", id)]));
    };

    let item = move |p: ProtocolSummary| {
        let id = p.id.clone();
        let id_add = id.clone();
        let id_drag = id.clone();
        let id_fav = id.clone();
        let is_fav = Memo::new(move |_| builder.favorites.with(|f| f.contains(&id)));
        view! {
            <li class="flex items-center gap-1">
                <button
                    type="button"
                    draggable="true"
                    class="group flex min-w-0 flex-1 items-center gap-2 rounded-md border border-slate-200 bg-white px-2 py-1.5 text-left text-sm hover:border-sky-400 hover:bg-sky-50 focus:border-sky-500 focus:ring-1 focus:ring-sky-500 focus:outline-none dark:border-slate-700 dark:bg-slate-800 dark:hover:bg-slate-700"
                    title=p.id.clone()
                    on:click=move |_| add(id_add.clone())
                    on:dragstart=move |_| dnd.dragging.set(Some(Drag::New(id_drag.clone())))
                    on:dragend=move |_| dnd.clear()
                >
                    <span class="truncate">{p.name.clone()}</span>
                    <code class="ml-auto shrink-0 font-mono text-[10px] text-slate-400">{p.id.clone()}</code>
                </button>
                <button
                    type="button"
                    class=move || if is_fav.get() { "text-amber-500" } else { "text-slate-300 hover:text-amber-500 dark:text-slate-600" }
                    aria-pressed=move || is_fav.get().to_string()
                    aria-label=tr("ui-bld-favorite")
                    title=tr("ui-bld-favorite")
                    on:click=move |_| builder.toggle_favorite(&id_fav)
                >
                    <Icon name=IconName::Star class="h-3.5 w-3.5" filled=Signal::derive(move || is_fav.get()) />
                </button>
            </li>
        }
    };

    let groups_view = move || {
        let Some(Ok(list)) = protocols.get() else {
            return view! { <p class="px-1 text-xs text-slate-400">{tr("ui-loading")}</p> }
                .into_any();
        };
        let q = query.get();
        let favs = builder.favorites.get();
        let fav_items: Vec<ProtocolSummary> = favs
            .iter()
            .filter_map(|id| list.iter().find(|p| &p.id == id).cloned())
            .filter(|p| matches(p, &q))
            .collect();
        view! {
            <div class="space-y-3">
                {(!fav_items.is_empty()).then(|| view! {
                    <div>
                        <p class="mb-1 px-1 text-[11px] font-semibold tracking-wide text-slate-500 uppercase">{tr("ui-bld-favorites")}</p>
                        <ul class="space-y-1">{fav_items.into_iter().map(&item).collect_view()}</ul>
                    </div>
                })}
                {GROUPS.into_iter().map(|(layer, key)| {
                    let items: Vec<ProtocolSummary> = list.iter().filter(|p| p.layer == layer && matches(p, &q)).cloned().collect();
                    (!items.is_empty()).then(|| view! {
                        <div>
                            <p class="mb-1 px-1 text-[11px] font-semibold tracking-wide text-slate-500 uppercase">{move || i18n.t(key)}</p>
                            <ul class="space-y-1">{items.into_iter().map(&item).collect_view()}</ul>
                        </div>
                    })
                }).collect_view()}
            </div>
        }.into_any()
    };

    view! {
        <section aria-label=tr("ui-bld-palette") class="flex h-full min-h-0 flex-col">
            <h2 class="mb-2 text-sm font-semibold">{tr("ui-bld-palette")}</h2>
            <div class="relative mb-2">
                <span class="pointer-events-none absolute inset-y-0 left-2 flex items-center text-slate-400">
                    <Icon name=IconName::Search class="h-4 w-4" />
                </span>
                <input
                    type="search"
                    class=format!("{INPUT} pl-8")
                    placeholder=tr("ui-bld-search")
                    aria-label=tr("ui-bld-search")
                    prop:value=move || query.get()
                    on:input=move |ev| query.set(event_target_value(&ev))
                />
            </div>
            <div class="min-h-0 flex-1 overflow-y-auto pr-1">{groups_view}</div>
        </section>
    }
}
