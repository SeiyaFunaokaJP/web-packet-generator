//! `/api/templates`: template library CRUD, import / export, favourites.

// utoipa's generated code spells out paths the lint considers redundant.
#![allow(unused_qualifications)]

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::{IntoParams, ToSchema};
use wpg_i18n::Locale;
use wpg_template::{lint, Template};

use crate::api::packets::template_from_value;
use crate::error::{ApiError, ApiJson, ApiPath, ApiQuery, ApiResult};
use crate::jobs::store_error;
use crate::localize::{lint_issue_json, request_locale};
use crate::state::AppState;
use crate::store::{ListFilter, TemplateSource, TemplateSummary};

/// Query of `GET /api/templates`.
#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TemplateQuery {
    /// Case-insensitive search in id, name, descriptions, tags and protocols.
    pub q: Option<String>,
    /// Required tag(s), comma separated.
    pub tag: Option<String>,
    /// Only favourites (`true`) / non-favourites (`false`).
    pub favorite: Option<bool>,
    /// Only `user` or `library` templates.
    pub source: Option<TemplateSource>,
}

/// `GET /api/templates` response.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TemplateList {
    /// Matching templates, sorted by name.
    pub templates: Vec<TemplateSummary>,
    /// Recently used template ids, most recent first.
    pub recent: Vec<String>,
}

/// A template with its index entry and lint findings.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TemplateDetail {
    /// Index entry.
    pub summary: TemplateSummary,
    /// The template (JSON form of `.wpg.yaml`).
    #[schema(value_type = Object)]
    pub template: Template,
    /// Lint findings with localised `message`.
    #[schema(value_type = Vec<Object>)]
    pub issues: Vec<Value>,
}

fn localise(mut s: TemplateSummary, locale: Locale) -> TemplateSummary {
    if let Some(d) = s.descriptions.get(locale.as_str()) {
        s.description = Some(d.clone());
    }
    s
}

fn detail(st: &AppState, id: &str, locale: Locale) -> ApiResult<TemplateDetail> {
    let (summary, template) = st.templates.get(id).map_err(store_error)?;
    let issues = lint(&template, st.catalog)
        .iter()
        .map(|i| lint_issue_json(i, locale))
        .collect();
    Ok(TemplateDetail {
        summary: localise(summary, locale),
        template,
        issues,
    })
}

/// Lists / searches templates.
#[utoipa::path(get, path = "/api/templates", tag = "templates", params(TemplateQuery),
    responses((status = 200, description = "Templates", body = TemplateList)))]
pub async fn list(
    State(st): State<AppState>,
    headers: HeaderMap,
    ApiQuery(q): ApiQuery<TemplateQuery>,
) -> Json<TemplateList> {
    let locale = request_locale(&headers);
    let filter = ListFilter {
        q: q.q.filter(|s| !s.trim().is_empty()),
        tags: q
            .tag
            .map(|t| {
                t.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        favorite: q.favorite,
        source: q.source,
    };
    Json(TemplateList {
        templates: st
            .templates
            .list(&filter)
            .into_iter()
            .map(|s| localise(s, locale))
            .collect(),
        recent: st.templates.recent(),
    })
}

/// Creates a template in the user directory (file name from `meta.name`).
#[utoipa::path(post, path = "/api/templates", tag = "templates",
    request_body(content = Object, description = "Template (JSON form of `.wpg.yaml`)"),
    responses(
        (status = 201, description = "Created", body = TemplateDetail),
        (status = 422, description = "Invalid template", body = crate::error::ErrorBody)))]
pub async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    ApiJson(body): ApiJson<Value>,
) -> ApiResult<(StatusCode, Json<TemplateDetail>)> {
    let t = template_from_value(&body)?;
    let id = st.templates.create(&t).map_err(store_error)?;
    Ok((
        StatusCode::CREATED,
        Json(detail(&st, &id, request_locale(&headers))?),
    ))
}

/// One template (also marks it as recently used).
#[utoipa::path(get, path = "/api/templates/{id}", tag = "templates",
    params(("id" = String, Path, description = "Template id")),
    responses(
        (status = 200, description = "Template", body = TemplateDetail),
        (status = 404, description = "Unknown id", body = crate::error::ErrorBody),
        (status = 422, description = "The file does not parse", body = crate::error::ErrorBody)))]
pub async fn get(
    State(st): State<AppState>,
    headers: HeaderMap,
    ApiPath(id): ApiPath<String>,
) -> ApiResult<Json<TemplateDetail>> {
    let d = detail(&st, &id, request_locale(&headers))?;
    st.templates.mark_used(&id);
    Ok(Json(d))
}

/// Replaces a user template.
#[utoipa::path(put, path = "/api/templates/{id}", tag = "templates",
    params(("id" = String, Path, description = "Template id")),
    request_body(content = Object, description = "Template (JSON form of `.wpg.yaml`)"),
    responses(
        (status = 200, description = "Updated", body = TemplateDetail),
        (status = 403, description = "Read-only library template", body = crate::error::ErrorBody),
        (status = 404, description = "Unknown id", body = crate::error::ErrorBody),
        (status = 422, description = "Invalid template", body = crate::error::ErrorBody)))]
