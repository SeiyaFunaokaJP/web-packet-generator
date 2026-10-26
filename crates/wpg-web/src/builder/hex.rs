//! Virtualised hex dump (spec §5 "Hex 9000B"): only the visible 16-byte rows are in
//! the DOM (fixed row height + top / bottom spacers), so a 9000-byte frame scrolls
//! smoothly. Hovering / clicking a byte highlights the owning field (and selects it
//! in the tree + editor); typing hex over a byte reverse-applies as a Manual override.

use leptos::prelude::*;

use super::preview::{field_at_byte, range_at_byte, reverse_apply, PreviewCtx};
use super::state::{edit, Builder};
use crate::i18n::tr;

/// Row height in pixels (must match the inline row style).
const ROW_H: f64 = 22.0;
/// Bytes per row.
const COLS: usize = 16;
/// Extra rows rendered above / below the viewport.
const OVERSCAN: usize = 4;
/// Visible viewport height in pixels.
const VIEW_H: f64 = 340.0;

/// Non-negative `f64` → `usize` (floored, saturating), for scroll maths.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn usize_from(f: f64) -> usize {
    if f.is_finite() && f > 0.0 {
        f.floor() as usize
    } else {
        0
    }
}

fn overlaps(byte: usize, range: Option<(usize, usize)>) -> bool {
    range.is_some_and(|(off, len)| {
        let (b0, b1) = (byte * 8, byte * 8 + 8);
        off < b1 && b0 < off + len
    })
}

#[component]
pub(crate) fn HexView() -> impl IntoView {
    let builder = Builder::use_ctx();
    let pv = PreviewCtx::use_ctx();
    let scroll_top = RwSignal::new(0.0f64);
    let caret = RwSignal::new(0usize);
    // Toggles between the two nibbles of the byte under the caret.
    let nibble_hi = RwSignal::new(true);
    // Bytes typed since the last build landed: fast typing must not read the
    // stale preview (the rebuild is debounced), so edits are overlaid until then.
    let overlay = RwSignal::new(Vec::<(usize, u8)>::new());
    Effect::new(move |_| {
        pv.preview.track();
        overlay.set(Vec::new());
    });

    let total_rows = move || pv.preview.with(|p| p.bytes.len().div_ceil(COLS)).max(1);

    let rows_view = move || {
        let p = pv.preview.get();
        let len = p.bytes.len();
        if len == 0 {
            return view! { <p class="p-3 text-xs text-slate-500">{tr("ui-bld-no-bytes")}</p> }
                .into_any();
        }
        let rows = len.div_ceil(COLS);
        let first = usize_from(scroll_top.get() / ROW_H).saturating_sub(OVERSCAN);
        let visible = usize_from((VIEW_H / ROW_H).ceil()) + OVERSCAN * 2;
        let last = (first + visible).min(rows);
        let sel = builder.range.get();
        let hov = builder.hover.get();

        let byte_cell = move |b: usize, val: u8| {
            let active = overlaps(b, sel) || overlaps(b, hov);
            let is_caret = caret.get() == b;
            let cls = move || {
                let base = "inline-block w-[1.35rem] cursor-pointer rounded px-0.5 text-center";
                if is_caret {
                    format!("{base} bg-sky-600 text-white")
                } else if active {
                    format!("{base} bg-sky-200 text-sky-900 dark:bg-sky-500/40 dark:text-sky-100")
                } else {
                    format!("{base} hover:bg-slate-200 dark:hover:bg-slate-700")
                }
            };
            view! {
                <span
                    id=format!("hxb-{b}")
                    tabindex="0"
                    class=cls
                    on:mouseenter=move |_| builder.hover.set(range_at_byte(&pv.preview.get_untracked().layout, b))
                    on:mouseleave=move |_| builder.hover.set(None)
                    on:focus=move |_| { caret.set(b); nibble_hi.set(true); builder.range.set(range_at_byte(&pv.preview.get_untracked().layout, b)); }
                    on:click=move |_| select_byte(builder, pv, b)
                    on:keydown=move |ev| hex_keydown(builder, pv, (caret, nibble_hi, overlay), b, len, &ev)
                >
                    {format!("{val:02x}")}
                </span>
            }
        };

        let ascii = move |bytes: &[u8], row_start: usize, n: usize| {
            (0..n)
                .map(|k| {
                    let c = bytes[row_start + k];
                    let ch = if (0x20..0x7f).contains(&c) { c as char } else { '.' };
                    let b = row_start + k;
                    let active = overlaps(b, sel) || overlaps(b, hov);
                    view! {
                        <span class=if active { "bg-sky-200 dark:bg-sky-500/40" } else { "" }>{ch.to_string()}</span>
                    }
                })
                .collect_view()
        };

        let body = (first..last)
            .map(|r| {
                let row_start = r * COLS;
                let n = (len - row_start).min(COLS);
                let bytes = p.bytes.clone();
                view! {
                    <div class="flex items-center gap-3 font-mono text-xs leading-none" style=format!("height:{ROW_H}px")>
                        <span class="w-14 shrink-0 text-right text-slate-400 tabular-nums">{format!("{row_start:04x}")}</span>
                        <span class="flex gap-px">
                            {(0..n).map(|k| byte_cell(row_start + k, p.bytes[row_start + k])).collect_view()}
                        </span>
                        <span class="ml-2 text-slate-500">{ascii(&bytes, row_start, n)}</span>
                    </div>
                }
            })
            .collect_view();

        view! {
            <div style=format!("height:{}px", first as f64 * ROW_H)></div>
            {body}
            <div style=format!("height:{}px", (rows.saturating_sub(last)) as f64 * ROW_H)></div>
        }
        .into_any()
    };

    view! {
        <section aria-label=tr("ui-bld-hex") class="flex h-full min-h-0 flex-col">
            <div class="mb-1 flex items-center justify-between">
                <h2 class="text-sm font-semibold">{tr("ui-bld-hex")}</h2>
                <span class="text-[11px] text-slate-500">{move || pv.preview.with(|p| format!("{} B", p.bytes.len()))}</span>
            </div>
            <div
                class="min-h-0 flex-1 overflow-y-auto rounded-md border border-slate-200 bg-white p-2 dark:border-slate-800 dark:bg-slate-950"
                style=format!("max-height:{VIEW_H}px")
                on:scroll=move |ev| {
                    if let Some(el) = ev.target().and_then(|t| wasm_element(&t)) {
                        scroll_top.set(f64::from(el.scroll_top()));
                    }
                }
            >
                <div style=move || format!("min-height:{}px", total_rows() as f64 * ROW_H)>
                    {rows_view}
                </div>
            </div>
        </section>
    }
}

