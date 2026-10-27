//! Web terminal over a real listener: a fake Telnet server and an in-process
//! `russh` SSH server on 127.0.0.1, driven through `/api/terminal/ws`.

mod common;

use std::borrow::Cow;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use russh::keys::ssh_key::private::{Ed25519Keypair, KeypairData};
use russh::keys::{HashAlg, PrivateKey};
use russh::server::{self, Auth, Msg, Server as _, Session};
use russh::{Channel, ChannelId, MethodKind, MethodSet};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use wpg_server::{Config, ServeOptions, Server};

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// A running wpg server with a logged-in client.
struct App {
    addr: SocketAddr,
    client: reqwest::Client,
    cookie: String,
    csrf: String,
    data_dir: std::path::PathBuf,
    _dir: tempfile::TempDir,
    serving: tokio::task::JoinHandle<std::io::Result<()>>,
}

impl App {
    async fn start(f: impl FnOnce(&mut Config)) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mut c = common::config(&dir);
        f(&mut c);
        let data_dir = c.data_dir.clone();
        let server = Server::bind(c, ServeOptions::default()).await.unwrap();
        let addr = server.local_addr().unwrap();
        let serving = tokio::spawn(server.run_until(std::future::pending()));
        let (client, csrf, cookie) = common::http_login(&format!("http://{addr}")).await;
        Self {
            addr,
            client,
            cookie,
            csrf,
            data_dir,
            _dir: dir,
            serving,
        }
    }

    fn enabled() -> impl FnOnce(&mut Config) {
        |c: &mut Config| c.terminal.enabled = true
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.addr)
    }

    async fn get(&self, path: &str) -> reqwest::Response {
        self.client.get(self.url(path)).send().await.unwrap()
    }

    async fn delete(&self, path: &str) -> reqwest::Response {
        self.client
            .delete(self.url(path))
            .header("x-csrf-token", &self.csrf)
            .send()
            .await
            .unwrap()
    }

    fn ws_request(&self) -> tokio_tungstenite::tungstenite::handshake::client::Request {
        let mut req = format!("ws://{}/api/terminal/ws", self.addr)
            .into_client_request()
            .unwrap();
        req.headers_mut()
            .insert("cookie", self.cookie.parse().unwrap());
        req.headers_mut()
            .insert("origin", format!("http://{}", self.addr).parse().unwrap());
        req
    }

    async fn open(&self, connect: Value) -> Term {
        let (ws, _) = tokio_tungstenite::connect_async(self.ws_request())
            .await
            .unwrap();
        let mut t = Term {
            ws,
            output: Vec::new(),
        };
        t.send(connect).await;
        t
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.serving.abort();
    }
}

/// The browser side of a session.
struct Term {
    ws: Ws,
    /// Binary output received so far.
    output: Vec<u8>,
}

impl Term {
    async fn send(&mut self, v: Value) {
        self.ws
            .send(Message::Text(v.to_string().into()))
            .await
            .unwrap();
    }

    /// The next JSON message (binary frames are collected into `output`).
    async fn json(&mut self) -> Value {
        loop {
            let msg = tokio::time::timeout(Duration::from_secs(15), self.ws.next())
                .await
                .expect("no message within 15 s")
                .expect("socket ended")
                .expect("socket error");
            match msg {
                Message::Text(t) => return serde_json::from_str(&t).unwrap(),
                Message::Binary(b) => self.output.extend_from_slice(&b),
                Message::Close(_) => panic!("closed; output: {:?}", self.text()),
                _ => {}
            }
        }
    }

    /// Skips `state` messages; returns the next other JSON message.
    async fn event(&mut self) -> Value {
        loop {
            let v = self.json().await;
            if v["type"] != "state" {
                return v;
            }
        }
    }

