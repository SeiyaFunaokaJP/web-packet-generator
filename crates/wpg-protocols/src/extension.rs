//! User-defined protocols (`protocols.d/`, spec §2.1 P3).
//!
//! An extension file is YAML with two lists:
//!
//! ```yaml
//! protocols:          # wpg_core::ProtocolDef, same shape as GET /api/protocols/{id}
//!   - id: acme_tlm
//!     name: ACME Telemetry
//!     layer: l7
//!     fields:
//!       - name: version
//!         type: { kind: uint, bits: 8 }
//!         default: { uint: 1 }
//! bind:               # entries added to the next-layer table of existing protocols
//!   - under: udp
//!     value: 47000
//!     protocol: acme_tlm
//! ```
//!
//! [`load_dirs`] reads every `*.yaml` / `*.yml` file of the given directories (in
//! file name order), [`with_extensions`] merges them into the built-in catalog and
//! [`install`] makes the result the process-wide [`catalog`](crate::catalog).
//! [`install_dirs`] does both and remembers the directories, so that [`reload`]
//! can read them again later (`POST /api/protocols/reload`).
//! Built-in protocols can not be replaced, and a binding value that a lower layer
//! already maps to another protocol is rejected rather than silently shadowed.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError, RwLock};

use serde::{Deserialize, Serialize};
use wpg_core::{Binding, BindingEntry, ProtocolDef};

use crate::{builtin, Catalog};

/// One extension file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Extension {
    /// Where it was loaded from (file name), for messages; set by the loader
    /// (a value in the file is ignored).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub source: String,
    /// New protocol definitions.
    pub protocols: Vec<ProtocolDef>,
    /// Entries added to the binding tables of existing protocols.
    pub bind: Vec<ExtensionBinding>,
}

/// "`protocol` is carried by `under` when its selector field equals `value`".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionBinding {
    /// Lower-layer protocol id (`udp`, `ethernet2`, ...).
    pub under: String,
    /// Selector value (UDP destination port, EtherType, ...).
    pub value: u64,
    /// Inclusive upper bound when the entry covers a range (`value..=max`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<u64>,
    /// Protocol id selected by the value (built-in or from an extension).
    pub protocol: String,
    /// Selector field; only needed when `under` has no next-layer binding yet,
    /// otherwise it must name the existing selector.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

/// Why extensions could not be loaded or merged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionError {
    /// File (or directory) the error is about.
    pub source: String,
    /// What is wrong.
    pub reason: String,
}

impl fmt::Display for ExtensionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.source, self.reason)
    }
}

impl std::error::Error for ExtensionError {}

fn err(source: &str, reason: impl Into<String>) -> ExtensionError {
    ExtensionError {
        source: source.to_owned(),
        reason: reason.into(),
    }
}

impl Extension {
    /// Parses one extension file (`source` names it in errors). Each protocol is
    /// validated on its own; references to other protocols are checked by
    /// [`with_extensions`].
    ///
    /// # Errors
    /// On YAML syntax / shape errors or an invalid definition.
    pub fn from_yaml(source: &str, text: &str) -> Result<Self, ExtensionError> {
        let de = serde_yaml::Deserializer::from_str(text);
        let mut ext: Self = serde_yaml::with::singleton_map_recursive::deserialize(de)
            .map_err(|e| err(source, e.to_string()))?;
        ext.source = source.to_owned();
        for (i, def) in ext.protocols.iter().enumerate() {
            def.validate()
                .map_err(|e| err(source, format!("protocols[{i}]: {e}")))?;
        }
        Ok(ext)
    }

    /// The file as YAML in the shape [`Extension::from_yaml`] reads (without
    /// `source`), e.g. to start a new definition from an existing one.
    ///
    /// # Errors
    /// If a value cannot be represented in YAML.
    pub fn to_yaml(&self) -> Result<String, ExtensionError> {
        let shown = Self {
            source: String::new(),
            ..self.clone()
        };
        let mut out = Vec::new();
        let mut ser = serde_yaml::Serializer::new(&mut out);
        serde_yaml::with::singleton_map_recursive::serialize(&shown, &mut ser)
            .map_err(|e| err(&self.source, e.to_string()))?;
        String::from_utf8(out).map_err(|e| err(&self.source, e.to_string()))
    }
}

