//! "Allow irregular packets" mode of the builder.
//!
//! Most users want packets that follow the protocol definitions, so the builder is
//! strict by default: computed fields (lengths, checksums, counters, padding) stay
//! locked to their Auto value, and anything that makes the packet out of spec is an
//! error that blocks Save / New job. Next-protocol selectors stay editable (a value
//! that does not announce the next layer is reported by the build). The findings
//! themselves come from the build's diagnostics, so a value that happens to equal
//! the computed one is not out of spec. Turning the mode on unlocks
//! those fields and turns the same findings into warnings, for deliberately
//! malformed test traffic.
//!
//! [`issues`] collects the findings from the template and its last in-browser
//! build; it is pure and unit tested.

use std::collections::BTreeMap;

use wpg_core::{AutoCompute, DiagLevel, FieldDef, FieldType};
use wpg_template::{FcsChoice, Template};

use super::preview::Preview;
use super::proto;

/// `localStorage` key of the per-browser "allow irregular" switch.
pub(crate) const PREF_KEY: &str = "wpg.builder.irregular";

/// A field whose value the builder computes (shown as Auto until overridden).
#[must_use]
pub(crate) fn is_computed(fd: &FieldDef) -> bool {
    fd.auto.is_some() || matches!(fd.ty, FieldType::Padding { .. })
}

/// A computed field that only an irregular packet overrides (everything but the
/// next-protocol selector, which may legitimately be set when no layer follows).
#[must_use]
pub(crate) fn is_locked(fd: &FieldDef) -> bool {
    is_computed(fd) && !matches!(fd.auto, Some(AutoCompute::NextProtocol))
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

/// Every out-of-spec finding of `t` and its build `p`: build errors, warning /
/// error diagnostics of the build (a length, checksum, count or next-protocol value
/// that does not match the packet, a value outside its constraints, ...), a
/// deliberately wrong FCS, and padding given a value.
///
/// A layer that its parent's binding table does not list is not a finding: many
/// valid stacks are selected by port or are not in the table at all (the layer
/// stack only hints at them).
#[must_use]
pub(crate) fn issues(t: &Template, p: &Preview) -> Vec<Issue> {
    let mut out = Vec::new();
    // An empty builder has nothing to check (its build only says "no layers").
    if t.layers.is_empty() {
        return out;
    }
    for (li, layer) in t.layers.iter().enumerate() {
        let Some(def) = proto::def(&layer.protocol) else {
            continue;
        };
        for fd in def
            .fields
            .iter()
            .filter(|f| matches!(f.ty, FieldType::Padding { .. }))
        {
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
    if t.packet.fcs == FcsChoice::Bad {
        out.push(Issue {
            layer: None,
            field: None,
            key: "ui-bld-irr-bad-fcs".into(),
            args: BTreeMap::new(),
            fallback: String::new(),
        });
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
    for d in p
        .diagnostics
        .iter()
        .filter(|d| d.level >= DiagLevel::Warning)
    {
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
    fn an_empty_builder_is_regular() {
        assert_eq!(found(&Template::new("t")), Vec::new());
    }

    #[test]
    fn a_default_stack_is_regular() {
        assert_eq!(found(&udp()), Vec::new());
    }

    #[test]
    fn ordinary_manual_values_are_regular() {
        let mut t = udp();
        edit::set_field(
            &mut t,
            0,
            "dst",
            FieldInput::Str("00:11:22:33:44:55".into()),
        );
        edit::set_field(&mut t, 1, "src", FieldInput::Str("10.0.0.1".into()));
        edit::set_field(&mut t, 1, "ttl", FieldInput::Uint(1));
        edit::set_generator(
            &mut t,
            1,
            "dst",
            GeneratorSpec::Text("10.0.0.1-10.0.0.9".into()),
        );
        // `auto` written out is still Auto.
        edit::set_field(&mut t, 1, "header_checksum", FieldInput::Str("auto".into()));
        assert_eq!(found(&t), Vec::new());
    }

    #[test]
    fn overriding_a_computed_field_is_irregular() {
        let mut t = udp();
        edit::set_field(&mut t, 1, "header_checksum", FieldInput::Uint(0xbeef));
        let i = found(&t);
        assert!(i.iter().any(|i| i.is_for(1, "header_checksum")), "{i:?}");

        let mut t = udp();
        edit::set_field(&mut t, 2, "length", FieldInput::Uint(9));
        assert!(found(&t).iter().any(|i| i.is_for(2, "length")));

        let mut t = udp();
        t.packet.fcs = FcsChoice::Bad;
        assert!(found(&t).iter().any(|i| i.key == "ui-bld-irr-bad-fcs"));
    }

    #[test]
    fn a_layer_missing_from_the_binding_table_is_regular() {
        // LLC under 802.3 is selected by the length field, not the binding table.
        let mut t = Template::new("t");
        edit::add_layer(&mut t, "dot3", None);
        edit::add_layer(&mut t, "llc", None);
        assert!(!proto::binding_ok("dot3", "llc"));
        assert_eq!(found(&t), Vec::new());
    }

    fn yaml_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for path in rd.flatten().map(|e| e.path()) {
            if path.is_dir() {
                yaml_files(&path, out);
            } else if path.to_string_lossy().ends_with(".wpg.yaml") {
                out.push(path);
            }
        }
    }

    /// The bundled templates: every regular one builds without findings (so the
    /// strict default does not flag them) and every `malformed/` one has some.
    #[test]
    fn bundled_templates() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates");
        let mut paths = Vec::new();
        yaml_files(&root, &mut paths);
        paths.sort();
        let mut wrong = Vec::new();
        for path in &paths {
            let text = std::fs::read_to_string(path).unwrap();
            let Ok(t) = Template::from_yaml_str(&text) else {
                continue;
            };
            let i = found(&t);
            let malformed = path.components().any(|c| c.as_os_str() == "malformed");
            if malformed == i.is_empty() {
                wrong.push(format!("{}: {:?}", path.display(), i.first()));
            }
        }
        assert!(paths.len() > 100, "{}", paths.len());
        assert!(
            wrong.is_empty(),
            "{} of {}: {wrong:#?}",
            wrong.len(),
            paths.len()
        );
    }

    #[test]
    fn locked_fields() {
        let ipv4 = proto::def("ipv4").unwrap();
        let locked: Vec<_> = ipv4
            .fields
            .iter()
            .filter(|f| is_locked(f))
            .map(|f| f.name.as_str())
            .collect();
        assert_eq!(
            locked,
            ["ihl", "total_length", "header_checksum", "padding"]
        );
        let protocol = ipv4.field("protocol").unwrap();
        assert!(is_computed(protocol) && !is_locked(protocol));
    }

    #[test]
    fn a_next_protocol_override_is_regular_unless_it_mismatches() {
        let mut t = Template::new("t");
        edit::add_layer(&mut t, "ethernet2", None);
        edit::set_field(&mut t, 0, "ethertype", FieldInput::Uint(0x0800));
        assert_eq!(found(&t), Vec::new());

        let mut t = udp();
        edit::set_field(&mut t, 0, "ethertype", FieldInput::Uint(0x86dd));
        assert!(!found(&t).is_empty());
    }
}
