//! `.wpgbundle`: a zip archive that carries templates (and optionally capture
//! attachments) between installations.
//!
//! ```text
//! manifest.json               { version, created, templates: [{path, id, name}], attachments: [..] }
//! templates/<id>.wpg.yaml     one file per template
//! attachments/<name>          optional captures (pcap / pcapng) or other data
//! README.md                   generated listing (informational)
//! ```
//!
//! Archive entries are stored with fixed timestamps so that exporting the same
//! input twice yields identical bytes (except `created`).

use std::collections::BTreeSet;
use std::io::{Read, Seek, Write};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use wpg_core::Resolve;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::{lint, LintIssue, LintLevel, Template, TemplateError};

/// Bundle layout version written by [`export_bundle`].
pub const BUNDLE_VERSION: u32 = 1;
/// Largest single entry that is read back (guards against zip bombs).
const MAX_ENTRY_BYTES: u64 = 256 * 1024 * 1024;

/// Errors of [`export_bundle`] / [`import_bundle`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BundleError {
    /// Archive level error.
    #[error("bundle archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
    /// I/O error.
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// `manifest.json` is not valid.
    #[error("invalid manifest.json: {0}")]
    Manifest(#[from] serde_json::Error),
    /// The bundle was written by a newer wpg.
    #[error("unsupported bundle version {0} (this build supports {BUNDLE_VERSION})")]
    UnsupportedVersion(u32),
    /// A template inside the bundle could not be read.
    #[error("{path}: {source}")]
    Template {
        /// Path inside the archive.
        path: String,
        /// Why.
        source: TemplateError,
    },
    /// An entry name is unsafe (absolute path, `..`, backslash, ...).
    #[error("unsafe entry name `{0}` in bundle")]
    UnsafePath(String),
    /// An entry is larger than the sanity limit.
    #[error("entry `{0}` is too large")]
    TooLarge(String),
    /// The manifest lists a template that is not in the archive.
    #[error("manifest lists `{0}` but the archive has no such file")]
    MissingEntry(String),
}

/// One template listed in the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestTemplate {
    /// Path inside the archive (`templates/<id>.wpg.yaml`).
    pub path: String,
    /// File-name safe identifier (unique within the bundle).
    pub id: String,
    /// `meta.name` of the template.
    pub name: String,
}

/// `manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Layout version.
    pub version: u32,
    /// Creation time, RFC 3339 UTC.
    pub created: String,
    /// The templates.
    #[serde(default)]
    pub templates: Vec<ManifestTemplate>,
    /// Attachment paths inside the archive (`attachments/<name>`).
    #[serde(default)]
    pub attachments: Vec<String>,
}

/// A template read from a bundle, with its lint findings.
#[derive(Debug, Clone)]
pub struct BundleTemplate {
    /// Path inside the archive.
    pub path: String,
    /// The template.
    pub template: Template,
    /// Findings of [`lint()`] against the given catalog.
    pub issues: Vec<LintIssue>,
}

impl BundleTemplate {
    /// Whether linting found errors.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.issues.iter().any(|i| i.level == LintLevel::Error)
    }
}

/// Everything [`import_bundle`] found.
#[derive(Debug, Clone)]
pub struct ImportedBundle {
    /// The manifest (synthesised when the archive has none).
    pub manifest: Manifest,
    /// Templates in manifest order (unlisted ones last).
    pub templates: Vec<BundleTemplate>,
    /// `(file name, bytes)` of the attachments.
    pub attachments: Vec<(String, Vec<u8>)>,
}

/// File-name safe identifier: lower-case ASCII letters / digits, `-` and `_`
/// (anything else becomes `-`); never empty.
#[must_use]
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_owned();
    if out.is_empty() {
        "template".to_owned()
    } else {
        out
    }
}

fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.contains('\\')
        && !name.contains(':')
        && name
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
}

