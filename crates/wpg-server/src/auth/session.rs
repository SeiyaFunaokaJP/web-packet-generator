//! Cookie sessions.
//!
//! A deliberately small hand-rolled store instead of `tower-sessions`: the only
//! session data is "which user" plus a CSRF token, the store is the `sessions`
//! table of the existing `SQLite` database, and the whole mechanism fits in this
//! file without another middleware stack.
//!
//! * The cookie `wpg_session` carries a random 256-bit token (hex); the database
//!   only stores its SHA-256, so a leaked database file does not leak live
//!   sessions. `HttpOnly; SameSite=Strict; Path=/`, plus `Secure` over TLS.
//! * Expiry is sliding: requests move it `auth.session_ttl_secs` ahead (written
//!   at most once per [`TOUCH_INTERVAL_SECS`]); the cookie's `Max-Age` is renewed
//!   at the same time.
//! * CSRF: a random token bound to the session, sent as the readable cookie
//!   `wpg_csrf` and in `GET /api/auth/status`; mutating requests must echo it in
//!   `X-CSRF-Token` (double submit, checked against the stored value).

use axum::http::{header, HeaderMap, HeaderValue};
use sha2::{Digest, Sha256};

/// Session cookie name.
pub const SESSION_COOKIE: &str = "wpg_session";
/// CSRF cookie name (readable by scripts).
pub const CSRF_COOKIE: &str = "wpg_csrf";
/// CSRF request header.
pub const CSRF_HEADER: &str = "x-csrf-token";
/// Sliding expiry is written at most this often per session.
pub const TOUCH_INTERVAL_SECS: i64 = 60;

/// 32 random bytes as 64 hex digits.
///
/// # Errors
/// OS random generator failure.
pub fn random_token() -> Result<String, getrandom::Error> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b)?;
    Ok(hex(&b))
}

/// Lower-case hex.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// The stored id of a session token.
#[must_use]
pub fn token_id(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

/// Constant-time string comparison.
#[must_use]
pub fn ct_eq(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// Value of cookie `name` from the `Cookie` headers.
#[must_use]
pub fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.trim_matches('"'))
}

/// `Set-Cookie` value. `max_age` 0 deletes the cookie.
#[must_use]
pub fn set_cookie(
    name: &str,
    value: &str,
    max_age: i64,
    http_only: bool,
    secure: bool,
) -> HeaderValue {
    let mut s = format!("{name}={value}; Path=/; Max-Age={max_age}; SameSite=Strict");
    if http_only {
        s.push_str("; HttpOnly");
    }
    if secure {
        s.push_str("; Secure");
    }
    HeaderValue::from_str(&s).unwrap_or_else(|_| HeaderValue::from_static(""))
}

/// Adds the session + CSRF cookies to `headers`.
pub fn append_cookies(headers: &mut HeaderMap, token: &str, csrf: &str, ttl: i64, secure: bool) {
    headers.append(
        header::SET_COOKIE,
        set_cookie(SESSION_COOKIE, token, ttl, true, secure),
    );
    headers.append(
        header::SET_COOKIE,
        set_cookie(CSRF_COOKIE, csrf, ttl, false, secure),
    );
}

/// Adds cookies deleting the session + CSRF cookies.
pub fn append_clear_cookies(headers: &mut HeaderMap, secure: bool) {
    headers.append(
        header::SET_COOKIE,
        set_cookie(SESSION_COOKIE, "", 0, true, secure),
    );
    headers.append(
        header::SET_COOKIE,
        set_cookie(CSRF_COOKIE, "", 0, false, secure),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_and_cookies() {
        let t = random_token().unwrap();
        assert_eq!(t.len(), 64);
        assert_ne!(t, random_token().unwrap());
        assert_eq!(token_id("abc").len(), 64);
        assert!(ct_eq("abc", "abc"));
        assert!(!ct_eq("abc", "abd"));
        assert!(!ct_eq("abc", "ab"));

        let mut h = HeaderMap::new();
        h.append(
            header::COOKIE,
            HeaderValue::from_static("a=1; wpg_session=xyz"),
        );
        h.append(header::COOKIE, HeaderValue::from_static("wpg_csrf=\"q\""));
        assert_eq!(cookie(&h, SESSION_COOKIE), Some("xyz"));
        assert_eq!(cookie(&h, CSRF_COOKIE), Some("q"));
        assert_eq!(cookie(&h, "nope"), None);

        let c = set_cookie(SESSION_COOKIE, "v", 10, true, true);
        assert_eq!(
            c.to_str().unwrap(),
            "wpg_session=v; Path=/; Max-Age=10; SameSite=Strict; HttpOnly; Secure"
        );
    }
}
