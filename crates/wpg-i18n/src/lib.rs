//! Tiny Fluent loader shared by the server, the CLI and the WASM front-end.
//!
//! The `.ftl` sources live in `locales/<lang>/main.ftl` at the repository root and are
//! embedded with [`include_str!`], so this crate has no runtime file I/O and builds for
//! `wasm32-unknown-unknown`.
//!
//! ```
//! use wpg_i18n::{t, Locale};
//! assert_eq!(t(Locale::En, "error-not-found"), "Not found");
//! ```

use std::cell::RefCell;
use std::fmt;
use std::str::FromStr;

use fluent_bundle::{FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

pub use fluent_bundle::{FluentArgs, FluentValue};

/// Raw Fluent source for English (the base / fallback locale).
pub const EN_FTL: &str = include_str!("../../../locales/en/main.ftl");
/// Raw Fluent source for Japanese.
pub const JA_FTL: &str = include_str!("../../../locales/ja/main.ftl");

/// Locales supported by wpg. Add a variant + a `locales/<lang>/main.ftl` to add a language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Locale {
    /// Japanese.
    Ja,
    /// English (default and fallback).
    #[default]
    En,
}

impl Locale {
    /// All supported locales, in a stable order.
    pub const ALL: [Locale; 2] = [Locale::Ja, Locale::En];

    /// BCP 47 language tag.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Locale::Ja => "ja",
            Locale::En => "en",
        }
    }

    /// Fluent source for this locale.
    #[must_use]
    pub const fn source(self) -> &'static str {
        match self {
            Locale::Ja => JA_FTL,
            Locale::En => EN_FTL,
        }
    }

    fn from_langid(id: &LanguageIdentifier) -> Option<Self> {
        match id.language.as_str() {
            "ja" => Some(Locale::Ja),
            "en" => Some(Locale::En),
            _ => None,
        }
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Locale {
    type Err = UnsupportedLocale;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_tag(s)
            .as_ref()
            .and_then(Locale::from_langid)
            .ok_or_else(|| UnsupportedLocale(s.to_owned()))
    }
}

/// Returned by [`Locale::from_str`] for tags that do not map to a supported locale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedLocale(pub String);

impl fmt::Display for UnsupportedLocale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unsupported locale: {:?}", self.0)
    }
}

impl std::error::Error for UnsupportedLocale {}

/// Parses a single language tag, accepting POSIX style (`ja_JP.UTF-8`) as well as BCP 47.
fn parse_tag(tag: &str) -> Option<LanguageIdentifier> {
    let tag = tag.trim();
    // Strip POSIX codeset / modifier (`ja_JP.UTF-8@euro`) and normalise `_` to `-`.
    let tag = tag
        .split(['.', '@'])
        .next()
        .unwrap_or(tag)
        .replace('_', "-");
    if tag.is_empty() || tag == "*" || tag.eq_ignore_ascii_case("c") || tag == "POSIX" {
        return None;
    }
    tag.parse().ok()
}

/// Picks the best supported locale from an HTTP `Accept-Language` header value, a
/// browser `navigator.language` string, or a POSIX `LANG` value.
///
/// Quality values (`;q=`) are honoured; ties keep header order. Falls back to
/// [`Locale::En`].
#[must_use]
pub fn negotiate(header: &str) -> Locale {
    let mut candidates: Vec<(usize, f32, &str)> = header
        .split(',')
        .enumerate()
        .filter_map(|(idx, part)| {
            let mut pieces = part.split(';');
            let tag = pieces.next()?.trim();
            let q = pieces
                .filter_map(|p| p.trim().strip_prefix("q="))
                .find_map(|q| q.trim().parse::<f32>().ok())
                .unwrap_or(1.0);
            (q > 0.0 && !tag.is_empty()).then_some((idx, q, tag))
        })
        .collect();
    candidates.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    candidates
        .into_iter()
        .find_map(|(_, _, tag)| tag.parse::<Locale>().ok())
        .unwrap_or_default()
}

type Bundle = FluentBundle<FluentResource>;

fn build_bundle(locale: Locale) -> Bundle {
    let langid: LanguageIdentifier = locale
        .as_str()
        .parse()
        .expect("static locale tags are valid");
    let resource = FluentResource::try_new(locale.source().to_owned())
        .unwrap_or_else(|(_, errors)| panic!("invalid Fluent syntax in {locale}: {errors:?}"));
    let mut bundle = FluentBundle::new(vec![langid]);
    // Unicode isolation marks (U+2068/U+2069) around placeables are noise in terminals,
    // JSON and logs; ja/en are both LTR so we do not need them.
    bundle.set_use_isolating(false);
    bundle
        .add_resource(resource)
        .unwrap_or_else(|errors| panic!("duplicate Fluent messages in {locale}: {errors:?}"));
    bundle
}

