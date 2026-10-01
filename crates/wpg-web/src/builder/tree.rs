//! Protocol tree from the built layout (TODO §2.5): layer → field (value, Auto
//! marker, bit offset / length), expandable and bidirectionally linked with the hex
//! view and the field editor. Builder diagnostics are shown inline below the tree.

use leptos::prelude::*;
use wpg_core::notation::format_value;
use wpg_core::{DiagLevel, FieldLayout};

use super::preview::PreviewCtx;
use super::proto;
use super::state::Builder;
use crate::i18n::{tr, use_i18n};

/// A short display of a field's value.
fn field_text(protocol: &str, f: &FieldLayout) -> String {
    if !f.children.is_empty() {
        return format!("{} item(s)", f.children.len());
    }
    proto::field_type(protocol, &f.name).map_or_else(
        || format!("{:?}", f.value),
        |ty| format_value(&f.value, &ty),
    )
}

#[component]
pub(crate) fn TreeView() -> impl IntoView {
    let i18n = use_i18n();
    let builder = Builder::use_ctx();
    let pv = PreviewCtx::use_ctx();

    let tree = move || {
        let p = pv.preview.get();
        if p.layout.is_empty() {
            return view! { <p class="p-2 text-xs text-slate-500">{tr("ui-bld-no-bytes")}</p> }
                .into_any();
        }
        let sel = builder.range.get();
        view! {
            <ul class="space-y-1 font-mono text-xs">
                {p.layout.clone().into_iter().enumerate().map(|(li, layer)| {
                    let proto = layer.protocol.clone();
                    let name = proto::def(&proto).map_or_else(|| proto.clone(), |d| d.name.clone());
                    view! {
                        <li>
                            <button type="button" class="font-sans text-sm font-medium hover:text-sky-700"
                                on:click=move |_| builder.select_layer(Some(li))
                            >{name} <span class="font-mono text-[10px] text-slate-400">{proto.clone()}</span></button>
                            <ul class="mt-0.5 space-y-px border-l border-slate-200 pl-2 dark:border-slate-700">
                                {layer.fields.iter().map(|f| field_node(builder, &proto, li, f, sel)).collect_view()}
                            </ul>
                        </li>
                    }
                }).collect_view()}
            </ul>
        }.into_any()
    };

    let diagnostics = move || {
        let p = pv.preview.get();
        let diags = p.diagnostics.clone();
        let errs = p.errors.clone();
        (!diags.is_empty() || !errs.is_empty()).then(|| {
            view! {
                <div class="mt-2 space-y-0.5 border-t border-slate-200 pt-2 dark:border-slate-800">
                    {errs.into_iter().map(|e| view! {
                        <p class="flex gap-1.5 text-[11px] text-rose-600" role="alert">
                            <span class="font-mono text-slate-400">{e.path}</span>
                            <span>{e.message}</span>
                        </p>
                    }).collect_view()}
                    {diags.into_iter().map(|d| {
                        let cls = match d.level {
                            DiagLevel::Error => "text-rose-600",
                            DiagLevel::Warning => "text-amber-600",
                            DiagLevel::Info => "text-slate-500",
                        };
                        let msg = i18n.t_dyn(&d.key, &d.args);
                        let msg = if msg == d.key { d.message.clone() } else { msg };
                        view! { <p class=format!("text-[11px] {cls}")>{msg}</p> }
                    }).collect_view()}
                </div>
            }
        })
    };

    view! {
        <section aria-label=tr("ui-bld-tree") class="flex h-full min-h-0 flex-col">
            <h2 class="mb-1 text-sm font-semibold">{tr("ui-bld-tree")}</h2>
            <div class="min-h-0 flex-1 overflow-y-auto pr-1">
                {tree}
                {diagnostics}
            </div>
        </section>
    }
}

/// One field node (recurses into TLV / list children).
fn field_node(
    builder: Builder,
    protocol: &str,
    layer: usize,
    f: &FieldLayout,
    sel: Option<(usize, usize)>,
) -> AnyView {
    let range = (f.bit_offset, f.bit_len);
    let selected = sel == Some(range);
    let text = field_text(protocol, f);
    let name = f.name.clone();
    let field_name = f.name.clone();
    let proto = protocol.to_owned();
    let children = f.children.clone();
    let (bo, bl) = (f.bit_offset, f.bit_len);
    let auto = f.auto;
    let cls = if selected {
        "flex w-full items-baseline gap-1.5 rounded bg-sky-100 px-1 text-left dark:bg-sky-500/20"
    } else {
        "flex w-full items-baseline gap-1.5 rounded px-1 text-left hover:bg-slate-100 dark:hover:bg-slate-800"
    };
    view! {
        <li>
            <button type="button" class=cls
                on:click=move |_| { builder.select_layer(Some(layer)); builder.sel_field.set(Some(name.clone())); builder.range.set(Some(range)); }
            >
                <span class="text-slate-700 dark:text-slate-300">{field_name}</span>
                <span class="text-slate-500">"="</span>
                <span class="truncate text-sky-800 dark:text-sky-300">{text}</span>
                {auto.then(|| view! { <span class="rounded bg-slate-200 px-1 text-[9px] text-slate-500 dark:bg-slate-700">"auto"</span> })}
                <span class="ml-auto shrink-0 text-[9px] text-slate-400">{format!("@{bo}·{bl}")}</span>
            </button>
            {(!children.is_empty()).then(|| view! {
                <ul class="space-y-px border-l border-slate-200 pl-2 dark:border-slate-700">
                    {children.iter().map(|c| field_node(builder, &proto, layer, c, sel)).collect_view()}
                </ul>
            })}
        </li>
    }
    .into_any()
}
