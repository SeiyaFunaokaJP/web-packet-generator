//! The MIB tree shared by the MIB tab (browse, select a node for its
//! details) and the operations tab (check boxes that add OIDs to the OID
//! field), and the module chooser both use: the whole tree from 1.3.6.1, or
//! one module from its top nodes.

use leptos::prelude::*;

use super::model::{parse_oid_list, request_oid, toggle_oid};
use crate::api::snmp::{self as api, TreeChild};
use crate::components::{Empty, ErrorBox, Loading, INPUT, LABEL};
use crate::i18n::{tr, use_i18n};

/// Where the whole tree starts.
const INTERNET: &str = "1.3.6.1";

/// The "module to show" chooser: every module with objects, uploaded ones
/// first. `reload` (bumped after uploads / deletions) refreshes the list.
#[component]
pub(crate) fn ModuleSelect(
    id: &'static str,
    module: RwSignal<Option<String>>,
    #[prop(optional)] reload: Option<RwSignal<u32>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let mods = LocalResource::new(move || {
        if let Some(r) = reload {
            r.track();
        }
        api::mibs()
    });
    let options = move || {
        let list = match mods.get() {
            Some(Ok(l)) => l,
            _ => Vec::new(),
        };
        let (bundled, uploaded): (Vec<_>, Vec<_>) = list
            .into_iter()
            .filter(|m| m.nodes > 0)
            .partition(|m| m.bundled);
        let group = |key: &'static str, mods: Vec<api::MibModule>| {
            (!mods.is_empty()).then(|| {
                view! {
                    <optgroup label=move || i18n.t(key)>
                        {mods
                            .into_iter()
                            .map(|m| {
                                let name = m.name.clone();
                                let sel = name.clone();
                                let value = name.clone();
                                view! {
                                    <option value=value selected=move || module.with(|v| v.as_deref() == Some(sel.as_str()))>
                                        {name}
                                    </option>
                                }
                            })
                            .collect_view()}
                    </optgroup>
                }
            })
        };
        view! {
            <option value="" selected=move || module.with(Option::is_none)>{tr("ui-snmp-mib-scope-all")}</option>
            {group("ui-snmp-mib-scope-uploaded", uploaded)}
            {group("ui-snmp-mib-scope-bundled", bundled)}
        }
    };
    view! {
        <label for=id class=LABEL>{tr("ui-snmp-mib-scope")}</label>
        <select id=id class=INPUT
            prop:value=move || module.get().unwrap_or_default()
            on:change=move |ev| {
                let v = event_target_value(&ev);
                module.set((!v.is_empty()).then_some(v));
            }>
            {options}
        </select>
    }
}

/// The tree: `module` picks the whole tree or one module, `selected` is the
/// node shown in the details, and `picked` (the operations tab) adds check
/// boxes that put the node into that OID list.
#[component]
pub(crate) fn MibTree(
    module: RwSignal<Option<String>>,
    selected: RwSignal<Option<String>>,
    labelledby: &'static str,
    #[prop(optional)] reload: Option<RwSignal<u32>>,
    #[prop(optional)] picked: Option<RwSignal<String>>,
) -> impl IntoView {
    let roots = LocalResource::new(move || {
        if let Some(r) = reload {
            r.track();
        }
        let m = module.get();
        async move {
            match m.as_deref() {
                None => api::tree(INTERNET, None).await,
                Some(m) => api::tree("", Some(m)).await,
            }
        }
    });
    view! {
        {move || module.with(Option::is_none).then(|| view! {
            <p class="font-mono text-xs text-slate-500">"iso.org.dod.internet (1.3.6.1)"</p>
        })}
        {move || match roots.get() {
            None => view! { <Loading /> }.into_any(),
            Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
            Some(Ok(list)) if list.is_empty() => view! { <Empty text=tr("ui-snmp-mib-module-empty") /> }.into_any(),
            Some(Ok(list)) => {
                let scope = module.get_untracked();
                view! {
                    <ul role="tree" aria-labelledby=labelledby class="text-sm">
                        {list.into_iter().map(|c| tree_item(c, 0, selected, scope.clone(), picked)).collect_view()}
                    </ul>
                }
                .into_any()
            }
        }}
    }
}

