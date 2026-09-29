//! Bottom drawer with the server log (WebSocket `log` events), level filter and
//! follow mode.

use leptos::html;
use leptos::prelude::*;

use super::{Icon, IconName, BTN_GHOST};
use crate::i18n::{tr, use_i18n};
use crate::ws::{use_live, LogEntry};

/// Severity rank of a `tracing` level name.
pub(crate) fn level_rank(level: &str) -> u8 {
    match level.to_ascii_uppercase().as_str() {
        "TRACE" => 0,
        "DEBUG" => 1,
        "INFO" => 2,
        "WARN" | "WARNING" => 3,
        "ERROR" => 4,
        _ => 2,
    }
}

const LEVELS: [&str; 5] = ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"];
const KEY_LEVEL: &str = "wpg.logs.level";

fn level_class(level: &str) -> &'static str {
    match level_rank(level) {
        4 => "text-rose-600 dark:text-rose-400",
        3 => "text-amber-600 dark:text-amber-400",
        2 => "text-sky-700 dark:text-sky-400",
        _ => "text-slate-500",
    }
}

/// The logs drawer.
#[component]
pub(crate) fn LogsPanel(#[prop(into)] on_close: Callback<()>) -> impl IntoView {
    let live = use_live();
    let i18n = use_i18n();
    let min_level = RwSignal::new(
        crate::prefs::load(KEY_LEVEL)
            .filter(|l| LEVELS.contains(&l.as_str()))
            .unwrap_or_else(|| "INFO".to_owned()),
    );
    let follow = RwSignal::new(true);
    let list = NodeRef::<html::Div>::new();

    let visible = move || {
        let min = level_rank(&min_level.get());
        live.logs.with(|l| {
            l.iter()
                .filter(|e| level_rank(&e.line.level) >= min)
                .cloned()
                .collect::<Vec<LogEntry>>()
        })
    };

    Effect::new(move |_| {
        live.logs.track();
        min_level.track();
        if follow.get() {
            if let Some(el) = list.get() {
                request_animation_frame(move || el.set_scroll_top(el.scroll_height()));
            }
        }
    });

    view! {
        <section
            aria-label=tr("ui-logs-title")
            class="fixed inset-x-0 bottom-0 z-30 flex h-64 flex-col border-t border-slate-200 bg-white shadow-[0_-4px_16px_rgba(0,0,0,0.08)] md:left-56 dark:border-slate-800 dark:bg-slate-900"
        >
            <div class="flex items-center gap-3 border-b border-slate-200 px-3 py-1.5 text-sm dark:border-slate-800">
                <h2 class="font-semibold">{tr("ui-logs-title")}</h2>
                <label class="flex items-center gap-1 text-xs text-slate-500">
                    {tr("ui-logs-level")}
                    <select
                        class="rounded border border-slate-300 bg-transparent px-1 py-0.5 text-xs dark:border-slate-700"
                        on:change=move |ev| {
                            let v = event_target_value(&ev);
                            crate::prefs::store(KEY_LEVEL, &v);
                            min_level.set(v);
                        }
                    >
                        {LEVELS
                            .into_iter()
                            .map(|l| {
                                view! {
                                    <option value=l selected=move || min_level.get() == l>
                                        {l}
                                    </option>
                                }
                            })
                            .collect_view()}
                    </select>
                </label>
                <label class="flex items-center gap-1 text-xs text-slate-500">
                    <input
                        type="checkbox"
                        prop:checked=move || follow.get()
                        on:change=move |ev| follow.set(event_target_checked(&ev))
                    />
                    {tr("ui-logs-follow")}
                </label>
                <span class="text-xs text-slate-400 tabular-nums">
                    {move || i18n.ta("ui-logs-count", &[("count", live.logs.with(std::collections::VecDeque::len).to_string())])}
                </span>
                <div class="ml-auto flex items-center gap-1">
                    <button
                        type="button"
                        class=BTN_GHOST
                        title=tr("ui-logs-clear")
                        aria-label=tr("ui-logs-clear")
                        on:click=move |_| live.logs.update(std::collections::VecDeque::clear)
                    >
                        <Icon name=IconName::Trash class="h-4 w-4" />
                    </button>
                    <button
                        type="button"
                        class=BTN_GHOST
                        title=tr("ui-close")
                        aria-label=tr("ui-close")
                        on:click=move |_| on_close.run(())
                    >
                        <Icon name=IconName::Close class="h-4 w-4" />
                    </button>
                </div>
            </div>
            <div
                node_ref=list
                class="flex-1 overflow-y-auto px-3 py-1 font-mono text-xs leading-5"
                role="log"
                aria-live="off"
            >
                <Show
                    when=move || live.logs.with(|l| !l.is_empty())
                    fallback=move || view! { <p class="py-2 text-slate-500">{tr("ui-logs-empty")}</p> }
                >
                    <For
                        each=visible
                        key=|e| e.seq
                        children=move |e: LogEntry| {
                            let l = e.line;
                            let ts = crate::fmt::time_of(i18n.current(), &l.ts);
                            let fields = if l.fields.is_empty() {
                                String::new()
                            } else {
                                serde_json::Value::Object(l.fields.clone()).to_string()
                            };
                            view! {
                                <div class="flex gap-2 whitespace-pre-wrap break-all">
                                    <span class="shrink-0 text-slate-400">{ts}</span>
                                    <span class=format!("w-11 shrink-0 font-semibold {}", level_class(&l.level))>
                                        {l.level.clone()}
                                    </span>
                                    <span class="hidden shrink-0 text-slate-400 lg:inline">{l.target.clone()}</span>
                                    <span>{l.message.clone()} " " <span class="text-slate-400">{fields}</span></span>
                                </div>
                            }
                        }
                    />
                </Show>
            </div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks() {
        assert!(level_rank("ERROR") > level_rank("WARN"));
        assert!(level_rank("warn") > level_rank("INFO"));
        assert!(level_rank("DEBUG") > level_rank("TRACE"));
        assert_eq!(level_rank("weird"), level_rank("INFO"));
    }
}
