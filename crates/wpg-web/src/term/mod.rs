//! Web terminal (ADR-0011): SSH / Telnet sessions in xterm.js tabs, bridged by
//! the server's `/api/terminal/ws`.
//!
//! * this module: the pure, unit-tested part: connection profiles (saved without
//!   secrets), terminal settings, the WebSocket messages and how they change a
//!   tab's state, i18n keys of server codes, paste helpers;
//! * [`TermStore`]: the reactive state (context created by the app root, so
//!   sessions survive navigating to other pages);
//! * [`xterm`]: `wasm-bindgen` bindings of the vendored xterm.js (loaded on first
//!   use from `/xterm/`);
//! * [`session`]: the browser glue of one tab (terminal + WebSocket).

pub(crate) mod session;
#[cfg(target_arch = "wasm32")]
pub(crate) mod xterm;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// `localStorage` key of the saved profiles.
const KEY_PROFILES: &str = "wpg.term.profiles";
/// `localStorage` key of the terminal settings.
const KEY_SETTINGS: &str = "wpg.term.settings";

/// Remote protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Proto {
    #[default]
    Ssh,
    Telnet,
}

impl Proto {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Proto::Ssh => "ssh",
            Proto::Telnet => "telnet",
        }
    }

    pub(crate) fn parse(s: &str) -> Self {
        if s == "telnet" {
            Proto::Telnet
        } else {
            Proto::Ssh
        }
    }

    pub(crate) const fn default_port(self) -> u16 {
        match self {
            Proto::Ssh => 22,
            Proto::Telnet => 23,
        }
    }

    pub(crate) const fn label_key(self) -> &'static str {
        match self {
            Proto::Ssh => "ui-term-proto-ssh",
            Proto::Telnet => "ui-term-proto-telnet",
        }
    }
}

/// A saved connection (never holds a password, key or passphrase).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Profile {
    pub(crate) name: String,
    pub(crate) protocol: Proto,
    pub(crate) host: String,
    pub(crate) port: Option<u16>,
    pub(crate) username: String,
    /// Log in with a private key rather than a password.
    pub(crate) use_key: bool,
    /// Record a session log (when the server allows it).
    pub(crate) record: bool,
    /// Remove escape sequences from the log.
    pub(crate) strip_ansi: bool,
    /// SSH jump hosts (`ProxyJump`), nearest first.
    pub(crate) jumps: Vec<JumpProfile>,
}

/// Most jump hosts a session may use (the server's limit).
pub(crate) const MAX_JUMPS: usize = 4;

/// One saved SSH jump host (no secrets).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct JumpProfile {
    pub(crate) host: String,
    pub(crate) port: Option<u16>,
    pub(crate) username: String,
    /// Log in with a private key rather than a password.
    pub(crate) use_key: bool,
}

/// The secrets of one jump host (memory only, aligned with
/// [`Profile::jumps`]).
#[derive(Clone, Default, PartialEq, Eq)]
pub(crate) struct JumpSecret {
    pub(crate) password: String,
    pub(crate) private_key: String,
    pub(crate) passphrase: String,
}

impl Profile {
    /// A fresh form.
    pub(crate) fn blank() -> Self {
        Self {
            record: true,
            strip_ansi: true,
            ..Self::default()
        }
    }

    /// Tab title: the profile name, else `user@host`, else `host`.
    pub(crate) fn title(&self) -> String {
        let host = match self.port {
            Some(p) if p != self.protocol.default_port() => format!("{}:{p}", self.host.trim()),
            _ => self.host.trim().to_owned(),
        };
        if !self.name.trim().is_empty() {
            self.name.trim().to_owned()
        } else if self.protocol == Proto::Ssh && !self.username.trim().is_empty() {
            format!("{}@{host}", self.username.trim())
        } else {
            host
        }
    }
}

/// Parses the stored profiles (garbage → empty).
pub(crate) fn parse_profiles(s: &str) -> Vec<Profile> {
    serde_json::from_str(s).unwrap_or_default()
}

