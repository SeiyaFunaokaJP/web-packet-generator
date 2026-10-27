//! Bindings of the vendored xterm.js 6 (`vendor/xterm`, copied to `/xterm/` by
//! Trunk) and its fit / web-links / search add-ons.
//!
//! The UMD bundles are loaded on first use ([`load`]) by inserting `<script>`
//! elements (same origin, so the CSP allows them); they define the globals
//! `Terminal`, `FitAddon.FitAddon`, `WebLinksAddon.WebLinksAddon` and
//! `SearchAddon.SearchAddon`. Objects are constructed through `Reflect` so nothing
//! touches those globals before they exist; methods are typed bindings.

use js_sys::{Array, Function, Object, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// Where Trunk puts `vendor/xterm`.
const BASE: &str = "/xterm/";
/// Scripts, in load order.
const SCRIPTS: [&str; 4] = [
    "xterm.js",
    "addon-fit.js",
    "addon-web-links.js",
    "addon-search.js",
];

#[wasm_bindgen]
extern "C" {
    /// `Terminal`.
    #[derive(Debug, Clone)]
    pub(crate) type XTerm;

    #[wasm_bindgen(method)]
    pub(crate) fn open(this: &XTerm, parent: &web_sys::HtmlElement);
    #[wasm_bindgen(method, js_name = write)]
    pub(crate) fn write_bytes(this: &XTerm, data: &js_sys::Uint8Array);
    #[wasm_bindgen(method, js_name = write)]
    pub(crate) fn write_str(this: &XTerm, data: &str);
    #[wasm_bindgen(method)]
    pub(crate) fn paste(this: &XTerm, data: &str);
    #[wasm_bindgen(method)]
    pub(crate) fn focus(this: &XTerm);
    #[wasm_bindgen(method)]
    pub(crate) fn clear(this: &XTerm);
    #[wasm_bindgen(method)]
    pub(crate) fn refresh(this: &XTerm, start: u32, end: u32);
    #[wasm_bindgen(method)]
    pub(crate) fn dispose(this: &XTerm);
    #[wasm_bindgen(method, js_name = hasSelection)]
    pub(crate) fn has_selection(this: &XTerm) -> bool;
    #[wasm_bindgen(method, js_name = getSelection)]
    pub(crate) fn get_selection(this: &XTerm) -> String;
    #[wasm_bindgen(method, js_name = loadAddon)]
    pub(crate) fn load_addon(this: &XTerm, addon: &JsValue);
    #[wasm_bindgen(method, js_name = onData)]
    pub(crate) fn on_data(this: &XTerm, cb: &Function) -> JsValue;
    #[wasm_bindgen(method, js_name = onBinary)]
    pub(crate) fn on_binary(this: &XTerm, cb: &Function) -> JsValue;
    #[wasm_bindgen(method, js_name = onResize)]
    pub(crate) fn on_resize(this: &XTerm, cb: &Function) -> JsValue;
    #[wasm_bindgen(method, js_name = onSelectionChange)]
    pub(crate) fn on_selection_change(this: &XTerm, cb: &Function) -> JsValue;
    #[wasm_bindgen(method, js_name = attachCustomKeyEventHandler)]
    pub(crate) fn attach_custom_key_event_handler(this: &XTerm, cb: &Function);
    #[wasm_bindgen(method, getter)]
    pub(crate) fn cols(this: &XTerm) -> u32;
    #[wasm_bindgen(method, getter)]
    pub(crate) fn rows(this: &XTerm) -> u32;
    #[wasm_bindgen(method, getter)]
    pub(crate) fn options(this: &XTerm) -> Object;

    /// `FitAddon.FitAddon`.
    #[derive(Debug, Clone)]
    pub(crate) type FitAddon;
    #[wasm_bindgen(method)]
    pub(crate) fn fit(this: &FitAddon);

    /// `SearchAddon.SearchAddon`.
    #[derive(Debug, Clone)]
    pub(crate) type SearchAddon;
    #[wasm_bindgen(method, js_name = findNext)]
    pub(crate) fn find_next(this: &SearchAddon, term: &str) -> bool;
    #[wasm_bindgen(method, js_name = findPrevious)]
    pub(crate) fn find_previous(this: &SearchAddon, term: &str) -> bool;
    #[wasm_bindgen(method, js_name = clearDecorations)]
    pub(crate) fn clear_decorations(this: &SearchAddon);
}

fn global(path: &[&str]) -> Result<JsValue, JsValue> {
    let mut v: JsValue = js_sys::global().into();
    for p in path {
        v = Reflect::get(&v, &JsValue::from_str(p))?;
        if v.is_undefined() {
            return Err(JsValue::from_str(&format!(
                "{} is not loaded",
                path.join(".")
            )));
        }
    }
    Ok(v)
}

fn construct(path: &[&str], args: &Array) -> Result<JsValue, JsValue> {
    let ctor: Function = global(path)?.dyn_into()?;
    Reflect::construct(&ctor, args)
}

/// Sets `obj[key] = value`.
pub(crate) fn set(obj: &JsValue, key: &str, value: &JsValue) {
    let _ = Reflect::set(obj, &JsValue::from_str(key), value);
}

impl XTerm {
    /// `new Terminal(options)`.
    pub(crate) fn create(options: &Object) -> Result<XTerm, JsValue> {
        Ok(construct(&["Terminal"], &Array::of1(options))?.unchecked_into())
    }

    /// `terminal.options[key] = value` (xterm applies it live).
    pub(crate) fn set_option(&self, key: &str, value: &JsValue) {
        set(&self.options(), key, value);
    }

    /// The whole buffer (scrollback + screen) as text, trailing blanks trimmed.
    pub(crate) fn buffer_text(&self) -> String {
        let run = || -> Result<String, JsValue> {
            let active = Reflect::get(&Reflect::get(self, &"buffer".into())?, &"active".into())?;
            let len = Reflect::get(&active, &"length".into())?
                .as_f64()
                .unwrap_or(0.0);
            let get_line: Function = Reflect::get(&active, &"getLine".into())?.dyn_into()?;
            let mut out = String::new();
            let mut i = 0.0;
            while i < len {
                let line = get_line.call1(&active, &JsValue::from_f64(i))?;
                if !line.is_undefined() {
                    let wrapped = Reflect::get(&line, &"isWrapped".into())?.is_truthy();
                    let to_string: Function =
                        Reflect::get(&line, &"translateToString".into())?.dyn_into()?;
                    let text = to_string.call1(&line, &JsValue::TRUE)?;
                    if wrapped && out.ends_with('\n') {
                        out.pop();
                    }
                    out.push_str(&text.as_string().unwrap_or_default());
                    out.push('\n');
                }
                i += 1.0;
            }
            Ok(out.trim_end_matches('\n').to_owned() + "\n")
        };
        run().unwrap_or_default()
    }
}

impl FitAddon {
    pub(crate) fn create() -> Result<FitAddon, JsValue> {
        Ok(construct(&["FitAddon", "FitAddon"], &Array::new())?.unchecked_into())
    }
}

impl SearchAddon {
    pub(crate) fn create() -> Result<SearchAddon, JsValue> {
        Ok(construct(&["SearchAddon", "SearchAddon"], &Array::new())?.unchecked_into())
    }
}

/// `new WebLinksAddon.WebLinksAddon(handler)`: links open in a new tab without
/// an opener.
pub(crate) fn web_links_addon() -> Result<JsValue, JsValue> {
    let handler = Closure::<dyn Fn(JsValue, String)>::new(|_ev: JsValue, uri: String| {
        if let Some(w) = web_sys::window() {
            let _ = w.open_with_url_and_target_and_features(&uri, "_blank", "noopener,noreferrer");
        }
    });
    let addon = construct(
        &["WebLinksAddon", "WebLinksAddon"],
        &Array::of1(handler.as_ref()),
    );
    handler.forget();
    addon
}

/// Inserts `<script src>` and resolves when it has loaded.
async fn load_script(doc: &web_sys::Document, src: &str) -> Result<(), JsValue> {
    let el = doc.create_element("script")?;
    el.set_attribute("src", src)?;
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let _ = Reflect::set(&el, &"onload".into(), &resolve);
        let _ = Reflect::set(&el, &"onerror".into(), &reject);
    });
    doc.head()
        .ok_or_else(|| JsValue::from_str("no <head>"))?
        .append_child(&el)?;
    wasm_bindgen_futures::JsFuture::from(promise).await?;
    Ok(())
}

