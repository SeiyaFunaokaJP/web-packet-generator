//! Authentication, CSRF, rate limiting, security headers, the localhost rule and
//! TLS (TODO §2.6).

mod common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Method, Request, StatusCode};
use common::{Creds, TestApp, TEST_PASSWORD};
use serde_json::{json, Value};
use wpg_server::config::Secret;
use wpg_server::{BindError, ServeOptions, Server};

fn json_req(method: Method, uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn with_creds(mut req: Request<Body>, c: &Creds, csrf: bool) -> Request<Body> {
    req.headers_mut()
        .insert(header::COOKIE, c.cookie().parse().unwrap());
    if csrf {
        req.headers_mut()
            .insert("x-csrf-token", c.csrf.parse().unwrap());
    }
    req
}

fn setup_body(p: &str, c: &str) -> Value {
    json!({ "password": p, "confirm": c })
}

fn null_job() -> Value {
    json!({
        "streams": [{ "template_id": "examples-arp-sweep" }],
        "backend": "null",
        "count": 10,
    })
}

async fn status(app: &TestApp, creds: Option<&Creds>) -> Value {
    let mut req = Request::get("/api/auth/status")
        .body(Body::empty())
        .unwrap();
    if let Some(c) = creds {
        req = with_creds(req, c, false);
    }
    let r = app.raw(req).await;
    r.expect(StatusCode::OK);
    assert_eq!(r.headers[header::CACHE_CONTROL], "no-store");
    r.json()
}

#[tokio::test]
async fn first_run_setup_flow() {
    let app = TestApp::anonymous();
    let s = status(&app, None).await;
    assert_eq!(
        s,
        json!({
            "configured": false,
            "authenticated": false,
            "setup_required": true,
            "user": null,
            "csrf_token": null,
        })
    );
    // everything protected answers setup_required; public endpoints and the UI work
    app.get("/api/jobs")
        .await
        .expect_error(StatusCode::UNAUTHORIZED, "setup_required");
    app.get("/api/nope")
        .await
        .expect_error(StatusCode::UNAUTHORIZED, "setup_required");
    for p in [
        "/api/health",
        "/api/version",
        "/api/openapi.json",
        "/",
        "/jobs",
    ] {
        app.get(p).await.expect(StatusCode::OK);
    }
    app.raw(json_req(
        Method::POST,
        "/api/auth/login",
        &json!({ "password": "x" }),
    ))
    .await
    .expect_error(StatusCode::UNAUTHORIZED, "setup_required");

    // validation (localised)
    let r = app
        .raw(json_req(
            Method::POST,
            "/api/auth/setup",
            &setup_body("password-1", "password-2"),
        ))
        .await;
    r.expect_error(StatusCode::BAD_REQUEST, "bad_request");
    let mut req = json_req(
        Method::POST,
        "/api/auth/setup",
        &setup_body("short", "short"),
    );
    req.headers_mut()
        .insert(header::ACCEPT_LANGUAGE, "ja".parse().unwrap());
    let j = app
        .raw(req)
        .await
        .expect_error(StatusCode::BAD_REQUEST, "bad_request");
    assert!(
        j["error"]["message"].as_str().unwrap().contains("8 文字"),
        "{j}"
    );
    // no form posts
    let r = app
        .raw(
            Request::post("/api/auth/setup")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from("password=abcdefgh&confirm=abcdefgh"))
                .unwrap(),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert!(app.state.auth.setup_required());

    // setup logs in
    let r = app
        .raw(json_req(
            Method::POST,
            "/api/auth/setup",
            &setup_body(TEST_PASSWORD, TEST_PASSWORD),
        ))
        .await;
    r.expect(StatusCode::NO_CONTENT);
    let cookies: Vec<&str> = r
        .headers
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap())
        .collect();
    let session = cookies
        .iter()
        .find(|c| c.starts_with("wpg_session="))
        .unwrap();
    assert!(session.contains("HttpOnly") && session.contains("SameSite=Strict"));
    assert!(!session.contains("Secure"), "plain HTTP");
    let csrf = cookies.iter().find(|c| c.starts_with("wpg_csrf=")).unwrap();
    assert!(!csrf.contains("HttpOnly"), "readable by the SPA");
    let creds = Creds::from_headers(&r.headers).unwrap();
    assert_eq!(creds.session.len(), 64);

    let s = status(&app, Some(&creds)).await;
    assert_eq!(s["configured"], true);
    assert_eq!(s["authenticated"], true);
    assert_eq!(s["setup_required"], false);
    assert_eq!(s["user"], json!({ "name": "admin", "role": "admin" }));
    assert_eq!(s["csrf_token"], creds.csrf.as_str());
    // anonymous status after setup
    let s = status(&app, None).await;
    assert_eq!(s["configured"], true);
    assert_eq!(s["authenticated"], false);
    assert_eq!(s["csrf_token"], Value::Null);

    // setup only once
    app.raw(json_req(
        Method::POST,
        "/api/auth/setup",
        &setup_body("another-password", "another-password"),
    ))
    .await
    .expect_error(StatusCode::CONFLICT, "already_configured");

    // the session works
    app.raw(with_creds(
        Request::get("/api/jobs").body(Body::empty()).unwrap(),
        &creds,
        false,
    ))
    .await
    .expect(StatusCode::OK);
}

