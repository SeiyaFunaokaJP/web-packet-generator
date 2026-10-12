//! Job detail drawer: state controls, live stats, per-stream / per-worker tables,
//! pacing accuracy, report download.

use leptos::prelude::*;

use crate::api::{self, JobAction, JobInfo, Stats};
use crate::components::{
    reason_key, use_toasts, Confirm, Drawer, ErrorBox, Icon, IconName, Loading, Stat, StateBadge,
    BTN, BTN_DANGER, TD, TD_NUM, TH,
};
use crate::fmt;
use crate::i18n::{tr, use_i18n};
use crate::pages::job_form::is_offline_backend;
use crate::sparkline::Sparkline;
use crate::ws::use_live;

/// The drawer for job `id`.
#[component]
pub(crate) fn JobDetail(id: String, #[prop(into)] on_close: Callback<()>) -> impl IntoView {
    let live = use_live();
    let i18n = use_i18n();
    let id = StoredValue::new(id);
    let job = LocalResource::new(move || {
        live.jobs_rev.track();
        let id = id.get_value();
        async move { api::job(&id).await }
    });
    let title = Signal::derive(move || {
        job.get()
            .and_then(Result::ok)
            .map_or_else(|| i18n.t("ui-job-detail"), |j| j.name)
    });
    view! {
        <Drawer title=title on_close=on_close>
            {move || match job.get() {
                None => view! { <Loading /> }.into_any(),
                Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                Some(Ok(j)) => view! { <JobBody job=j on_refresh=move || job.refetch() on_deleted=on_close /> }.into_any(),
            }}
        </Drawer>
    }
}

