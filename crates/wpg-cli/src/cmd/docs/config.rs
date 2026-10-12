//! Configuration reference (`reference/config.md`): every `wpg.toml` key from the
//! JSON Schema of [`Config`], its default from [`Config::default`], the matching
//! `WPG_*` environment variable, a description from Fluent
//! (`docgen-config-<section>-<key>`, required for every key by a test) and the
//! commented example of `wpg config init`.

use std::fmt::Write as _;

use serde_json::{Map, Value};
use wpg_i18n::Locale;
use wpg_server::Config;

use super::md::{code, finish, heading, para, table, tx};
use super::schema::{ref_name, sorted};
use super::{src_path, Output, GENERATED};

/// Keys whose built-in default depends on the machine (described instead of shown).
const MACHINE_DEFAULTS: &[&str] = &["data_dir"];

/// One key of a section.
#[derive(Debug)]
struct Key {
    path: Vec<String>,
    ty: String,
    default: String,
}

/// A table (`[server]`, `[server.tls]`; the root is the empty path).
#[derive(Debug, Default)]
struct Section {
    path: Vec<String>,
    keys: Vec<Key>,
}

/// Resolves a `$ref` against the schema's `$defs`.
fn resolve<'a>(root: &'a Value, s: &'a Value) -> &'a Value {
    match s.get("$ref").and_then(Value::as_str) {
        Some(r) => root
            .get("$defs")
            .and_then(|d| d.get(ref_name(r)))
            .unwrap_or(s),
        None => s,
    }
}

/// The non-null alternative of `T | null` (`anyOf` or a type list).
fn non_null<'a>(root: &'a Value, s: &'a Value) -> &'a Value {
    if let Some(a) = s.get("anyOf").and_then(Value::as_array) {
        if let Some(x) = a
            .iter()
            .find(|x| x.get("type").and_then(Value::as_str) != Some("null"))
        {
            return resolve(root, x);
        }
    }
    resolve(root, s)
}

fn is_table(s: &Value) -> bool {
    s.get("properties").is_some()
}

/// TOML-ish type notation: `string`, `integer`, `boolean`, `string[]`,
/// `"pretty" | "json"` (no `null`: an absent key is "unset").
fn toml_type(root: &Value, s: &Value) -> String {
    let s = non_null(root, s);
    if let Some(vs) = s.get("oneOf").and_then(Value::as_array) {
        return vs
            .iter()
            .filter_map(|v| v.get("const").or_else(|| v.get("enum")?.get(0)))
            .map(|c| code(&c.to_string()))
            .collect::<Vec<_>>()
            .join(" | ");
    }
    if let Some(e) = s.get("enum").and_then(Value::as_array) {
        return e
            .iter()
            .map(|c| code(&c.to_string()))
            .collect::<Vec<_>>()
            .join(" | ");
    }
    let t = match s.get("type") {
        Some(Value::String(t)) => t.clone(),
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(Value::as_str)
            .find(|t| *t != "null")
            .unwrap_or("any")
            .to_owned(),
        _ => "any".to_owned(),
    };
    if t == "array" {
        let inner = s.get("items").map_or_else(
            || "any".to_owned(),
            |i| {
                let i = non_null(root, i);
                i.get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("any")
                    .to_owned()
            },
        );
        return code(&format!("{inner}[]"));
    }
    code(&t)
}

fn default_text(locale: Locale, defaults: &Value, path: &[String]) -> String {
    let dotted = path.join(".");
    if MACHINE_DEFAULTS.contains(&dotted.as_str()) {
        return tx(locale, "docgen-config-default-machine");
    }
    let mut v = defaults;
    for p in path {
        match v.get(p) {
            Some(x) => v = x,
            None => return String::new(),
        }
    }
    match v {
        Value::Null => String::new(),
        other => code(&other.to_string()),
    }
}

fn collect(
    root: &Value,
    s: &Value,
    path: &[String],
    defaults: &Value,
    locale: Locale,
    out: &mut Vec<Section>,
) {
    let mut section = Section {
        path: path.to_vec(),
        keys: Vec::new(),
    };
    let mut tables = Vec::new();
    let empty = Map::new();
    let props = s
        .get("properties")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    for (k, v) in sorted(props) {
        let mut p = path.to_vec();
        p.push(k.clone());
        let inner = non_null(root, v);
        if is_table(inner) {
            tables.push((p, inner));
        } else {
            section.keys.push(Key {
                ty: toml_type(root, v),
                default: default_text(locale, defaults, &p),
                path: p,
            });
        }
    }
    out.push(section);
    for (p, inner) in tables {
        collect(root, inner, &p, defaults, locale, out);
    }
}

/// `WPG_SERVER__TLS__CERT`, `WPG_DATA_DIR`.
fn env_name(path: &[String]) -> String {
    format!(
        "WPG_{}",
        path.iter()
            .map(|p| p.to_ascii_uppercase())
            .collect::<Vec<_>>()
            .join("__")
    )
}

/// Fluent key of the description of a key (`docgen-config-server-self-signed-tls`).
pub(crate) fn desc_key(path: &[String]) -> String {
    format!("docgen-config-{}", path.join("-").replace('_', "-"))
}

/// `reference/config.md` of `locale`.
pub(crate) fn output(locale: Locale) -> Output {
    let schema = Config::json_schema();
    let defaults = serde_json::to_value(Config::default()).unwrap_or(Value::Null);
    let mut sections = Vec::new();
    collect(&schema, &schema, &[], &defaults, locale, &mut sections);

    let mut s = format!("{GENERATED}\n\n");
    heading(&mut s, 1, &tx(locale, "docgen-config-title"));
    para(&mut s, &tx(locale, "docgen-config-intro"));
    heading(&mut s, 2, &tx(locale, "docgen-config-keys"));
    for sec in &sections {
        if sec.keys.is_empty() {
            continue;
        }
        let title = if sec.path.is_empty() {
            tx(locale, "docgen-config-top-level")
        } else {
            code(&format!("[{}]", sec.path.join(".")))
        };
        heading(&mut s, 3, &title);
        if !sec.path.is_empty() {
            para(&mut s, &tx(locale, &desc_key(&sec.path)));
        }
        let rows: Vec<Vec<String>> = sec
            .keys
            .iter()
            .map(|k| {
                vec![
                    code(k.path.last().map_or("", String::as_str)),
                    k.ty.clone(),
                    k.default.clone(),
                    code(&env_name(&k.path)),
                    tx(locale, &desc_key(&k.path)),
                ]
            })
            .collect();
        table(
            &mut s,
            &[
                tx(locale, "docgen-col-key"),
                tx(locale, "docgen-col-type"),
                tx(locale, "docgen-col-default"),
                tx(locale, "docgen-col-env"),
                tx(locale, "docgen-col-description"),
            ],
            &rows,
        );
    }
    heading(&mut s, 2, &tx(locale, "docgen-config-example"));
    para(&mut s, &tx(locale, "docgen-config-example-intro"));
    let _ = write!(s, "```toml\n{}\n```\n", Config::example_toml().trim_end());
    Output::whole(src_path(locale, "reference/config.md"), finish(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_names() {
        assert_eq!(env_name(&["data_dir".into()]), "WPG_DATA_DIR");
        assert_eq!(
            env_name(&["server".into(), "tls".into(), "cert".into()]),
            "WPG_SERVER__TLS__CERT"
        );
        assert_eq!(
            desc_key(&["server".into(), "self_signed_tls".into()]),
            "docgen-config-server-self-signed-tls"
        );
    }
}
