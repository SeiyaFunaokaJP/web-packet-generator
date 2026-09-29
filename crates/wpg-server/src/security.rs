//! Security response headers (TODO §2.6 "CSP ヘッダ").
//!
//! Every response gets `X-Content-Type-Options`, `Referrer-Policy`,
//! `X-Frame-Options`, `Permissions-Policy` and a Content Security Policy:
//!
//! * the app ([`APP_CSP`]): scripts only from the server plus
//!   `'wasm-unsafe-eval'` for the Leptos WebAssembly module, inline styles
//!   allowed (Leptos / Tailwind set `style` attributes), `ws:` / `wss:` for the
//!   event stream. Trunk's `index.html` boots the module with an inline
//!   `<script type="module">`; [`index_csp`] allows exactly those scripts by
//!   their SHA-256 hash instead of `'unsafe-inline'`.
//! * the Swagger UI under `/api/docs` ([`DOCS_CSP`]): its bundled initializer
//!   needs inline scripts / styles, so that path alone gets them.
//!
//! Handlers that already set a CSP (the SPA's `index.html`) keep it.

use axum::extract::Request;
use axum::http::{header, HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use base64::Engine as _;
use sha2::{Digest, Sha256};

/// CSP of the application.
pub const APP_CSP: &str = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; \
     style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; \
     connect-src 'self' ws: wss:; object-src 'none'; base-uri 'self'; \
     form-action 'self'; frame-ancestors 'none'";

/// CSP of the Swagger UI (`/api/docs`).
pub const DOCS_CSP: &str = "default-src 'self'; script-src 'self' 'unsafe-inline'; \
     style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; \
     object-src 'none'; base-uri 'self'; frame-ancestors 'none'";

/// `Permissions-Policy`: no powerful features.
pub const PERMISSIONS_POLICY: &str =
    "camera=(), microphone=(), geolocation=(), payment=(), usb=(), interest-cohort=()";

/// Middleware adding the security headers.
pub async fn headers(req: Request, next: Next) -> Response {
    let docs = {
        let p = req.uri().path();
        p == "/api/docs" || p.starts_with("/api/docs/")
    };
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    if !h.contains_key(header::CONTENT_SECURITY_POLICY) {
        h.insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(if docs { DOCS_CSP } else { APP_CSP }),
        );
    }
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static(PERMISSIONS_POLICY),
    );
    h.insert(
        HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    );
    resp
}

/// Bodies of the inline `<script>` elements of an HTML page (no `src`).
fn inline_scripts(html: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find("<script") {
        let after = &rest[start..];
        let Some(tag_end) = after.find('>') else {
            break;
        };
        let tag = &after[..tag_end];
        let body_start = start + tag_end + 1;
        let Some(len) = rest[body_start..].find("</script>") else {
            break;
        };
        if !tag.contains(" src=") && !tag.contains(" src =") {
            out.push(&rest[body_start..body_start + len]);
        }
        rest = &rest[body_start + len + "</script>".len()..];
    }
    out
}

/// [`APP_CSP`] extended with the hashes of the page's inline scripts.
#[must_use]
pub fn index_csp(html: &str) -> String {
    let hashes: Vec<String> = inline_scripts(html)
        .into_iter()
        .map(|s| {
            format!(
                "'sha256-{}'",
                base64::engine::general_purpose::STANDARD.encode(Sha256::digest(s.as_bytes()))
            )
        })
        .collect();
    if hashes.is_empty() {
        return APP_CSP.to_owned();
    }
    APP_CSP.replacen(
        "script-src 'self' 'wasm-unsafe-eval'",
        &format!("script-src 'self' 'wasm-unsafe-eval' {}", hashes.join(" ")),
        1,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_script_hashes() {
        let html = r#"<html><head><script type="module">import init from '/a.js';init();</script>
            <script src="/x.js"></script><script>
x()</script></head></html>"#;
        let s = inline_scripts(html);
        assert_eq!(s, ["import init from '/a.js';init();", "\nx()"]);
        let csp = index_csp(html);
        assert_eq!(csp.matches("'sha256-").count(), 2, "{csp}");
        assert!(csp.contains("'wasm-unsafe-eval' 'sha256-"));
        assert_eq!(index_csp("<p>no scripts</p>"), APP_CSP);
    }
}
