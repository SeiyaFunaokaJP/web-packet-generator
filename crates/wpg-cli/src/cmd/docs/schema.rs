//! JSON Schema rendering shared by the API and template references, and the key
//! reference at the end of `reference/template-schema.md` (from
//! [`Template::json_schema`]).

use std::fmt::Write as _;

use serde_json::{Map, Value};
use wpg_i18n::Locale;
use wpg_template::Template;

use super::md::{code, doc_text, heading, para, table, tx};
use super::{src_path, Output};

/// Region of the generated key reference in `template-schema.md`.
const REGION: &str = "template-schema-keys";

/// Anchor of a named schema (`schema-<lowercase name>`).
pub(crate) fn anchor(name: &str) -> String {
    format!("schema-{}", name.to_ascii_lowercase())
}

/// Last segment of a `$ref` (`#/$defs/Meta` → `Meta`).
pub(crate) fn ref_name(r: &str) -> &str {
    r.rsplit('/').next().unwrap_or(r)
}

/// Properties of a map, sorted by key (independent of `serde_json/preserve_order`).
pub(crate) fn sorted(m: &Map<String, Value>) -> Vec<(&String, &Value)> {
    let mut v: Vec<_> = m.iter().collect();
    v.sort_by(|a, b| a.0.cmp(b.0));
    v
}

