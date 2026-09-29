//! Shared helpers of the wpg-server integration tests.

#![allow(dead_code, unreachable_pub)]

use std::path::PathBuf;

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

/// A test application.
pub struct TestApp {
    pub dir: tempfile::TempDir,
    pub state: AppState,
    pub app: Router,
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
        let app = wpg_server::app(state.clone());
        Self { dir, state, app }
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

    pub async fn send(&self, req: Request<Body>) -> Resp {
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
