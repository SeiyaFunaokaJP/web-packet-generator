//! API reference (`reference/api.md`) and `docs/schema/openapi.json`, both from
//! the utoipa document of the server ([`wpg_server::api::openapi`]).

use std::fmt::Write as _;

use serde_json::{Map, Value};
use wpg_i18n::{try_t, Locale};

use super::md::{code, doc_text, finish, heading, para, table, tx};
use super::schema::{description, ref_name, render_named, sorted, type_of};
use super::{src_path, Output, GENERATED};

/// HTTP methods in display order.
const METHODS: [&str; 7] = ["get", "post", "put", "patch", "delete", "head", "options"];

/// The OpenAPI document with every object's keys sorted, so the output does not
/// depend on whether `serde_json/preserve_order` happens to be enabled.
fn document() -> Value {
    fn sort(v: Value) -> Value {
        match v {
            Value::Object(m) => {
                let mut pairs: Vec<(String, Value)> = m.into_iter().collect();
                pairs.sort_by(|a, b| a.0.cmp(&b.0));
                Value::Object(pairs.into_iter().map(|(k, v)| (k, sort(v))).collect())
            }
            Value::Array(a) => Value::Array(a.into_iter().map(sort).collect()),
            other => other,
        }
    }
    sort(serde_json::to_value(wpg_server::api::openapi()).unwrap_or(Value::Null))
}

