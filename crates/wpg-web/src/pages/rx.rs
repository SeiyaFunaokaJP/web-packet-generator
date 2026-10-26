//! `/rx`: receive / verification screen (ROADMAP M9 "GUI 受信画面"): start a capture
//! (backend, interface or capture file, live-validated filter, limits, pcapng
//! file, signature verification), the list of captures, and the selected
//! capture's live counters, per-stream verification table, first N frames and a
//! decoded view of one frame.

use leptos::prelude::*;
use wpg_core::notation::format_value;
use wpg_core::{DiagLevel, FieldLayout, LayerLayout};

use crate::api::{
    self, ApiError, CaptureInfo, CaptureSpec, CapturedFrame, JobStatus, RxBackendInfo, RxStats,
    StreamVerify,
};
use crate::builder::preview::range_at_byte;
use crate::builder::proto;
use crate::components::{
    reason_key, use_toasts, Confirm, Empty, ErrorBox, Icon, IconName, Loading, PageHeader, Stat,
    StateBadge, BTN, BTN_DANGER, BTN_GHOST, BTN_PRIMARY, CARD, INPUT, INPUT_INVALID, LABEL, TD,
    TD_NUM, TH,
};
use crate::fmt;
use crate::i18n::{tr, use_i18n, I18n};
use crate::validate::{self, Invalid};
use crate::ws::use_live;

/// Most frames a capture may keep (`wpg_server::rx::MAX_KEEP`).
pub(crate) const MAX_KEEP: usize = 10_000;
/// Frames kept when the field is empty (`wpg_server::rx::DEFAULT_KEEP`).
pub(crate) const DEFAULT_KEEP: usize = 100;
/// Frame rows rendered at first ("show more" adds this many).
const PAGE_ROWS: usize = 200;
/// Delay before a filter edit is sent for validation.
const FILTER_DEBOUNCE_MS: u64 = 250;

/// The backend pre-selected in the form: a real interface backend when one is
/// available, else the capture-file replay.
pub(crate) fn default_backend(list: &[RxBackendInfo]) -> String {
    ["afpacket", "pcap"]
        .into_iter()
        .find(|n| list.iter().any(|b| b.name == *n && b.available))
        .unwrap_or("pcapfile")
        .to_owned()
}

/// `keep` field: empty = the server default, `0` = keep nothing, at most
/// [`MAX_KEEP`].
pub(crate) fn parse_keep(s: &str) -> Result<usize, Invalid> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(DEFAULT_KEEP);
    }
    if t == "0" {
        return Ok(0);
    }
    let n = validate::parse_positive(t)?;
    usize::try_from(n)
        .ok()
        .filter(|n| *n <= MAX_KEEP)
        .ok_or("ui-rx-keep-max")
}

/// Character (not byte) position of a filter error, for the caret marker.
pub(crate) fn char_pos(text: &str, byte_pos: usize) -> usize {
    let mut end = byte_pos.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].chars().count()
}

/// Hex text → octets (ignores a trailing odd nibble / invalid pairs).
pub(crate) fn hex_bytes(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .filter_map(|p| std::str::from_utf8(p).ok())
        .filter_map(|p| u8::from_str_radix(p, 16).ok())
        .collect()
}

/// Seconds between two nanosecond timestamps (`0.000123`).
pub(crate) fn rel_secs(first_ns: u64, ts_ns: u64) -> String {
    #[allow(clippy::cast_precision_loss)] // display only
    let d = ts_ns.saturating_sub(first_ns) as f64 / 1e9;
    format!("{d:.6}")
}

fn opt(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_owned())
}

/// The raw form values (pure conversion to a [`CaptureSpec`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct FormValues {
    pub(crate) name: String,
    pub(crate) backend: String,
    pub(crate) iface: String,
    pub(crate) filter: String,
    pub(crate) count: String,
    pub(crate) duration: String,
    pub(crate) keep: String,
    pub(crate) save: String,
    pub(crate) verify: bool,
    pub(crate) promisc: bool,
    pub(crate) include_outgoing: bool,
}

impl FormValues {
    /// Whether the backend reads a capture file instead of an interface.
    fn file_input(&self) -> bool {
        self.backend == "pcapfile"
    }

    /// The interface / file field (`Err(i18n key)`).
    pub(crate) fn iface_check(&self) -> Result<(), Invalid> {
        match self.backend.as_str() {
            "pcapfile" => validate::check_capture_name(&self.iface),
            "null" => Ok(()),
            _ if self.iface.trim().is_empty() => Err("ui-jobs-need-iface"),
            _ => Ok(()),
        }
    }

    /// The pcapng output name (`Err(i18n key)`).
    pub(crate) fn save_check(&self) -> Result<(), Invalid> {
        validate::optional(&self.save, validate::check_capture_name)?;
        if self.file_input()
            && !self.save.trim().is_empty()
            && self.save.trim() == self.iface.trim()
        {
            return Err("ui-rx-save-same");
        }
        Ok(())
    }

    pub(crate) fn count_check(&self) -> Result<(), Invalid> {
        validate::optional(&self.count, validate::parse_positive)
    }

    pub(crate) fn duration_check(&self) -> Result<(), Invalid> {
        validate::optional(&self.duration, validate::parse_duration)
    }

    pub(crate) fn keep_check(&self) -> Result<(), Invalid> {
        parse_keep(&self.keep).map(|_| ())
    }

    pub(crate) fn all_ok(&self) -> bool {
        self.iface_check().is_ok()
            && self.save_check().is_ok()
            && self.count_check().is_ok()
            && self.duration_check().is_ok()
            && self.keep_check().is_ok()
    }

    /// The `POST /api/captures` body.
    pub(crate) fn spec(&self) -> CaptureSpec {
        CaptureSpec {
            name: opt(&self.name),
            backend: opt(&self.backend),
            iface: if self.backend == "null" {
                None
            } else {
                opt(&self.iface)
            },
            filter: opt(&self.filter),
            count: validate::parse_positive(&self.count).ok(),
            duration: opt(&self.duration),
            keep: parse_keep(&self.keep).unwrap_or(DEFAULT_KEEP),
            save: opt(&self.save),
            verify: self.verify,
            promisc: self.promisc,
            include_outgoing: self.include_outgoing,
        }
    }
}

/// State of the live filter check.
#[derive(Debug, Clone, PartialEq)]
enum FilterUi {
    /// Empty filter: every frame matches.
    Empty,
    Checking,
    Valid(String),
    Invalid {
        message: String,
        pos: usize,
    },
    /// The check itself failed (network); the server re-checks on start.
    Unchecked(String),
}

/// A capture's state, preferring a newer one pushed over the WebSocket.
fn live_status(id: StoredValue<String>, initial: JobStatus) -> Signal<JobStatus> {
    let live = use_live();
    Signal::derive(move || {
        live.capture_states
            .with(|m| m.get(&id.get_value()).cloned())
            .filter(|s| !initial.is_terminal() || s.is_terminal())
            .unwrap_or_else(|| initial.clone())
    })
}

