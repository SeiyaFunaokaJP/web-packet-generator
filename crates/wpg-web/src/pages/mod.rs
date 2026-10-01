//! Screens (TODO §2.5 "画面構成"). The builder lives in [`crate::builder`].

pub(crate) mod auth;
pub(crate) mod dashboard;
pub(crate) mod job_detail;
pub(crate) mod job_form;
pub(crate) mod jobs;
pub(crate) mod settings;
pub(crate) mod templates;

use leptos::prelude::*;

use crate::components::{Icon, IconName, PageHeader, CARD};
use crate::i18n::tr;

/// Placeholder body for screens of a later milestone.
#[component]
pub(crate) fn ComingLater(
    #[prop(into)] title: Signal<String>,
    #[prop(into)] text: Signal<String>,
    icon: IconName,
) -> impl IntoView {
    view! {
        <PageHeader title=title />
        <div class=format!("{CARD} flex flex-col items-center gap-3 py-12 text-center")>
            <Icon name=icon class="h-10 w-10 text-slate-400" />
            <p class="max-w-lg text-sm text-slate-600 dark:text-slate-400">{move || text.get()}</p>
        </div>
    }
}

/// `/rx`: receive / verification counters (M9).
#[component]
pub(crate) fn RxPage() -> impl IntoView {
    view! { <ComingLater title=tr("ui-nav-rx") text=tr("ui-rx-placeholder") icon=IconName::Rx /> }
}

/// Unknown route.
#[component]
pub(crate) fn NotFound() -> impl IntoView {
    view! {
        <div class="mx-auto max-w-md py-16 text-center">
            <p class="text-4xl font-bold text-slate-300">"404"</p>
            <p class="mt-2 text-sm">{tr("ui-not-found")}</p>
            <a href="/" class="mt-4 inline-block text-sm text-sky-700 underline dark:text-sky-400">
                {tr("ui-back-to-dashboard")}
            </a>
        </div>
    }
}
