//! Shared UI building blocks. Tailwind classes are written as full literals (the
//! Tailwind CLI scans the sources).

pub(crate) mod logs;
pub(crate) mod misc;
pub(crate) mod modal;

use leptos::prelude::*;

pub(crate) use logs::LogsPanel;
pub(crate) use misc::*;
pub(crate) use modal::{Confirm, Drawer, Modal};

/// Primary button.
pub(crate) const BTN_PRIMARY: &str = "inline-flex items-center justify-center gap-1.5 rounded-md bg-sky-600 px-3 py-1.5 text-sm font-medium text-white shadow-sm hover:bg-sky-700 disabled:cursor-not-allowed disabled:opacity-50 dark:bg-sky-500 dark:hover:bg-sky-400 dark:text-slate-950";
/// Secondary button.
pub(crate) const BTN: &str = "inline-flex items-center justify-center gap-1.5 rounded-md border border-slate-300 bg-white px-3 py-1.5 text-sm font-medium text-slate-700 shadow-sm hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700";
/// Destructive button.
pub(crate) const BTN_DANGER: &str = "inline-flex items-center justify-center gap-1.5 rounded-md bg-rose-600 px-3 py-1.5 text-sm font-medium text-white shadow-sm hover:bg-rose-700 disabled:cursor-not-allowed disabled:opacity-50";
/// Icon-only / low emphasis button.
pub(crate) const BTN_GHOST: &str = "inline-flex items-center justify-center gap-1 rounded-md p-1.5 text-slate-500 hover:bg-slate-100 hover:text-slate-900 disabled:opacity-40 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-100";
/// Text inputs / selects.
pub(crate) const INPUT: &str = "block w-full rounded-md border border-slate-300 bg-white px-2.5 py-1.5 text-sm shadow-sm placeholder:text-slate-400 focus:border-sky-500 focus:ring-1 focus:ring-sky-500 focus:outline-none disabled:opacity-50 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-100";
/// Invalid input.
pub(crate) const INPUT_INVALID: &str = "block w-full rounded-md border border-rose-500 bg-white px-2.5 py-1.5 text-sm shadow-sm focus:border-rose-500 focus:ring-1 focus:ring-rose-500 focus:outline-none dark:bg-slate-900 dark:text-slate-100";
/// Form labels.
pub(crate) const LABEL: &str = "mb-1 block text-xs font-medium text-slate-600 dark:text-slate-400";
/// Cards.
pub(crate) const CARD: &str = "rounded-lg border border-slate-200 bg-white p-4 shadow-sm dark:border-slate-800 dark:bg-slate-900";
/// Table header cells.
pub(crate) const TH: &str = "px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400";
/// Table cells.
pub(crate) const TD: &str = "px-3 py-2 text-sm align-top";
/// Numeric table cells.
pub(crate) const TD_NUM: &str = "px-3 py-2 text-right text-sm tabular-nums align-top";

/// Icons (24 × 24 outline paths, drawn for wpg).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IconName {
    Dashboard,
    Builder,
    Jobs,
    Templates,
    Rx,
    Settings,
    Logs,
    Help,
    Sun,
    Moon,
    Plus,
    Close,
    Star,
    Copy,
    Trash,
    Download,
    Upload,
    Play,
    Pause,
    Stop,
    Refresh,
    Logout,
    Edit,
    Search,
    External,
    Check,
    Warning,
}

impl IconName {
    const fn path(self) -> &'static str {
        match self {
            IconName::Dashboard => "M4 4h7v8H4zM13 4h7v5h-7zM13 11h7v9h-7zM4 14h7v6H4z",
            IconName::Builder => "M12 3 3 7.5l9 4.5 9-4.5L12 3zM3 12l9 4.5 9-4.5M3 16.5 12 21l9-4.5",
            IconName::Jobs => "M13 2 4 14h7l-1 8 9-12h-7l1-8z",
            IconName::Templates => "M7 3h7l5 5v13H7zM14 3v5h5M10 13h6M10 17h6",
            IconName::Rx => "M3 13h5l2 3h4l2-3h5M3 13l2.5-8h13L21 13v7H3z",
            IconName::Settings => "M4 6h9M17 6h3M4 12h3M11 12h9M4 18h11M19 18h1M13 6a2 2 0 1 0 4 0 2 2 0 1 0-4 0M7 12a2 2 0 1 0 4 0 2 2 0 1 0-4 0M15 18a2 2 0 1 0 4 0 2 2 0 1 0-4 0",
            IconName::Logs => "M4 5h16v14H4zM7.5 9.5 10 12l-2.5 2.5M12 15h4.5",
            IconName::Help => "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .8-1 1.5v.7M12 17h.01",
            IconName::Sun => "M12 16.5a4.5 4.5 0 1 0 0-9 4.5 4.5 0 0 0 0 9zM12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4",
            IconName::Moon => "M20.5 14.5A8.5 8.5 0 0 1 9.5 3.5a8.5 8.5 0 1 0 11 11z",
            IconName::Plus => "M12 5v14M5 12h14",
            IconName::Close => "M6 6l12 12M18 6 6 18",
            IconName::Star => "M12 3.5l2.6 5.3 5.9.9-4.3 4.1 1 5.8L12 16.9l-5.2 2.7 1-5.8-4.3-4.1 5.9-.9z",
            IconName::Copy => "M9 9h11v11H9zM5 15H4V4h11v1",
            IconName::Trash => "M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3",
            IconName::Download => "M12 4v11M7 10.5l5 5 5-5M5 20h14",
            IconName::Upload => "M12 20V9M7 13.5l5-5 5 5M5 4h14",
            IconName::Play => "M7 4.5v15l12.5-7.5z",
            IconName::Pause => "M7 5h3.5v14H7zM13.5 5H17v14h-3.5z",
            IconName::Stop => "M6 6h12v12H6z",
            IconName::Refresh => "M20 12a8 8 0 1 1-2.35-5.65M20 4v5h-5",
            IconName::Logout => "M14 4h5v16h-5M10 8l-4 4 4 4M6 12h10",
            IconName::Edit => "M4 20h4L19 9l-4-4L4 16v4zM13.5 6.5l4 4",
            IconName::Search => "M10.5 17.5a7 7 0 1 0 0-14 7 7 0 0 0 0 14zM20 20l-4.5-4.5",
            IconName::External => "M14 4h6v6M20 4l-9 9M18 14v6H4V6h6",
            IconName::Check => "M5 12.5l4.5 4.5L19 7.5",
            IconName::Warning => "M12 3.5 2.5 20h19L12 3.5zM12 10v4.5M12 17.5h.01",
        }
    }
}

/// An inline SVG icon (decorative: `aria-hidden`; label the button instead).
#[component]
pub(crate) fn Icon(
    name: IconName,
    #[prop(optional, into)] class: Option<String>,
    /// Fill the shape (e.g. an active star).
    #[prop(optional, into)]
    filled: Signal<bool>,
) -> impl IntoView {
    let class = class.unwrap_or_else(|| "h-5 w-5".to_owned());
    view! {
        <svg
            class=format!("shrink-0 {class}")
            viewBox="0 0 24 24"
            fill=move || if filled.get() { "currentColor" } else { "none" }
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
        >
            <path d=name.path()></path>
        </svg>
    }
}
