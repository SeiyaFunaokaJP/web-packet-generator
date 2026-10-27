//! WebSocket event stream over a real listener (port 0).

mod common;

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;
use wpg_server::{ServeOptions, Server};

async fn next_json<S>(ws: &mut S) -> Value
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(10), ws.next())
            .await
            .expect("no WebSocket message within 10 s")
            .expect("stream ended")
            .expect("WebSocket error");
        if let Message::Text(t) = msg {
            return serde_json::from_str(&t).unwrap();
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn job_events_over_websocket() {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::bind(common::config(&dir), ServeOptions::default())
        .await
        .unwrap();
    let addr = server.local_addr().unwrap();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let serving = tokio::spawn(server.run_until(async {
        let _ = stop_rx.await;
    }));
    let base = format!("http://{addr}");
    let http = |path: &str| format!("{base}{path}");
    let (_client, csrf, cookie) = common::http_login(&base).await;
    let auth = Auth {
        cookie: &cookie,
        csrf: &csrf,
    };

    let (mut ws, _) = tokio_tungstenite::connect_async(ws_request(addr, Some(&cookie)))
        .await
        .unwrap();
    ws.send(Message::Text(
        json!({ "type": "subscribe", "jobs": "all", "logs": false })
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    // give the server a moment to apply the subscription
    tokio::time::sleep(Duration::from_millis(100)).await;

    // create a job over plain HTTP (hyper-less: raw TCP request)
    let body = json!({
        "streams": [{ "template_id": "examples-arp-sweep" }],
        "backend": "null",
        "rate": "2000pps",
        "count": 100_000_000,
    })
    .to_string();
    let resp = raw_http(addr, &auth, "POST", "/api/jobs", Some(&body)).await;
    assert!(resp.starts_with("HTTP/1.1 201"), "{resp}");
    let job: Value = serde_json::from_str(resp.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    let id = job["id"].as_str().unwrap().to_owned();

    // stats arrive (100 ms cadence) and carry growing counters
    let mut stats = 0;
    let mut last_packets = 0;
    while stats < 3 {
        let ev = next_json(&mut ws).await;
        if ev["type"] == "stats" {
            assert_eq!(ev["job_id"], id);
            let p = ev["snapshot"]["packets"].as_u64().unwrap();
            assert!(p >= last_packets);
            last_packets = p;
            stats += 1;
        }
    }
    assert!(last_packets > 0);

    // stop → job_state finished/stopped
    let resp = raw_http(addr, &auth, "POST", &format!("/api/jobs/{id}/stop"), None).await;
    assert!(resp.starts_with("HTTP/1.1 200"), "{resp}");
    loop {
        let ev = next_json(&mut ws).await;
        if ev["type"] == "job_state" && ev["state"]["state"] == "finished" {
            assert_eq!(ev["job_id"], id);
            assert_eq!(ev["state"]["detail"], "stopped");
            break;
        }
    }
    let resp = raw_http(addr, &auth, "GET", "/api/jobs/history", None).await;
    assert!(resp.contains(&id), "{resp}");

    // template changes are announced
    std::fs::write(
        dir.path().join("data/templates/new.wpg.yaml"),
        "schema: 1\nmeta:\n  name: new\n",
    )
    .unwrap();
    let resp = raw_http(
        addr,
        &auth,
        "POST",
        "/api/templates/import",
        Some("schema: 1\nmeta:\n  name: via api\n"),
    )
    .await;
    assert!(resp.starts_with("HTTP/1.1 201"), "{resp}");
    loop {
        if next_json(&mut ws).await["type"] == "templates_changed" {
            break;
        }
    }

    // graceful shutdown closes the socket and ends the server
    let _ = http;
    stop_tx.send(()).unwrap();
    let closed = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(Ok(m)) = ws.next().await {
            if matches!(m, Message::Close(_)) {
                break;
            }
        }
    })
    .await;
    assert!(closed.is_ok(), "socket not closed on shutdown");
    tokio::time::timeout(Duration::from_secs(5), serving)
        .await
        .expect("server did not stop")
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn log_lines_reach_websocket_clients() {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::bind(common::config(&dir), ServeOptions::default())
        .await
        .unwrap();
    let addr = server.local_addr().unwrap();
    let logs = server.state().logs.clone();
    let serving = tokio::spawn(server.run_until(std::future::pending()));
    let (_client, _csrf, cookie) = common::http_login(&format!("http://{addr}")).await;
    let (mut ws, _) = tokio_tungstenite::connect_async(ws_request(addr, Some(&cookie)))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    // the global subscriber with the WebSocket sink (as `wpg serve` installs it)
    let _guard = wpg_server::init_tracing(&wpg_server::config::LogConfig {
        level: "info".into(),
        ..wpg_server::config::LogConfig::default()
    })
    .unwrap();
    assert!(logs.receiver_count() >= 1);
    tracing::info!(target: "wpg_test", "hello from the log");
    loop {
        let ev = next_json(&mut ws).await;
        // The subscriber is process-global: log lines of the tests running in
        // parallel (jobs, captures) can arrive first.
        if ev["type"] == "log" && ev["target"] == "wpg_test" {
            assert_eq!(ev["message"], "hello from the log");
            assert_eq!(ev["level"], "INFO");
            assert_eq!(ev["target"], "wpg_test");
            assert!(ev["ts"].is_string());
            break;
        }
    }
    serving.abort();
}

/// Session cookie + CSRF token for [`raw_http`].
struct Auth<'a> {
    cookie: &'a str,
    csrf: &'a str,
}

/// `GET /api/ws` upgrade request, with the session cookie when given.
fn ws_request(
    addr: std::net::SocketAddr,
    cookie: Option<&str>,
) -> tokio_tungstenite::tungstenite::handshake::client::Request {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let mut req = format!("ws://{addr}/api/ws").into_client_request().unwrap();
    if let Some(c) = cookie {
        req.headers_mut().insert("cookie", c.parse().unwrap());
    }
    req
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_requires_a_session_and_same_origin() {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::bind(common::config(&dir), ServeOptions::default())
        .await
        .unwrap();
    let addr = server.local_addr().unwrap();
    let serving = tokio::spawn(server.run_until(std::future::pending()));
    // before setup
    let e = tokio_tungstenite::connect_async(ws_request(addr, None))
        .await
        .unwrap_err();
    assert!(e.to_string().contains("401"), "{e}");
    let (_client, _csrf, cookie) = common::http_login(&format!("http://{addr}")).await;
    // no cookie / a wrong cookie
    for c in [None, Some("wpg_session=0000")] {
        let e = tokio_tungstenite::connect_async(ws_request(addr, c))
            .await
            .unwrap_err();
        assert!(e.to_string().contains("401"), "{e}");
    }
    // cross-site origin
    let mut req = ws_request(addr, Some(&cookie));
    req.headers_mut()
        .insert("origin", "https://evil.example".parse().unwrap());
    let e = tokio_tungstenite::connect_async(req).await.unwrap_err();
    assert!(e.to_string().contains("403"), "{e}");
    // same origin with the cookie
    let mut req = ws_request(addr, Some(&cookie));
    req.headers_mut()
        .insert("origin", format!("http://{addr}").parse().unwrap());
    let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();
    ws.close(None).await.unwrap();
    serving.abort();
}

/// Minimal HTTP/1.1 client over a raw socket.
async fn raw_http(
    addr: std::net::SocketAddr,
    auth: &Auth<'_>,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
    let body = body.unwrap_or("");
    let ctype = if body.starts_with('{') {
        "application/json"
    } else {
        "text/yaml"
    };
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nCookie: {}\r\nX-CSRF-Token: {}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\n\r\n{body}",
        auth.cookie,
        auth.csrf,
        body.len()
    );
    s.write_all(req.as_bytes()).await.unwrap();
    let mut out = Vec::new();
    s.read_to_end(&mut out).await.unwrap();
    String::from_utf8_lossy(&out).into_owned()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn capture_events_over_websocket() {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::bind(common::config(&dir), ServeOptions::default())
        .await
        .unwrap();
    let addr = server.local_addr().unwrap();
    let serving = tokio::spawn(server.run_until(std::future::pending()));
    let (_client, csrf, cookie) = common::http_login(&format!("http://{addr}")).await;
    let auth = Auth {
        cookie: &cookie,
        csrf: &csrf,
    };
    let (mut ws, _) = tokio_tungstenite::connect_async(ws_request(addr, Some(&cookie)))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let body = json!({ "backend": "null", "filter": "udp" }).to_string();
    let resp = raw_http(addr, &auth, "POST", "/api/captures", Some(&body)).await;
    assert!(resp.starts_with("HTTP/1.1 201"), "{resp}");
    let cap: Value = serde_json::from_str(resp.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    let id = cap["id"].as_str().unwrap().to_owned();

    let (mut running, mut stats) = (false, 0);
    while !running || stats < 2 {
        let ev = next_json(&mut ws).await;
        match ev["type"].as_str() {
            Some("capture_state") => {
                assert_eq!(ev["capture_id"], id);
                assert_eq!(ev["state"]["state"], "running");
                running = true;
            }
            Some("rx_stats") => {
                assert_eq!(ev["capture_id"], id);
                assert_eq!(ev["snapshot"]["packets"], 0);
                stats += 1;
            }
            _ => {}
        }
    }
    let resp = raw_http(
        addr,
        &auth,
        "POST",
        &format!("/api/captures/{id}/stop"),
        None,
    )
    .await;
    assert!(resp.starts_with("HTTP/1.1 200"), "{resp}");
    loop {
        let ev = next_json(&mut ws).await;
        if ev["type"] == "capture_state" {
            assert_eq!(
                ev["state"],
                json!({ "state": "finished", "detail": "stopped" })
            );
            break;
        }
    }
    serving.abort();
}
