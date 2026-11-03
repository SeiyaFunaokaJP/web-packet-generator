//! Run-time field overrides (`wpg send --set` / `--gen`) and variable files
//! (`--vars-file`), applied to a [`Template`] before [`crate::compile()`].
//!
//! * `--set LAYER.FIELD=VALUE` writes `VALUE` (human notation, as in `fields:`) and
//!   drops a generator of that field, so the field becomes Manual;
//! * `--gen LAYER.FIELD=SPEC` gives the field a generator (see
//!   [`parse_generator_shorthand`]) and drops its fixed value.
//!
//! `LAYER` is a 0-based layer index, a protocol id (its first layer) or the n-th
//! layer of a protocol (`ipv4[1]`, counting from 0), like `stream.order`. Template
//! fields are flat (TLV and list fields take their raw octets as a value here), so
//! `FIELD` is a single field name.

use wpg_core::{raw_protocol, Generator, ProtocolDef, Resolve, RAW_PROTOCOL_ID};

use crate::compile::VarOverrides;
use crate::input::FieldInput;
use crate::lint::LintIssue;
use crate::{GeneratorSpec, Template, TemplateError};

/// What an override does to its field.
#[derive(Debug, Clone, PartialEq)]
pub enum OverrideAction {
    /// A fixed value in human notation (`--set`).
    Value(String),
    /// A generator (`--gen`).
    Generator(GeneratorSpec),
}

/// One `--set` / `--gen` argument.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldOverride {
    /// The argument as given (used in error messages).
    pub source: String,
    /// Layer reference: index, protocol id or `proto[n]`.
    pub layer: String,
    /// Field name.
    pub field: String,
    /// The change.
    pub action: OverrideAction,
}

/// Splits `LAYER.FIELD=REST`; `None` if a part is missing.
fn split_target(arg: &str) -> Option<(&str, &str, &str)> {
    let (target, rest) = arg.split_once('=')?;
    let (layer, field) = target.trim().split_once('.')?;
    let (layer, field) = (layer.trim(), field.trim());
    (!layer.is_empty() && !field.is_empty()).then_some((layer, field, rest))
}

impl FieldOverride {
    /// Parses `--set LAYER.FIELD=VALUE`; `None` for a malformed argument.
    #[must_use]
    pub fn parse_set(arg: &str) -> Option<Self> {
        let (layer, field, value) = split_target(arg)?;
        Some(Self {
            source: arg.to_owned(),
            layer: layer.to_owned(),
            field: field.to_owned(),
            action: OverrideAction::Value(value.to_owned()),
        })
    }

    /// Parses `--gen LAYER.FIELD=SPEC`.
    ///
    /// # Errors
    /// `None` for a malformed argument, `Some(message)` for a bad generator.
    pub fn parse_gen(arg: &str) -> Result<Self, Option<String>> {
        let (layer, field, spec) = split_target(arg).ok_or(None)?;
        Ok(Self {
            source: arg.to_owned(),
            layer: layer.to_owned(),
            field: field.to_owned(),
            action: OverrideAction::Generator(parse_generator_shorthand(spec).map_err(Some)?),
        })
    }
}

