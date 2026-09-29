//! Packet builder (TODO §2.5 画面構成 2, M4 phase 2). **Phase 1 ships only this
//! placeholder route**; the planned layout of the module is:
//!
//! | module | role |
//! |---|---|
//! | `state.rs` | Document model: a `wpg_template::Template` as the single source of truth, selection, Undo/Redo (command stack), local draft autosave (`localStorage`, key per template id) |
//! | `palette.rs` | Left: protocol palette from `GET /api/protocols` (search, category, favourites) |
//! | `dnd.rs` | Thin wrapper over HTML5 drag & drop (`dragstart` / `dragover` / `drop`, `DataTransfer`), plus the keyboard equivalents (Space to pick up, arrows to move, Enter to drop; §2.5 a11y) |
//! | `stack.rs` | Centre: layer stack (add / reorder / delete by D&D or keyboard), next-header auto-suggestions |
//! | `fields.rs` | Right: field editor per `wpg_core::ir::FieldType` (uint, mac, ipv4/6, nsap, enum, flags, tlv, list …) with Auto/Manual toggles and generator settings (range / list / random / counter), human notation parsing with inline errors |
//! | `hex.rs` | Bottom: self-made virtualised hex view (16 B rows, only visible rows in the DOM; §5 risk "Hex 9000B"), byte-range highlight, direct editing that re-parses via `wpg_core::parse` |
//! | `tree.rs` | Bottom: field tree from `wpg_core::build::Built::layout` (`LayerLayout` / `FieldLayout` bit offsets), two-way highlight with the hex view |
//! | `preview.rs` | In-browser build with `wpg-core` + `wpg-protocols` compiled to WASM, cross-checked against `POST /api/render` |
//!
//! Routes: `/builder` (new), `/builder?template=<id>` (open; "Open in builder" on
//! the templates screen already links there).

use leptos::prelude::*;
use leptos_router::hooks::use_query_map;

use crate::components::IconName;
use crate::i18n::use_i18n;
use crate::pages::ComingLater;

/// `/builder` placeholder.
#[component]
pub(crate) fn BuilderPage() -> impl IntoView {
    let i18n = use_i18n();
    let query = use_query_map();
    let text = move || match query.with(|q| q.get("template")) {
        Some(id) => i18n.ta("ui-builder-placeholder-template", &[("id", id)]),
        None => i18n.t("ui-builder-placeholder"),
    };
    view! {
        <ComingLater
            title=Signal::derive(move || i18n.t("ui-nav-builder"))
            text=Signal::derive(text)
            icon=IconName::Builder
        />
    }
}
