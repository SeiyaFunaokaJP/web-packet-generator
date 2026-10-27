//! `/snmp`: the SNMP manager tool (ADR-0010). Three tabs share the target
//! form and the pcap recording controls:
//!
//! * operations: Get / GetNext / GetBulk / Walk / Set with readable results,
//!   tables as rows × columns, auto refresh with counter rates;
//! * notifications: the trap / inform receiver with its live list, and the
//!   trap / inform sender;
//! * MIBs: upload (drag & drop), module list with load problems, tree browser
//!   with node details.

mod mibs;
pub(crate) mod model;
mod ops;
mod target;
mod traps;

use leptos::prelude::*;

use crate::api::snmp::{self as api, CaptureStatus};
use crate::components::{
    use_toasts, ErrorBox, Icon, IconName, Loading, PageHeader, BTN, BTN_GHOST,
};
use crate::i18n::{tr, use_i18n};
use model::TargetValues;

/// The tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    Ops,
    Traps,
    Mibs,
}

impl Tab {
    const ALL: [Tab; 3] = [Tab::Ops, Tab::Traps, Tab::Mibs];

    const fn key(self) -> &'static str {
        match self {
            Tab::Ops => "ui-snmp-tab-ops",
            Tab::Traps => "ui-snmp-tab-traps",
            Tab::Mibs => "ui-snmp-tab-mibs",
        }
    }

    const fn id(self) -> &'static str {
        match self {
            Tab::Ops => "snmp-tab-ops",
            Tab::Traps => "snmp-tab-traps",
            Tab::Mibs => "snmp-tab-mibs",
        }
    }

    /// The neighbour for ←/→ in the tab list.
    pub(crate) fn step(self, forward: bool) -> Tab {
        let i = Tab::ALL.iter().position(|t| *t == self).unwrap_or(0);
        let n = Tab::ALL.len();
        Tab::ALL[if forward {
            (i + 1) % n
        } else {
            (i + n - 1) % n
        }]
    }
}

/// State shared by the tabs (context).
#[derive(Debug, Clone, Copy)]
pub(crate) struct SnmpCtx {
    pub(crate) target: RwSignal<TargetValues>,
    /// The OID field of the operations tab (the MIB browser fills it).
    pub(crate) oids: RwSignal<String>,
    pub(crate) tab: RwSignal<Tab>,
    /// Bumped after every exchange: the recording counter refetches.
    pub(crate) capture_rev: RwSignal<u64>,
}

pub(crate) fn use_snmp() -> SnmpCtx {
    expect_context::<SnmpCtx>()
}

#[component]
pub(crate) fn SnmpPage() -> impl IntoView {
    let status = LocalResource::new(api::status);
    let ctx = SnmpCtx {
        target: RwSignal::new(TargetValues::default()),
        oids: RwSignal::new("system".to_owned()),
        tab: RwSignal::new(Tab::Ops),
        capture_rev: RwSignal::new(0),
    };
    provide_context(ctx);

    view! {
        <PageHeader title=tr("ui-nav-snmp")>
            <CaptureControls />
        </PageHeader>
        {move || match status.get() {
            None => view! { <Loading /> }.into_any(),
            Some(Err(e)) => view! { <ErrorBox error=e /> }.into_any(),
            Some(Ok(s)) if !s.enabled => view! {
                <p class="rounded-md bg-amber-50 px-3 py-2 text-sm text-amber-800 dark:bg-amber-950/40 dark:text-amber-200" role="status">
                    {tr("ui-snmp-disabled")}
                </p>
            }
            .into_any(),
            Some(Ok(_)) => view! { <Tabs /> }.into_any(),
        }}
    }
}

