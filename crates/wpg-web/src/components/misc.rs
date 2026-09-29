//! Badges, error boxes, toasts, page headers and other small pieces.

use std::time::Duration;

use leptos::prelude::*;

use crate::api::{ApiError, JobStatus};
use crate::i18n::{tr, use_i18n};

/// i18n key of a job state name.
pub(crate) fn state_key(state: &str) -> &'static str {
    match state {
        "created" => "ui-state-created",
        "scheduled" => "ui-state-scheduled",
        "running" => "ui-state-running",
        "paused" => "ui-state-paused",
        "finished" => "ui-state-finished",
        "failed" => "ui-state-failed",
        _ => "ui-state-unknown",
    }
}

/// i18n key of a finish reason (`count_reached`, ...).
pub(crate) fn reason_key(detail: &str) -> Option<&'static str> {
    Some(match detail {
        "count_reached" => "ui-reason-count-reached",
        "duration_elapsed" => "ui-reason-duration-elapsed",
        "source_exhausted" => "ui-reason-source-exhausted",
        "stopped" => "ui-reason-stopped",
        _ => return None,
    })
}

/// Coloured job-state pill (with the finish reason / error as tooltip).
#[component]
pub(crate) fn StateBadge(#[prop(into)] status: Signal<JobStatus>) -> impl IntoView {
    let i18n = use_i18n();
    let class = move || {
        let c = match status.with(|s| s.state.clone()).as_str() {
            "running" => "bg-emerald-100 text-emerald-800 ring-emerald-600/20 dark:bg-emerald-500/15 dark:text-emerald-300",
            "paused" => "bg-amber-100 text-amber-800 ring-amber-600/20 dark:bg-amber-500/15 dark:text-amber-300",
            "scheduled" => "bg-indigo-100 text-indigo-800 ring-indigo-600/20 dark:bg-indigo-500/15 dark:text-indigo-300",
            "failed" => "bg-rose-100 text-rose-800 ring-rose-600/20 dark:bg-rose-500/15 dark:text-rose-300",
            "finished" => "bg-sky-100 text-sky-800 ring-sky-600/20 dark:bg-sky-500/15 dark:text-sky-300",
            _ => "bg-slate-100 text-slate-700 ring-slate-500/20 dark:bg-slate-500/15 dark:text-slate-300",
        };
        format!("inline-flex items-center gap-1 whitespace-nowrap rounded-full px-2 py-0.5 text-xs font-medium ring-1 ring-inset {c}")
    };
    let title = move || {
        status.with(|s| match (&s.detail, s.state.as_str()) {
            (Some(d), "finished") => reason_key(d).map_or_else(|| d.clone(), |k| i18n.t(k)),
            (Some(d), _) => d.clone(),
            (None, _) => String::new(),
        })
    };
    view! {
        <span class=class title=title>
            {move || {
                (status.with(|s| s.state == "running"))
                    .then(|| view! { <span class="h-1.5 w-1.5 animate-pulse rounded-full bg-current"></span> })
            }}
            {move || i18n.t(state_key(&status.with(|s| s.state.clone())))}
        </span>
    }
}

/// An error message (with lint findings when the server sent them).
#[component]
pub(crate) fn ErrorBox(#[prop(into)] error: Signal<ApiError>) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <div
            role="alert"
            class="rounded-md border border-rose-300 bg-rose-50 px-3 py-2 text-sm text-rose-800 dark:border-rose-800 dark:bg-rose-950/50 dark:text-rose-200"
        >
            <p>{move || error.with(|e| e.message(i18n))}</p>
            {move || {
                let issues = error.with(ApiError::lint_issues);
                (!issues.is_empty())
                    .then(|| {
                        view! {
                            <ul class="mt-2 list-disc space-y-0.5 pl-5 text-xs">
                                {issues
                                    .into_iter()
                                    .map(|i| {
                                        view! {
                                            <li>
                                                <code class="font-mono">{i.path}</code>
                                                " "
                                                {i.message}
                                            </li>
                                        }
                                    })
                                    .collect_view()}
                            </ul>
                        }
                    })
            }}
        </div>
    }
}

/// "Loading…" placeholder.
#[component]
pub(crate) fn Loading() -> impl IntoView {
    view! {
        <p class="animate-pulse py-4 text-sm text-slate-500" role="status">
            {tr("ui-loading")}
        </p>
    }
}

