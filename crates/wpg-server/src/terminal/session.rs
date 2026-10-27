//! One web terminal session on `/api/terminal/ws`.
//!
//! 1. The browser sends [`ClientMsg::Connect`] (within 30 s).
//! 2. The server checks the host, resolves it, keeps the addresses the allowlist
//!    admits, takes a session slot and dials them in turn (`state` messages report
//!    progress).
//! 3. SSH: key exchange (an unknown host key is shown as `host_key` and waits for
//!    the user's answer; a changed one is refused), then authentication in the
//!    order public key → password → keyboard-interactive (prompts go to the
//!    browser as `auth_prompt`; a single hidden prompt is answered with the given
//!    password) → password prompt; then a PTY (`xterm-256color`) and a shell.
//!    Telnet: the connection is the session.
//! 4. `connected`; remote output flows as binary frames, keystrokes come back as
//!    binary frames or `input`; `resize`, `record` and `disconnect` control it.
//! 5. `closed` with the reason and byte counts, then the WebSocket closes.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket};
use russh::client::{self, KeyboardInteractiveAuthResponse};
use russh::keys::PrivateKeyWithHashAlg;
use russh::{ChannelMsg, MethodKind, MethodSet};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, watch};

use super::recorder::{self, Recorder};
use super::ssh::{self, HostKeyAnswer, SshEvent};
use super::telnet::Telnet;
use super::{
    target, ActiveSession, ClientMsg, ConnectRequest, PromptItem, Protocol, ServerMsg, Slot,
    Terminal, TERM,
};
use crate::auth::{now_secs, Principal, AUDIT};

/// How long the browser has to send `connect`.
const CONNECT_WAIT: Duration = Duration::from_secs(30);
/// How long a host key / authentication prompt waits for the user.
const PROMPT_WAIT: Duration = Duration::from_secs(180);
/// Name resolution limit.
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(10);
/// Per-address TCP connect limit.
const DIAL_TIMEOUT: Duration = Duration::from_secs(10);
/// SSH handshake / each authentication exchange (without user prompts).
const SSH_STEP_TIMEOUT: Duration = Duration::from_secs(30);
/// Interactive password prompts after the given password failed / was missing.
const PASSWORD_PROMPTS: usize = 3;
/// Keyboard-interactive rounds before giving up.
const KI_ROUNDS: usize = 10;

/// Why the session ended early.
#[derive(Debug)]
enum Stop {
    /// The browser went away / sent `disconnect`.
    Client,
    /// The server shuts down.
    Shutdown,
    /// A failure reported to the browser.
    Fail(Fail),
}

#[derive(Debug)]
struct Fail {
    code: &'static str,
    detail: Option<String>,
    info: Option<serde_json::Value>,
}

fn fail(code: &'static str, detail: impl Into<Option<String>>) -> Stop {
    Stop::Fail(Fail {
        code,
        detail: detail.into(),
        info: None,
    })
}

/// A message from the browser.
enum In {
    Msg(ClientMsg),
    Bytes(Vec<u8>),
    Bad(String),
    Closed,
}

async fn recv(socket: &mut WebSocket) -> In {
    loop {
        return match socket.recv().await {
            None | Some(Err(_) | Ok(Message::Close(_))) => In::Closed,
            Some(Ok(Message::Text(t))) => match serde_json::from_str::<ClientMsg>(&t) {
                Ok(m) => In::Msg(m),
                Err(e) => In::Bad(e.to_string()),
            },
            Some(Ok(Message::Binary(b))) => In::Bytes(b.to_vec()),
            Some(Ok(_)) => continue,
        };
    }
}

async fn send(socket: &mut WebSocket, m: &ServerMsg) -> bool {
    match serde_json::to_string(m) {
        Ok(t) => socket.send(Message::Text(t.into())).await.is_ok(),
        Err(_) => true,
    }
}

async fn send_data(socket: &mut WebSocket, data: &[u8]) -> bool {
    data.is_empty()
        || socket
            .send(Message::Binary(data.to_vec().into()))
            .await
            .is_ok()
}

