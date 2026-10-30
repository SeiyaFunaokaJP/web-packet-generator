//! `/api/snmp`: operations against an in-process agent on 127.0.0.1 (v2c and
//! v3), MIB upload / browse / delete, the trap receiver with traps and informs
//! sent through the API, the pcap recording, and the disabled switch.

mod common;

use std::collections::BTreeMap;

use axum::http::{Method, StatusCode};
use serde_json::{json, Value as Json};
use wpg_snmp::agent::{Agent, AgentConfig};
use wpg_snmp::usm::{AuthProtocol, PrivProtocol, UsmUser};
use wpg_snmp::{Oid, Value};

use common::TestApp;

fn oid(s: &str) -> Oid {
    s.parse().unwrap()
}

async fn agent() -> Agent {
    let mut d = BTreeMap::new();
    d.insert(
        oid("1.3.6.1.2.1.1.1.0"),
        Value::OctetString(b"wpg agent".to_vec()),
    );
    d.insert(oid("1.3.6.1.2.1.1.3.0"), Value::TimeTicks(360_000));
    d.insert(
        oid("1.3.6.1.2.1.1.5.0"),
        Value::OctetString(b"agent1".to_vec()),
    );
    for i in 1..=2u32 {
        d.insert(
            oid(&format!("1.3.6.1.2.1.2.2.1.1.{i}")),
            Value::Integer(i64::from(i)),
        );
        d.insert(
            oid(&format!("1.3.6.1.2.1.2.2.1.2.{i}")),
            Value::OctetString(format!("eth{i}").into_bytes()),
        );
        d.insert(oid(&format!("1.3.6.1.2.1.2.2.1.7.{i}")), Value::Integer(1));
        d.insert(
            oid(&format!("1.3.6.1.2.1.2.2.1.8.{i}")),
            Value::Integer(i64::from(i)),
        );
        d.insert(
            oid(&format!("1.3.6.1.2.1.2.2.1.10.{i}")),
            Value::Counter32(500 * i),
        );
    }
    Agent::start(
        AgentConfig {
            listen: "127.0.0.1:0".parse().unwrap(),
            community: "public".into(),
            write_community: "private".into(),
            users: vec![UsmUser {
                name: "ops".into(),
                auth: Some((AuthProtocol::Sha256, "authpass99".into())),
                privacy: Some((PrivProtocol::Aes128, "privpass99".into())),
            }],
            engine_id: b"\x80\x00\x00\x00\x04wpg-api-agent".to_vec(),
        },
        d,
    )
    .await
    .unwrap()
}

fn target(a: &Agent, community: &str) -> Json {
    json!({ "host": "127.0.0.1", "port": a.local_addr().port(), "community": community, "timeout_ms": 500 })
}

