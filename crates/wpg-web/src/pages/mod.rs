//! Screens (spec §2.5 "画面構成"). The builder lives in [`crate::builder`].

pub(crate) mod auth;
pub(crate) mod dashboard;
pub(crate) mod job_detail;
pub(crate) mod job_form;
pub(crate) mod jobs;
pub(crate) mod rx;
pub(crate) mod settings;
pub(crate) mod snmp;
pub(crate) mod templates;
pub(crate) mod terminal;

use leptos::prelude::*;

use crate::i18n::tr;
pub(crate) use rx::RxPage;

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
