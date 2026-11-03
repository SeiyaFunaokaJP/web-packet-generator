//! `/api/tokens`: API tokens of the logged-in user (Settings page).

use serde::{Deserialize, Serialize};

use super::{get, post, request, ApiError, Body, Method};

/// A token as listed (never the secret).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct TokenInfo {
    pub(crate) id: i64,
    pub(crate) name: String,
    /// `wpg_` + 8 hex digits.
    pub(crate) prefix: String,
    pub(crate) user: String,
    /// Unix seconds.
    pub(crate) created: i64,
    #[serde(default)]
    pub(crate) last_used: Option<i64>,
    #[serde(default)]
    pub(crate) expires: Option<i64>,
}

/// `POST /api/tokens` response: the only place the secret appears.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct CreatedToken {
    pub(crate) token: TokenInfo,
    pub(crate) secret: String,
}

#[derive(Debug, Serialize)]
struct CreateToken<'a> {
    name: &'a str,
    expires_in_days: Option<u32>,
}

pub(crate) async fn tokens() -> Result<Vec<TokenInfo>, ApiError> {
    get("/api/tokens").await
}

pub(crate) async fn create_token(
    name: &str,
    expires_in_days: Option<u32>,
) -> Result<CreatedToken, ApiError> {
    post(
        "/api/tokens",
        &CreateToken {
            name,
            expires_in_days,
        },
    )
    .await
}

pub(crate) async fn revoke_token(id: i64) -> Result<(), ApiError> {
    request(Method::Delete, &format!("/api/tokens/{id}"), Body::Empty)
        .await
        .map(|_| ())
}