async fn state(socket: &mut WebSocket, s: &str) {
    send(
        socket,
        &ServerMsg::State {
            state: s.to_owned(),
        },
    )
    .await;
}

async fn sleep_until_opt(t: Option<tokio::time::Instant>) {
    match t {
        Some(t) => tokio::time::sleep_until(t).await,
        None => std::future::pending().await,
    }
}

/// Resolves when the server starts shutting down.
async fn shutting_down(rx: &mut watch::Receiver<bool>) {
    loop {
        if *rx.borrow_and_update() {
            return;
        }
        if rx.changed().await.is_err() {
            return std::future::pending().await;
        }
    }
}

/// The validated connection request.
struct Target {
    protocol: Protocol,
    host: String,
    port: u16,
    cols: u16,
    rows: u16,
}

fn validate(req: &ConnectRequest) -> Result<Target, Stop> {
    let host = req
        .host
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_owned();
    target::check_host(&host).map_err(|e| fail("bad_request", e.to_string()))?;
    let port = req.port.unwrap_or(req.protocol.default_port());
    if port == 0 {
        return Err(fail("bad_request", "port 0".to_owned()));
    }
    if req.protocol == Protocol::Ssh && req.username.as_deref().is_none_or(|u| u.trim().is_empty())
    {
        return Err(fail("bad_request", "username".to_owned()));
    }
    Ok(Target {
        protocol: req.protocol,
        host,
        port,
        cols: req.cols.clamp(10, 1000),
        rows: req.rows.clamp(2, 500),
    })
}

/// Resolves `t` and applies the allowlist.
async fn permitted_addrs(term: &Terminal, t: &Target) -> Result<Vec<SocketAddr>, Stop> {
    let lookup = tokio::time::timeout(
        RESOLVE_TIMEOUT,
        tokio::net::lookup_host((t.host.as_str(), t.port)),
    )
    .await
    .map_err(|_| fail("resolve_failed", "timeout".to_owned()))?
    .map_err(|e| fail("resolve_failed", e.to_string()))?;
    let mut addrs: Vec<SocketAddr> = Vec::new();
    for a in lookup {
        if !addrs.contains(&a) {
            addrs.push(a);
        }
    }
    if addrs.is_empty() {
        return Err(fail("resolve_failed", None));
    }
    let ok = term.allowlist().permit(&t.host, t.port, &addrs);
    if ok.is_empty() {
        return Err(fail("target_not_allowed", None));
    }
    Ok(ok)
}

async fn dial(addrs: &[SocketAddr]) -> Result<TcpStream, Stop> {
    let mut last = String::new();
    for a in addrs {
        match tokio::time::timeout(DIAL_TIMEOUT, TcpStream::connect(a)).await {
            Ok(Ok(s)) => {
                let _ = s.set_nodelay(true);
                return Ok(s);
            }
            Ok(Err(e)) => last = format!("{a}: {e}"),
            Err(_) => last = format!("{a}: timeout"),
        }
    }
    Err(fail("connect_failed", last))
}

/// The remote side of an established session (one per session: the size
/// difference of the variants does not matter).
#[allow(clippy::large_enum_variant)]
enum Remote {
    Telnet {
        rd: OwnedReadHalf,
        wr: OwnedWriteHalf,
        proto: Telnet,
        buf: Box<[u8]>,
    },
    Ssh {
        handle: client::Handle<ssh::Client>,
        channel: russh::Channel<client::Msg>,
    },
}

enum RemoteEvent {
    Data(Vec<u8>),
    Closed,
    Error(String),
}