pub async fn update(
    State(st): State<AppState>,
    headers: HeaderMap,
    ApiPath(id): ApiPath<String>,
    ApiJson(body): ApiJson<Value>,
) -> ApiResult<Json<TemplateDetail>> {
    let t = template_from_value(&body)?;
    st.templates.update(&id, &t).map_err(store_error)?;
    Ok(Json(detail(&st, &id, request_locale(&headers))?))
}

/// Deletes a user template.
#[utoipa::path(delete, path = "/api/templates/{id}", tag = "templates",
    params(("id" = String, Path, description = "Template id")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 403, description = "Read-only library template", body = crate::error::ErrorBody),
        (status = 404, description = "Unknown id", body = crate::error::ErrorBody)))]
pub async fn delete(
    State(st): State<AppState>,
    ApiPath(id): ApiPath<String>,
) -> ApiResult<StatusCode> {
    st.templates.delete(&id).map_err(store_error)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Copies a template (library or user) into the user directory.
#[utoipa::path(post, path = "/api/templates/{id}/duplicate", tag = "templates",
    params(("id" = String, Path, description = "Template id")),
    responses(
        (status = 201, description = "The copy", body = TemplateDetail),
        (status = 404, description = "Unknown id", body = crate::error::ErrorBody)))]
pub async fn duplicate(
    State(st): State<AppState>,
    headers: HeaderMap,
    ApiPath(id): ApiPath<String>,
) -> ApiResult<(StatusCode, Json<TemplateDetail>)> {
    let new = st.templates.duplicate(&id).map_err(store_error)?;
    Ok((
        StatusCode::CREATED,
        Json(detail(&st, &new, request_locale(&headers))?),
    ))
}

/// Downloads a template as `.wpg.yaml` (the file itself, comments included).
#[utoipa::path(get, path = "/api/templates/{id}/export", tag = "templates",
    params(("id" = String, Path, description = "Template id")),
    responses(
        (status = 200, description = "YAML", body = String, content_type = "text/yaml"),
        (status = 404, description = "Unknown id", body = crate::error::ErrorBody)))]
pub async fn export(
    State(st): State<AppState>,
    ApiPath(id): ApiPath<String>,
) -> ApiResult<Response> {
    let (summary, yaml) = st.templates.export_yaml(&id).map_err(store_error)?;
    st.templates.mark_used(&id);
    let file = summary
        .path
        .rsplit('/')
        .next()
        .map_or_else(|| format!("{id}.wpg.yaml"), str::to_owned)
        .replace(".wpg.json", ".wpg.yaml");
    let mut resp = yaml.into_response();
    let h = resp.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/yaml; charset=utf-8"),
    );
    if let Ok(v) = HeaderValue::from_str(&format!(
        "attachment; filename=\"{}\"",
        file.replace(['"', '\\'], "_")
    )) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    Ok(resp)
}

/// Imports a `.wpg.yaml` (or JSON) document into the user directory.
#[utoipa::path(post, path = "/api/templates/import", tag = "templates",
    request_body(content = String, content_type = "text/yaml", description = "Template YAML"),
    responses(
        (status = 201, description = "Imported", body = TemplateDetail),
        (status = 422, description = "Invalid template", body = crate::error::ErrorBody)))]
pub async fn import(
    State(st): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<(StatusCode, Json<TemplateDetail>)> {
    let text = std::str::from_utf8(&body)
        .map_err(|e| ApiError::bad_request("api-error-invalid-body", e))?;
    let t = Template::from_yaml_str(text).map_err(ApiError::template)?;
    let id = st.templates.create(&t).map_err(store_error)?;
    Ok((
        StatusCode::CREATED,
        Json(detail(&st, &id, request_locale(&headers))?),
    ))
}

/// Body of `POST /api/templates/{id}/favorite` (empty body = toggle).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Favorite {
    /// New value.
    pub favorite: bool,
}

/// `POST /api/templates/{id}/favorite` response.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct FavoriteState {
    /// Template id.
    pub id: String,
    /// Current value.
    pub favorite: bool,
}

/// Sets (or, without a body, toggles) the favourite flag.
#[utoipa::path(post, path = "/api/templates/{id}/favorite", tag = "templates",
    params(("id" = String, Path, description = "Template id")),
    request_body(content = Option<Favorite>, description = "`{\"favorite\": bool}`; empty or `null` = toggle"),
    responses(
        (status = 200, description = "New state", body = FavoriteState),
        (status = 404, description = "Unknown id", body = crate::error::ErrorBody)))]
pub async fn favorite(
    State(st): State<AppState>,
    ApiPath(id): ApiPath<String>,
    body: Bytes,
) -> ApiResult<Json<FavoriteState>> {
    let value = if body.iter().all(u8::is_ascii_whitespace) {
        None
    } else {
        let f: Option<Favorite> = serde_json::from_slice(&body)
            .map_err(|e| ApiError::bad_request("api-error-invalid-body", e))?;
        f.map(|f| f.favorite)
    };
    let favorite = st.templates.set_favorite(&id, value).map_err(store_error)?;
    Ok(Json(FavoriteState { id, favorite }))
}
