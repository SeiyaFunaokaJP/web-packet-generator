//! Editable document model of the builder (TODO §2.5).
//!
//! The single source of truth is a [`wpg_template::Template`] held in a Leptos
//! signal. Every mutation goes through [`Builder::edit`], which snapshots the
//! document for Undo/Redo (rapid edits coalesce into one step), marks the document
//! dirty and autosaves a draft to `localStorage` (keyed by the template id, or
//! `"new"`). The model converts to / from the JSON the REST API expects.
//!
//! The pure pieces (history, JSON round trip, the layer / field edit operations)
//! are unit tested natively; the reactive wrapper only wires them to signals.

use leptos::prelude::*;
use serde_json::Value;
use wpg_template::{FieldInput, GeneratorSpec, Template, TemplateLayer};

/// Coalesce edits that land within this many milliseconds into one Undo step.
const COALESCE_MS: f64 = 450.0;
/// Cap the Undo history so a long session does not grow without bound.
const MAX_HISTORY: usize = 200;

/// `localStorage` key of the autosaved draft for a template id (`None` = new).
#[must_use]
pub(crate) fn draft_key(id: Option<&str>) -> String {
    format!("wpg.builder.draft.{}", id.unwrap_or("new"))
}

/// The template as the JSON object the API accepts (`POST` / `PUT /api/templates`).
#[must_use]
pub(crate) fn template_to_value(t: &Template) -> Value {
    serde_json::to_value(t).unwrap_or(Value::Null)
}

/// Parses a template JSON object (the shape of `TemplateDetail.template`).
///
/// # Errors
/// A message if the value is not a valid template.
pub(crate) fn template_from_value(v: &Value) -> Result<Template, String> {
    Template::from_json_str(&v.to_string()).map_err(|e| e.to_string())
}

/// A blank template used for a fresh builder session.
#[must_use]
pub(crate) fn blank_template() -> Template {
    Template::new(String::new())
}

// ---------------------------------------------------------------------------
// Undo / redo history
// ---------------------------------------------------------------------------

/// A bounded Undo/Redo stack over whole-document snapshots.
#[derive(Debug, Clone, Default)]
pub(crate) struct History {
    past: Vec<Template>,
    future: Vec<Template>,
    last_ms: f64,
    coalescing: bool,
}

impl History {
    /// Records `current` before a mutation. Edits closer than [`COALESCE_MS`] to the
    /// previous coalescing edit collapse into a single Undo step.
    pub(crate) fn snapshot(&mut self, current: &Template, now: f64, coalesce: bool) {
        if coalesce && self.coalescing && now - self.last_ms < COALESCE_MS && !self.past.is_empty()
        {
            self.last_ms = now;
            return;
        }
        self.past.push(current.clone());
        if self.past.len() > MAX_HISTORY {
            self.past.remove(0);
        }
        self.future.clear();
        self.last_ms = now;
        self.coalescing = coalesce;
    }

    /// Undoes one step, given the live document; returns the state to restore.
    pub(crate) fn undo(&mut self, current: &Template) -> Option<Template> {
        let prev = self.past.pop()?;
        self.future.push(current.clone());
        self.coalescing = false;
        Some(prev)
    }

