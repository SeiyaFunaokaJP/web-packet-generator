//! Root component: contexts, router, application shell and global shortcuts.

use leptos::prelude::*;
use leptos_router::components::{Outlet, ParentRoute, Route, Router, Routes};
use leptos_router::hooks::{use_location, use_navigate};
use leptos_router::{path, NavigateOptions};
use wasm_bindgen::JsCast;
use wpg_i18n::Locale;

use crate::auth::{redirect_for, use_auth, Auth, AuthPhase};
use crate::components::{
    Icon, IconName, LogsPanel, Modal, ToastRegion, Toasts, BTN, BTN_GHOST, BTN_PRIMARY,
};
use crate::i18n::{locale_label_key, tr, use_i18n, I18n};
use crate::prefs::{self, Theme};
use crate::shortcuts::{self, Action, Screen};
use crate::ws::{self, use_live, Live, WsStatus};
use crate::{api, builder, pages};

/// Shell-wide UI state (context).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ui {
    pub(crate) logs_open: RwSignal<bool>,
    pub(crate) help_open: RwSignal<bool>,
    pub(crate) theme: RwSignal<Theme>,
}

/// The [`Ui`] context.
pub(crate) fn use_ui() -> Ui {
    expect_context::<Ui>()
}

/// Root component.
#[component]
pub(crate) fn App() -> impl IntoView {
    let i18n = I18n {
        locale: RwSignal::new(prefs::load_locale()),
    };
    let ui = Ui {
        logs_open: RwSignal::new(false),
        help_open: RwSignal::new(false),
        theme: RwSignal::new(prefs::load_theme()),
    };
    let auth = Auth::new();
    let live = Live::new();
    provide_context(i18n);
    provide_context(ui);
    provide_context(auth);
    provide_context(live);
    provide_context(Toasts::new());

    // Language: <html lang>, Accept-Language of API calls, persistence.
    Effect::new(move |_| {
        let l = i18n.locale.get();
        api::set_locale(l);
        prefs::save_locale(l);
        if let Some(root) = document().document_element() {
            let _ = root.set_attribute("lang", l.as_str());
        }
        document().set_title(&wpg_i18n::t(l, "app-name"));
    });
    // Theme.
    Effect::new(move |_| {
        let t = ui.theme.get();
        prefs::apply_theme(t);
        prefs::save_theme(t);
    });

    api::set_locale(i18n.current());
    api::set_unauthorized_hook(move || auth.refresh_later());
    auth.refresh_later();
    // The event stream needs a session: connect once the app may be shown, and
    // skip the back-off whenever that becomes true again (after a login).
    let ws_started = StoredValue::new(false);
    Effect::new(move |_| {
        if auth.phase.with(AuthPhase::allows_app) {
            if ws_started.get_value() {
                ws::reconnect_now();
            } else {
                ws_started.set_value(true);
                ws::connect(live);
            }
        }
    });

    view! {
        <Router>
            <Guard />
            <Shortcuts />
            <Routes fallback=|| view! { <pages::NotFound /> }>
                <Route path=path!("/login") view=pages::auth::LoginPage />
                <Route path=path!("/setup") view=pages::auth::SetupPage />
                <ParentRoute path=path!("") view=Shell>
                    <Route path=path!("") view=pages::dashboard::DashboardPage />
                    <Route path=path!("builder") view=builder::BuilderPage />
                    <Route path=path!("jobs") view=pages::jobs::JobsPage />
                    <Route path=path!("templates") view=pages::templates::TemplatesPage />
                    <Route path=path!("rx") view=pages::RxPage />
                    <Route path=path!("settings") view=pages::settings::SettingsPage />
                    <Route path=path!("*any") view=pages::NotFound />
                </ParentRoute>
            </Routes>
        </Router>
    }
}

/// Sends the user to `/setup` / `/login` (or away from them) as the session
/// requires.
#[component]
fn Guard() -> impl IntoView {
    let auth = use_auth();
    let location = use_location();
    let navigate = use_navigate();
    Effect::new(move |_| {
        let phase = auth.phase.get();
        let path = location.pathname.get();
        let search = location.search.get();
        if let Some(to) = redirect_for(&phase, &path, &search) {
            navigate(
                &to,
                NavigateOptions {
                    replace: true,
                    ..NavigateOptions::default()
                },
            );
        }
    });
}