    /// Reads until the output contains `needle`.
    async fn expect_output(&mut self, needle: &str) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        while !self.text().contains(needle) {
            let msg = tokio::time::timeout_at(deadline, self.ws.next())
                .await
                .unwrap_or_else(|_| panic!("{needle:?} not seen; output: {:?}", self.text()))
                .expect("socket ended")
                .expect("socket error");
            match msg {
                Message::Binary(b) => self.output.extend_from_slice(&b),
                Message::Text(t) => {
                    let v: Value = serde_json::from_str(&t).unwrap();
                    assert_ne!(v["type"], "closed", "closed early: {v}");
                }
                _ => {}
            }
        }
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.output).into_owned()
    }

    async fn input(&mut self, s: &str) {
        self.send(json!({ "type": "input", "data": s })).await;
    }

    /// `disconnect`, then the `closed` message.
    async fn disconnect(&mut self) -> Value {
        self.send(json!({ "type": "disconnect" })).await;
        self.until("closed").await
    }

    async fn until(&mut self, ty: &str) -> Value {
        loop {
            let v = self.json().await;
            if v["type"] == ty {
                return v;
            }
        }
    }
}

// ------------------------------------------------------------------ telnet

const IAC: u8 = 255;

/// A Telnet server: negotiates ECHO / SGA / NAWS, prints a prompt and echoes
/// lower-case letters (CR as CR LF). Every byte it receives goes to the channel.
async fn telnet_server() -> (u16, mpsc::UnboundedReceiver<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            let Ok((mut s, _)) = listener.accept().await else {
                return;
            };
            let tx = tx.clone();
            tokio::spawn(async move {
                s.write_all(&[IAC, 251, 1, IAC, 251, 3, IAC, 253, 31])
                    .await
                    .unwrap();
                s.write_all(b"\x1b[1mlogin:\x1b[0m ").await.unwrap();
                let mut buf = [0u8; 1024];
                loop {
                    let n = match s.read(&mut buf).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => n,
                    };
                    let _ = tx.send(buf[..n].to_vec());
                    let mut echo = Vec::new();
                    for &b in &buf[..n] {
                        match b {
                            b'a'..=b'z' => echo.push(b),
                            b'\r' => echo.extend_from_slice(b"\r\n"),
                            _ => {}
                        }
                    }
                    if !echo.is_empty() && s.write_all(&echo).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    (port, rx)
}

/// Waits until the bytes received by the fake server contain `needle`.
async fn server_saw(rx: &mut mpsc::UnboundedReceiver<Vec<u8>>, seen: &mut Vec<u8>, needle: &[u8]) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while !seen.windows(needle.len()).any(|w| w == needle) {
        let chunk = tokio::time::timeout_at(deadline, rx.recv())
            .await
            .unwrap_or_else(|_| panic!("server never saw {needle:?}; got {seen:?}"))
            .unwrap();
        seen.extend_from_slice(&chunk);
    }
}

fn telnet(port: u16) -> Value {
    json!({ "type": "connect", "protocol": "telnet", "host": "127.0.0.1", "port": port })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disabled_by_default() {
    let app = App::start(|_| {}).await;
    let s: Value = app.get("/api/terminal/status").await.json().await.unwrap();
    assert_eq!(s["enabled"], false);
    assert_eq!(s["max_sessions"], 8);
    let e = tokio_tungstenite::connect_async(app.ws_request())
        .await
        .unwrap_err();
    match e {
        tokio_tungstenite::tungstenite::Error::Http(resp) => {
            assert_eq!(resp.status(), 403);
            let body = String::from_utf8_lossy(resp.body().as_deref().unwrap_or_default());
            assert!(body.contains("terminal_disabled"), "{body}");
        }
        e => panic!("expected HTTP 403, got {e}"),
    }
    // logs and host keys stay readable
    assert_eq!(app.get("/api/terminal/logs").await.status(), 200);
    assert_eq!(app.get("/api/terminal/known-hosts").await.status(), 200);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_needs_session_and_same_origin() {
    let app = App::start(App::enabled()).await;
    let mut req = app.ws_request();
    req.headers_mut().remove("cookie");
    let e = tokio_tungstenite::connect_async(req).await.unwrap_err();
    assert!(e.to_string().contains("401"), "{e}");
    let mut req = app.ws_request();
    req.headers_mut()
        .insert("origin", "https://evil.example".parse().unwrap());
    let e = tokio_tungstenite::connect_async(req).await.unwrap_err();
    assert!(e.to_string().contains("403"), "{e}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn telnet_session_with_log() {
    let (port, mut rx) = telnet_server().await;
    let app = App::start(App::enabled()).await;
    let mut seen = Vec::new();
    let mut t = app.open(telnet(port)).await;
    let connected = t.event().await;
    assert_eq!(connected["type"], "connected", "{connected}");
    assert_eq!(connected["protocol"], "telnet");
    assert_eq!(connected["port"], port);
    let rec = t.event().await;
    assert_eq!(rec["type"], "record");
    assert_eq!(rec["enabled"], true);
    let log = rec["file"].as_str().unwrap().to_owned();
    assert!(log.contains("_telnet_127.0.0.1_"), "{log}");

    t.expect_output("login:").await;
    // negotiation: DO ECHO, DO SGA, WILL NAWS + the window size
    server_saw(
        &mut rx,
        &mut seen,
        &[IAC, 253, 1, IAC, 253, 3, IAC, 251, 31],
    )
    .await;
    server_saw(&mut rx, &mut seen, &[IAC, 250, 31, 0, 80, 0, 24, IAC, 240]).await;
    // keystrokes: CR goes out as CR NUL
    t.input("hello\r").await;
    server_saw(&mut rx, &mut seen, b"hello\r\0").await;
    t.expect_output("hello\r\n").await;
    // binary frames are keystrokes too
    t.ws.send(Message::Binary(b"bin".to_vec().into()))
        .await
        .unwrap();
    server_saw(&mut rx, &mut seen, b"bin").await;
    // resize -> NAWS
    t.send(json!({ "type": "resize", "cols": 132, "rows": 43 }))
        .await;
    server_saw(&mut rx, &mut seen, &[IAC, 250, 31, 0, 132, 0, 43, IAC, 240]).await;

    // the running log is listed as active and cannot be deleted
    let logs: Value = app.get("/api/terminal/logs").await.json().await.unwrap();
    assert_eq!(logs[0]["name"], log.as_str());
    assert_eq!(logs[0]["active"], true);
    let r = app.delete(&format!("/api/terminal/logs/{log}")).await;
    assert_eq!(r.status(), 409);
    let s: Value = app.get("/api/terminal/status").await.json().await.unwrap();
    assert_eq!(s["sessions"][0]["log"], log.as_str());

    // stop / restart recording
    t.send(json!({ "type": "record", "enabled": false })).await;
    let r = t.until("record").await;
    assert_eq!(r["enabled"], false);
    assert_eq!(r["file"], log.as_str());

    let closed = t.disconnect().await;
    assert_eq!(closed["reason"], "client");
    assert!(closed["bytes_in"].as_u64().unwrap() > 0);
    assert!(closed["bytes_out"].as_u64().unwrap() >= 9);

    // the log holds the remote output without escape sequences
    let r = app.get(&format!("/api/terminal/logs/{log}")).await;
    assert_eq!(r.status(), 200);
    assert!(r
        .headers()
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap()
        .contains(&log));
    let body = r.text().await.unwrap();
    assert!(body.starts_with("login: hello\n"), "{body:?}");
    assert!(!body.contains('\x1b'));
    let logs: Value = app.get("/api/terminal/logs").await.json().await.unwrap();
    assert_eq!(logs[0]["active"], false);
    assert_eq!(
        app.get("/api/terminal/status")
            .await
            .json::<Value>()
            .await
            .unwrap()["sessions"],
        json!([])
    );
    assert_eq!(
        app.delete(&format!("/api/terminal/logs/{log}"))
            .await
            .status(),
        204
    );
    assert_eq!(
        app.get(&format!("/api/terminal/logs/{log}")).await.status(),
        404
    );
    assert_eq!(
        app.get("/api/terminal/logs/..%2Fwpg.sqlite").await.status(),
        404
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn telnet_without_log_and_idle_timeout() {
    let (port, _rx) = telnet_server().await;
    let app = App::start(|c| {
        c.terminal.enabled = true;
        c.terminal.idle_timeout_secs = 1;
    })
    .await;
    let mut connect = telnet(port);
    connect["record"] = json!(false);
    let mut t = app.open(connect).await;
    assert_eq!(t.event().await["type"], "connected");
    let rec = t.event().await;
    assert_eq!(rec, json!({ "type": "record", "enabled": false }));
    let closed = t.until("closed").await;
    assert_eq!(closed["reason"], "idle");
    let logs: Value = app.get("/api/terminal/logs").await.json().await.unwrap();
    assert_eq!(logs, json!([]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn policy_refusals() {
    let (port, _rx) = telnet_server().await;
    // allowlist
    let app = App::start(|c| {
        c.terminal.enabled = true;
        c.terminal.allowed_targets = vec!["192.0.2.0/24".into(), "localhost:1".into()];
    })
    .await;
    let mut t = app.open(telnet(port)).await;
    let e = t.event().await;
    assert_eq!(e["type"], "error");
    assert_eq!(e["code"], "target_not_allowed");
    assert_eq!(e["fatal"], true);
    assert_eq!(t.until("closed").await["reason"], "error");
    // a malformed request
    let mut t = app
        .open(json!({ "type": "connect", "protocol": "ssh", "host": "127.0.0.1" }))
        .await;
    let e = t.event().await;
    assert_eq!(
        (e["code"].as_str(), e["detail"].as_str()),
        (Some("bad_request"), Some("username"))
    );
    let mut t = app
        .open(json!({ "type": "resize", "cols": 1, "rows": 1 }))
        .await;
    assert_eq!(t.event().await["code"], "bad_request");
    drop(app);

    // session limit
    let app = App::start(|c| {
        c.terminal.enabled = true;
        c.terminal.max_sessions = 1;
        c.terminal.allowed_targets = vec!["127.0.0.1".into()];
    })
    .await;
    let mut first = app.open(telnet(port)).await;
    assert_eq!(first.event().await["type"], "connected");
    let mut second = app.open(telnet(port)).await;
    let e = second.event().await;
    assert_eq!(e["code"], "too_many_sessions", "{e}");
    first.disconnect().await;
    let mut third = app.open(telnet(port)).await;
    assert_eq!(third.event().await["type"], "connected");
}

// ------------------------------------------------------------------ ssh

fn host_key(seed: u8) -> PrivateKey {
    PrivateKey::new(
        KeypairData::Ed25519(Ed25519Keypair::from_seed(&[seed; 32])),
        "",
    )
    .unwrap()
}

fn fingerprint(seed: u8) -> String {
    host_key(seed)
        .public_key()
        .fingerprint(HashAlg::Sha256)
        .to_string()
}

/// An SSH server accepting `lab` / `pw` (password or keyboard-interactive),
/// echoing the shell input and recording the PTY sizes.
#[derive(Clone)]
struct SshServer {
    sizes: Arc<Mutex<Vec<(u32, u32)>>>,
}

impl server::Server for SshServer {
    type Handler = Self;
    fn new_client(&mut self, _: Option<SocketAddr>) -> Self {
        self.clone()
    }
}

impl server::Handler for SshServer {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        Ok(if user == "lab" && password == "pw" {
            Auth::Accept
        } else {
            // like OpenSSH: password stays available
            Auth::Reject {
                proceed_with_methods: Some(MethodSet::from(&[MethodKind::Password][..])),
                partial_success: false,
            }
        })
    }

    async fn auth_publickey(
        &mut self,
        user: &str,
        key: server::VerifiedPublicKey<'_>,
    ) -> Result<Auth, Self::Error> {
        Ok(
            if user == "lab" && key.public_key.key_data() == host_key(9).public_key().key_data() {
                Auth::Accept
            } else {
                Auth::reject()
            },
        )
    }

    async fn auth_keyboard_interactive<'a>(
        &'a mut self,
        user: &str,
        _submethods: &str,
        response: Option<server::Response<'a>>,
    ) -> Result<Auth, Self::Error> {
        Ok(match response {
            None => Auth::Partial {
                name: Cow::Borrowed(""),
                instructions: Cow::Borrowed(""),
                prompts: Cow::Owned(vec![(Cow::Borrowed("Password: "), false)]),
            },
            Some(mut r) => {
                let answer = r.next();
                if user == "lab" && answer.as_deref() == Some(b"pw".as_slice()) {
                    Auth::Accept
                } else {
                    Auth::reject()
                }
            }
        })
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        reply: server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        term: &str,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        assert_eq!(term, "xterm-256color");
        self.sizes.lock().unwrap().push((col_width, row_height));
        session.channel_success(channel)
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        session.data(channel, b"welcome\r\n$ ".to_vec())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.data(channel, data.to_vec())
    }

    async fn window_change_request(
        &mut self,
        _channel: ChannelId,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.sizes.lock().unwrap().push((col_width, row_height));
        Ok(())
    }
}

async fn ssh_server(seed: u8, methods: &[MethodKind]) -> (u16, Arc<Mutex<Vec<(u32, u32)>>>) {
    let config = Arc::new(server::Config {
        keys: vec![host_key(seed)],
        methods: MethodSet::from(methods),
        auth_rejection_time: Duration::from_millis(10),
        auth_rejection_time_initial: Some(Duration::ZERO),
        ..server::Config::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let sizes = Arc::new(Mutex::new(Vec::new()));
    let mut srv = SshServer {
        sizes: Arc::clone(&sizes),
    };
    tokio::spawn(async move {
        let _ = srv.run_on_socket(config, &listener).await;
    });
    (port, sizes)
}

fn ssh(port: u16, password: Option<&str>) -> Value {
    json!({
        "type": "connect", "protocol": "ssh", "host": "127.0.0.1", "port": port,
        "username": "lab", "password": password, "cols": 100, "rows": 30, "record": false,
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ssh_session_with_tofu() {
    let (port, sizes) = ssh_server(1, &[MethodKind::Password]).await;
    let app = App::start(App::enabled()).await;

    // first contact: the fingerprint is shown and stored on acceptance
    let mut t = app.open(ssh(port, Some("pw"))).await;
    let hk = t.event().await;
    assert_eq!(hk["type"], "host_key", "{hk}");
    assert_eq!(hk["fingerprint"], fingerprint(1));
    assert_eq!(hk["algorithm"], "ssh-ed25519");
    assert_eq!(hk["port"], port);
    t.send(json!({ "type": "host_key", "accept": true, "save": true }))
        .await;
    let c = t.event().await;
    assert_eq!(c["type"], "connected", "{c}");
    assert_eq!(c["fingerprint"], fingerprint(1));
    assert_eq!(
        t.event().await,
        json!({ "type": "record", "enabled": false })
    );
    t.expect_output("welcome\r\n$ ").await;
    t.input("show version\r").await;
    t.expect_output("show version\r").await;
    t.send(json!({ "type": "resize", "cols": 120, "rows": 40 }))
        .await;
    t.input("x").await;
    t.expect_output("\rx").await;
    for _ in 0..100 {
        if sizes.lock().unwrap().len() >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(*sizes.lock().unwrap(), [(100, 30), (120, 40)]);
    let closed = t.disconnect().await;
    assert_eq!(closed["reason"], "client");
    assert_eq!(closed["bytes_out"], 14);

    let keys: Value = app
        .get("/api/terminal/known-hosts")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(keys.as_array().unwrap().len(), 1);
    assert_eq!(keys[0]["host"], "127.0.0.1");
    assert_eq!(keys[0]["port"], port);
    assert_eq!(keys[0]["fingerprint"], fingerprint(1));
    let file = std::fs::read_to_string(app.data_dir.join("terminal/known_hosts")).unwrap();
    assert!(
        file.starts_with(&format!("[127.0.0.1]:{port} ssh-ed25519 ")),
        "{file}"
    );

    // second contact: trusted silently; a wrong password leads to a prompt
    let mut t = app.open(ssh(port, Some("wrong"))).await;
    let p = t.event().await;
    assert_eq!(p["type"], "auth_prompt", "{p}");
    assert_eq!(p["prompts"][0]["echo"], false);
    assert!(p["prompts"][0]["prompt"]
        .as_str()
        .unwrap()
        .contains("lab@127.0.0.1"));
    t.send(json!({ "type": "auth_response", "responses": ["pw"] }))
        .await;
    assert_eq!(t.event().await["type"], "connected");
    t.expect_output("welcome").await;
    t.disconnect().await;

    // without any password the prompts eventually fail
    let mut t = app.open(ssh(port, None)).await;
    for _ in 0..3 {
        assert_eq!(t.event().await["type"], "auth_prompt");
        t.send(json!({ "type": "auth_response", "responses": ["nope"] }))
            .await;
    }
    let e = t.event().await;
    assert_eq!(e["code"], "auth_failed", "{e}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ssh_host_key_mismatch_and_rejection() {
    let (port, _) = ssh_server(2, &[MethodKind::Password]).await;
    let app = App::start(App::enabled()).await;
    // a different key is on file for this host
    std::fs::create_dir_all(app.data_dir.join("terminal")).unwrap();
    std::fs::write(
        app.data_dir.join("terminal/known_hosts"),
        format!(
            "[127.0.0.1]:{port} {}\n",
            host_key(1).public_key().to_openssh().unwrap()
        ),
    )
    .unwrap();
    let mut t = app.open(ssh(port, Some("pw"))).await;
    let e = t.event().await;
    assert_eq!(e["code"], "host_key_changed", "{e}");
    assert_eq!(e["fatal"], true);
    assert_eq!(e["info"]["fingerprint"], fingerprint(2));
    assert_eq!(e["info"]["known_fingerprint"], fingerprint(1));
    assert_eq!(t.until("closed").await["reason"], "error");

    // the administrator removes the stale key; the user then declines the new one
    let r = app
        .delete(&format!("/api/terminal/known-hosts/127.0.0.1/{port}"))
        .await;
    assert_eq!(r.status(), 204);
    let r = app
        .delete(&format!("/api/terminal/known-hosts/127.0.0.1/{port}"))
        .await;
    assert_eq!(r.status(), 404);
    let mut t = app.open(ssh(port, Some("pw"))).await;
    assert_eq!(t.event().await["type"], "host_key");
    t.send(json!({ "type": "host_key", "accept": false })).await;
    let e = t.event().await;
    assert_eq!(e["code"], "host_key_rejected", "{e}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ssh_keyboard_interactive() {
    let (port, _) = ssh_server(3, &[MethodKind::KeyboardInteractive]).await;
    let app = App::start(App::enabled()).await;
    // the given password answers the lone hidden prompt; the key is trusted once
    for _ in 0..2 {
        let mut t = app.open(ssh(port, Some("pw"))).await;
        assert_eq!(t.event().await["type"], "host_key");
        t.send(json!({ "type": "host_key", "accept": true, "save": false }))
            .await;
        assert_eq!(t.event().await["type"], "connected");
        t.expect_output("welcome").await;
        t.disconnect().await;
    }
    // without a password the prompt goes to the browser
    let mut t = app.open(ssh(port, None)).await;
    t.event().await;
    t.send(json!({ "type": "host_key", "accept": true })).await;
    let p = t.event().await;
    assert_eq!(p["type"], "auth_prompt");
    assert_eq!(
        p["prompts"],
        json!([{ "prompt": "Password: ", "echo": false }])
    );
    t.send(json!({ "type": "auth_response", "responses": ["pw"] }))
        .await;
    assert_eq!(t.event().await["type"], "connected");
    assert!(!app.data_dir.join("terminal/known_hosts").exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ssh_public_key() {
    let (port, _) = ssh_server(4, &[MethodKind::PublicKey]).await;
    let app = App::start(App::enabled()).await;
    let pem = |seed| {
        host_key(seed)
            .to_openssh(russh::keys::ssh_key::LineEnding::LF)
            .unwrap()
            .to_string()
    };
    let mut connect = ssh(port, None);
    connect["private_key"] = json!(pem(9));
    let mut t = app.open(connect.clone()).await;
    t.event().await;
    t.send(json!({ "type": "host_key", "accept": true, "save": true }))
        .await;
    assert_eq!(t.event().await["type"], "connected");
    t.expect_output("welcome").await;
    t.disconnect().await;
    // another key is refused; garbage is reported as such
    connect["private_key"] = json!(pem(8));
    let mut t = app.open(connect.clone()).await;
    assert_eq!(t.event().await["code"], "auth_failed");
    connect["private_key"] = json!("-----BEGIN OPENSSH PRIVATE KEY-----\nnope\n");
    let mut t = app.open(connect).await;
    assert_eq!(t.event().await["code"], "key_invalid");
}