#[tokio::test]
async fn sessions_csrf_and_logout() {
    let app = TestApp::anonymous_with(|c| c.auth.password = Some(Secret::new(TEST_PASSWORD)));
    assert!(!app.state.auth.setup_required());
    assert_eq!(status(&app, None).await["setup_required"], false);
    // a configured password disables the setup screen
    app.raw(json_req(
        Method::POST,
        "/api/auth/setup",
        &setup_body("password-x", "password-x"),
    ))
    .await
    .expect_error(StatusCode::CONFLICT, "already_configured");

    // unauthenticated
    app.get("/api/jobs")
        .await
        .expect_error(StatusCode::UNAUTHORIZED, "unauthorized");
    app.post("/api/jobs", null_job())
        .await
        .expect_error(StatusCode::UNAUTHORIZED, "unauthorized");
    let bogus = Creds {
        session: "0".repeat(64),
        csrf: "x".into(),
    };
    app.raw(with_creds(
        Request::get("/api/jobs").body(Body::empty()).unwrap(),
        &bogus,
        true,
    ))
    .await
    .expect_error(StatusCode::UNAUTHORIZED, "unauthorized");

    // wrong password / unknown user
    app.raw(json_req(
        Method::POST,
        "/api/auth/login",
        &json!({ "password": "wrong" }),
    ))
    .await
    .expect_error(StatusCode::UNAUTHORIZED, "unauthorized");
    app.raw(json_req(
        Method::POST,
        "/api/auth/login",
        &json!({ "password": TEST_PASSWORD, "username": "root" }),
    ))
    .await
    .expect_error(StatusCode::UNAUTHORIZED, "unauthorized");

    let creds = app.login().await;
    // GET needs no CSRF token, mutations do
    app.raw(with_creds(
        Request::get("/api/jobs").body(Body::empty()).unwrap(),
        &creds,
        false,
    ))
    .await
    .expect(StatusCode::OK);
    app.raw(with_creds(
        json_req(Method::POST, "/api/jobs", &null_job()),
        &creds,
        false,
    ))
    .await
    .expect_error(StatusCode::FORBIDDEN, "csrf_failed");
    let mut req = with_creds(
        json_req(Method::POST, "/api/jobs", &null_job()),
        &creds,
        false,
    );
    req.headers_mut()
        .insert("x-csrf-token", "0".repeat(64).parse().unwrap());
    app.raw(req)
        .await
        .expect_error(StatusCode::FORBIDDEN, "csrf_failed");
    let r = app
        .raw(with_creds(
            json_req(Method::POST, "/api/jobs", &null_job()),
            &creds,
            true,
        ))
        .await;
    r.expect(StatusCode::CREATED);
    let id = r.json()["id"].as_str().unwrap().to_owned();
    app.raw(with_creds(
        Request::delete(format!("/api/jobs/{id}"))
            .body(Body::empty())
            .unwrap(),
        &creds,
        false,
    ))
    .await
    .expect_error(StatusCode::FORBIDDEN, "csrf_failed");

    // logout needs the CSRF token, then invalidates the session
    app.raw(with_creds(
        Request::post("/api/auth/logout")
            .body(Body::empty())
            .unwrap(),
        &creds,
        false,
    ))
    .await
    .expect_error(StatusCode::FORBIDDEN, "csrf_failed");
    let r = app
        .raw(with_creds(
            Request::post("/api/auth/logout")
                .body(Body::empty())
                .unwrap(),
            &creds,
            true,
        ))
        .await;
    r.expect(StatusCode::NO_CONTENT);
    assert!(r
        .headers
        .get_all(header::SET_COOKIE)
        .iter()
        .all(|v| v.to_str().unwrap().contains("Max-Age=0")));
    app.raw(with_creds(
        Request::get("/api/jobs").body(Body::empty()).unwrap(),
        &creds,
        true,
    ))
    .await
    .expect_error(StatusCode::UNAUTHORIZED, "unauthorized");
    assert_eq!(status(&app, Some(&creds)).await["authenticated"], false);
    // logging out twice is harmless
    app.raw(
        Request::post("/api/auth/logout")
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .expect(StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn login_rate_limit() {
    let app = TestApp::anonymous_with(|c| c.auth.password = Some(Secret::new(TEST_PASSWORD)));
    let from = |ip: [u8; 4], password: &str| {
        let mut req = json_req(
            Method::POST,
            "/api/auth/login",
            &json!({ "password": password }),
        );
        req.extensions_mut()
            .insert(ConnectInfo(SocketAddr::from((ip, 40000))));
        req
    };
    for _ in 0..5 {
        app.raw(from([192, 0, 2, 1], "wrong"))
            .await
            .expect_error(StatusCode::UNAUTHORIZED, "unauthorized");
    }
    // locked out, even with the right password
    for pw in ["wrong", TEST_PASSWORD] {
        let r = app.raw(from([192, 0, 2, 1], pw)).await;
        let j = r.expect_error(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
        let secs = j["error"]["details"]["retry_after_secs"].as_u64().unwrap();
        assert!((1..=60).contains(&secs), "{j}");
        assert_eq!(r.headers[header::RETRY_AFTER], secs.to_string());
    }
    // other clients are not affected
    app.raw(from([192, 0, 2, 2], TEST_PASSWORD))
        .await
        .expect(StatusCode::NO_CONTENT);
    // every attempt is audited in the database
    assert_eq!(
        app.state.db.failed_attempts_since("192.0.2.1", 0).unwrap(),
        5
    );
}

#[tokio::test]
async fn setup_only_from_localhost_unless_insecure() {
    let app = TestApp::anonymous();
    let mut req = json_req(
        Method::POST,
        "/api/auth/setup",
        &setup_body(TEST_PASSWORD, TEST_PASSWORD),
    );
    req.extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([192, 0, 2, 9], 1234))));
    app.raw(req)
        .await
        .expect_error(StatusCode::FORBIDDEN, "forbidden");
    app.state.auth.set_allow_insecure(true);
    let mut req = json_req(
        Method::POST,
        "/api/auth/setup",
        &setup_body(TEST_PASSWORD, TEST_PASSWORD),
    );
    req.extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([192, 0, 2, 9], 1234))));
    app.raw(req).await.expect(StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn sessions_expire_and_end_on_password_change() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = common::config(&dir);
    c.auth.password = Some(Secret::new(TEST_PASSWORD));
    c.auth.session_ttl_secs = 1;
    let app = TestApp::from_state(
        tempfile::tempdir().unwrap(),
        wpg_server::AppState::new(c.clone()).unwrap(),
    );
    let creds = app.login().await;
    let get = || {
        with_creds(
            Request::get("/api/jobs").body(Body::empty()).unwrap(),
            &creds,
            false,
        )
    };
    app.raw(get()).await.expect(StatusCode::OK);
    tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
    app.raw(get())
        .await
        .expect_error(StatusCode::UNAUTHORIZED, "unauthorized");

    // restart with the same password: sessions survive
    c.auth.session_ttl_secs = 3600;
    let app = TestApp::from_state(
        tempfile::tempdir().unwrap(),
        wpg_server::AppState::new(c.clone()).unwrap(),
    );
    let creds = app.login().await;
    drop(app);
    let app = TestApp::from_state(
        tempfile::tempdir().unwrap(),
        wpg_server::AppState::new(c.clone()).unwrap(),
    );
    app.raw(with_creds(
        Request::get("/api/jobs").body(Body::empty()).unwrap(),
        &creds,
        false,
    ))
    .await
    .expect(StatusCode::OK);
    // a new password ends them
    drop(app);
    c.auth.password = Some(Secret::new("a-new-password"));
    let app = TestApp::from_state(
        tempfile::tempdir().unwrap(),
        wpg_server::AppState::new(c).unwrap(),
    );
    app.raw(with_creds(
        Request::get("/api/jobs").body(Body::empty()).unwrap(),
        &creds,
        false,
    ))
    .await
    .expect_error(StatusCode::UNAUTHORIZED, "unauthorized");
}

