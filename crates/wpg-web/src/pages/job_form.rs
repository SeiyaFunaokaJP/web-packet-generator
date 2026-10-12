//! "New job" form: streams (templates + weights, mix), backend, interface, rate,
//! limits, dry run; confirmation before sending on a real interface (§5 誤用対策).

use leptos::prelude::*;

use crate::api::{self, ApiError, BackendItem, CreateJob, JobSpec, JobStream, TemplateQuery};
use crate::components::{
    use_toasts, Confirm, ErrorBox, Icon, IconName, Loading, Modal, BTN, BTN_GHOST, BTN_PRIMARY,
    INPUT, INPUT_INVALID, LABEL,
};
use crate::fmt;
use crate::i18n::{tr, use_i18n, I18n};
use crate::validate::{self, RateKind};

/// Backends that never touch a network interface.
pub(crate) fn is_offline_backend(name: &str) -> bool {
    matches!(name, "null" | "pcapfile")
}

/// Whether starting a job on `backend` (with `dry_run`) needs the confirmation
/// dialog: anything that really transmits.
pub(crate) fn needs_confirmation(backend: &str, dry_run: &str, start: bool) -> bool {
    start && dry_run.trim().is_empty() && !is_offline_backend(backend)
}

/// Hint under the rate input.
pub(crate) fn rate_hint(i18n: I18n, input: &str) -> Result<String, &'static str> {
    if input.trim().is_empty() {
        return Ok(i18n.t("ui-rate-default"));
    }
    let r = validate::parse_rate(input)?;
    Ok(match r {
        RateKind::Pps(v) => i18n.ta("ui-rate-is-pps", &[("rate", fmt::si(v, "pps"))]),
        RateKind::Bps(v) => i18n.ta("ui-rate-is-bps", &[("rate", fmt::si(v, "bps"))]),
        RateKind::Percent(p) => i18n.ta("ui-rate-is-percent", &[("rate", format!("{p}"))]),
        RateKind::Ifg(_) => i18n.ta("ui-rate-is-ifg", &[("rate", input.trim().to_owned())]),
        RateKind::Unlimited => i18n.t("ui-rate-is-unlimited"),
    })
}

/// Signature stream id: empty = server default (0), else `0..=u32::MAX`.
pub(crate) fn parse_stream_id(s: &str) -> Result<Option<u32>, validate::Invalid> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let cleaned: String = t.chars().filter(|c| *c != '_').collect();
    cleaned
        .parse::<u32>()
        .map(Some)
        .map_err(|_| "ui-valid-stream-id")
}

#[derive(Debug, Clone, Copy)]
struct StreamRow {
    key: u32,
    template: RwSignal<String>,
    weight: RwSignal<String>,
}

fn opt(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_owned())
}

