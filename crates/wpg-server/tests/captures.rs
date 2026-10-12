//! `/api/captures`: a signed dry run is captured back from its pcapng file,
//! verified, kept, saved and downloaded; filter checks and error cases.

mod common;

use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

use common::{wait_terminal, TestApp};

async fn wait_capture(app: &TestApp, id: &str) -> Value {
    for _ in 0..500 {
        let j = app.get(&format!("/api/captures/{id}")).await.json();
        if j["status"]["state"] != "running" {
            return j;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("capture {id} did not finish");
}

fn udp_template() -> Value {
    serde_yaml::from_str(include_str!(
        "../../../templates/examples/eth-ipv4-udp-range.wpg.yaml"
    ))
    .unwrap()
}

#[tokio::test]
async fn signed_dry_run_is_verified() {
    let app = TestApp::new();
    let r = app
        .post(
            "/api/jobs",
            json!({
                "streams": [{ "template": udp_template() }],
                "dry_run": "signed",
                "count": 200,
                "rate": "unlimited",
                "signature": true,
                "stream_id": 7,
            }),
        )
        .await;
    r.expect(StatusCode::CREATED);
    let job = r.json();
    assert_eq!(job["spec"]["signature"], true);
    let done = wait_terminal(&app, job["id"].as_str().unwrap()).await;
    assert_eq!(done["report"]["stats"]["packets"], 200, "{done}");
    assert_eq!(done["report"]["stats"]["unsigned"], 0, "{done}");

    let r = app
        .post(
            "/api/captures",
            json!({
                "backend": "pcapfile",
                "iface": "signed.pcapng",
                "filter": "udp  and  wpg",
                "keep": 3,
                "save": "rx-copy",
                "verify": true,
            }),
        )
        .await;
    r.expect(StatusCode::CREATED);
    let cap = r.json();
    assert_eq!(cap["filter"], "udp  and  wpg");
    let id = cap["id"].as_str().unwrap().to_owned();
    let done = wait_capture(&app, &id).await;
    assert_eq!(done["status"]["state"], "finished", "{done}");
    assert_eq!(done["status"]["detail"], "end_of_input");
    assert_eq!(done["saved"], true);
    let s = &done["stats"];
    assert_eq!(
        (s["packets"].as_u64(), s["matched"].as_u64()),
        (Some(200), Some(200))
    );
    assert_eq!(s["saved"], 200);
    let st = &s["verify"]["streams"][0];
    assert_eq!(st["stream_id"], 7);
    assert_eq!(
        (st["received"].as_u64(), st["lost"].as_u64()),
        (Some(200), Some(0)),
        "{st}"
    );
    assert_eq!(st["out_of_order"], 0);

    let pk = app.get(&format!("/api/captures/{id}/packets")).await.json();
    let pk = pk.as_array().unwrap();
    assert_eq!(pk.len(), 3);
    assert_eq!(pk[1]["index"], 1);
    assert_eq!(pk[1]["signature"]["seq"], 1);
    assert!(pk[0]["hex"].as_str().unwrap().starts_with("ffffffffffff"));

    let r = app.get(&format!("/api/captures/{id}/pcapng")).await;
    r.expect(StatusCode::OK);
    assert_eq!(&r.bytes[..4], &[0x0a, 0x0d, 0x0d, 0x0a]);
    assert!(app.dir.path().join("data/captures/rx-copy.pcapng").exists());

    let list = app.get("/api/captures").await.json();
    assert_eq!(list.as_array().unwrap().len(), 1);
    app.post(&format!("/api/captures/{id}/stop"), json!(null))
        .await
        .expect_error(StatusCode::CONFLICT, "invalid_state");
    app.call(Method::DELETE, &format!("/api/captures/{id}"), None, None)
        .await
        .expect(StatusCode::NO_CONTENT);
    app.get(&format!("/api/captures/{id}"))
        .await
        .expect_error(StatusCode::NOT_FOUND, "not_found");
}

#[tokio::test]
async fn null_capture_runs_until_stopped() {
    let app = TestApp::new();
    let r = app
        .post(
            "/api/captures",
            json!({ "backend": "null", "filter": "arp" }),
        )
        .await;
    r.expect(StatusCode::CREATED);
    let id = r.json()["id"].as_str().unwrap().to_owned();
    assert_eq!(
        app.get(&format!("/api/captures/{id}")).await.json()["status"]["state"],
        "running"
    );
    // No pcapng while running / when nothing was saved.
    app.get(&format!("/api/captures/{id}/pcapng"))
        .await
        .expect_error(StatusCode::CONFLICT, "invalid_state");
    app.post(&format!("/api/captures/{id}/stop"), json!(null))
        .await
        .expect(StatusCode::OK);
    let done = wait_capture(&app, &id).await;
    assert_eq!(done["status"]["detail"], "stopped");
    app.get(&format!("/api/captures/{id}/pcapng"))
        .await
        .expect_error(StatusCode::NOT_FOUND, "not_found");
    assert_eq!(app.state.rx.running(), 0);
}

#[tokio::test]
async fn filters_and_errors() {
    let app = TestApp::new();
    let ok = app
        .post("/api/captures/filter", json!({ "filter": " udp port 53 " }))
        .await
        .json();
    assert_eq!(ok, json!({ "ok": true, "normalized": "udp port 53" }));
    let bad = app
        .post("/api/captures/filter", json!({ "filter": "udp port x" }))
        .await
        .json();
    assert_eq!(
        (bad["ok"].as_bool(), bad["pos"].as_u64()),
        (Some(false), Some(9))
    );

    let b = app.get("/api/captures/backends").await.json();
    assert_eq!(b[0]["name"], "null");

    let e = app
        .post(
            "/api/captures",
            json!({ "backend": "null", "filter": "bogus" }),
        )
        .await
        .expect_error(StatusCode::BAD_REQUEST, "bad_request");
    assert!(
        e["error"]["message"].as_str().unwrap().contains("bogus"),
        "{e}"
    );
    for spec in [
        json!({ "backend": "pcapfile" }),
        json!({ "backend": "pcapfile", "iface": "../etc/passwd" }),
        json!({ "backend": "afpacket" }),
        json!({ "backend": "null", "keep": 100_000 }),
        json!({ "backend": "null", "count": 0 }),
        json!({ "backend": "null", "duration": "soon" }),
        json!({ "backend": "warp", "iface": "x" }),
    ] {
        app.post("/api/captures", spec.clone())
            .await
            .expect_error(StatusCode::BAD_REQUEST, "bad_request");
    }
    app.post(
        "/api/captures",
        json!({ "backend": "pcapfile", "iface": "missing.pcapng" }),
    )
    .await
    .expect_error(StatusCode::NOT_FOUND, "not_found");
    app.get("/api/captures/nope")
        .await
        .expect_error(StatusCode::NOT_FOUND, "not_found");
    assert!(app
        .get("/api/captures")
        .await
        .json()
        .as_array()
        .unwrap()
        .is_empty());
}
