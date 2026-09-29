//! File-backed template store (TODO §2.4 "管理機能", "ライブラリパス複数指定 …
//! ホットリロード").
//!
//! Templates live as files: any number of read-only *library* directories (Git
//! checkouts, the bundled `templates/`) plus one writable *user* directory. The
//! store scans them recursively for `*.wpg.yaml` / `*.wpg.yml` / `*.wpg.json` and
//! keeps an in-memory index (name, tags, protocol stack, descriptions). The index
//! is refreshed after every write through the store and by a debounced file
//! watcher ([`TemplateStore::watch`]), which also announces
//! [`ServerEvent::TemplatesChanged`].
//!
//! Template ids are stable slugs of the path relative to its root
//! (`examples/arp-sweep.wpg.yaml` → `examples-arp-sweep`); the user directory is
//! scanned first, so on a collision the user copy keeps the plain slug and later
//! ones get `-2`, `-3`, ...
//!
//! Favourites and the recently used list are kept in `<data_dir>/template-meta.json`
//! (they move to `SQLite` with the authentication work).

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, RwLock, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use indexmap::IndexMap;
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebounceEventResult, Debouncer};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;
use wpg_template::{Template, TemplateError};

use crate::events::{EventBus, ServerEvent};

/// Recently used entries kept.
const RECENT_MAX: usize = 20;
/// Directory recursion limit.
const MAX_DEPTH: usize = 16;
/// Template file suffixes.
const SUFFIXES: &[&str] = &[".wpg.yaml", ".wpg.yml", ".wpg.json"];

/// Where a template comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TemplateSource {
    /// Writable user directory.
    User,
    /// Read-only library path.
    Library,
}

/// Index entry of a template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct TemplateSummary {
    /// Stable id (slug of the relative path).
    pub id: String,
    /// `meta.name` (the file stem when the file does not parse).
    pub name: String,
    /// Description in the requested language (`meta.descriptions[lang]`, else
    /// `meta.description`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// All localised descriptions (`ja`, `en`).
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    #[schema(value_type = BTreeMap<String, String>)]
    pub descriptions: IndexMap<String, String>,
    /// Tags.
    pub tags: Vec<String>,
    /// Protocol stack, outermost first.
    pub protocols: Vec<String>,
    /// Author.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Library or user template.
    pub source: TemplateSource,
    /// Path relative to its root directory.
    pub path: String,
    /// The root directory.
    pub root: String,
    /// Whether it can be updated / deleted through the API.
    pub writable: bool,
    /// Marked as favourite.
    pub favorite: bool,
    /// Last use (open in the GUI, job start), Unix milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used: Option<u64>,
    /// File modification time, Unix milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified: Option<u64>,
    /// Load error when the file does not parse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Store errors.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StoreError {
    /// Unknown id.
    #[error("template `{0}` not found")]
    NotFound(String),
    /// The template is in a read-only library.
    #[error("template `{0}` is read-only")]
    ReadOnly(String),
    /// The file exists but does not parse.
    #[error("template `{id}` cannot be loaded: {error}")]
    Broken {
        /// Template id.
        id: String,
        /// Load error.
        error: String,
    },
    /// Parse / serialisation error.
    #[error(transparent)]
    Template(#[from] TemplateError),
    /// File system error.
    #[error("{path}: {source}")]
    Io {
        /// Path.
        path: String,
        /// Error.
        source: std::io::Error,
    },
}

fn io_err(path: &Path) -> impl FnOnce(std::io::Error) -> StoreError + '_ {
    move |source| StoreError::Io {
        path: path.display().to_string(),
        source,
    }
}

#[derive(Debug, Clone)]
struct Root {
    dir: PathBuf,
    writable: bool,
}

#[derive(Debug, Clone)]
struct Entry {
    summary: TemplateSummary,
    file: PathBuf,
    template: Option<Template>,
}

/// Favourites and recently used templates (`template-meta.json`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct UserMeta {
    #[serde(default)]
    favorites: BTreeSet<String>,
    /// Most recent first.
    #[serde(default)]
    recent: Vec<(String, u64)>,
}