/// Global keyboard shortcuts (`g d`, `g j`, `?`, ...).
#[component]
fn Shortcuts() -> impl IntoView {
    let ui = use_ui();
    let auth = use_auth();
    let navigate = use_navigate();
    let parser = StoredValue::new(shortcuts::Parser::default());
    let handle = window_event_listener(leptos::ev::keydown, move |ev| {
        if ev.default_prevented() || ev.is_composing() {
            return;
        }
        let typing = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok())
            .is_some_and(|el| {
                shortcuts::is_typing_target(&el.tag_name(), el.is_content_editable())
            });
        if typing && ev.key() != "Escape" {
            return;
        }
        let modifier = ev.ctrl_key() || ev.meta_key() || ev.alt_key();
        let now = crate::dom::now_ms();
        let Some(action) = parser
            .try_update_value(|p| p.feed(&ev.key(), modifier, now))
            .flatten()
        else {
            return;
        };
        if !auth.phase.with_untracked(AuthPhase::allows_app) && action != Action::Escape {
            return;
        }
        match action {
            Action::Go(screen) => {
                ev.prevent_default();
                navigate(screen.path(), NavigateOptions::default());
            }
            Action::NewJob => {
                ev.prevent_default();
                navigate("/jobs?new=1", NavigateOptions::default());
            }
            Action::Help => {
                ev.prevent_default();
                ui.help_open.update(|o| *o = !*o);
            }
            Action::ToggleLogs => ui.logs_open.update(|o| *o = !*o),
            Action::Escape => ui.help_open.set(false),
        }
    });
    // The shell lives as long as the page; keep the listener.
    std::mem::forget(handle);
}

struct NavItem {
    screen: Screen,
    key: &'static str,
    icon: IconName,
    chord: &'static str,
}

const NAV: [NavItem; 6] = [
    NavItem {
        screen: Screen::Dashboard,
        key: "ui-nav-dashboard",
        icon: IconName::Dashboard,
        chord: "g d",
    },
    NavItem {
        screen: Screen::Builder,
        key: "ui-nav-builder",
        icon: IconName::Builder,
        chord: "g b",
    },
    NavItem {
        screen: Screen::Jobs,
        key: "ui-nav-jobs",
        icon: IconName::Jobs,
        chord: "g j",
    },
    NavItem {
        screen: Screen::Templates,
        key: "ui-nav-templates",
        icon: IconName::Templates,
        chord: "g t",
    },
    NavItem {
        screen: Screen::Rx,
        key: "ui-nav-rx",
        icon: IconName::Rx,
        chord: "g r",
    },
    NavItem {
        screen: Screen::Settings,
        key: "ui-nav-settings",
        icon: IconName::Settings,
        chord: "g s",
    },
];

fn is_active(screen: Screen, path: &str) -> bool {
    match screen {
        Screen::Dashboard => path == "/",
        s => path == s.path() || path.starts_with(&format!("{}/", s.path())),
    }
}

