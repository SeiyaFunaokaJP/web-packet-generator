//! API tokens: CRUD under `/api/tokens` (session only) and `Authorization:
//! Bearer` authentication (no CSRF, revocation, expiry, rate limit).

mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use common::{wait_terminal, TestApp};
use serde_json::{json, Value};

fn bearer(method: Method, uri: &str, token: &str, body: Option<&Value>) -> Request<Body> {
    let b = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"));
    match body {
        Some(v) => b
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(v.to_string())),
        None => b.body(Body::empty()),
    }
    .unwrap()
}

async fn create(app: &TestApp, body: Value) -> (i64, String) {
    let r = app.post("/api/tokens", body).await;
    r.expect(StatusCode::CREATED);
    assert_eq!(r.headers[header::CACHE_CONTROL], "no-store");
    let j = r.json();
    let secret = j["secret"].as_str().unwrap().to_owned();
    assert!(secret.starts_with(j["token"]["prefix"].as_str().unwrap()));
    (j["token"]["id"].as_i64().unwrap(), secret)
}

#[tokio::test]
async fn token_crud_with_a_session() {
    let app = TestApp::new();
    let r = app.get("/api/tokens").await;
    r.expect(StatusCode::OK);
    assert_eq!(r.json(), json!([]));

    let (id, secret) = create(&app, json!({ "name": "ci" })).await;
    assert!(secret.starts_with("wpg_") && secret.len() == 68, "{secret}");
    let (id2, _) = create(&app, json!({ "name": "nightly", "expires_in_days": 30 })).await;

    let list = app.get("/api/tokens").await.json();
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0]["name"], "ci");
    assert_eq!(list[0]["user"], "admin");
    assert_eq!(list[0]["expires"], Value::Null);
    assert!(list[1]["expires"].as_i64().unwrap() > list[1]["created"].as_i64().unwrap());
    // the secret is never listed
    assert!(!app.get("/api/tokens").await.text().contains(&secret));

    // validation
    for body in [
        json!({ "name": "  " }),
        json!({ "name": "x".repeat(101) }),
        json!({ "name": "a", "expires_in_days": 0 }),
        json!({ "name": "a", "expires_in_days": 4000 }),
    ] {
        app.post("/api/tokens", body)
            .await
            .expect_error(StatusCode::BAD_REQUEST, "bad_request");
    }

    // revoke (CSRF still applies to session requests)
    let mut req = Request::delete(format!("/api/tokens/{id2}"))
        .body(Body::empty())
        .unwrap();
    let c = app.login().await;
    req.headers_mut()
        .insert(header::COOKIE, c.cookie().parse().unwrap());
    req.headers_mut()
        .insert("x-csrf-token", "wrong".parse().unwrap());
    app.raw(req)
        .await
        .expect_error(StatusCode::FORBIDDEN, "csrf_failed");
    app.call(Method::DELETE, &format!("/api/tokens/{id2}"), None, None)
        .await
        .expect(StatusCode::NO_CONTENT);
    app.call(Method::DELETE, &format!("/api/tokens/{id2}"), None, None)
        .await
        .expect_error(StatusCode::NOT_FOUND, "not_found");
    let list = app.get("/api/tokens").await.json();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["id"], id);

    // the OpenAPI document describes them
    let doc = app.get("/api/openapi.json").await.json();
    assert!(doc["paths"]["/api/tokens"]["post"].is_object());
    assert!(doc["paths"]["/api/tokens/{id}"]["delete"].is_object());
    assert_eq!(
        doc["components"]["securitySchemes"]["bearerAuth"]["scheme"],
        "bearer"
    );
}

