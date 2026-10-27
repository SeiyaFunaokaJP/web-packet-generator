//! Integration tests for the HTTP API, driven through `tower::ServiceExt::oneshot`
//! (no socket needed).

mod common;

use std::time::Duration;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use common::{wait_terminal, TestApp};
use serde_json::{json, Value};

const ARP_SWEEP: &str = include_str!("../../../templates/examples/arp-sweep.wpg.yaml");

// ---------------------------------------------------------------- system

#[tokio::test]
async fn health_is_ok_and_localised() {
    let app = TestApp::new();
    let r = app.get("/api/health").await;
    r.expect(StatusCode::OK);
    assert_eq!(r.content_type(), "application/json");
    let j = r.json();
    assert_eq!(j["status"], "ok");
    assert_eq!(j["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(j["locale"], "en");
    assert_eq!(j["message"], "Service is healthy");

    let j = app
        .call(
            Method::GET,
            "/api/health",
            None,
            Some("ja-JP,ja;q=0.9,en;q=0.8"),
        )
        .await
        .json();
    assert_eq!(j["locale"], "ja");
    assert_eq!(j["message"], "サービスは正常です");
}

#[tokio::test]
async fn version_ifaces_doctor_backends() {
    let app = TestApp::with(|c| c.tx.allowed_ifaces = vec!["lo".into()]);
    let v = app.get("/api/version").await.json();
    assert_eq!(v["api_version"], 1);
    assert_eq!(v["os"], std::env::consts::OS);

    let r = app.get("/api/system/ifaces?refresh=true").await;
    r.expect(StatusCode::OK);
    let list = r.json();
    for i in list.as_array().unwrap() {
        assert!(i["name"].is_string());
        assert!(i["usable"].is_boolean());
        assert_eq!(i["allowed"], i["name"] == "lo");
    }
    app.get("/api/system/ifaces?refresh=maybe")
        .await
        .expect_error(StatusCode::BAD_REQUEST, "bad_request");

    let d = app
        .call(Method::GET, "/api/system/doctor", None, Some("ja"))
        .await
        .json();
    assert_eq!(d["locale"], "ja");
    assert!(d["checks"][0]["summary"].is_string());

    let b = app.get("/api/system/backends").await.json();
    let names: Vec<&str> = b["backends"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"null") && names.contains(&"pcapfile"));
    assert!(b["default"].is_string());
}

#[tokio::test]
async fn unknown_api_is_localised_json_404() {
    let app = TestApp::new();
    let r = app.call(Method::GET, "/api/nope", None, Some("ja")).await;
    let j = r.expect_error(StatusCode::NOT_FOUND, "not_found");
    assert!(j["error"]["message"]
        .as_str()
        .unwrap()
        .contains("見つかりません"));
    // English by default
    let j = app
        .get("/api/nope")
        .await
        .expect_error(StatusCode::NOT_FOUND, "not_found");
    assert!(j["error"]["message"]
        .as_str()
        .unwrap()
        .contains("not found"));
}

#[tokio::test]
async fn root_serves_html_and_spa_fallback() {
    let app = TestApp::new();
    for path in ["/", "/templates/some/route"] {
        let r = app.get(path).await;
        r.expect(StatusCode::OK);
        assert!(r.content_type().starts_with("text/html"));
        assert!(r.text().contains("<html"));
    }
}

#[tokio::test]
async fn middleware_request_id_and_compression() {
    let app = TestApp::new();
    let r = app
        .send(
            Request::get("/api/protocols")
                .header(header::ACCEPT_ENCODING, "gzip")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    r.expect(StatusCode::OK);
    assert_eq!(r.headers["content-encoding"], "gzip");
    assert!(r.headers.contains_key("x-request-id"));
    // no CORS outside dev mode
    let r = app
        .send(
            Request::get("/api/health")
                .header(header::ORIGIN, "http://localhost:8081")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert!(!r.headers.contains_key("access-control-allow-origin"));
}

#[tokio::test]
async fn dev_mode_allows_trunk_origin() {
    let dir = tempfile::tempdir().unwrap();
    let state = wpg_server::AppState::new(common::config(&dir))
        .unwrap()
        .with_dev(true);
    let app = TestApp::from_state(dir, state);
    let r = app
        .send(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/api/jobs")
                .header(header::ORIGIN, "http://localhost:8081")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(
        r.headers["access-control-allow-origin"],
        "http://localhost:8081"
    );
}

// ---------------------------------------------------------------- protocols

#[tokio::test]
async fn protocols_catalog_and_definition() {
    let app = TestApp::new();
    let list = app.get("/api/protocols").await.json();
    assert_eq!(list[0]["id"], "ethernet2");
    assert_eq!(list[0]["fields"][2], "ethertype");
    let eth_next = list[0]["next"].as_array().unwrap();
    assert!(eth_next.contains(&json!("ipv4")));

    let v4 = app.get("/api/protocols/ipv4").await;
    v4.expect(StatusCode::OK);
    let v4 = v4.json();
    assert_eq!(v4["id"], "ipv4");
    assert!(v4["fields"].as_array().unwrap().len() > 10);

    // descriptions are localised next to their key
    let arp_en = app.get("/api/protocols/arp").await.json();
    let arp_ja = app
        .call(Method::GET, "/api/protocols/arp", None, Some("ja"))
        .await
        .json();
    let desc = |v: &Value| v["fields"][0]["description"].as_str().unwrap().to_owned();
    assert_eq!(arp_en["fields"][0]["description_key"], "arp-htype");
    assert_ne!(desc(&arp_en), desc(&arp_ja));
    assert!(desc(&arp_ja).contains("ハードウェア"));

    // No user-defined protocols in this process.
    assert_eq!(app.get("/api/protocols/extensions").await.json(), json!([]));
    app.call(Method::GET, "/api/protocols/nope", None, Some("ja"))
        .await
        .expect_error(StatusCode::NOT_FOUND, "not_found");
}

// ---------------------------------------------------------------- render / parse / lint

#[tokio::test]
async fn render_preview() {
    let app = TestApp::new();
    let r = app
        .post(
            "/api/render",
            json!({ "template": ARP_SWEEP, "count": 3, "index": 10 }),
        )
        .await;
    r.expect(StatusCode::OK);
    let j = r.json();
    assert_eq!(j["cardinality"], 254);
    let pkts = j["packets"].as_array().unwrap();
    assert_eq!(pkts.len(), 3);
    assert_eq!(pkts[0]["index"], 10);
    assert_eq!(pkts[0]["len"], 60);
    assert_eq!(pkts[0]["bytes_hex"].as_str().unwrap().len(), 120);
    assert_eq!(pkts[0]["layout"][1]["protocol"], "arp");
    assert_eq!(j["plan"]["strategy"], "pre_render");
    // target IP of packet 10 = 192.168.1.11 → last 4 octets of the ARP part
    assert!(pkts[0]["bytes_hex"].as_str().unwrap()[76..84].eq("c0a8010b"));

    // the object form gives the same bytes; vars override defaults
    let t: Value = serde_yaml::from_str(ARP_SWEEP).unwrap();
    let j2 = app
        .post(
            "/api/render",
            json!({ "template": t, "count": 3, "index": 10 }),
        )
        .await
        .json();
    assert_eq!(j2["packets"], j["packets"]);
    let j3 = app
        .post(
            "/api/render",
            json!({ "template": ARP_SWEEP, "vars": {"first": "10.0.0.1", "last": "10.0.0.4"}, "count": 10 }),
        )
        .await
        .json();
    // `tx.count: 254` cycles the 4 addresses
    assert_eq!(j3["cardinality"], 254);
    let tpa = |i: usize| j3["packets"][i]["bytes_hex"].as_str().unwrap()[76..84].to_owned();
    assert_eq!(tpa(0), "0a000001");
    assert_eq!(tpa(3), "0a000004");
    assert_eq!(tpa(4), "0a000001");
}

#[tokio::test]
async fn render_errors() {
    let app = TestApp::new();
    app.post(
        "/api/render",
        json!({ "template": ARP_SWEEP, "count": 257 }),
    )
    .await
    .expect_error(StatusCode::BAD_REQUEST, "bad_request");
    app.post("/api/render", json!({ "template": ARP_SWEEP, "count": 0 }))
        .await
        .expect_error(StatusCode::BAD_REQUEST, "bad_request");
    let bad = "schema: 1\nmeta: {name: x}\nlayers:\n  - protocol: nope\n";
    let j = app
        .call(
            Method::POST,
            "/api/render",
            Some(json!({ "template": bad })),
            Some("ja"),
        )
        .await
        .expect_error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_template");
    let details = j["error"]["details"].as_array().unwrap();
    assert_eq!(details[0]["key"], "lint-unknown-protocol");
    assert!(j["error"]["message"]
        .as_str()
        .unwrap()
        .contains("テンプレート"));
    // body that is not JSON / misses fields
    app.send(
        Request::post("/api/render")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{not json"))
            .unwrap(),
    )
    .await
    .expect_error(StatusCode::BAD_REQUEST, "bad_request");
    app.post("/api/render", json!({ "count": 1 }))
        .await
        .expect_error(StatusCode::BAD_REQUEST, "bad_request");
}

#[tokio::test]
async fn parse_round_trip() {
    let app = TestApp::new();
    let r = app
        .post("/api/render", json!({ "template": ARP_SWEEP }))
        .await
        .json();
    let hex = r["packets"][0]["bytes_hex"].as_str().unwrap().to_owned();
    // spaced / colon separated input is accepted
    let spaced: Vec<String> = hex
        .as_bytes()
        .chunks(2)
        .map(|c| String::from_utf8(c.to_vec()).unwrap())
        .collect();
    let p = app
        .post("/api/parse", json!({ "bytes_hex": spaced.join(":") }))
        .await;
    p.expect(StatusCode::OK);
    let p = p.json();
    assert_eq!(p["bytes_hex"], hex);
    assert_eq!(p["len"], 60);
    assert_eq!(p["layout"][0]["protocol"], "ethernet2");
    assert_eq!(p["layout"][1]["protocol"], "arp");
    assert!(p["packet"]["layers"].is_array());

    app.post("/api/parse", json!({ "bytes_hex": "abc" }))
        .await
        .expect_error(StatusCode::BAD_REQUEST, "bad_request");
    app.post(
        "/api/parse",
        json!({ "bytes_hex": hex, "first_protocol": "nope" }),
    )
    .await
    .expect_error(StatusCode::NOT_FOUND, "not_found");
}

#[tokio::test]
async fn lint_reports_issues() {
    let app = TestApp::new();
    let ok = app
        .post("/api/lint", json!({ "template": ARP_SWEEP }))
        .await
        .json();
    assert_eq!(ok["ok"], true, "{ok}");
    assert_eq!(ok["errors"], 0);

    let bad = "schema: 1\nmeta: {name: x}\nlayers:\n  - protocol: nope\n";
    let j = app
        .call(
            Method::POST,
            "/api/lint",
            Some(json!({ "template": bad })),
            Some("ja"),
        )
        .await
        .json();
    assert_eq!(j["ok"], false);
    assert!(j["errors"].as_u64().unwrap() >= 1);
    assert_eq!(j["issues"][0]["key"], "lint-unknown-protocol");
    let msg = j["issues"][0]["message"].as_str().unwrap();
    assert!(msg.contains("nope"), "{msg}");

    let j = app
        .post("/api/lint", json!({ "template": "schema: [" }))
        .await
        .json();
    assert_eq!(j["errors"], 1);
    assert_eq!(j["issues"][0]["key"], "lint-parse-error");
}

// ---------------------------------------------------------------- templates

#[tokio::test]
async fn template_crud() {
    let app = TestApp::new();
    let list = app.get("/api/templates").await.json();
    let ids: Vec<&str> = list["templates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"examples-arp-sweep"), "{ids:?}");

    // search + tags + localised description
    let j = app
        .call(
            Method::GET,
            "/api/templates?q=sweep&tag=arp",
            None,
            Some("ja"),
        )
        .await
        .json();
    let found = j["templates"].as_array().unwrap();
    // every hit matches the query; the bundled library may hold several
    for t in found {
        let tags = t["tags"].as_array().unwrap();
        assert!(tags.contains(&json!("arp")), "{t}");
    }
    let arp = found
        .iter()
        .find(|t| t["id"] == "examples-arp-sweep")
        .expect("examples-arp-sweep found");
    assert!(arp["description"].as_str().unwrap().contains("スイープ"));
    assert_eq!(arp["source"], "library");

    // a language the bundled templates are not described in falls back to English
    let ko = app
        .call(
            Method::GET,
            "/api/templates?q=sweep&tag=arp",
            None,
            Some("ko"),
        )
        .await
        .json();
    let arp_ko = ko["templates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == "examples-arp-sweep")
        .expect("examples-arp-sweep found");
    assert_eq!(arp_ko["description"], arp["descriptions"]["en"]);
    assert_eq!(arp["protocols"], json!(["ethernet2", "arp"]));

    // library templates are read-only
    let lib = app.get("/api/templates/examples-arp-sweep").await.json();
    assert_eq!(lib["summary"]["writable"], false);
    assert_eq!(lib["issues"], json!([]));
    app.call(
        Method::PUT,
        "/api/templates/examples-arp-sweep",
        Some(lib["template"].clone()),
        None,
    )
    .await
    .expect_error(StatusCode::FORBIDDEN, "read_only");
    app.call(
        Method::DELETE,
        "/api/templates/examples-arp-sweep",
        None,
        None,
    )
    .await
    .expect_error(StatusCode::FORBIDDEN, "read_only");

    // create
    let mut t = lib["template"].clone();
    t["meta"]["name"] = json!("My ARP");
    let r = app.post("/api/templates", t.clone()).await;
    r.expect(StatusCode::CREATED);
    let created = r.json();
    let id = created["summary"]["id"].as_str().unwrap().to_owned();
    assert_eq!(id, "my-arp");
    assert_eq!(created["summary"]["writable"], true);
    assert!(app
        .dir
        .path()
        .join("data/templates/my-arp.wpg.yaml")
        .is_file());
    app.post("/api/templates", json!({ "schema": 1 }))
        .await
        .expect_error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_template");

    // update
    t["meta"]["tags"] = json!(["mine"]);
    let r = app
        .call(Method::PUT, &format!("/api/templates/{id}"), Some(t), None)
        .await;
    r.expect(StatusCode::OK);
    assert_eq!(r.json()["summary"]["tags"], json!(["mine"]));

    // duplicate (from the library)
    let r = app
        .post("/api/templates/examples-arp-sweep/duplicate", json!(null))
        .await;
    r.expect(StatusCode::CREATED);
    assert_eq!(r.json()["template"]["meta"]["name"], "arp-sweep (copy)");

    // export keeps the file text (comments)
    let r = app.get("/api/templates/examples-arp-sweep/export").await;
    r.expect(StatusCode::OK);
    assert!(r.content_type().starts_with("text/yaml"));
    assert!(r.headers[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .contains("arp-sweep.wpg.yaml"));
    assert_eq!(r.text(), ARP_SWEEP);

    // import YAML text
    let yaml = ARP_SWEEP.replace("name: arp-sweep", "name: imported sweep");
    let r = app
        .send(
            Request::post("/api/templates/import")
                .header(header::CONTENT_TYPE, "text/yaml")
                .body(Body::from(yaml))
                .unwrap(),
        )
        .await;
    r.expect(StatusCode::CREATED);
    assert_eq!(r.json()["summary"]["id"], "imported-sweep");
    app.send(
        Request::post("/api/templates/import")
            .body(Body::from("schema: ["))
            .unwrap(),
    )
    .await
    .expect_error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_template");

    // favourites: toggle, explicit value, filter
    let f = app
        .post("/api/templates/examples-arp-sweep/favorite", json!(null))
        .await;
    assert_eq!(f.json()["favorite"], true);
    let f = app
        .post(
            "/api/templates/examples-arp-sweep/favorite",
            json!({ "favorite": true }),
        )
        .await;
    assert_eq!(f.json()["favorite"], true);
    let favs = app.get("/api/templates?favorite=true").await.json();
    assert_eq!(favs["templates"].as_array().unwrap().len(), 1);
    // recent: the GET above marked the library template as used
    assert!(favs["recent"]
        .as_array()
        .unwrap()
        .contains(&json!("examples-arp-sweep")));

    // delete
    app.call(Method::DELETE, &format!("/api/templates/{id}"), None, None)
        .await
        .expect(StatusCode::NO_CONTENT);
    app.get(&format!("/api/templates/{id}"))
        .await
        .expect_error(StatusCode::NOT_FOUND, "not_found");
    app.post("/api/templates/nope/favorite", json!(null))
        .await
        .expect_error(StatusCode::NOT_FOUND, "not_found");
}

// ---------------------------------------------------------------- jobs

#[tokio::test]
async fn job_lifecycle_null_backend() {
    let app = TestApp::new();
    let r = app
        .post(
            "/api/jobs",
            json!({
                "streams": [{ "template_id": "examples-arp-sweep" }],
                "backend": "null",
                "count": 1000,
                "rate": "unlimited",
            }),
        )
        .await;
    r.expect(StatusCode::CREATED);
    let job = r.json();
    let id = job["id"].as_str().unwrap().to_owned();
    assert_eq!(job["backend"], "null");
    assert_eq!(job["iface"], "null0");
    let done = wait_terminal(&app, &id).await;
    assert_eq!(
        done["status"],
        json!({"state": "finished", "detail": "count_reached"})
    );
    assert_eq!(done["report"]["stats"]["packets"], 1000);
    let stats = app.get(&format!("/api/jobs/{id}/stats")).await.json();
    assert_eq!(stats["packets"], 1000);
    // history
    let h = app.get("/api/jobs/history").await.json();
    assert_eq!(h[0]["id"], id);
    // persisted in the database
    assert_eq!(app.state.db.jobs(10).unwrap()[0].id, id);
    assert!(app.dir.path().join("data/wpg.sqlite").is_file());
    // a finished job cannot be paused / started again
    app.post(&format!("/api/jobs/{id}/pause"), json!(null))
        .await
        .expect_error(StatusCode::CONFLICT, "invalid_state");
    app.post(&format!("/api/jobs/{id}/start"), json!(null))
        .await
        .expect_error(StatusCode::CONFLICT, "invalid_state");
    // list + delete
    let list = app.get("/api/jobs").await.json();
    assert_eq!(list[0]["id"], id);
    app.call(Method::DELETE, &format!("/api/jobs/{id}"), None, None)
        .await
        .expect(StatusCode::NO_CONTENT);
    assert_eq!(app.get("/api/jobs").await.json(), json!([]));
    // still readable from the history
    app.get(&format!("/api/jobs/{id}"))
        .await
        .expect(StatusCode::OK);
    app.get("/api/jobs/nope")
        .await
        .expect_error(StatusCode::NOT_FOUND, "not_found");
}

#[tokio::test]
async fn job_created_then_started_paused_resumed_stopped() {
    let app = TestApp::new();
    let job = app
        .post(
            "/api/jobs",
            json!({
                "name": "endless",
                "streams": [{ "template_id": "examples-arp-sweep" }],
                "backend": "null",
                "count": 100_000_000,
                "rate": "10kpps",
                "start": false,
            }),
        )
        .await;
    job.expect(StatusCode::CREATED);
    let id = job.json()["id"].as_str().unwrap().to_owned();
    assert_eq!(job.json()["status"]["state"], "created");
    app.get(&format!("/api/jobs/{id}/stats"))
        .await
        .expect_error(StatusCode::CONFLICT, "invalid_state");
    app.post(&format!("/api/jobs/{id}/pause"), json!(null))
        .await
        .expect_error(StatusCode::CONFLICT, "invalid_state");

    let r = app
        .post(&format!("/api/jobs/{id}/start"), json!(null))
        .await;
    r.expect(StatusCode::OK);
    tokio::time::sleep(Duration::from_millis(150)).await;
    app.post(&format!("/api/jobs/{id}/pause"), json!(null))
        .await
        .expect(StatusCode::OK);
    let mut paused = false;
    for _ in 0..100 {
        let j = app.get(&format!("/api/jobs/{id}")).await.json();
        if j["status"]["state"] == "paused" {
            paused = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(paused);
    let overview = app.get("/api/stats/overview").await.json();
    assert_eq!(overview["totals"]["active"], 1);
    assert_eq!(overview["jobs"][0]["id"], id);
    app.post(&format!("/api/jobs/{id}/resume"), json!(null))
        .await
        .expect(StatusCode::OK);
    app.post(&format!("/api/jobs/{id}/stop"), json!(null))
        .await
        .expect(StatusCode::OK);
    let done = wait_terminal(&app, &id).await;
    assert_eq!(
        done["status"],
        json!({"state": "finished", "detail": "stopped"})
    );
    assert!(done["report"]["stats"]["packets"].as_u64().unwrap() > 0);
    app.post(&format!("/api/jobs/{id}/stop"), json!(null))
        .await
        .expect_error(StatusCode::CONFLICT, "invalid_state");
}

#[tokio::test]
async fn job_mixes_streams_and_dry_runs() {
    let app = TestApp::new();
    let inline: Value = serde_yaml::from_str(include_str!(
        "../../../templates/examples/eth-ipv4-udp-range.wpg.yaml"
    ))
    .unwrap();
    let r = app
        .post(
            "/api/jobs",
            json!({
                "streams": [
                    { "template_id": "examples-arp-sweep", "weight": 2 },
                    { "template": inline, "vars": { "dst_ip": "10.9.9.9" } },
                ],
                "mix": "weighted",
                "dry_run": "mix",
                "count": 30,
                "rate": "unlimited",
            }),
        )
        .await;
    r.expect(StatusCode::CREATED);
    let job = r.json();
    assert_eq!(job["backend"], "pcapfile");
    let id = job["id"].as_str().unwrap().to_owned();
    let done = wait_terminal(&app, &id).await;
    assert_eq!(done["report"]["stats"]["packets"], 30, "{done}");
    let per = &done["report"]["stats"]["per_stream"];
    assert_eq!(per[0]["packets"], 20);
    assert_eq!(per[1]["packets"], 10);
    let file = app.dir.path().join("data/captures/mix.pcapng");
    assert!(file.metadata().unwrap().len() > 30 * 60);

    // several streams need a single worker; dry runs stay inside the data dir
    app.post(
        "/api/jobs",
        json!({
            "streams": [{ "template_id": "examples-arp-sweep" }, { "template_id": "examples-arp-sweep" }],
            "backend": "null",
            "workers": 2,
        }),
    )
    .await
    .expect_error(StatusCode::BAD_REQUEST, "bad_request");
    app.post(
        "/api/jobs",
        json!({ "streams": [{ "template_id": "examples-arp-sweep" }], "dry_run": "../escape" }),
    )
    .await
    .expect_error(StatusCode::BAD_REQUEST, "bad_request");
    app.post("/api/jobs", json!({ "streams": [] }))
        .await
        .expect_error(StatusCode::BAD_REQUEST, "bad_request");
    app.post(
        "/api/jobs",
        json!({ "streams": [{ "template_id": "nope" }], "backend": "null" }),
    )
    .await
    .expect_error(StatusCode::NOT_FOUND, "not_found");
    app.post(
        "/api/jobs",
        json!({ "streams": [{ "template_id": "examples-arp-sweep" }], "backend": "warp", "iface": "x" }),
    )
    .await
    .expect_error(StatusCode::BAD_GATEWAY, "backend_error");
    // a failed start does not leave a job behind
    assert!(app
        .get("/api/jobs")
        .await
        .json()
        .as_array()
        .unwrap()
        .iter()
        .all(|j| j["id"] == id));
}

#[tokio::test]
async fn policy_violations() {
    let app = TestApp::with(|c| {
        c.tx.allowed_ifaces = vec!["eth-allowed".into()];
        c.tx.max_rate = Some("1kpps".into());
    });
    // interface not in the allow list (checked before the backend is opened)
    let j = app
        .call(
            Method::POST,
            "/api/jobs",
            Some(json!({
                "streams": [{ "template_id": "examples-arp-sweep" }],
                "backend": "afpacket",
                "iface": "eth9",
            })),
            Some("ja"),
        )
        .await
        .expect_error(StatusCode::FORBIDDEN, "policy_violation");
    assert!(j["error"]["message"].as_str().unwrap().contains("eth9"));
    // rate above the maximum
    let j = app
        .post(
            "/api/jobs",
            json!({
                "streams": [{ "template_id": "examples-arp-sweep" }],
                "backend": "afpacket",
                "iface": "eth-allowed",
                "rate": "10kpps",
            }),
        )
        .await
        .expect_error(StatusCode::FORBIDDEN, "policy_violation");
    assert!(j["error"]["message"].as_str().unwrap().contains("1kpps"));
    app.post(
        "/api/jobs",
        json!({
            "streams": [{ "template_id": "examples-arp-sweep" }],
            "backend": "afpacket",
            "iface": "eth-allowed",
            "rate": "unlimited",
        }),
    )
    .await
    .expect_error(StatusCode::FORBIDDEN, "policy_violation");
    // the null backend sends nothing and is not restricted
    let r = app
        .post(
            "/api/jobs",
            json!({
                "streams": [{ "template_id": "examples-arp-sweep" }],
                "backend": "null",
                "rate": "100kpps",
                "count": 10,
            }),
        )
        .await;
    r.expect(StatusCode::CREATED);
    assert_eq!(
        app.get("/api/jobs").await.json().as_array().unwrap().len(),
        1
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn job_manager_shutdown_records_reports() {
    let app = TestApp::new();
    let state = app.state.clone();
    let spec = wpg_server::jobs::JobSpec {
        streams: vec![wpg_server::jobs::JobStream {
            template_id: Some("examples-arp-sweep".into()),
            weight: 1,
            ..Default::default()
        }],
        backend: Some("null".into()),
        count: Some(u64::MAX / 2),
        ..Default::default()
    };
    let job = state.jobs.create(spec, true).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    tokio::time::timeout(Duration::from_secs(5), state.jobs.shutdown())
        .await
        .unwrap();
    let info = state.jobs.get(&job.id).unwrap();
    assert!(info.status.is_terminal(), "{:?}", info.status);
    assert!(info.report.is_some());
    assert_eq!(state.jobs.history()[0].id, job.id);
}

// ---------------------------------------------------------------- OpenAPI

#[tokio::test]
async fn openapi_lists_every_route() {
    let app = TestApp::new();
    let r = app.get("/api/openapi.json").await;
    r.expect(StatusCode::OK);
    let doc = r.json();
    assert!(doc["openapi"].as_str().unwrap().starts_with("3."));
    let paths = doc["paths"].as_object().unwrap();
    let expected: &[(&str, &[&str])] = &[
        ("/api/auth/status", &["get"]),
        ("/api/auth/setup", &["post"]),
        ("/api/auth/login", &["post"]),
        ("/api/auth/logout", &["post"]),
        ("/api/health", &["get"]),
        ("/api/version", &["get"]),
        ("/api/system/ifaces", &["get"]),
        ("/api/system/doctor", &["get"]),
        ("/api/system/backends", &["get"]),
        ("/api/protocols", &["get"]),
        ("/api/protocols/extensions", &["get"]),
        ("/api/protocols/{id}", &["get"]),
        ("/api/render", &["post"]),
        ("/api/parse", &["post"]),
        ("/api/lint", &["post"]),
        ("/api/templates", &["get", "post"]),
        ("/api/templates/{id}", &["get", "put", "delete"]),
        ("/api/templates/{id}/duplicate", &["post"]),
        ("/api/templates/{id}/export", &["get"]),
        ("/api/templates/import", &["post"]),
        ("/api/templates/{id}/favorite", &["post"]),
        ("/api/jobs", &["get", "post"]),
        ("/api/jobs/{id}", &["get", "delete"]),
        ("/api/jobs/{id}/start", &["post"]),
        ("/api/jobs/{id}/pause", &["post"]),
        ("/api/jobs/{id}/resume", &["post"]),
        ("/api/jobs/{id}/stop", &["post"]),
        ("/api/jobs/{id}/stats", &["get"]),
        ("/api/jobs/history", &["get"]),
        ("/api/stats/overview", &["get"]),
        ("/api/captures", &["get", "post"]),
        ("/api/captures/backends", &["get"]),
        ("/api/captures/filter", &["post"]),
        ("/api/captures/{id}", &["get", "delete"]),
        ("/api/captures/{id}/stop", &["post"]),
        ("/api/captures/{id}/packets", &["get"]),
        ("/api/captures/{id}/pcapng", &["get"]),
        ("/api/ws", &["get"]),
        ("/api/snmp/status", &["get"]),
        ("/api/snmp/get", &["post"]),
        ("/api/snmp/getnext", &["post"]),
        ("/api/snmp/getbulk", &["post"]),
        ("/api/snmp/walk", &["post"]),
        ("/api/snmp/set", &["post"]),
        ("/api/snmp/trap", &["post"]),
        ("/api/snmp/inform", &["post"]),
        ("/api/snmp/mibs", &["get", "post"]),
        ("/api/snmp/mibs/{module}", &["delete"]),
        ("/api/snmp/mibs/tree", &["get"]),
        ("/api/snmp/mibs/node", &["get"]),
        ("/api/snmp/mibs/search", &["get"]),
        ("/api/snmp/receiver", &["get"]),
        ("/api/snmp/receiver/start", &["post"]),
        ("/api/snmp/receiver/stop", &["post"]),
        ("/api/snmp/traps", &["get", "delete"]),
        ("/api/snmp/capture", &["get", "post"]),
        ("/api/snmp/capture/download", &["get"]),
    ];
    for (path, methods) in expected {
        let item = paths
            .get(*path)
            .unwrap_or_else(|| panic!("{path} missing from OpenAPI"));
        for m in *methods {
            assert!(item.get(*m).is_some(), "{m} {path} missing");
        }
    }
    assert_eq!(paths.len(), expected.len(), "{:?}", paths.keys());
    let schemas = doc["components"]["schemas"].as_object().unwrap();
    // cookie authentication, with the public endpoints opted out
    let schemes = &doc["components"]["securitySchemes"];
    assert_eq!(schemes["cookieAuth"]["in"], "cookie");
    assert_eq!(schemes["cookieAuth"]["name"], "wpg_session");
    assert_eq!(schemes["csrfToken"]["name"], "X-CSRF-Token");
    assert_eq!(doc["security"][0]["cookieAuth"], json!([]));
    assert_eq!(paths["/api/health"]["get"]["security"], json!([{}]));
    assert_eq!(paths["/api/auth/login"]["post"]["security"], json!([{}]));
    for s in [
        "AuthStatus",
        "ErrorBody",
        "JobInfo",
        "JobSpec",
        "ServerEvent",
        "TemplateSummary",
    ] {
        assert!(schemas.contains_key(s), "schema {s} missing");
    }
    // same document as the library function
    assert_eq!(
        serde_json::to_value(wpg_server::api::openapi()).unwrap(),
        doc
    );

    // Swagger UI (vendored assets)
    let r = app.get("/api/docs/").await;
    r.expect(StatusCode::OK);
    assert!(r.text().to_lowercase().contains("swagger"));
    assert!(app.get("/api/docs").await.status.is_redirection());
}
