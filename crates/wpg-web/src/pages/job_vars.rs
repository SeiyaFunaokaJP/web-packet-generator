//! Template variable inputs of the job form: one input per declared variable,
//! typed by `variables[].type` / `choices` / `min` / `max` / `required`.

use std::collections::BTreeMap;

use leptos::prelude::*;
use wpg_template::{check_value, VarType, Variable};

use crate::api;
use crate::components::{INPUT, INPUT_INVALID, LABEL};
use crate::i18n::{tr, use_i18n};

/// Values entered for the variables of one stream (`name` → text; empty = default).
pub(crate) type VarValues = BTreeMap<String, String>;

/// Why the value of `v` cannot be sent: `(Fluent key, arguments)`.
pub(crate) fn var_error(
    v: &Variable,
    value: &str,
) -> Option<(&'static str, Vec<(&'static str, String)>)> {
    if value.trim().is_empty() {
        let has_default = v.default.is_some() && !v.required;
        return (!has_default).then(|| ("ui-var-required", Vec::new()));
    }
    check_value(v, value.trim()).err().map(|e| {
        (
            e.key(),
            vec![
                ("name", v.name.clone()),
                ("value", value.trim().to_owned()),
                ("expected", e.expected()),
            ],
        )
    })
}

/// Whether every declared variable has an acceptable value (or a default).
pub(crate) fn vars_ok(defs: &[Variable], values: &VarValues) -> bool {
    defs.iter()
        .all(|v| var_error(v, values.get(&v.name).map_or("", String::as_str)).is_none())
}

/// The values to send: trimmed, without the empty ones.
pub(crate) fn vars_to_send(values: &VarValues) -> VarValues {
    values
        .iter()
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(k, v)| (k.clone(), v.trim().to_owned()))
        .collect()
}

