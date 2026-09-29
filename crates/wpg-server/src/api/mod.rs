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
            language of `Accept-Language` (en, ja).",
        license(name = "MIT OR Apache-2.0")
    ),
    tags(
        (name = "system", description = "Health, version, interfaces, diagnostics, backends"),
        (name = "protocols", description = "Protocol catalog"),
        (name = "packets", description = "Render / parse / lint"),
        (name = "templates", description = "Template library"),
        (name = "jobs", description = "Transmit jobs and statistics"),
        (name = "events", description = "WebSocket event stream"),
    ),
    components(schemas(
        crate::error::ErrorBody,
        crate::error::ErrorInfo,
        crate::events::ServerEvent,
        ws::ClientMessage,
        crate::jobs::JobSpec,
        crate::jobs::JobStream,
        crate::jobs::JobStatus,
        crate::jobs::Mix,
    ))
)]
struct ApiDoc;

fn parts() -> (Router<AppState>, utoipa::openapi::OpenApi) {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
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
        .routes(routes!(ws::ws))
        .split_for_parts()
}

/// The API routes (with state applied): the handlers above plus
/// `/api/openapi.json` and the Swagger UI at `/api/docs/`.
pub fn router(state: AppState) -> Router {
    let (router, doc) = parts();
    router
        .with_state(state)
        .merge(SwaggerUi::new("/api/docs").url("/api/openapi.json", doc))
        .route("/api/{*rest}", axum::routing::any(api_not_found))
        .route("/api", axum::routing::any(api_not_found))
}

/// The OpenAPI document (as served at `/api/openapi.json`).
#[must_use]
pub fn openapi() -> utoipa::openapi::OpenApi {
    parts().1
}

async fn api_not_found(_headers: HeaderMap) -> Response {
    ApiError::not_found("API endpoint").into_response()
}