/// Layout of every authenticated screen.
#[component]
fn Shell() -> impl IntoView {
    let ui = use_ui();
    let auth = use_auth();
    let i18n = use_i18n();
    let location = use_location();

    let links = move |mobile: bool| {
        NAV.iter()
            .map(|item| {
                let screen = item.screen;
                let active = move || is_active(screen, &location.pathname.get());
                let class = move || {
                    let base = if mobile {
                        "flex shrink-0 items-center gap-1.5 rounded-md px-2.5 py-1.5 text-sm"
                    } else {
                        "flex items-center gap-3 rounded-md px-3 py-2 text-sm"
                    };
                    let state = if active() {
                        "bg-sky-50 font-semibold text-sky-800 dark:bg-sky-500/10 dark:text-sky-300"
                    } else {
                        "text-slate-600 hover:bg-slate-100 hover:text-slate-900 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-100"
                    };
                    format!("{base} {state}")
                };
                let key = item.key;
                let icon = item.icon;
                let chord = item.chord;
                view! {
                    <a
                        href=screen.path()
                        class=class
                        aria-current=move || active().then_some("page")
                        aria-keyshortcuts=chord.replace(' ', "+")
                        title=move || format!("{} ({chord})", i18n.t(key))
                    >
                        <Icon name=icon class="h-5 w-5" />
                        <span>{move || i18n.t(key)}</span>
                    </a>
                }
            })
            .collect_view()
    };

    view! {
        <a
            href="#main"
            class="sr-only z-[70] rounded bg-white px-3 py-2 text-sm focus:not-sr-only focus:fixed focus:top-2 focus:left-2 dark:bg-slate-800"
        >
            {tr("ui-skip-to-content")}
        </a>
        <div class="flex h-full">
            <aside class="hidden w-56 shrink-0 flex-col border-r border-slate-200 bg-white md:flex dark:border-slate-800 dark:bg-slate-900">
                <div class="flex h-14 items-center gap-2 border-b border-slate-200 px-4 dark:border-slate-800">
                    <span class="grid h-7 w-7 place-items-center rounded-md bg-sky-600 text-xs font-bold text-white">
                        "wpg"
                    </span>
                    <span class="truncate text-sm font-semibold">{tr("app-name")}</span>
                </div>
                <nav aria-label=tr("ui-nav-label") class="flex flex-1 flex-col gap-1 p-3">
                    {links(false)}
                </nav>
                <p class="p-3 text-xs text-slate-400">
                    <kbd class="rounded border border-slate-300 px-1 dark:border-slate-700">"?"</kbd>
                    " "
                    {tr("ui-shortcut-hint")}
                </p>
            </aside>
            <div class="flex min-w-0 flex-1 flex-col">
                <TopBar />
                <nav
                    aria-label=tr("ui-nav-label")
                    class="flex gap-1 overflow-x-auto border-b border-slate-200 bg-white px-2 py-1.5 md:hidden dark:border-slate-800 dark:bg-slate-900"
                >
                    {links(true)}
                </nav>
                <main
                    id="main"
                    tabindex="-1"
                    class=move || {
                        if ui.logs_open.get() {
                            "flex-1 overflow-y-auto p-4 pb-72 focus:outline-none md:p-6 md:pb-72"
                        } else {
                            "flex-1 overflow-y-auto p-4 focus:outline-none md:p-6"
                        }
                    }
                >
                    {move || match auth.phase.get() {
                        AuthPhase::Loading => view! { <crate::components::Loading /> }.into_any(),
                        AuthPhase::Unreachable(_) => view! { <Unreachable /> }.into_any(),
                        p if p.allows_app() => view! { <Outlet /> }.into_any(),
                        _ => view! { <crate::components::Loading /> }.into_any(),
                    }}
                </main>
            </div>
        </div>
        <Show when=move || ui.logs_open.get()>
            <LogsPanel on_close=move || ui.logs_open.set(false) />
        </Show>
        <Show when=move || ui.help_open.get()>
            <HelpOverlay />
        </Show>
        <ToastRegion />
    }
}

/// Shown when `/api/auth/status` could not be fetched.
#[component]
fn Unreachable() -> impl IntoView {
    let auth = use_auth();
    view! {
        <div class="mx-auto max-w-md py-16 text-center" role="alert">
            <Icon name=IconName::Warning class="mx-auto h-10 w-10 text-amber-500" />
            <p class="mt-4 text-sm">{tr("ui-server-unreachable")}</p>
            <button type="button" class=format!("{BTN} mt-4") on:click=move |_| auth.refresh_later()>
                <Icon name=IconName::Refresh class="h-4 w-4" />
                {tr("ui-retry")}
            </button>
        </div>
    }
}

