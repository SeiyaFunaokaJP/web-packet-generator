//! Client preferences persisted in `localStorage` (language, theme, logs level).

use wpg_i18n::Locale;

const KEY_LOCALE: &str = "wpg.locale";
const KEY_THEME: &str = "wpg.theme";

/// Reads a `localStorage` entry (`None` natively / when storage is disabled).
pub(crate) fn load(key: &str) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()?
            .local_storage()
            .ok()??
            .get_item(key)
            .ok()?
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = key;
        None
    }
}

/// Writes a `localStorage` entry (ignored when storage is unavailable).
pub(crate) fn store(key: &str, value: &str) {
    #[cfg(target_arch = "wasm32")]
    if let Some(s) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = s.set_item(key, value);
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (key, value);
}

/// Colour scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Theme {
    Light,
    Dark,
    /// Follow `prefers-color-scheme`.
    #[default]
    System,
}

impl Theme {
    pub(crate) const ALL: [Theme; 3] = [Theme::Light, Theme::Dark, Theme::System];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
            Theme::System => "system",
        }
    }

    pub(crate) fn parse(s: &str) -> Option<Self> {
        Theme::ALL.into_iter().find(|t| t.as_str() == s)
    }

    /// i18n key of the name.
    pub(crate) const fn label_key(self) -> &'static str {
        match self {
            Theme::Light => "ui-theme-light",
            Theme::Dark => "ui-theme-dark",
            Theme::System => "ui-theme-system",
        }
    }
}

/// Whether the OS prefers a dark scheme.
pub(crate) fn system_prefers_dark() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.match_media("(prefers-color-scheme: dark)").ok().flatten())
            .is_some_and(|m| m.matches())
    }
    #[cfg(not(target_arch = "wasm32"))]
    false
}

/// Resolves [`Theme::System`].
pub(crate) fn is_dark(theme: Theme) -> bool {
    match theme {
        Theme::Light => false,
        Theme::Dark => true,
        Theme::System => system_prefers_dark(),
    }
}

/// Sets / clears `class="dark"` on `<html>`.
pub(crate) fn apply_theme(theme: Theme) {
    #[cfg(target_arch = "wasm32")]
    if let Some(root) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
    {
        let _ = root.class_list().toggle_with_force("dark", is_dark(theme));
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = theme;
}

pub(crate) fn load_theme() -> Theme {
    load(KEY_THEME)
        .and_then(|s| Theme::parse(&s))
        .unwrap_or_default()
}

pub(crate) fn save_theme(theme: Theme) {
    store(KEY_THEME, theme.as_str());
}

/// Saved language, else `navigator.language`, else English.
pub(crate) fn load_locale() -> Locale {
    if let Some(l) = load(KEY_LOCALE).and_then(|s| s.parse().ok()) {
        return l;
    }
    #[cfg(target_arch = "wasm32")]
    if let Some(lang) = web_sys::window().and_then(|w| {
        let langs = w.navigator().languages();
        let joined: Vec<String> = langs.iter().filter_map(|v| v.as_string()).collect();
        if joined.is_empty() {
            w.navigator().language()
        } else {
            Some(joined.join(","))
        }
    }) {
        return wpg_i18n::negotiate(&lang);
    }
    Locale::default()
}

pub(crate) fn save_locale(l: Locale) {
    store(KEY_LOCALE, l.as_str());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_round_trip() {
        for t in Theme::ALL {
            assert_eq!(Theme::parse(t.as_str()), Some(t));
        }
        assert_eq!(Theme::parse("sepia"), None);
        assert!(is_dark(Theme::Dark) && !is_dark(Theme::Light));
        assert_eq!(load_theme(), Theme::System, "no storage natively");
        assert_eq!(load_locale(), Locale::En);
    }
}