#[tokio::test]
async fn operations_v2c_and_v3() {
    let a = agent().await;
    let app = TestApp::new();

    let r = app.get("/api/snmp/status").await;
    r.expect(StatusCode::OK);
    let s = r.json();
    assert_eq!(s["enabled"], true);
    assert!(s["nodes"].as_u64().unwrap() > 700, "{s}");

    let r = app
        .post("/api/snmp/get", json!({ "target": target(&a, "public"), "oids": ["sysDescr.0", "SNMPv2-MIB::sysUpTime.0", "1.3.6.1.2.1.2.2.1.8.2"] }))
        .await;
    r.expect(StatusCode::OK);
    let j = r.json();
    assert_eq!(j["op"], "get");
    assert_eq!(j["error_status"], 0);
    let vb = j["varbinds"].as_array().unwrap();
    assert_eq!(vb[0]["name"], "SNMPv2-MIB::sysDescr.0");
    assert_eq!(vb[0]["value"], "wpg agent");
    assert_eq!(vb[1]["type"], "TimeTicks");
    assert_eq!(vb[1]["value"], "01:00:00.00 (360000)");
    assert_eq!(vb[2]["value"], "down(2)");
    assert_eq!(vb[2]["raw"], "2");

    // Walk of ifTable: bindings plus the table view.
    let r = app
        .post(
            "/api/snmp/walk",
            json!({ "target": target(&a, "public"), "oid": "ifTable", "max_repetitions": 3 }),
        )
        .await;
    r.expect(StatusCode::OK);
    let j = r.json();
    assert_eq!(j["varbinds"].as_array().unwrap().len(), 10);
    let t = &j["tables"][0];
    assert_eq!(t["entry"], "IF-MIB::ifEntry");
    assert_eq!(t["index_names"], json!(["ifIndex"]));
    assert_eq!(t["rows"].as_array().unwrap().len(), 2);
    assert_eq!(t["rows"][1]["cells"][1], "eth2");
    let counter_col = t["columns"]
        .as_array()
        .unwrap()
        .iter()
        .position(|c| c["name"] == "ifInOctets")
        .unwrap();
    assert_eq!(t["columns"][counter_col]["counter"], true);
    assert_eq!(t["rows"][1]["numeric"][counter_col], 1000);

    // Several roots are walked one after another (a scalar instance falls
    // back to a Get); the limit counts them all.
    let r = app
        .post(
            "/api/snmp/walk",
            json!({ "target": target(&a, "public"), "oids": ["sysDescr.0", "ifTable"], "max_repetitions": 3 }),
        )
        .await;
    r.expect(StatusCode::OK);
    let j = r.json();
    let vb = j["varbinds"].as_array().unwrap();
    assert_eq!(vb.len(), 11);
    assert_eq!(vb[0]["name"], "SNMPv2-MIB::sysDescr.0");
    assert_eq!(j["tables"][0]["entry"], "IF-MIB::ifEntry");
    let r = app
        .post(
            "/api/snmp/walk",
            json!({ "target": target(&a, "public"), "oids": ["sysDescr.0", "ifTable"], "limit": 4 }),
        )
        .await;
    let j = r.json();
    assert_eq!(j["varbinds"].as_array().unwrap().len(), 4);
    assert_eq!(j["truncated"], true);
    app.post("/api/snmp/walk", json!({ "target": target(&a, "public") }))
        .await
        .expect_error(StatusCode::BAD_REQUEST, "bad_request");

    let r = app
        .post(
            "/api/snmp/getbulk",
            json!({ "target": target(&a, "public"), "oids": ["sysDescr"], "max_repetitions": 2 }),
        )
        .await;
    r.expect(StatusCode::OK);
    assert_eq!(r.json()["varbinds"].as_array().unwrap().len(), 2);

    // Set with MIB-typed values (enum label) through the read-write community.
    let r = app
        .post(
            "/api/snmp/set",
            json!({ "target": target(&a, "private"), "varbinds": [
            { "oid": "ifAdminStatus.2", "value": "down" },
            { "oid": "sysName.0", "value": "renamed" }
        ] }),
        )
        .await;
    r.expect(StatusCode::OK);
    assert_eq!(r.json()["error_status"], 0, "{}", r.text());
    assert_eq!(
        a.store().lock().unwrap()[&oid("1.3.6.1.2.1.2.2.1.7.2")],
        Value::Integer(2)
    );
    // Read-only community: noAccess in the response, not an HTTP error.
    let r = app
        .post("/api/snmp/set", json!({ "target": target(&a, "public"), "varbinds": [{ "oid": "sysName.0", "value": "x" }] }))
        .await;
    r.expect(StatusCode::OK);
    assert_eq!(r.json()["error_status_name"], "noAccess");

    // SNMPv3 authPriv (SHA-256 / AES-128).
    let mut t3 = target(&a, "");
    t3["version"] = json!("v3");
    t3["v3"] = json!({ "user": "ops", "auth_protocol": "SHA-256", "auth_password": "authpass99", "priv_protocol": "AES", "priv_password": "privpass99" });
    let r = app
        .post(
            "/api/snmp/get",
            json!({ "target": t3, "oids": ["sysName.0"] }),
        )
        .await;
    r.expect(StatusCode::OK);
    let j = r.json();
    assert_eq!(j["varbinds"][0]["value"], "renamed");
    assert_eq!(j["version"], "v3");
    assert!(
        j["engine_id"].as_str().unwrap().starts_with("80000000"),
        "{j}"
    );
    // Wrong pass phrase: a Report, 502 snmp_error.
    t3["v3"]["auth_password"] = json!("wrong-pass");
    let r = app
        .post(
            "/api/snmp/get",
            json!({ "target": t3, "oids": ["sysName.0"] }),
        )
        .await;
    let e = r.expect_error(StatusCode::BAD_GATEWAY, "snmp_error");
    assert_eq!(e["error"]["details"]["report"], "usmStatsWrongDigests");
    // Too short: rejected before sending.
    t3["v3"]["auth_password"] = json!("short");
    app.post(
        "/api/snmp/get",
        json!({ "target": t3, "oids": ["sysName.0"] }),
    )
    .await
    .expect_error(StatusCode::BAD_REQUEST, "bad_request");

    // Unknown name; silence → 504 with a localised message.
    app.post(
        "/api/snmp/get",
        json!({ "target": target(&a, "public"), "oids": ["noSuchObjectName.0"] }),
    )
    .await
    .expect_error(StatusCode::BAD_REQUEST, "bad_request");
    let mut silent = target(&a, "nope");
    silent["retries"] = json!(0);
    silent["timeout_ms"] = json!(100);
    let r = app
        .call(
            Method::POST,
            "/api/snmp/get",
            Some(json!({ "target": silent, "oids": ["sysDescr.0"] })),
            Some("ja"),
        )
        .await;
    let e = r.expect_error(StatusCode::GATEWAY_TIMEOUT, "snmp_error");
    assert!(
        e["error"]["message"]
            .as_str()
            .unwrap()
            .contains("127.0.0.1"),
        "{e}"
    );
}