/// The declared variables in the JSON of a template (`TemplateDetail.template`).
pub(crate) fn declared(template: &serde_json::Value) -> Vec<Variable> {
    template
        .get("variables")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

/// Inputs for the variables of the template `template` (an id; empty = none).
/// Loads the declarations into `defs`, edits `values`.
#[component]
pub(crate) fn VarFields(
    /// Unique prefix of the input ids.
    id_prefix: String,
    template: RwSignal<String>,
    defs: RwSignal<Vec<Variable>>,
    values: RwSignal<VarValues>,
    /// Show "required" errors (after the first submit).
    submitted: RwSignal<bool>,
) -> impl IntoView {
    let detail = LocalResource::new(move || {
        let id = template.get();
        async move {
            if id.trim().is_empty() {
                None
            } else {
                api::template(&id).await.ok()
            }
        }
    });
    Effect::new(move |_| {
        let list = detail.get().flatten().map(|d| declared(&d.template));
        defs.set(list.unwrap_or_default());
    });
    let id_prefix = StoredValue::new(id_prefix);

    view! {
        <Show when=move || defs.with(|d| !d.is_empty())>
            <fieldset class="mt-2 grid gap-3 rounded-md border border-slate-200 p-3 sm:grid-cols-2 dark:border-slate-800">
                <legend class="px-1 text-xs font-semibold text-slate-600 dark:text-slate-400">
                    {tr("ui-jobs-vars")}
                </legend>
                <For
                    each=move || defs.get()
                    key=|v| v.name.clone()
                    children=move |v: Variable| {
                        let id = format!("{}-{}", id_prefix.get_value(), v.name);
                        view! { <VarInput id=id var=v values=values submitted=submitted /> }
                    }
                />
            </fieldset>
        </Show>
    }
}

#[component]
fn VarInput(
    id: String,
    var: Variable,
    values: RwSignal<VarValues>,
    submitted: RwSignal<bool>,
) -> impl IntoView {
    let i18n = use_i18n();
    let var = StoredValue::new(var);
    let name = var.with_value(|v| v.name.clone());
    let value = {
        let name = name.clone();
        move || values.with(|m| m.get(&name).cloned().unwrap_or_default())
    };
    let set = {
        let name = name.clone();
        move |s: String| {
            values.update(|m| {
                m.insert(name.clone(), s);
            });
        }
    };
    // Type errors show at once; a missing required value after a submit.
    let error = {
        let value = value.clone();
        move || {
            let text = value();
            let err = var.with_value(|v| var_error(v, &text))?;
            (!text.trim().is_empty() || submitted.get()).then_some(err)
        }
    };
    let invalid = {
        let error = error.clone();
        move || error().is_some()
    };
    let required = var.with_value(|v| v.required || v.default.is_none());
    let default = var.with_value(|v| v.default.clone().filter(|_| !v.required));
    let kind = var.with_value(|v| v.kind);
    let choices = var.with_value(|v| {
        if v.choices.is_empty() && kind == Some(VarType::Bool) {
            vec!["true".to_owned(), "false".to_owned()]
        } else {
            v.choices.clone()
        }
    });
    let description = var.with_value(|v| v.description.clone());
    let class = {
        let invalid = invalid.clone();
        move || if invalid() { INPUT_INVALID } else { INPUT }
    };

    let input = if choices.is_empty() {
        let numeric = kind.is_some_and(VarType::is_numeric);
        let (min, max) = var.with_value(|v| (v.min, v.max));
        let value = value.clone();
        let invalid = invalid.clone();
        view! {
            <input
                id=id.clone()
                class=class
                type=if numeric { "number" } else { "text" }
                inputmode=numeric.then_some("numeric")
                min=min.map(|m| m.to_string())
                max=max.map(|m| m.to_string())
                spellcheck="false"
                autocomplete="off"
                required=required
                aria-invalid=move || invalid().to_string()
                placeholder=default.clone().unwrap_or_default()
                prop:value=value
                on:input=move |ev| set(event_target_value(&ev))
            />
        }
        .into_any()
    } else {
        let current = value.clone();
        let empty_label = default.clone().map_or_else(
            || i18n.t("ui-var-choose"),
            |d| i18n.ta("ui-var-default", &[("value", d)]),
        );
        view! {
            <select
                id=id.clone()
                class=class
                required=required
                on:change=move |ev| set(event_target_value(&ev))
            >
                <option value="" selected={
                    let current = current.clone();
                    move || current().is_empty()
                }>{empty_label}</option>
                {choices
                    .into_iter()
                    .map(|c| {
                        let current = current.clone();
                        let (c2, label) = (c.clone(), c.clone());
                        view! {
                            <option value=c selected=move || current() == c2>{label}</option>
                        }
                    })
                    .collect_view()}
            </select>
        }
        .into_any()
    };

    view! {
        <div>
            <label for=id class=LABEL>
                <span class="font-mono">{name}</span>
                {required.then(|| view! { <span class="text-rose-600 dark:text-rose-400" aria-hidden="true">" *"</span> })}
            </label>
            {input}
            {move || match error() {
                Some((key, args)) => view! {
                    <p class="mt-1 text-xs text-rose-600 dark:text-rose-400">{i18n.ta(key, &args)}</p>
                }
                .into_any(),
                None => description
                    .clone()
                    .map(|d| view! { <p class="mt-1 text-xs text-slate-500">{d}</p> })
                    .into_any(),
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn var(j: serde_json::Value) -> Variable {
        serde_json::from_value(j).unwrap()
    }

    #[test]
    fn errors() {
        let vlan = var(
            serde_json::json!({ "name": "vlan", "type": "uint", "min": 1, "max": 4094, "default": 100 }),
        );
        assert_eq!(var_error(&vlan, ""), None, "the default is used");
        assert_eq!(var_error(&vlan, "4094"), None);
        let (key, args) = var_error(&vlan, "4095").unwrap();
        assert_eq!(key, "lint-var-out-of-range");
        assert!(args.contains(&("expected", "1..4094".to_owned())));

        let ip = var(serde_json::json!({ "name": "target", "type": "ipv4", "required": true }));
        assert_eq!(var_error(&ip, " ").unwrap().0, "ui-var-required");
        assert_eq!(var_error(&ip, "10.0.0.1"), None);
        assert_eq!(var_error(&ip, "10.0.0").unwrap().0, "lint-var-bad-type");

        let nodefault = var(serde_json::json!({ "name": "n" }));
        assert_eq!(var_error(&nodefault, "").unwrap().0, "ui-var-required");

        let defs = vec![vlan, ip];
        let mut values = VarValues::new();
        assert!(!vars_ok(&defs, &values));
        values.insert("target".into(), " 10.0.0.1 ".into());
        values.insert("vlan".into(), String::new());
        assert!(vars_ok(&defs, &values));
        let sent = vars_to_send(&values);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent["target"], "10.0.0.1");
    }

    #[test]
    fn declared_from_json() {
        let j = serde_json::json!({
            "variables": [{ "name": "v", "type": "mac", "default": "02:00:00:00:00:01" }]
        });
        let d = declared(&j);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].kind, Some(VarType::Mac));
        assert!(declared(&serde_json::json!({})).is_empty());
    }
}