/// A capture's latest statistics (WebSocket, else the fetched snapshot).
fn live_stats(id: StoredValue<String>, initial: Option<RxStats>) -> Signal<Option<RxStats>> {
    let live = use_live();
    Signal::derive(move || live.capture(&id.get_value()).or_else(|| initial.clone()))
}

/// Localised finish reason / error of a capture.
fn status_detail(i18n: I18n, s: &JobStatus) -> Option<String> {
    s.detail
        .clone()
        .map(|d| reason_key(&d).map_or(d, |k| i18n.t(k)))
}

#[component]
pub(crate) fn RxPage() -> impl IntoView {
    let live = use_live();
    let i18n = use_i18n();
    let toasts = use_toasts();
    let selected = RwSignal::new(None::<String>);
    let captures = LocalResource::new(move || {
        live.captures_rev.track();
        api::captures()
    });
    // (id, name) awaiting delete confirmation.
    let deleting = RwSignal::new(None::<(String, String)>);
    let busy = RwSignal::new(false);

    let stop = Callback::new(move |id: String| {
        leptos::task::spawn_local(async move {
            match api::stop_capture(&id).await {
                Ok(_) => live.captures_rev.update(|r| *r += 1),
                Err(e) => toasts.error(e.message(i18n)),
            }
        });
    });
    let ask_delete = Callback::new(move |target: (String, String)| deleting.set(Some(target)));
    let delete = move || {
        let Some((id, _)) = deleting.get_untracked() else {
            return;
        };
        busy.set(true);
        leptos::task::spawn_local(async move {
            match api::delete_capture(&id).await {
                Ok(()) => {
                    live.forget_capture(&id);
                    if selected.get_untracked().as_deref() == Some(id.as_str()) {
                        selected.set(None);
                    }
                    live.captures_rev.update(|r| *r += 1);
                    toasts.ok(i18n.t("ui-rx-deleted"));
                }
                Err(e) => toasts.error(e.message(i18n)),
            }
            busy.set(false);
            deleting.set(None);
        });
    };

    view! {
        <PageHeader title=tr("ui-nav-rx") />
        <div class="space-y-6">
            <StartForm on_started=move |info: CaptureInfo| {
                live.captures_rev.update(|r| *r += 1);
                selected.set(Some(info.id));
            } />

            <section aria-labelledby="rx-list-h">
                <h2 id="rx-list-h" class="mb-2 text-base font-semibold">{tr("ui-rx-list")}</h2>
                {move || match captures.get() {
                    None => view! { <Loading /> }.into_any(),
                    Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                    Some(Ok(list)) if list.is_empty() => view! { <Empty text=tr("ui-rx-empty") /> }.into_any(),
                    Some(Ok(list)) => view! {
                        <CaptureTable list=list selected=selected on_stop=stop on_delete=ask_delete />
                    }.into_any(),
                }}
            </section>

            {move || selected.get().map(|id| view! {
                <CaptureDetail id=id on_stop=stop on_delete=ask_delete />
            })}
        </div>

        <Show when=move || deleting.with(Option::is_some)>
            <Confirm
                title=tr("ui-rx-delete-title")
                confirm_label=tr("ui-delete")
                danger=true
                busy=busy
                on_confirm=move || delete()
                on_cancel=move || deleting.set(None)
            >
                <p>{tr("ui-rx-delete-body")}</p>
                <p class="font-mono text-xs">{move || deleting.get().map(|(_, n)| n)}</p>
            </Confirm>
        </Show>
    }
}

