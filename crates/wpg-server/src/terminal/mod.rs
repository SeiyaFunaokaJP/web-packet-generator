//! Web terminal (ADR-0011): SSH / Telnet sessions the server opens on behalf of a
//! logged-in user and bridges to the browser over `/api/terminal/ws`.
//!
//! * [`target`]: `terminal.allowed_targets` (host / host:port / CIDR);
//! * [`telnet`]: the Telnet option negotiation (ECHO, SGA, NAWS, TTYPE);
//! * [`ssh`]: the `russh` client (password, public key, keyboard-interactive);
//! * [`known_hosts`]: trust-on-first-use SSH host keys;
//! * [`recorder`]: session logs under `<data_dir>/terminal-logs`;
//! * [`session`]: the WebSocket protocol ([`ClientMsg`] / [`ServerMsg`]).
//!
//! Safety rails (spec §2.6): off by default (`terminal.enabled`), target
//! allowlist, a global session limit, an idle timeout, audit events (target
//! `wpg::audit`) for the start and end of every session with the byte counts,
//! same-origin WebSocket check. Passwords and private keys only live in memory
//! for the duration of the connection attempt.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::{Config, TerminalConfig};

pub mod known_hosts;
pub mod recorder;
pub mod session;
pub mod ssh;
pub mod target;
pub mod telnet;

/// Terminal type announced to the remote side.
pub const TERM: &str = "xterm-256color";

/// `YYYY-MM-DD` and `HH:MM:SS` (UTC) of Unix `secs`.
#[must_use]
pub fn utc_parts(secs: u64) -> (String, String) {
    let (y, m, d) = crate::tls::ymd(secs);
    let s = secs % 86_400;
    (
        format!("{y:04}-{m:02}-{d:02}"),
        format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60),
    )
}

/// RFC 3339 UTC time of Unix `secs`.
#[must_use]
pub fn rfc3339(secs: u64) -> String {
    let (d, t) = utc_parts(secs);
    format!("{d}T{t}Z")
}

/// Remote protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    /// SSH 2 (default port 22).
    Ssh,
    /// Telnet (default port 23).
    Telnet,
}

impl Protocol {
    /// Name in logs and file names.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Protocol::Ssh => "ssh",
            Protocol::Telnet => "telnet",
        }
    }

    /// Well-known port.
    #[must_use]
    pub fn default_port(self) -> u16 {
        match self {
            Protocol::Ssh => 22,
            Protocol::Telnet => 23,
        }
    }
}

/// A running session (`GET /api/terminal/status`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct ActiveSession {
    /// Session id.
    pub id: String,
    /// wpg user who opened it.
    pub user: String,
    /// Protocol.
    pub protocol: Protocol,
    /// Target host as entered.
    pub host: String,
    /// Target port.
    pub port: u16,
    /// Start (Unix seconds).
    pub started: u64,
    /// Log file being written, if recording.
    pub log: Option<String>,
}

/// `GET /api/terminal/status`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TerminalStatus {
    /// `terminal.enabled`.
    pub enabled: bool,
    /// `terminal.record_sessions`.
    pub record_sessions: bool,
    /// `terminal.idle_timeout_secs`.
    pub idle_timeout_secs: u64,
    /// `terminal.max_sessions`.
    pub max_sessions: usize,
    /// `terminal.allowed_targets` (empty = any target).
    pub allowed_targets: Vec<String>,
    /// Running sessions (all users).
    pub sessions: Vec<ActiveSession>,
}

struct Inner {
    config: TerminalConfig,
    log_dir: PathBuf,
    allow: target::Allowlist,
    known: known_hosts::KnownHosts,
    sessions: Mutex<HashMap<String, ActiveSession>>,
}

/// The web terminal service (cheap to clone).
#[derive(Clone)]
pub struct Terminal {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Terminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Terminal")
            .field("enabled", &self.inner.config.enabled)
            .finish_non_exhaustive()
    }
}

/// A session slot; dropping it frees the slot.
#[derive(Debug)]
pub struct Slot {
    terminal: Terminal,
    id: String,
}

impl Slot {
    /// The session id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Records the log file the session writes (`None`: not recording).
    pub fn set_log(&self, log: Option<String>) {
        if let Some(s) = self.terminal.lock().get_mut(&self.id) {
            s.log = log;
        }
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.terminal.lock().remove(&self.id);
    }
}

