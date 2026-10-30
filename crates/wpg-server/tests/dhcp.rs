//! `/api/dhcp`: settings, validation, interface policy, leases and activity
//! (without opening sockets: starting needs an existing interface and
//! privileges, covered by wpg-dhcp's unit tests and manual checks).

mod common;

use axum::http::{Method, StatusCode};
use common::TestApp;
use serde_json::json;

fn settings() -> serde_json::Value {
    json!({
        "addresses": ["192.168.77.1/24"],
        "v4": { "enabled": true, "range_start": "192.168.77.100", "range_end": "192.168.77.110",
                "subnet_mask": "255.255.255.0", "lease_secs": 600 },
        "v6": { "enabled": false }
    })
}

#[tokio::test]
async fn settings_round_trip_and_errors() {
    let app = TestApp::new();
    let st = app.get("/api/dhcp").await;
    st.expect(StatusCode::OK);
    let st = st.json();
    assert_eq!(st["enabled"], true);
    assert_eq!(st["running"], false);
    assert_eq!(st["iface"], "");

    // No interface chosen yet.
    app.post("/api/dhcp/start", json!(null))
        .await
        .expect_error(StatusCode::BAD_REQUEST, "bad_request");

    // Invalid settings: 400 with the problems in `details`.
    let mut bad = settings();
    bad["v4"]["lease_secs"] = json!(1);
    bad["addresses"] = json!(["nope"]);
    let r = app
        .call(
            Method::PUT,
            "/api/dhcp",
            Some(json!({ "settings": bad })),
            None,
        )
        .await;
    let e = r.expect_error(StatusCode::BAD_REQUEST, "bad_request");
    let fields: Vec<_> = e["error"]["details"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["field"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(fields, ["addresses[0]", "v4.lease_secs"]);

    // Saved and returned.
    let r = app
        .call(
            Method::PUT,
            "/api/dhcp",
            Some(json!({ "iface": "nosuch0", "settings": settings() })),
            None,
        )
        .await;
    r.expect(StatusCode::OK);
    let st = r.json();
    assert_eq!(st["iface"], "nosuch0");
    assert_eq!(st["settings"]["v4"]["lease_secs"], 600);
    assert_eq!(st["settings"]["addresses"][0], "192.168.77.1/24");
    // Unknown interface.
    app.post("/api/dhcp/start", json!(null))
        .await
        .expect_error(StatusCode::BAD_REQUEST, "bad_request");
    // Stop is always fine.
    app.post("/api/dhcp/stop", json!(null))
        .await
        .expect(StatusCode::OK);

    // Leases and activity start empty.
    let r = app.get("/api/dhcp/leases").await;
    r.expect(StatusCode::OK);
    assert_eq!(r.json(), json!([]));
    app.call(Method::DELETE, "/api/dhcp/leases?key=10.0.0.1", None, None)
        .await
        .expect_error(StatusCode::NOT_FOUND, "not_found");
    app.call(Method::DELETE, "/api/dhcp/leases", None, None)
        .await
        .expect(StatusCode::NO_CONTENT);
    let r = app.get("/api/dhcp/activity").await;
    r.expect(StatusCode::OK);
    assert_eq!(r.json(), json!([]));
}

#[tokio::test]
async fn interface_policy_pinning_and_disabled() {
    // Not in tx.allowed_ifaces.
    let app = TestApp::with(|c| c.tx.allowed_ifaces = vec!["eth9".into()]);
    let r = app
        .call(
            Method::PUT,
            "/api/dhcp",
            Some(json!({ "iface": "eth1", "settings": settings() })),
            None,
        )
        .await;
    r.expect_error(StatusCode::FORBIDDEN, "policy_violation");

    // Pinned by dhcp.iface: the requested interface is ignored.
    let app = TestApp::with(|c| c.dhcp.iface = "eth5".into());
    let r = app
        .call(
            Method::PUT,
            "/api/dhcp",
            Some(json!({ "iface": "eth1", "settings": settings() })),
            None,
        )
        .await;
    r.expect(StatusCode::OK);
    let st = r.json();
    assert_eq!(st["iface"], "eth5");
    assert_eq!(st["pinned_iface"], "eth5");

    // dhcp.enabled = false.
    let app = TestApp::with(|c| c.dhcp.enabled = false);
    app.get("/api/dhcp")
        .await
        .expect_error(StatusCode::NOT_FOUND, "not_found");
}