/// The "new capture" form.
#[component]
fn StartForm(#[prop(into)] on_started: Callback<CaptureInfo>) -> impl IntoView {
    let i18n = use_i18n();
    let toasts = use_toasts();
    let backends = LocalResource::new(api::rx_backends);
    let ifaces = LocalResource::new(|| api::ifaces(false));

    let name = RwSignal::new(String::new());
    let backend = RwSignal::new(String::new());
    let iface = RwSignal::new(String::new());
    let filter = RwSignal::new(String::new());
    let count = RwSignal::new(String::new());
    let duration = RwSignal::new(String::new());
    let keep = RwSignal::new(DEFAULT_KEEP.to_string());
    let save = RwSignal::new(String::new());
    let verify = RwSignal::new(true);
    let promisc = RwSignal::new(false);
    let outgoing = RwSignal::new(false);
    let submitted = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<ApiError>);
    let filter_ui = RwSignal::new(FilterUi::Empty);

    // Default backend once known.
    Effect::new(move |_| {
        if let Some(Ok(list)) = backends.get() {
            if backend.get_untracked().is_empty() {
                backend.set(default_backend(&list));
            }
        }
    });
    // A different backend means a different kind of interface / file.
    Effect::new(move |prev: Option<String>| {
        let b = backend.get();
        if prev.is_some_and(|p| p != b) {
            iface.set(String::new());
        }
        b
    });

    // Live, debounced filter validation.
    let gen = StoredValue::new(0u32);
    Effect::new(move |_| {
        let text = filter.get();
        let g = gen.get_value().wrapping_add(1);
        gen.set_value(g);
        if text.trim().is_empty() {
            filter_ui.set(FilterUi::Empty);
            return;
        }
        filter_ui.set(FilterUi::Checking);
        set_timeout(
            move || {
                if gen.get_value() != g {
                    return;
                }
                leptos::task::spawn_local(async move {
                    let res = api::check_filter(&text).await;
                    if gen.get_value() != g {
                        return;
                    }
                    filter_ui.set(match res {
                        Ok(r) if r.ok => FilterUi::Valid(r.normalized.unwrap_or(text)),
                        Ok(r) => FilterUi::Invalid {
                            message: r.error.unwrap_or_default(),
                            pos: char_pos(&text, r.pos.unwrap_or(0)),
                        },
                        Err(e) => FilterUi::Unchecked(e.message(i18n)),
                    });
                });
            },
            std::time::Duration::from_millis(FILTER_DEBOUNCE_MS),
        );
    });

    let values = move || FormValues {
        name: name.get(),
        backend: backend.get(),
        iface: iface.get(),
        filter: filter.get(),
        count: count.get(),
        duration: duration.get(),
        keep: keep.get(),
        save: save.get(),
        verify: verify.get(),
        promisc: promisc.get(),
        include_outgoing: outgoing.get(),
    };
    let filter_bad = move || filter_ui.with(|f| matches!(f, FilterUi::Invalid { .. }));

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        submitted.set(true);
        let v = values();
        if !v.all_ok() || filter_bad() || busy.get_untracked() {
            return;
        }
        let spec = v.spec();
        busy.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            match api::create_capture(&spec).await {
                Ok(info) => {
                    toasts.ok(i18n.ta("ui-rx-started", &[("id", info.name.clone())]));
                    on_started.run(info);
                }
                Err(e) => error.set(Some(e)),
            }
            busy.set(false);
        });
    };

    let field_error = move |r: Result<(), Invalid>, id: &'static str| {
        r.err().filter(|_| submitted.get()).map(|k| {
            view! { <p id=id class="mt-1 text-xs text-rose-600 dark:text-rose-400">{move || i18n.t(k)}</p> }
        })
    };
    let input_class = move |ok: bool| {
        if ok || !submitted.get() {
            INPUT
        } else {
            INPUT_INVALID
        }
    };
    let invalid = move |ok: bool| (!ok && submitted.get()).to_string();

    let iface_field = move || {
        let b = backend.get();
        match b.as_str() {
            "pcapfile" => view! {
                <input
                    id="rx-iface"
                    class=move || input_class(values().iface_check().is_ok())
                    placeholder="capture.pcapng"
                    aria-describedby="rx-iface-hint rx-iface-err"
                    aria-invalid=move || invalid(values().iface_check().is_ok())
                    prop:value=move || iface.get()
                    on:input=move |ev| iface.set(event_target_value(&ev))
                />
                <p id="rx-iface-hint" class="mt-1 text-xs text-slate-500">{tr("ui-rx-file-hint")}</p>
            }
            .into_any(),
            "null" => view! {
                <input id="rx-iface" class=INPUT disabled=true placeholder=tr("ui-jobs-iface-not-needed") />
            }
            .into_any(),
            _ => {
                let any = b == "afpacket";
                view! {
                    {move || match ifaces.get() {
                        None => view! { <Loading /> }.into_any(),
                        Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                        Some(Ok(list)) => view! {
                            <select
                                id="rx-iface"
                                class=move || input_class(values().iface_check().is_ok())
                                aria-describedby="rx-iface-err"
                                aria-invalid=move || invalid(values().iface_check().is_ok())
                                on:change=move |ev| iface.set(event_target_value(&ev))
                            >
                                <option value="" selected=move || iface.with(String::is_empty)>
                                    {tr("ui-jobs-choose-iface")}
                                </option>
                                {any.then(|| view! {
                                    <option value="any" selected=move || iface.get() == "any">{tr("ui-rx-iface-any")}</option>
                                })}
                                {list
                                    .into_iter()
                                    .map(|x| {
                                        let n = x.name.clone();
                                        let n2 = x.name.clone();
                                        let state = if x.up && x.running { "▲" } else { "▽" };
                                        view! {
                                            <option value=n selected=move || iface.get() == n2>
                                                {format!("{} {state}", x.name)}
                                            </option>
                                        }
                                    })
                                    .collect_view()}
                            </select>
                        }
                        .into_any(),
                    }}
                }
                .into_any()
            }
        }
    };

    let filter_hint = move || {
        let (cls, text, caret) = match filter_ui.get() {
            FilterUi::Empty => ("text-slate-500", i18n.t("ui-rx-filter-empty"), None),
            FilterUi::Checking => ("text-slate-500", i18n.t("ui-rx-filter-checking"), None),
            FilterUi::Valid(n) => (
                "text-emerald-700 dark:text-emerald-400",
                i18n.ta("ui-rx-filter-ok", &[("filter", n)]),
                None,
            ),
            FilterUi::Invalid { message, pos } => (
                "text-rose-600 dark:text-rose-400",
                i18n.ta(
                    "ui-rx-filter-error",
                    &[("error", message), ("offset", (pos + 1).to_string())],
                ),
                Some(pos),
            ),
            FilterUi::Unchecked(e) => ("text-amber-700 dark:text-amber-400", e, None),
        };
        view! {
            <p class=format!("mt-1 text-xs {cls}")>{text}</p>
            {caret.map(|pos| {
                let text = filter.get_untracked();
                view! {
                    <pre class="mt-1 overflow-x-auto rounded bg-slate-100 px-2 py-1 font-mono text-xs dark:bg-slate-800" aria-hidden="true">
                        {text}"\n"{" ".repeat(pos)}<span class="text-rose-600">"^"</span>
                    </pre>
                }
            })}
        }
    };

    let checkbox = move |sig: RwSignal<bool>, key: &'static str| {
        view! {
            <label class="flex items-center gap-2 text-sm">
                <input
                    type="checkbox"
                    prop:checked=move || sig.get()
                    on:change=move |ev| sig.set(event_target_checked(&ev))
                />
                {tr(key)}
            </label>
        }
    };

    view! {
        <section aria-labelledby="rx-new-h" class=CARD>
            <h2 id="rx-new-h" class="mb-3 text-base font-semibold">{tr("ui-rx-new")}</h2>
            <form on:submit=submit novalidate class="space-y-4">
                <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
                    <div>
                        <label for="rx-backend" class=LABEL>{tr("ui-jobs-backend")}</label>
                        {move || match backends.get() {
                            None => view! { <Loading /> }.into_any(),
                            Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                            Some(Ok(list)) => view! {
                                <select id="rx-backend" class=INPUT on:change=move |ev| backend.set(event_target_value(&ev))>
                                    {list
                                        .into_iter()
                                        .map(|x| {
                                            let n = x.name.clone();
                                            let n2 = x.name.clone();
                                            let reason = x.reason.clone().unwrap_or_default();
                                            let label = if x.available {
                                                x.name.clone()
                                            } else {
                                                format!("{} — {reason}", x.name)
                                            };
                                            view! {
                                                <option value=n disabled=!x.available title=reason selected=move || backend.get() == n2>
                                                    {label}
                                                </option>
                                            }
                                        })
                                        .collect_view()}
                                </select>
                            }
                            .into_any(),
                        }}
                        <p class="mt-1 text-xs text-slate-500">
                            {move || {
                                let b = backend.get();
                                backends
                                    .get()
                                    .and_then(Result::ok)
                                    .and_then(|l| l.into_iter().find(|x| x.name == b))
                                    .and_then(|x| x.reason)
                            }}
                        </p>
                    </div>
                    <div>
                        <label for="rx-iface" class=LABEL>
                            {move || i18n.t(if backend.get() == "pcapfile" { "ui-rx-file" } else { "ui-jobs-iface" })}
                        </label>
                        {iface_field}
                        {move || field_error(values().iface_check(), "rx-iface-err")}
                    </div>
                    <div class="sm:col-span-2">
                        <label for="rx-filter" class=LABEL>{tr("ui-rx-filter")}</label>
                        <input
                            id="rx-filter"
                            class=move || if filter_bad() { INPUT_INVALID } else { INPUT }
                            placeholder="udp port 5001 and wpg"
                            spellcheck="false"
                            autocomplete="off"
                            aria-describedby="rx-filter-hint"
                            aria-invalid=move || filter_bad().to_string()
                            prop:value=move || filter.get()
                            on:input=move |ev| filter.set(event_target_value(&ev))
                        />
                        <div id="rx-filter-hint" aria-live="polite">{filter_hint}</div>
                    </div>
                    <div>
                        <label for="rx-count" class=LABEL>{tr("ui-jobs-count")}</label>
                        <input
                            id="rx-count"
                            class=move || input_class(values().count_check().is_ok())
                            inputmode="numeric"
                            placeholder=tr("ui-unlimited")
                            aria-describedby="rx-count-hint"
                            aria-invalid=move || invalid(values().count_check().is_ok())
                            prop:value=move || count.get()
                            on:input=move |ev| count.set(event_target_value(&ev))
                        />
                        <p id="rx-count-hint" class="mt-1 text-xs text-slate-500">{tr("ui-rx-count-hint")}</p>
                        {move || field_error(values().count_check(), "rx-count-err")}
                    </div>
                    <div>
                        <label for="rx-duration" class=LABEL>{tr("ui-jobs-duration")}</label>
                        <input
                            id="rx-duration"
                            class=move || input_class(values().duration_check().is_ok())
                            placeholder="10s · 5m"
                            aria-invalid=move || invalid(values().duration_check().is_ok())
                            prop:value=move || duration.get()
                            on:input=move |ev| duration.set(event_target_value(&ev))
                        />
                        {move || field_error(values().duration_check(), "rx-duration-err")}
                    </div>
                    <div>
                        <label for="rx-keep" class=LABEL>{tr("ui-rx-keep")}</label>
                        <input
                            id="rx-keep"
                            class=move || input_class(values().keep_check().is_ok())
                            inputmode="numeric"
                            aria-describedby="rx-keep-hint"
                            aria-invalid=move || invalid(values().keep_check().is_ok())
                            prop:value=move || keep.get()
                            on:input=move |ev| keep.set(event_target_value(&ev))
                        />
                        <p id="rx-keep-hint" class="mt-1 text-xs text-slate-500">{tr("ui-rx-keep-hint")}</p>
                        {move || field_error(values().keep_check(), "rx-keep-err")}
                    </div>
                    <div>
                        <label for="rx-save" class=LABEL>{tr("ui-rx-save")}</label>
                        <input
                            id="rx-save"
                            class=move || input_class(values().save_check().is_ok())
                            placeholder="rx.pcapng"
                            aria-describedby="rx-save-hint"
                            aria-invalid=move || invalid(values().save_check().is_ok())
                            prop:value=move || save.get()
                            on:input=move |ev| save.set(event_target_value(&ev))
                        />
                        <p id="rx-save-hint" class="mt-1 text-xs text-slate-500">{tr("ui-rx-save-hint")}</p>
                        {move || field_error(values().save_check(), "rx-save-err")}
                    </div>
                    <div class="sm:col-span-2">
                        <label for="rx-name" class=LABEL>{tr("ui-jobs-name")}</label>
                        <input
                            id="rx-name"
                            class=INPUT
                            placeholder=tr("ui-rx-name-placeholder")
                            prop:value=move || name.get()
                            on:input=move |ev| name.set(event_target_value(&ev))
                        />
                    </div>
                    <fieldset class="flex flex-col justify-end gap-1 sm:col-span-2">
                        <legend class="sr-only">{tr("ui-rx-options")}</legend>
                        {checkbox(verify, "ui-rx-verify")}
                        {checkbox(promisc, "ui-rx-promisc")}
                        {checkbox(outgoing, "ui-rx-outgoing")}
                    </fieldset>
                </div>

                {move || error.get().map(|e| view! { <ErrorBox error=e /> })}

                <div class="flex justify-end">
                    <button type="submit" class=BTN_PRIMARY disabled=move || busy.get()>
                        <Icon name=IconName::Play class="h-4 w-4" />
                        {tr("ui-rx-start")}
                    </button>
                </div>
            </form>
        </section>
    }
}

