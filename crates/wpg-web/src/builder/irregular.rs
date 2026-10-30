//! "Allow irregular packets" mode of the builder.
//!
//! Most users want packets that follow the protocol definitions, so the builder is
//! strict by default: computed fields (lengths, checksums, next-protocol selectors,
//! padding) stay locked to their Auto value, and anything that makes the packet
//! out of spec is an error that blocks Save / New job. Turning the mode on unlocks
//! those fields and turns the same findings into warnings, for deliberately
//! malformed test traffic.
//!
//! [`issues`] collects the findings from the template and its last in-browser
//! build; it is pure and unit tested.

use std::collections::BTreeMap;

use wpg_core::{DiagLevel, FieldDef, FieldType};
use wpg_template::Template;

use super::preview::Preview;
use super::proto;

/// `localStorage` key of the per-browser "allow irregular" switch.
pub(crate) const PREF_KEY: &str = "wpg.builder.irregular";

/// A field whose value the builder computes (locked unless irregular is allowed).
#[must_use]
pub(crate) fn is_computed(fd: &FieldDef) -> bool {
    fd.auto.is_some() || matches!(fd.ty, FieldType::Padding { .. })
}

/// One out-of-spec finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Issue {
    /// Layer index, when the finding is about one.
    pub(crate) layer: Option<usize>,
    /// Top-level field name, when the finding is about one.
    pub(crate) field: Option<String>,
    /// Fluent key of the message.
    pub(crate) key: String,
    /// Fluent arguments.
    pub(crate) args: BTreeMap<String, String>,
    /// English fallback (diagnostics only; empty for `ui-*` keys).
    pub(crate) fallback: String,
}

impl Issue {
    fn ui(layer: usize, field: Option<&str>, key: &str, args: &[(&str, &str)]) -> Self {
        Self {
            layer: Some(layer),
            field: field.map(str::to_owned),
            key: key.to_owned(),
            args: args
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            fallback: String::new(),
        }
    }

    /// Whether this finding is about `field` of `layer`.
    #[must_use]
    pub(crate) fn is_for(&self, layer: usize, field: &str) -> bool {
        self.layer == Some(layer) && self.field.as_deref() == Some(field)
    }
}

/// Every out-of-spec finding of `t` and its build `p`: computed fields given a
/// Manual value or a generator, layers their parent does not announce, build errors
/// and warning / error diagnostics.
#[must_use]
pub(crate) fn issues(t: &Template, p: &Preview) -> Vec<Issue> {
    let mut out = Vec::new();
    for (li, layer) in t.layers.iter().enumerate() {
        if let Some(def) = proto::def(&layer.protocol) {
            for fd in def.fields.iter().filter(|f| is_computed(f)) {
                let manual = layer.fields.get(&fd.name).is_some_and(|i| !i.is_auto());
                if manual || layer.generators.contains_key(&fd.name) {
                    out.push(Issue::ui(
                        li,
                        Some(&fd.name),
                        "ui-bld-irr-auto-override",
                        &[("field", &fd.name)],
                    ));
                }
            }
        }
        if li > 0 {
            let parent = &t.layers[li - 1].protocol;
            if !proto::binding_ok(parent, &layer.protocol) {
                out.push(Issue::ui(
                    li,
                    None,
                    "ui-bld-irr-binding",
                    &[("parent", parent), ("child", &layer.protocol)],
                ));
            }
        }
    }
    for e in &p.errors {
        out.push(Issue {
            layer: None,
            field: None,
            key: String::new(),
            args: BTreeMap::new(),
            fallback: e.message.clone(),
        });
    }
    for d in p.diagnostics.iter().filter(|d| d.level >= DiagLevel::Warning) {
        // A computed field overridden on purpose is already reported above.
        if let (Some(l), Some(f)) = (d.layer, d.field.as_deref()) {
            if out.iter().any(|i| i.is_for(l, f)) {
                continue;
            }
        }
        out.push(Issue {
            layer: d.layer,
            field: d.field.clone(),
            key: d.key.clone(),
            args: d.args.clone(),
            fallback: d.message.clone(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::preview::build_preview;
    use crate::builder::state::edit;
    use wpg_template::{FieldInput, GeneratorSpec};

    fn udp() -> Template {
        let mut t = Template::new("t");
        for p in ["ethernet2", "ipv4", "udp", "payload"] {
            edit::add_layer(&mut t, p, None);
        }
        t
    }

    fn found(t: &Template) -> Vec<Issue> {
        issues(t, &build_preview(t, 0))
    }

    #[test]
    fn a_default_stack_is_regular() {
        assert_eq!(found(&udp()), Vec::new());
    }

    #[test]
    fn ordinary_manual_values_are_regular() {
        let mut t = udp();
        edit::set_field(&mut t, 0, "dst", FieldInput::Str("00:11:22:33:44:55".into()));
        edit::set_field(&mut t, 1, "src", FieldInput::Str("10.0.0.1".into()));
        edit::set_field(&mut t, 1, "ttl", FieldInput::Uint(1));
        edit::set_generator(&mut t, 1, "dst", GeneratorSpec::Text("10.0.0.1-10.0.0.9".into()));
        // `auto` written out is still Auto.
        edit::set_field(&mut t, 1, "header_checksum", FieldInput::Str("auto".into()));
        assert_eq!(found(&t), Vec::new());
    }

    #[test]
    fn overriding_a_computed_field_is_irregular() {
        let mut t = udp();
        edit::set_field(&mut t, 1, "header_checksum", FieldInput::Uint(0xbeef));
        let i = found(&t);
        assert_eq!(i.len(), 1, "{i:?}");
        assert!(i[0].is_for(1, "header_checksum"));
        assert_eq!(i[0].key, "ui-bld-irr-auto-override");

        let mut t = udp();
        edit::set_generator(&mut t, 2, "length", GeneratorSpec::Text("1-9".into()));
        assert!(found(&t).iter().any(|i| i.is_for(2, "length")));
    }

    #[test]
    fn an_unannounced_layer_is_irregular() {
        let mut t = Template::new("t");
        edit::add_layer(&mut t, "udp", None);
        edit::add_layer(&mut t, "ipv4", None);
        let i = found(&t);
        assert!(
            i.iter()
                .any(|i| i.key == "ui-bld-irr-binding" && i.layer == Some(1)),
            "{i:?}"
        );
    }

    #[test]
    fn computed_fields() {
        let ipv4 = proto::def("ipv4").unwrap();
        let computed: Vec<_> = ipv4
            .fields
            .iter()
            .filter(|f| is_computed(f))
            .map(|f| f.name.as_str())
            .collect();
        for f in ["ihl", "total_length", "protocol", "header_checksum", "padding"] {
            assert!(computed.contains(&f), "{f} in {computed:?}");
        }
        assert!(!computed.contains(&"src"));
    }
}