/// Muted "nothing here" text.
#[component]
pub(crate) fn Empty(#[prop(into)] text: Signal<String>) -> impl IntoView {
    view! {
        <p class="rounded-md border border-dashed border-slate-300 px-4 py-6 text-center text-sm text-slate-500 dark:border-slate-700">
            {move || text.get()}
        </p>
    }
}

/// Page title row with optional actions.
#[component]
pub(crate) fn PageHeader(
    #[prop(into)] title: Signal<String>,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    view! {
        <div class="mb-5 flex flex-wrap items-center justify-between gap-3">
            <h1 class="text-xl font-semibold tracking-tight">{move || title.get()}</h1>
            <div class="flex flex-wrap items-center gap-2">{children.map(|c| c())}</div>
        </div>
    }
}

/// Label + big value tile.
#[component]
pub(crate) fn Stat(
    #[prop(into)] label: Signal<String>,
    #[prop(into)] value: Signal<String>,
) -> impl IntoView {
    view! {
        <div class="min-w-0">
            <dt class="truncate text-xs text-slate-500 dark:text-slate-400">{move || label.get()}</dt>
            <dd class="truncate text-lg font-semibold tabular-nums">{move || value.get()}</dd>
        </div>
    }
}

/// Note shown on narrow screens where editing is disabled (TODO §2.5
/// "モバイル/狭幅は最低限の閲覧のみ").
#[component]
pub(crate) fn ViewOnlyNote() -> impl IntoView {
    view! {
        <p class="mb-4 rounded-md bg-amber-50 px-3 py-2 text-xs text-amber-800 md:hidden dark:bg-amber-950/40 dark:text-amber-200">
            {tr("ui-view-only")}
        </p>
    }
}

/// Kind of a toast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToastKind {
    Success,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Toast {
    id: u64,
    kind: ToastKind,
    text: String,
}

/// Transient notifications (context).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Toasts {
    items: RwSignal<Vec<Toast>>,
    next: StoredValue<u64>,
}

impl Toasts {
    pub(crate) fn new() -> Self {
        Self {
            items: RwSignal::new(Vec::new()),
            next: StoredValue::new(0),
        }
    }

    pub(crate) fn push(self, kind: ToastKind, text: String) {
        let id = self.next.get_value();
        self.next.set_value(id + 1);
        self.items.update(|v| v.push(Toast { id, kind, text }));
        let items = self.items;
        let ttl = if kind == ToastKind::Error { 8000 } else { 4000 };
        set_timeout(
            move || items.update(|v| v.retain(|t| t.id != id)),
            Duration::from_millis(ttl),
        );
    }

    pub(crate) fn ok(self, text: String) {
        self.push(ToastKind::Success, text);
    }

    pub(crate) fn error(self, text: String) {
        self.push(ToastKind::Error, text);
    }
}

/// The [`Toasts`] context.
pub(crate) fn use_toasts() -> Toasts {
    expect_context::<Toasts>()
}

/// Renders the toasts (an `aria-live` region).
#[component]
pub(crate) fn ToastRegion() -> impl IntoView {
    let toasts = use_toasts();
    view! {
        <div
            class="pointer-events-none fixed right-4 bottom-4 z-[60] flex w-80 max-w-[calc(100vw-2rem)] flex-col gap-2"
            aria-live="polite"
            aria-atomic="false"
        >
            <For
                each=move || toasts.items.get()
                key=|t| t.id
                children=move |t: Toast| {
                    let class = match t.kind {
                        ToastKind::Success => "border-emerald-300 bg-emerald-50 text-emerald-900 dark:border-emerald-800 dark:bg-emerald-950 dark:text-emerald-100",
                        ToastKind::Error => "border-rose-300 bg-rose-50 text-rose-900 dark:border-rose-800 dark:bg-rose-950 dark:text-rose-100",
                    };
                    let role = if t.kind == ToastKind::Error { "alert" } else { "status" };
                    view! {
                        <div
                            role=role
                            class=format!("pointer-events-auto rounded-md border px-3 py-2 text-sm shadow-lg {class}")
                        >
                            {t.text}
                        </div>
                    }
                }
            />
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys() {
        for s in [
            "created",
            "scheduled",
            "running",
            "paused",
            "finished",
            "failed",
        ] {
            assert_eq!(state_key(s), format!("ui-state-{s}"));
        }
        assert_eq!(state_key("zombie"), "ui-state-unknown");
        assert_eq!(reason_key("stopped"), Some("ui-reason-stopped"));
        assert_eq!(reason_key("x"), None);
    }
}
