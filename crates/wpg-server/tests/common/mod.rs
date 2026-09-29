//! Shared helpers of the wpg-server integration tests.

#![allow(dead_code, unreachable_pub)]

use std::path::PathBuf;
use std::sync::Mutex;

use axum::body::{to_bytes, Body};
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use serde_json::Value;
use tower::ServiceExt;
use wpg_server::{AppState, Config};

/// The bundled template library (`templates/` at the repository root).
pub fn repo_templates() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../templates")
}

/// A configuration with a temporary data dir and the bundled library.
pub fn config(dir: &tempfile::TempDir) -> Config {
    let mut c = Config {
        data_dir: dir.path().join("data"),
        ..Config::default()
    };
    c.templates.library_paths = vec![repo_templates()];
    c.server.bind = "127.0.0.1:0".parse().unwrap();
    c
}

/// Admin password the test helpers set on the setup screen.
pub const TEST_PASSWORD: &str = "test-password-1";

/// Session cookie + CSRF token of a logged-in client.
#[derive(Debug, Clone)]
pub struct Creds {
    pub session: String,
    pub csrf: String,
}

impl Creds {
    /// From the `Set-Cookie` headers of a login / setup response.
    pub fn from_headers(h: &axum::http::HeaderMap) -> Option<Self> {
        let mut session = None;
        let mut csrf = None;
        for v in h.get_all(header::SET_COOKIE) {
            let v = v.to_str().unwrap();
            let (kv, _) = v.split_once(';').unwrap_or((v, ""));
            match kv.split_once('=') {
                Some(("wpg_session", x)) if !x.is_empty() => session = Some(x.to_owned()),
                Some(("wpg_csrf", x)) if !x.is_empty() => csrf = Some(x.to_owned()),
                _ => {}
            }
        }
        Some(Self {
            session: session?,
            csrf: csrf?,
        })
    }

    pub fn cookie(&self) -> String {
        format!("wpg_session={}; wpg_csrf={}", self.session, self.csrf)
    }
}

/// A test application. Unless created with [`TestApp::anonymous`], the first
/// request completes the first-run setup (which logs in) and every request then
/// carries the session cookie and the CSRF token, so the API tests run with
/// authentication enforced.
pub struct TestApp {
    pub dir: tempfile::TempDir,
    pub state: AppState,
    pub app: Router,
    pub anonymous: bool,
    creds: Mutex<Option<Creds>>,
}

impl TestApp {
    pub fn new() -> Self {
        Self::with(|_| {})
    }

