//! HTML5 drag & drop for the layer stack, plus the keyboard equivalents that make
//! it operable without a pointer (TODO §2.5 a11y): select a row, `Alt`+`↑`/`↓` to
//! move it, `Delete` to remove it, `Enter` on a palette item to append. A shared
//! [`DragCtx`] carries what is being dragged and where; the stack draws a drop
//! indicator and announces moves through an `aria-live` region.

use leptos::prelude::*;

/// What a drag is carrying.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Drag {
    /// A protocol dragged from the palette (inserted as a new layer).
    New(String),
    /// An existing layer being reordered (its current index).
    Move(usize),
}

/// Drag & drop state shared by the palette and the stack (context).
#[derive(Clone, Copy)]
pub(crate) struct DragCtx {
    /// The active drag, if any.
    pub(crate) dragging: RwSignal<Option<Drag>>,
    /// Insertion slot the pointer is over (0..=len), for the drop indicator.
    pub(crate) over: RwSignal<Option<usize>>,
    /// `aria-live` message for the last move / add.
    pub(crate) announce: RwSignal<String>,
}

impl DragCtx {
    pub(crate) fn new() -> Self {
        Self {
            dragging: RwSignal::new(None),
            over: RwSignal::new(None),
            announce: RwSignal::new(String::new()),
        }
    }

    /// The [`DragCtx`] context.
    pub(crate) fn use_ctx() -> DragCtx {
        expect_context::<DragCtx>()
    }

    /// Clears the current drag.
    pub(crate) fn clear(self) {
        self.dragging.set(None);
        self.over.set(None);
    }

    pub(crate) fn say(self, msg: String) {
        self.announce.set(msg);
    }
}

/// The destination index of a drop of `dragging` before slot `slot` in a stack of
/// `len` layers. A move to a slot after itself accounts for the removed row.
#[must_use]
pub(crate) fn drop_target(dragging: &Drag, slot: usize, len: usize) -> Option<usize> {
    let slot = slot.min(len);
    match dragging {
        Drag::New(_) => Some(slot),
        Drag::Move(from) => {
            let to = if slot > *from { slot - 1 } else { slot };
            (to != *from).then_some(to.min(len.saturating_sub(1)))
        }
    }
}

/// The `aria-live` region for drag / keyboard announcements.
#[component]
pub(crate) fn DndAnnounce() -> impl IntoView {
    let dnd = DragCtx::use_ctx();
    view! {
        <div class="sr-only" aria-live="assertive" aria-atomic="true">
            {move || dnd.announce.get()}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_targets() {
        // New item drops exactly at the slot.
        assert_eq!(drop_target(&Drag::New("ipv4".into()), 2, 3), Some(2));
        assert_eq!(drop_target(&Drag::New("ipv4".into()), 9, 3), Some(3));
        // Moving row 0 to slot 0 or 1 is a no-op (same place).
        assert_eq!(drop_target(&Drag::Move(0), 0, 3), None);
        assert_eq!(drop_target(&Drag::Move(0), 1, 3), None);
        // Moving row 0 to the end.
        assert_eq!(drop_target(&Drag::Move(0), 3, 3), Some(2));
        // Moving row 2 to the front.
        assert_eq!(drop_target(&Drag::Move(2), 0, 3), Some(0));
    }
}