struct Inner {
    roots: Vec<Root>,
    index: RwLock<BTreeMap<String, Entry>>,
    /// Serialises scans and writes.
    scan_lock: Mutex<()>,
    meta_path: PathBuf,
    meta: Mutex<UserMeta>,
    events: EventBus,
    watcher: Mutex<Option<Debouncer<notify::RecommendedWatcher>>>,
}

/// The template store (cheap to clone).
#[derive(Clone)]
pub struct TemplateStore {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for TemplateStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TemplateStore")
            .field("roots", &self.inner.roots)
            .field("templates", &read(&self.inner.index).len())
            .finish_non_exhaustive()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn read<T>(m: &RwLock<T>) -> std::sync::RwLockReadGuard<'_, T> {
    m.read().unwrap_or_else(PoisonError::into_inner)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Slug of a relative template path: lower-case alphanumerics separated by `-`,
/// without the template suffix.
#[must_use]
pub fn slug(rel: &str) -> String {
    let lower = rel.to_ascii_lowercase().replace('\\', "/");
    let stem = SUFFIXES
        .iter()
        .find_map(|s| lower.strip_suffix(s))
        .unwrap_or(&lower);
    let mut out = String::with_capacity(stem.len());
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
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

fn is_template_file(p: &Path) -> bool {
    p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
        let n = n.to_ascii_lowercase();
        SUFFIXES.iter().any(|s| n.ends_with(s)) && !n.starts_with('.')
    })
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for e in entries {
        let p = e.path();
        let hidden = e.file_name().to_string_lossy().starts_with('.');
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() && !hidden {
            walk(&p, depth + 1, out);
        } else if (ft.is_file() || ft.is_symlink()) && p.is_file() && is_template_file(&p) {
            out.push(p);
        }
    }
}

fn mtime_ms(p: &Path) -> Option<u64> {
    let t = std::fs::metadata(p).ok()?.modified().ok()?;
    let d = t.duration_since(UNIX_EPOCH).ok()?;
    u64::try_from(d.as_millis()).ok()
}