#[tokio::test]
async fn bearer_requests() {
    let app = TestApp::new();
    let (id, secret) = create(&app, json!({ "name": "ci" })).await;

    // valid: reads and writes without CSRF or cookies
    let r = app
        .raw(bearer(Method::GET, "/api/jobs", &secret, None))
        .await;
    r.expect(StatusCode::OK);
    assert!(
        r.headers.get(header::SET_COOKIE).is_none(),
        "no session cookies for tokens"
    );
    let job = json!({
        "streams": [{ "template_id": "examples-arp-sweep" }],
        "backend": "null",
        "count": 10,
    });
    let r = app
        .raw(bearer(Method::POST, "/api/jobs", &secret, Some(&job)))
        .await;
    r.expect(StatusCode::CREATED);
    let jid = r.json()["id"].as_str().unwrap().to_owned();
    let done = wait_terminal(&app, &jid).await;
    assert_eq!(done["status"]["state"], "finished");

    // last use is recorded
    let list = app.get("/api/tokens").await.json();
    assert!(list[0]["last_used"].is_i64(), "{list}");

    // tokens cannot manage tokens
    for (m, uri) in [
        (Method::GET, "/api/tokens".to_owned()),
        (Method::DELETE, format!("/api/tokens/{id}")),
    ] {
        app.raw(bearer(m, &uri, &secret, None))
            .await
            .expect_error(StatusCode::FORBIDDEN, "forbidden");
    }
    app.raw(bearer(
        Method::POST,
        "/api/tokens",
        &secret,
        Some(&json!({ "name": "x" })),
    ))
    .await
    .expect_error(StatusCode::FORBIDDEN, "forbidden");

    // a bearer header wins over a session cookie: a wrong token is not rescued
    let c = app.login().await;
    let mut req = bearer(Method::GET, "/api/jobs", "wpg_nope", None);
    req.headers_mut()
        .insert(header::COOKIE, c.cookie().parse().unwrap());
    app.raw(req)
        .await
        .expect_error(StatusCode::UNAUTHORIZED, "unauthorized");

    // revoked
    app.call(Method::DELETE, &format!("/api/tokens/{id}"), None, None)
        .await
        .expect(StatusCode::NO_CONTENT);
    app.raw(bearer(Method::GET, "/api/jobs", &secret, None))
        .await
        .expect_error(StatusCode::UNAUTHORIZED, "unauthorized");
}

#[tokio::test]
async fn expired_and_wrong_tokens() {
    let app = TestApp::new();
    app.login().await;
    let db = app.state.auth.db();
    let admin = db.user_by_name("admin").unwrap().unwrap();
    // created 10 days ago with a 1-day expiry
    let now = wpg_server::auth::now_secs();
    let (_, expired) =
        wpg_server::auth::token::create(db, &admin, "old", Some(1), now - 10 * 86_400).unwrap();
    app.raw(bearer(Method::GET, "/api/jobs", &expired, None))
        .await
        .expect_error(StatusCode::UNAUTHORIZED, "unauthorized");

    // public endpoints ignore the header
    app.raw(bearer(Method::GET, "/api/health", "garbage", None))
        .await
        .expect(StatusCode::OK);

    // wrong tokens are rate limited per client address, like logins
    let wrong = format!("wpg_{}", "0".repeat(64));
    let mut limited = false;
    for _ in 0..10 {
        let r = app
            .raw(bearer(Method::GET, "/api/jobs", &wrong, None))
            .await;
        if r.status == StatusCode::TOO_MANY_REQUESTS {
            let j = r.expect_error(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
            assert!(j["error"]["details"]["retry_after_secs"].as_u64().unwrap() >= 1);
            limited = true;
            break;
        }
        r.expect_error(StatusCode::UNAUTHORIZED, "unauthorized");
    }
    assert!(limited, "failures must be rate limited");
    // ... which also blocks a valid token from that address for a while
    let (_, good) = create(&app, json!({ "name": "good" })).await;
    app.raw(bearer(Method::GET, "/api/jobs", &good, None))
        .await
        .expect_error(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
    // other schemes are rejected, not ignored
    let mut req = Request::get("/api/jobs").body(Body::empty()).unwrap();
    req.headers_mut()
        .insert(header::AUTHORIZATION, "Basic YWRtaW46eA==".parse().unwrap());
    let r = app.raw(req).await;
    assert!(
        r.status == StatusCode::UNAUTHORIZED || r.status == StatusCode::TOO_MANY_REQUESTS,
        "{}",
        r.status
    );
}