/// Adds `p` or replaces the profile with the same name; sorted by name.
pub(crate) fn upsert_profile(list: &mut Vec<Profile>, p: Profile) {
    list.retain(|x| x.name != p.name);
    list.push(p);
    list.sort_by_key(|p| p.name.to_lowercase());
}

pub(crate) fn load_profiles() -> Vec<Profile> {
    crate::prefs::load(KEY_PROFILES)
        .map(|s| parse_profiles(&s))
        .unwrap_or_default()
}

pub(crate) fn save_profiles(list: &[Profile]) {
    if let Ok(s) = serde_json::to_string(list) {
        crate::prefs::store(KEY_PROFILES, &s);
    }
}

/// Terminal settings (all tabs).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct TermSettings {
    pub(crate) font_size: u8,
    pub(crate) scrollback: u32,
    /// Copy the selection to the clipboard (Tera Term style).
    pub(crate) copy_on_select: bool,
    /// Right / middle click pastes.
    pub(crate) right_click_paste: bool,
    /// Ask before pasting / sending several lines.
    pub(crate) confirm_paste: bool,
    /// Pause after each line when pasting / sending several lines (slow
    /// consoles drop characters otherwise).
    pub(crate) line_delay_ms: u32,
}

impl Default for TermSettings {
    fn default() -> Self {
        Self {
            font_size: 14,
            scrollback: 10_000,
            copy_on_select: true,
            right_click_paste: true,
            confirm_paste: true,
            line_delay_ms: 0,
        }
    }
}

impl TermSettings {
    pub(crate) const FONT_MIN: u8 = 8;
    pub(crate) const FONT_MAX: u8 = 32;
    pub(crate) const SCROLLBACK_MAX: u32 = 200_000;
    pub(crate) const DELAY_MAX: u32 = 2000;

    /// Brings every value into its range.
    pub(crate) fn clamped(mut self) -> Self {
        self.font_size = self.font_size.clamp(Self::FONT_MIN, Self::FONT_MAX);
        self.scrollback = self.scrollback.clamp(100, Self::SCROLLBACK_MAX);
        self.line_delay_ms = self.line_delay_ms.min(Self::DELAY_MAX);
        self
    }

    pub(crate) fn parse(s: &str) -> Self {
        serde_json::from_str::<Self>(s)
            .unwrap_or_default()
            .clamped()
    }
}

pub(crate) fn load_settings() -> TermSettings {
    crate::prefs::load(KEY_SETTINGS)
        .map(|s| TermSettings::parse(&s))
        .unwrap_or_default()
}

pub(crate) fn save_settings(s: &TermSettings) {
    if let Ok(t) = serde_json::to_string(s) {
        crate::prefs::store(KEY_SETTINGS, &t);
    }
}

/// What a tab connects with: the profile plus the secrets (kept in memory for
/// reconnecting, never stored).
#[derive(Clone, Default, PartialEq, Eq)]
pub(crate) struct ConnectParams {
    pub(crate) profile: Profile,
    pub(crate) password: String,
    pub(crate) private_key: String,
    pub(crate) passphrase: String,
    /// Secrets of `profile.jumps` (same order; missing entries are empty).
    pub(crate) jump_secrets: Vec<JumpSecret>,
}

impl std::fmt::Debug for ConnectParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectParams")
            .field("profile", &self.profile)
            .finish_non_exhaustive()
    }
}

