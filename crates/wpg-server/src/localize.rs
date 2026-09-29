//! Localisation helpers for API payloads (`Accept-Language`).

use axum::http::{header, HeaderMap};
use serde_json::{json, Value};
use wpg_core::Diagnostic;
use wpg_i18n::{negotiate, t, t_args, t_diag, FluentArgs, Locale};
use wpg_template::LintIssue;

/// Locale requested by the client via `Accept-Language` (default English).
#[must_use]
pub fn request_locale(headers: &HeaderMap) -> Locale {
    headers
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok())
        .map(negotiate)
        .unwrap_or_default()
}

/// Translates `key` with string arguments.
#[must_use]
pub fn tr(locale: Locale, key: &str, args: &[(&str, String)]) -> String {
    let mut a = FluentArgs::new();
    for (k, v) in args {
        a.set(*k, v.clone());
    }
    t_args(locale, key, &a)
}

/// A lint issue as JSON with a localised `message` (English fallback text when the
/// key has no translation).
#[must_use]
pub fn lint_issue_json(issue: &LintIssue, locale: Locale) -> Value {
    let mut a = FluentArgs::new();
    for (k, v) in &issue.args {
        a.set(k.as_str(), v.as_str());
    }
    let text = t_args(locale, &issue.key, &a);
    let message = if text == issue.key {
        issue.message.clone()
    } else {
        text
    };
    json!({
        "level": issue.level,
        "path": issue.path,
        "key": issue.key,
        "args": issue.args,
        "message": message,
    })
}

/// A diagnostic as JSON with a localised `message`.
#[must_use]
pub fn diagnostic_json(d: &Diagnostic, locale: Locale) -> Value {
    let mut v = serde_json::to_value(d).unwrap_or(Value::Null);
    if let Value::Object(m) = &mut v {
        m.insert("message".into(), Value::String(t_diag(locale, d)));
    }
    v
}

/// Adds a localised `description` next to every `description_key` in `v`
/// (protocol fields, TLV variants), recursively.
pub fn add_descriptions(v: &mut Value, locale: Locale) {
    match v {
        Value::Object(m) => {
            if let Some(Value::String(key)) = m.get("description_key") {
                let text = t(locale, key);
                if text != *key && !m.contains_key("description") {
                    m.insert("description".into(), Value::String(text));
                }
            }
            for child in m.values_mut() {
                add_descriptions(child, locale);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|c| add_descriptions(c, locale)),
        _ => {}
    }
}