/// Parses the generator shorthand of `--gen`:
///
/// | shorthand | generator |
/// |---|---|
/// | `range:A..B` / `range:A..B/STEP` | `range` from `A` to `B` (inclusive) |
/// | `list:a,b,c` | `list` |
/// | `random` / `random:MIN..MAX` | `random` |
/// | `counter` / `counter:START` / `counter:START/STEP` | `counter` |
/// | `expr:EXPR` | `expr` |
/// | anything else | the template / GUI shorthand: `a-b`, `a..b`, `1,5,9`, `10.0.0.0/24`, `00:11:22:*` |
///
/// Operands are checked against the field type when the template is compiled.
///
/// # Errors
/// A message for a malformed shorthand.
pub fn parse_generator_shorthand(s: &str) -> Result<GeneratorSpec, String> {
    let s = s.trim();
    let (kind, arg) = match s.split_once(':') {
        Some((k, a)) => (k.trim(), Some(a.trim())),
        None => (s, None),
    };
    let range = |a: &str| -> Result<(String, String), String> {
        let (lo, hi) = a
            .split_once("..")
            .filter(|(lo, hi)| !lo.trim().is_empty() && !hi.trim().is_empty())
            .ok_or_else(|| format!("`{a}` is not `START..END`"))?;
        Ok((lo.trim().to_owned(), hi.trim().to_owned()))
    };
    let generator = match (kind, arg) {
        ("range", Some(a)) => {
            let (bounds, step) = match a.rsplit_once('/') {
                Some((b, st)) => (
                    b,
                    st.trim()
                        .parse::<u64>()
                        .ok()
                        .filter(|v| *v > 0)
                        .ok_or_else(|| format!("step `{st}` is not a positive integer"))?,
                ),
                None => (a, 1),
            };
            let (start, end) = range(bounds)?;
            Generator::Range { start, end, step }
        }
        ("list", Some(a)) => {
            let values: Vec<String> = a.split(',').map(|v| v.trim().to_owned()).collect();
            if values.iter().any(String::is_empty) {
                return Err(format!("`{a}` has an empty list element"));
            }
            Generator::List { values }
        }
        ("random", None) => Generator::Random {
            min: None,
            max: None,
            seed: None,
        },
        ("random", Some(a)) => {
            let (min, max) = range(a)?;
            Generator::Random {
                min: Some(min),
                max: Some(max),
                seed: None,
            }
        }
        ("counter", arg) => {
            let (start, step) = match arg.map(|a| a.split_once('/').unwrap_or((a, ""))) {
                None => ("", ""),
                Some((s, st)) => (s.trim(), st.trim()),
            };
            Generator::Counter {
                start: if start.is_empty() {
                    0
                } else {
                    start
                        .parse()
                        .map_err(|_| format!("counter start `{start}` is not an integer"))?
                },
                step: if step.is_empty() {
                    1
                } else {
                    step.parse()
                        .map_err(|_| format!("counter step `{step}` is not an integer"))?
                },
                wrap: None,
            }
        }
        ("expr", Some(a)) if !a.is_empty() => Generator::Expr { expr: a.to_owned() },
        ("range" | "list" | "expr", _) => {
            return Err(format!("`{kind}` needs an argument after `{kind}:`"));
        }
        _ if s.is_empty() => return Err("empty generator".to_owned()),
        _ => return Ok(GeneratorSpec::Text(s.to_owned())),
    };
    Ok(GeneratorSpec::Full(generator))
}

/// Index of the layer `r` names: `N`, `proto` (first layer) or `proto[n]`.
fn find_layer(t: &Template, r: &str) -> Option<usize> {
    if let Ok(i) = r.parse::<usize>() {
        return (i < t.layers.len()).then_some(i);
    }
    let (proto, nth) = match r.strip_suffix(']').and_then(|s| s.rsplit_once('[')) {
        Some((p, n)) => (p, n.trim().parse::<usize>().ok()?),
        None => (r, 0),
    };
    t.layers
        .iter()
        .enumerate()
        .filter(|(_, l)| l.protocol == proto)
        .nth(nth)
        .map(|(i, _)| i)
}

/// Edit distance (for suggestions).
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != *cb)).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

/// The candidates close to `name` (edit distance or substring), best first; all
/// candidates when none is close.
#[must_use]
pub fn close_matches(name: &str, candidates: &[String]) -> Vec<String> {
    let lower = name.to_ascii_lowercase();
    let mut scored: Vec<(usize, &String)> = candidates
        .iter()
        .filter_map(|c| {
            let cl = c.to_ascii_lowercase();
            let d = distance(&lower, &cl);
            let close = d <= (lower.chars().count() / 3).max(2)
                || (lower.len() >= 2 && (cl.contains(&lower) || lower.contains(&cl)));
            close.then_some((d, c))
        })
        .collect();
    if scored.is_empty() {
        return candidates.to_vec();
    }
    scored.sort_by_key(|(d, _)| *d);
    scored.into_iter().map(|(_, c)| c.clone()).collect()
}