/// Exports `templates` (`(preferred id, template)`) and `attachments`
/// (`(file name, bytes)`) as a bundle written to `out`.
///
/// # Errors
/// Archive / I/O errors, serialisation errors and unsafe attachment names.
pub fn export_bundle<W: Write + Seek>(
    templates: &[(String, Template)],
    attachments: &[(String, Vec<u8>)],
    out: W,
) -> Result<Manifest, BundleError> {
    export_bundle_at(templates, attachments, out, &now_rfc3339())
}

/// [`export_bundle`] with an explicit `created` timestamp (reproducible output).
///
/// # Errors
/// See [`export_bundle`].
pub fn export_bundle_at<W: Write + Seek>(
    templates: &[(String, Template)],
    attachments: &[(String, Vec<u8>)],
    out: W,
    created: &str,
) -> Result<Manifest, BundleError> {
    let mut used = BTreeSet::new();
    let mut entries = Vec::new();
    for (pref, t) in templates {
        let base = slug(if pref.is_empty() { &t.meta.name } else { pref });
        let mut id = base.clone();
        let mut n = 2;
        while !used.insert(id.clone()) {
            id = format!("{base}-{n}");
            n += 1;
        }
        entries.push(ManifestTemplate {
            path: format!("templates/{id}.wpg.yaml"),
            id,
            name: t.meta.name.clone(),
        });
    }
    let mut attach_paths = Vec::new();
    for (name, _) in attachments {
        let path = format!("attachments/{name}");
        if name.contains('/') || !safe_name(&path) {
            return Err(BundleError::UnsafePath(path));
        }
        attach_paths.push(path);
    }
    let manifest = Manifest {
        version: BUNDLE_VERSION,
        created: created.to_owned(),
        templates: entries,
        attachments: attach_paths,
    };

    let mut zip = ZipWriter::new(out);
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file("manifest.json", opts)?;
    zip.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;
    for ((_, t), e) in templates.iter().zip(&manifest.templates) {
        zip.start_file(&e.path, opts)?;
        zip.write_all(
            t.to_yaml_string()
                .map_err(|source| BundleError::Template {
                    path: e.path.clone(),
                    source,
                })?
                .as_bytes(),
        )?;
    }
    for ((_, data), path) in attachments.iter().zip(&manifest.attachments) {
        zip.start_file(path, opts)?;
        zip.write_all(data)?;
    }
    zip.start_file("README.md", opts)?;
    zip.write_all(readme(&manifest, templates).as_bytes())?;
    zip.finish()?;
    Ok(manifest)
}

fn readme(m: &Manifest, templates: &[(String, Template)]) -> String {
    let mut s = String::from("# wpg bundle\n\n");
    s.push_str(&format!(
        "Created {} by wpg. Import with `wpg template import <this file> -o <dir>`.\n\n",
        m.created
    ));
    s.push_str("## Templates\n\n");
    for ((_, t), e) in templates.iter().zip(&m.templates) {
        let stack: Vec<&str> = t.layers.iter().map(|l| l.protocol.as_str()).collect();
        s.push_str(&format!(
            "- `{}` **{}**: {}",
            e.path,
            e.name,
            stack.join(" / ")
        ));
        if let Some(d) = &t.meta.description {
            s.push_str(&format!(" - {}", d.lines().next().unwrap_or_default()));
        }
        s.push('\n');
    }
    if !m.attachments.is_empty() {
        s.push_str("\n## Attachments\n\n");
        for a in &m.attachments {
            s.push_str(&format!("- `{a}`\n"));
        }
    }
    s
}

fn read_entry<R: Read + Seek>(zip: &mut ZipArchive<R>, name: &str) -> Result<Vec<u8>, BundleError> {
    let f = zip.by_name(name)?;
    let mut buf = Vec::new();
    f.take(MAX_ENTRY_BYTES + 1).read_to_end(&mut buf)?;
    if buf.len() as u64 > MAX_ENTRY_BYTES {
        return Err(BundleError::TooLarge(name.to_owned()));
    }
    Ok(buf)
}

