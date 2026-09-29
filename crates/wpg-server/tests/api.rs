//! Integration tests for the HTTP API, driven through `tower::ServiceExt::oneshot`
//! (no socket needed).

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

async fn get(uri: &str, accept_language: Option<&str>) -> (StatusCode, Option<String>, Vec<u8>) {
    let mut req = Request::builder().uri(uri);
    if let Some(lang) = accept_language {
        req = req.header(header::ACCEPT_LANGUAGE, lang);
    }
    let resp = wpg_server::router()
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let ctype = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap().to_owned());
    let body = to_bytes(resp.into_body(), 1 << 20).await.unwrap().to_vec();
    (status, ctype, body)
}

#[tokio::test]
async fn health_is_ok() {
    let (status, ctype, body) = get("/api/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ctype.as_deref(), Some("application/json"));
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(json["locale"], "en");
    assert_eq!(json["message"], "Service is healthy");
}

#[tokio::test]
async fn health_is_localised() {
    let (status, _, body) = get("/api/health", Some("ja-JP,ja;q=0.9,en;q=0.8")).await;
    assert_eq!(status, StatusCode::OK);
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["locale"], "ja");
    assert_eq!(json["message"], "サービスは正常です");
}

#[tokio::test]
async fn protocols_lists_catalog() {
    let (status, _, body) = get("/api/protocols", None).await;
    assert_eq!(status, StatusCode::OK);
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json[0]["id"], "ethernet2");
    assert_eq!(json[0]["fields"][2]["name"], "ethertype");
}

#[tokio::test]
async fn unknown_api_is_json_404() {
    let (status, _, body) = get("/api/nope", Some("ja")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "見つかりません");
}

#[tokio::test]
async fn root_serves_html() {
    let (status, ctype, body) = get("/", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(ctype.unwrap().starts_with("text/html"));
    assert!(String::from_utf8(body).unwrap().contains("<html"));
}
