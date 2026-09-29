//! Guards the bundled template library (`templates/**`): every file parses, compiles
//! and renders; everything outside `templates/malformed/` lints clean; metadata
//! (ja/en descriptions, tags) is complete and consistent with the catalog; and the
//! per-protocol samples cover every built-in protocol.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use wpg_core::parse;
use wpg_protocols::catalog;
use wpg_template::render::render_with;
use wpg_template::{compile, lint, LintLevel, Template, VarOverrides};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates")
}

fn all_files() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                walk(&p, out);
            } else if p.to_string_lossy().ends_with(".wpg.yaml") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(&root(), &mut out);
    out
}

/// Path relative to `templates/`, with `/` separators.
fn rel(p: &Path) -> String {
    p.strip_prefix(root())
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

fn load(p: &Path) -> Template {
    Template::from_path(p).unwrap_or_else(|e| panic!("{}: {e}", rel(p)))
}

fn is_malformed(p: &Path) -> bool {
    rel(p).starts_with("malformed/")
}

fn tag_values<'a>(t: &'a Template, prefix: &str) -> BTreeSet<&'a str> {
    t.meta
        .tags
        .iter()
        .filter_map(|tag| tag.strip_prefix(prefix))
        .collect()
}

#[test]
fn library_layout_has_the_expected_groups() {
    let files: Vec<String> = all_files().iter().map(|p| rel(p)).collect();
    let count = |prefix: &str| files.iter().filter(|f| f.starts_with(prefix)).count();
    assert!(count("examples/") >= 5, "{files:?}");
    assert!(count("protocols/") >= 30, "{files:?}");
    assert!(count("scenarios/") >= 8, "{files:?}");
    assert!(count("malformed/") >= 4, "{files:?}");
    for f in &files {
        let top = f.split('/').next().unwrap();
        assert!(
            ["examples", "protocols", "scenarios", "malformed"].contains(&top),
            "unexpected top-level group in {f}"
        );
    }
}

#[test]
fn every_template_compiles_and_renders() {
    for f in all_files() {
        let name = rel(&f);
        let t = load(&f);
        let c = compile(&t, &VarOverrides::new(), catalog())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut n = 0;
        render_with(&c, catalog(), Some(3), &mut |_, bytes| {
            assert!(!bytes.is_empty(), "{name}: empty frame");
            n += 1;
            Ok(())
        })
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(n >= 1, "{name}: rendered nothing");
        // the canonical YAML form round-trips
        assert_eq!(
            Template::from_yaml_str(&t.to_yaml_string().unwrap()).unwrap(),
            t,
            "{name}"
        );
    }
}

#[test]
fn lint_is_clean_except_for_deliberate_malformed_samples() {
    let mut malformed_with_warning = 0;
    for f in all_files() {
        let name = rel(&f);
        let issues = lint(&load(&f), catalog());
        if is_malformed(&f) {
            let errors: Vec<_> = issues
                .iter()
                .filter(|i| i.level == LintLevel::Error)
                .collect();
            assert!(errors.is_empty(), "{name}: {errors:#?}");
            if !issues.is_empty() {
                malformed_with_warning += 1;
            }
        } else {
            assert!(issues.is_empty(), "{name}: {issues:#?}");
        }
    }
    assert!(
        malformed_with_warning >= 4,
        "malformed samples should trigger build warnings (found {malformed_with_warning})"
    );
}

#[test]
fn metadata_is_complete_and_names_are_unique() {
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    for f in all_files() {
        let name = rel(&f);
        let t = load(&f);
        assert_eq!(t.schema, 1, "{name}");
        for lang in ["ja", "en"] {
            let d = t.meta.descriptions.get(lang).map_or("", |s| s.trim());
            assert!(d.len() >= 10, "{name}: missing meta.descriptions.{lang}");
        }
        assert!(
            t.meta.descriptions["ja"].chars().any(|c| !c.is_ascii()),
            "{name}: ja description has no Japanese text"
        );
        assert!(!t.meta.tags.is_empty(), "{name}: no tags");
        assert_eq!(
            t.meta.tags.iter().collect::<BTreeSet<_>>().len(),
            t.meta.tags.len(),
            "{name}: duplicate tags"
        );
        if let Some(prev) = names.insert(t.meta.name.clone(), name.clone()) {
            panic!("meta.name `{}` used by {prev} and {name}", t.meta.name);
        }
        // `malformed` is reserved for templates in malformed/
        assert_eq!(
            t.meta.tags.iter().any(|x| x == "malformed"),
            is_malformed(&f),
            "{name}: the `malformed` tag and the malformed/ folder must go together"
        );
        // every file states how it is meant to be used
        let text = std::fs::read_to_string(&f).unwrap();
        assert!(text.starts_with('#'), "{name}: missing leading comment");
    }
}

