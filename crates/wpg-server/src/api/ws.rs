//! `GET /api/ws`: server-pushed events.
//!
//! Server → client: [`ServerEvent`] as JSON (`stats`, `job_state`, `log`,
//! `templates_changed`). Client → server: `{"type": "subscribe", "jobs": "all" |
//! ["<id>", ...], "logs": true|false}` (default: all jobs, logs on). A client that
//! falls behind skips the events it missed (bounded channels, no back-pressure on
//! the TX engine).

// utoipa's generated code spells out paths the lint considers redundant.
#![allow(unused_qualifications)]

use std::collections::HashSet;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::broadcast::error::RecvError;
use utoipa::ToSchema;

use crate::events::ServerEvent;
use crate::state::AppState;

/// Client → server message.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Selects the jobs whose `stats` / `job_state` events are sent.
    Subscribe {
        /// `"all"` or a list of job ids.
        #[schema(value_type = Object)]
        #[serde(default)]
        jobs: Option<Value>,
        /// Receive `log` events (default true).
        #[serde(default)]
        logs: Option<bool>,
    },
}

#[derive(Debug)]
struct Subscription {
    jobs: Option<HashSet<String>>,
    logs: bool,
}

impl Subscription {
    fn wants(&self, ev: &ServerEvent) -> bool {
        match (ev.job_id(), &self.jobs) {
            (Some(id), Some(set)) => set.contains(id),
            _ => true,
        }
    }

    fn apply(&mut self, msg: ClientMessage) {
        let ClientMessage::Subscribe { jobs, logs } = msg;
        match jobs {
            Some(Value::Array(ids)) => {
                self.jobs = Some(
                    ids.into_iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect(),
                );
            }
            Some(Value::String(s)) if s == "all" => self.jobs = None,
            _ => {}
        }
        if let Some(l) = logs {
            self.logs = l;
        }
    }
}

/// WebSocket upgrade.
#[utoipa::path(get, path = "/api/ws", tag = "events",
    responses((status = 101, description = "WebSocket: JSON `ServerEvent` messages", body = ServerEvent)))]
pub async fn ws(State(st): State<AppState>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(move |socket| session(st, socket))
}

async fn send(socket: &mut WebSocket, ev: &ServerEvent) -> bool {
    match serde_json::to_string(ev) {
        Ok(text) => socket.send(Message::Text(text.into())).await.is_ok(),
        Err(_) => true,
    }
}

async fn session(st: AppState, mut socket: WebSocket) {
    let mut events = st.subscribe();
    let mut logs = st.logs.subscribe();
    let mut shutdown = st.shutdown.clone();
    let mut sub = Subscription {
        jobs: None,
        logs: true,
    };
    if *shutdown.borrow() {
        return;
    }
    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                let _ = socket.send(Message::Close(None)).await;
                break;
            }
            msg = socket.recv() => match msg {
                Some(Ok(Message::Text(t))) => {
                    if let Ok(m) = serde_json::from_str::<ClientMessage>(&t) {
                        sub.apply(m);
                    }
                }
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            ev = events.recv() => match ev {
                Ok(ev) => {
                    if sub.wants(&ev) && !send(&mut socket, &ev).await {
                        break;
                    }
                }
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => break,
            },
            line = logs.recv() => match line {
                Ok(line) => {
                    if sub.logs && !send(&mut socket, &ServerEvent::Log { line }).await {
                        break;
                    }
                }
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => break,
            },
        }
    }
}