/// The captures table.
#[component]
fn CaptureTable(
    list: Vec<CaptureInfo>,
    selected: RwSignal<Option<String>>,
    #[prop(into)] on_stop: Callback<String>,
    #[prop(into)] on_delete: Callback<(String, String)>,
) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <div class="overflow-x-auto rounded-lg border border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
            <table class="min-w-full divide-y divide-slate-200 dark:divide-slate-800">
                <caption class="sr-only">{tr("ui-rx-list")}</caption>
                <thead>
                    <tr>
                        <th scope="col" class=TH>{tr("ui-jobs-name")}</th>
                        <th scope="col" class=TH>{tr("ui-job-state")}</th>
                        <th scope="col" class=TH>{tr("ui-rx-source")}</th>
                        <th scope="col" class=format!("{TH} text-right")>{tr("ui-rx-matched")}</th>
                        <th scope="col" class=format!("{TH} text-right max-sm:hidden")>{tr("ui-stat-packets")}</th>
                        <th scope="col" class=format!("{TH} text-right")>{tr("ui-rx-lost")}</th>
                        <th scope="col" class=format!("{TH} max-lg:hidden")>{tr("ui-job-started")}</th>
                        <th scope="col" class=TH><span class="sr-only">{tr("ui-rx-actions")}</span></th>
                    </tr>
                </thead>
                <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                    {list
                        .into_iter()
                        .map(|c| {
                            let id = StoredValue::new(c.id.clone());
                            let name = StoredValue::new(c.name.clone());
                            let status = live_status(id, c.status.clone());
                            let stats = live_stats(id, c.stats.clone());
                            let verify = c.spec.verify;
                            let started = c.started;
                            let is_sel = move || selected.get().as_deref() == Some(id.get_value().as_str());
                            view! {
                                <tr class=move || if is_sel() { "bg-sky-50 dark:bg-sky-500/10" } else { "hover:bg-slate-50 dark:hover:bg-slate-800/50" }>
                                    <td class=TD>
                                        <button
                                            type="button"
                                            class="text-left font-medium text-sky-700 hover:underline dark:text-sky-400"
                                            aria-current=move || is_sel().then_some("true")
                                            on:click=move |_| selected.set(Some(id.get_value()))
                                        >
                                            {c.name.clone()}
                                        </button>
                                        <span class="block max-w-64 truncate font-mono text-xs text-slate-400" title=c.filter.clone()>
                                            {if c.filter.is_empty() { "—".to_owned() } else { c.filter.clone() }}
                                        </span>
                                    </td>
                                    <td class=TD><StateBadge status=status /></td>
                                    <td class=format!("{TD} text-xs")>
                                        <span class="font-mono">{c.backend.clone()}</span>
                                        <span class="block max-w-48 truncate font-mono text-slate-500" title=c.iface.clone()>{c.iface.clone()}</span>
                                    </td>
                                    <td class=TD_NUM>{move || stats.get().map_or_else(|| "–".to_owned(), |s| fmt::int(i18n.locale.get(), s.matched))}</td>
                                    <td class=format!("{TD_NUM} max-sm:hidden")>{move || stats.get().map_or_else(|| "–".to_owned(), |s| fmt::int(i18n.locale.get(), s.packets))}</td>
                                    <td class=TD_NUM>
                                        {move || {
                                            stats
                                                .get()
                                                .and_then(|s| s.verify)
                                                .filter(|_| verify)
                                                .map_or_else(|| "–".to_owned(), |v| fmt::int(i18n.locale.get(), v.lost()))
                                        }}
                                    </td>
                                    <td class=format!("{TD} text-xs text-slate-500 max-lg:hidden")>
                                        {move || fmt::datetime(i18n.locale.get(), started)}
                                    </td>
                                    <td class=format!("{TD} whitespace-nowrap text-right")>
                                        <Show when=move || status.with(|s| !s.is_terminal())>
                                            <button
                                                type="button"
                                                class=BTN_GHOST
                                                title=tr("ui-rx-stop")
                                                aria-label=move || i18n.ta("ui-rx-stop-named", &[("id", name.get_value())])
                                                on:click=move |_| on_stop.run(id.get_value())
                                            >
                                                <Icon name=IconName::Stop class="h-4 w-4" />
                                            </button>
                                        </Show>
                                        <button
                                            type="button"
                                            class=BTN_GHOST
                                            title=tr("ui-delete")
                                            aria-label=move || i18n.ta("ui-rx-delete-named", &[("id", name.get_value())])
                                            on:click=move |_| on_delete.run((id.get_value(), name.get_value()))
                                        >
                                            <Icon name=IconName::Trash class="h-4 w-4" />
                                        </button>
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

/// The selected capture: counters, verification, frames.
#[component]
fn CaptureDetail(
    id: String,
    #[prop(into)] on_stop: Callback<String>,
    #[prop(into)] on_delete: Callback<(String, String)>,
) -> impl IntoView {
    let live = use_live();
    let id = StoredValue::new(id);
    let info = LocalResource::new(move || {
        live.captures_rev.track();
        let id = id.get_value();
        async move { api::capture(&id).await }
    });
    // Re-render the body only when the capture ends (or fails to load), not on
    // every refetch: the counters are live anyway, and a selected frame stays put.
    let shape = Memo::new(move |_| {
        info.get()
            .map(|r| r.map(|c| c.status.is_terminal()).map_err(|_| ()))
    });
    view! {
        {move || match shape.get() {
            None => view! { <Loading /> }.into_any(),
            Some(Err(())) => match info.get_untracked() {
                Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
                _ => view! { <Loading /> }.into_any(),
            },
            Some(Ok(_)) => match info.get_untracked() {
                Some(Ok(c)) => view! { <DetailBody info=c on_stop=on_stop on_delete=on_delete /> }.into_any(),
                _ => view! { <Loading /> }.into_any(),
            },
        }}
    }
}

#[component]
fn DetailBody(
    info: CaptureInfo,
    #[prop(into)] on_stop: Callback<String>,
    #[prop(into)] on_delete: Callback<(String, String)>,
) -> impl IntoView {
    let i18n = use_i18n();
    let id = StoredValue::new(info.id.clone());
    let name = StoredValue::new(info.name.clone());
    let status = live_status(id, info.status.clone());
    let stats = live_stats(id, info.stats.clone());
    let saved = info.saved;
    let verify = info.spec.verify;

    let num = move |f: fn(&RxStats) -> u64| {
        Signal::derive(move || {
            stats
                .get()
                .map_or_else(|| "–".to_owned(), |s| fmt::int(i18n.locale.get(), f(&s)))
        })
    };
    let rate = move |f: fn(&RxStats) -> f64, unit: &'static str| {
        Signal::derive(move || {
            stats
                .get()
                .map_or_else(|| "–".to_owned(), |s| fmt::si(f(&s), unit))
        })
    };
    let size = move |f: fn(&RxStats) -> u64| {
        Signal::derive(move || {
            stats
                .get()
                .map_or_else(|| "–".to_owned(), |s| fmt::bytes(f(&s)))
        })
    };

    view! {
        <section aria-labelledby="rx-detail-h" class=format!("{CARD} space-y-6")>
            <div class="flex flex-wrap items-center gap-2">
                <h2 id="rx-detail-h" class="mr-2 text-base font-semibold">{info.name.clone()}</h2>
                <StateBadge status=status />
                <span class="text-xs text-slate-500">{move || status.with(|s| status_detail(i18n, s))}</span>
                <div class="ml-auto flex flex-wrap gap-2">
                    <Show when=move || status.with(|s| !s.is_terminal())>
                        <button type="button" class=BTN on:click=move |_| on_stop.run(id.get_value())>
                            <Icon name=IconName::Stop class="h-4 w-4" />
                            {tr("ui-rx-stop")}
                        </button>
                    </Show>
                    <Show when=move || saved && status.with(JobStatus::is_terminal)>
                        <a class=BTN href=move || api::capture_pcapng_url(&id.get_value()) download="">
                            <Icon name=IconName::Download class="h-4 w-4" />
                            {tr("ui-rx-download")}
                        </a>
                    </Show>
                    <button type="button" class=BTN_DANGER on:click=move |_| on_delete.run((id.get_value(), name.get_value()))>
                        <Icon name=IconName::Trash class="h-4 w-4" />
                        {tr("ui-delete")}
                    </button>
                </div>
            </div>

            <dl class="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1 text-sm sm:grid-cols-[auto_1fr_auto_1fr]">
                <dt class="text-slate-500">{tr("ui-jobs-backend")}</dt><dd class="font-mono">{info.backend.clone()}</dd>
                <dt class="text-slate-500">{tr("ui-rx-source")}</dt><dd class="font-mono break-all">{info.iface.clone()}</dd>
                <dt class="text-slate-500">{tr("ui-rx-filter")}</dt>
                <dd class="font-mono break-all">{if info.filter.is_empty() { "—".to_owned() } else { info.filter.clone() }}</dd>
                <dt class="text-slate-500">{tr("ui-job-started")}</dt>
                <dd>{move || fmt::datetime(i18n.locale.get(), info.started)}</dd>
            </dl>

            <section aria-labelledby="rx-counters-h">
                <h3 id="rx-counters-h" class="mb-2 text-sm font-semibold">{tr("ui-rx-counters")}</h3>
                <dl class="grid grid-cols-2 gap-4 sm:grid-cols-4 lg:grid-cols-6">
                    <Stat label=tr("ui-rx-matched") value=num(|s| s.matched) />
                    <Stat label=tr("ui-rx-matched-pps") value=rate(|s| s.matched_pps, "pps") />
                    <Stat label=tr("ui-rx-matched-bps") value=rate(|s| s.matched_bps, "bps") />
                    <Stat label=tr("ui-rx-matched-bytes") value=size(|s| s.matched_bytes) />
                    <Stat label=tr("ui-stat-dropped") value=num(|s| s.dropped) />
                    <Stat label=tr("ui-stat-elapsed") value=Signal::derive(move || stats.get().map_or_else(|| "–".to_owned(), |s| fmt::duration(s.t.as_secs_f64()))) />
                    <Stat label=tr("ui-stat-packets") value=num(|s| s.packets) />
                    <Stat label=tr("ui-stat-pps") value=rate(|s| s.pps, "pps") />
                    <Stat label=tr("ui-stat-bps") value=rate(|s| s.bps, "bps") />
                    <Stat label=tr("ui-stat-bytes") value=size(|s| s.bytes) />
                    <Stat label=tr("ui-rx-saved") value=num(|s| s.saved) />
                    <Stat label=tr("ui-rx-kept") value=Signal::derive(move || stats.get().map_or_else(|| "–".to_owned(), |s| s.kept.to_string())) />
                </dl>
            </section>

            <VerifySection stats=stats enabled=verify />
            <FramesSection id=id.get_value() stats=stats status=status />
        </section>
    }
}

/// Per-stream verification table.
#[component]
fn VerifySection(stats: Signal<Option<RxStats>>, enabled: bool) -> impl IntoView {
    let i18n = use_i18n();
    let report = move || stats.get().and_then(|s| s.verify).unwrap_or_default();
    let body = move || {
        if !enabled {
            return view! { <p class="text-xs text-slate-500">{tr("ui-rx-verify-off")}</p> }
                .into_any();
        }
        let r = report();
        let summary = view! {
            <dl class="mb-3 grid grid-cols-3 gap-4 sm:max-w-lg">
                <Stat label=tr("ui-rx-signed") value=fmt::int(i18n.locale.get_untracked(), r.signed) />
                <Stat label=tr("ui-rx-unsigned") value=fmt::int(i18n.locale.get_untracked(), r.unsigned) />
                <Stat label=tr("ui-rx-lost") value=fmt::int(i18n.locale.get_untracked(), r.lost()) />
            </dl>
        };
        if r.streams.is_empty() {
            return view! { {summary} <Empty text=tr("ui-rx-verify-none") /> }.into_any();
        }
        view! {
            {summary}
            <div class="overflow-x-auto rounded-md border border-slate-200 dark:border-slate-800">
                <table class="min-w-full divide-y divide-slate-200 dark:divide-slate-800">
                    <caption class="sr-only">{tr("ui-rx-verify-title")}</caption>
                    <thead>
                        <tr>
                            <th scope="col" class=TH>{tr("ui-rx-stream")}</th>
                            <th scope="col" class=format!("{TH} text-right")>{tr("ui-rx-received")}</th>
                            <th scope="col" class=format!("{TH} text-right")>{tr("ui-rx-seq-range")}</th>
                            <th scope="col" class=format!("{TH} text-right")>{tr("ui-rx-lost")}</th>
                            <th scope="col" class=format!("{TH} text-right")>{tr("ui-rx-out-of-order")}</th>
                            <th scope="col" class=format!("{TH} text-right")>{tr("ui-rx-duplicates")}</th>
                            <th scope="col" class=format!("{TH} text-right")>{tr("ui-rx-latency")}</th>
                            <th scope="col" class=format!("{TH} text-right")>{tr("ui-rx-jitter")}</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                        {r.streams.into_iter().map(|s| stream_row(i18n, &s)).collect_view()}
                    </tbody>
                </table>
            </div>
            <p class="mt-1 text-xs text-slate-500">{tr("ui-rx-latency-note")}</p>
        }
        .into_any()
    };
    view! {
        <section aria-labelledby="rx-verify-h">
            <h3 id="rx-verify-h" class="mb-2 text-sm font-semibold">{tr("ui-rx-verify-title")}</h3>
            {body}
        </section>
    }
}

fn stream_row(i18n: I18n, s: &StreamVerify) -> impl IntoView {
    let loc = i18n.locale.get_untracked();
    let lost_cls = if s.lost > 0 {
        format!("{TD_NUM} font-semibold text-rose-600 dark:text-rose-400")
    } else {
        TD_NUM.to_owned()
    };
    view! {
        <tr>
            <td class=format!("{TD} font-mono")>{s.stream_id}</td>
            <td class=TD_NUM>{fmt::int(loc, s.received)}</td>
            <td class=format!("{TD_NUM} font-mono text-xs")>{format!("{} – {}", s.first_seq, s.last_seq)}</td>
            <td class=lost_cls>{format!("{} ({})", fmt::int(loc, s.lost), fmt::percent(s.loss_ratio()))}</td>
            <td class=TD_NUM>{fmt::int(loc, s.out_of_order)}</td>
            <td class=TD_NUM>{fmt::int(loc, s.duplicates)}</td>
            <td class=format!("{TD_NUM} whitespace-nowrap text-xs")>
                {format!("{} / {} / {}", fmt::nanos(s.latency_min_ns), fmt::nanos(s.latency_avg_ns), fmt::nanos(s.latency_max_ns))}
            </td>
            <td class=format!("{TD_NUM} text-xs")>{fmt::nanos(s.jitter_ns)}</td>
        </tr>
    }
}

/// The first N frames plus the selected frame's bytes and layers.
#[component]
fn FramesSection(
    id: String,
    stats: Signal<Option<RxStats>>,
    status: Signal<JobStatus>,
) -> impl IntoView {
    let i18n = use_i18n();
    let id = StoredValue::new(id);
    // Refetch when more frames were kept, and once more when the capture ends.
    let key = Memo::new(move |_| {
        (
            stats.with(|s| s.as_ref().map_or(0, |s| s.kept)),
            status.with(JobStatus::is_terminal),
        )
    });
    let frames = LocalResource::new(move || {
        key.track();
        let id = id.get_value();
        async move { api::capture_packets(&id).await }
    });
    let selected = RwSignal::new(None::<u64>);
    let shown = RwSignal::new(PAGE_ROWS);

    let table = move || match frames.get() {
        None => view! { <Loading /> }.into_any(),
        Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
        Some(Ok(list)) if list.is_empty() => {
            view! { <Empty text=tr("ui-rx-frames-empty") /> }.into_any()
        }
        Some(Ok(list)) => {
            let first = list.first().map_or(0, |f| f.ts_ns);
            let total = list.len();
            let rows = list
                .into_iter()
                .take(shown.get())
                .map(|f| {
                    let idx = f.index;
                    let is_sel = move || selected.get() == Some(idx);
                    let sig = f.signature.map_or_else(
                        || "–".to_owned(),
                        |s| {
                            i18n.ta(
                                "ui-rx-sig-value",
                                &[("stream", s.stream_id.to_string()), ("seq", s.seq.to_string())],
                            )
                        },
                    );
                    view! {
                        <tr class=move || if is_sel() { "bg-sky-50 dark:bg-sky-500/10" } else { "hover:bg-slate-50 dark:hover:bg-slate-800/50" }>
                            <td class=TD>
                                <button
                                    type="button"
                                    class="font-mono text-sky-700 hover:underline dark:text-sky-400"
                                    aria-pressed=move || is_sel().to_string()
                                    aria-label=move || i18n.ta("ui-rx-frame-title", &[("index", idx.to_string())])
                                    on:click=move |_| selected.set(Some(idx))
                                >
                                    {idx}
                                </button>
                            </td>
                            <td class=format!("{TD_NUM} font-mono text-xs")>{rel_secs(first, f.ts_ns)}</td>
                            <td class=TD_NUM>{f.len}</td>
                            <td class=format!("{TD} text-xs")>{sig}</td>
                        </tr>
                    }
                })
                .collect_view();
            view! {
                <div class="max-h-[28rem] overflow-auto rounded-md border border-slate-200 dark:border-slate-800">
                    <table class="min-w-full divide-y divide-slate-200 dark:divide-slate-800">
                        <caption class="sr-only">{tr("ui-rx-frames")}</caption>
                        <thead class="sticky top-0 bg-white dark:bg-slate-900">
                            <tr>
                                <th scope="col" class=TH>"#"</th>
                                <th scope="col" class=format!("{TH} text-right")>{tr("ui-rx-time")}</th>
                                <th scope="col" class=format!("{TH} text-right")>{tr("ui-rx-length")}</th>
                                <th scope="col" class=TH>{tr("ui-rx-signature")}</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-100 dark:divide-slate-800">{rows}</tbody>
                    </table>
                </div>
                {(total > shown.get()).then(|| view! {
                    <button type="button" class=format!("{BTN} mt-2") on:click=move |_| shown.update(|n| *n += PAGE_ROWS)>
                        {move || i18n.ta("ui-rx-show-more", &[("count", shown.get().min(total).to_string()), ("max", total.to_string())])}
                    </button>
                })}
            }
            .into_any()
        }
    };

    let packet = move || {
        let Some(idx) = selected.get() else {
            return view! { <p class="text-xs text-slate-500">{tr("ui-rx-select-frame")}</p> }
                .into_any();
        };
        let frame = frames
            .get()
            .and_then(Result::ok)
            .and_then(|l| l.into_iter().find(|f| f.index == idx));
        match frame {
            Some(f) => view! { <PacketView frame=f /> }.into_any(),
            None => view! { <p class="text-xs text-slate-500">{tr("ui-rx-select-frame")}</p> }
                .into_any(),
        }
    };

    view! {
        <section aria-labelledby="rx-frames-h">
            <h3 id="rx-frames-h" class="mb-2 text-sm font-semibold">{tr("ui-rx-frames")}</h3>
            <div class="grid gap-4 xl:grid-cols-2">
                <div class="min-w-0">{table}</div>
                <div class="min-w-0">{packet}</div>
            </div>
        </section>
    }
}

/// A short display of a decoded field's value.
fn field_text(protocol: &str, f: &FieldLayout) -> String {
    if !f.children.is_empty() {
        return format!("[{}]", f.children.len());
    }
    proto::field_type(protocol, &f.name).map_or_else(
        || format!("{:?}", f.value),
        |ty| format_value(&f.value, &ty),
    )
}

fn in_range(byte: usize, range: Option<(usize, usize)>) -> bool {
    range.is_some_and(|(off, len)| {
        let (b0, b1) = (byte * 8, byte * 8 + 8);
        off < b1 && b0 < off + len
    })
}

/// Hex dump + decoded layers of one frame (decoded in the browser with the same
/// `wpg_core::parse` the server's `/api/parse` uses). Selecting a field
/// highlights its octets; clicking an octet selects its field.
#[component]
fn PacketView(frame: CapturedFrame) -> impl IntoView {
    let i18n = use_i18n();
    let bytes = hex_bytes(&frame.hex);
    let parsed = wpg_core::parse(&bytes, "ethernet2", proto::catalog());
    let range = RwSignal::new(None::<(usize, usize)>);
    let (layout, diagnostics, parse_error) = match parsed {
        Ok(p) => (p.layout, p.diagnostics, None),
        Err(e) => (Vec::new(), Vec::new(), Some(e.to_string())),
    };
    let layout_sv = StoredValue::new(layout.clone());

    let hex_rows = bytes
        .chunks(16)
        .enumerate()
        .map(|(r, chunk)| {
            let start = r * 16;
            let cells = chunk
                .iter()
                .enumerate()
                .map(|(k, &b)| {
                    let i = start + k;
                    view! {
                        <span
                            class=move || if in_range(i, range.get()) {
                                "inline-block w-[1.35rem] cursor-pointer rounded px-0.5 text-center bg-sky-200 text-sky-900 dark:bg-sky-500/40 dark:text-sky-100"
                            } else {
                                "inline-block w-[1.35rem] cursor-pointer rounded px-0.5 text-center hover:bg-slate-200 dark:hover:bg-slate-700"
                            }
                            on:click=move |_| range.set(layout_sv.with_value(|l| range_at_byte(l, i)))
                        >
                            {format!("{b:02x}")}
                        </span>
                    }
                })
                .collect_view();
            let ascii: String = chunk
                .iter()
                .map(|&c| if (0x20..0x7f).contains(&c) { char::from(c) } else { '.' })
                .collect();
            view! {
                <div class="flex items-center gap-3 font-mono text-xs leading-5">
                    <span class="w-10 shrink-0 text-right text-slate-400 tabular-nums">{format!("{start:04x}")}</span>
                    <span class="flex gap-px">{cells}</span>
                    <span class="ml-2 whitespace-pre text-slate-500">{ascii}</span>
                </div>
            }
        })
        .collect_view();

    let tree = layout
        .into_iter()
        .map(|layer: LayerLayout| {
            let p = layer.protocol.clone();
            let display = proto::def(&p).map_or_else(|| p.clone(), |d| d.name.clone());
            let lrange = (layer.offset * 8, layer.len * 8);
            view! {
                <li>
                    <button
                        type="button"
                        class="font-sans text-sm font-medium hover:text-sky-700"
                        aria-pressed=move || (range.get() == Some(lrange)).to_string()
                        on:click=move |_| range.set(Some(lrange))
                    >
                        {display}" "<span class="font-mono text-[10px] text-slate-400">{p.clone()}</span>
                    </button>
                    <ul class="mt-0.5 space-y-px border-l border-slate-200 pl-2 dark:border-slate-700">
                        {layer.fields.iter().map(|f| field_node(range, &p, f)).collect_view()}
                    </ul>
                </li>
            }
        })
        .collect_view();

    let diags = diagnostics
        .into_iter()
        .map(|d| {
            let cls = match d.level {
                DiagLevel::Error => "text-rose-600",
                DiagLevel::Warning => "text-amber-600",
                DiagLevel::Info => "text-slate-500",
            };
            let msg = i18n.t_dyn(&d.key, &d.args);
            let msg = if msg == d.key { d.message.clone() } else { msg };
            view! { <p class=format!("text-[11px] {cls}")>{msg}</p> }
        })
        .collect_view();

    let index = frame.index;
    let title = move || i18n.ta("ui-rx-frame-title", &[("index", index.to_string())]);
    let (kept, len) = (bytes.len(), frame.len);
    let truncated = (kept < len).then_some(move || {
        i18n.ta(
            "ui-rx-truncated",
            &[("bytes", kept.to_string()), ("len", len.to_string())],
        )
    });
    view! {
        <div class="space-y-3" role="group" aria-label=title>
            <h4 class="text-sm font-semibold">{title}" · "{format!("{len} B")}</h4>
            {truncated.map(|t| view! { <p class="text-xs text-amber-700 dark:text-amber-400">{t}</p> })}
            <section aria-label=tr("ui-bld-hex") class="max-h-80 overflow-auto rounded-md border border-slate-200 bg-white p-2 dark:border-slate-800 dark:bg-slate-950">
                {hex_rows}
            </section>
            <section aria-label=tr("ui-bld-tree") class="max-h-96 overflow-auto rounded-md border border-slate-200 p-2 dark:border-slate-800">
                {parse_error.map(|e| view! {
                    <p class="text-xs text-rose-600" role="alert">{move || i18n.ta("ui-rx-decode-failed", &[("error", e.clone())])}</p>
                })}
                <ul class="space-y-1 font-mono text-xs">{tree}</ul>
                <div class="mt-2 space-y-0.5">{diags}</div>
            </section>
        </div>
    }
}

/// One decoded field (recurses into TLV / list children).
fn field_node(range: RwSignal<Option<(usize, usize)>>, protocol: &str, f: &FieldLayout) -> AnyView {
    let r = (f.bit_offset, f.bit_len);
    let text = field_text(protocol, f);
    let proto_id = protocol.to_owned();
    let children = f.children.clone();
    view! {
        <li>
            <button
                type="button"
                class=move || if range.get() == Some(r) {
                    "flex w-full items-baseline gap-1.5 rounded bg-sky-100 px-1 text-left dark:bg-sky-500/20"
                } else {
                    "flex w-full items-baseline gap-1.5 rounded px-1 text-left hover:bg-slate-100 dark:hover:bg-slate-800"
                }
                aria-pressed=move || (range.get() == Some(r)).to_string()
                on:click=move |_| range.set(Some(r))
            >
                <span class="text-slate-700 dark:text-slate-300">{f.name.clone()}</span>
                <span class="text-slate-500">"="</span>
                <span class="truncate text-sky-800 dark:text-sky-300">{text}</span>
                <span class="ml-auto shrink-0 text-[9px] text-slate-400">{format!("@{}·{}", f.bit_offset, f.bit_len)}</span>
            </button>
            {(!children.is_empty()).then(|| view! {
                <ul class="space-y-px border-l border-slate-200 pl-2 dark:border-slate-700">
                    {children.iter().map(|c| field_node(range, &proto_id, c)).collect_view()}
                </ul>
            })}
        </li>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(name: &str, available: bool) -> RxBackendInfo {
        RxBackendInfo {
            name: name.into(),
            available,
            reason: None,
        }
    }

    #[test]
    fn default_backends() {
        assert_eq!(
            default_backend(&[b("null", true), b("pcapfile", true), b("afpacket", true)]),
            "afpacket"
        );
        assert_eq!(
            default_backend(&[b("afpacket", false), b("pcap", true)]),
            "pcap"
        );
        assert_eq!(default_backend(&[b("afpacket", false)]), "pcapfile");
        assert_eq!(default_backend(&[]), "pcapfile");
    }

    #[test]
    fn keep_values() {
        assert_eq!(parse_keep(""), Ok(DEFAULT_KEEP));
        assert_eq!(parse_keep(" 0 "), Ok(0));
        assert_eq!(parse_keep("10000"), Ok(10_000));
        assert_eq!(parse_keep("10001"), Err("ui-rx-keep-max"));
        assert_eq!(parse_keep("x"), Err("ui-valid-integer"));
    }

    #[test]
    fn helpers() {
        assert_eq!(char_pos("udp port x", 9), 9);
        assert_eq!(char_pos("日本 x", 3), 1);
        assert_eq!(char_pos("日本 x", 4), 1, "inside a character");
        assert_eq!(char_pos("ab", 99), 2);
        assert_eq!(hex_bytes("00ff10"), vec![0, 255, 16]);
        assert_eq!(hex_bytes("0aZZ1"), vec![10]);
        assert_eq!(rel_secs(1_000, 124_000), "0.000123");
        assert_eq!(rel_secs(5, 1), "0.000000");
        assert!(in_range(1, Some((8, 8))));
        assert!(in_range(1, Some((12, 1))));
        assert!(!in_range(2, Some((8, 8))));
        assert!(!in_range(0, None));
    }

    #[test]
    fn form_to_spec() {
        let mut v = FormValues {
            backend: "pcapfile".into(),
            iface: " run.pcapng ".into(),
            filter: " udp and wpg ".into(),
            keep: String::new(),
            verify: true,
            ..FormValues::default()
        };
        assert!(v.all_ok());
        let s = v.spec();
        assert_eq!(s.backend.as_deref(), Some("pcapfile"));
        assert_eq!(s.iface.as_deref(), Some("run.pcapng"));
        assert_eq!(s.filter.as_deref(), Some("udp and wpg"));
        assert_eq!(s.keep, DEFAULT_KEEP);
        assert!(s.verify && s.count.is_none() && s.save.is_none());

        v.save = "run.pcapng".into();
        assert_eq!(v.save_check(), Err("ui-rx-save-same"));
        v.save = "../x".into();
        assert_eq!(v.save_check(), Err("ui-valid-capture-name"));
        v.save = "out.pcapng".into();
        assert_eq!(v.save_check(), Ok(()));
        v.iface = "/etc/passwd".into();
        assert_eq!(v.iface_check(), Err("ui-valid-capture-name"));

        v.backend = "afpacket".into();
        v.iface = String::new();
        assert_eq!(v.iface_check(), Err("ui-jobs-need-iface"));
        v.iface = "eth0".into();
        v.count = "0".into();
        assert_eq!(v.count_check(), Err("ui-valid-range"));
        v.count = "1000".into();
        v.duration = "5 days".into();
        assert!(v.duration_check().is_err());
        v.duration = "10s".into();
        v.keep = "20000".into();
        assert!(!v.all_ok());
        v.keep = "50".into();
        assert!(v.all_ok());
        let s = v.spec();
        assert_eq!(
            (s.count, s.duration.as_deref(), s.keep),
            (Some(1000), Some("10s"), 50)
        );

        v.backend = "null".into();
        assert_eq!(v.iface_check(), Ok(()));
        assert!(v.spec().iface.is_none());
    }
}