/// The form in a modal.
#[component]
pub(crate) fn JobForm(
    /// Pre-selected template id.
    initial_template: Option<String>,
    #[prop(into)] on_close: Callback<()>,
    /// Called with the new job id.
    #[prop(into)]
    on_created: Callback<String>,
) -> impl IntoView {
    let i18n = use_i18n();
    let toasts = use_toasts();
    let templates = LocalResource::new(|| api::templates(TemplateQuery::default()));
    let backends = LocalResource::new(api::backends);
    let ifaces = LocalResource::new(|| api::ifaces(false));

    let next_key = StoredValue::new(1u32);
    let streams = RwSignal::new(vec![StreamRow {
        key: 0,
        template: RwSignal::new(initial_template.unwrap_or_default()),
        weight: RwSignal::new("1".to_owned()),
    }]);
    let name = RwSignal::new(String::new());
    let mix = RwSignal::new("weighted".to_owned());
    let backend = RwSignal::new(String::new());
    let iface = RwSignal::new(String::new());
    let rate = RwSignal::new(String::new());
    let count = RwSignal::new(String::new());
    let duration = RwSignal::new(String::new());
    let burst = RwSignal::new(String::new());
    let workers = RwSignal::new(String::new());
    let dry_run = RwSignal::new(String::new());
    let start = RwSignal::new(true);
    let signature = RwSignal::new(false);
    let stream_id = RwSignal::new(String::new());
    let submitted = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<ApiError>);
    let confirming = RwSignal::new(false);

    // Default backend once known.
    Effect::new(move |_| {
        if let Some(Ok(b)) = backends.get() {
            if backend.get_untracked().is_empty() {
                backend.set(b.default.clone());
            }
        }
    });

    let backend_item = move || -> Option<BackendItem> {
        let name = backend.get();
        backends
            .get()
            .and_then(Result::ok)
            .and_then(|b| b.backends.into_iter().find(|x| x.name == name))
    };
    let effective_backend = move || {
        if dry_run.with(|d| d.trim().is_empty()) {
            backend.get()
        } else {
            "pcapfile".to_owned()
        }
    };
    let needs_iface = move || !is_offline_backend(&effective_backend());
    let needs_capture = move || backend.get() == "pcapfile";
    let workers_ok =
        move || backend_item().is_some_and(|b| b.multi_worker) && streams.with(Vec::len) == 1;

    // Field checks: `Err(i18n key)`.
    let rate_check = move || rate_hint(i18n, &rate.get());
    let count_check = move || validate::optional(&count.get(), validate::parse_positive);
    let duration_check = move || validate::optional(&duration.get(), validate::parse_duration);
    let burst_check = move || validate::optional(&burst.get(), validate::parse_positive);
    let workers_check = move || validate::optional(&workers.get(), validate::parse_positive);
    let stream_id_check = move || {
        if signature.get() {
            parse_stream_id(&stream_id.get()).map(|_| ())
        } else {
            Ok(())
        }
    };
    let capture_check = move || {
        if needs_capture() {
            validate::check_capture_name(&dry_run.get())
        } else {
            validate::optional(&dry_run.get(), validate::check_capture_name)
        }
    };
    let iface_check = move || {
        if needs_iface() && iface.with(|i| i.trim().is_empty()) {
            Err("ui-jobs-need-iface")
        } else {
            Ok(())
        }
    };
    let streams_check = move || {
        streams.with(|s| {
            if s.iter().any(|r| r.template.with(|t| t.trim().is_empty())) {
                Err("ui-jobs-need-template")
            } else if s
                .iter()
                .any(|r| validate::parse_positive(&r.weight.get()).is_err())
            {
                Err("ui-jobs-bad-weight")
            } else {
                Ok(())
            }
        })
    };
    let all_ok = move || {
        rate_check().is_ok()
            && count_check().is_ok()
            && duration_check().is_ok()
            && burst_check().is_ok()
            && workers_check().is_ok()
            && stream_id_check().is_ok()
            && capture_check().is_ok()
            && iface_check().is_ok()
            && streams_check().is_ok()
    };

    let build = move || -> CreateJob {
        let stream_list = streams.with_untracked(|s| {
            s.iter()
                .map(|r| JobStream {
                    template_id: opt(&r.template.get_untracked()),
                    weight: validate::parse_positive(&r.weight.get_untracked())
                        .ok()
                        .and_then(|w| u32::try_from(w).ok())
                        .unwrap_or(1),
                    ..JobStream::default()
                })
                .collect::<Vec<_>>()
        });
        let dry = opt(&dry_run.get_untracked());
        let be = backend.get_untracked();
        CreateJob {
            spec: JobSpec {
                name: opt(&name.get_untracked()),
                streams: stream_list,
                mix: mix.get_untracked(),
                iface: if dry.is_none() && !is_offline_backend(&be) {
                    opt(&iface.get_untracked())
                } else {
                    None
                },
                backend: if dry.is_some() { None } else { opt(&be) },
                rate: opt(&rate.get_untracked()),
                count: validate::parse_positive(&count.get_untracked()).ok(),
                duration: opt(&duration.get_untracked()),
                burst: validate::parse_positive(&burst.get_untracked())
                    .ok()
                    .and_then(|b| u32::try_from(b).ok()),
                workers: if workers_ok() {
                    validate::parse_positive(&workers.get_untracked())
                        .ok()
                        .and_then(|w| usize::try_from(w).ok())
                } else {
                    None
                },
                dry_run: dry,
                signature: signature.get_untracked(),
                stream_id: if signature.get_untracked() {
                    parse_stream_id(&stream_id.get_untracked()).ok().flatten()
                } else {
                    None
                },
            },
            start: start.get_untracked(),
        }
    };

    let send = move || {
        let body = build();
        busy.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            match api::create_job(&body).await {
                Ok(job) => {
                    toasts.ok(i18n.ta("ui-jobs-created", &[("id", job.name.clone())]));
                    on_created.run(job.id);
                }
                Err(e) => error.set(Some(e)),
            }
            busy.set(false);
            confirming.set(false);
        });
    };

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        submitted.set(true);
        if !all_ok() || busy.get_untracked() {
            return;
        }
        if needs_confirmation(
            &backend.get_untracked(),
            &dry_run.get_untracked(),
            start.get_untracked(),
        ) {
            confirming.set(true);
        } else {
            send();
        }
    };

    // Shows the error of a field (after the first submit, or at once for rate hints).
    let field_error = move |r: Result<(), &'static str>| {
        r.err()
            .filter(|_| submitted.get())
            .map(|k| view! { <p class="mt-1 text-xs text-rose-600 dark:text-rose-400">{move || i18n.t(k)}</p> })
    };
    let input_class = move |ok: bool| {
        if ok || !submitted.get() {
            INPUT
        } else {
            INPUT_INVALID
        }
    };

    let add_stream = move |_| {
        let key = next_key.get_value();
        next_key.set_value(key + 1);
        streams.update(|s| {
            s.push(StreamRow {
                key,
                template: RwSignal::new(String::new()),
                weight: RwSignal::new("1".to_owned()),
            });
        });
    };

    view! {
        <Modal title=tr("ui-jobs-new") on_close=on_close wide=true>
            <form on:submit=submit novalidate class="space-y-5">
                // Streams
                <fieldset>
                    <legend class="mb-2 text-sm font-semibold">{tr("ui-jobs-streams")}</legend>
                    {move || match templates.get() {
                        None => view! { <Loading /> }.into_any(),
                        Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                        Some(Ok(list)) => {
                            let list = StoredValue::new(list.templates);
                            view! {
                                <div class="space-y-2">
                                    <For
                                        each=move || streams.get()
                                        key=|r| r.key
                                        children=move |row: StreamRow| {
                                            let tid = format!("stream-tpl-{}", row.key);
                                            let wid = format!("stream-w-{}", row.key);
                                            view! {
                                                <div class="flex flex-wrap items-end gap-2">
                                                    <div class="min-w-48 flex-1">
                                                        <label for=tid.clone() class=LABEL>{tr("ui-jobs-template")}</label>
                                                        <select
                                                            id=tid
                                                            class=INPUT
                                                            on:change=move |ev| row.template.set(event_target_value(&ev))
                                                        >
                                                            <option value="" selected=move || row.template.with(String::is_empty)>
                                                                {tr("ui-jobs-choose-template")}
                                                            </option>
                                                            {list
                                                                .get_value()
                                                                .into_iter()
                                                                .map(|t| {
                                                                    let id = t.id.clone();
                                                                    let id2 = t.id.clone();
                                                                    let label = if t.error.is_some() {
                                                                        format!("{} ⚠", t.name)
                                                                    } else {
                                                                        t.name.clone()
                                                                    };
                                                                    view! {
                                                                        <option
                                                                            value=id
                                                                            disabled=t.error.is_some()
                                                                            selected=move || row.template.get() == id2
                                                                        >
                                                                            {label}
                                                                        </option>
                                                                    }
                                                                })
                                                                .collect_view()}
                                                        </select>
                                                    </div>
                                                    <Show when=move || { streams.with(Vec::len) > 1 && mix.get() == "weighted" }>
                                                        <div class="w-24">
                                                            <label for=wid.clone() class=LABEL>{tr("ui-jobs-weight")}</label>
                                                            <input
                                                                id=wid.clone()
                                                                class=INPUT
                                                                inputmode="numeric"
                                                                prop:value=move || row.weight.get()
                                                                on:input=move |ev| row.weight.set(event_target_value(&ev))
                                                            />
                                                        </div>
                                                    </Show>
                                                    <Show when=move || { streams.with(Vec::len) > 1 }>
                                                        <button
                                                            type="button"
                                                            class=BTN_GHOST
                                                            aria-label=tr("ui-jobs-remove-stream")
                                                            title=tr("ui-jobs-remove-stream")
                                                            on:click=move |_| streams.update(|s| s.retain(|r| r.key != row.key))
                                                        >
                                                            <Icon name=IconName::Trash class="h-4 w-4" />
                                                        </button>
                                                    </Show>
                                                </div>
                                            }
                                        }
                                    />
                                </div>
                            }
                                .into_any()
                        }
                    }}
                    {move || field_error(streams_check())}
                    <div class="mt-2 flex flex-wrap items-center gap-3">
                        <button type="button" class=BTN on:click=add_stream>
                            <Icon name=IconName::Plus class="h-4 w-4" />
                            {tr("ui-jobs-add-stream")}
                        </button>
                        <Show when=move || { streams.with(Vec::len) > 1 }>
                            <label class="flex items-center gap-2 text-sm">
                                {tr("ui-jobs-mix")}
                                <select class="rounded-md border border-slate-300 bg-white px-2 py-1 text-sm dark:border-slate-700 dark:bg-slate-900" on:change=move |ev| mix.set(event_target_value(&ev))>
                                    <option value="weighted" selected=move || mix.get() == "weighted">{tr("ui-mix-weighted")}</option>
                                    <option value="sequential" selected=move || mix.get() == "sequential">{tr("ui-mix-sequential")}</option>
                                    <option value="round_robin" selected=move || mix.get() == "round_robin">{tr("ui-mix-round-robin")}</option>
                                </select>
                            </label>
                        </Show>
                    </div>
                </fieldset>

                // Where
                <fieldset class="grid gap-4 sm:grid-cols-2">
                    <legend class="mb-2 text-sm font-semibold sm:col-span-2">{tr("ui-jobs-target")}</legend>
                    <div>
                        <label for="job-backend" class=LABEL>{tr("ui-jobs-backend")}</label>
                        {move || match backends.get() {
                            None => view! { <Loading /> }.into_any(),
                            Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                            Some(Ok(b)) => {
                                view! {
                                    <select id="job-backend" class=INPUT on:change=move |ev| backend.set(event_target_value(&ev))>
                                        {b
                                            .backends
                                            .into_iter()
                                            .map(|x| {
                                                let n = x.name.clone();
                                                let n2 = x.name.clone();
                                                let label = if x.available {
                                                    x.name.clone()
                                                } else {
                                                    format!("{} — {}", x.name, x.reason.clone().unwrap_or_default())
                                                };
                                                view! {
                                                    <option
                                                        value=n
                                                        disabled=!x.available
                                                        title=x.reason.clone().unwrap_or_default()
                                                        selected=move || backend.get() == n2
                                                    >
                                                        {label}
                                                    </option>
                                                }
                                            })
                                            .collect_view()}
                                    </select>
                                }
                                    .into_any()
                            }
                        }}
                        <p class="mt-1 text-xs text-slate-500">
                            {move || backend_item().and_then(|b| b.reason)}
                        </p>
                    </div>
                    <div>
                        <label for="job-iface" class=LABEL>{tr("ui-jobs-iface")}</label>
                        {move || match ifaces.get() {
                            None => view! { <Loading /> }.into_any(),
                            Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                            Some(Ok(list)) => {
                                view! {
                                    <select
                                        id="job-iface"
                                        class=move || input_class(iface_check().is_ok())
                                        disabled=move || !needs_iface()
                                        on:change=move |ev| iface.set(event_target_value(&ev))
                                    >
                                        <option value="" selected=move || iface.with(String::is_empty)>
                                            {move || i18n.t(if needs_iface() { "ui-jobs-choose-iface" } else { "ui-jobs-iface-not-needed" })}
                                        </option>
                                        {list
                                            .into_iter()
                                            .map(|x| {
                                                let n = x.name.clone();
                                                let n2 = x.name.clone();
                                                let state = if x.up && x.running { "▲" } else { "▽" };
                                                let label = if x.allowed {
                                                    format!("{} {state}", x.name)
                                                } else {
                                                    format!("{} {state} ({})", x.name, i18n.t("ui-jobs-iface-denied"))
                                                };
                                                view! {
                                                    <option value=n disabled=!x.allowed selected=move || iface.get() == n2>
                                                        {label}
                                                    </option>
                                                }
                                            })
                                            .collect_view()}
                                    </select>
                                }
                                    .into_any()
                            }
                        }}
                        {move || field_error(iface_check())}
                    </div>
                    <div class="sm:col-span-2">
                        <label for="job-dry" class=LABEL>{tr("ui-jobs-dry-run")}</label>
                        <input
                            id="job-dry"
                            class=move || input_class(capture_check().is_ok())
                            placeholder="capture.pcapng"
                            aria-describedby="job-dry-hint"
                            prop:value=move || dry_run.get()
                            on:input=move |ev| dry_run.set(event_target_value(&ev))
                        />
                        <p id="job-dry-hint" class="mt-1 text-xs text-slate-500">{tr("ui-jobs-dry-run-hint")}</p>
                        {move || field_error(capture_check())}
                    </div>
                </fieldset>

                // How
                <fieldset class="grid gap-4 sm:grid-cols-3">
                    <legend class="mb-2 text-sm font-semibold sm:col-span-3">{tr("ui-jobs-how")}</legend>
                    <div class="sm:col-span-3">
                        <label for="job-rate" class=LABEL>{tr("ui-jobs-rate")}</label>
                        <input
                            id="job-rate"
                            class=move || if rate_check().is_ok() { INPUT } else { INPUT_INVALID }
                            placeholder="1Mpps · 10Gbps · 50% · ifg=10us · unlimited"
                            aria-describedby="job-rate-hint"
                            aria-invalid=move || rate_check().is_err().to_string()
                            prop:value=move || rate.get()
                            on:input=move |ev| rate.set(event_target_value(&ev))
                        />
                        <p
                            id="job-rate-hint"
                            class=move || if rate_check().is_ok() { "mt-1 text-xs text-slate-500" } else { "mt-1 text-xs text-rose-600 dark:text-rose-400" }
                            aria-live="polite"
                        >
                            {move || match rate_check() {
                                Ok(s) => s,
                                Err(k) => i18n.t(k),
                            }}
                        </p>
                    </div>
                    <div>
                        <label for="job-count" class=LABEL>{tr("ui-jobs-count")}</label>
                        <input
                            id="job-count"
                            class=move || input_class(count_check().is_ok())
                            inputmode="numeric"
                            placeholder=tr("ui-jobs-from-template")
                            prop:value=move || count.get()
                            on:input=move |ev| count.set(event_target_value(&ev))
                        />
                        {move || field_error(count_check())}
                    </div>
                    <div>
                        <label for="job-duration" class=LABEL>{tr("ui-jobs-duration")}</label>
                        <input
                            id="job-duration"
                            class=move || input_class(duration_check().is_ok())
                            placeholder="10s · 5m"
                            prop:value=move || duration.get()
                            on:input=move |ev| duration.set(event_target_value(&ev))
                        />
                        {move || field_error(duration_check())}
                    </div>
                    <div>
                        <label for="job-burst" class=LABEL>{tr("ui-jobs-burst")}</label>
                        <input
                            id="job-burst"
                            class=move || input_class(burst_check().is_ok())
                            inputmode="numeric"
                            placeholder="1"
                            prop:value=move || burst.get()
                            on:input=move |ev| burst.set(event_target_value(&ev))
                        />
                        {move || field_error(burst_check())}
                    </div>
                    <div>
                        <label for="job-workers" class=LABEL>{tr("ui-jobs-workers")}</label>
                        <input
                            id="job-workers"
                            class=move || input_class(workers_check().is_ok())
                            inputmode="numeric"
                            placeholder="1"
                            disabled=move || !workers_ok()
                            title=move || (!workers_ok()).then(|| i18n.t("ui-jobs-workers-disabled"))
                            prop:value=move || workers.get()
                            on:input=move |ev| workers.set(event_target_value(&ev))
                        />
                        {move || field_error(workers_check())}
                    </div>
                    <div class="sm:col-span-2">
                        <label for="job-name" class=LABEL>{tr("ui-jobs-name")}</label>
                        <input
                            id="job-name"
                            class=INPUT
                            placeholder=tr("ui-jobs-name-placeholder")
                            prop:value=move || name.get()
                            on:input=move |ev| name.set(event_target_value(&ev))
                        />
                    </div>
                </fieldset>

                // Receive verification (TODO M9)
                <fieldset class="grid gap-4 sm:grid-cols-3">
                    <legend class="mb-2 text-sm font-semibold sm:col-span-3">{tr("ui-jobs-verification")}</legend>
                    <div class="sm:col-span-2">
                        <label class="flex items-center gap-2 text-sm">
                            <input
                                type="checkbox"
                                aria-describedby="job-signature-hint"
                                prop:checked=move || signature.get()
                                on:change=move |ev| signature.set(event_target_checked(&ev))
                            />
                            {tr("ui-jobs-signature")}
                        </label>
                        <p id="job-signature-hint" class="mt-1 text-xs text-slate-500">{tr("ui-jobs-signature-hint")}</p>
                    </div>
                    <div>
                        <label for="job-stream-id" class=LABEL>{tr("ui-jobs-stream-id")}</label>
                        <input
                            id="job-stream-id"
                            class=move || input_class(stream_id_check().is_ok())
                            inputmode="numeric"
                            placeholder="0"
                            disabled=move || !signature.get()
                            aria-invalid=move || (stream_id_check().is_err() && submitted.get()).to_string()
                            prop:value=move || stream_id.get()
                            on:input=move |ev| stream_id.set(event_target_value(&ev))
                        />
                        {move || field_error(stream_id_check())}
                    </div>
                </fieldset>

                <label class="flex items-center gap-2 text-sm">
                    <input
                        type="checkbox"
                        prop:checked=move || start.get()
                        on:change=move |ev| start.set(event_target_checked(&ev))
                    />
                    {tr("ui-jobs-start-now")}
                </label>

                {move || error.get().map(|e| view! { <ErrorBox error=e /> })}

                <div class="flex justify-end gap-2 border-t border-slate-200 pt-4 dark:border-slate-800">
                    <button type="button" class=BTN on:click=move |_| on_close.run(())>
                        {tr("ui-cancel")}
                    </button>
                    <button type="submit" class=BTN_PRIMARY disabled=move || busy.get()>
                        <Icon name=IconName::Play class="h-4 w-4" />
                        {move || i18n.t(if start.get() { "ui-jobs-create-start" } else { "ui-jobs-create" })}
                    </button>
                </div>
            </form>
        </Modal>
        <Show when=move || confirming.get()>
            <Confirm
                title=tr("ui-confirm-send-title")
                confirm_label=tr("ui-confirm-send")
                danger=true
                busy=busy
                on_confirm=move || send()
                on_cancel=move || confirming.set(false)
            >
                <p>{tr("ui-confirm-send-body")}</p>
                <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 rounded-md bg-slate-50 p-3 font-mono text-xs dark:bg-slate-800">
                    <dt class="text-slate-500">{tr("ui-jobs-iface")}</dt>
                    <dd class="font-semibold">{move || iface.get()}</dd>
                    <dt class="text-slate-500">{tr("ui-jobs-backend")}</dt>
                    <dd>{move || backend.get()}</dd>
                    <dt class="text-slate-500">{tr("ui-jobs-rate")}</dt>
                    <dd class="font-semibold">
                        {move || opt(&rate.get()).unwrap_or_else(|| i18n.t("ui-rate-default"))}
                    </dd>
                </dl>
            </Confirm>
        </Show>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_rules() {
        assert!(needs_confirmation("afpacket", "", true));
        assert!(needs_confirmation("pcap", " ", true));
        assert!(
            !needs_confirmation("afpacket", "", false),
            "not started yet"
        );
        assert!(
            !needs_confirmation("afpacket", "cap.pcapng", true),
            "dry run"
        );
        assert!(!needs_confirmation("null", "", true));
        assert!(!needs_confirmation("pcapfile", "", true));
        assert_eq!(opt("  x "), Some("x".to_owned()));
        assert_eq!(opt("  "), None);
        assert_eq!(parse_stream_id(""), Ok(None));
        assert_eq!(parse_stream_id(" 0 "), Ok(Some(0)));
        assert_eq!(parse_stream_id("4_294_967_295"), Ok(Some(u32::MAX)));
        assert_eq!(parse_stream_id("4294967296"), Err("ui-valid-stream-id"));
        assert_eq!(parse_stream_id("-1"), Err("ui-valid-stream-id"));
    }
}
