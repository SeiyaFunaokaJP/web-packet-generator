//! Reactive translations on top of `wpg-i18n` (Fluent, `locales/*/main.ftl`).
//!
//! Every UI string is a `ui-*` key; see the `ui_keys_exist` test, which scans the
//! sources for them.

use leptos::prelude::*;
use wpg_i18n::{FluentArgs, Locale};

/// The current UI language (context).
#[derive(Debug, Clone, Copy)]
pub(crate) struct I18n {
    pub(crate) locale: RwSignal<Locale>,
}

impl I18n {
    /// Translates `key` (tracks the locale when called in a reactive scope).
    pub(crate) fn t(self, key: &str) -> String {
        wpg_i18n::t(self.locale.get(), key)
    }

    /// Translates `key` with string arguments.
    pub(crate) fn ta(self, key: &str, args: &[(&'static str, String)]) -> String {
        let mut a = FluentArgs::new();
        for (k, v) in args {
            a.set(*k, v.clone());
        }
        wpg_i18n::t_args(self.locale.get(), key, &a)
    }

    /// Translates `key` with dynamic (owned) argument names, e.g. the arguments of a
    /// `wpg_core::Diagnostic`.
    pub(crate) fn t_dyn(
        self,
        key: &str,
        args: &std::collections::BTreeMap<String, String>,
    ) -> String {
        let mut a = FluentArgs::new();
        for (k, v) in args {
            a.set(k.clone(), v.clone());
        }
        wpg_i18n::t_args(self.locale.get(), key, &a)
    }

    /// The locale without tracking.
    pub(crate) fn current(self) -> Locale {
        self.locale.get_untracked()
    }
}

/// The [`I18n`] context.
pub(crate) fn use_i18n() -> I18n {
    expect_context::<I18n>()
}

/// A reactive translation usable directly as view text or attribute value:
/// `{tr("ui-nav-jobs")}`, `aria-label=tr("ui-close")`.
pub(crate) fn tr(key: &'static str) -> impl Fn() -> String + Copy + Send + Sync + 'static {
    let i18n = use_i18n();
    move || i18n.t(key)
}

/// Native name of a locale for the language switch.
pub(crate) fn locale_label_key(l: Locale) -> &'static str {
    match l {
        Locale::Ja => "ui-lang-ja",
        Locale::En => "ui-lang-en",
        Locale::ZhCn => "ui-lang-zh-cn",
        Locale::ZhTw => "ui-lang-zh-tw",
        Locale::Ko => "ui-lang-ko",
        Locale::De => "ui-lang-de",
        Locale::Fr => "ui-lang-fr",
        Locale::Es => "ui-lang-es",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    /// Every `"ui-..."` literal in the sources exists in every locale file (the
    /// "Fluent キー未定義検出" of spec §5, for the web UI).
    #[test]
    fn ui_keys_exist() {
        const SOURCES: &[&str] = &[
            include_str!("app.rs"),
            include_str!("api/mod.rs"),
            include_str!("auth.rs"),
            include_str!("builder/mod.rs"),
            include_str!("builder/palette.rs"),
            include_str!("builder/stack.rs"),
            include_str!("builder/fields.rs"),
            include_str!("builder/hex.rs"),
            include_str!("builder/tree.rs"),
            include_str!("components/mod.rs"),
            include_str!("components/logs.rs"),
            include_str!("components/modal.rs"),
            include_str!("components/misc.rs"),
            include_str!("i18n.rs"),
            include_str!("pages/auth.rs"),
            include_str!("pages/dashboard.rs"),
            include_str!("pages/job_detail.rs"),
            include_str!("pages/job_form.rs"),
            include_str!("pages/jobs.rs"),
            include_str!("pages/mod.rs"),
            include_str!("pages/rx.rs"),
            include_str!("pages/settings.rs"),
            include_str!("pages/templates.rs"),
            include_str!("pages/terminal.rs"),
            include_str!("term/mod.rs"),
            include_str!("term/session.rs"),
            include_str!("prefs.rs"),
            include_str!("shortcuts.rs"),
            include_str!("validate.rs"),
            include_str!("ws.rs"),
        ];
        let mut used = BTreeSet::new();
        for src in SOURCES {
            for part in src.split('"').skip(1).step_by(2) {
                if part.starts_with("ui-")
                    && part.len() > 3
                    && part
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                    && !part.ends_with('-')
                {
                    used.insert(part.to_owned());
                }
            }
        }
        assert!(used.len() > 50, "scan found only {} keys", used.len());
        for locale in wpg_i18n::Locale::ALL {
            let src = locale.source();
            let missing: Vec<_> = used
                .iter()
                .filter(|k| !src.contains(&format!("\n{k} =")))
                .collect();
            assert!(missing.is_empty(), "{locale} lacks {missing:?}");
        }
    }
}
