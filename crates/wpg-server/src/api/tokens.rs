//! `/api/tokens`: API tokens of the logged-in user (see [`crate::auth::token`]).
//! These endpoints need a session; bearer requests get 403.

// utoipa's generated code spells out paths the lint considers redundant.
#![allow(unused_qualifications)]

use axum::extract::State;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::auth::token::{self, TokenError};
use crate::auth::{now_secs, Principal, AUDIT};
use crate::db::{Role, TokenInfo, User};
use crate::error::{codes, ApiError, ApiJson, ApiPath, ApiResult};
use crate::state::AppState;

/// `POST /api/tokens` body.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateToken {
    /// Name, to recognise the token later (1 to 100 characters).
    pub name: String,
    /// Expiry in days (1 to 3650); omitted or `null`: never expires.
    #[serde(default)]
    pub expires_in_days: Option<u32>,
}

/// `POST /api/tokens` response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreatedToken {
    /// The stored token.
    pub token: TokenInfo,
    /// The secret to send as `Authorization: Bearer <secret>`. Only this response
    /// contains it.
    pub secret: String,
}

fn user(st: &AppState, p: &Principal) -> ApiResult<User> {
    st.auth
        .db()
        .user_by_name(&p.user)
        .map_err(ApiError::internal)?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                codes::UNAUTHORIZED,
                "auth-error-unauthorized",
            )
        })
}

/// Admins see and revoke every token, other users their own.
fn owner_filter(u: &User) -> Option<i64> {
    (u.role != Role::Admin).then_some(u.id)
}

/// API tokens (admins: all; others: their own), oldest first.
#[utoipa::path(get, path = "/api/tokens", tag = "auth",
    responses(
        (status = 200, description = "Tokens (without secrets)", body = Vec<TokenInfo>),
        (status = 403, description = "`forbidden`: bearer requests cannot manage tokens", body = crate::error::ErrorBody)))]
pub async fn list(
    State(st): State<AppState>,
    Extension(p): Extension<Principal>,
) -> ApiResult<Json<Vec<TokenInfo>>> {
    let u = user(&st, &p)?;
    st.auth
        .db()
        .tokens(owner_filter(&u))
        .map(Json)
        .map_err(ApiError::internal)
}

/// Creates a token for the logged-in user, with that user's role. The response
/// is the only place the secret appears.
#[utoipa::path(post, path = "/api/tokens", tag = "auth", request_body = CreateToken,
    responses(
        (status = 201, description = "Created", body = CreatedToken),
        (status = 400, description = "Invalid name or expiry", body = crate::error::ErrorBody),
        (status = 403, description = "`forbidden`: bearer requests cannot manage tokens", body = crate::error::ErrorBody)))]
pub async fn create(
    State(st): State<AppState>,
    Extension(p): Extension<Principal>,
    ApiJson(req): ApiJson<CreateToken>,
) -> ApiResult<Response> {
    let u = user(&st, &p)?;
    let (info, secret) =
        token::create(st.auth.db(), &u, &req.name, req.expires_in_days, now_secs()).map_err(
            |e| match e {
                TokenError::Name => ApiError::new(
                    StatusCode::BAD_REQUEST,
                    codes::BAD_REQUEST,
                    "auth-error-token-name",
                )
                .arg("max", token::MAX_NAME),
                TokenError::Expiry => ApiError::new(
                    StatusCode::BAD_REQUEST,
                    codes::BAD_REQUEST,
                    "auth-error-token-expiry",
                )
                .arg("max", token::MAX_DAYS),
                e => ApiError::internal(e),
            },
        )?;
    tracing::info!(target: AUDIT, user = %p.user, ip = %p.ip, token = info.id, name = %info.name, "API token created");
    let mut resp = (
        StatusCode::CREATED,
        Json(CreatedToken {
            token: info,
            secret,
        }),
    )
        .into_response();
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(resp)
}

/// Revokes (deletes) a token; requests with it fail from now on.
#[utoipa::path(delete, path = "/api/tokens/{id}", tag = "auth",
    params(("id" = i64, Path, description = "Token id")),
    responses(
        (status = 204, description = "Revoked"),
        (status = 403, description = "`forbidden`: bearer requests cannot manage tokens", body = crate::error::ErrorBody),
        (status = 404, description = "Unknown id", body = crate::error::ErrorBody)))]
pub async fn revoke(
    State(st): State<AppState>,
    Extension(p): Extension<Principal>,
    ApiPath(id): ApiPath<i64>,
) -> ApiResult<StatusCode> {
    let u = user(&st, &p)?;
    if st
        .auth
        .db()
        .delete_token(id, owner_filter(&u))
        .map_err(ApiError::internal)?
    {
        tracing::info!(target: AUDIT, user = %p.user, ip = %p.ip, token = id, "API token revoked");
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found(format!("token {id}")))
    }
}