thread_local! {
    /// The pending / finished load (shared by concurrent callers).
    static LOADING: std::cell::RefCell<Option<js_sys::Promise>> = const { std::cell::RefCell::new(None) };
}

/// Loads xterm.js, its CSS and the add-ons once.
///
/// # Errors
/// A script that failed to load (message for the UI).
pub(crate) async fn load() -> Result<(), String> {
    if global(&["SearchAddon", "SearchAddon"]).is_ok() {
        return Ok(());
    }
    let promise = LOADING.with(|l| {
        l.borrow_mut()
            .get_or_insert_with(|| {
                wasm_bindgen_futures::future_to_promise(async {
                    let doc = web_sys::window()
                        .and_then(|w| w.document())
                        .ok_or_else(|| JsValue::from_str("no document"))?;
                    let css = doc.create_element("link")?;
                    css.set_attribute("rel", "stylesheet")?;
                    css.set_attribute("href", &format!("{BASE}xterm.css"))?;
                    if let Some(h) = doc.head() {
                        h.append_child(&css)?;
                    }
                    for s in SCRIPTS {
                        load_script(&doc, &format!("{BASE}{s}")).await?;
                    }
                    Ok(JsValue::TRUE)
                })
            })
            .clone()
    });
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(_) => Ok(()),
        Err(e) => {
            // allow a retry
            LOADING.with(|l| *l.borrow_mut() = None);
            Err(e
                .as_string()
                .or_else(|| {
                    Reflect::get(&e, &"type".into())
                        .ok()
                        .and_then(|t| t.as_string())
                })
                .unwrap_or_else(|| "xterm.js".to_owned()))
        }
    }
}