#[tokio::test]
async fn mib_upload_browse_delete() {
    let app = TestApp::new();
    let r = app
        .post(
            "/api/snmp/mibs",
            json!({ "files": [
                { "name": "WPG-EXAMPLE-VENDOR-MIB.my", "content": include_str!("../../wpg-snmp/tests/data/WPG-EXAMPLE-VENDOR-MIB.my") },
                { "name": "notes.txt", "content": "just some text" }
            ] }),
        )
        .await;
    r.expect(StatusCode::OK);
    let j = r.json();
    assert_eq!(j["files"][0]["stored"], true);
    assert_eq!(j["files"][0]["modules"], json!(["WPG-EXAMPLE-VENDOR-MIB"]));
    assert_eq!(j["files"][1]["stored"], false);
    assert!(!j["files"][1]["errors"].as_array().unwrap().is_empty());
    let m = &j["modules"][0];
    assert_eq!(m["status"], "partial");
    assert_eq!(m["missing_imports"], json!(["ACME-SMI-MISSING"]));
    assert!(app
        .state
        .config
        .mib_dir()
        .join("WPG-EXAMPLE-VENDOR-MIB.my")
        .is_file());

    let list = app.get("/api/snmp/mibs").await.json();
    let names: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"IF-MIB") && names.contains(&"WPG-EXAMPLE-VENDOR-MIB"),
        "{names:?}"
    );

    let hits = app
        .get("/api/snmp/mibs/search?q=wpgFan&limit=10")
        .await
        .json();
    assert!(hits
        .as_array()
        .unwrap()
        .iter()
        .any(|h| h["name"] == "wpgFanState" && h["role"] == "column"));
    let n = app.get("/api/snmp/mibs/node?id=wpgFanState.3").await.json();
    assert_eq!(n["oid"], "1.3.6.1.4.1.99999.1.1.1.3");
    assert_eq!(n["instance"], "3");
    assert_eq!(n["enums"][2]["label"], "failed");
    assert_eq!(n["access"], "read-only");
    let n = app.get("/api/snmp/mibs/node?id=ifEntry").await.json();
    assert_eq!(n["role"], "row");
    assert_eq!(n["index"], json!(["ifIndex"]));
    assert_eq!(n["path"][0]["name"], "iso");
    let kids = app.get("/api/snmp/mibs/tree?oid=wpgFanEntry").await.json();
    assert_eq!(kids.as_array().unwrap().len(), 5);
    let roots = app.get("/api/snmp/mibs/tree").await.json();
    assert!(roots.as_array().unwrap().iter().any(|c| c["name"] == "iso"));
    // Per module: its top nodes, then only its branches.
    let tops = app
        .get("/api/snmp/mibs/tree?module=WPG-EXAMPLE-VENDOR-MIB")
        .await
        .json();
    let tops = tops.as_array().unwrap();
    assert!(!tops.is_empty());
    assert!(tops.iter().all(|c| c["module"] == "WPG-EXAMPLE-VENDOR-MIB"));
    let below = app
        .get("/api/snmp/mibs/tree?module=WPG-EXAMPLE-VENDOR-MIB&oid=wpgFanEntry")
        .await
        .json();
    assert_eq!(below.as_array().unwrap().len(), 5);
    app.get("/api/snmp/mibs/tree?module=NOPE-MIB")
        .await
        .expect_error(StatusCode::BAD_REQUEST, "bad_request");

    // Survives a restart of the registry (persisted file).
    app.state.snmp.reload();
    assert!(app
        .get("/api/snmp/mibs/node?id=wpgFanTemp")
        .await
        .status
        .is_success());

    app.call(Method::DELETE, "/api/snmp/mibs/IF-MIB", None, None)
        .await
        .expect_error(StatusCode::CONFLICT, "read_only");
    let r = app
        .call(
            Method::DELETE,
            "/api/snmp/mibs/WPG-EXAMPLE-VENDOR-MIB",
            None,
            None,
        )
        .await;
    r.expect(StatusCode::OK);
    assert_eq!(r.json()["removed"], json!(["WPG-EXAMPLE-VENDOR-MIB"]));
    app.get("/api/snmp/mibs/node?id=wpgFanTemp")
        .await
        .expect_error(StatusCode::BAD_REQUEST, "bad_request");
    app.post(
        "/api/snmp/mibs",
        json!({ "files": [{ "name": "x.exe", "content": "" }] }),
    )
    .await
    .expect_error(StatusCode::BAD_REQUEST, "bad_request");
}