impl Remote {
    /// Next output (cancel safe).
    async fn recv(&mut self) -> RemoteEvent {
        match self {
            Remote::Telnet { rd, proto, buf, .. } => match rd.read(buf).await {
                Ok(0) => RemoteEvent::Closed,
                Ok(n) => RemoteEvent::Data(proto.receive(&buf[..n])),
                Err(e) => RemoteEvent::Error(e.to_string()),
            },
            Remote::Ssh { channel, .. } => loop {
                match channel.wait().await {
                    Some(ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. }) => {
                        return RemoteEvent::Data(data.to_vec())
                    }
                    Some(ChannelMsg::Close) | None => return RemoteEvent::Closed,
                    Some(_) => {}
                }
            },
        }
    }

    async fn send(&mut self, data: &[u8]) -> Result<(), String> {
        match self {
            Remote::Telnet { wr, proto, .. } => wr
                .write_all(&proto.encode(data))
                .await
                .map_err(|e| e.to_string()),
            Remote::Ssh { channel, .. } => channel.data(data).await.map_err(|e| e.to_string()),
        }
    }

    /// Writes pending Telnet negotiation replies.
    async fn flush(&mut self) -> Result<(), String> {
        if let Remote::Telnet { wr, proto, .. } = self {
            let p = proto.take_pending();
            if !p.is_empty() {
                wr.write_all(&p).await.map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    async fn resize(&mut self, cols: u16, rows: u16) -> Result<(), String> {
        match self {
            Remote::Telnet { proto, .. } => {
                proto.resize(cols, rows);
                self.flush().await
            }
            Remote::Ssh { channel, .. } => channel
                .window_change(u32::from(cols), u32::from(rows), 0, 0)
                .await
                .map_err(|e| e.to_string()),
        }
    }

    async fn close(self) {
        match self {
            Remote::Telnet { mut wr, .. } => {
                let _ = wr.shutdown().await;
            }
            Remote::Ssh { handle, channel } => {
                let _ = channel.eof().await;
                let _ = channel.close().await;
                let _ = handle
                    .disconnect(russh::Disconnect::ByApplication, "", "en")
                    .await;
            }
        }
    }
}

/// Byte counters and the log of a running session.
struct Live {
    bytes_in: u64,
    bytes_out: u64,
    recorder: Option<Recorder>,
    /// The last log file (reported after stopping, for the download link).
    last_log: Option<String>,
}

impl Live {
    fn empty() -> Self {
        Self {
            bytes_in: 0,
            bytes_out: 0,
            recorder: None,
            last_log: None,
        }
    }
}

/// Runs a session on an upgraded WebSocket.
pub async fn run(
    term: Terminal,
    mut shutdown: watch::Receiver<bool>,
    mut socket: WebSocket,
    who: Principal,
) {
    let started = Instant::now();
    let first = tokio::select! {
        r = tokio::time::timeout(CONNECT_WAIT, recv(&mut socket)) => r,
        () = shutting_down(&mut shutdown) => return,
    };
    let req = match first {
        Ok(In::Msg(ClientMsg::Connect(req))) => req,
        Ok(In::Closed) => return,
        Ok(_) | Err(_) => {
            let stop = fail("bad_request", "expected connect".to_owned());
            report(&mut socket, stop, started, &Live::empty()).await;
            return;
        }
    };
    let t = match validate(&req) {
        Ok(t) => t,
        Err(stop) => {
            report(&mut socket, stop, started, &Live::empty()).await;
            return;
        }
    };
    let id = uuid::Uuid::new_v4().simple().to_string();
    let target = format!("{}:{}", t.host, t.port);
    let login = req.username.clone().unwrap_or_default();
    let record = req.record.unwrap_or(true) && term.config().record_sessions;
    let strip = req.strip_ansi.unwrap_or(true);
    let mut live = Live::empty();

    let opened = open(&term, &mut shutdown, &mut socket, req, &t, &id, &who).await;
    let (remote, slot, address, fingerprint) = match opened {
        Ok(v) => v,
        Err(stop) => {
            if let Stop::Fail(f) = &stop {
                tracing::warn!(
                    target: AUDIT,
                    user = %who.user,
                    ip = %who.ip,
                    protocol = t.protocol.as_str(),
                    target = %target,
                    login = %login,
                    code = f.code,
                    detail = f.detail.as_deref().unwrap_or(""),
                    "terminal session refused"
                );
            }
            report(&mut socket, stop, started, &live).await;
            return;
        }
    };

    tracing::info!(
        target: AUDIT,
        user = %who.user,
        ip = %who.ip,
        session = %id,
        protocol = t.protocol.as_str(),
        target = %target,
        address = %address,
        login = %login,
        "terminal session start"
    );
    send(
        &mut socket,
        &ServerMsg::Connected {
            session_id: id.clone(),
            protocol: t.protocol,
            host: t.host.clone(),
            port: t.port,
            address: address.to_string(),
            fingerprint,
        },
    )
    .await;

    let reason = pump(
        &term,
        &mut shutdown,
        &mut socket,
        remote,
        &slot,
        &t,
        &mut live,
        (record, strip),
    )
    .await;
    if let Some(r) = live.recorder.take() {
        r.finish().await;
    }
    drop(slot);
    let duration = started.elapsed().as_secs();
    tracing::info!(
        target: AUDIT,
        user = %who.user,
        ip = %who.ip,
        session = %id,
        protocol = t.protocol.as_str(),
        target = %target,
        reason,
        bytes_in = live.bytes_in,
        bytes_out = live.bytes_out,
        duration_secs = duration,
        log = live.last_log.as_deref().unwrap_or(""),
        "terminal session end"
    );
    send(
        &mut socket,
        &ServerMsg::Closed {
            reason: reason.to_owned(),
            bytes_in: live.bytes_in,
            bytes_out: live.bytes_out,
            duration_secs: duration,
        },
    )
    .await;
    let _ = socket.send(Message::Close(None)).await;
}

/// Reports an early end (`error` + `closed`) and closes the socket.
async fn report(socket: &mut WebSocket, stop: Stop, started: Instant, live: &Live) {
    let reason = match stop {
        Stop::Client => "client",
        Stop::Shutdown => "shutdown",
        Stop::Fail(f) => {
            send(
                socket,
                &ServerMsg::Error {
                    code: f.code.to_owned(),
                    detail: f.detail,
                    fatal: true,
                    info: f.info,
                },
            )
            .await;
            "error"
        }
    };
    send(
        socket,
        &ServerMsg::Closed {
            reason: reason.to_owned(),
            bytes_in: live.bytes_in,
            bytes_out: live.bytes_out,
            duration_secs: started.elapsed().as_secs(),
        },
    )
    .await;
    let _ = socket.send(Message::Close(None)).await;
}

type Opened = (Remote, Slot, SocketAddr, Option<String>);

/// Steps 2 and 3: resolve, check, take a slot, dial, log in.
async fn open(
    term: &Terminal,
    shutdown: &mut watch::Receiver<bool>,
    socket: &mut WebSocket,
    req: ConnectRequest,
    t: &Target,
    id: &str,
    who: &Principal,
) -> Result<Opened, Stop> {
    state(socket, "resolving").await;
    let addrs = tokio::select! {
        r = permitted_addrs(term, t) => r?,
        () = shutting_down(shutdown) => return Err(Stop::Shutdown),
    };
    let slot = term
        .try_acquire(ActiveSession {
            id: id.to_owned(),
            user: who.user.clone(),
            protocol: t.protocol,
            host: t.host.clone(),
            port: t.port,
            started: u64::try_from(now_secs()).unwrap_or(0),
            log: None,
        })
        .ok_or_else(|| fail("too_many_sessions", term.config().max_sessions.to_string()))?;
    state(socket, "connecting").await;
    let tcp = tokio::select! {
        r = dial(&addrs) => r?,
        () = shutting_down(shutdown) => return Err(Stop::Shutdown),
    };
    let address = tcp.peer_addr().unwrap_or(addrs[0]);
    match t.protocol {
        Protocol::Telnet => {
            let (rd, wr) = tcp.into_split();
            let remote = Remote::Telnet {
                rd,
                wr,
                proto: Telnet::new(TERM, t.cols, t.rows),
                buf: vec![0; 16 * 1024].into_boxed_slice(),
            };
            Ok((remote, slot, address, None))
        }
        Protocol::Ssh => {
            let (remote, fp) = ssh_open(term, shutdown, socket, req, t, tcp, who).await?;
            Ok((remote, slot, address, fp))
        }
    }
}

/// Waits for the browser's answer to a prompt: the first message `pick` maps to
/// `Some`. `disconnect` / a closed socket / shutdown / timeout end the session.
async fn wait_answer<T>(
    socket: &mut WebSocket,
    shutdown: &mut watch::Receiver<bool>,
    mut pick: impl FnMut(ClientMsg) -> Option<T>,
) -> Result<T, Stop> {
    let deadline = tokio::time::Instant::now() + PROMPT_WAIT;
    loop {
        tokio::select! {
            m = recv(socket) => match m {
                In::Msg(ClientMsg::Disconnect) | In::Closed => return Err(Stop::Client),
                In::Msg(m) => {
                    if let Some(v) = pick(m) {
                        return Ok(v);
                    }
                }
                In::Bytes(_) | In::Bad(_) => {}
            },
            () = tokio::time::sleep_until(deadline) => return Err(fail("timeout", None)),
            () = shutting_down(shutdown) => return Err(Stop::Shutdown),
        }
    }
}

/// Bounds an SSH exchange that needs no user input.
async fn step<T>(
    fut: impl std::future::Future<Output = Result<T, russh::Error>>,
) -> Result<T, Stop> {
    match tokio::time::timeout(SSH_STEP_TIMEOUT, fut).await {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => Err(fail("ssh_error", e.to_string())),
        Err(_) => Err(fail("timeout", None)),
    }
}

fn host_key_changed(
    algorithm: &str,
    fingerprint: &str,
    known: &super::known_hosts::KnownHost,
) -> Stop {
    Stop::Fail(Fail {
        code: "host_key_changed",
        detail: None,
        info: Some(json!({
            "algorithm": algorithm,
            "fingerprint": fingerprint,
            "known_algorithm": known.algorithm,
            "known_fingerprint": known.fingerprint,
            "known_comment": known.comment,
        })),
    })
}

/// SSH: handshake with host key confirmation, authentication, PTY + shell.
async fn ssh_open(
    term: &Terminal,
    shutdown: &mut watch::Receiver<bool>,
    socket: &mut WebSocket,
    req: ConnectRequest,
    t: &Target,
    tcp: TcpStream,
    who: &Principal,
) -> Result<(Remote, Option<String>), Stop> {
    let (ev_tx, mut ev_rx) = mpsc::channel(8);
    let handler = ssh::Client::new(term.clone(), &t.host, t.port, &who.user, ev_tx);
    let connect = client::connect_stream(ssh::config(), tcp, handler);
    tokio::pin!(connect);
    let mut reply: Option<tokio::sync::oneshot::Sender<HostKeyAnswer>> = None;
    let mut changed: Option<Stop> = None;
    let mut rejected = false;
    let mut fingerprint = None;
    let mut banners = Vec::new();
    let mut deadline = tokio::time::Instant::now() + SSH_STEP_TIMEOUT;
    let res = loop {
        tokio::select! {
            r = &mut connect => break r,
            Some(ev) = ev_rx.recv() => match ev {
                SshEvent::Prompt { algorithm, fingerprint, reply: r } => {
                    send(socket, &ServerMsg::HostKey {
                        host: t.host.clone(),
                        port: t.port,
                        algorithm,
                        fingerprint,
                    }).await;
                    reply = Some(r);
                    deadline = tokio::time::Instant::now() + PROMPT_WAIT;
                }
                SshEvent::Changed { algorithm, fingerprint, known } => {
                    changed = Some(host_key_changed(&algorithm, &fingerprint, &known));
                }
                SshEvent::Verified(fp) => fingerprint = Some(fp),
                SshEvent::Banner(b) => banners.push(b),
            },
            m = recv(socket) => match m {
                In::Msg(ClientMsg::HostKey { accept, save }) => {
                    if let Some(r) = reply.take() {
                        rejected = !accept;
                        let _ = r.send(HostKeyAnswer { accept, save });
                        deadline = tokio::time::Instant::now() + SSH_STEP_TIMEOUT;
                    }
                }
                In::Msg(ClientMsg::Disconnect) | In::Closed => return Err(Stop::Client),
                In::Msg(_) | In::Bytes(_) | In::Bad(_) => {}
            },
            () = tokio::time::sleep_until(deadline) => return Err(fail("timeout", None)),
            () = shutting_down(shutdown) => return Err(Stop::Shutdown),
        }
    };
    // The handler reports before `connect` resolves.
    while let Ok(ev) = ev_rx.try_recv() {
        match ev {
            SshEvent::Verified(fp) => fingerprint = Some(fp),
            SshEvent::Banner(b) => banners.push(b),
            SshEvent::Changed {
                algorithm,
                fingerprint,
                known,
            } => changed = Some(host_key_changed(&algorithm, &fingerprint, &known)),
            SshEvent::Prompt { .. } => {}
        }
    }
    let mut handle = match res {
        Ok(h) => h,
        Err(e) => {
            return Err(changed.unwrap_or_else(|| {
                if rejected {
                    fail("host_key_rejected", None)
                } else {
                    fail("ssh_error", e.to_string())
                }
            }))
        }
    };

    state(socket, "authenticating").await;
    authenticate(&mut handle, shutdown, socket, req, t).await?;
    while let Ok(ev) = ev_rx.try_recv() {
        if let SshEvent::Banner(b) = ev {
            banners.push(b);
        }
    }
    for b in banners {
        send_data(
            socket,
            b.replace("\r\n", "\n").replace('\n', "\r\n").as_bytes(),
        )
        .await;
    }

    let channel = step(handle.channel_open_session()).await?;
    step(channel.request_pty(false, TERM, u32::from(t.cols), u32::from(t.rows), 0, 0, &[])).await?;
    step(channel.request_shell(false)).await?;
    Ok((Remote::Ssh { handle, channel }, fingerprint))
}

fn has(methods: &MethodSet, m: MethodKind) -> bool {
    methods.contains(&m)
}

/// Authentication: none (to learn the methods) → public key → password →
/// keyboard-interactive → password prompts. Secrets are dropped on return.
async fn authenticate(
    handle: &mut client::Handle<ssh::Client>,
    shutdown: &mut watch::Receiver<bool>,
    socket: &mut WebSocket,
    req: ConnectRequest,
    t: &Target,
) -> Result<(), Stop> {
    let user = req.username.clone().unwrap_or_default().trim().to_owned();
    let password = req.password.filter(|p| !p.is_empty());
    let mut methods = match step(handle.authenticate_none(user.clone())).await? {
        client::AuthResult::Success => return Ok(()),
        client::AuthResult::Failure {
            remaining_methods, ..
        } if remaining_methods.is_empty() => MethodSet::from(
            &[
                MethodKind::PublicKey,
                MethodKind::Password,
                MethodKind::KeyboardInteractive,
            ][..],
        ),
        client::AuthResult::Failure {
            remaining_methods, ..
        } => remaining_methods,
    };
    if let Some(pem) = req.private_key.filter(|k| !k.trim().is_empty()) {
        let passphrase = req.passphrase.filter(|p| !p.is_empty());
        let key = russh::keys::decode_secret_key(&pem, passphrase.as_deref())
            .map_err(|e| fail("key_invalid", e.to_string()))?;
        if has(&methods, MethodKind::PublicKey) {
            let hash = handle
                .best_supported_rsa_hash()
                .await
                .ok()
                .flatten()
                .flatten();
            let key = PrivateKeyWithHashAlg::new(Arc::new(key), hash);
            match step(handle.authenticate_publickey(user.clone(), key)).await? {
                client::AuthResult::Success => return Ok(()),
                client::AuthResult::Failure {
                    remaining_methods, ..
                } => methods = remaining_methods,
            }
        }
    }
    let mut password_failed = false;
    if let Some(pw) = &password {
        if has(&methods, MethodKind::Password) {
            match step(handle.authenticate_password(user.clone(), pw.clone())).await? {
                client::AuthResult::Success => return Ok(()),
                client::AuthResult::Failure {
                    remaining_methods, ..
                } => {
                    methods = remaining_methods;
                    password_failed = true;
                }
            }
        }
    }
    if has(&methods, MethodKind::KeyboardInteractive) {
        let auto = if password_failed {
            None
        } else {
            password.clone()
        };
        if keyboard_interactive(handle, shutdown, socket, &user, auto).await? {
            return Ok(());
        }
    } else if has(&methods, MethodKind::Password) {
        for _ in 0..PASSWORD_PROMPTS {
            let prompt = ServerMsg::AuthPrompt {
                name: String::new(),
                instructions: String::new(),
                prompts: vec![PromptItem {
                    prompt: format!("{user}@{}'s password: ", t.host),
                    echo: false,
                }],
            };
            send(socket, &prompt).await;
            let pw = wait_answer(socket, shutdown, |m| match m {
                ClientMsg::AuthResponse { mut responses } => {
                    Some(responses.drain(..).next().unwrap_or_default())
                }
                _ => None,
            })
            .await?;
            if step(handle.authenticate_password(user.clone(), pw))
                .await?
                .success()
            {
                return Ok(());
            }
        }
    }
    Err(fail("auth_failed", None))
}

/// The keyboard-interactive exchange; `auto` answers the first lone hidden
/// prompt (the usual "Password:").
async fn keyboard_interactive(
    handle: &mut client::Handle<ssh::Client>,
    shutdown: &mut watch::Receiver<bool>,
    socket: &mut WebSocket,
    user: &str,
    mut auto: Option<String>,
) -> Result<bool, Stop> {
    let mut resp =
        step(handle.authenticate_keyboard_interactive_start(user.to_owned(), None)).await?;
    for _ in 0..KI_ROUNDS {
        let (name, instructions, prompts) = match resp {
            KeyboardInteractiveAuthResponse::Success => return Ok(true),
            KeyboardInteractiveAuthResponse::Failure { .. } => return Ok(false),
            KeyboardInteractiveAuthResponse::InfoRequest {
                name,
                instructions,
                prompts,
            } => (name, instructions, prompts),
        };
        let answers = if prompts.is_empty() {
            Vec::new()
        } else if prompts.len() == 1 && !prompts[0].echo && auto.is_some() {
            vec![auto.take().unwrap_or_default()]
        } else {
            let n = prompts.len();
            let msg = ServerMsg::AuthPrompt {
                name,
                instructions,
                prompts: prompts
                    .into_iter()
                    .map(|p| PromptItem {
                        prompt: p.prompt,
                        echo: p.echo,
                    })
                    .collect(),
            };
            send(socket, &msg).await;
            let mut a = wait_answer(socket, shutdown, |m| match m {
                ClientMsg::AuthResponse { responses } => Some(responses),
                _ => None,
            })
            .await?;
            a.resize(n, String::new());
            a
        };
        resp = step(handle.authenticate_keyboard_interactive_respond(answers)).await?;
    }
    Ok(false)
}

/// Starts a log file; reports `record` (or `log_failed`).
async fn start_recording(
    term: &Terminal,
    socket: &mut WebSocket,
    slot: &Slot,
    t: &Target,
    live: &mut Live,
    strip: bool,
) {
    let name = recorder::file_name(
        u64::try_from(now_secs()).unwrap_or(0),
        t.protocol.as_str(),
        &t.host,
        t.port,
        &slot.id()[..8],
    );
    match Recorder::create(term.log_dir(), name.clone(), strip).await {
        Ok(r) => {
            live.recorder = Some(r);
            live.last_log = Some(name.clone());
            slot.set_log(Some(name.clone()));
            send(
                socket,
                &ServerMsg::Record {
                    enabled: true,
                    file: Some(name),
                },
            )
            .await;
        }
        Err(e) => {
            tracing::error!("{}: {e}", term.log_dir().display());
            send(
                socket,
                &ServerMsg::Error {
                    code: "log_failed".into(),
                    detail: Some(e.to_string()),
                    fatal: false,
                    info: None,
                },
            )
            .await;
            send(
                socket,
                &ServerMsg::Record {
                    enabled: false,
                    file: None,
                },
            )
            .await;
        }
    }
}

async fn stop_recording(socket: &mut WebSocket, slot: &Slot, live: &mut Live) {
    if let Some(r) = live.recorder.take() {
        r.finish().await;
    }
    slot.set_log(None);
    send(
        socket,
        &ServerMsg::Record {
            enabled: false,
            file: live.last_log.clone(),
        },
    )
    .await;
}

/// Step 4: shuttles bytes until one side ends; returns the `closed` reason.
#[allow(clippy::too_many_arguments)] // the session's pieces, borrowed separately
async fn pump(
    term: &Terminal,
    shutdown: &mut watch::Receiver<bool>,
    socket: &mut WebSocket,
    mut remote: Remote,
    slot: &Slot,
    t: &Target,
    live: &mut Live,
    (record, strip): (bool, bool),
) -> &'static str {
    if record {
        start_recording(term, socket, slot, t, live, strip).await;
    } else {
        send(
            socket,
            &ServerMsg::Record {
                enabled: false,
                file: None,
            },
        )
        .await;
    }
    let idle = term.config().idle_timeout_secs;
    let mut last = tokio::time::Instant::now();
    let reason = loop {
        let idle_at = (idle > 0).then(|| last + Duration::from_secs(idle));
        tokio::select! {
            ev = remote.recv() => match ev {
                RemoteEvent::Data(d) => {
                    if !d.is_empty() {
                        last = tokio::time::Instant::now();
                        live.bytes_in += d.len() as u64;
                        if let Some(r) = &mut live.recorder {
                            match r.write(&d).await {
                                Ok(true) => {}
                                Ok(false) => stop_recording(socket, slot, live).await,
                                Err(e) => {
                                    tracing::error!("terminal log: {e}");
                                    stop_recording(socket, slot, live).await;
                                }
                            }
                        }
                        if !send_data(socket, &d).await {
                            break "client";
                        }
                    }
                }
                RemoteEvent::Closed => break "remote",
                RemoteEvent::Error(e) => {
                    send(socket, &ServerMsg::Error {
                        code: "connection_lost".into(),
                        detail: Some(e),
                        fatal: true,
                        info: None,
                    }).await;
                    break "error";
                }
            },
            m = recv(socket) => {
                let input = match m {
                    In::Bytes(b) => Some(b),
                    In::Msg(ClientMsg::Input { data }) => Some(data.into_bytes()),
                    In::Msg(ClientMsg::Resize { cols, rows }) => {
                        if remote.resize(cols.clamp(10, 1000), rows.clamp(2, 500)).await.is_err() {
                            break "remote";
                        }
                        None
                    }
                    In::Msg(ClientMsg::Record { enabled, strip_ansi }) => {
                        if enabled && !term.config().record_sessions {
                            send(socket, &ServerMsg::Error {
                                code: "recording_disabled".into(),
                                detail: None,
                                fatal: false,
                                info: None,
                            }).await;
                        } else if enabled && live.recorder.is_none() {
                            start_recording(term, socket, slot, t, live, strip_ansi.unwrap_or(strip)).await;
                        } else if !enabled {
                            stop_recording(socket, slot, live).await;
                        }
                        None
                    }
                    In::Msg(ClientMsg::Disconnect) | In::Closed => break "client",
                    In::Msg(_) => None,
                    In::Bad(e) => {
                        send(socket, &ServerMsg::Error {
                            code: "bad_request".into(),
                            detail: Some(e),
                            fatal: false,
                            info: None,
                        }).await;
                        None
                    }
                };
                if let Some(data) = input.filter(|d| !d.is_empty()) {
                    last = tokio::time::Instant::now();
                    live.bytes_out += data.len() as u64;
                    if remote.send(&data).await.is_err() {
                        break "remote";
                    }
                }
            },
            () = sleep_until_opt(idle_at) => break "idle",
            () = shutting_down(shutdown) => break "shutdown",
        }
        if remote.flush().await.is_err() {
            break "remote";
        }
    };
    remote.close().await;
    reason
}