/// Reads one extension file.
///
/// # Errors
/// If the file cannot be read or does not parse.
pub fn load_file(path: &Path) -> Result<Extension, ExtensionError> {
    let shown = path.display().to_string();
    let text = std::fs::read_to_string(path).map_err(|e| err(&shown, e.to_string()))?;
    Extension::from_yaml(&shown, &text)
}

/// The `*.yaml` / `*.yml` files directly in `dir`, sorted by file name.
///
/// # Errors
/// If the directory cannot be read.
pub fn files_in(dir: &Path) -> Result<Vec<PathBuf>, ExtensionError> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| err(&dir.display().to_string(), e.to_string()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_file()
                && p.extension().is_some_and(|x| {
                    x.eq_ignore_ascii_case("yaml") || x.eq_ignore_ascii_case("yml")
                })
        })
        .collect();
    files.sort();
    Ok(files)
}

/// Loads every `*.yaml` / `*.yml` file directly in `dirs`, sorted by file name
/// within each directory.
///
/// # Errors
/// If a directory or file cannot be read or a file does not parse.
pub fn load_dirs<P: AsRef<Path>>(dirs: &[P]) -> Result<Vec<Extension>, ExtensionError> {
    let mut out = Vec::new();
    for dir in dirs {
        for path in files_in(dir.as_ref())? {
            out.push(load_file(&path)?);
        }
    }
    Ok(out)
}

/// The built-in catalog plus `extensions`.
///
/// # Errors
/// If an extension redefines a protocol id, binds under an unknown protocol, to
/// a value already in use or through another selector field, or leaves a
/// reference to an unknown protocol.
pub fn with_extensions(extensions: &[Extension]) -> Result<Catalog, ExtensionError> {
    let mut defs = builtin();
    for ext in extensions {
        for (i, def) in ext.protocols.iter().enumerate() {
            if defs.iter().any(|d| d.id == def.id) {
                return Err(err(
                    &ext.source,
                    format!("protocols[{i}]: protocol `{}` is already defined", def.id),
                ));
            }
            def.validate()
                .map_err(|e| err(&ext.source, format!("protocols[{i}]: {e}")))?;
            defs.push(def.clone());
        }
    }
    for ext in extensions {
        for (i, b) in ext.bind.iter().enumerate() {
            apply_binding(&mut defs, b)
                .map_err(|reason| err(&ext.source, format!("bind[{i}]: {reason}")))?;
        }
    }
    for ext in extensions {
        let ids = ext
            .protocols
            .iter()
            .map(|d| d.id.as_str())
            .chain(ext.bind.iter().map(|b| b.under.as_str()));
        for id in ids {
            let Some(def) = defs.iter().find(|d| d.id == id) else {
                continue;
            };
            def.validate()
                .map_err(|e| err(&ext.source, e.to_string()))?;
            if let Some(unknown) = referenced(def).find(|r| !defs.iter().any(|d| d.id == *r)) {
                return Err(err(
                    &ext.source,
                    format!("`{id}` refers to unknown protocol `{unknown}`"),
                ));
            }
        }
    }
    Ok(Catalog::new(defs))
}

/// Protocol ids a definition's binding refers to.
fn referenced(def: &ProtocolDef) -> impl Iterator<Item = &str> {
    def.next.iter().flat_map(|b| {
        b.table
            .iter()
            .map(|e| e.protocol.as_str())
            .chain(b.default.as_deref())
            .chain(b.peek.iter().flat_map(|p| {
                p.table
                    .iter()
                    .map(|e| e.protocol.as_str())
                    .chain(p.default.as_deref())
            }))
    })
}