    /// Redoes one step.
    pub(crate) fn redo(&mut self, current: &Template) -> Option<Template> {
        let next = self.future.pop()?;
        self.past.push(current.clone());
        self.coalescing = false;
        Some(next)
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Pure edit operations on the template
// ---------------------------------------------------------------------------

/// Layer / field mutations of the document model (pure; unit tested).
pub(crate) mod edit {
    use super::{FieldInput, GeneratorSpec, Template, TemplateLayer};

    /// Inserts a new layer of `protocol` at `at` (clamped; `None` = append).
    pub(crate) fn add_layer(t: &mut Template, protocol: &str, at: Option<usize>) -> usize {
        let layer = TemplateLayer {
            protocol: protocol.to_owned(),
            ..TemplateLayer::default()
        };
        let idx = at.unwrap_or(t.layers.len()).min(t.layers.len());
        t.layers.insert(idx, layer);
        idx
    }

    /// Removes layer `i` if it exists.
    pub(crate) fn remove_layer(t: &mut Template, i: usize) {
        if i < t.layers.len() {
            t.layers.remove(i);
        }
    }

    /// Moves layer `from` to index `to` (both clamped).
    pub(crate) fn move_layer(t: &mut Template, from: usize, to: usize) {
        if from >= t.layers.len() || to >= t.layers.len() || from == to {
            return;
        }
        let layer = t.layers.remove(from);
        t.layers.insert(to, layer);
    }

    /// Duplicates layer `i` right after it; returns the new index.
    pub(crate) fn duplicate_layer(t: &mut Template, i: usize) -> Option<usize> {
        let clone = t.layers.get(i)?.clone();
        t.layers.insert(i + 1, clone);
        Some(i + 1)
    }

    /// Sets a Manual field value (`fields[name] = input`).
    pub(crate) fn set_field(t: &mut Template, layer: usize, name: &str, input: FieldInput) {
        if let Some(l) = t.layers.get_mut(layer) {
            l.fields.insert(name.to_owned(), input);
        }
    }

    /// Removes a Manual field value (back to Auto / default).
    pub(crate) fn clear_field(t: &mut Template, layer: usize, name: &str) {
        if let Some(l) = t.layers.get_mut(layer) {
            l.fields.shift_remove(name);
        }
    }

    /// Sets a field generator.
    pub(crate) fn set_generator(t: &mut Template, layer: usize, name: &str, spec: GeneratorSpec) {
        if let Some(l) = t.layers.get_mut(layer) {
            l.generators.insert(name.to_owned(), spec);
        }
    }

    /// Removes a field generator.
    pub(crate) fn clear_generator(t: &mut Template, layer: usize, name: &str) {
        if let Some(l) = t.layers.get_mut(layer) {
            l.generators.shift_remove(name);
        }
    }
}

// ---------------------------------------------------------------------------
// Reactive wrapper
// ---------------------------------------------------------------------------

/// The reactive builder document (a context shared by every panel).
#[derive(Clone, Copy)]
pub(crate) struct Builder {
    /// The live template.
    pub(crate) template: RwSignal<Template>,
    /// Undo / redo history.
    pub(crate) history: RwSignal<History>,
    /// Unsaved changes since the last save / load.
    pub(crate) dirty: RwSignal<bool>,
    /// Server id when the template was loaded from / saved to the server.
    pub(crate) server_id: RwSignal<Option<String>>,
    /// Selected layer index (drives the field editor and highlighting).
    pub(crate) sel_layer: RwSignal<Option<usize>>,
    /// Selected field name within the selected layer.
    pub(crate) sel_field: RwSignal<Option<String>>,
    /// Selected byte range as `(bit_offset, bit_len)` (hex ⇄ tree ⇄ editor link).
    pub(crate) range: RwSignal<Option<(usize, usize)>>,
    /// Hovered byte range (transient highlight).
    pub(crate) hover: RwSignal<Option<(usize, usize)>>,
    /// Stream index shown in the preview.
    pub(crate) index: RwSignal<u64>,
    /// Whether the preview recomputes live on every edit.
    pub(crate) auto_recompute: RwSignal<bool>,
    /// Palette favourites (protocol ids).
    pub(crate) favorites: RwSignal<Vec<String>>,
}

impl Builder {
    /// Creates the document, restoring a draft for `id` when one exists.
    pub(crate) fn new(id: Option<String>) -> Self {
        let restored = id
            .as_deref()
            .or(Some("new"))
            .and_then(|k| crate::prefs::load(&draft_key(id.as_deref())).map(|s| (k, s)))
            .and_then(|(_, s)| serde_json::from_str::<Value>(&s).ok())
            .and_then(|v| template_from_value(&v).ok());
        let template = restored.unwrap_or_else(blank_template);
        let favorites = crate::prefs::load("wpg.builder.favorites")
            .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
            .unwrap_or_default();
        let sel = (!template.layers.is_empty()).then_some(0);
        Self {
            template: RwSignal::new(template),
            history: RwSignal::new(History::default()),
            dirty: RwSignal::new(false),
            server_id: RwSignal::new(id),
            sel_layer: RwSignal::new(sel),
            sel_field: RwSignal::new(None),
            range: RwSignal::new(None),
            hover: RwSignal::new(None),
            index: RwSignal::new(0),
            auto_recompute: RwSignal::new(true),
            favorites: RwSignal::new(favorites),
        }
    }

    /// The [`Builder`] context.
    pub(crate) fn use_ctx() -> Builder {
        expect_context::<Builder>()
    }

    /// Applies a mutation, recording it for Undo (coalesced when `coalesce`), marking
    /// the document dirty and autosaving the draft.
    pub(crate) fn edit(self, coalesce: bool, f: impl FnOnce(&mut Template)) {
        let now = crate::dom::now_ms();
        let before = self.template.get_untracked();
        self.history.update(|h| h.snapshot(&before, now, coalesce));
        self.template.update(f);
        self.dirty.set(true);
        self.save_draft();
    }

    /// Replaces the whole document (server load / import); clears history and dirty.
    pub(crate) fn replace(self, template: Template, server_id: Option<String>) {
        let sel = (!template.layers.is_empty()).then_some(0);
        self.template.set(template);
        self.history.set(History::default());
        self.server_id.set(server_id);
        self.sel_layer.set(sel);
        self.sel_field.set(None);
        self.range.set(None);
        self.dirty.set(false);
        self.index.set(0);
        self.save_draft();
    }

    /// Undoes one step.
    pub(crate) fn undo(self) {
        let current = self.template.get_untracked();
        if let Some(prev) = self.history.try_update(|h| h.undo(&current)).flatten() {
            self.template.set(prev);
            self.dirty.set(true);
            self.clamp_selection();
            self.save_draft();
        }
    }

    /// Redoes one step.
    pub(crate) fn redo(self) {
        let current = self.template.get_untracked();
        if let Some(next) = self.history.try_update(|h| h.redo(&current)).flatten() {
            self.template.set(next);
            self.dirty.set(true);
            self.clamp_selection();
            self.save_draft();
        }
    }

    /// Keeps the selection inside the current layer count.
    fn clamp_selection(self) {
        let n = self.template.with_untracked(|t| t.layers.len());
        self.sel_layer.update(|s| {
            if let Some(i) = *s {
                if i >= n {
                    *s = n.checked_sub(1);
                }
            } else if n > 0 {
                *s = Some(0);
            }
        });
    }

    /// Selects a layer (and clears the field selection).
    pub(crate) fn select_layer(self, i: Option<usize>) {
        self.sel_layer.set(i);
        self.sel_field.set(None);
        self.range.set(None);
    }

    /// Writes the current draft to `localStorage`.
    pub(crate) fn save_draft(self) {
        let key = draft_key(self.server_id.get_untracked().as_deref());
        let json = self
            .template
            .with_untracked(|t| serde_json::to_string(&template_to_value(t)).unwrap_or_default());
        crate::prefs::store(&key, &json);
    }

    /// Toggles a palette favourite and persists the set.
    pub(crate) fn toggle_favorite(self, id: &str) {
        let id = id.to_owned();
        self.favorites.update(|f| {
            if let Some(pos) = f.iter().position(|x| *x == id) {
                f.remove(pos);
            } else {
                f.push(id);
            }
        });
        let json = self
            .favorites
            .with_untracked(|f| serde_json::to_string(f).unwrap_or_default());
        crate::prefs::store("wpg.builder.favorites", &json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wpg_template::FieldInput;

    fn eth() -> Template {
        let mut t = Template::new("t");
        edit::add_layer(&mut t, "ethernet2", None);
        t
    }

    #[test]
    fn json_round_trip() {
        let mut t = eth();
        edit::add_layer(&mut t, "ipv4", None);
        edit::set_field(
            &mut t,
            0,
            "dst",
            FieldInput::Str("ff:ff:ff:ff:ff:ff".into()),
        );
        let v = template_to_value(&t);
        let back = template_from_value(&v).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn layer_operations() {
        let mut t = Template::new("t");
        edit::add_layer(&mut t, "ethernet2", None);
        edit::add_layer(&mut t, "ipv4", None);
        edit::add_layer(&mut t, "udp", None);
        assert_eq!(
            t.layers
                .iter()
                .map(|l| l.protocol.as_str())
                .collect::<Vec<_>>(),
            ["ethernet2", "ipv4", "udp"]
        );
        edit::move_layer(&mut t, 2, 0);
        assert_eq!(t.layers[0].protocol, "udp");
        let n = edit::duplicate_layer(&mut t, 0).unwrap();
        assert_eq!(n, 1);
        assert_eq!(t.layers[1].protocol, "udp");
        edit::remove_layer(&mut t, 1);
        assert_eq!(t.layers.len(), 3);
        // out-of-range ops are no-ops
        edit::remove_layer(&mut t, 99);
        edit::move_layer(&mut t, 99, 0);
        assert_eq!(t.layers.len(), 3);
    }

    #[test]
    fn field_and_generator_edits() {
        let mut t = eth();
        edit::set_field(
            &mut t,
            0,
            "src",
            FieldInput::Str("02:00:00:00:00:01".into()),
        );
        assert!(t.layers[0].fields.contains_key("src"));
        edit::clear_field(&mut t, 0, "src");
        assert!(!t.layers[0].fields.contains_key("src"));
        edit::set_generator(&mut t, 0, "src", GeneratorSpec::Text("00:11:22:*".into()));
        assert!(t.layers[0].generators.contains_key("src"));
        edit::clear_generator(&mut t, 0, "src");
        assert!(t.layers[0].generators.is_empty());
    }

    #[test]
    fn history_coalesces_and_reverts() {
        let mut h = History::default();
        let v0 = Template::new("v0");
        let mut v1 = v0.clone();
        edit::add_layer(&mut v1, "ethernet2", None);
        // Two rapid edits coalesce into one undo step.
        h.snapshot(&v0, 1000.0, true);
        h.snapshot(&v1, 1100.0, true);
        assert!(h.can_undo());
        let restored = h.undo(&v1).unwrap();
        assert_eq!(restored, v0);
        assert!(h.can_redo());
        assert_eq!(h.redo(&v0).unwrap(), v1);
        // A non-coalescing edit is always its own step.
        let mut h = History::default();
        h.snapshot(&v0, 1000.0, false);
        h.snapshot(&v1, 1050.0, false);
        assert_eq!(h.undo(&v1).unwrap(), v1);
        assert_eq!(h.undo(&v1).unwrap(), v0);
    }

    #[test]
    fn draft_keys() {
        assert_eq!(draft_key(None), "wpg.builder.draft.new");
        assert_eq!(draft_key(Some("abc")), "wpg.builder.draft.abc");
    }
}