/// Reads a bundle: the manifest, every template (parsed and linted against
/// `catalog`) and the attachments. Lint errors do not fail the import; check
/// [`BundleTemplate::has_errors`].
///
/// # Errors
/// Archive errors, a newer bundle version, unsafe entry names, unreadable templates
/// or a manifest entry without a file.
pub fn import_bundle<R: Read + Seek>(
    reader: R,
    catalog: &dyn Resolve,
) -> Result<ImportedBundle, BundleError> {
    let mut zip = ZipArchive::new(reader)?;
    let names: Vec<String> = zip
        .file_names()
        .map(|n| n.map(std::borrow::Cow::into_owned))
        .collect::<Result<_, _>>()?;
    for n in &names {
        if !safe_name(n.trim_end_matches('/')) {
            return Err(BundleError::UnsafePath(n.clone()));
        }
    }
    let is_template = |n: &str| n.starts_with("templates/") && n.ends_with(".wpg.yaml");
    let mut manifest = if names.iter().any(|n| n == "manifest.json") {
        let m: Manifest = serde_json::from_slice(&read_entry(&mut zip, "manifest.json")?)?;
        if m.version == 0 || m.version > BUNDLE_VERSION {
            return Err(BundleError::UnsupportedVersion(m.version));
        }
        m
    } else {
        Manifest {
            version: BUNDLE_VERSION,
            created: String::new(),
            templates: Vec::new(),
            attachments: Vec::new(),
        }
    };
    // manifest order first, then templates the manifest does not know
    let mut paths: Vec<String> = Vec::new();
    for e in &manifest.templates {
        if !names.contains(&e.path) {
            return Err(BundleError::MissingEntry(e.path.clone()));
        }
        paths.push(e.path.clone());
    }
    for n in &names {
        if is_template(n) && !paths.contains(n) {
            paths.push(n.clone());
        }
    }
    let mut templates = Vec::new();
    for path in paths {
        let text = String::from_utf8_lossy(&read_entry(&mut zip, &path)?).into_owned();
        let template = Template::from_yaml_str(&text).map_err(|source| BundleError::Template {
            path: path.clone(),
            source,
        })?;
        if !manifest.templates.iter().any(|e| e.path == path) {
            let id = path
                .trim_start_matches("templates/")
                .trim_end_matches(".wpg.yaml")
                .to_owned();
            manifest.templates.push(ManifestTemplate {
                path: path.clone(),
                id,
                name: template.meta.name.clone(),
            });
        }
        let issues = lint(&template, catalog);
        templates.push(BundleTemplate {
            path,
            template,
            issues,
        });
    }
    let mut attachments = Vec::new();
    for n in &names {
        if let Some(file) = n.strip_prefix("attachments/") {
            if !file.is_empty() && !file.contains('/') && !n.ends_with('/') {
                attachments.push((file.to_owned(), read_entry(&mut zip, n)?));
                if !manifest.attachments.contains(n) {
                    manifest.attachments.push(n.clone());
                }
            }
        }
    }
    Ok(ImportedBundle {
        manifest,
        templates,
        attachments,
    })
}

/// RFC 3339 UTC timestamp (seconds) of now.
fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    rfc3339(secs)
}