#[component]
fn Tabs() -> impl IntoView {
    let i18n = use_i18n();
    let ctx = use_snmp();
    let on_key = move |ev: leptos::ev::KeyboardEvent| {
        let forward = match ev.key().as_str() {
            "ArrowRight" => true,
            "ArrowLeft" => false,
            _ => return,
        };
        ev.prevent_default();
        let next = ctx.tab.get_untracked().step(forward);
        ctx.tab.set(next);
        crate::dom::focus_id(next.id());
    };
    view! {
        <div role="tablist" aria-label=tr("ui-nav-snmp") class="mb-4 flex gap-1 border-b border-slate-200 dark:border-slate-800" on:keydown=on_key>
            {Tab::ALL
                .into_iter()
                .map(|t| {
                    let active = move || ctx.tab.get() == t;
                    view! {
                        <button
                            type="button"
                            role="tab"
                            id=t.id()
                            aria-selected=move || active().to_string()
                            aria-controls=format!("{}-panel", t.id())
                            tabindex=move || if active() { "0" } else { "-1" }
                            class=move || {
                                if active() {
                                    "-mb-px border-b-2 border-sky-600 px-3 py-2 text-sm font-semibold text-sky-800 dark:border-sky-400 dark:text-sky-300"
                                } else {
                                    "-mb-px border-b-2 border-transparent px-3 py-2 text-sm text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-100"
                                }
                            }
                            on:click=move |_| ctx.tab.set(t)
                        >
                            {move || i18n.t(t.key())}
                        </button>
                    }
                })
                .collect_view()}
        </div>
        // Panels stay mounted (form contents, results and the tree survive tab
        // switches); only the visibility changes.
        {Tab::ALL
            .into_iter()
            .map(|t| {
                let panel = match t {
                    Tab::Ops => view! { <ops::OpsPanel /> }.into_any(),
                    Tab::Traps => view! { <traps::TrapsPanel /> }.into_any(),
                    Tab::Mibs => view! { <mibs::MibsPanel /> }.into_any(),
                };
                view! {
                    <div
                        role="tabpanel"
                        id=format!("{}-panel", t.id())
                        aria-labelledby=t.id()
                        class=move || if ctx.tab.get() == t { "" } else { "hidden" }
                    >
                        {panel}
                    </div>
                }
            })
            .collect_view()}
    }
}

/// The "record pcap" switch, frame counter, downloads and clear.
#[component]
fn CaptureControls() -> impl IntoView {
    let i18n = use_i18n();
    let toasts = use_toasts();
    let ctx = use_snmp();
    let state = RwSignal::new(None::<CaptureStatus>);
    Effect::new(move |_| {
        ctx.capture_rev.track();
        leptos::task::spawn_local(async move {
            if let Ok(s) = api::capture().await {
                state.set(Some(s));
            }
        });
    });
    let control = move |enabled: Option<bool>, clear: bool| {
        leptos::task::spawn_local(async move {
            match api::capture_control(enabled, clear).await {
                Ok(s) => state.set(Some(s)),
                Err(e) => toasts.error(e.message(i18n)),
            }
        });
    };
    let frames = move || state.get().map_or(0, |s| s.frames);
    view! {
        <div class="flex flex-wrap items-center gap-2 text-sm" role="group" aria-label=tr("ui-snmp-pcap")>
            <label class="flex items-center gap-2" title=tr("ui-snmp-pcap-hint")>
                <input
                    type="checkbox"
                    prop:checked=move || state.get().is_some_and(|s| s.enabled)
                    on:change=move |ev| control(Some(event_target_checked(&ev)), false)
                />
                {tr("ui-snmp-pcap")}
            </label>
            <span class="tabular-nums text-xs text-slate-500" aria-live="polite">
                {move || i18n.ta("ui-snmp-pcap-frames", &[("count", frames().to_string())])}
            </span>
            <a
                class=BTN
                href=api::capture_url("pcap")
                download=""
                aria-disabled=move || (frames() == 0).to_string()
                title=tr("ui-snmp-pcap-download")
            >
                <Icon name=IconName::Download class="h-4 w-4" />
                ".pcap"
            </a>
            <a class=BTN href=api::capture_url("pcapng") download="" title=tr("ui-snmp-pcap-download")>
                <Icon name=IconName::Download class="h-4 w-4" />
                ".pcapng"
            </a>
            <button
                type="button"
                class=BTN_GHOST
                title=tr("ui-snmp-pcap-clear")
                aria-label=tr("ui-snmp-pcap-clear")
                on:click=move |_| control(None, true)
            >
                <Icon name=IconName::Trash class="h-4 w-4" />
            </button>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_cycle() {
        assert_eq!(Tab::Ops.step(true), Tab::Traps);
        assert_eq!(Tab::Mibs.step(true), Tab::Ops);
        assert_eq!(Tab::Ops.step(false), Tab::Mibs);
    }
}