fn rel_string(p: &Path, root: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Search filters of [`TemplateStore::list`].
#[derive(Debug, Clone, Default)]
pub struct ListFilter {
    /// Case-insensitive substring of id, name, descriptions, tags or protocols.
    pub q: Option<String>,
    /// Required tags (all of them).
    pub tags: Vec<String>,
    /// Only favourites.
    pub favorite: Option<bool>,
    /// Only this source.
    pub source: Option<TemplateSource>,
}

impl TemplateStore {
    /// Opens the store: `user_dir` (created if missing) plus read-only `libraries`,
    /// and performs the first scan. Favourites / recent are kept in `meta_path`.
    ///
    /// # Errors
    /// If the user directory cannot be created.
    pub fn open(
        user_dir: &Path,
        libraries: &[PathBuf],
        meta_path: PathBuf,
        events: EventBus,
    ) -> Result<Self, StoreError> {
        std::fs::create_dir_all(user_dir).map_err(io_err(user_dir))?;
        let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        let mut seen = HashSet::new();
        let mut roots = Vec::new();
        for (dir, writable) in
            std::iter::once((user_dir, true)).chain(libraries.iter().map(|p| (p.as_path(), false)))
        {
            if seen.insert(canon(dir)) {
                roots.push(Root {
                    dir: dir.to_path_buf(),
                    writable,
                });
            }
        }
        let meta = std::fs::read(&meta_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let store = Self {
            inner: Arc::new(Inner {
                roots,
                index: RwLock::new(BTreeMap::new()),
                scan_lock: Mutex::new(()),
                meta_path,
                meta: Mutex::new(meta),
                events,
                watcher: Mutex::new(None),
            }),
        };
        store.rescan();
        Ok(store)
    }

    /// The writable directory.
    #[must_use]
    pub fn user_dir(&self) -> &Path {
        &self.inner.roots[0].dir
    }

    /// Library directories (read-only).
    #[must_use]
    pub fn library_dirs(&self) -> Vec<PathBuf> {
        self.inner
            .roots
            .iter()
            .filter(|r| !r.writable)
            .map(|r| r.dir.clone())
            .collect()
    }

    /// Re-reads every root. Returns whether the index changed (and announces it).
    pub fn rescan(&self) -> bool {
        let _g = lock(&self.inner.scan_lock);
        self.rescan_locked()
    }

    fn rescan_locked(&self) -> bool {
        let meta = lock(&self.inner.meta).clone();
        let mut next: BTreeMap<String, Entry> = BTreeMap::new();
        for root in &self.inner.roots {
            let mut files = Vec::new();
            walk(&root.dir, 0, &mut files);
            for file in files {
                let rel = rel_string(&file, &root.dir);
                let base = slug(&rel);
                let mut id = base.clone();
                let mut n = 2;
                while next.contains_key(&id) {
                    id = format!("{base}-{n}");
                    n += 1;
                }
                let loaded = Template::from_path(&file);
                let stem = file
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let (summary_src, error, template) = match loaded {
                    Ok(t) => (Some(t.clone()), None, Some(t)),
                    Err(e) => (None, Some(e.to_string()), None),
                };
                let mut summary = TemplateSummary {
                    id: id.clone(),
                    name: stem,
                    description: None,
                    descriptions: IndexMap::new(),
                    tags: Vec::new(),
                    protocols: Vec::new(),
                    author: None,
                    source: if root.writable {
                        TemplateSource::User
                    } else {
                        TemplateSource::Library
                    },
                    path: rel,
                    root: root.dir.display().to_string(),
                    writable: root.writable,
                    favorite: meta.favorites.contains(&id),
                    last_used: meta.recent.iter().find(|(i, _)| *i == id).map(|(_, t)| *t),
                    modified: mtime_ms(&file),
                    error,
                };
                if let Some(t) = summary_src {
                    summary.name = t.meta.name;
                    summary.description = t.meta.description;
                    summary.descriptions = t.meta.descriptions;
                    summary.tags = t.meta.tags;
                    summary.author = t.meta.author;
                    summary.protocols = t.layers.into_iter().map(|l| l.protocol).collect();
                }
                next.insert(
                    id,
                    Entry {
                        summary,
                        file,
                        template,
                    },
                );
            }
        }
        let changed = {
            let mut index = self
                .inner
                .index
                .write()
                .unwrap_or_else(PoisonError::into_inner);
            let changed = index.len() != next.len()
                || index.iter().zip(next.iter()).any(|((a, ea), (b, eb))| {
                    a != b || ea.summary.modified != eb.summary.modified || ea.file != eb.file
                });
            *index = next;
            changed
        };
        if changed {
            let _ = self.inner.events.send(ServerEvent::TemplatesChanged);
        }
        changed
    }

    /// Starts the debounced file watcher (idempotent). Directories that do not exist
    /// are skipped.
    ///
    /// # Errors
    /// If the OS watcher cannot be created.
    pub fn watch(&self, debounce: Duration) -> Result<(), notify::Error> {
        let mut slot = lock(&self.inner.watcher);
        if slot.is_some() {
            return Ok(());
        }
        let weak: Weak<Inner> = Arc::downgrade(&self.inner);
        let mut debouncer = new_debouncer(debounce, move |res: DebounceEventResult| {
            if let (Ok(events), Some(inner)) = (res, weak.upgrade()) {
                if !events.is_empty() {
                    let store = TemplateStore { inner };
                    if store.rescan() {
                        tracing::debug!("template library changed; index reloaded");
                    }
                }
            }
        })?;
        for root in &self.inner.roots {
            if root.dir.is_dir() {
                if let Err(e) = debouncer
                    .watcher()
                    .watch(&root.dir, RecursiveMode::Recursive)
                {
                    tracing::warn!("cannot watch {}: {e}", root.dir.display());
                }
            }
        }
        *slot = Some(debouncer);
        Ok(())
    }

    /// Lists templates matching `filter`, sorted by name.
    #[must_use]
    pub fn list(&self, filter: &ListFilter) -> Vec<TemplateSummary> {
        let q = filter.q.as_deref().map(str::to_lowercase);
        let mut v: Vec<TemplateSummary> = read(&self.inner.index)
            .values()
            .map(|e| &e.summary)
            .filter(|s| {
                let text_ok = q.as_deref().is_none_or(|q| {
                    let hay = |x: &str| x.to_lowercase().contains(q);
                    hay(&s.id)
                        || hay(&s.name)
                        || s.description.as_deref().is_some_and(hay)
                        || s.descriptions.values().any(|d| hay(d))
                        || s.tags.iter().any(|t| hay(t))
                        || s.protocols.iter().any(|p| hay(p))
                });
                let tags_ok = filter
                    .tags
                    .iter()
                    .all(|t| s.tags.iter().any(|x| x.eq_ignore_ascii_case(t)));
                text_ok
                    && tags_ok
                    && filter.favorite.is_none_or(|f| s.favorite == f)
                    && filter.source.is_none_or(|src| s.source == src)
            })
            .cloned()
            .collect();
        v.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        });
        v
    }

    /// Summary of one template.
    ///
    /// # Errors
    /// [`StoreError::NotFound`].
    pub fn summary(&self, id: &str) -> Result<TemplateSummary, StoreError> {
        read(&self.inner.index)
            .get(id)
            .map(|e| e.summary.clone())
            .ok_or_else(|| StoreError::NotFound(id.to_owned()))
    }

    /// The parsed template.
    ///
    /// # Errors
    /// [`StoreError::NotFound`], or [`StoreError::Broken`] for a file that does not
    /// parse.
    pub fn get(&self, id: &str) -> Result<(TemplateSummary, Template), StoreError> {
        let index = read(&self.inner.index);
        let e = index
            .get(id)
            .ok_or_else(|| StoreError::NotFound(id.to_owned()))?;
        match &e.template {
            Some(t) => Ok((e.summary.clone(), t.clone())),
            None => Err(StoreError::Broken {
                id: id.to_owned(),
                error: e.summary.error.clone().unwrap_or_default(),
            }),
        }
    }

    /// YAML text of a template (the file itself for YAML files, so comments are
    /// kept; JSON files are converted).
    ///
    /// # Errors
    /// Unknown id or I/O errors.
    pub fn export_yaml(&self, id: &str) -> Result<(TemplateSummary, String), StoreError> {
        let (summary, file) = {
            let index = read(&self.inner.index);
            let e = index
                .get(id)
                .ok_or_else(|| StoreError::NotFound(id.to_owned()))?;
            (e.summary.clone(), e.file.clone())
        };
        let is_json = summary.path.to_ascii_lowercase().ends_with(".json");
        if is_json {
            let (_, t) = self.get(id)?;
            Ok((summary, t.to_yaml_string()?))
        } else {
            let text = std::fs::read_to_string(&file).map_err(io_err(&file))?;
            Ok((summary, text))
        }
    }

    fn unique_file(&self, name: &str) -> PathBuf {
        let dir = self.user_dir();
        let base = slug(name);
        let mut n = 1;
        loop {
            let stem = if n == 1 {
                base.clone()
            } else {
                format!("{base}-{n}")
            };
            let p = dir.join(format!("{stem}.wpg.yaml"));
            let taken = p.exists() || read(&self.inner.index).contains_key(&stem);
            if !taken {
                return p;
            }
            n += 1;
        }
    }

    fn id_of_file(&self, file: &Path) -> Option<String> {
        read(&self.inner.index)
            .iter()
            .find(|(_, e)| e.file == file)
            .map(|(id, _)| id.clone())
    }

    /// Saves a new template into the user directory (file name from `meta.name`).
    /// Returns its id.
    ///
    /// # Errors
    /// Serialisation or I/O errors.
    pub fn create(&self, template: &Template) -> Result<String, StoreError> {
        let _g = lock(&self.inner.scan_lock);
        let file = self.unique_file(&template.meta.name);
        std::fs::write(&file, template.to_yaml_string()?).map_err(io_err(&file))?;
        self.rescan_locked();
        self.id_of_file(&file)
            .ok_or_else(|| StoreError::NotFound(file.display().to_string()))
    }

    /// Parses YAML (or JSON) text and saves it as a new user template.
    ///
    /// # Errors
    /// Parse errors, see [`Self::create`].
    pub fn import_yaml(&self, text: &str) -> Result<String, StoreError> {
        let t = Template::from_yaml_str(text)?;
        self.create(&t)
    }

    /// Replaces a user template.
    ///
    /// # Errors
    /// [`StoreError::NotFound`], [`StoreError::ReadOnly`], I/O errors.
    pub fn update(&self, id: &str, template: &Template) -> Result<(), StoreError> {
        let _g = lock(&self.inner.scan_lock);
        let file = self.writable_file(id)?;
        template.save(&file)?;
        self.rescan_locked();
        Ok(())
    }

    /// Deletes a user template.
    ///
    /// # Errors
    /// [`StoreError::NotFound`], [`StoreError::ReadOnly`], I/O errors.
    pub fn delete(&self, id: &str) -> Result<(), StoreError> {
        let _g = lock(&self.inner.scan_lock);
        let file = self.writable_file(id)?;
        std::fs::remove_file(&file).map_err(io_err(&file))?;
        self.rescan_locked();
        let mut meta = lock(&self.inner.meta);
        meta.favorites.remove(id);
        meta.recent.retain(|(i, _)| i != id);
        self.save_meta(&meta);
        Ok(())
    }

    /// Copies any template (library or user) into the user directory as
    /// `<name> (copy)`. Returns the new id.
    ///
    /// # Errors
    /// See [`Self::get`] and [`Self::create`].
    pub fn duplicate(&self, id: &str) -> Result<String, StoreError> {
        let (_, mut t) = self.get(id)?;
        t.meta.name = format!("{} (copy)", t.meta.name);
        self.create(&t)
    }

    fn writable_file(&self, id: &str) -> Result<PathBuf, StoreError> {
        let index = read(&self.inner.index);
        let e = index
            .get(id)
            .ok_or_else(|| StoreError::NotFound(id.to_owned()))?;
        if !e.summary.writable {
            return Err(StoreError::ReadOnly(id.to_owned()));
        }
        Ok(e.file.clone())
    }

    /// Sets (or toggles, with `None`) the favourite flag. Returns the new value.
    ///
    /// # Errors
    /// [`StoreError::NotFound`].
    pub fn set_favorite(&self, id: &str, value: Option<bool>) -> Result<bool, StoreError> {
        let mut index = self
            .inner
            .index
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        let e = index
            .get_mut(id)
            .ok_or_else(|| StoreError::NotFound(id.to_owned()))?;
        let fav = value.unwrap_or(!e.summary.favorite);
        e.summary.favorite = fav;
        drop(index);
        let mut meta = lock(&self.inner.meta);
        if fav {
            meta.favorites.insert(id.to_owned());
        } else {
            meta.favorites.remove(id);
        }
        self.save_meta(&meta);
        Ok(fav)
    }

    /// Records that a template was used (recent list).
    pub fn mark_used(&self, id: &str) {
        let now = now_ms();
        {
            let mut index = self
                .inner
                .index
                .write()
                .unwrap_or_else(PoisonError::into_inner);
            match index.get_mut(id) {
                Some(e) => e.summary.last_used = Some(now),
                None => return,
            }
        }
        let mut meta = lock(&self.inner.meta);
        meta.recent.retain(|(i, _)| i != id);
        meta.recent.insert(0, (id.to_owned(), now));
        meta.recent.truncate(RECENT_MAX);
        self.save_meta(&meta);
    }

    /// Recently used ids, most recent first.
    #[must_use]
    pub fn recent(&self) -> Vec<String> {
        lock(&self.inner.meta)
            .recent
            .iter()
            .map(|(i, _)| i.clone())
            .collect()
    }

    fn save_meta(&self, meta: &UserMeta) {
        let path = &self.inner.meta_path;
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let res = serde_json::to_vec_pretty(meta)
            .map_err(std::io::Error::other)
            .and_then(|b| std::fs::write(path, b));
        if let Err(e) = res {
            tracing::warn!("cannot save {}: {e}", path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(slug("examples/arp-sweep.wpg.yaml"), "examples-arp-sweep");
        assert_eq!(slug("My Template.WPG.YAML"), "my-template");
        assert_eq!(slug("a//b__c.wpg.json"), "a-b-c");
        assert_eq!(slug("日本語.wpg.yaml"), "template");
    }

    fn store(dir: &Path, lib: &Path) -> TemplateStore {
        TemplateStore::open(
            &dir.join("user"),
            &[lib.to_path_buf()],
            dir.join("meta.json"),
            crate::events::bus(),
        )
        .unwrap()
    }

    #[test]
    fn crud_and_meta() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        std::fs::create_dir_all(lib.join("sub")).unwrap();
        let mut t = Template::new("Lib One");
        t.meta.tags = vec!["arp".into()];
        t.save(lib.join("sub/one.wpg.yaml")).unwrap();
        std::fs::write(lib.join("broken.wpg.yaml"), "schema: 1\nmeta: [").unwrap();
        std::fs::write(lib.join("notes.yaml"), "not a template").unwrap();

        let s = store(tmp.path(), &lib);
        let all = s.list(&ListFilter::default());
        assert_eq!(all.len(), 2, "{all:?}");
        let one = s.summary("sub-one").unwrap();
        assert!(!one.writable);
        assert_eq!(one.source, TemplateSource::Library);
        assert!(s.summary("broken").unwrap().error.is_some());
        assert!(matches!(s.get("broken"), Err(StoreError::Broken { .. })));

        // read-only library
        assert!(matches!(
            s.update("sub-one", &t),
            Err(StoreError::ReadOnly(_))
        ));
        assert!(matches!(s.delete("sub-one"), Err(StoreError::ReadOnly(_))));

        // duplicate into the user dir
        let copy = s.duplicate("sub-one").unwrap();
        assert_eq!(copy, "lib-one-copy");
        let (sum, tt) = s.get(&copy).unwrap();
        assert!(sum.writable);
        assert_eq!(tt.meta.name, "Lib One (copy)");

        // create twice with the same name → unique ids
        let a = s.create(&Template::new("Same")).unwrap();
        let b = s.create(&Template::new("Same")).unwrap();
        assert_ne!(a, b);

        // update / search / delete
        let mut t2 = Template::new("Renamed");
        t2.meta.tags = vec!["x".into()];
        s.update(&a, &t2).unwrap();
        assert_eq!(s.get(&a).unwrap().1.meta.name, "Renamed");
        let found = s.list(&ListFilter {
            q: Some("renam".into()),
            ..ListFilter::default()
        });
        assert_eq!(found.len(), 1);
        let tagged = s.list(&ListFilter {
            tags: vec!["ARP".into()],
            ..ListFilter::default()
        });
        assert_eq!(tagged.len(), 2); // original + copy
        s.delete(&a).unwrap();
        assert!(matches!(s.get(&a), Err(StoreError::NotFound(_))));

        // favourites + recent persist
        assert!(s.set_favorite("sub-one", None).unwrap());
        s.mark_used(&b);
        drop(s);
        let s = store(tmp.path(), &lib);
        assert!(s.summary("sub-one").unwrap().favorite);
        assert_eq!(s.recent(), vec![b.clone()]);
        assert!(s.summary(&b).unwrap().last_used.is_some());

        // import / export
        let id = s
            .import_yaml("schema: 1\nmeta:\n  name: Imported\n")
            .unwrap();
        let (_, yaml) = s.export_yaml(&id).unwrap();
        assert!(yaml.contains("Imported"));
    }

    #[test]
    fn watcher_picks_up_external_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        std::fs::create_dir_all(&lib).unwrap();
        let bus = crate::events::bus();
        let mut rx = bus.subscribe();
        let s = TemplateStore::open(
            &tmp.path().join("user"),
            std::slice::from_ref(&lib),
            tmp.path().join("meta.json"),
            bus,
        )
        .unwrap();
        s.watch(Duration::from_millis(100)).unwrap();
        Template::new("Hot").save(lib.join("hot.wpg.yaml")).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while s.summary("hot").is_err() {
            assert!(std::time::Instant::now() < deadline, "watcher did not fire");
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(matches!(rx.try_recv(), Ok(ServerEvent::TemplatesChanged)));
    }
}