impl Terminal {
    /// The service for `config`.
    #[must_use]
    pub fn new(config: &Config) -> Self {
        Self {
            inner: Arc::new(Inner {
                config: config.terminal.clone(),
                log_dir: config.terminal_log_dir(),
                allow: target::Allowlist::new(&config.terminal.allowed_targets),
                known: known_hosts::KnownHosts::new(config.known_hosts_path()),
                sessions: Mutex::new(HashMap::new()),
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, ActiveSession>> {
        self.inner
            .sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// `[terminal]`.
    #[must_use]
    pub fn config(&self) -> &TerminalConfig {
        &self.inner.config
    }

    /// `terminal.enabled`.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.inner.config.enabled
    }

    /// Session log directory.
    #[must_use]
    pub fn log_dir(&self) -> &std::path::Path {
        &self.inner.log_dir
    }

    /// The target allowlist.
    #[must_use]
    pub fn allowlist(&self) -> &target::Allowlist {
        &self.inner.allow
    }

    /// Trusted SSH host keys.
    #[must_use]
    pub fn known_hosts(&self) -> &known_hosts::KnownHosts {
        &self.inner.known
    }

    /// Takes a session slot unless `terminal.max_sessions` are running.
    #[must_use]
    pub fn try_acquire(&self, info: ActiveSession) -> Option<Slot> {
        let mut m = self.lock();
        if m.len() >= self.inner.config.max_sessions {
            return None;
        }
        let id = info.id.clone();
        m.insert(id.clone(), info);
        Some(Slot {
            terminal: self.clone(),
            id,
        })
    }

    /// Log files currently being written.
    #[must_use]
    pub fn active_logs(&self) -> Vec<String> {
        self.lock().values().filter_map(|s| s.log.clone()).collect()
    }

    /// `GET /api/terminal/status`.
    #[must_use]
    pub fn status(&self) -> TerminalStatus {
        let c = &self.inner.config;
        let mut sessions: Vec<ActiveSession> = self.lock().values().cloned().collect();
        sessions.sort_by_key(|s| s.started);
        TerminalStatus {
            enabled: c.enabled,
            record_sessions: c.record_sessions,
            idle_timeout_secs: c.idle_timeout_secs,
            max_sessions: c.max_sessions,
            allowed_targets: c.allowed_targets.clone(),
            sessions,
        }
    }
}

/// Client → server message on `/api/terminal/ws`. Keystrokes may also arrive
/// as binary frames (raw bytes).
#[derive(Deserialize, ToSchema)]
#[schema(as = TerminalClientMessage)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    /// First message: opens the session.
    Connect(ConnectRequest),
    /// Keystrokes / pasted text (UTF-8).
    Input {
        /// The text.
        data: String,
    },
    /// The terminal was resized.
    Resize {
        /// Columns.
        cols: u16,
        /// Rows.
        rows: u16,
    },
    /// Answer to a `host_key` prompt.
    HostKey {
        /// Trust the key.
        accept: bool,
        /// Store it in the known hosts (otherwise trust it for this session).
        #[serde(default)]
        save: bool,
    },
    /// Answers to an `auth_prompt` (one per prompt).
    AuthResponse {
        /// The answers.
        responses: Vec<String>,
    },
    /// Starts / stops the session log.
    Record {
        /// Record from now on.
        enabled: bool,
        /// Remove escape sequences (default true).
        #[serde(default)]
        strip_ansi: Option<bool>,
    },
    /// Ends the session.
    Disconnect,
}

impl std::fmt::Debug for ClientMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print keystrokes or answers (passwords).
        let name = match self {
            ClientMsg::Connect(c) => return c.fmt(f),
            ClientMsg::Input { .. } => "Input",
            ClientMsg::Resize { .. } => "Resize",
            ClientMsg::HostKey { .. } => "HostKey",
            ClientMsg::AuthResponse { .. } => "AuthResponse",
            ClientMsg::Record { .. } => "Record",
            ClientMsg::Disconnect => "Disconnect",
        };
        f.write_str(name)
    }
}

/// `connect`: where to and how to log in.
#[derive(Clone, Deserialize, ToSchema)]
#[schema(as = TerminalConnectRequest)]
pub struct ConnectRequest {
    /// `ssh` or `telnet`.
    pub protocol: Protocol,
    /// Host name or address.
    pub host: String,
    /// Port (default 22 / 23).
    #[serde(default)]
    pub port: Option<u16>,
    /// SSH user name.
    #[serde(default)]
    pub username: Option<String>,
    /// SSH password (also answers a single keyboard-interactive password prompt).
    #[serde(default)]
    pub password: Option<String>,
    /// SSH private key (OpenSSH / PEM / `PuTTY` `.ppk` text).
    #[serde(default)]
    pub private_key: Option<String>,
    /// Passphrase of `private_key`.
    #[serde(default)]
    pub passphrase: Option<String>,
    /// Terminal width.
    #[serde(default = "default_cols")]
    pub cols: u16,
    /// Terminal height.
    #[serde(default = "default_rows")]
    pub rows: u16,
    /// Record a session log (default: `terminal.record_sessions`).
    #[serde(default)]
    pub record: Option<bool>,
    /// Remove escape sequences from the log (default true).
    #[serde(default)]
    pub strip_ansi: Option<bool>,
}

fn default_cols() -> u16 {
    80
}

fn default_rows() -> u16 {
    24
}

impl std::fmt::Debug for ConnectRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectRequest")
            .field("protocol", &self.protocol)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "<redacted>"))
            .field(
                "private_key",
                &self.private_key.as_ref().map(|_| "<redacted>"),
            )
            .field("cols", &self.cols)
            .field("rows", &self.rows)
            .finish_non_exhaustive()
    }
}

