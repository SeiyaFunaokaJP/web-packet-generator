//! "New job" form: streams (templates + weights, mix), backend, interface, rate,
//! limits, write-to-file; confirmation before sending on a real interface (§5 誤用対策).
//! It can start from an earlier job's spec ("Edit as new job").

use leptos::prelude::*;

use crate::api::{self, ApiError, BackendItem, CreateJob, JobSpec, JobStream, TemplateQuery};
use crate::components::{
    use_toasts, Confirm, ErrorBox, Icon, IconName, Loading, Modal, BTN, BTN_GHOST, BTN_PRIMARY,
    INPUT, INPUT_INVALID, LABEL,
};
use crate::fmt;
use crate::i18n::{tr, use_i18n, I18n};
use crate::pages::job_vars::{vars_ok, vars_to_send, VarFields, VarValues};
use crate::prefs;
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

/// The interface to preselect: the only allowed one, else the last used one if
/// it is still allowed.
pub(crate) fn pick_iface(allowed: &[String], last: Option<&str>) -> Option<String> {
    match allowed {
        [only] => Some(only.clone()),
        _ => last
            .filter(|l| allowed.iter().any(|a| a == l))
            .map(str::to_owned),
    }
}

/// Days since 1970-01-01 to a proleptic Gregorian `(year, month, day)`.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// Default file name of a write-to-file job: `<template>-<YYYYMMDD-HHMMSS>.pcapng`,
/// with the date and time of `unix_ms` read as UTC (callers shift it to local time).
pub(crate) fn default_capture_name(template: &str, unix_ms: f64) -> String {
    let base: String = template
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let base = if base.trim_matches('-').is_empty() {
        "job".to_owned()
    } else {
        base
    };
    #[allow(clippy::cast_possible_truncation)] // whole seconds of a wall clock
    let secs = (unix_ms / 1000.0).floor() as i64;
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let s = secs.rem_euclid(86_400);
    format!(
        "{base}-{y:04}{m:02}{d:02}-{:02}{:02}{:02}.pcapng",
        s / 3600,
        s % 3600 / 60,
        s % 60
    )
}

#[derive(Debug, Clone, Copy)]
struct StreamRow {
    key: u32,
    template: RwSignal<String>,
    weight: RwSignal<String>,
    /// Variables declared by the chosen template.
    var_defs: RwSignal<Vec<wpg_template::Variable>>,
    /// Values entered for them (seeded from the base stream).
    vars: RwSignal<VarValues>,
    /// The stream this row was seeded from (inline template, variables).
    base: StoredValue<JobStream>,
}

impl StreamRow {
    fn new(key: u32, base: JobStream) -> Self {
        let weight = if base.weight == 0 { 1 } else { base.weight };
        Self {
            key,
            template: RwSignal::new(base.template_id.clone().unwrap_or_default()),
            weight: RwSignal::new(weight.to_string()),
            var_defs: RwSignal::new(Vec::new()),
            vars: RwSignal::new(base.vars.clone()),
            base: StoredValue::new(base),
        }
    }

    /// An inline template (no id): shown read-only, sent back as is.
    fn is_inline(&self) -> bool {
        self.base
            .with_value(|b| b.template.is_some() && b.template_id.is_none())
    }

    fn to_stream(self) -> JobStream {
        let base = self.base.get_value();
        let template_id = opt(&self.template.get_untracked());
        let weight = validate::parse_positive(&self.weight.get_untracked())
            .ok()
            .and_then(|w| u32::try_from(w).ok())
            .unwrap_or(1);
        if self.is_inline() {
            return JobStream { weight, ..base };
        }
        // Values are cleared when the template changes.
        let vars = vars_to_send(&self.vars.get_untracked());
        JobStream {
            template_id,
            template: None,
            vars,
            weight,
        }
    }
}

fn opt(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_owned())
}

fn num<T: ToString>(v: Option<T>) -> String {
    v.map(|x| x.to_string()).unwrap_or_default()
}

