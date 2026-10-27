//! `/api/terminal`: status, session logs and SSH host keys (the session itself
//! is a WebSocket, see [`crate::term::session`]).

use serde::Deserialize;

use super::{enc, get, request, ApiError, Body, Method};

/// A running session (`GET /api/terminal/status`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct ActiveSession {
    pub(crate) id: String,
    pub(crate) user: String,
    pub(crate) protocol: String,
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) started: u64,
    #[serde(default)]
    pub(crate) log: Option<String>,
}

/// `GET /api/terminal/status`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct TerminalStatus {
    pub(crate) enabled: bool,
    pub(crate) record_sessions: bool,
    pub(crate) idle_timeout_secs: u64,
    pub(crate) max_sessions: usize,
    #[serde(default)]
    pub(crate) allowed_targets: Vec<String>,
    #[serde(default)]
    pub(crate) sessions: Vec<ActiveSession>,
}

/// A session log.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct LogInfo {
    pub(crate) name: String,
    pub(crate) size: u64,
    pub(crate) modified: u64,
    pub(crate) active: bool,
}

/// A trusted SSH host key.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct KnownHost {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) algorithm: String,
    pub(crate) fingerprint: String,
    #[serde(default)]
    pub(crate) comment: String,
}

pub(crate) async fn status() -> Result<TerminalStatus, ApiError> {
    get("/api/terminal/status").await
}

pub(crate) async fn logs() -> Result<Vec<LogInfo>, ApiError> {
    get("/api/terminal/logs").await
}

/// URL of a log download (a plain link downloads it).
pub(crate) fn log_url(name: &str) -> String {
    format!("/api/terminal/logs/{}", enc(name))
}

pub(crate) async fn delete_log(name: &str) -> Result<(), ApiError> {
    request(Method::Delete, &log_url(name), Body::Empty)
        .await
        .map(|_| ())
}

pub(crate) async fn known_hosts() -> Result<Vec<KnownHost>, ApiError> {
    get("/api/terminal/known-hosts").await
}

pub(crate) async fn delete_known_host(host: &str, port: u16) -> Result<(), ApiError> {
    request(
        Method::Delete,
        &format!("/api/terminal/known-hosts/{}/{port}", enc(host)),
        Body::Empty,
    )
    .await
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_status_and_urls() {
        let s: TerminalStatus = serde_json::from_str(
            r#"{"enabled":true,"record_sessions":false,"idle_timeout_secs":60,"max_sessions":2,
                "allowed_targets":["10.0.0.0/8"],"sessions":[{"id":"a","user":"admin","protocol":"ssh",
                "host":"r1","port":22,"started":1}]}"#,
        )
        .unwrap();
        assert!(s.enabled && !s.record_sessions);
        assert_eq!(s.sessions[0].log, None);
        assert_eq!(log_url("a b.log"), "/api/terminal/logs/a%20b.log");
    }
}