    pub fn with(f: impl FnOnce(&mut Config)) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mut c = config(&dir);
        f(&mut c);
        let state = AppState::new(c).unwrap();
        Self::from_state(dir, state)
    }

    pub fn from_state(dir: tempfile::TempDir, state: AppState) -> Self {
        let app = wpg_server::app(state.clone());
        Self {
            dir,
            state,
            app,
            anonymous: false,
            creds: Mutex::new(None),
        }
    }

    /// Requests without automatic setup / login / cookies.
    pub fn anonymous() -> Self {
        Self::anonymous_with(|_| {})
    }

    pub fn anonymous_with(f: impl FnOnce(&mut Config)) -> Self {
        let mut a = Self::with(f);
        a.anonymous = true;
        a
    }

    /// Completes the setup screen (or logs in when a password exists) and
    /// returns the credentials.
    pub async fn login(&self) -> Creds {
        if let Some(c) = self.creds.lock().unwrap().clone() {
            return c;
        }
        let (uri, body) = if self.state.auth.setup_required() {
            (
                "/api/auth/setup",
                serde_json::json!({ "password": TEST_PASSWORD, "confirm": TEST_PASSWORD }),
            )
        } else {
            (
                "/api/auth/login",
                serde_json::json!({ "password": TEST_PASSWORD }),
            )
        };
        let req = Request::post(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let r = self.raw(req).await;
        r.expect(StatusCode::NO_CONTENT);
        let c = Creds::from_headers(&r.headers).expect("session cookies");
        *self.creds.lock().unwrap() = Some(c.clone());
        c
    }

    pub async fn call(
        &self,
        method: Method,
        uri: &str,
        body: Option<Value>,
        lang: Option<&str>,
    ) -> Resp {
        let mut req = Request::builder().method(method).uri(uri);
        if let Some(l) = lang {
            req = req.header(header::ACCEPT_LANGUAGE, l);
        }
        let body = match body {
            Some(v) => {
                req = req.header(header::CONTENT_TYPE, "application/json");
                Body::from(serde_json::to_vec(&v).unwrap())
            }
            None => Body::empty(),
        };
        self.send(req.body(body).unwrap()).await
    }

    /// Sends `req` with the test credentials (unless anonymous / already set).
    pub async fn send(&self, mut req: Request<Body>) -> Resp {
        if !self.anonymous && req.uri().path().starts_with("/api/") {
            let c = self.login().await;
            let h = req.headers_mut();
            if !h.contains_key(header::COOKIE) {
                h.insert(header::COOKIE, c.cookie().parse().unwrap());
            }
            if !h.contains_key("x-csrf-token") {
                h.insert("x-csrf-token", c.csrf.parse().unwrap());
            }
        }
        self.raw(req).await
    }

    /// Sends `req` as is.
    pub async fn raw(&self, req: Request<Body>) -> Resp {
        let resp = self.app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let headers = resp.headers().clone();
        let bytes = to_bytes(resp.into_body(), 64 << 20).await.unwrap().to_vec();
        Resp {
            status,
            headers,
            bytes,
        }
    }

    pub async fn get(&self, uri: &str) -> Resp {
        self.call(Method::GET, uri, None, None).await
    }

    pub async fn post(&self, uri: &str, body: Value) -> Resp {
        self.call(Method::POST, uri, Some(body), None).await
    }
}

pub struct Resp {
    pub status: StatusCode,
    pub headers: axum::http::HeaderMap,
    pub bytes: Vec<u8>,
}

impl Resp {
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.bytes)
            .unwrap_or_else(|e| panic!("not JSON ({e}): {}", String::from_utf8_lossy(&self.bytes)))
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }

    pub fn content_type(&self) -> String {
        self.headers
            .get(header::CONTENT_TYPE)
            .map(|v| v.to_str().unwrap().to_owned())
            .unwrap_or_default()
    }

    #[track_caller]
    pub fn expect(&self, status: StatusCode) -> &Self {
        assert_eq!(self.status, status, "body: {}", self.text());
        self
    }

    /// Asserts an error response with `code`.
    #[track_caller]
    pub fn expect_error(&self, status: StatusCode, code: &str) -> Value {
        self.expect(status);
        let j = self.json();
        assert_eq!(j["error"]["code"], code, "{j}");
        assert!(j["error"]["message"].is_string(), "{j}");
        j
    }
}

/// Polls a job until it is terminal (5 s timeout).
pub async fn wait_terminal(app: &TestApp, id: &str) -> Value {
    for _ in 0..500 {
        let j = app.get(&format!("/api/jobs/{id}")).await.json();
        let s = j["status"]["state"].as_str().unwrap().to_owned();
        if s == "finished" || s == "failed" {
            return j;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("job {id} did not finish");
}

/// A reqwest client (cookie store, self-signed certificates accepted) that
/// completed the setup screen of the server at `base`; returns it with the CSRF
/// token and the cookie header value.
pub async fn http_login(base: &str) -> (reqwest::Client, String, String) {
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    let r = client
        .post(format!("{base}/api/auth/setup"))
        .json(&serde_json::json!({ "password": TEST_PASSWORD, "confirm": TEST_PASSWORD }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 204, "{:?}", r.text().await);
    let creds = Creds::from_headers(r.headers()).unwrap();
    let status: Value = client
        .get(format!("{base}/api/auth/status"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(status["csrf_token"], creds.csrf.as_str());
    (client, creds.csrf.clone(), creds.cookie())
}