thread_local! {
    // `FluentBundle`'s default memoizer is not `Sync`; one lazily built bundle per thread
    // is cheap (few KiB) and works identically on native and on WASM.
    static BUNDLES: RefCell<[Option<Bundle>; 2]> = const { RefCell::new([None, None]) };
}

fn with_bundle<R>(locale: Locale, f: impl FnOnce(&Bundle) -> R) -> R {
    let idx = match locale {
        Locale::Ja => 0,
        Locale::En => 1,
    };
    BUNDLES.with(|cell| {
        let mut bundles = cell.borrow_mut();
        f(bundles[idx].get_or_insert_with(|| build_bundle(locale)))
    })
}

fn format(locale: Locale, key: &str, args: Option<&FluentArgs<'_>>) -> Option<String> {
    with_bundle(locale, |bundle| {
        let pattern = bundle.get_message(key)?.value()?;
        let mut errors = Vec::new();
        let text = bundle.format_pattern(pattern, args, &mut errors);
        Some(text.into_owned())
    })
}

/// Translates `key` for `locale`, with Fluent arguments.
///
/// Missing keys fall back to English, and finally to the key itself so that a missing
/// translation is visible but never fatal.
#[must_use]
pub fn t_args(locale: Locale, key: &str, args: &FluentArgs<'_>) -> String {
    format(locale, key, Some(args))
        .or_else(|| format(Locale::En, key, Some(args)))
        .unwrap_or_else(|| key.to_owned())
}

/// Translates `key` for `locale` (no arguments). See [`t_args`] for fallback rules.
#[must_use]
pub fn t(locale: Locale, key: &str) -> String {
    format(locale, key, None)
        .or_else(|| format(Locale::En, key, None))
        .unwrap_or_else(|| key.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn message_ids(source: &str) -> BTreeSet<String> {
        let resource = fluent_syntax::parser::parse(source).expect("ftl must parse cleanly");
        resource
            .body
            .iter()
            .filter_map(|entry| match entry {
                fluent_syntax::ast::Entry::Message(m) => Some(m.id.name.to_owned()),
                fluent_syntax::ast::Entry::Term(t) => Some(format!("-{}", t.id.name)),
                _ => None,
            })
            .collect()
    }

    /// The "i18n 抜け漏れ lint" (TODO §5): every locale has exactly the same keys.
    #[test]
    fn all_locales_have_identical_key_sets() {
        let en = message_ids(EN_FTL);
        assert!(!en.is_empty());
        for locale in Locale::ALL {
            let other = message_ids(locale.source());
            let missing: Vec<_> = en.difference(&other).collect();
            let extra: Vec<_> = other.difference(&en).collect();
            assert!(missing.is_empty(), "{locale} is missing keys: {missing:?}");
            assert!(extra.is_empty(), "{locale} has keys not in en: {extra:?}");
        }
    }

    #[test]
    fn every_bundle_builds_and_formats_every_key() {
        for locale in Locale::ALL {
            for key in message_ids(locale.source()) {
                let mut args = FluentArgs::new();
                args.set("addr", "127.0.0.1:8080");
                args.set("command", "send");
                let text = t_args(locale, &key, &args);
                assert_ne!(text, key, "{locale}:{key} did not resolve");
                assert!(
                    !text.contains("{"),
                    "{locale}:{key} left a placeable: {text}"
                );
            }
        }
    }

    #[test]
    fn translates_and_interpolates() {
        assert_eq!(t(Locale::Ja, "error-not-found"), "見つかりません");
        let mut args = FluentArgs::new();
        args.set("command", "send");
        assert_eq!(
            t_args(Locale::En, "error-not-implemented", &args),
            "`send` is not implemented yet."
        );
    }

    #[test]
    fn unknown_key_falls_back_to_key() {
        assert_eq!(t(Locale::Ja, "no-such-key"), "no-such-key");
    }

    #[test]
    fn negotiation() {
        assert_eq!(negotiate("ja-JP,ja;q=0.9,en-US;q=0.8,en;q=0.7"), Locale::Ja);
        assert_eq!(negotiate("en-US,en;q=0.9,ja;q=0.8"), Locale::En);
        assert_eq!(negotiate("fr-FR, ja;q=0.5, en;q=0.4"), Locale::Ja);
        assert_eq!(negotiate("en;q=0.1, ja;q=0.9"), Locale::Ja);
        assert_eq!(negotiate("ja;q=0, en"), Locale::En);
        assert_eq!(negotiate("ja"), Locale::Ja);
        assert_eq!(negotiate("ja_JP.UTF-8"), Locale::Ja);
        assert_eq!(negotiate("C.UTF-8"), Locale::En);
        assert_eq!(negotiate("de-DE"), Locale::En);
        assert_eq!(negotiate(""), Locale::En);
        assert_eq!(negotiate("*"), Locale::En);
    }

    #[test]
    fn from_str_roundtrip() {
        for locale in Locale::ALL {
            assert_eq!(locale.as_str().parse::<Locale>(), Ok(locale));
        }
        assert!("xx".parse::<Locale>().is_err());
    }
}