/// The form in a modal.
#[component]
pub(crate) fn JobForm(
    /// Pre-selected template id.
    initial_template: Option<String>,
    /// Settings to start from (another job's spec); wins over `initial_template`.
    #[prop(optional)]
    initial_spec: Option<JobSpec>,
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

    let seed = initial_spec.unwrap_or_else(|| JobSpec {
        streams: vec![JobStream {
            template_id: initial_template,
            weight: 1,
            ..JobStream::default()
        }],
        ..JobSpec::default()
    });
    let seed_streams = if seed.streams.is_empty() {
        vec![JobStream::default()]
    } else {
        seed.streams.clone()
    };
    let next_key = StoredValue::new(u32::try_from(seed_streams.len()).unwrap_or(u32::MAX));
    let streams = RwSignal::new(
        seed_streams
            .into_iter()
            .zip(0u32..)
            .map(|(s, key)| StreamRow::new(key, s))
            .collect::<Vec<_>>(),
    );
    let advanced_open =
        RwSignal::new(seed.burst.is_some() || seed.workers.is_some() || seed.signature);
    let name = RwSignal::new(seed.name.unwrap_or_default());
    let mix = RwSignal::new(if seed.mix.is_empty() {
        "weighted".to_owned()
    } else {
        seed.mix
    });
    let backend = RwSignal::new(seed.backend.unwrap_or_default());
    let iface = RwSignal::new(seed.iface.unwrap_or_default());
    let rate = RwSignal::new(seed.rate.unwrap_or_default());
    let count = RwSignal::new(num(seed.count));
    let duration = RwSignal::new(seed.duration.unwrap_or_default());
    let burst = RwSignal::new(num(seed.burst));
    let workers = RwSignal::new(num(seed.workers));
    let to_file = RwSignal::new(seed.dry_run.is_some());
    let file_name = RwSignal::new(seed.dry_run.unwrap_or_default());
    let start = RwSignal::new(true);
    let signature = RwSignal::new(seed.signature);
    let stream_id = RwSignal::new(num(seed.stream_id));
    let touched = RwSignal::new(false);
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
    // Preselect the only allowed interface, else the last used one.
    Effect::new(move |_| {
        if let Some(Ok(list)) = ifaces.get() {
            if iface.get_untracked().is_empty() {
                let allowed: Vec<String> = list
                    .iter()
                    .filter(|x| x.allowed)
                    .map(|x| x.name.clone())
                    .collect();
                if let Some(n) = pick_iface(&allowed, prefs::load_last_iface().as_deref()) {
                    iface.set(n);
                }
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
    // The pcapfile backend always writes a file.
    let file_backend = move || backend.get() == "pcapfile";
    let writes_file = move || to_file.get() || file_backend();
    let dry_value = move || {
        if writes_file() {
            file_name.get()
        } else {
            String::new()
        }
    };
    let effective_backend = move || {
        if writes_file() {
            "pcapfile".to_owned()
        } else {
            backend.get()
        }
    };
    let needs_iface = move || !is_offline_backend(&effective_backend());
    let workers_ok =
        move || backend_item().is_some_and(|b| b.multi_worker) && streams.with(Vec::len) == 1;
    let fill_file_name = move || {
        if file_name.with_untracked(|f| f.trim().is_empty()) {
            let tpl = streams.with_untracked(|s| {
                s.first()
                    .map(|r| r.template.get_untracked())
                    .unwrap_or_default()
            });
            let now = crate::dom::now_ms();
            // The browser's local time, as shown elsewhere in the UI.
            #[cfg(target_arch = "wasm32")]
            let now = now - js_sys::Date::new_0().get_timezone_offset() * 60_000.0;
            file_name.set(default_capture_name(&tpl, now));
        }
    };

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
        if writes_file() {
            validate::check_capture_name(&file_name.get())
        } else {
            Ok(())
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
            if s.iter()
                .any(|r| !r.is_inline() && r.template.with(|t| t.trim().is_empty()))
            {
                Err("ui-jobs-need-template")
            } else if s
                .iter()
                .any(|r| validate::parse_positive(&r.weight.get()).is_err())
            {
                Err("ui-jobs-bad-weight")
            } else if s
                .iter()
                .any(|r| !r.is_inline() && !r.var_defs.with(|d| r.vars.with(|v| vars_ok(d, v))))
            {
                Err("ui-jobs-bad-vars")
            } else {
                Ok(())
            }
        })
    };
    let advanced_ok =
        move || burst_check().is_ok() && workers_check().is_ok() && stream_id_check().is_ok();
    let all_ok = move || {
        rate_check().is_ok()
            && count_check().is_ok()
            && duration_check().is_ok()
            && advanced_ok()
            && capture_check().is_ok()
            && iface_check().is_ok()
            && streams_check().is_ok()
    };

    let build = move || -> CreateJob {
        let stream_list = streams.with_untracked(|s| s.iter().map(|r| r.to_stream()).collect());
        let dry = if writes_file() {
            opt(&file_name.get_untracked())
        } else {
            None
        };
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
                    if let Some(i) = &body.spec.iface {
                        prefs::save_last_iface(i);
                    }
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
        if !advanced_ok() {
            advanced_open.set(true);
        }
        if !all_ok() || busy.get_untracked() {
            return;
        }
        if needs_confirmation(
            &backend.get_untracked(),
            &dry_value(),
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
        touched.set(true);
        streams.update(|s| {
            s.push(StreamRow::new(
                key,
                JobStream {
                    weight: 1,
                    ..JobStream::default()
                },
            ));
        });
    };

    view! {
        <Modal title=tr("ui-jobs-new") on_close=on_close wide=true dirty=touched>
            <form
                on:submit=submit
                on:input=move |_| touched.set(true)
                on:change=move |_| touched.set(true)
                novalidate
                class="space-y-5"
            >
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
                                            let picker = if row.is_inline() {
                                                view! {
                                                    <span class=LABEL>{tr("ui-jobs-template")}</span>
                                                    <p class="py-2 font-mono text-sm text-slate-500">{tr("ui-job-inline-template")}</p>
                                                }
                                                    .into_any()
                                            } else {
                                                view! {
                                                    <label for=tid.clone() class=LABEL>{tr("ui-jobs-template")}</label>
                                                    <select
                                                        id=tid
                                                        class=INPUT
                                                        on:change=move |ev| {
                                                            row.vars.set(VarValues::new());
                                                            row.template.set(event_target_value(&ev));
                                                        }
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
                                                }
                                                    .into_any()
                                            };
                                            let var_fields = (!row.is_inline()).then(|| view! {
                                                <VarFields
                                                    id_prefix=format!("stream-var-{}", row.key)
                                                    template=row.template
                                                    defs=row.var_defs
                                                    values=row.vars
                                                    submitted=submitted
                                                />
                                            });
                                            view! {
                                                <div>
                                                <div class="flex flex-wrap items-end gap-2">
                                                    <div class="min-w-48 flex-1">{picker}</div>
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
                                                            on:click=move |_| {
                                                                touched.set(true);
                                                                streams.update(|s| s.retain(|r| r.key != row.key));
                                                            }
                                                        >
                                                            <Icon name=IconName::Trash class="h-4 w-4" />
                                                        </button>
                                                    </Show>
                                                </div>
                                                {var_fields}
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
                    <div class="mt-4">
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

                // Send to
                <fieldset class="grid gap-4 sm:grid-cols-2">
                    <legend class="mb-2 text-sm font-semibold sm:col-span-2">{tr("ui-jobs-target")}</legend>
                    <div>
                        <label for="job-backend" class=LABEL>{tr("ui-jobs-backend")}</label>
                        {move || match backends.get() {
                            None => view! { <Loading /> }.into_any(),
                            Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                            Some(Ok(b)) => {
                                view! {
                                    <select
                                        id="job-backend"
                                        class=INPUT
                                        on:change=move |ev| {
                                            backend.set(event_target_value(&ev));
                                            if backend.get_untracked() == "pcapfile" {
                                                fill_file_name();
                                            }
                                        }
                                    >
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
                        <label class="flex items-center gap-2 text-sm">
                            <input
                                type="checkbox"
                                prop:checked=writes_file
                                disabled=file_backend
                                on:change=move |ev| {
                                    let on = event_target_checked(&ev);
                                    to_file.set(on);
                                    if on {
                                        fill_file_name();
                                    }
                                }
                            />
                            {tr("ui-jobs-dry-run")}
                        </label>
                        <Show when=move || writes_file()>
                            <div class="mt-2">
                                <label for="job-dry" class=LABEL>{tr("ui-jobs-dry-run-file")}</label>
                                <input
                                    id="job-dry"
                                    class=move || input_class(capture_check().is_ok())
                                    aria-describedby="job-dry-hint"
                                    prop:value=move || file_name.get()
                                    on:input=move |ev| file_name.set(event_target_value(&ev))
                                />
                                <p id="job-dry-hint" class="mt-1 text-xs text-slate-500">{tr("ui-jobs-dry-run-hint")}</p>
                                {move || field_error(capture_check())}
                            </div>
                        </Show>
                    </div>
                </fieldset>

                // Rate & limits
                <fieldset class="grid gap-4 sm:grid-cols-2">
                    <legend class="mb-2 text-sm font-semibold sm:col-span-2">{tr("ui-jobs-how")}</legend>
                    <div class="sm:col-span-2">
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
                </fieldset>

                // Advanced: burst, workers, receive verification (ROADMAP M9)
                <details
                    class="rounded-md border border-slate-200 px-4 py-2 dark:border-slate-800"
                    prop:open=move || advanced_open.get()
                >
                    <summary class="cursor-pointer text-sm font-semibold">{tr("ui-jobs-advanced")}</summary>
                    <div class="mt-3 space-y-5 pb-2">
                        <div class="grid gap-4 sm:grid-cols-2">
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
                        </div>
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
                    </div>
                </details>

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

    #[test]
    fn iface_preselection() {
        let one = vec!["eth0".to_owned()];
        let two = vec!["eth0".to_owned(), "eth1".to_owned()];
        assert_eq!(pick_iface(&one, None), Some("eth0".to_owned()));
        assert_eq!(pick_iface(&one, Some("eth9")), Some("eth0".to_owned()));
        assert_eq!(pick_iface(&two, None), None);
        assert_eq!(pick_iface(&two, Some("eth1")), Some("eth1".to_owned()));
        assert_eq!(pick_iface(&two, Some("eth9")), None, "no longer allowed");
        assert_eq!(pick_iface(&[], Some("eth0")), None);
    }

    #[test]
    fn capture_names() {
        assert_eq!(
            default_capture_name("protocols-l4-udp", 0.0),
            "protocols-l4-udp-19700101-000000.pcapng"
        );
        assert_eq!(
            default_capture_name("a/b c", 1_700_000_000_000.0),
            "a-b-c-20231114-221320.pcapng"
        );
        assert_eq!(
            default_capture_name("", 951_782_400_000.0),
            "job-20000229-000000.pcapng"
        );
        assert!(validate::check_capture_name(&default_capture_name("x", 0.0)).is_ok());
    }
}