#[component]
fn JobBody(
    job: JobInfo,
    #[prop(into)] on_refresh: Callback<()>,
    #[prop(into)] on_deleted: Callback<()>,
) -> impl IntoView {
    let live = use_live();
    let i18n = use_i18n();
    let toasts = use_toasts();
    let job = StoredValue::new(job);
    let jid = job.with_value(|j| j.id.clone());
    let jid = StoredValue::new(jid);
    let status = Signal::derive(move || {
        let fallback = job.with_value(|j| j.status.clone());
        live.job_states
            .with(|m| m.get(&jid.get_value()).cloned())
            .filter(|s| !fallback.is_terminal() || s.is_terminal())
            .unwrap_or(fallback)
    });
    let stats = Signal::derive(move || -> Option<Stats> {
        live.job(&jid.get_value())
            .map(|l| l.latest)
            .or_else(|| job.with_value(|j| j.stats.clone()))
    });
    let series = Signal::derive(move || {
        live.job(&jid.get_value())
            .map(|l| l.pps.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default()
    });
    let busy = RwSignal::new(false);
    let confirm_start = RwSignal::new(false);
    let confirm_delete = RwSignal::new(false);

    let act = move |action: JobAction| {
        busy.set(true);
        leptos::task::spawn_local(async move {
            match api::job_action(&jid.get_value(), action).await {
                Ok(_) => on_refresh.run(()),
                Err(e) => toasts.error(e.message(i18n)),
            }
            busy.set(false);
            confirm_start.set(false);
        });
    };
    let delete = move || {
        busy.set(true);
        leptos::task::spawn_local(async move {
            let id = jid.get_value();
            match api::delete_job(&id).await {
                Ok(()) => {
                    live.forget(&id);
                    live.jobs_rev.update(|r| *r += 1);
                    toasts.ok(i18n.t("ui-job-deleted"));
                    on_deleted.run(());
                }
                Err(e) => toasts.error(e.message(i18n)),
            }
            busy.set(false);
            confirm_delete.set(false);
        });
    };
    let offline = job.with_value(|j| is_offline_backend(&j.backend));

    let control = move |action: JobAction, icon: IconName, key: &'static str| {
        view! {
            <Show when=move || action.allowed(&status.with(|s| s.state.clone()))>
                <button
                    type="button"
                    class=BTN
                    disabled=move || busy.get()
                    on:click=move |_| {
                        if action == JobAction::Start && !offline {
                            confirm_start.set(true);
                        } else {
                            act(action);
                        }
                    }
                >
                    <Icon name=icon class="h-4 w-4" />
                    {move || i18n.t(key)}
                </button>
            </Show>
        }
    };

    let report = job.with_value(JobInfo::report_view);
    let raw_report = job.with_value(|j| j.report.clone());
    let spec = job.with_value(|j| j.spec.clone());
    let sig = (spec.signature, spec.stream_id);
    let j = job.get_value();

    view! {
        <div class="space-y-6">
            // Controls
            <div class="flex flex-wrap items-center gap-2">
                <StateBadge status=status />
                <span class="text-xs text-slate-500">
                    {move || {
                        status
                            .with(|s| s.detail.clone())
                            .map(|d| reason_key(&d).map_or(d, |k| i18n.t(k)))
                    }}
                </span>
                <div class="ml-auto flex flex-wrap gap-2 max-md:hidden">
                    {control(JobAction::Start, IconName::Play, "ui-job-start")}
                    {control(JobAction::Pause, IconName::Pause, "ui-job-pause")}
                    {control(JobAction::Resume, IconName::Play, "ui-job-resume")}
                    {control(JobAction::Stop, IconName::Stop, "ui-job-stop")}
                    {raw_report
                        .clone()
                        .map(|r| {
                            let file = format!("wpg-job-{}-report.json", j.id);
                            view! {
                                <button
                                    type="button"
                                    class=BTN
                                    on:click=move |_| {
                                        let text = serde_json::to_string_pretty(&r).unwrap_or_default();
                                        crate::dom::download_text(&file, "application/json", &text);
                                    }
                                >
                                    <Icon name=IconName::Download class="h-4 w-4" />
                                    {tr("ui-job-download-report")}
                                </button>
                            }
                        })}
                    <button type="button" class=BTN_DANGER disabled=move || busy.get() on:click=move |_| confirm_delete.set(true)>
                        <Icon name=IconName::Trash class="h-4 w-4" />
                        {tr("ui-delete")}
                    </button>
                </div>
            </div>

            // Live counters
            <section aria-labelledby="job-live">
                <h3 id="job-live" class="mb-2 text-sm font-semibold">{tr("ui-job-live")}</h3>
                <dl class="grid grid-cols-2 gap-4 sm:grid-cols-4">
                    <Stat label=tr("ui-stat-pps") value=Signal::derive(move || stats.get().map(|s| fmt::si(s.pps, "pps")).unwrap_or_else(|| "–".into())) />
                    <Stat label=tr("ui-stat-bps") value=Signal::derive(move || stats.get().map(|s| fmt::si(s.bps, "bps")).unwrap_or_else(|| "–".into())) />
                    <Stat label=tr("ui-stat-l1-bps") value=Signal::derive(move || stats.get().map(|s| fmt::si(s.l1_bps, "bps")).unwrap_or_else(|| "–".into())) />
                    <Stat label=tr("ui-stat-ratio") value=Signal::derive(move || stats.get().and_then(|s| s.rate_achieved_ratio).map(fmt::percent).unwrap_or_else(|| "–".into())) />
                    <Stat label=tr("ui-stat-packets") value=Signal::derive(move || stats.get().map(|s| fmt::int(i18n.locale.get(), s.packets)).unwrap_or_else(|| "–".into())) />
                    <Stat label=tr("ui-stat-bytes") value=Signal::derive(move || stats.get().map(|s| fmt::bytes(s.bytes)).unwrap_or_else(|| "–".into())) />
                    <Stat label=tr("ui-stat-dropped") value=Signal::derive(move || stats.get().map(|s| fmt::int(i18n.locale.get(), s.dropped)).unwrap_or_else(|| "–".into())) />
                    <Stat label=tr("ui-stat-elapsed") value=Signal::derive(move || stats.get().map(|s| fmt::duration(s.t.as_secs_f64())).unwrap_or_else(|| "–".into())) />
                </dl>
                <Sparkline
                    values=series
                    label=Signal::derive(move || i18n.t("ui-spark-pps"))
                    class="mt-3 h-16 w-full text-sky-600 dark:text-sky-400"
                />
            </section>

            // Per stream
            <section aria-labelledby="job-streams">
                <h3 id="job-streams" class="mb-2 text-sm font-semibold">{tr("ui-job-streams")}</h3>
                <div class="overflow-x-auto rounded-md border border-slate-200 dark:border-slate-800">
                    <table class="min-w-full divide-y divide-slate-200 dark:divide-slate-800">
                        <thead>
                            <tr>
                                <th scope="col" class=TH>"#"</th>
                                <th scope="col" class=TH>{tr("ui-jobs-template")}</th>
                                <th scope="col" class=TH>{tr("ui-jobs-weight")}</th>
                                <th scope="col" class=format!("{TH} text-right")>{tr("ui-stat-packets")}</th>
                                <th scope="col" class=format!("{TH} text-right")>{tr("ui-stat-bytes")}</th>
                                <th scope="col" class=format!("{TH} text-right")>{tr("ui-stat-pps")}</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                            {spec
                                .streams
                                .iter()
                                .enumerate()
                                .map(|(i, s)| {
                                    let name = s
                                        .template_id
                                        .clone()
                                        .unwrap_or_else(|| i18n.t("ui-job-inline-template"));
                                    let cell = move |f: fn(&api::StreamStats) -> String| {
                                        move || {
                                            stats
                                                .get()
                                                .and_then(|st| st.per_stream.get(i).map(f))
                                                .unwrap_or_else(|| "–".to_owned())
                                        }
                                    };
                                    view! {
                                        <tr>
                                            <td class=TD>{i + 1}</td>
                                            <td class=format!("{TD} font-mono text-xs")>{name}</td>
                                            <td class=TD>{s.weight}</td>
                                            <td class=TD_NUM>{cell(|x| fmt::group_thousands(x.packets))}</td>
                                            <td class=TD_NUM>{cell(|x| fmt::bytes(x.bytes))}</td>
                                            <td class=TD_NUM>{cell(|x| fmt::si(x.pps, "pps"))}</td>
                                        </tr>
                                    }
                                })
                                .collect_view()}
                        </tbody>
                    </table>
                </div>
            </section>

            // Per worker + accuracy (final report)
            <section aria-labelledby="job-workers">
                <h3 id="job-workers" class="mb-2 text-sm font-semibold">{tr("ui-job-workers")}</h3>
                {match report.clone() {
                    None => view! { <p class="text-xs text-slate-500">{tr("ui-job-workers-pending")}</p> }.into_any(),
                    Some(r) => view! {
                        <div class="overflow-x-auto rounded-md border border-slate-200 dark:border-slate-800">
                            <table class="min-w-full divide-y divide-slate-200 dark:divide-slate-800">
                                <thead>
                                    <tr>
                                        <th scope="col" class=TH>"#"</th>
                                        <th scope="col" class=TH>{tr("ui-job-state")}</th>
                                        <th scope="col" class=format!("{TH} text-right")>{tr("ui-stat-packets")}</th>
                                        <th scope="col" class=format!("{TH} text-right")>{tr("ui-stat-bytes")}</th>
                                        <th scope="col" class=format!("{TH} text-right")>{tr("ui-stat-dropped")}</th>
                                        <th scope="col" class=format!("{TH} text-right")>{tr("ui-job-active")}</th>
                                        <th scope="col" class=TH>{tr("ui-job-core")}</th>
                                    </tr>
                                </thead>
                                <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                                    {r.workers.into_iter().map(|w| {
                                        let core = match (w.core, w.pinned) {
                                            (Some(c), Some(true)) => format!("{c} ✓"),
                                            (Some(c), _) => format!("{c} ✗"),
                                            (None, _) => "–".to_owned(),
                                        };
                                        view! {
                                            <tr>
                                                <td class=TD>{w.index}</td>
                                                <td class=TD><StateBadge status=w.state /></td>
                                                <td class=TD_NUM>{fmt::group_thousands(w.packets)}</td>
                                                <td class=TD_NUM>{fmt::bytes(w.bytes)}</td>
                                                <td class=TD_NUM>{fmt::group_thousands(w.dropped)}</td>
                                                <td class=TD_NUM>{fmt::duration(w.active.as_secs_f64())}</td>
                                                <td class=TD>{core}</td>
                                            </tr>
                                        }
                                    }).collect_view()}
                                </tbody>
                            </table>
                        </div>
                    }.into_any(),
                }}
            </section>

            {report.map(|r| {
                let a = r.accuracy;
                let unit = if a.unit == "bps" { "bps" } else { "pps" };
                view! {
                    <section aria-labelledby="job-accuracy">
                        <h3 id="job-accuracy" class="mb-2 text-sm font-semibold">{tr("ui-job-accuracy")}</h3>
                        <dl class="grid grid-cols-2 gap-4 sm:grid-cols-4">
                            <Stat label=tr("ui-acc-target") value=a.target.map_or_else(|| i18n.t("ui-unlimited"), |t| fmt::si(t, unit)) />
                            <Stat label=tr("ui-acc-achieved") value=fmt::si(a.achieved, unit) />
                            <Stat label=tr("ui-stat-ratio") value=a.ratio.map_or_else(|| "–".to_owned(), fmt::percent) />
                            <Stat label=tr("ui-acc-jitter") value=format!("{:.1} / {:.1} µs", a.mean_jitter.as_secs_f64() * 1e6, a.max_jitter.as_secs_f64() * 1e6) />
                        </dl>
                        {r.backend_detail.map(|d| view! { <p class="mt-2 text-xs text-slate-500">{d}</p> })}
                        {r.pin_error.map(|d| view! { <p class="mt-1 text-xs text-amber-600">{d}</p> })}
                    </section>
                }
            })}

            // Spec
            <section aria-labelledby="job-spec">
                <h3 id="job-spec" class="mb-2 text-sm font-semibold">{tr("ui-job-spec")}</h3>
                <dl class="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1 text-sm">
                    <dt class="text-slate-500">{tr("ui-jobs-backend")}</dt><dd class="font-mono">{j.backend.clone()}</dd>
                    <dt class="text-slate-500">{tr("ui-jobs-iface")}</dt><dd class="font-mono break-all">{j.iface.clone()}</dd>
                    <dt class="text-slate-500">{tr("ui-jobs-rate")}</dt><dd class="font-mono">{j.rate.clone()}</dd>
                    <dt class="text-slate-500">{tr("ui-jobs-workers")}</dt><dd>{j.workers}</dd>
                    <dt class="text-slate-500">{tr("ui-jobs-mix")}</dt><dd>{spec.mix.clone()}</dd>
                    <dt class="text-slate-500">{tr("ui-jobs-count")}</dt><dd>{spec.count.map_or_else(|| "–".to_owned(), |c| c.to_string())}</dd>
                    <dt class="text-slate-500">{tr("ui-jobs-duration")}</dt><dd>{spec.duration.clone().unwrap_or_else(|| "–".to_owned())}</dd>
                    <dt class="text-slate-500">{tr("ui-job-signature")}</dt>
                    <dd>
                        {move || if sig.0 {
                            i18n.ta("ui-job-signature-on", &[("id", sig.1.unwrap_or(0).to_string())])
                        } else {
                            i18n.t("ui-no")
                        }}
                    </dd>
                    <dt class="text-slate-500">{tr("ui-job-created")}</dt><dd>{move || fmt::datetime(i18n.locale.get(), j.created)}</dd>
                    <dt class="text-slate-500">{tr("ui-job-started")}</dt><dd>{move || j.started.map_or_else(|| "–".to_owned(), |t| fmt::datetime(i18n.locale.get(), t))}</dd>
                    <dt class="text-slate-500">{tr("ui-job-finished")}</dt><dd>{move || j.finished.map_or_else(|| "–".to_owned(), |t| fmt::datetime(i18n.locale.get(), t))}</dd>
                    <dt class="text-slate-500">"ID"</dt><dd class="font-mono text-xs">{j.id.clone()}</dd>
                </dl>
            </section>
        </div>

        <Show when=move || confirm_start.get()>
            <Confirm
                title=tr("ui-confirm-send-title")
                confirm_label=tr("ui-confirm-send")
                danger=true
                busy=busy
                on_confirm=move || act(JobAction::Start)
                on_cancel=move || confirm_start.set(false)
            >
                <p>{tr("ui-confirm-send-body")}</p>
                <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 rounded-md bg-slate-50 p-3 font-mono text-xs dark:bg-slate-800">
                    <dt class="text-slate-500">{tr("ui-jobs-iface")}</dt>
                    <dd class="font-semibold">{job.with_value(|j| j.iface.clone())}</dd>
                    <dt class="text-slate-500">{tr("ui-jobs-backend")}</dt>
                    <dd>{job.with_value(|j| j.backend.clone())}</dd>
                    <dt class="text-slate-500">{tr("ui-jobs-rate")}</dt>
                    <dd class="font-semibold">{job.with_value(|j| j.rate.clone())}</dd>
                </dl>
            </Confirm>
        </Show>
        <Show when=move || confirm_delete.get()>
            <Confirm
                title=tr("ui-job-delete-title")
                confirm_label=tr("ui-delete")
                danger=true
                busy=busy
                on_confirm=move || delete()
                on_cancel=move || confirm_delete.set(false)
            >
                <p>{tr("ui-job-delete-body")}</p>
            </Confirm>
        </Show>
    }
}