#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
fn rfc3339(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    struct NoCatalog;
    impl Resolve for NoCatalog {
        fn protocol(&self, _: &str) -> Option<&wpg_core::ProtocolDef> {
            None
        }
    }

    fn t(name: &str, protocol: &str) -> Template {
        let mut t = Template::new(name);
        t.meta.description = Some(format!("about {name}"));
        t.layers.push(crate::TemplateLayer {
            protocol: protocol.into(),
            ..crate::TemplateLayer::default()
        });
        t
    }

    fn export(templates: &[(String, Template)], att: &[(String, Vec<u8>)]) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        export_bundle_at(templates, att, &mut buf, "2026-09-29T00:00:00Z").unwrap();
        buf.into_inner()
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("ARP sweep (v2)!"), "arp-sweep-v2");
        assert_eq!(slug("日本語"), "template");
        assert_eq!(slug("a_b"), "a_b");
    }

    #[test]
    fn round_trip_with_attachments_and_duplicate_ids() {
        let templates = vec![
            ("same".to_owned(), t("First", "ethernet2")),
            ("same".to_owned(), t("Second", "arp")),
            (String::new(), t("Named Only", "ipv4")),
        ];
        let att = vec![("cap.pcapng".to_owned(), vec![1, 2, 3, 4])];
        let bytes = export(&templates, &att);
        // deterministic
        assert_eq!(bytes, export(&templates, &att));

        let b = import_bundle(Cursor::new(&bytes), &NoCatalog).unwrap();
        assert_eq!(b.manifest.version, BUNDLE_VERSION);
        assert_eq!(b.manifest.created, "2026-09-29T00:00:00Z");
        let paths: Vec<&str> = b.templates.iter().map(|x| x.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "templates/same.wpg.yaml",
                "templates/same-2.wpg.yaml",
                "templates/named-only.wpg.yaml"
            ]
        );
        for (got, (_, want)) in b.templates.iter().zip(&templates) {
            assert_eq!(&got.template, want);
            // the empty catalog knows no protocol: lint reports errors, import does not fail
            assert!(got.has_errors());
        }
        assert_eq!(b.attachments, att);
        assert_eq!(b.manifest.templates[2].name, "Named Only");

        let mut z = ZipArchive::new(Cursor::new(&bytes)).unwrap();
        let readme = String::from_utf8(read_entry(&mut z, "README.md").unwrap()).unwrap();
        assert!(readme.contains("templates/same-2.wpg.yaml") && readme.contains("cap.pcapng"));
    }

    #[test]
    fn rejects_bad_bundles() {
        assert!(import_bundle(Cursor::new(b"not a zip".to_vec()), &NoCatalog).is_err());

        let raw = |files: &[(&str, &str)]| {
            let mut z = ZipWriter::new(Cursor::new(Vec::new()));
            for (n, c) in files {
                z.start_file(*n, SimpleFileOptions::default()).unwrap();
                z.write_all(c.as_bytes()).unwrap();
            }
            z.finish().unwrap().into_inner()
        };
        let newer = raw(&[("manifest.json", r#"{"version":9,"created":""}"#)]);
        assert!(matches!(
            import_bundle(Cursor::new(newer), &NoCatalog),
            Err(BundleError::UnsupportedVersion(9))
        ));
        let evil = raw(&[("templates/../../evil.wpg.yaml", "x")]);
        assert!(matches!(
            import_bundle(Cursor::new(evil), &NoCatalog),
            Err(BundleError::UnsafePath(_))
        ));
        let missing = raw(&[(
            "manifest.json",
            r#"{"version":1,"created":"","templates":[{"path":"templates/a.wpg.yaml","id":"a","name":"a"}]}"#,
        )]);
        assert!(matches!(
            import_bundle(Cursor::new(missing), &NoCatalog),
            Err(BundleError::MissingEntry(_))
        ));
        let broken = raw(&[("templates/x.wpg.yaml", "schema: 1\nlayerz: 1\n")]);
        assert!(matches!(
            import_bundle(Cursor::new(broken), &NoCatalog),
            Err(BundleError::Template { .. })
        ));
        // a bundle without manifest still yields its templates
        let bare = raw(&[("templates/x.wpg.yaml", "schema: 1\nmeta: {name: X}\n")]);
        let b = import_bundle(Cursor::new(bare), &NoCatalog).unwrap();
        assert_eq!(b.manifest.templates[0].id, "x");
        assert_eq!(b.templates.len(), 1);
    }

    #[test]
    fn unsafe_attachment_names_are_refused() {
        let mut buf = Cursor::new(Vec::new());
        let r = export_bundle(&[], &[("../x".to_owned(), vec![])], &mut buf);
        assert!(matches!(r, Err(BundleError::UnsafePath(_))));
        let r = export_bundle(&[], &[("a/b".to_owned(), vec![])], &mut buf);
        assert!(matches!(r, Err(BundleError::UnsafePath(_))));
    }

    #[test]
    fn timestamps() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
        assert!(now_rfc3339().starts_with("20"));
    }
}