fn apply_binding(defs: &mut [ProtocolDef], b: &ExtensionBinding) -> Result<(), String> {
    let max = b.max.unwrap_or(b.value);
    if max < b.value {
        return Err(format!("binding `{}`: max is below value", b.protocol));
    }
    let lower = defs
        .iter_mut()
        .find(|d| d.id == b.under)
        .ok_or_else(|| format!("binding under unknown protocol `{}`", b.under))?;
    let binding = match (&mut lower.next, &b.field) {
        (Some(existing), Some(field)) if existing.field != *field => {
            return Err(format!(
                "`{}` selects its next layer by `{}`, not `{field}`",
                b.under, existing.field
            ))
        }
        (Some(existing), _) => existing,
        (next @ None, Some(field)) => next.insert(Binding::new(field.clone(), Vec::new())),
        (None, None) => {
            return Err(format!(
                "`{}` has no next-layer selector: give `field`",
                b.under
            ))
        }
    };
    let selector = &binding.field;
    let bits = lower
        .fields
        .iter()
        .find(|f| f.name == *selector)
        .filter(|f| f.ty.is_unsigned_int())
        .and_then(|f| f.ty.int_bits())
        .ok_or_else(|| format!("`{}` has no unsigned field `{selector}`", b.under))?;
    if bits < 64 && max >> bits != 0 {
        return Err(format!(
            "value {max} does not fit the {bits}-bit field `{}.{selector}`",
            b.under
        ));
    }
    if let Some(taken) = binding
        .table
        .iter()
        .find(|e| e.value <= max && b.value <= e.max.unwrap_or(e.value))
    {
        return Err(format!(
            "`{}.{selector}` value {} is already bound to `{}`",
            b.under, taken.value, taken.protocol
        ));
    }
    let mut entry = BindingEntry::new(b.value, b.protocol.clone());
    entry.max = b.max;
    binding.table.push(entry);
    Ok(())
}

struct Installed {
    catalog: &'static Catalog,
    extensions: &'static [Extension],
}

static INSTALLED: RwLock<Option<Installed>> = RwLock::new(None);

/// Every catalog [`install`] has built. Catalogs are handed out as `&'static`
/// (running jobs keep them), so they are never freed; installing a set of
/// extensions that was installed before reuses its catalog instead of building
/// another one, which bounds the memory to one catalog per distinct set.
type Built = (&'static [Extension], &'static Catalog);
static BUILT: Mutex<Vec<Built>> = Mutex::new(Vec::new());

/// Directories given to the last [`install_dirs`].
static DIRS: RwLock<Vec<PathBuf>> = RwLock::new(Vec::new());

/// The catalog made current by [`install`], if any.
pub(crate) fn installed() -> Option<&'static Catalog> {
    INSTALLED
        .read()
        .ok()
        .and_then(|g| g.as_ref().map(|i| i.catalog))
}

/// Merges `extensions` into the built-in catalog and makes the result the one
/// [`catalog`](crate::catalog) returns from now on. Call it at startup, before
/// anything keeps a reference to the catalog; calling it again (the web UI after
/// fetching the server's extensions, [`reload`]) switches the catalog for later
/// callers while earlier references stay valid. An empty list goes back to the
/// built-in catalog.
///
/// # Errors
/// As [`with_extensions`]; the current catalog is then left unchanged.
pub fn install(extensions: Vec<Extension>) -> Result<&'static Catalog, ExtensionError> {
    if extensions.is_empty() {
        *INSTALLED.write().unwrap_or_else(PoisonError::into_inner) = None;
        return Ok(crate::builtin_catalog());
    }
    let mut built = BUILT.lock().unwrap_or_else(PoisonError::into_inner);
    let (extensions, catalog) =
        if let Some(&hit) = built.iter().find(|(e, _)| **e == extensions[..]) {
            hit
        } else {
            let catalog: &'static Catalog = Box::leak(Box::new(with_extensions(&extensions)?));
            let extensions: &'static [Extension] = Box::leak(extensions.into_boxed_slice());
            built.push((extensions, catalog));
            (extensions, catalog)
        };
    *INSTALLED.write().unwrap_or_else(PoisonError::into_inner) = Some(Installed {
        catalog,
        extensions,
    });
    Ok(catalog)
}

/// [`load_dirs`] + [`install`], remembering `dirs` for [`reload`]. Returns the
/// installed extensions.
///
/// # Errors
/// As [`load_dirs`] and [`install`]; the current catalog is then left unchanged.
pub fn install_dirs<P: AsRef<Path>>(dirs: &[P]) -> Result<&'static [Extension], ExtensionError> {
    let dirs: Vec<PathBuf> = dirs.iter().map(|d| d.as_ref().to_path_buf()).collect();
    let exts = load_dirs(&dirs)?;
    *DIRS.write().unwrap_or_else(PoisonError::into_inner) = dirs;
    install(exts)?;
    Ok(extensions())
}

/// The directories of the last [`install_dirs`] (empty when it was not called).
#[must_use]
pub fn dirs() -> Vec<PathBuf> {
    DIRS.read().map(|g| g.clone()).unwrap_or_default()
}

/// Reads the directories of the last [`install_dirs`] again and installs what
/// they hold now (files added, changed or removed since). Returns the installed
/// extensions.
///
/// # Errors
/// As [`install_dirs`]; the current catalog is then left unchanged.
pub fn reload() -> Result<&'static [Extension], ExtensionError> {
    install_dirs(&dirs())
}