/// Compact type notation: `string`, `integer`, `T[]`, `map<string, T>`,
/// `"a" | "b"`, links for named schemas.
pub(crate) fn type_of(s: &Value) -> String {
    let Some(o) = s.as_object() else {
        return code("any");
    };
    if let Some(r) = o.get("$ref").and_then(Value::as_str) {
        let n = ref_name(r);
        return format!("[{}](#{})", code(n), anchor(n));
    }
    if let Some(c) = o.get("const") {
        return code(&c.to_string());
    }
    if let Some(e) = o.get("enum").and_then(Value::as_array) {
        return e
            .iter()
            .map(|v| code(&v.to_string()))
            .collect::<Vec<_>>()
            .join(" | ");
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(a) = o.get(key).and_then(Value::as_array) {
            return a.iter().map(type_of).collect::<Vec<_>>().join(" | ");
        }
    }
    if let Some(a) = o.get("allOf").and_then(Value::as_array) {
        if a.len() == 1 {
            return type_of(&a[0]);
        }
        return code("object");
    }
    let types: Vec<String> = match o.get("type") {
        Some(Value::String(t)) => vec![t.clone()],
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        _ => return code("any"),
    };
    types
        .iter()
        .map(|t| match t.as_str() {
            "array" => match o.get("items") {
                Some(items) => array_of(&type_of(items)),
                None => code("any[]"),
            },
            "object" => match o.get("additionalProperties") {
                Some(ap @ Value::Object(_)) => {
                    format!("{}{}{}", code("map<string, "), type_of(ap), code(">"))
                }
                _ => code("object"),
            },
            other => code(other),
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn array_of(inner: &str) -> String {
    // `T`[] reads as an array of T whether T is code or a link.
    if inner.contains(" | ") {
        format!("({inner}){}", code("[]"))
    } else {
        format!("{inner}{}", code("[]"))
    }
}

/// Description of a schema (one paragraph per blank line of the doc comment).
pub(crate) fn description(s: &Value) -> String {
    doc_text(
        s.get("description")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )
}

fn properties_table(out: &mut String, locale: Locale, o: &Map<String, Value>) {
    let Some(props) = o.get("properties").and_then(Value::as_object) else {
        return;
    };
    let required: Vec<&str> = o
        .get("required")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let rows: Vec<Vec<String>> = sorted(props)
        .into_iter()
        .map(|(k, v)| {
            vec![
                code(k),
                type_of(v),
                if required.contains(&k.as_str()) {
                    tx(locale, "docgen-yes")
                } else {
                    String::new()
                },
                description(v),
            ]
        })
        .collect();
    table(
        out,
        &[
            tx(locale, "docgen-col-key"),
            tx(locale, "docgen-col-type"),
            tx(locale, "docgen-col-required"),
            tx(locale, "docgen-col-description"),
        ],
        &rows,
    );
}

/// The discriminating value of a tagged-union variant (`type: "stats"`).
fn tag_of(o: &Map<String, Value>) -> Option<String> {
    let props = o.get("properties")?.as_object()?;
    sorted(props).into_iter().find_map(|(k, v)| {
        let one = v
            .get("enum")
            .and_then(Value::as_array)
            .filter(|a| a.len() == 1)
            .and_then(|a| a.first())
            .or_else(|| v.get("const"))?;
        Some(format!("{k}: {one}"))
    })
}

/// Merges the parts of an `allOf` into one object schema.
fn merged(o: &Map<String, Value>) -> Map<String, Value> {
    let mut m = o.clone();
    if let Some(parts) = o.get("allOf").and_then(Value::as_array) {
        let mut props = Map::new();
        let mut required = Vec::new();
        for p in parts {
            if let Some(pp) = p.get("properties").and_then(Value::as_object) {
                props.extend(pp.clone());
            }
            if let Some(r) = p.get("required").and_then(Value::as_array) {
                required.extend(r.iter().cloned());
            }
        }
        if !props.is_empty() {
            m.insert("properties".into(), Value::Object(props));
            m.insert("required".into(), Value::Array(required));
        }
    }
    m
}

/// A named schema: anchor, heading, description, then its keys / variants / values.
pub(crate) fn render_named(out: &mut String, locale: Locale, name: &str, s: &Value, level: usize) {
    let _ = writeln!(out, "<a id=\"{}\"></a>\n", anchor(name));
    heading(out, level, &code(name));
    render_body(out, locale, s, level + 1);
}

fn render_body(out: &mut String, locale: Locale, s: &Value, level: usize) {
    let Some(o) = s.as_object() else { return };
    let o = merged(o);
    para(out, &description(s));
    if o.contains_key("properties") {
        properties_table(out, locale, &o);
        return;
    }
    let variants = o
        .get("oneOf")
        .or_else(|| o.get("anyOf"))
        .and_then(Value::as_array);
    if let Some(vs) = variants {
        let simple = vs.iter().all(|v| {
            let m = v.as_object().map(merged).unwrap_or_default();
            !m.contains_key("properties")
        });
        if simple {
            let rows: Vec<Vec<String>> = vs
                .iter()
                .map(|v| vec![type_of(v), description(v)])
                .collect();
            table(
                out,
                &[
                    tx(locale, "docgen-col-value"),
                    tx(locale, "docgen-col-description"),
                ],
                &rows,
            );
            return;
        }
        for (i, v) in vs.iter().enumerate() {
            let Some(vo) = v.as_object() else { continue };
            let m = merged(vo);
            let title = tag_of(&m).map_or_else(
                || {
                    crate::cmd::docs::md::txa(
                        locale,
                        "docgen-variant",
                        &[("n", (i + 1).to_string())],
                    )
                },
                |t| code(&t),
            );
            heading(out, level, &title);
            render_body(out, locale, &Value::Object(m.clone()), level + 1);
            if !m.contains_key("properties") {
                para(out, &type_of(v));
            }
        }
        return;
    }
    if o.contains_key("enum") || o.contains_key("type") || o.contains_key("$ref") {
        para(
            out,
            &format!("{}: {}", tx(locale, "docgen-col-type"), type_of(s)),
        );
    }
}

/// The key reference region of `reference/template-schema.md`.
pub(crate) fn output(locale: Locale) -> Output {
    let schema = Template::json_schema();
    let mut s = String::new();
    heading(&mut s, 2, &tx(locale, "docgen-schema-heading"));
    para(&mut s, &tx(locale, "docgen-schema-intro"));
    let title = schema
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("Template")
        .to_owned();
    render_named(&mut s, locale, &title, &schema, 3);
    if let Some(defs) = schema.get("$defs").and_then(Value::as_object) {
        for (name, def) in sorted(defs) {
            render_named(&mut s, locale, name, def, 3);
        }
    }
    let body = format!("{}\n", s.trim_end());
    Output::region(
        src_path(locale, "reference/template-schema.md"),
        REGION,
        body,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn types() {
        assert_eq!(type_of(&json!({"type": "string"})), "`string`");
        assert_eq!(
            type_of(&json!({"type": ["string", "null"]})),
            "`string` | `null`"
        );
        assert_eq!(
            type_of(&json!({"type": "array", "items": {"$ref": "#/$defs/Meta"}})),
            "[`Meta`](#schema-meta)`[]`"
        );
        assert_eq!(type_of(&json!({"enum": ["a", "b"]})), "`\"a\"` | `\"b\"`");
    }
}