impl ConnectParams {
    /// The `connect` message.
    pub(crate) fn message(&self, cols: u32, rows: u32) -> Value {
        let p = &self.profile;
        let mut m = json!({
            "type": "connect",
            "protocol": p.protocol.as_str(),
            "host": p.host.trim(),
            "cols": cols,
            "rows": rows,
            "record": p.record,
            "strip_ansi": p.strip_ansi,
        });
        if let Some(port) = p.port {
            m["port"] = json!(port);
        }
        if p.protocol == Proto::Ssh {
            m["username"] = json!(p.username.trim());
            if p.use_key && !self.private_key.trim().is_empty() {
                m["private_key"] = json!(self.private_key);
                if !self.passphrase.is_empty() {
                    m["passphrase"] = json!(self.passphrase);
                }
            } else if !self.password.is_empty() {
                m["password"] = json!(self.password);
            }
        }
        if !p.jumps.is_empty() {
            let empty = JumpSecret::default();
            let jumps: Vec<Value> = p
                .jumps
                .iter()
                .enumerate()
                .map(|(i, j)| {
                    let s = self.jump_secrets.get(i).unwrap_or(&empty);
                    let mut v = json!({ "host": j.host.trim(), "username": j.username.trim() });
                    if let Some(port) = j.port {
                        v["port"] = json!(port);
                    }
                    if j.use_key && !s.private_key.trim().is_empty() {
                        v["private_key"] = json!(s.private_key);
                        if !s.passphrase.is_empty() {
                            v["passphrase"] = json!(s.passphrase);
                        }
                    } else if !s.password.is_empty() {
                        v["password"] = json!(s.password);
                    }
                    v
                })
                .collect();
            m["jump"] = Value::Array(jumps);
        }
        m
    }

    /// Form errors (i18n keys).
    pub(crate) fn problems(&self) -> Vec<&'static str> {
        let p = &self.profile;
        let mut out = Vec::new();
        let host = p.host.trim();
        if host.is_empty() || host.contains(char::is_whitespace) {
            out.push("ui-term-err-host");
        }
        if p.port == Some(0) {
            out.push("ui-term-err-port");
        }
        if p.protocol == Proto::Ssh {
            if p.username.trim().is_empty() {
                out.push("ui-term-err-username");
            }
            if p.use_key && self.private_key.trim().is_empty() {
                out.push("ui-term-err-key");
            }
        }
        for (i, j) in p.jumps.iter().enumerate() {
            let host = j.host.trim();
            if host.is_empty() || host.contains(char::is_whitespace) {
                out.push("ui-term-err-jump-host");
            }
            if j.port == Some(0) {
                out.push("ui-term-err-port");
            }
            if j.username.trim().is_empty() {
                out.push("ui-term-err-jump-user");
            }
            if j.use_key
                && self
                    .jump_secrets
                    .get(i)
                    .is_none_or(|s| s.private_key.trim().is_empty())
            {
                out.push("ui-term-err-jump-key");
            }
        }
        if p.jumps.len() > MAX_JUMPS {
            out.push("ui-term-err-jump-count");
        }
        out.dedup();
        out
    }
}

/// One keyboard-interactive / password prompt.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct Prompt {
    pub(crate) prompt: String,
    #[serde(default)]
    pub(crate) echo: bool,
}

/// Server → browser message (`/api/terminal/ws`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ServerMsg {
    State {
        state: String,
        /// The host of this phase (sessions with jump hosts).
        #[serde(default)]
        host: Option<String>,
    },
    HostKey {
        host: String,
        port: u16,
        algorithm: String,
        fingerprint: String,
    },
    AuthPrompt {
        /// The host asking (the target or a jump host).
        #[serde(default)]
        host: String,
        #[serde(default)]
        name: String,
        #[serde(default)]
        instructions: String,
        prompts: Vec<Prompt>,
    },
    Connected {
        #[serde(default)]
        address: String,
        #[serde(default)]
        fingerprint: Option<String>,
        /// Jump hosts (`user@host:port`).
        #[serde(default)]
        via: Vec<String>,
    },
    Record {
        enabled: bool,
        #[serde(default)]
        file: Option<String>,
    },
    Error {
        code: String,
        #[serde(default)]
        detail: Option<String>,
        #[serde(default)]
        fatal: bool,
        #[serde(default)]
        info: Option<Value>,
    },
    Closed {
        reason: String,
        #[serde(default)]
        bytes_in: u64,
        #[serde(default)]
        bytes_out: u64,
        #[serde(default)]
        duration_secs: u64,
    },
    #[serde(other)]
    Unknown,
}

