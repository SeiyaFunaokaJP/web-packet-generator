//! The JSON API under `/api` and its OpenAPI document.

use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Router;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use utoipa_swagger_ui::SwaggerUi;

use crate::error::ApiError;
use crate::state::AppState;

pub mod auth;
pub mod captures;
pub mod jobs;
pub mod packets;
pub mod protocols;
pub mod system;
pub mod templates;
pub mod ws;

/// Base OpenAPI document (paths are added by the router).
#[derive(OpenApi)]
#[openapi(
    info(
        title = "wpg — Web Packet Generator API",
        description = "REST + WebSocket API of the wpg server. Errors are \
            `{ \"error\": { \"code\", \"message\", \"details\" } }` with `message` in the \
            language of `Accept-Language` (en, ja).\n\n\
            Authentication: log in with `POST /api/auth/login` (or complete \
            `POST /api/auth/setup` on first run); the `wpg_session` cookie then \
            authenticates every request and POST / PUT / DELETE must also send the \
            session's CSRF token (cookie `wpg_csrf`, `GET /api/auth/status`) in \
            `X-CSRF-Token`. Without a session protected endpoints answer 401 \
            (`unauthorized`, or `setup_required` while no password exists).",
        license(name = "MIT OR Apache-2.0")
    ),
    modifiers(&SecuritySchemes),
    security(("cookieAuth" = [])),
    tags(
        (name = "auth", description = "Login, logout, first-run setup"),
        (name = "system", description = "Health, version, interfaces, diagnostics, backends"),
        (name = "protocols", description = "Protocol catalog"),
        (name = "packets", description = "Render / parse / lint"),
        (name = "templates", description = "Template library"),
        (name = "jobs", description = "Transmit jobs and statistics"),
        (name = "captures", description = "Receive counters, captured frames and signature verification"),
        (name = "events", description = "WebSocket event stream"),
    ),
    components(schemas(
        crate::auth::AuthStatus,
        crate::auth::UserInfo,
        crate::db::Role,
        crate::error::ErrorBody,
        crate::error::ErrorInfo,
        crate::events::ServerEvent,
        ws::ClientMessage,
        crate::jobs::JobSpec,
        crate::jobs::JobStream,
        crate::jobs::JobStatus,
        crate::jobs::Mix,
        crate::rx::CaptureSpec,
        crate::rx::CaptureInfo,
    ))
)]
struct ApiDoc;

/// `cookieAuth` (the session cookie) and `csrfToken` (the header required on
/// mutating requests).
struct SecuritySchemes;

impl utoipa::Modify for SecuritySchemes {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "cookieAuth",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::with_description(
                crate::auth::session::SESSION_COOKIE,
                "Session cookie set by `POST /api/auth/login` / `POST /api/auth/setup`",
            ))),
        );
        components.add_security_scheme(
            "csrfToken",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::with_description(
                "X-CSRF-Token",
                "Required on POST / PUT / DELETE: the `csrf_token` of `GET /api/auth/status` \
                 (also the `wpg_csrf` cookie)",
            ))),
        );
    }
}

fn parts() -> (Router<AppState>, utoipa::openapi::OpenApi) {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(auth::status))
        .routes(routes!(auth::setup))
        .routes(routes!(auth::login))
        .routes(routes!(auth::logout))
        .routes(routes!(system::health))
        .routes(routes!(system::version))
        .routes(routes!(system::ifaces))
        .routes(routes!(system::doctor))
        .routes(routes!(system::backends))
        .routes(routes!(protocols::list))
        .routes(routes!(protocols::get))
        .routes(routes!(packets::render))
        .routes(routes!(packets::parse))
        .routes(routes!(packets::lint_template))
        .routes(routes!(templates::list, templates::create))
        .routes(routes!(
            templates::get,
            templates::update,
            templates::delete
        ))
        .routes(routes!(templates::duplicate))
        .routes(routes!(templates::export))
        .routes(routes!(templates::import))
        .routes(routes!(templates::favorite))
        .routes(routes!(jobs::list, jobs::create))
        .routes(routes!(jobs::history))
        .routes(routes!(jobs::get, jobs::delete))
        .routes(routes!(jobs::start))
        .routes(routes!(jobs::pause))
        .routes(routes!(jobs::resume))
        .routes(routes!(jobs::stop))
        .routes(routes!(jobs::stats))
        .routes(routes!(jobs::overview))
        .routes(routes!(captures::list, captures::create))
        .routes(routes!(captures::backends))
        .routes(routes!(captures::check_filter))
        .routes(routes!(captures::get, captures::delete))
        .routes(routes!(captures::stop))
        .routes(routes!(captures::packets))
        .routes(routes!(captures::pcapng))
        .routes(routes!(ws::ws))
        .split_for_parts()
}

/// The API routes (with state applied): the handlers above plus
/// `/api/openapi.json` and the Swagger UI at `/api/docs/`, behind the
/// authentication middleware ([`crate::auth::require`]).
pub fn router(state: AppState) -> Router {
    let (router, doc) = parts();
    router
        .with_state(state.clone())
        .merge(SwaggerUi::new("/api/docs").url("/api/openapi.json", doc))
        .route("/api/{*rest}", axum::routing::any(api_not_found))
        .route("/api", axum::routing::any(api_not_found))
        .layer(axum::middleware::from_fn_with_state(
            state,
            crate::auth::require,
        ))
}

/// The OpenAPI document (as served at `/api/openapi.json`).
#[must_use]
pub fn openapi() -> utoipa::openapi::OpenApi {
    parts().1
}

async fn api_not_found(_headers: HeaderMap) -> Response {
    ApiError::not_found("API endpoint").into_response()
}