/// `docs/schema/openapi.json` (the document served at `/api/openapi.json`).
pub(crate) fn openapi_json() -> Output {
    let mut text = serde_json::to_string_pretty(&document()).unwrap_or_default();
    text.push('\n');
    Output::whole("docs/schema/openapi.json", text)
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Body schemas of a request body / response: `application/json` → type.
fn content_types(c: Option<&Value>) -> String {
    let Some(m) = c.and_then(Value::as_object) else {
        return String::new();
    };
    sorted(m)
        .into_iter()
        .map(|(mime, v)| match v.get("schema") {
            Some(s) => format!("{} {}", code(mime), type_of(s)),
            None => code(mime),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

struct Op<'a> {
    method: &'static str,
    path: &'a str,
    op: &'a Map<String, Value>,
}

impl Op<'_> {
    fn title(&self) -> String {
        format!("{} {}", self.method.to_ascii_uppercase(), self.path)
    }

    fn anchor(&self) -> String {
        let mut a = format!("op-{}", self.method);
        for c in self.path.chars() {
            if c.is_ascii_alphanumeric() {
                a.push(c.to_ascii_lowercase());
            } else if !a.ends_with('-') {
                a.push('-');
            }
        }
        a.trim_end_matches('-').to_owned()
    }

    fn summary(&self) -> String {
        doc_text(&one_line(
            self.op
                .get("summary")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        ))
    }
}

fn operation(out: &mut String, locale: Locale, op: &Op<'_>) {
    let _ = writeln!(out, "<a id=\"{}\"></a>\n", op.anchor());
    heading(out, 4, &code(&op.title()));
    para(out, &op.summary());
    if let Some(d) = op.op.get("description").and_then(Value::as_str) {
        para(out, &doc_text(d));
    }
    if let Some(params) = op.op.get("parameters").and_then(Value::as_array) {
        let rows: Vec<Vec<String>> = params
            .iter()
            .map(|p| {
                vec![
                    code(p.get("name").and_then(Value::as_str).unwrap_or_default()),
                    p.get("in")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    p.get("schema").map(type_of).unwrap_or_default(),
                    if p.get("required").and_then(Value::as_bool) == Some(true) {
                        tx(locale, "docgen-yes")
                    } else {
                        String::new()
                    },
                    description(p),
                ]
            })
            .collect();
        para(out, &format!("**{}**", tx(locale, "docgen-api-params")));
        table(
            out,
            &[
                tx(locale, "docgen-col-name"),
                tx(locale, "docgen-col-in"),
                tx(locale, "docgen-col-type"),
                tx(locale, "docgen-col-required"),
                tx(locale, "docgen-col-description"),
            ],
            &rows,
        );
    }
    if let Some(body) = op.op.get("requestBody") {
        let mut line = format!(
            "**{}**: {}",
            tx(locale, "docgen-api-body"),
            content_types(body.get("content"))
        );
        let d = description(body);
        if !d.is_empty() {
            let _ = write!(line, " — {}", one_line(&d));
        }
        para(out, &line);
    }
    if let Some(resp) = op.op.get("responses").and_then(Value::as_object) {
        let rows: Vec<Vec<String>> = sorted(resp)
            .into_iter()
            .map(|(status, r)| {
                vec![
                    code(status),
                    description(r),
                    content_types(r.get("content")),
                ]
            })
            .collect();
        para(out, &format!("**{}**", tx(locale, "docgen-api-responses")));
        table(
            out,
            &[
                tx(locale, "docgen-col-status"),
                tx(locale, "docgen-col-description"),
                tx(locale, "docgen-col-body"),
            ],
            &rows,
        );
    }
}

/// `reference/api.md` of `locale`.
pub(crate) fn output(locale: Locale) -> Output {
    let doc = document();
    let mut s = format!("{GENERATED}\n\n");
    heading(&mut s, 1, &tx(locale, "docgen-api-title"));
    para(&mut s, &tx(locale, "docgen-api-intro"));

    let mut ops: Vec<Op<'_>> = Vec::new();
    if let Some(paths) = doc.get("paths").and_then(Value::as_object) {
        for (path, item) in sorted(paths) {
            for method in METHODS {
                if let Some(op) = item.get(method).and_then(Value::as_object) {
                    ops.push(Op { method, path, op });
                }
            }
        }
    }
    let tag_of = |op: &Op<'_>| -> String {
        op.op
            .get("tags")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(Value::as_str)
            .unwrap_or("default")
            .to_owned()
    };
    // Tags in the order the document declares them, then any undeclared ones.
    let mut tags: Vec<(String, String)> = doc
        .get("tags")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|t| Some((t.get("name")?.as_str()?.to_owned(), description(t))))
                .collect()
        })
        .unwrap_or_default();
    for op in &ops {
        let t = tag_of(op);
        if !tags.iter().any(|(n, _)| *n == t) {
            tags.push((t, String::new()));
        }
    }

    let rank = |op: &Op<'_>| tags.iter().position(|(n, _)| *n == tag_of(op));
    ops.sort_by_key(|op| rank(op));

    heading(&mut s, 2, &tx(locale, "docgen-api-endpoints"));
    let rows: Vec<Vec<String>> = ops
        .iter()
        .map(|op| {
            vec![
                tag_of(op),
                format!("[{}](#{})", code(&op.title()), op.anchor()),
                op.summary(),
            ]
        })
        .collect();
    table(
        &mut s,
        &[
            tx(locale, "docgen-col-tag"),
            tx(locale, "docgen-col-endpoint"),
            tx(locale, "docgen-col-summary"),
        ],
        &rows,
    );
    for (tag, fallback) in &tags {
        let in_tag: Vec<&Op<'_>> = ops.iter().filter(|op| tag_of(op) == *tag).collect();
        if in_tag.is_empty() {
            continue;
        }
        heading(&mut s, 3, &code(tag));
        let d = try_t(locale, &format!("docgen-api-tag-{tag}")).unwrap_or_else(|| fallback.clone());
        para(&mut s, &d);
        for op in in_tag {
            operation(&mut s, locale, op);
        }
    }

    heading(&mut s, 2, &tx(locale, "docgen-api-schemas"));
    para(&mut s, &tx(locale, "docgen-api-schemas-intro"));
    if let Some(schemas) = doc
        .pointer("/components/schemas")
        .and_then(Value::as_object)
    {
        for (name, schema) in sorted(schemas) {
            render_named(&mut s, locale, ref_name(name), schema, 3);
        }
    }
    Output::whole(src_path(locale, "reference/api.md"), finish(s))
}
