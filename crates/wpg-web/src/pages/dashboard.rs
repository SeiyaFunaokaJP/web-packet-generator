//! `/`: running jobs with live rates and sparklines, interfaces, doctor summary.

use std::time::Duration;

use leptos::prelude::*;

use crate::api::{self, Iface, JobBrief};
use crate::components::{
    Empty, ErrorBox, Icon, IconName, Loading, PageHeader, Stat, StateBadge, BTN, CARD, TD, TH,
};
use crate::fmt;
use crate::i18n::{tr, use_i18n};
use crate::sparkline::Sparkline;
use crate::ws::use_live;

/// Refresh period of the overview (live rates come from the WebSocket).
const OVERVIEW_EVERY: Duration = Duration::from_secs(5);

#[component]
pub(crate) fn DashboardPage() -> impl IntoView {
    let i18n = use_i18n();
    let live = use_live();
    let tick = RwSignal::new(0u32);
    if let Ok(h) = set_interval_with_handle(move || tick.update(|t| *t += 1), OVERVIEW_EVERY) {
        on_cleanup(move || h.clear());
    }
    let overview = LocalResource::new(move || {
        tick.track();
        live.jobs_rev.track();
        api::overview()
    });
    let ifaces_refresh = RwSignal::new(false);
    let ifaces = LocalResource::new(move || api::ifaces(ifaces_refresh.get()));
    let doctor = LocalResource::new(api::doctor);

    // Totals: live WebSocket rates of the active jobs, else the overview's.
    let totals = move || {
        overview.get().and_then(Result::ok).map(|o| {
            let (mut pps, mut bps) = (0.0, 0.0);
            for j in &o.jobs {
                let (p, b) = live
                    .job(&j.id)
                    .map_or((j.pps, j.bps), |l| (l.latest.pps, l.latest.bps));
                pps += p;
                bps += b;
            }
            (
                o.totals.active,
                o.totals.running,
                pps,
                bps,
                o.templates,
                o.uptime_secs,
            )
        })
    };

    view! {
        <PageHeader title=tr("ui-nav-dashboard") />
        <section aria-labelledby="dash-totals" class=format!("{CARD} mb-6")>
            <h2 id="dash-totals" class="sr-only">{tr("ui-dash-totals")}</h2>
            <dl class="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-6">
                <Stat label=tr("ui-dash-active-jobs") value=Signal::derive(move || totals().map(|t| t.0.to_string()).unwrap_or_default()) />
                <Stat label=tr("ui-dash-running-jobs") value=Signal::derive(move || totals().map(|t| t.1.to_string()).unwrap_or_default()) />
                <Stat label=tr("ui-dash-total-pps") value=Signal::derive(move || totals().map(|t| fmt::si(t.2, "pps")).unwrap_or_default()) />
                <Stat label=tr("ui-dash-total-bps") value=Signal::derive(move || totals().map(|t| fmt::si(t.3, "bps")).unwrap_or_default()) />
                <Stat label=tr("ui-dash-templates") value=Signal::derive(move || totals().map(|t| t.4.to_string()).unwrap_or_default()) />
                <Stat label=tr("ui-dash-uptime") value=Signal::derive(move || totals().map(|t| fmt::duration(t.5 as f64)).unwrap_or_default()) />
            </dl>
        </section>

        <div class="grid gap-6 xl:grid-cols-3">
            <section aria-labelledby="dash-running" class="xl:col-span-2">
                <div class="mb-3 flex items-center justify-between">
                    <h2 id="dash-running" class="text-sm font-semibold">{tr("ui-dash-running")}</h2>
                    <a href="/jobs?new=1" class=format!("{BTN} hidden md:inline-flex")>
                        <Icon name=IconName::Plus class="h-4 w-4" />
                        {tr("ui-jobs-new")}
                    </a>
                </div>
                {move || match overview.get() {
                    None => view! { <Loading /> }.into_any(),
                    Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                    Some(Ok(o)) if o.jobs.is_empty() => {
                        view! { <Empty text=tr("ui-dash-no-running") /> }.into_any()
                    }
                    Some(Ok(o)) => {
                        view! {
                            <div class="grid gap-3 sm:grid-cols-2">
                                {o.jobs.into_iter().map(|j| view! { <JobCard job=j /> }).collect_view()}
                            </div>
                        }
                            .into_any()
                    }
                }}
            </section>

            <section aria-labelledby="dash-doctor">
                <h2 id="dash-doctor" class="mb-3 text-sm font-semibold">{tr("ui-dash-doctor")}</h2>
                <div class=CARD>
                    {move || match doctor.get() {
                        None => view! { <Loading /> }.into_any(),
                        Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                        Some(Ok(r)) => {
                            let (ok, warn, fail) = r.counts();
                            let (cls, key) = if fail > 0 {
                                ("bg-rose-100 text-rose-800 dark:bg-rose-500/15 dark:text-rose-300", "ui-doctor-fail")
                            } else if warn > 0 {
                                ("bg-amber-100 text-amber-800 dark:bg-amber-500/15 dark:text-amber-300", "ui-doctor-warn")
                            } else {
                                ("bg-emerald-100 text-emerald-800 dark:bg-emerald-500/15 dark:text-emerald-300", "ui-doctor-ok")
                            };
                            view! {
                                <div class="flex flex-wrap items-center gap-3">
                                    <span class=format!("rounded-full px-2.5 py-0.5 text-sm font-semibold {cls}")>
                                        {move || i18n.t(key)}
                                    </span>
                                    <span class="text-xs text-slate-500 tabular-nums">
                                        {move || {
                                            i18n.ta(
                                                "ui-doctor-counts",
                                                &[
                                                    ("ok", ok.to_string()),
                                                    ("warn", warn.to_string()),
                                                    ("fail", fail.to_string()),
                                                ],
                                            )
                                        }}
                                    </span>
                                </div>
                                <ul class="mt-3 space-y-1 text-xs">
                                    {r
                                        .checks
                                        .into_iter()
                                        .filter(|c| c.status == "warn" || c.status == "fail")
                                        .take(4)
                                        .map(|c| {
                                            view! {
                                                <li class="flex gap-1.5">
                                                    <Icon
                                                        name=IconName::Warning
                                                        class=if c.status == "fail" { "h-4 w-4 text-rose-500" } else { "h-4 w-4 text-amber-500" }
                                                    />
                                                    <span>{c.summary}</span>
                                                </li>
                                            }
                                        })
                                        .collect_view()}
                                </ul>
                                <a href="/settings#doctor" class="mt-3 inline-block text-xs text-sky-700 underline dark:text-sky-400">
                                    {tr("ui-doctor-details")}
                                </a>
                            }
                                .into_any()
                        }
                    }}
                </div>
            </section>
        </div>

        <section aria-labelledby="dash-ifaces" class="mt-6">
            <div class="mb-3 flex items-center justify-between">
                <h2 id="dash-ifaces" class="text-sm font-semibold">{tr("ui-dash-ifaces")}</h2>
                <button
                    type="button"
                    class=BTN
                    on:click=move |_| {
                        ifaces_refresh.set(true);
                        ifaces.refetch();
                    }
                >
                    <Icon name=IconName::Refresh class="h-4 w-4" />
                    {tr("ui-refresh")}
                </button>
            </div>
            {move || match ifaces.get() {
                None => view! { <Loading /> }.into_any(),
                Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                Some(Ok(list)) => view! { <IfaceTable list=list /> }.into_any(),
            }}
        </section>
    }
}