/// A host key waiting for the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostKeyPrompt {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) algorithm: String,
    pub(crate) fingerprint: String,
}

/// Questions waiting for the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuthPrompt {
    /// The host asking (empty from older servers).
    pub(crate) host: String,
    pub(crate) name: String,
    pub(crate) instructions: String,
    pub(crate) prompts: Vec<Prompt>,
}

/// Where a tab is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Phase {
    /// Created, not yet connected.
    Idle,
    /// `opening`, `resolving`, `connecting`, `authenticating`.
    Progress(String),
    HostKey(HostKeyPrompt),
    Auth(AuthPrompt),
    Connected,
    Closed {
        reason: String,
        bytes_in: u64,
        bytes_out: u64,
        duration_secs: u64,
    },
}

impl Phase {
    /// The session is (being) established: input / reconnect make no sense.
    pub(crate) fn is_live(&self) -> bool {
        !matches!(self, Phase::Idle | Phase::Closed { .. })
    }
}

/// A server-reported failure.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TermError {
    pub(crate) code: String,
    pub(crate) detail: Option<String>,
    pub(crate) info: Option<Value>,
}

/// The reactive state of one tab.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TabState {
    pub(crate) id: u32,
    pub(crate) title: String,
    pub(crate) protocol: Proto,
    pub(crate) phase: Phase,
    pub(crate) error: Option<TermError>,
    pub(crate) recording: bool,
    pub(crate) log_file: Option<String>,
    pub(crate) fingerprint: Option<String>,
    pub(crate) address: Option<String>,
    /// The host the current progress step concerns (jump host sessions).
    pub(crate) step_host: Option<String>,
    /// Jump hosts of the connected session.
    pub(crate) via: Vec<String>,
}

impl TabState {
    pub(crate) fn new(id: u32, profile: &Profile) -> Self {
        Self {
            id,
            title: profile.title(),
            protocol: profile.protocol,
            phase: Phase::Idle,
            error: None,
            recording: false,
            log_file: None,
            fingerprint: None,
            address: None,
            step_host: None,
            via: Vec::new(),
        }
    }

    /// Applies a server message.
    pub(crate) fn apply(&mut self, msg: ServerMsg) {
        match msg {
            ServerMsg::State { state, host } => {
                self.phase = Phase::Progress(state);
                self.step_host = host;
            }
            ServerMsg::HostKey {
                host,
                port,
                algorithm,
                fingerprint,
            } => {
                self.phase = Phase::HostKey(HostKeyPrompt {
                    host,
                    port,
                    algorithm,
                    fingerprint,
                });
            }
            ServerMsg::AuthPrompt {
                host,
                name,
                instructions,
                prompts,
            } => {
                self.phase = Phase::Auth(AuthPrompt {
                    host,
                    name,
                    instructions,
                    prompts,
                });
            }
            ServerMsg::Connected {
                address,
                fingerprint,
                via,
            } => {
                self.phase = Phase::Connected;
                self.error = None;
                self.step_host = None;
                self.via = via;
                self.address = Some(address).filter(|a| !a.is_empty());
                self.fingerprint = fingerprint;
            }
            ServerMsg::Record { enabled, file } => {
                self.recording = enabled;
                if file.is_some() {
                    self.log_file = file;
                }
            }
            ServerMsg::Error {
                code,
                detail,
                fatal: _,
                info,
            } => self.error = Some(TermError { code, detail, info }),
            ServerMsg::Closed {
                reason,
                bytes_in,
                bytes_out,
                duration_secs,
            } => {
                self.recording = false;
                self.phase = Phase::Closed {
                    reason,
                    bytes_in,
                    bytes_out,
                    duration_secs,
                };
            }
            ServerMsg::Unknown => {}
        }
    }

    /// The WebSocket closed without a `closed` message.
    pub(crate) fn lost(&mut self) {
        if !matches!(self.phase, Phase::Closed { .. }) {
            self.recording = false;
            self.phase = Phase::Closed {
                reason: "network".to_owned(),
                bytes_in: 0,
                bytes_out: 0,
                duration_secs: 0,
            };
        }
    }
}