/// One tree node (children fetched on first expansion, restricted to
/// `scope`). A plain function returning [`AnyView`] so it can recurse.
#[allow(clippy::needless_pass_by_value)] // consumed by the view (owned strings)
fn tree_item(
    child: TreeChild,
    depth: usize,
    selected: RwSignal<Option<String>>,
    scope: Option<String>,
    picked: Option<RwSignal<String>>,
) -> AnyView {
    let i18n = use_i18n();
    let expanded = RwSignal::new(false);
    let oid = StoredValue::new(child.oid.clone());
    let kids_scope = StoredValue::new(scope.clone());
    let kids = LocalResource::new(move || {
        let open = expanded.get();
        let o = oid.get_value();
        let m = kids_scope.get_value();
        async move {
            if open {
                api::tree(&o, m.as_deref()).await.map(Some)
            } else {
                Ok(None)
            }
        }
    });
    let label = child.name.clone().unwrap_or_else(|| child.arc.to_string());
    let role = child.role.clone().unwrap_or_default();
    let has_children = child.has_children;
    let is_sel = move || selected.with(|s| s.as_deref() == Some(oid.get_value().as_str()));
    let pad = format!("padding-left: {}rem", depth as f64 * 0.75);
    // What a check box puts into the OID list.
    let token = request_oid(
        &child.oid,
        child.name.as_deref(),
        child.module.as_deref(),
        child.role.as_deref(),
    );
    let check = picked.map(|list| {
        let t = token.clone();
        let checked = {
            let t = t.clone();
            move || list.with(|l| parse_oid_list(l).contains(&t))
        };
        let what = t.clone();
        view! {
            <input type="checkbox" class="mr-1"
                aria-label=move || i18n.ta("ui-snmp-mib-pick-named", &[("what", what.clone())])
                prop:checked=checked
                on:change=move |_| list.update(|l| *l = toggle_oid(l, &t)) />
        }
    });
    view! {
        <li role="treeitem" aria-expanded=move || has_children.then(|| expanded.get().to_string()) aria-selected=move || is_sel().to_string()>
            <div class="flex items-center gap-1" style=pad>
                {if has_children {
                    view! {
                        <button type="button" class="w-5 text-slate-500" aria-label=move || if expanded.get() { "−" } else { "+" }
                            on:click=move |_| expanded.update(|e| *e = !*e)>
                            {move || if expanded.get() { "▾" } else { "▸" }}
                        </button>
                    }.into_any()
                } else {
                    view! { <span class="w-5"></span> }.into_any()
                }}
                {check}
                <button type="button"
                    title=token.clone()
                    class=move || if is_sel() { "rounded bg-sky-100 px-1 text-left text-sky-900 dark:bg-sky-500/20 dark:text-sky-200" } else { "rounded px-1 text-left hover:bg-slate-100 dark:hover:bg-slate-800" }
                    on:click=move |_| {
                        selected.set(Some(oid.get_value()));
                        // Clicking the name also opens the branch.
                        if has_children && !expanded.get_untracked() {
                            expanded.set(true);
                        }
                    }>
                    {label}
                    <span class="ml-1 font-mono text-xs text-slate-400">{child.arc}</span>
                    {(!role.is_empty()).then(|| view! { <span class="ml-1 text-xs text-slate-500">{role.clone()}</span> })}
                </button>
            </div>
            {move || expanded.get().then(|| view! {
                <ul role="group">
                    {move || match kids.get() {
                        Some(Ok(Some(list))) => list.into_iter().map(|c| tree_item(c, depth + 1, selected, kids_scope.get_value(), picked)).collect_view().into_any(),
                        Some(Err(e)) => view! { <li><ErrorBox error=e /></li> }.into_any(),
                        _ => view! { <li class="text-xs text-slate-500">"…"</li> }.into_any(),
                    }}
                </ul>
            })}
        </li>
    }
    .into_any()
}