#[tokio::test]
async fn security_headers() {
    let app = TestApp::new();
    for path in ["/", "/api/health", "/api/jobs"] {
        let r = app.get(path).await;
        r.expect(StatusCode::OK);
        let csp = r.headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
        assert!(csp.contains("default-src 'self'"), "{csp}");
        assert!(csp.contains("'wasm-unsafe-eval'"), "{csp}");
        assert!(csp.contains("connect-src 'self' ws: wss:"), "{csp}");
        assert!(csp.contains("frame-ancestors 'none'"), "{csp}");
        assert!(!csp.contains("script-src 'self' 'unsafe-inline'"), "{csp}");
        assert_eq!(r.headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert_eq!(r.headers[header::X_FRAME_OPTIONS], "DENY");
        assert_eq!(r.headers[header::REFERRER_POLICY], "no-referrer");
        assert!(r.headers.contains_key("permissions-policy"));
    }
    // Swagger UI gets its own, relaxed policy
    let r = app.get("/api/docs/").await;
    r.expect(StatusCode::OK);
    let csp = r.headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert!(csp.contains("script-src 'self' 'unsafe-inline'"), "{csp}");
}

#[tokio::test]
async fn job_history_survives_restarts() {
    let dir = tempfile::tempdir().unwrap();
    let c = common::config(&dir);
    let id = {
        let app = TestApp::from_state(
            tempfile::tempdir().unwrap(),
            wpg_server::AppState::new(c.clone()).unwrap(),
        );
        let r = app.post("/api/jobs", null_job()).await;
        r.expect(StatusCode::CREATED);
        let id = r.json()["id"].as_str().unwrap().to_owned();
        common::wait_terminal(&app, &id).await;
        id
    };
    let state = wpg_server::AppState::new(c).unwrap();
    assert_eq!(state.jobs.history()[0].id, id);
    assert_eq!(state.jobs.get(&id).unwrap().id, id);
    // the setup user persisted too
    assert!(!state.auth.setup_required());
}

// ---------------------------------------------------------------- real sockets

#[tokio::test]
async fn localhost_only_without_password() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = common::config(&dir);
    c.server.bind = "0.0.0.0:0".parse().unwrap();
    match Server::bind(c.clone(), ServeOptions::default()).await {
        Err(BindError::Insecure(addr)) => assert_eq!(addr, c.server.bind),
        other => panic!("expected a refusal, got {other:?}"),
    }
    // --allow-insecure
    let s = Server::bind(
        c.clone(),
        ServeOptions {
            allow_insecure: true,
            ..ServeOptions::default()
        },
    )
    .await
    .unwrap();
    drop(s);
    // loopback is fine
    let mut l = c.clone();
    l.server.bind = "127.0.0.1:0".parse().unwrap();
    drop(Server::bind(l, ServeOptions::default()).await.unwrap());
    // with a password any address is fine
    c.auth.password = Some(Secret::new(TEST_PASSWORD));
    drop(Server::bind(c, ServeOptions::default()).await.unwrap());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn https_with_self_signed_certificate() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = common::config(&dir);
    c.server.self_signed_tls = true;
    let server = Server::bind(c, ServeOptions::default()).await.unwrap();
    let tls = server.tls().unwrap();
    assert!(tls.self_signed && tls.generated);
    assert!(dir.path().join("data/tls/cert.pem").is_file());
    let addr = server.local_addr().unwrap();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let serving = tokio::spawn(server.run_until(async {
        let _ = stop_rx.await;
    }));
    let base = format!("https://{addr}");

    // plain HTTP is not answered
    assert!(reqwest::get(format!("http://{addr}/api/health"))
        .await
        .is_err());

    let anon = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    let s: Value = anon
        .get(format!("{base}/api/auth/status"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(s["setup_required"], true);

    let setup = anon
        .post(format!("{base}/api/auth/setup"))
        .json(&setup_body(TEST_PASSWORD, TEST_PASSWORD))
        .send()
        .await
        .unwrap();
    assert_eq!(setup.status(), 204);
    assert!(setup
        .headers()
        .get_all("set-cookie")
        .iter()
        .all(|v| v.to_str().unwrap().contains("; Secure")));

    // log in again with a cookie jar and drive a job
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    let r = client
        .post(format!("{base}/api/auth/login"))
        .json(&json!({ "password": TEST_PASSWORD }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 204);
    let csrf = Creds::from_headers(r.headers()).unwrap().csrf;
    let r = client
        .post(format!("{base}/api/jobs"))
        .header("x-csrf-token", &csrf)
        .json(&null_job())
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 201, "{:?}", r.text().await);
    let r = client
        .post(format!("{base}/api/auth/logout"))
        .header("x-csrf-token", &csrf)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 204);
    let r = client.get(format!("{base}/api/jobs")).send().await.unwrap();
    assert_eq!(r.status(), 401);

    stop_tx.send(()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(10), serving)
        .await
        .expect("server did not stop")
        .unwrap()
        .unwrap();
}