#[tokio::test]
async fn receiver_traps_informs_and_pcap() {
    let app = TestApp::new();
    let mut events = app.state.subscribe();
    let r = app
        .post(
            "/api/snmp/capture",
            json!({ "enabled": true, "clear": true }),
        )
        .await;
    r.expect(StatusCode::OK);
    let r = app
        .post(
            "/api/snmp/receiver/start",
            json!({ "listen": "127.0.0.1:0", "communities": ["public"],
                    "users": [{ "user": "trapper", "auth_protocol": "SHA", "auth_password": "trappass1", "priv_protocol": "DES", "priv_password": "trappass2" }] }),
        )
        .await;
    r.expect(StatusCode::OK);
    let st = r.json();
    assert_eq!(st["running"], true);
    assert_eq!(st["users"], json!(["trapper"]));
    let port: u16 = st["listen"]
        .as_str()
        .unwrap()
        .rsplit(':')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let to = |community: &str| json!({ "host": "127.0.0.1", "port": port, "community": community, "timeout_ms": 1000 });

    let r = app
        .post("/api/snmp/trap", json!({ "target": to("public"), "trap_oid": "linkDown", "uptime": 4200,
            "varbinds": [{ "oid": "ifIndex.2", "value": "2" }, { "oid": "ifOperStatus.2", "value": "down" }] }))
        .await;
    r.expect(StatusCode::OK);
    assert_eq!(r.json()["varbinds"][1]["value"], "IF-MIB::linkDown");
    let r = app
        .post(
            "/api/snmp/inform",
            json!({ "target": to("public"), "trap_oid": "IF-MIB::linkUp", "varbinds": [] }),
        )
        .await;
    r.expect(StatusCode::OK);
    assert_eq!(r.json()["op"], "inform");
    let mut v1 = to("public");
    v1["version"] = json!("v1");
    app.post(
        "/api/snmp/trap",
        json!({ "target": v1, "trap_oid": "coldStart" }),
    )
    .await
    .expect(StatusCode::OK);
    let mut v3 = to("");
    v3["version"] = json!("v3");
    v3["v3"] = json!({ "user": "trapper", "auth_protocol": "SHA", "auth_password": "trappass1", "priv_protocol": "DES", "priv_password": "trappass2" });
    app.post(
        "/api/snmp/inform",
        json!({ "target": v3, "trap_oid": "linkUp" }),
    )
    .await
    .expect(StatusCode::OK);

    let mut list = Json::Null;
    for _ in 0..100 {
        list = app.get("/api/snmp/traps").await.json();
        if list.as_array().unwrap().len() >= 4 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let l = list.as_array().unwrap();
    assert_eq!(l.len(), 4, "{list}");
    // newest first
    assert_eq!(l[0]["version"], "v3");
    assert_eq!(l[0]["user"], "trapper");
    assert_eq!(l[0]["acknowledged"], true);
    assert_eq!(l[0]["trap_name"], "IF-MIB::linkUp");
    assert_eq!(l[1]["version"], "v1");
    assert_eq!(l[1]["trap_name"], "SNMPv2-MIB::coldStart");
    assert_eq!(l[2]["pdu_type"], "inform");
    let trap = &l[3];
    assert_eq!(trap["trap_name"], "IF-MIB::linkDown");
    assert_eq!(trap["uptime"], 4200);
    assert_eq!(trap["varbinds"][1]["name"], "IF-MIB::ifOperStatus.2");
    assert_eq!(trap["varbinds"][1]["value"], "down(2)");
    // Pushed on the event bus too.
    let mut pushed = 0;
    while let Ok(ev) = events.try_recv() {
        if matches!(ev, wpg_server::events::ServerEvent::SnmpTrap { .. }) {
            pushed += 1;
        }
    }
    assert_eq!(pushed, 4);

    let cap = app.get("/api/snmp/capture").await.json();
    assert!(cap["frames"].as_u64().unwrap() >= 10, "{cap}");
    let r = app.get("/api/snmp/capture/download").await;
    r.expect(StatusCode::OK);
    assert_eq!(&r.bytes[..4], &[0xd4, 0xc3, 0xb2, 0xa1]);
    assert!(r.headers["content-disposition"]
        .to_str()
        .unwrap()
        .contains(".pcap\""));
    let r = app.get("/api/snmp/capture/download?format=pcapng").await;
    assert_eq!(&r.bytes[..4], &[0x0a, 0x0d, 0x0d, 0x0a]);
    app.get("/api/snmp/capture/download?format=cap")
        .await
        .expect_error(StatusCode::BAD_REQUEST, "bad_request");

    let st = app.post("/api/snmp/receiver/stop", json!({})).await.json();
    assert_eq!(st["running"], false);
    assert_eq!(st["received"], 4);
    app.call(Method::DELETE, "/api/snmp/traps", None, None)
        .await
        .expect(StatusCode::NO_CONTENT);
    assert_eq!(app.get("/api/snmp/traps").await.json(), json!([]));
    app.post(
        "/api/snmp/receiver/start",
        json!({ "listen": "not-an-address" }),
    )
    .await
    .expect_error(StatusCode::BAD_REQUEST, "bad_request");
}

#[tokio::test]
async fn disabled() {
    let app = TestApp::with(|c| c.snmp.enabled = false);
    let s = app.get("/api/snmp/status").await.json();
    assert_eq!(s["enabled"], false);
    app.get("/api/snmp/mibs")
        .await
        .expect_error(StatusCode::NOT_FOUND, "not_found");
    app.post(
        "/api/snmp/get",
        json!({ "target": { "host": "127.0.0.1" }, "oids": ["sysDescr.0"] }),
    )
    .await
    .expect_error(StatusCode::NOT_FOUND, "not_found");
}