/// One keyboard-interactive prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct PromptItem {
    /// Prompt text from the server.
    pub prompt: String,
    /// Whether the answer may be shown while typed.
    pub echo: bool,
}

/// Server → client message on `/api/terminal/ws`. Remote output travels as
/// binary frames.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[schema(as = TerminalServerMessage)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    /// Progress: `resolving`, `connecting`, `authenticating`.
    State {
        /// The phase.
        state: String,
    },
    /// An unknown SSH host key: confirm the fingerprint (`host_key` message).
    HostKey {
        /// Host.
        host: String,
        /// Port.
        port: u16,
        /// Key algorithm.
        algorithm: String,
        /// `SHA256:...`.
        fingerprint: String,
    },
    /// The server asks questions (keyboard-interactive, or the password when
    /// none was given): answer with `auth_response`.
    AuthPrompt {
        /// Request name from the server.
        name: String,
        /// Instructions from the server.
        instructions: String,
        /// The prompts.
        prompts: Vec<PromptItem>,
    },
    /// The session is up.
    Connected {
        /// Session id.
        session_id: String,
        /// Protocol.
        protocol: Protocol,
        /// Host.
        host: String,
        /// Port.
        port: u16,
        /// Address actually connected to.
        address: String,
        /// SSH host key fingerprint.
        #[serde(skip_serializing_if = "Option::is_none")]
        fingerprint: Option<String>,
    },
    /// Log state after `connected` / `record`.
    Record {
        /// Recording.
        enabled: bool,
        /// The log file (`GET /api/terminal/logs/{name}`).
        #[serde(skip_serializing_if = "Option::is_none")]
        file: Option<String>,
    },
    /// A failure; `fatal` ones are followed by `closed`.
    Error {
        /// `target_not_allowed`, `too_many_sessions`, `resolve_failed`,
        /// `connect_failed`, `host_key_changed`, `host_key_rejected`,
        /// `auth_failed`, `key_invalid`, `ssh_error`, `timeout`, `bad_request`,
        /// `recording_disabled`, `log_failed`.
        code: String,
        /// Untranslated detail (an OS / protocol error).
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
        /// The session ends.
        fatal: bool,
        /// `host_key_changed`: `{algorithm, fingerprint, known_algorithm,
        /// known_fingerprint}`.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schema(value_type = Option<Object>)]
        info: Option<serde_json::Value>,
    },
    /// The session ended.
    Closed {
        /// `remote` (the host closed it), `client`, `idle`, `shutdown`, `error`.
        reason: String,
        /// Bytes received from the host.
        bytes_in: u64,
        /// Bytes sent to the host.
        bytes_out: u64,
        /// Session length in seconds.
        duration_secs: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(1_793_096_100), "2026-10-27T10:15:00Z");
    }

    #[test]
    fn messages() {
        let m: ClientMsg = serde_json::from_str(
            r#"{"type":"connect","protocol":"ssh","host":"r1","username":"u","password":"secret"}"#,
        )
        .unwrap();
        let ClientMsg::Connect(c) = &m else {
            panic!("connect")
        };
        assert_eq!((c.cols, c.rows, c.port), (80, 24, None));
        assert!(!format!("{m:?}").contains("secret"));
        let m: ClientMsg =
            serde_json::from_str(r#"{"type":"auth_response","responses":["pw"]}"#).unwrap();
        assert_eq!(format!("{m:?}"), "AuthResponse");
        assert!(serde_json::from_str::<ClientMsg>(r#"{"type":"nope"}"#).is_err());
        let s = serde_json::to_value(ServerMsg::Record {
            enabled: false,
            file: None,
        })
        .unwrap();
        assert_eq!(s, serde_json::json!({"type": "record", "enabled": false}));
    }

    #[test]
    fn slots() {
        let mut c = Config::default();
        c.terminal.max_sessions = 1;
        let t = Terminal::new(&c);
        let info = |id: &str| ActiveSession {
            id: id.into(),
            user: "admin".into(),
            protocol: Protocol::Telnet,
            host: "h".into(),
            port: 23,
            started: 1,
            log: None,
        };
        let a = t.try_acquire(info("a")).unwrap();
        assert!(t.try_acquire(info("b")).is_none());
        a.set_log(Some("x.log".into()));
        assert_eq!(t.active_logs(), ["x.log"]);
        assert_eq!(t.status().sessions.len(), 1);
        drop(a);
        assert!(t.active_logs().is_empty());
        assert!(t.try_acquire(info("b")).is_some());
    }
}