/// The extensions of the current catalog (empty when only built-ins are in use).
#[must_use]
pub fn extensions() -> &'static [Extension] {
    INSTALLED
        .read()
        .ok()
        .and_then(|g| g.as_ref().map(|i| i.extensions))
        .unwrap_or(&[])
}

#[cfg(test)]
mod tests {
    use wpg_core::{parse, FieldValue, Layer, Packet};

    use super::*;

    const ACME: &str = r"
protocols:
  - id: acme_tlm
    name: ACME Telemetry
    layer: l7
    fields:
      - name: version
        type: { kind: uint, bits: 8 }
        default: { uint: 1 }
      - name: sensor
        type: { kind: uint, bits: 16 }
        default: { uint: 0 }
      - name: value
        type: { kind: uint, bits: 32 }
        default: { uint: 0 }
bind:
  - under: udp
    value: 47000
    protocol: acme_tlm
";

    fn acme() -> Extension {
        Extension::from_yaml("acme.yaml", ACME).unwrap()
    }

    #[test]
    fn builds_and_parses_through_the_binding() {
        let cat = with_extensions(&[acme()]).unwrap();
        assert!(cat.get("acme_tlm").is_some());
        let pkt = Packet::from_layers(vec![
            Layer::new("ethernet2"),
            Layer::new("ipv4"),
            // UDP ports are plain fields (no Auto): the stream picks them.
            Layer::new("udp").with("dst_port", FieldValue::UInt(47000)),
            Layer::new("acme_tlm")
                .with("sensor", FieldValue::UInt(7))
                .with("value", FieldValue::UInt(0xdead_beef)),
        ])
        .unpadded();
        let built = pkt.build(&cat).unwrap();
        assert_eq!(&built.bytes[42..49], &[1, 0, 7, 0xde, 0xad, 0xbe, 0xef]);
        let parsed = parse(&built.bytes, "ethernet2", &cat).unwrap();
        let ids: Vec<&str> = parsed
            .packet
            .layers
            .iter()
            .map(|l| l.protocol.as_str())
            .collect();
        assert_eq!(ids, ["ethernet2", "ipv4", "udp", "acme_tlm"]);
        // The built-in catalog is untouched.
        assert!(builtin().iter().all(|d| d.id != "acme_tlm"));
    }

    #[test]
    fn json_round_trip_for_the_web_ui() {
        let ext = acme();
        let json = serde_json::to_string(&ext).unwrap();
        let back: Extension = serde_json::from_str(&json).unwrap();
        assert_eq!(back.protocols, ext.protocols);
        assert_eq!(back.bind, ext.bind);
    }

    #[test]
    fn rejects_conflicts() {
        let dup = Extension::from_yaml(
            "dup.yaml",
            "protocols:\n  - id: udp\n    name: U\n    layer: l4\n    fields: []\n",
        )
        .unwrap();
        let e = with_extensions(&[dup]).unwrap_err();
        assert_eq!(e.source, "dup.yaml");
        assert!(e.reason.contains("already defined"), "{e}");

        let taken = ACME.replace("47000", "4789");
        let e = with_extensions(&[Extension::from_yaml("t.yaml", &taken).unwrap()]).unwrap_err();
        assert!(e.reason.contains("vxlan"), "{e}");

        let wide = ACME.replace("47000", "70000");
        let e = with_extensions(&[Extension::from_yaml("w.yaml", &wide).unwrap()]).unwrap_err();
        assert!(e.reason.contains("does not fit"), "{e}");

        let unknown = ACME.replace("under: udp", "under: nope");
        let e = with_extensions(&[Extension::from_yaml("u.yaml", &unknown).unwrap()]).unwrap_err();
        assert!(e.reason.contains("nope"), "{e}");

        let field = ACME.replace(
            "protocol: acme_tlm\n",
            "protocol: acme_tlm\n    field: src_port\n",
        );
        let e = with_extensions(&[Extension::from_yaml("f.yaml", &field).unwrap()]).unwrap_err();
        assert!(e.reason.contains("dst_port"), "{e}");

        let e = Extension::from_yaml("s.yaml", "protocols: [1]").unwrap_err();
        assert_eq!(e.source, "s.yaml");
        let e = Extension::from_yaml("k.yaml", "protcols: []").unwrap_err();
        assert!(e.reason.contains("protcols"), "{e}");
    }