/// A running job with live counters and a pps sparkline.
#[component]
fn JobCard(job: JobBrief) -> impl IntoView {
    let i18n = use_i18n();
    let live = use_live();
    let id = StoredValue::new(job.id.clone());
    let fallback = (job.pps, job.bps, job.packets);
    let state = {
        let initial = job.status.clone();
        Signal::derive(move || {
            live.job_states
                .with(|m| m.get(&id.get_value()).cloned())
                .unwrap_or_else(|| initial.clone())
        })
    };
    let cur = move || {
        live.job(&id.get_value())
            .map_or(fallback, |l| (l.latest.pps, l.latest.bps, l.latest.packets))
    };
    let series = Signal::derive(move || {
        live.job(&id.get_value())
            .map(|l| l.pps.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default()
    });
    view! {
        <a
            href=format!("/jobs?job={}", api::enc(&job.id))
            class=format!("{CARD} block transition hover:border-sky-400 dark:hover:border-sky-600")
        >
            <div class="flex items-start justify-between gap-2">
                <div class="min-w-0">
                    <p class="truncate font-medium">{job.name.clone()}</p>
                    <p class="truncate text-xs text-slate-500">
                        {format!("{} · {}", job.backend, job.iface)}
                    </p>
                </div>
                <StateBadge status=state />
            </div>
            <dl class="mt-3 grid grid-cols-3 gap-2">
                <Stat label=tr("ui-stat-pps") value=Signal::derive(move || fmt::si(cur().0, "pps")) />
                <Stat label=tr("ui-stat-bps") value=Signal::derive(move || fmt::si(cur().1, "bps")) />
                <Stat label=tr("ui-stat-packets") value=Signal::derive(move || fmt::int(i18n.locale.get(), cur().2)) />
            </dl>
            <Sparkline
                values=series
                label=Signal::derive(move || i18n.t("ui-spark-pps"))
                class="mt-2 h-10 w-full text-sky-600 dark:text-sky-400"
            />
        </a>
    }
}

/// Interface list (`/api/system/ifaces`).
#[component]
pub(crate) fn IfaceTable(list: Vec<Iface>) -> impl IntoView {
    let i18n = use_i18n();
    let yes_no = move |b: bool| i18n.t(if b { "ui-yes" } else { "ui-no" });
    view! {
        <div class="overflow-x-auto rounded-lg border border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
            <table class="min-w-full divide-y divide-slate-200 dark:divide-slate-800">
                <caption class="sr-only">{tr("ui-dash-ifaces")}</caption>
                <thead>
                    <tr>
                        <th scope="col" class=TH>{tr("ui-iface-name")}</th>
                        <th scope="col" class=TH>{tr("ui-iface-state")}</th>
                        <th scope="col" class=TH>{tr("ui-iface-mac")}</th>
                        <th scope="col" class=TH>{tr("ui-iface-addrs")}</th>
                        <th scope="col" class=TH>{tr("ui-iface-mtu")}</th>
                        <th scope="col" class=TH>{tr("ui-iface-speed")}</th>
                        <th scope="col" class=TH>{tr("ui-iface-allowed")}</th>
                    </tr>
                </thead>
                <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                    {list
                        .into_iter()
                        .map(|i| {
                            let state_key = if !i.up {
                                "ui-iface-down"
                            } else if i.running {
                                "ui-iface-up"
                            } else {
                                "ui-iface-no-carrier"
                            };
                            let dot = if i.up && i.running { "bg-emerald-500" } else if i.up { "bg-amber-400" } else { "bg-slate-400" };
                            let addrs = i.ipv4.iter().chain(i.ipv6.iter()).cloned().collect::<Vec<_>>().join(", ");
                            let speed = i.speed_mbps.map(|s| fmt::si(s as f64 * 1e6, "bps")).unwrap_or_else(|| "–".to_owned());
                            let allowed = i.allowed;
                            view! {
                                <tr>
                                    <td class=TD>
                                        <span class="font-mono">{i.name.clone()}</span>
                                        {i.loopback.then(|| view! { <span class="ml-1 text-xs text-slate-400">"(lo)"</span> })}
                                        {i.driver.clone().map(|d| view! { <span class="block text-xs text-slate-400">{d}</span> })}
                                    </td>
                                    <td class=TD>
                                        <span class="inline-flex items-center gap-1.5">
                                            <span class=format!("h-2 w-2 rounded-full {dot}")></span>
                                            {move || i18n.t(state_key)}
                                        </span>
                                    </td>
                                    <td class=format!("{TD} font-mono text-xs")>{i.mac.clone().unwrap_or_else(|| "–".to_owned())}</td>
                                    <td class=format!("{TD} font-mono text-xs break-all")>{addrs}</td>
                                    <td class=TD>{i.mtu.map_or_else(|| "–".to_owned(), |m| m.to_string())}</td>
                                    <td class=TD>{speed}</td>
                                    <td class=TD>{move || yes_no(allowed)}</td>
                                </tr>
                            }
                        })
                        .collect_view()}
                </tbody>
            </table>
        </div>
    }
}
