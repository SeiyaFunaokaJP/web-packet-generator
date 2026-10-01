//! `/jobs`: live and finished jobs; `?new=1|<template id>` opens the form,
//! `?job=<id>` the detail drawer.

use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_query_map};
use leptos_router::NavigateOptions;

use crate::api::{self, JobInfo};
use crate::components::{
    Empty, ErrorBox, Icon, IconName, Loading, PageHeader, StateBadge, ViewOnlyNote, BTN_PRIMARY,
    TD, TD_NUM, TH,
};
use crate::fmt;
use crate::i18n::{tr, use_i18n};
use crate::pages::job_detail::JobDetail;
use crate::pages::job_form::JobForm;
use crate::ws::use_live;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Live,
    History,
}

#[component]
pub(crate) fn JobsPage() -> impl IntoView {
    let live = use_live();
    let query = use_query_map();
    let navigate = use_navigate();
    let tab = RwSignal::new(Tab::Live);

    let jobs = LocalResource::new(move || {
        live.jobs_rev.track();
        let t = tab.get();
        async move {
            match t {
                Tab::Live => api::jobs().await,
                Tab::History => api::job_history().await,
            }
        }
    });

    let selected = move || query.with(|q| q.get("job"));
    let new_form = move || query.with(|q| q.get("new"));
    let go = {
        let navigate = navigate.clone();
        move |to: String| navigate(&to, NavigateOptions::default())
    };
    let open_job = {
        let go = go.clone();
        Callback::new(move |id: String| go(format!("/jobs?job={}", api::enc(&id))))
    };
    let close = {
        let go = go.clone();
        Callback::new(move |()| go("/jobs".to_owned()))
    };

    let tab_button = move |t: Tab, key: &'static str| {
        view! {
            <button
                type="button"
                role="tab"
                aria-selected=move || (tab.get() == t).to_string()
                class=move || {
                    if tab.get() == t {
                        "border-b-2 border-sky-600 px-3 py-2 text-sm font-semibold text-sky-700 dark:text-sky-300"
                    } else {
                        "border-b-2 border-transparent px-3 py-2 text-sm text-slate-500 hover:text-slate-800 dark:hover:text-slate-200"
                    }
                }
                on:click=move |_| tab.set(t)
            >
                {tr(key)}
            </button>
        }
    };

    view! {
        <PageHeader title=tr("ui-nav-jobs")>
            <a href="/jobs?new=1" class=format!("{BTN_PRIMARY} max-md:hidden") aria-keyshortcuts="n">
                <Icon name=IconName::Plus class="h-4 w-4" />
                {tr("ui-jobs-new")}
            </a>
        </PageHeader>
        <ViewOnlyNote />
        <div role="tablist" aria-label=tr("ui-nav-jobs") class="mb-4 flex gap-2 border-b border-slate-200 dark:border-slate-800">
            {tab_button(Tab::Live, "ui-jobs-tab-live")}
            {tab_button(Tab::History, "ui-jobs-tab-history")}
        </div>
        {move || match jobs.get() {
            None => view! { <Loading /> }.into_any(),
            Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
            Some(Ok(list)) if list.is_empty() => view! { <Empty text=tr("ui-jobs-empty") /> }.into_any(),
            Some(Ok(list)) => view! { <JobTable list=list on_open=open_job /> }.into_any(),
        }}
        {move || {
            new_form()
                .map(|v| {
                    let initial = (v != "1" && !v.is_empty()).then_some(v);
                    view! {
                        <JobForm
                            initial_template=initial
                            on_close=close
                            on_created=move |id: String| {
                                live.jobs_rev.update(|r| *r += 1);
                                open_job.run(id);
                            }
                        />
                    }
                })
        }}
        {move || selected().map(|id| view! { <JobDetail id=id on_close=close /> })}
    }
}

#[component]
fn JobTable(list: Vec<JobInfo>, #[prop(into)] on_open: Callback<String>) -> impl IntoView {
    let i18n = use_i18n();
    let live = use_live();
    view! {
        <div class="overflow-x-auto rounded-lg border border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
            <table class="min-w-full divide-y divide-slate-200 dark:divide-slate-800">
                <caption class="sr-only">{tr("ui-nav-jobs")}</caption>
                <thead>
                    <tr>
                        <th scope="col" class=TH>{tr("ui-jobs-name")}</th>
                        <th scope="col" class=TH>{tr("ui-job-state")}</th>
                        <th scope="col" class=TH>{tr("ui-jobs-target")}</th>
                        <th scope="col" class=format!("{TH} text-right")>{tr("ui-stat-pps")}</th>
                        <th scope="col" class=format!("{TH} text-right max-sm:hidden")>{tr("ui-stat-bps")}</th>
                        <th scope="col" class=format!("{TH} text-right")>{tr("ui-stat-packets")}</th>
                        <th scope="col" class=format!("{TH} max-lg:hidden")>{tr("ui-job-created")}</th>
                    </tr>
                </thead>
                <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                    {list
                        .into_iter()
                        .map(|j| {
                            let id = StoredValue::new(j.id.clone());
                            let initial = j.status.clone();
                            let status = Signal::derive(move || {
                                live.job_states
                                    .with(|m| m.get(&id.get_value()).cloned())
                                    .filter(|s| !initial.is_terminal() || s.is_terminal())
                                    .unwrap_or_else(|| initial.clone())
                            });
                            let fallback = j.stats.clone().unwrap_or_default();
                            let cur = move || {
                                live.job(&id.get_value())
                                    .map_or_else(|| fallback.clone(), |l| l.latest)
                            };
                            let cur2 = cur.clone();
                            let cur3 = cur.clone();
                            let created = j.created;
                            view! {
                                <tr class="hover:bg-slate-50 dark:hover:bg-slate-800/50">
                                    <td class=TD>
                                        <button
                                            type="button"
                                            class="text-left font-medium text-sky-700 hover:underline dark:text-sky-400"
                                            on:click=move |_| on_open.run(id.get_value())
                                        >
                                            {j.name.clone()}
                                        </button>
                                        <span class="block font-mono text-xs text-slate-400">{j.rate.clone()}</span>
                                    </td>
                                    <td class=TD><StateBadge status=status /></td>
                                    <td class=format!("{TD} text-xs")>
                                        <span class="font-mono">{j.backend.clone()}</span>
                                        <span class="block max-w-48 truncate font-mono text-slate-500" title=j.iface.clone()>{j.iface.clone()}</span>
                                    </td>
                                    <td class=TD_NUM>{move || fmt::si(cur().pps, "pps")}</td>
                                    <td class=format!("{TD_NUM} max-sm:hidden")>{move || fmt::si(cur2().bps, "bps")}</td>
                                    <td class=TD_NUM>{move || fmt::int(i18n.locale.get(), cur3().packets)}</td>
                                    <td class=format!("{TD} text-xs text-slate-500 max-lg:hidden")>
                                        {move || fmt::datetime(i18n.locale.get(), created)}
                                    </td>
                                </tr>
                            }
                        })
                        .collect_view()}
                </tbody>
            </table>
        </div>
    }
}