/// Applies `--set` / `--gen` overrides to `t` in order.
///
/// # Errors
/// [`TemplateError::Lint`] naming every override whose layer or field does not
/// exist: `lint-override-layer` lists the template's layers, `lint-override-field`
/// the closest field names (all of them when none is close).
pub fn apply_field_overrides(
    t: &mut Template,
    overrides: &[FieldOverride],
    resolve: &dyn Resolve,
) -> Result<(), TemplateError> {
    let raw = raw_protocol();
    let lookup = |id: &str| -> Option<&ProtocolDef> {
        resolve
            .protocol(id)
            .or_else(|| (id == RAW_PROTOCOL_ID).then_some(&raw))
    };
    let mut issues = Vec::new();
    for o in overrides {
        let Some(i) = find_layer(t, &o.layer) else {
            let mut known: Vec<String> = Vec::new();
            for (i, l) in t.layers.iter().enumerate() {
                let nth = t.layers[..i]
                    .iter()
                    .filter(|x| x.protocol == l.protocol)
                    .count();
                known.push(if nth == 0 {
                    l.protocol.clone()
                } else {
                    format!("{}[{nth}]", l.protocol)
                });
            }
            // the stack is short: list all of it, in order
            let known = known.join(", ");
            issues.push(
                LintIssue::error(
                    &o.source,
                    "lint-override-layer",
                    format!("no layer `{}` (layers: {known})", o.layer),
                )
                .arg("value", &o.layer)
                .arg("protocols", known),
            );
            continue;
        };
        let layer = &mut t.layers[i];
        // an unknown protocol is reported by the compiler
        if let Some(def) = lookup(&layer.protocol) {
            if def.field(&o.field).is_none() {
                let names: Vec<String> = def.fields.iter().map(|f| f.name.clone()).collect();
                let known = close_matches(&o.field, &names).join(", ");
                issues.push(
                    LintIssue::error(
                        &o.source,
                        "lint-override-field",
                        format!(
                            "protocol `{}` has no field `{}` (fields: {known})",
                            layer.protocol, o.field
                        ),
                    )
                    .arg("protocol", &layer.protocol)
                    .arg("field", &o.field)
                    .arg("fields", known),
                );
                continue;
            }
        }
        match &o.action {
            OverrideAction::Value(v) => {
                layer.generators.shift_remove(&o.field);
                layer
                    .fields
                    .insert(o.field.clone(), FieldInput::Str(v.clone()));
            }
            OverrideAction::Generator(g) => {
                layer.fields.shift_remove(&o.field);
                layer.generators.insert(o.field.clone(), g.clone());
            }
        }
    }
    if issues.is_empty() {
        Ok(())
    } else {
        Err(TemplateError::Lint(issues))
    }
}

