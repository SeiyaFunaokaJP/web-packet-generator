//! Small browser helpers (cookies, downloads, file reading, clocks). Everything that
//! touches a browser API is gated on `wasm32`; natively the functions are inert.

/// Value of cookie `name` in a `document.cookie` string.
pub(crate) fn parse_cookie(cookies: &str, name: &str) -> Option<String> {
    cookies.split(';').find_map(|c| {
        let (k, v) = c.trim().split_once('=')?;
        (k == name && !v.is_empty()).then(|| v.to_owned())
    })
}

/// A cookie readable by scripts (not `HttpOnly`).
pub(crate) fn cookie(name: &str) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        let doc = web_sys::window()?
            .document()?
            .dyn_into::<web_sys::HtmlDocument>()
            .ok()?;
        parse_cookie(&doc.cookie().ok()?, name)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = name;
        None
    }
}

/// Milliseconds from a monotonic-enough clock (`Date.now()`).
pub(crate) fn now_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        0.0
    }
}

/// Offers `text` as a file download.
pub(crate) fn download_text(filename: &str, mime: &str, text: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::{JsCast, JsValue};
        let run = || -> Result<(), JsValue> {
            let parts = js_sys::Array::of1(&JsValue::from_str(text));
            let opts = web_sys::BlobPropertyBag::new();
            opts.set_type(mime);
            let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &opts)?;
            let url = web_sys::Url::create_object_url_with_blob(&blob)?;
            let doc = web_sys::window()
                .and_then(|w| w.document())
                .ok_or_else(|| JsValue::from_str("no document"))?;
            let a: web_sys::HtmlAnchorElement = doc.create_element("a")?.dyn_into()?;
            a.set_href(&url);
            a.set_download(filename);
            a.click();
            web_sys::Url::revoke_object_url(&url)
        };
        let _ = run();
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (filename, mime, text);
}

/// Text of the first file chosen in an `<input type="file">`.
#[cfg(target_arch = "wasm32")]
pub(crate) async fn read_first_file(input: &web_sys::HtmlInputElement) -> Option<(String, String)> {
    let file = input.files()?.get(0)?;
    let text = wasm_bindgen_futures::JsFuture::from(file.text())
        .await
        .ok()?
        .as_string()?;
    Some((file.name(), text))
}

/// Moves keyboard focus to the element with `id` (dialogs, skip link).
pub(crate) fn focus_id(id: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        if let Some(el) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(id))
            .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = el.focus();
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = id;
}

/// `ws://` / `wss://` URL of `path` on the current origin.
pub(crate) fn ws_url(path: &str) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(loc) = web_sys::window().map(|w| w.location()) {
            let proto = if loc.protocol().ok().as_deref() == Some("https:") {
                "wss"
            } else {
                "ws"
            };
            if let Ok(host) = loc.host() {
                return format!("{proto}://{host}{path}");
            }
        }
    }
    format!("ws://127.0.0.1:8080{path}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookies() {
        let c = "theme=dark; wpg_csrf=abc123; other=1";
        assert_eq!(parse_cookie(c, "wpg_csrf").as_deref(), Some("abc123"));
        assert_eq!(parse_cookie(c, "missing"), None);
        assert_eq!(parse_cookie("wpg_csrf=", "wpg_csrf"), None);
        assert_eq!(parse_cookie("", "x"), None);
        assert_eq!(ws_url("/api/ws"), "ws://127.0.0.1:8080/api/ws");
    }
}