#[test]
fn tags_match_the_layer_stack() {
    let cat = catalog();
    for f in all_files() {
        let name = rel(&f);
        if name.starts_with("examples/") {
            continue;
        }
        let t = load(&f);
        let layers: BTreeSet<&str> = t.layers.iter().map(|l| l.protocol.as_str()).collect();
        let tagged = tag_values(&t, "protocol:");
        assert!(!tagged.is_empty(), "{name}: no protocol:<id> tag");
        for id in &tagged {
            assert!(cat.get(id).is_some(), "{name}: unknown tag protocol:{id}");
            assert!(
                layers.contains(id),
                "{name}: tagged protocol:{id} is not in the layer stack {layers:?}"
            );
        }
        let layer_tags = tag_values(&t, "layer:");
        assert_eq!(layer_tags.len(), 1, "{name}: exactly one layer:<l2|l3|l4> tag");
        assert!(
            ["l2", "l3", "l4"].contains(layer_tags.iter().next().unwrap()),
            "{name}: {layer_tags:?}"
        );
        if name.starts_with("scenarios/") {
            assert!(t.meta.tags.iter().any(|x| x == "scenario"), "{name}");
        }
    }
}

#[test]
fn protocol_samples_cover_the_whole_catalog() {
    let mut used = BTreeSet::new();
    for f in all_files() {
        if rel(&f).starts_with("protocols/") {
            for l in &load(&f).layers {
                used.insert(l.protocol.clone());
            }
        }
    }
    let missing: Vec<&str> = catalog()
        .iter()
        .map(|p| p.id.as_str())
        .filter(|id| !used.contains(*id))
        .collect();
    assert!(missing.is_empty(), "no sample for protocols {missing:?}");
}

/// The first frame of every well-formed protocol sample decodes back to the same
/// protocol stack (payload octets show up as `raw`).
#[test]
fn well_formed_protocol_samples_parse_back_to_their_stack() {
    for f in all_files() {
        let name = rel(&f);
        if !name.starts_with("protocols/") {
            continue;
        }
        let t = load(&f);
        // a fragment's payload is not a UDP datagram of its own
        if t.meta.tags.iter().any(|x| x == "fragmentation") && t.layers.len() <= 3 {
            continue;
        }
        let c = compile(&t, &VarOverrides::new(), catalog()).unwrap();
        let built = c.packet.build(catalog()).unwrap();
        let parsed = parse(&built.bytes, &t.layers[0].protocol, catalog())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!parsed.has_errors(), "{name}: {:?}", parsed.diagnostics);
        let norm = |ids: Vec<String>| -> Vec<String> {
            let mut v: Vec<String> = ids
                .into_iter()
                .map(|p| if p == "payload" { "raw".to_owned() } else { p })
                .collect();
            // trailing padding / payload bytes may or may not decode as a raw layer
            while v.last().is_some_and(|p| p == "raw") {
                v.pop();
            }
            v
        };
        let want = norm(t.layers.iter().map(|l| l.protocol.clone()).collect());
        let got = norm(parsed.packet.layers.iter().map(|l| l.protocol.clone()).collect());
        assert_eq!(got, want, "{name}");
    }
}

#[test]
fn malformed_samples_state_their_intent() {
    for f in all_files().iter().filter(|f| is_malformed(f)) {
        let name = rel(f);
        let t = load(f);
        for lang in ["ja", "en"] {
            assert!(
                t.meta.descriptions[lang].starts_with('['),
                "{name}: {lang} description should start with a `[Malformed]`-style marker"
            );
        }
    }
}
