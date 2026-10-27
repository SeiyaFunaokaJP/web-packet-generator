//! Browser glue of the terminal tabs: one xterm.js instance and one
//! `/api/terminal/ws` WebSocket per tab. The JS objects live in a thread-local
//! registry keyed by tab id (closures look tabs up by id, so nothing forms a
//! reference cycle); the reactive state is in [`super::TermStore`].
//!
//! Natively (unit tests, clippy) every function is an inert stub.

use leptos::prelude::{GetUntracked, Set};

use super::{is_multiline, PendingPaste, TermStore};

/// Pastes `text` into tab `id`, asking first (via [`TermStore::paste`]) when it
/// spans several lines and the setting says so, or when it comes from a file.
pub(crate) fn request_paste(store: TermStore, id: u32, text: String, file: Option<String>) {
    if text.is_empty() {
        return;
    }
    let s = store.settings.get_untracked();
    if file.is_some() || (s.confirm_paste && is_multiline(&text)) {
        store.paste.set(Some(PendingPaste {
            tab: id,
            text,
            file,
        }));
    } else {
        send_text(store, id, &text);
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) use imp::*;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use stub::*;

#[cfg(not(target_arch = "wasm32"))]
mod stub {
    use super::super::{ConnectParams, TermSettings, TermStore};
    use crate::i18n::I18n;

    pub(crate) fn open(_: TermStore, _: I18n, _: ConnectParams) {}
    pub(crate) fn mount(_: u32) {}
    pub(crate) fn fit(_: u32) {}
    pub(crate) fn focus(_: u32) {}
    pub(crate) fn send_json(_: u32, _: &serde_json::Value) {}
    pub(crate) fn disconnect(_: u32) {}
    pub(crate) fn reconnect(_: u32) {}
    pub(crate) fn close(store: TermStore, id: u32) {
        store.remove(id);
    }
    pub(crate) fn send_text(_: TermStore, _: u32, _: &str) {}
    pub(crate) fn paste_clipboard(_: TermStore, _: u32) {}
    pub(crate) fn copy_selection(_: u32) {}
    pub(crate) fn clear(_: u32) {}
    pub(crate) fn find(_: u32, _: &str, _: bool) -> bool {
        false
    }
    pub(crate) fn scrollback_text(_: u32) -> String {
        String::new()
    }
    pub(crate) fn apply_settings(_: &TermSettings) {}
    pub(crate) fn apply_theme(_: bool) {}
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::any::Any;
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::rc::Rc;

    use js_sys::{Object, Reflect, Uint8Array};
    use leptos::prelude::*;
    use serde_json::json;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_sys::{BinaryType, HtmlElement, MessageEvent, WebSocket};

    use super::super::xterm::{self, set, FitAddon, SearchAddon, XTerm};
    use super::super::{
        closed_key, error_key, split_lines, ConnectParams, Phase, ServerMsg, TabState, TermError,
        TermSettings, TermStore,
    };
    use crate::i18n::I18n;

    const FONT: &str = "ui-monospace, \"Cascadia Mono\", \"SFMono-Regular\", Menlo, Consolas, \"Noto Sans Mono\", \"Noto Sans Mono CJK JP\", monospace";

    struct Tab {
        term: XTerm,
        fit: FitAddon,
        search: SearchAddon,
        el: HtmlElement,
        opened: Cell<bool>,
        connect_pending: Cell<bool>,
        /// Keystrokes go out only while connected.
        connected: Cell<bool>,
        ws: RefCell<Option<WebSocket>>,
        params: RefCell<ConnectParams>,
        store: TermStore,
        i18n: I18n,
        /// Closures of the terminal / element listeners.
        keep: RefCell<Vec<Box<dyn Any>>>,
        /// Closures of the current WebSocket.
        ws_keep: RefCell<Vec<Box<dyn Any>>>,
        observer: RefCell<Option<web_sys::ResizeObserver>>,
    }

    thread_local! {
        static TABS: RefCell<HashMap<u32, Rc<Tab>>> = RefCell::new(HashMap::new());
    }

    fn get(id: u32) -> Option<Rc<Tab>> {
        TABS.with(|t| t.borrow().get(&id).cloned())
    }

    fn dark() -> bool {
        web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.document_element())
            .is_some_and(|e| e.class_list().contains("dark"))
    }

    fn theme(dark: bool) -> Object {
        let t = Object::new();
        let (bg, fg, cursor, sel) = if dark {
            ("#020617", "#e2e8f0", "#38bdf8", "#334155")
        } else {
            ("#ffffff", "#0f172a", "#0369a1", "#bae6fd")
        };
        set(&t, "background", &bg.into());
        set(&t, "foreground", &fg.into());
        set(&t, "cursor", &cursor.into());
        set(&t, "selectionBackground", &sel.into());
        t
    }

    fn options(s: &TermSettings) -> Object {
        let o = Object::new();
        set(&o, "fontSize", &JsValue::from(s.font_size));
        set(&o, "fontFamily", &FONT.into());
        set(&o, "scrollback", &JsValue::from(s.scrollback));
        set(&o, "cursorBlink", &JsValue::TRUE);
        set(&o, "allowProposedApi", &JsValue::TRUE);
        set(&o, "macOptionIsMeta", &JsValue::TRUE);
        set(&o, "theme", &theme(dark()));
        o
    }

    /// Writes a local notice line (`[wpg] ...`) into the terminal.
    fn notice(tab: &Tab, text: &str, error: bool) {
        let color = if error { "31" } else { "90" };
        tab.term
            .write_str(&format!("\r\n\x1b[{color}m[wpg] {text}\x1b[0m\r\n"));
    }

    fn send_bytes(id: u32, data: &[u8]) {
        if let Some(tab) = get(id) {
            if !tab.connected.get() {
                return;
            }
            if let Some(ws) = tab.ws.borrow().as_ref() {
                let _ = ws.send_with_u8_array(data);
            }
        }
    }

    /// Sends a control message on tab `id`'s socket.
    pub(crate) fn send_json(id: u32, v: &serde_json::Value) {
        if let Some(tab) = get(id) {
            if let Some(ws) = tab.ws.borrow().as_ref() {
                let _ = ws.send_with_str(&v.to_string());
            }
        }
    }

    fn clipboard() -> Option<JsValue> {
        let nav = web_sys::window()?.navigator();
        Reflect::get(&nav, &"clipboard".into())
            .ok()
            .filter(|c| !c.is_undefined() && !c.is_null())
    }

    /// Copies `text` (the async Clipboard API, or a hidden text area on plain
    /// HTTP where that API does not exist).
    fn clipboard_write(text: &str) {
        if let Some(c) = clipboard() {
            if let Ok(f) = Reflect::get(&c, &"writeText".into()) {
                if let Ok(f) = f.dyn_into::<js_sys::Function>() {
                    let _ = f.call1(&c, &JsValue::from_str(text));
                    return;
                }
            }
        }
        let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        let Ok(ta) = doc.create_element("textarea") else {
            return;
        };
        let Ok(ta) = ta.dyn_into::<web_sys::HtmlTextAreaElement>() else {
            return;
        };
        ta.set_value(text);
        let _ = ta.set_attribute("style", "position:fixed;left:-9999px;top:0");
        if let Some(body) = doc.body() {
            let _ = body.append_child(&ta);
            ta.select();
            if let Ok(hd) = doc.clone().dyn_into::<web_sys::HtmlDocument>() {
                let _ = hd.exec_command("copy");
            }
            ta.remove();
        }
    }

    /// Copies the selection of tab `id`.
    pub(crate) fn copy_selection(id: u32) {
        if let Some(tab) = get(id) {
            if tab.term.has_selection() {
                clipboard_write(&tab.term.get_selection());
            }
        }
    }

    /// Pastes the clipboard into tab `id` (needs HTTPS or localhost; otherwise
    /// a notice points to Ctrl+V / Shift+Insert).
    pub(crate) fn paste_clipboard(store: TermStore, id: u32) {
        let read = clipboard().and_then(|c| {
            let f: js_sys::Function = Reflect::get(&c, &"readText".into()).ok()?.dyn_into().ok()?;
            f.call0(&c).ok()?.dyn_into::<js_sys::Promise>().ok()
        });
        let Some(promise) = read else {
            if let Some(tab) = get(id) {
                notice(&tab, &tab.i18n.t("ui-term-notice-clipboard"), false);
            }
            return;
        };
        leptos::task::spawn_local(async move {
            match wasm_bindgen_futures::JsFuture::from(promise).await {
                Ok(v) => {
                    if let Some(text) = v.as_string() {
                        super::request_paste(store, id, text, None);
                    }
                }
                Err(_) => {
                    if let Some(tab) = get(id) {
                        notice(&tab, &tab.i18n.t("ui-term-notice-clipboard"), false);
                    }
                }
            }
        });
    }

    /// Sends text as typed: `term.paste` (bracketed paste aware), or line by
    /// line with the configured pause.
    pub(crate) fn send_text(store: TermStore, id: u32, text: &str) {
        let Some(tab) = get(id) else {
            return;
        };
        let delay = store.settings.get_untracked().line_delay_ms;
        if delay == 0 || !super::super::is_multiline(text) {
            tab.term.paste(text);
            return;
        }
        let lines = split_lines(text);
        leptos::task::spawn_local(async move {
            for line in lines {
                if get(id).is_none_or(|t| !t.connected.get()) {
                    return;
                }
                send_bytes(id, format!("{line}\r").as_bytes());
                gloo_timers::future::TimeoutFuture::new(delay).await;
            }
        });
    }

    fn handle(id: u32, msg: ServerMsg) {
        let Some(tab) = get(id) else {
            return;
        };
        let i18n = tab.i18n;
        match &msg {
            ServerMsg::Connected { .. } => {
                tab.connected.set(true);
                tab.term.focus();
            }
            ServerMsg::Error { code, detail, .. } => {
                let text = i18n.ta(
                    error_key(code),
                    &[("detail", detail.clone().unwrap_or_default())],
                );
                notice(&tab, &text, true);
            }
            ServerMsg::Closed { reason, .. } => {
                tab.connected.set(false);
                let r = i18n.t(closed_key(reason));
                notice(
                    &tab,
                    &i18n.ta("ui-term-notice-closed", &[("reason", r)]),
                    false,
                );
            }
            _ => {}
        }
        tab.store.update(id, |t| t.apply(msg));
    }

    fn connect(tab: &Rc<Tab>, id: u32) {
        drop_ws(tab);
        tab.store.update(id, |t| {
            t.phase = Phase::Progress("opening".into());
            t.error = None;
        });
        let Ok(ws) = WebSocket::new(&crate::dom::ws_url("/api/terminal/ws")) else {
            tab.store.update(id, TabState::lost);
            return;
        };
        ws.set_binary_type(BinaryType::Arraybuffer);
        let on_open = Closure::<dyn FnMut()>::new(move || {
            if let Some(tab) = get(id) {
                let msg = tab
                    .params
                    .borrow()
                    .message(tab.term.cols(), tab.term.rows());
                send_json(id, &msg);
            }
        });
        let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |ev: MessageEvent| {
            let data = ev.data();
            if let Some(text) = data.as_string() {
                if let Ok(msg) = serde_json::from_str::<ServerMsg>(&text) {
                    handle(id, msg);
                }
            } else if let Ok(buf) = data.dyn_into::<js_sys::ArrayBuffer>() {
                if let Some(tab) = get(id) {
                    tab.term.write_bytes(&Uint8Array::new(&buf));
                }
            }
        });
        let on_close = Closure::<dyn FnMut()>::new(move || {
            if let Some(tab) = get(id) {
                tab.connected.set(false);
                let was_open = tab
                    .store
                    .tab(id)
                    .is_some_and(|t| !matches!(t.phase, Phase::Closed { .. }));
                if was_open {
                    notice(&tab, &tab.i18n.t("ui-term-notice-lost"), true);
                }
                tab.store.update(id, TabState::lost);
            }
        });
        ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        *tab.ws_keep.borrow_mut() =
            vec![Box::new(on_open), Box::new(on_message), Box::new(on_close)];
        *tab.ws.borrow_mut() = Some(ws);
    }

    /// Closes the socket without reporting a lost connection.
    fn drop_ws(tab: &Tab) {
        tab.connected.set(false);
        if let Some(ws) = tab.ws.borrow_mut().take() {
            ws.set_onopen(None);
            ws.set_onmessage(None);
            ws.set_onclose(None);
            let _ = ws.close();
        }
        tab.ws_keep.borrow_mut().clear();
    }

    fn listen<E: wasm_bindgen::convert::FromWasmAbi + 'static>(
        tab: &Tab,
        event: &str,
        capture: bool,
        f: impl FnMut(E) + 'static,
    ) {
        let c = Closure::<dyn FnMut(E)>::new(f);
        let _ = tab.el.add_event_listener_with_callback_and_bool(
            event,
            c.as_ref().unchecked_ref(),
            capture,
        );
        tab.keep.borrow_mut().push(Box::new(c));
    }

    fn create(
        id: u32,
        store: TermStore,
        i18n: I18n,
        params: ConnectParams,
    ) -> Result<Rc<Tab>, JsValue> {
        let settings = store.settings.get_untracked();
        let term = XTerm::create(&options(&settings))?;
        let fitter = FitAddon::create()?;
        term.load_addon(&fitter);
        term.load_addon(&xterm::web_links_addon()?);
        let search = SearchAddon::create()?;
        term.load_addon(&search);
        let doc = web_sys::window()
            .and_then(|w| w.document())
            .ok_or_else(|| JsValue::from_str("no document"))?;
        let el: HtmlElement = doc.create_element("div")?.dyn_into()?;
        el.set_class_name("h-full w-full");
        let tab = Rc::new(Tab {
            term,
            fit: fitter,
            search,
            el,
            opened: Cell::new(false),
            connect_pending: Cell::new(true),
            connected: Cell::new(false),
            ws: RefCell::new(None),
            params: RefCell::new(params),
            store,
            i18n,
            keep: RefCell::new(Vec::new()),
            ws_keep: RefCell::new(Vec::new()),
            observer: RefCell::new(None),
        });

        let on_data =
            Closure::<dyn FnMut(String)>::new(move |s: String| send_bytes(id, s.as_bytes()));
        let on_binary = Closure::<dyn FnMut(String)>::new(move |s: String| {
            let bytes: Vec<u8> = s.chars().map(|c| (u32::from(c) & 0xff) as u8).collect();
            send_bytes(id, &bytes);
        });
        let on_resize = Closure::<dyn FnMut(JsValue)>::new(move |size: JsValue| {
            let n = |k: &str| {
                Reflect::get(&size, &k.into())
                    .ok()
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0)
            };
            if get(id).is_some_and(|t| t.connected.get()) {
                send_json(
                    id,
                    &json!({ "type": "resize", "cols": n("cols"), "rows": n("rows") }),
                );
            }
        });
        let on_selection = Closure::<dyn FnMut()>::new(move || {
            if store.settings.get_untracked().copy_on_select {
                copy_selection(id);
            }
        });
        let on_key = Closure::<dyn FnMut(web_sys::KeyboardEvent) -> bool>::new(
            move |ev: web_sys::KeyboardEvent| {
                if ev.type_() != "keydown" {
                    return true;
                }
                let key = ev.key();
                let ctrl_shift = ev.ctrl_key() && ev.shift_key() && !ev.alt_key();
                if (ctrl_shift && key.eq_ignore_ascii_case("c"))
                    || (ev.ctrl_key() && !ev.shift_key() && key == "Insert")
                {
                    ev.prevent_default();
                    copy_selection(id);
                    return false;
                }
                if (ctrl_shift && key.eq_ignore_ascii_case("v"))
                    || (ev.shift_key() && !ev.ctrl_key() && key == "Insert")
                {
                    ev.prevent_default();
                    paste_clipboard(store, id);
                    return false;
                }
                true
            },
        );
        tab.term.on_data(on_data.as_ref().unchecked_ref());
        tab.term.on_binary(on_binary.as_ref().unchecked_ref());
        tab.term.on_resize(on_resize.as_ref().unchecked_ref());
        tab.term
            .on_selection_change(on_selection.as_ref().unchecked_ref());
        tab.term
            .attach_custom_key_event_handler(on_key.as_ref().unchecked_ref());
        tab.keep.borrow_mut().extend([
            Box::new(on_data) as Box<dyn Any>,
            Box::new(on_binary),
            Box::new(on_resize),
            Box::new(on_selection),
            Box::new(on_key),
        ]);

        // Right / middle click paste (Tera Term style), the browser's paste
        // (Ctrl+V) goes through the multi-line confirmation as well.
        listen(
            &tab,
            "contextmenu",
            false,
            move |ev: web_sys::MouseEvent| {
                if store.settings.get_untracked().right_click_paste {
                    ev.prevent_default();
                    paste_clipboard(store, id);
                }
            },
        );
        listen(&tab, "auxclick", false, move |ev: web_sys::MouseEvent| {
            if ev.button() == 1 && store.settings.get_untracked().right_click_paste {
                ev.prevent_default();
                paste_clipboard(store, id);
            }
        });
        listen(&tab, "paste", true, move |ev: web_sys::Event| {
            let text = Reflect::get(&ev, &"clipboardData".into())
                .ok()
                .filter(|d| !d.is_null() && !d.is_undefined())
                .and_then(|d| {
                    let f: js_sys::Function =
                        Reflect::get(&d, &"getData".into()).ok()?.dyn_into().ok()?;
                    f.call1(&d, &"text/plain".into()).ok()?.as_string()
                });
            if let Some(text) = text {
                ev.prevent_default();
                ev.stop_propagation();
                super::request_paste(store, id, text, None);
            }
        });

        let refit = Closure::<dyn FnMut()>::new(move || {
            request_animation_frame(move || fit(id));
        });
        if let Ok(ro) = web_sys::ResizeObserver::new(refit.as_ref().unchecked_ref()) {
            ro.observe(&tab.el);
            *tab.observer.borrow_mut() = Some(ro);
        }
        tab.keep.borrow_mut().push(Box::new(refit));
        Ok(tab)
    }

    /// Opens a new tab connecting with `params` (loads xterm.js on first use).
    pub(crate) fn open(store: TermStore, i18n: I18n, params: ConnectParams) {
        let id = store.next_id();
        let mut state = TabState::new(id, &params.profile);
        state.phase = Phase::Progress("opening".into());
        store.tabs.update(|v| v.push(state));
        store.active.set(Some(id));
        store.show_form.set(false);
        leptos::task::spawn_local(async move {
            if let Err(e) = xterm::load().await {
                store.update(id, |t| {
                    t.error = Some(TermError {
                        code: "xterm_load".into(),
                        detail: Some(e),
                        info: None,
                    });
                    t.lost();
                });
                return;
            }
            match create(id, store, i18n, params) {
                Ok(tab) => {
                    TABS.with(|t| t.borrow_mut().insert(id, tab));
                    mount(id);
                }
                Err(e) => store.update(id, |t| {
                    t.error = Some(TermError {
                        code: "xterm_load".into(),
                        detail: e.as_string(),
                        info: None,
                    });
                    t.lost();
                }),
            }
        });
    }

    /// Puts tab `id` into its host element (`#term-host-<id>`, rendered by the
    /// page), opening the terminal and connecting on the first call.
    pub(crate) fn mount(id: u32) {
        let Some(tab) = get(id) else {
            return;
        };
        let Some(host) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(&format!("term-host-{id}")))
        else {
            return;
        };
        let moved = tab.el.parent_element().as_ref() != Some(&host);
        if moved {
            let _ = host.append_child(&tab.el);
        }
        if !tab.opened.get() {
            tab.opened.set(true);
            tab.term.open(&tab.el);
        } else if moved {
            // back on the page after navigating away: redraw
            tab.term.refresh(0, tab.term.rows().saturating_sub(1));
        }
        tab.fit.fit();
        tab.term.focus();
        if tab.connect_pending.replace(false) {
            connect(&tab, id);
        }
    }

    /// Resizes tab `id` to its element.
    pub(crate) fn fit(id: u32) {
        if let Some(tab) = get(id) {
            if tab.opened.get() && tab.el.offset_width() > 0 {
                tab.fit.fit();
            }
        }
    }

    pub(crate) fn focus(id: u32) {
        if let Some(tab) = get(id) {
            tab.term.focus();
        }
    }

    pub(crate) fn disconnect(id: u32) {
        send_json(id, &json!({ "type": "disconnect" }));
    }

    /// Connects tab `id` again with the same parameters.
    pub(crate) fn reconnect(id: u32) {
        if let Some(tab) = get(id) {
            notice(&tab, &tab.i18n.t("ui-term-notice-reconnecting"), false);
            connect(&tab, id);
        }
    }

    /// Ends and removes tab `id`.
    pub(crate) fn close(store: TermStore, id: u32) {
        if let Some(tab) = TABS.with(|t| t.borrow_mut().remove(&id)) {
            if tab.connected.get() {
                send_json(id, &json!({ "type": "disconnect" }));
            }
            drop_ws(&tab);
            if let Some(ro) = tab.observer.borrow_mut().take() {
                ro.disconnect();
            }
            tab.term.dispose();
            tab.el.remove();
            tab.keep.borrow_mut().clear();
        }
        store.remove(id);
    }

    pub(crate) fn clear(id: u32) {
        if let Some(tab) = get(id) {
            tab.term.clear();
        }
    }

    /// Finds `text` in the scrollback (next / previous match).
    pub(crate) fn find(id: u32, text: &str, forward: bool) -> bool {
        get(id).is_some_and(|tab| {
            if text.is_empty() {
                tab.search.clear_decorations();
                false
            } else if forward {
                tab.search.find_next(text)
            } else {
                tab.search.find_previous(text)
            }
        })
    }

    /// The scrollback of tab `id` as plain text.
    pub(crate) fn scrollback_text(id: u32) -> String {
        get(id).map(|t| t.term.buffer_text()).unwrap_or_default()
    }

    /// Applies font size / scrollback to every tab.
    pub(crate) fn apply_settings(s: &TermSettings) {
        TABS.with(|t| {
            for tab in t.borrow().values() {
                tab.term.set_option("fontSize", &JsValue::from(s.font_size));
                tab.term
                    .set_option("scrollback", &JsValue::from(s.scrollback));
                if tab.opened.get() && tab.el.offset_width() > 0 {
                    tab.fit.fit();
                }
            }
        });
    }

    /// Follows the light / dark theme.
    pub(crate) fn apply_theme(dark: bool) {
        TABS.with(|t| {
            for tab in t.borrow().values() {
                tab.term.set_option("theme", &theme(dark));
            }
        });
    }
}