/// Selects the field owning byte `b` (editor + tree + hex highlight).
fn select_byte(builder: Builder, pv: PreviewCtx, b: usize) {
    let layout = pv.preview.get_untracked().layout;
    if let Some((layer, field, off, len)) = field_at_byte(&layout, b) {
        builder.select_layer(Some(layer));
        builder.sel_field.set(Some(field));
        builder.range.set(Some((off, len)));
    }
}

/// Hex typing / caret navigation on the byte under the caret.
fn hex_keydown(
    builder: Builder,
    pv: PreviewCtx,
    (caret, nibble_hi, overlay): Caret,
    b: usize,
    len: usize,
    ev: &leptos::ev::KeyboardEvent,
) {
    let key = ev.key();
    let mv = |to: usize| {
        ev.prevent_default();
        let to = to.min(len - 1);
        caret.set(to);
        nibble_hi.set(true);
        crate::dom::focus_id(&format!("hxb-{to}"));
    };
    match key.as_str() {
        "ArrowRight" => return mv(b + 1),
        "ArrowLeft" => return mv(b.saturating_sub(1)),
        "ArrowDown" => return mv(b + COLS),
        "ArrowUp" => return mv(b.saturating_sub(COLS)),
        _ => {}
    }
    let Some(digit) = key.chars().next().and_then(|c| c.to_digit(16)) else {
        return;
    };
    if key.len() != 1 {
        return;
    }
    ev.prevent_default();
    let p = pv.preview.get_untracked();
    let mut bytes = p.bytes.clone();
    overlay.with_untracked(|o| {
        for &(i, v) in o {
            if let Some(x) = bytes.get_mut(i) {
                *x = v;
            }
        }
    });
    let Some(&old) = bytes.get(b) else { return };
    let new_byte = if nibble_hi.get_untracked() {
        (u8::try_from(digit).unwrap_or(0) << 4) | (old & 0x0f)
    } else {
        (old & 0xf0) | u8::try_from(digit).unwrap_or(0)
    };
    if let Some(rev) = reverse_apply(&bytes, &p.layout, b, new_byte) {
        overlay.update(|o| {
            o.retain(|(i, _)| *i != b);
            o.push((b, new_byte));
        });
        let (layer, field, input) = (rev.layer, rev.field, rev.input);
        builder.edit(true, move |t| edit::set_field(t, layer, &field, input));
    }
    if nibble_hi.get_untracked() {
        nibble_hi.set(false);
    } else {
        nibble_hi.set(true);
        let to = (b + 1).min(len - 1);
        caret.set(to);
    }
    // The live rebuild re-renders the row and drops focus; restore it on the caret
    // byte once the new DOM is in place so multi-nibble typing keeps working.
    let target = caret.get_untracked();
    set_timeout(
        move || crate::dom::focus_id(&format!("hxb-{target}")),
        std::time::Duration::from_millis(80),
    );
}

/// Caret position, nibble toggle and typed-byte overlay of the hex view.
type Caret = (RwSignal<usize>, RwSignal<bool>, RwSignal<Vec<(usize, u8)>>);

/// `EventTarget` → `Element` (scroll offset), only on wasm.
fn wasm_element(t: &web_sys::EventTarget) -> Option<web_sys::Element> {
    use wasm_bindgen::JsCast;
    t.clone().dyn_into::<web_sys::Element>().ok()
}