/// Top bar: connection state, language, theme, logs, help, logout.
#[component]
fn TopBar() -> impl IntoView {
    let ui = use_ui();
    let i18n = use_i18n();
    let auth = use_auth();
    let live = use_live();
    let toasts = crate::components::use_toasts();

    let dot = move || match live.status.get() {
        WsStatus::Open => "bg-emerald-500",
        WsStatus::Connecting => "bg-amber-400 animate-pulse",
        WsStatus::Closed => "bg-rose-500",
    };
    let ws_label = move || i18n.t(live.status.get().label_key());
    let dark = move || prefs::is_dark(ui.theme.get());
    let warn_count = move || {
        live.logs.with(|l| {
            l.iter()
                .filter(|e| crate::components::logs::level_rank(&e.line.level) >= 3)
                .count()
        })
    };
    let logout = move |_| {
        leptos::task::spawn_local(async move {
            if let Err(e) = auth.logout().await {
                toasts.error(e.message(i18n));
            }
        });
    };

    view! {
        <header class="flex h-14 shrink-0 items-center gap-2 border-b border-slate-200 bg-white px-3 md:px-5 dark:border-slate-800 dark:bg-slate-900">
            <span class="text-sm font-semibold md:hidden">{tr("app-name")}</span>
            <div class="ml-auto flex items-center gap-1 sm:gap-2">
                <span
                    class="flex items-center gap-1.5 rounded-full px-2 py-1 text-xs text-slate-500"
                    role="status"
                    title=ws_label
                >
                    <span class=move || format!("h-2.5 w-2.5 rounded-full {}", dot())></span>
                    <span class="hidden lg:inline">{ws_label}</span>
                    <span class="sr-only lg:hidden">{ws_label}</span>
                </span>
                <select
                    aria-label=tr("ui-language")
                    title=tr("ui-language")
                    class="max-w-28 rounded-md border border-slate-300 bg-white px-1.5 py-1 text-xs text-slate-700 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-200"
                    on:change=move |ev| {
                        if let Ok(l) = event_target_value(&ev).parse() {
                            i18n.locale.set(l);
                        }
                    }
                >
                    {Locale::ALL
                        .into_iter()
                        .map(|l| {
                            view! {
                                <option value=l.as_str() lang=l.as_str() selected=move || i18n.locale.get() == l>
                                    {move || i18n.t(locale_label_key(l))}
                                </option>
                            }
                        })
                        .collect_view()}
                </select>
                <button
                    type="button"
                    class=BTN_GHOST
                    aria-label=move || i18n.t(if dark() { "ui-theme-to-light" } else { "ui-theme-to-dark" })
                    title=move || i18n.t(if dark() { "ui-theme-to-light" } else { "ui-theme-to-dark" })
                    on:click=move |_| ui.theme.set(if dark() { Theme::Light } else { Theme::Dark })
                >
                    {move || {
                        if dark() {
                            view! { <Icon name=IconName::Sun /> }.into_any()
                        } else {
                            view! { <Icon name=IconName::Moon /> }.into_any()
                        }
                    }}
                </button>
                <button
                    type="button"
                    class=move || format!("{BTN_GHOST} relative")
                    aria-pressed=move || ui.logs_open.get().to_string()
                    aria-keyshortcuts="l"
                    aria-label=tr("ui-logs-toggle")
                    title=move || format!("{} (l)", i18n.t("ui-logs-toggle"))
                    on:click=move |_| ui.logs_open.update(|o| *o = !*o)
                >
                    <Icon name=IconName::Logs />
                    <Show when=move || { warn_count() > 0 }>
                        <span class="absolute -top-0.5 -right-0.5 rounded-full bg-amber-500 px-1 text-[10px] leading-4 font-bold text-white">
                            {move || warn_count().min(99)}
                        </span>
                    </Show>
                </button>
                <button
                    type="button"
                    class=BTN_GHOST
                    aria-keyshortcuts="Shift+?"
                    aria-label=tr("ui-help")
                    title=move || format!("{} (?)", i18n.t("ui-help"))
                    on:click=move |_| ui.help_open.set(true)
                >
                    <Icon name=IconName::Help />
                </button>
                <Show when=move || auth.enabled.get() && matches!(auth.phase.get(), AuthPhase::Authenticated { .. })>
                    <button
                        type="button"
                        class=BTN_GHOST
                        aria-label=tr("ui-logout")
                        title=tr("ui-logout")
                        on:click=logout
                    >
                        <Icon name=IconName::Logout />
                    </button>
                </Show>
            </div>
        </header>
    }
}

/// `?` overlay listing the shortcuts.
#[component]
fn HelpOverlay() -> impl IntoView {
    let ui = use_ui();
    let i18n = use_i18n();
    view! {
        <Modal title=tr("ui-help-title") on_close=move || ui.help_open.set(false)>
            <table class="w-full text-sm">
                <caption class="sr-only">{tr("ui-help-title")}</caption>
                <tbody>
                    {shortcuts::HELP
                        .iter()
                        .map(|(keys, key)| {
                            view! {
                                <tr class="border-b border-slate-100 last:border-0 dark:border-slate-800">
                                    <td class="py-1.5 pr-4 whitespace-nowrap">
                                        {keys
                                            .split(' ')
                                            .map(|k| {
                                                view! {
                                                    <kbd class="mr-1 rounded border border-slate-300 bg-slate-50 px-1.5 py-0.5 font-mono text-xs dark:border-slate-700 dark:bg-slate-800">
                                                        {k}
                                                    </kbd>
                                                }
                                            })
                                            .collect_view()}
                                    </td>
                                    <td class="py-1.5">{move || i18n.t(key)}</td>
                                </tr>
                            }
                        })
                        .collect_view()}
                </tbody>
            </table>
            <div class="mt-4 flex justify-end">
                <button type="button" class=BTN_PRIMARY on:click=move |_| ui.help_open.set(false)>
                    {tr("ui-close")}
                </button>
            </div>
        </Modal>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_nav() {
        assert!(is_active(Screen::Dashboard, "/"));
        assert!(!is_active(Screen::Dashboard, "/jobs"));
        assert!(is_active(Screen::Jobs, "/jobs"));
        assert!(is_active(Screen::Jobs, "/jobs/abc"));
        assert!(!is_active(Screen::Jobs, "/jobsx"));
    }
}