    #[test]
    fn rejects_dangling_references() {
        let src = ACME.replace(
            "      - name: value\n",
            "      - name: kind\n        type: { kind: uint, bits: 8 }\n        default: { uint: 0 }\n      - name: value\n",
        ) + "    next_dummy: 0\n";
        // Unknown keys inside a definition are reported by the parser.
        assert!(Extension::from_yaml("x.yaml", &src).is_err());

        let src = ACME.replace(
            "    fields:\n",
            "    next:\n      field: version\n      table: []\n      default: missing_proto\n    fields:\n",
        );
        let e = with_extensions(&[Extension::from_yaml("r.yaml", &src).unwrap()]).unwrap_err();
        assert!(e.reason.contains("missing_proto"), "{e}");
    }

    #[test]
    fn loads_directories_in_name_order() {
        let dir = std::env::temp_dir().join(format!("wpg-ext-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("b.yml"), "bind: []\n").unwrap();
        std::fs::write(dir.join("a.yaml"), ACME).unwrap();
        std::fs::write(dir.join("notes.txt"), "ignored").unwrap();
        let exts = load_dirs(&[&dir]).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        let names: Vec<&str> = exts
            .iter()
            .map(|e| e.source.rsplit(['/', '\\']).next().unwrap())
            .collect();
        assert_eq!(names, ["a.yaml", "b.yml"]);
        assert!(load_dirs(&[dir]).is_err());
    }

    #[test]
    fn install_switches_the_process_catalog() {
        let cat = install(vec![acme()]).unwrap();
        assert!(std::ptr::eq(cat, crate::catalog()));
        assert!(crate::catalog().get("acme_tlm").is_some());
        assert!(crate::builtin_catalog().get("acme_tlm").is_none());
        assert_eq!(extensions().len(), 1);
        // A failing install leaves the current catalog alone.
        let bad = Extension::from_yaml("b", &ACME.replace("47000", "4789")).unwrap();
        assert!(install(vec![bad]).is_err());
        assert!(crate::catalog().get("acme_tlm").is_some());
        // The same set again reuses its catalog instead of building another.
        assert!(std::ptr::eq(install(vec![acme()]).unwrap(), cat));

        // `reload` reads the directories of `install_dirs` again.
        let dir = std::env::temp_dir().join(format!("wpg-ext-reload-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.yaml"), ACME).unwrap();
        assert_eq!(install_dirs(&[&dir]).unwrap().len(), 1);
        assert_eq!(dirs(), std::slice::from_ref(&dir));
        std::fs::write(dir.join("a.yaml"), ACME.replace("acme_tlm", "acme_v2")).unwrap();
        reload().unwrap();
        assert!(crate::catalog().get("acme_v2").is_some());
        assert!(crate::catalog().get("acme_tlm").is_none());
        // A broken file leaves the catalog alone.
        std::fs::write(dir.join("b.yaml"), "protcols: []").unwrap();
        assert!(reload().is_err());
        assert!(crate::catalog().get("acme_v2").is_some());
        // No files: back to the built-in catalog.
        std::fs::remove_file(dir.join("a.yaml")).unwrap();
        std::fs::remove_file(dir.join("b.yaml")).unwrap();
        assert!(reload().unwrap().is_empty());
        assert!(std::ptr::eq(crate::catalog(), crate::builtin_catalog()));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn yaml_round_trip_and_error_paths() {
        let ext = acme();
        let yaml = ext.to_yaml().unwrap();
        assert!(!yaml.contains("source"), "{yaml}");
        let back = Extension::from_yaml("acme.yaml", &yaml).unwrap();
        assert_eq!(back, ext);
        // A built-in definition, renamed, is a valid starting point.
        let mut udp = builtin().into_iter().find(|d| d.id == "udp").unwrap();
        udp.id = "my_udp".into();
        let yaml = Extension {
            protocols: vec![udp],
            ..Extension::default()
        }
        .to_yaml()
        .unwrap();
        let ext = Extension::from_yaml("u.yaml", &yaml).unwrap();
        with_extensions(&[ext]).unwrap();

        let taken = ACME.replace("47000", "4789");
        let e = with_extensions(&[Extension::from_yaml("t.yaml", &taken).unwrap()]).unwrap_err();
        assert!(e.reason.starts_with("bind[0]: "), "{e}");
    }
}