/// Parses a variables file: a YAML (or JSON) map of variable names to scalar
/// values (`src_ip: 10.0.0.1`, `count: 5`, `flag: true` = 1).
///
/// # Errors
/// A message for a syntax error, a non-map document or a non-scalar value.
pub fn parse_vars_file(text: &str) -> Result<VarOverrides, String> {
    use serde_yaml::Value;
    let doc: Value = serde_yaml::from_str(text).map_err(|e| e.to_string())?;
    let map = match doc {
        Value::Null => return Ok(VarOverrides::new()),
        Value::Mapping(m) => m,
        _ => return Err("expected a map of variable names to values".to_owned()),
    };
    let mut out = VarOverrides::new();
    for (k, v) in map {
        let Some(name) = k.as_str().map(str::to_owned) else {
            return Err(format!("variable name {k:?} is not text"));
        };
        let value = match v {
            Value::String(s) => s,
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => u8::from(b).to_string(),
            _ => return Err(format!("`{name}`: expected a single value")),
        };
        out.insert(name, value);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TemplateLayer;
    use wpg_core::ProtocolLayer;

    fn catalog() -> Vec<ProtocolDef> {
        let mut ip = ProtocolDef::new("ipv4", "IPv4", ProtocolLayer::L3);
        for name in ["ttl", "src", "dst", "protocol"] {
            ip.fields.push(wpg_core::FieldDef::new(
                name,
                wpg_core::FieldType::UInt { bits: 8 },
            ));
        }
        vec![ip]
    }

    fn template() -> Template {
        let mut t = Template::new("t");
        for p in ["ethernet2", "ipv4", "ipv4"] {
            t.layers.push(TemplateLayer {
                protocol: p.into(),
                ..TemplateLayer::default()
            });
        }
        t.layers[1]
            .generators
            .insert("ttl".into(), GeneratorSpec::Text("1-9".into()));
        t
    }

    #[test]
    fn resolves_layers() {
        let t = template();
        assert_eq!(find_layer(&t, "ipv4"), Some(1));
        assert_eq!(find_layer(&t, "ipv4[0]"), Some(1));
        assert_eq!(find_layer(&t, "ipv4[1]"), Some(2));
        assert_eq!(find_layer(&t, "ipv4[2]"), None);
        assert_eq!(find_layer(&t, "2"), Some(2));
        assert_eq!(find_layer(&t, "3"), None);
        assert_eq!(find_layer(&t, "ipv6"), None);
        assert_eq!(find_layer(&t, "ipv4[x]"), None);
    }

    #[test]
    fn parses_targets() {
        let o = FieldOverride::parse_set("ipv4[1].ttl=a=b").unwrap();
        assert_eq!((o.layer.as_str(), o.field.as_str()), ("ipv4[1]", "ttl"));
        assert_eq!(o.action, OverrideAction::Value("a=b".into()));
        assert!(FieldOverride::parse_set("ipv4=1").is_none());
        assert!(FieldOverride::parse_set("ipv4.ttl").is_none());
        assert!(FieldOverride::parse_set(".ttl=1").is_none());
        assert!(matches!(FieldOverride::parse_gen("ttl=random"), Err(None)));
        assert!(matches!(
            FieldOverride::parse_gen("ipv4.ttl=range:5"),
            Err(Some(_))
        ));
    }

    #[test]
    fn applies_set_and_gen() {
        let mut t = template();
        let o = [
            FieldOverride::parse_set("ipv4.ttl=7").unwrap(),
            FieldOverride::parse_gen("ipv4[1].src=list:1,2").unwrap(),
        ];
        apply_field_overrides(&mut t, &o, &catalog()).unwrap();
        assert_eq!(t.layers[1].fields["ttl"], FieldInput::Str("7".into()));
        assert!(t.layers[1].generators.is_empty());
        assert_eq!(
            t.layers[2].generators["src"],
            GeneratorSpec::Full(Generator::List {
                values: vec!["1".into(), "2".into()]
            })
        );
    }

    #[test]
    fn reports_bad_layer_and_field_with_suggestions() {
        let mut t = template();
        let o = [
            FieldOverride::parse_set("ipv5.ttl=1").unwrap(),
            FieldOverride::parse_set("ipv4.tll=1").unwrap(),
        ];
        let Err(TemplateError::Lint(issues)) = apply_field_overrides(&mut t, &o, &catalog()) else {
            panic!("expected errors");
        };
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].key, "lint-override-layer");
        assert_eq!(issues[0].path, "ipv5.ttl=1");
        assert!(
            issues[0]
                .message
                .contains("(layers: ethernet2, ipv4, ipv4[1])"),
            "{}",
            issues[0].message
        );
        assert_eq!(issues[1].key, "lint-override-field");
        assert!(
            issues[1].message.contains("(fields: ttl"),
            "{}",
            issues[1].message
        );
    }

    #[test]
    fn suggests_close_names() {
        let c: Vec<String> = ["ttl", "src", "dst", "protocol"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert_eq!(close_matches("tll", &c)[0], "ttl");
        assert_eq!(close_matches("proto", &c), vec!["protocol"]);
        assert_eq!(close_matches("zzzzzzzz", &c), c);
    }

    #[test]
    fn parses_generator_shorthand() {
        use GeneratorSpec::{Full, Text};
        let p = |s| parse_generator_shorthand(s).unwrap();
        assert_eq!(
            p("range:1..100"),
            Full(Generator::Range {
                start: "1".into(),
                end: "100".into(),
                step: 1
            })
        );
        assert_eq!(
            p("range:10.0.0.1..10.0.0.9/2"),
            Full(Generator::Range {
                start: "10.0.0.1".into(),
                end: "10.0.0.9".into(),
                step: 2
            })
        );
        assert_eq!(
            p("list:a, b,c"),
            Full(Generator::List {
                values: vec!["a".into(), "b".into(), "c".into()]
            })
        );
        assert_eq!(
            p("random"),
            Full(Generator::Random {
                min: None,
                max: None,
                seed: None
            })
        );
        assert_eq!(
            p("random:1024..65535"),
            Full(Generator::Random {
                min: Some("1024".into()),
                max: Some("65535".into()),
                seed: None
            })
        );
        assert_eq!(
            p("counter:5/-1"),
            Full(Generator::Counter {
                start: 5,
                step: -1,
                wrap: None
            })
        );
        assert_eq!(
            p("counter"),
            Full(Generator::Counter {
                start: 0,
                step: 1,
                wrap: None
            })
        );
        assert_eq!(
            p("expr:$0.src + 1"),
            Full(Generator::Expr {
                expr: "$0.src + 1".into()
            })
        );
        // GUI shorthand passes through, including MAC / IPv6 colons
        assert_eq!(p("10.0.0.0/24"), Text("10.0.0.0/24".into()));
        assert_eq!(p("00:11:22:*"), Text("00:11:22:*".into()));
        assert_eq!(p("fe80::1-fe80::9"), Text("fe80::1-fe80::9".into()));
        for bad in [
            "",
            "range:1",
            "range:1..5/0",
            "list:",
            "list:a,,b",
            "random:5",
            "counter:x",
            "expr:",
        ] {
            assert!(parse_generator_shorthand(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn parses_vars_files() {
        let v = parse_vars_file("a: 1\nb: x\nc: true\n").unwrap();
        assert_eq!(v["a"], "1");
        assert_eq!(v["b"], "x");
        assert_eq!(v["c"], "1");
        let v = parse_vars_file(r#"{"src_ip": "10.0.0.1"}"#).unwrap();
        assert_eq!(v["src_ip"], "10.0.0.1");
        assert!(parse_vars_file("").unwrap().is_empty());
        assert!(parse_vars_file("[1, 2]").is_err());
        assert!(parse_vars_file("a: [1]").is_err());
        assert!(parse_vars_file("a: [").is_err());
    }
}
