//! `POST /api/protocols/reload`: the server reads its protocol directories
//! again (own test binary: it switches the process-wide catalog).

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::TestApp;
use serde_json::json;
use wpg_server::events::ServerEvent;

const ACME: &str = "protocols:
  - id: acme_tlm
    name: ACME Telemetry
    layer: l7
    fields:
      - name: version
        type: { kind: uint, bits: 8 }
        default: { uint: 1 }
bind:
  - under: udp
    value: 47000
    protocol: acme_tlm
";

fn ids(app_json: &serde_json::Value) -> Vec<String> {
    app_json
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn reload_switches_the_catalog() {
    let protos = tempfile::tempdir().unwrap();
    std::fs::write(protos.path().join("acme.yaml"), ACME).unwrap();
    wpg_protocols::extension::install_dirs(&[protos.path()]).unwrap();

    let app = TestApp::new();
    let mut events = app.state.subscribe();
    let list = app.get("/api/protocols").await;
    assert!(ids(&list.json()).contains(&"acme_tlm".to_owned()));

    // Mutating: needs a session and the CSRF token.
    let anon = TestApp::anonymous();
    anon.post("/api/protocols/reload", json!({}))
        .await
        .expect(StatusCode::UNAUTHORIZED);
    let c = app.login().await;
    let req = Request::post("/api/protocols/reload")
        .header("cookie", c.cookie())
        .body(Body::empty())
        .unwrap();
    app.raw(req)
        .await
        .expect_error(StatusCode::FORBIDDEN, "csrf_failed");

    // A new file and a renamed protocol show up after the reload.
    std::fs::write(
        protos.path().join("acme.yaml"),
        ACME.replace("acme_tlm", "acme_v2"),
    )
    .unwrap();
    std::fs::write(
        protos.path().join("more.yml"),
        "protocols:\n  - id: more_proto\n    name: More\n    layer: l7\n    fields: []\n",
    )
    .unwrap();
    let r = app.post("/api/protocols/reload", json!({})).await;
    r.expect(StatusCode::OK);
    let body = r.json();
    let files = body["files"].as_array().unwrap();
    assert_eq!(files.len(), 2, "{body}");
    assert_eq!(files[0]["protocols"], json!(["acme_v2"]));
    assert_eq!(files[0]["bindings"], 1);
    assert_eq!(body["dirs"].as_array().unwrap().len(), 1);
    let got = ids(&app.get("/api/protocols").await.json());
    assert!(got.contains(&"acme_v2".to_owned()) && got.contains(&"more_proto".to_owned()));
    assert!(!got.contains(&"acme_tlm".to_owned()));
    app.get("/api/protocols/acme_v2")
        .await
        .expect(StatusCode::OK);
    let exts = app.get("/api/protocols/extensions").await.json();
    assert_eq!(exts.as_array().unwrap().len(), 2);
    let ev = events.try_recv().expect("protocols_changed event");
    assert!(matches!(ev, ServerEvent::ProtocolsChanged), "{ev:?}");

    // A broken file: 422 with the file and reason, nothing changes.
    std::fs::write(protos.path().join("zz.yaml"), "protcols: []\n").unwrap();
    let r = app.post("/api/protocols/reload", json!({})).await;
    let err = r.expect_error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_protocols");
    let file = err["error"]["details"]["file"].as_str().unwrap();
    assert!(file.ends_with("zz.yaml"), "{err}");
    assert!(
        err["error"]["message"]
            .as_str()
            .unwrap()
            .contains("zz.yaml"),
        "{err}"
    );
    let got = ids(&app.get("/api/protocols").await.json());
    assert!(got.contains(&"acme_v2".to_owned()));
    assert!(events.try_recv().is_err());
}
