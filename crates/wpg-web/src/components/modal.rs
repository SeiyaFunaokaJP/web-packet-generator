//! Dialogs: [`Modal`], [`Confirm`] and the side [`Drawer`]. All are `role="dialog"`
//! with `aria-modal`, close on Escape / backdrop click and take focus on open.

use std::sync::atomic::{AtomicU32, Ordering};

use leptos::prelude::*;

use super::{Icon, IconName, BTN, BTN_DANGER, BTN_GHOST, BTN_PRIMARY};
use crate::i18n::tr;

fn next_id(prefix: &str) -> String {
    static N: AtomicU32 = AtomicU32::new(0);
    format!("{prefix}-{}", N.fetch_add(1, Ordering::Relaxed))
}

/// Focuses `id` once the dialog is in the DOM.
fn focus_soon(id: String) {
    request_animation_frame(move || crate::dom::focus_id(&id));
}

/// A centred dialog.
#[component]
pub(crate) fn Modal(
    #[prop(into)] title: Signal<String>,
    #[prop(into)] on_close: Callback<()>,
    /// Wider panel (editors).
    #[prop(optional)]
    wide: bool,
    /// Element id to focus on open (default: the dialog itself).
    #[prop(optional)]
    focus: Option<String>,
    children: Children,
) -> impl IntoView {
    let panel = next_id("dlg");
    let heading = format!("{panel}-title");
    let labelledby = heading.clone();
    focus_soon(focus.unwrap_or_else(|| panel.clone()));
    let width = if wide { "max-w-4xl" } else { "max-w-lg" };
    view! {
        <div
            class="fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-slate-900/60 p-4 pt-[8vh]"
            on:click=move |_| on_close.run(())
        >
            <div
                id=panel
                role="dialog"
                aria-modal="true"
                aria-labelledby=labelledby
                tabindex="-1"
                class=format!(
                    "w-full {width} rounded-lg border border-slate-200 bg-white shadow-xl focus:outline-none dark:border-slate-700 dark:bg-slate-900",
                )
                on:click=|ev| ev.stop_propagation()
                on:keydown=move |ev| {
                    if ev.key() == "Escape" {
                        ev.stop_propagation();
                        on_close.run(());
                    }
                }
            >
                <div class="flex items-center justify-between gap-4 border-b border-slate-200 px-5 py-3 dark:border-slate-800">
                    <h2 id=heading class="text-base font-semibold">
                        {move || title.get()}
                    </h2>
                    <button
                        type="button"
                        class=BTN_GHOST
                        aria-label=tr("ui-close")
                        title=tr("ui-close")
                        on:click=move |_| on_close.run(())
                    >
                        <Icon name=IconName::Close />
                    </button>
                </div>
                <div class="px-5 py-4">{children()}</div>
            </div>
        </div>
    }
}

/// A confirmation dialog; the Cancel button has the initial focus (safe default).
#[component]
pub(crate) fn Confirm(
    #[prop(into)] title: Signal<String>,
    #[prop(into)] confirm_label: Signal<String>,
    #[prop(into)] on_confirm: Callback<()>,
    #[prop(into)] on_cancel: Callback<()>,
    /// Red confirm button.
    #[prop(optional)]
    danger: bool,
    /// Disable the confirm button (e.g. while a request runs).
    #[prop(optional, into)]
    busy: Signal<bool>,
    children: Children,
) -> impl IntoView {
    let cancel_id = next_id("cancel");
    view! {
        <Modal title=title on_close=on_cancel focus=cancel_id.clone()>
            <div class="space-y-3 text-sm">{children()}</div>
            <div class="mt-5 flex justify-end gap-2">
                <button id=cancel_id type="button" class=BTN on:click=move |_| on_cancel.run(())>
                    {tr("ui-cancel")}
                </button>
                <button
                    type="button"
                    class=if danger { BTN_DANGER } else { BTN_PRIMARY }
                    disabled=move || busy.get()
                    on:click=move |_| on_confirm.run(())
                >
                    {move || confirm_label.get()}
                </button>
            </div>
        </Modal>
    }
}

/// A panel sliding in from the right (job details).
#[component]
pub(crate) fn Drawer(
    #[prop(into)] title: Signal<String>,
    #[prop(into)] on_close: Callback<()>,
    children: Children,
) -> impl IntoView {
    let panel = next_id("drawer");
    let heading = format!("{panel}-title");
    let labelledby = heading.clone();
    focus_soon(panel.clone());
    view! {
        <div class="fixed inset-0 z-40 bg-slate-900/40" on:click=move |_| on_close.run(())></div>
        <aside
            id=panel
            role="dialog"
            aria-modal="true"
            aria-labelledby=labelledby
            tabindex="-1"
            class="fixed inset-y-0 right-0 z-40 flex w-full max-w-3xl flex-col border-l border-slate-200 bg-white shadow-2xl focus:outline-none dark:border-slate-800 dark:bg-slate-900"
            on:keydown=move |ev| {
                if ev.key() == "Escape" {
                    ev.stop_propagation();
                    on_close.run(());
                }
            }
        >
            <div class="flex items-center justify-between gap-4 border-b border-slate-200 px-5 py-3 dark:border-slate-800">
                <h2 id=heading class="truncate text-base font-semibold">
                    {move || title.get()}
                </h2>
                <button
                    type="button"
                    class=BTN_GHOST
                    aria-label=tr("ui-close")
                    title=tr("ui-close")
                    on:click=move |_| on_close.run(())
                >
                    <Icon name=IconName::Close />
                </button>
            </div>
            <div class="flex-1 overflow-y-auto px-5 py-4">{children()}</div>
        </aside>
    }
}
