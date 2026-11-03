//! API tokens for scripts and the `wpg remote` CLI (spec §2.6 "API トークン").
//!
//! * A token is `wpg_` followed by 32 random bytes as 64 hex digits, sent as
//!   `Authorization: Bearer wpg_...`. It acts as the user that created it, with
//!   that user's role; there are no finer scopes.
//! * Only the SHA-256 of the token is stored (`api_tokens.hash`), like the
//!   session ids. A fast hash is enough here: the token has 256 bits of entropy,
//!   so there is nothing to brute-force from a leaked hash, while argon2 would cost
//!   19 MiB and tens of milliseconds on every API request (an easy way to
//!   exhaust the server). The first 12 characters are kept as `prefix` so that
//!   people can tell their tokens apart.
//! * Bearer requests need no CSRF token: browsers never attach the header on
//!   their own. Failed bearer authentications count against the login rate limit
//!   of the client address.
//! * Revoking deletes the row, which takes effect on the next request. Tokens
//!   may expire; they survive password changes. Tokens are managed with a
//!   session (the web UI) or `wpg config token` on the server; a bearer request
//!   cannot create or revoke tokens.

use axum::http::{header, HeaderMap};
use thiserror::Error;

use super::session;
use crate::db::{Db, DbError, NewToken, TokenInfo, User};

/// Prefix of every token.
pub const PREFIX: &str = "wpg_";

/// Length of the displayed prefix (`wpg_` + 8 hex digits).
pub const SHOWN: usize = PREFIX.len() + 8;

/// Longest expiry accepted, in days.
pub const MAX_DAYS: u32 = 3650;

/// Longest token name, in characters.
pub const MAX_NAME: usize = 100;

/// A new random token.
///
/// # Errors
/// OS random generator failure.
pub fn generate() -> Result<String, getrandom::Error> {
    Ok(format!("{PREFIX}{}", session::random_token()?))
}

/// The stored hash of a token.
#[must_use]
pub fn hash(token: &str) -> String {
    session::token_id(token)
}

/// Whether `s` has the shape of a token (prefix and 64 lower-case hex digits).
#[must_use]
pub fn well_formed(s: &str) -> bool {
    s.strip_prefix(PREFIX).is_some_and(|h| {
        h.len() == 64
            && h.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// The bearer credential of a request: `None` without an `Authorization`
/// header, `Some("")` for any other scheme or an unreadable value.
#[must_use]
pub fn bearer(headers: &HeaderMap) -> Option<&str> {
    let v = headers.get(header::AUTHORIZATION)?;
    let v = v.to_str().unwrap_or("");
    let (scheme, rest) = v.split_once(' ').unwrap_or((v, ""));
    Some(if scheme.eq_ignore_ascii_case("bearer") {
        rest.trim()
    } else {
        ""
    })
}

/// Errors of [`create`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TokenError {
    /// Empty or longer than [`MAX_NAME`].
    #[error("the token name must have 1 to {MAX_NAME} characters")]
    Name,
    /// Expiry outside 1..=[`MAX_DAYS`] days.
    #[error("the expiry must be between 1 and {MAX_DAYS} days")]
    Expiry,
    /// OS random generator failure.
    #[error("random generator: {0}")]
    Random(getrandom::Error),
    /// Database error.
    #[error(transparent)]
    Db(#[from] DbError),
}

/// Creates a token for `user` named `name` (trimmed), expiring after
/// `expires_in_days` (never when `None`). Returns the row and the secret, which
/// is not stored and cannot be shown again.
///
/// # Errors
/// [`TokenError`].
pub fn create(
    db: &Db,
    user: &User,
    name: &str,
    expires_in_days: Option<u32>,
    now: i64,
) -> Result<(TokenInfo, String), TokenError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME {
        return Err(TokenError::Name);
    }
    let expires = match expires_in_days {
        None => None,
        Some(d) if (1..=MAX_DAYS).contains(&d) => Some(now.saturating_add(i64::from(d) * 86_400)),
        Some(_) => return Err(TokenError::Expiry),
    };
    let secret = generate().map_err(TokenError::Random)?;
    let info = db.insert_token(&NewToken {
        user_id: user.id,
        name,
        hash: &hash(&secret),
        prefix: &secret[..SHOWN],
        now,
        expires,
    })?;
    Ok((info, secret))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn shape_and_header() {
        let t = generate().unwrap();
        assert!(well_formed(&t), "{t}");
        assert_eq!(t.len(), 68);
        assert!(!well_formed("wpg_abc"));
        assert!(!well_formed(&t.to_uppercase()));
        assert!(!well_formed(&t[4..]));
        assert_eq!(hash(&t).len(), 64);

        let mut h = HeaderMap::new();
        assert_eq!(bearer(&h), None);
        h.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer  wpg_x "),
        );
        assert_eq!(bearer(&h), Some("wpg_x"));
        h.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("bearer wpg_y"),
        );
        assert_eq!(bearer(&h), Some("wpg_y"));
        h.insert(header::AUTHORIZATION, HeaderValue::from_static("Basic abc"));
        assert_eq!(bearer(&h), Some(""));
    }

    #[test]
    fn create_validates() {
        let db = Db::open_in_memory().unwrap();
        let u = db
            .create_user("admin", crate::db::Role::Admin, "h", 1)
            .unwrap()
            .unwrap();
        let (info, secret) = create(&db, &u, " ci ", Some(2), 1000).unwrap();
        assert_eq!(info.name, "ci");
        assert_eq!(info.expires, Some(1000 + 2 * 86_400));
        assert!(secret.starts_with(&info.prefix));
        assert_eq!(info.prefix.len(), SHOWN);
        assert!(db.token_by_hash(&hash(&secret), 1000).unwrap().is_some());
        assert!(matches!(
            create(&db, &u, " ", None, 0),
            Err(TokenError::Name)
        ));
        let long = "x".repeat(MAX_NAME + 1);
        assert!(matches!(
            create(&db, &u, &long, None, 0),
            Err(TokenError::Name)
        ));
        assert!(matches!(
            create(&db, &u, "a", Some(0), 0),
            Err(TokenError::Expiry)
        ));
        assert!(matches!(
            create(&db, &u, "a", Some(MAX_DAYS + 1), 0),
            Err(TokenError::Expiry)
        ));
    }
}