/// i18n key of a progress state.
pub(crate) fn state_key(state: &str) -> &'static str {
    match state {
        "resolving" => "ui-term-state-resolving",
        "connecting" => "ui-term-state-connecting",
        "authenticating" => "ui-term-state-authenticating",
        "jumping" => "ui-term-state-jumping",
        _ => "ui-term-state-opening",
    }
}

/// i18n key of an error code (takes `$detail`).
pub(crate) fn error_key(code: &str) -> &'static str {
    match code {
        "target_not_allowed" => "ui-term-error-target-not-allowed",
        "too_many_sessions" => "ui-term-error-too-many-sessions",
        "resolve_failed" => "ui-term-error-resolve-failed",
        "connect_failed" => "ui-term-error-connect-failed",
        "host_key_changed" => "ui-term-error-host-key-changed",
        "host_key_rejected" => "ui-term-error-host-key-rejected",
        "auth_failed" => "ui-term-error-auth-failed",
        "key_invalid" => "ui-term-error-key-invalid",
        "ssh_error" => "ui-term-error-ssh",
        "timeout" => "ui-term-error-timeout",
        "bad_request" => "ui-term-error-bad-request",
        "recording_disabled" => "ui-term-error-recording-disabled",
        "log_failed" => "ui-term-error-log-failed",
        "connection_lost" => "ui-term-error-connection-lost",
        "xterm_load" => "ui-term-error-xterm",
        _ => "ui-term-error-other",
    }
}

/// i18n key of a `closed` reason.
pub(crate) fn closed_key(reason: &str) -> &'static str {
    match reason {
        "remote" => "ui-term-closed-remote",
        "client" => "ui-term-closed-client",
        "idle" => "ui-term-closed-idle",
        "shutdown" => "ui-term-closed-shutdown",
        "error" => "ui-term-closed-error",
        _ => "ui-term-closed-network",
    }
}

/// Lines of a paste / file (CR LF / CR / LF separated; a trailing newline does
/// not add an empty line).
pub(crate) fn split_lines(text: &str) -> Vec<String> {
    let norm = text.replace("\r\n", "\n").replace('\r', "\n");
    let trimmed = norm.strip_suffix('\n').unwrap_or(&norm);
    if trimmed.is_empty() {
        return Vec::new();
    }
    trimmed.split('\n').map(str::to_owned).collect()
}

/// Whether pasting `text` deserves a confirmation (more than one line).
pub(crate) fn is_multiline(text: &str) -> bool {
    split_lines(text).len() > 1 || text.trim_end_matches(['\r', '\n']).len() < text.len()
}

/// A paste / file waiting for confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingPaste {
    pub(crate) tab: u32,
    pub(crate) text: String,
    /// File name when sending a file.
    pub(crate) file: Option<String>,
}

/// Reactive terminal state (context of the app root).
#[derive(Debug, Clone, Copy)]
pub(crate) struct TermStore {
    pub(crate) tabs: RwSignal<Vec<TabState>>,
    pub(crate) active: RwSignal<Option<u32>>,
    pub(crate) settings: RwSignal<TermSettings>,
    pub(crate) profiles: RwSignal<Vec<Profile>>,
    pub(crate) paste: RwSignal<Option<PendingPaste>>,
    /// The connection form is shown (instead of the active terminal).
    pub(crate) show_form: RwSignal<bool>,
    next: StoredValue<u32>,
}

impl TermStore {
    pub(crate) fn new() -> Self {
        Self {
            tabs: RwSignal::new(Vec::new()),
            active: RwSignal::new(None),
            settings: RwSignal::new(load_settings()),
            profiles: RwSignal::new(load_profiles()),
            paste: RwSignal::new(None),
            show_form: RwSignal::new(true),
            next: StoredValue::new(1),
        }
    }

    pub(crate) fn next_id(self) -> u32 {
        let id = self.next.get_value();
        self.next.set_value(id + 1);
        id
    }

    /// Updates tab `id` (no-op when it is gone).
    pub(crate) fn update(self, id: u32, f: impl FnOnce(&mut TabState)) {
        self.tabs.update(|v| {
            if let Some(t) = v.iter_mut().find(|t| t.id == id) {
                f(t);
            }
        });
    }

    /// Tab `id` (untracked).
    pub(crate) fn tab(self, id: u32) -> Option<TabState> {
        self.tabs
            .with_untracked(|v| v.iter().find(|t| t.id == id).cloned())
    }

    /// Removes tab `id`, activating a neighbour.
    pub(crate) fn remove(self, id: u32) {
        let mut next = None;
        self.tabs.update(|v| {
            if let Some(i) = v.iter().position(|t| t.id == id) {
                v.remove(i);
                next = v.get(i.min(v.len().saturating_sub(1))).map(|t| t.id);
            }
        });
        if self.active.get_untracked() == Some(id) {
            self.active.set(next);
        }
        if next.is_none() {
            self.show_form.set(true);
        }
    }
}

/// The [`TermStore`] context.
pub(crate) fn use_term() -> TermStore {
    expect_context::<TermStore>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> ConnectParams {
        ConnectParams {
            profile: Profile {
                name: String::new(),
                protocol: Proto::Ssh,
                host: " r1.lab ".into(),
                port: None,
                username: "admin".into(),
                use_key: false,
                record: true,
                strip_ansi: true,
                jumps: Vec::new(),
            },
            password: "pw".into(),
            private_key: "KEY".into(),
            passphrase: "pp".into(),
            jump_secrets: Vec::new(),
        }
    }

    #[test]
    fn jump_hosts() {
        let mut p = params();
        assert!(p.message(80, 24).get("jump").is_none());
        p.profile.jumps = vec![
            JumpProfile {
                host: " bastion ".into(),
                port: Some(2222),
                username: "ops".into(),
                use_key: false,
            },
            JumpProfile {
                host: "inner".into(),
                port: None,
                username: "ops".into(),
                use_key: true,
            },
        ];
        p.jump_secrets = vec![JumpSecret {
            password: "jpw".into(),
            ..JumpSecret::default()
        }];
        // the second jump host has no key yet
        assert_eq!(p.problems(), ["ui-term-err-jump-key"]);
        p.jump_secrets.push(JumpSecret {
            private_key: "JKEY".into(),
            ..JumpSecret::default()
        });
        assert!(p.problems().is_empty());
        let m = p.message(80, 24);
        assert_eq!(
            m["jump"],
            json!([
                { "host": "bastion", "port": 2222, "username": "ops", "password": "jpw" },
                { "host": "inner", "username": "ops", "private_key": "JKEY" },
            ])
        );
        assert!(!format!("{p:?}").contains("JKEY"));
        let text = serde_json::to_string(&p.profile).unwrap();
        assert!(!text.contains("jpw") && !text.contains("JKEY"), "{text}");
        p.profile.jumps[0].host.clear();
        p.profile.jumps[1].username.clear();
        assert_eq!(
            p.problems(),
            ["ui-term-err-jump-host", "ui-term-err-jump-user"]
        );
        p.profile.jumps = vec![JumpProfile::default(); MAX_JUMPS + 1];
        assert!(p.problems().contains(&"ui-term-err-jump-count"));
        // the status follows the hops
        let mut t = TabState::new(1, &params().profile);
        let msg = |s: &str| serde_json::from_str::<ServerMsg>(s).unwrap();
        t.apply(msg(r#"{"type":"state","state":"jumping","host":"r1:22"}"#));
        assert_eq!(t.step_host.as_deref(), Some("r1:22"));
        t.apply(msg(
            r#"{"type":"auth_prompt","host":"bastion","prompts":[{"prompt":"Password: "}]}"#,
        ));
        assert!(matches!(&t.phase, Phase::Auth(a) if a.host == "bastion"));
        t.apply(msg(
            r#"{"type":"connected","address":"r1:22","via":["ops@bastion:2222"]}"#,
        ));
        assert_eq!(t.via, ["ops@bastion:2222"]);
        assert_eq!(t.step_host, None);
        assert_eq!(state_key("jumping"), "ui-term-state-jumping");
    }

    #[test]
    fn connect_message() {
        let mut p = params();
        let m = p.message(120, 40);
        assert_eq!(m["host"], "r1.lab");
        assert_eq!(m["username"], "admin");
        assert_eq!(m["password"], "pw");
        assert!(m.get("private_key").is_none());
        assert!(m.get("port").is_none());
        assert_eq!(
            (m["cols"].as_u64(), m["rows"].as_u64()),
            (Some(120), Some(40))
        );
        p.profile.use_key = true;
        p.profile.port = Some(2222);
        let m = p.message(80, 24);
        assert_eq!(m["private_key"], "KEY");
        assert_eq!(m["passphrase"], "pp");
        assert!(m.get("password").is_none());
        assert_eq!(m["port"], 2222);
        p.profile.protocol = Proto::Telnet;
        let m = p.message(80, 24);
        assert!(m.get("username").is_none() && m.get("private_key").is_none());
        assert!(!format!("{p:?}").contains("KEY"));
    }

    #[test]
    fn form_problems_and_titles() {
        let mut p = params();
        assert!(p.problems().is_empty());
        p.profile.username = " ".into();
        p.profile.host = "a b".into();
        assert_eq!(p.problems(), ["ui-term-err-host", "ui-term-err-username"]);
        p.profile.protocol = Proto::Telnet;
        p.profile.host = "sw1".into();
        p.profile.port = Some(0);
        assert_eq!(p.problems(), ["ui-term-err-port"]);
        p.profile.port = Some(2001);
        assert_eq!(p.profile.title(), "sw1:2001");
        p.profile.protocol = Proto::Ssh;
        p.profile.username = "u".into();
        p.profile.port = Some(22);
        assert_eq!(p.profile.title(), "u@sw1");
        p.profile.name = "Core".into();
        assert_eq!(p.profile.title(), "Core");
        p.profile.use_key = true;
        p.private_key.clear();
        assert_eq!(p.problems(), ["ui-term-err-key"]);
    }

    #[test]
    fn profiles_round_trip() {
        let mut list = parse_profiles("not json");
        assert!(list.is_empty());
        upsert_profile(&mut list, params().profile);
        let mut b = Profile::blank();
        b.name = "B".into();
        upsert_profile(&mut list, b.clone());
        b.host = "changed".into();
        upsert_profile(&mut list, b);
        assert_eq!(list.len(), 2);
        let text = serde_json::to_string(&list).unwrap();
        assert!(!text.contains("pw"), "no secrets: {text}");
        let back = parse_profiles(&text);
        assert_eq!(back, list);
        assert_eq!(back[1].host, "changed");
        // older / partial entries get defaults
        let p = parse_profiles(r#"[{"name":"x","host":"h"}]"#);
        assert_eq!(p[0].protocol, Proto::Ssh);
    }

    #[test]
    fn settings_are_clamped() {
        let s = TermSettings::parse(r#"{"font_size":200,"scrollback":5,"line_delay_ms":99999}"#);
        assert_eq!(s.font_size, TermSettings::FONT_MAX);
        assert_eq!(s.scrollback, 100);
        assert_eq!(s.line_delay_ms, TermSettings::DELAY_MAX);
        assert!(s.copy_on_select);
        assert_eq!(TermSettings::parse("x"), TermSettings::default());
    }

    #[test]
    fn tab_state_follows_messages() {
        let mut t = TabState::new(1, &params().profile);
        assert_eq!(t.title, "admin@r1.lab");
        let msg = |s: &str| serde_json::from_str::<ServerMsg>(s).unwrap();
        t.apply(msg(r#"{"type":"state","state":"connecting"}"#));
        assert_eq!(t.phase, Phase::Progress("connecting".into()));
        assert!(t.phase.is_live());
        t.apply(msg(
            r#"{"type":"host_key","host":"r1","port":22,"algorithm":"ssh-ed25519","fingerprint":"SHA256:x"}"#,
        ));
        assert!(matches!(&t.phase, Phase::HostKey(h) if h.fingerprint == "SHA256:x"));
        t.apply(msg(
            r#"{"type":"auth_prompt","name":"","instructions":"","prompts":[{"prompt":"Password: ","echo":false}]}"#,
        ));
        assert!(matches!(&t.phase, Phase::Auth(a) if a.prompts.len() == 1 && !a.prompts[0].echo));
        t.apply(msg(
            r#"{"type":"connected","session_id":"s","protocol":"ssh","host":"r1","port":22,"address":"10.0.0.1:22","fingerprint":"SHA256:x"}"#,
        ));
        assert_eq!(t.phase, Phase::Connected);
        assert_eq!(t.address.as_deref(), Some("10.0.0.1:22"));
        t.apply(msg(r#"{"type":"record","enabled":true,"file":"a.log"}"#));
        assert!(t.recording);
        t.apply(msg(r#"{"type":"record","enabled":false}"#));
        assert!(!t.recording);
        assert_eq!(
            t.log_file.as_deref(),
            Some("a.log"),
            "kept for the download"
        );
        t.apply(msg(
            r#"{"type":"error","code":"log_failed","detail":"disk","fatal":false}"#,
        ));
        assert_eq!(
            t.error.as_ref().map(|e| e.code.as_str()),
            Some("log_failed")
        );
        t.apply(msg(
            r#"{"type":"closed","reason":"remote","bytes_in":10,"bytes_out":2,"duration_secs":3}"#,
        ));
        assert!(
            matches!(&t.phase, Phase::Closed { reason, bytes_in: 10, .. } if reason == "remote")
        );
        assert!(!t.phase.is_live());
        t.lost();
        assert!(matches!(&t.phase, Phase::Closed { reason, .. } if reason == "remote"));
        t.apply(msg(r#"{"type":"something_new"}"#));
        let mut u = TabState::new(2, &Profile::blank());
        u.lost();
        assert!(matches!(&u.phase, Phase::Closed { reason, .. } if reason == "network"));
    }

    #[test]
    fn keys_and_lines() {
        assert_eq!(error_key("auth_failed"), "ui-term-error-auth-failed");
        assert_eq!(error_key("???"), "ui-term-error-other");
        assert_eq!(closed_key("idle"), "ui-term-closed-idle");
        assert_eq!(closed_key("x"), "ui-term-closed-network");
        assert_eq!(state_key("authenticating"), "ui-term-state-authenticating");
        assert_eq!(split_lines("a\r\nb\rc\n"), ["a", "b", "c"]);
        assert_eq!(split_lines("one"), ["one"]);
        assert!(split_lines("").is_empty());
        assert_eq!(split_lines("\n\nx"), ["", "", "x"]);
        assert!(is_multiline("a\nb"));
        assert!(is_multiline("conf t\n"));
        assert!(!is_multiline("show ip int brief"));
        assert_eq!(Proto::parse("telnet").default_port(), 23);
        assert_eq!(Proto::parse("x"), Proto::Ssh);
    }

    #[test]
    fn store_tabs() {
        let owner = Owner::new();
        owner.set();
        let s = TermStore::new();
        let a = s.next_id();
        let b = s.next_id();
        s.tabs.update(|v| {
            v.push(TabState::new(a, &Profile::blank()));
            v.push(TabState::new(b, &Profile::blank()));
        });
        s.active.set(Some(b));
        s.update(a, |t| t.title = "A".into());
        assert_eq!(s.tab(a).map(|t| t.title), Some("A".into()));
        s.remove(b);
        assert_eq!(s.active.get_untracked(), Some(a));
        s.remove(a);
        assert_eq!(s.active.get_untracked(), None);
        assert!(s.show_form.get_untracked());
    }
}
