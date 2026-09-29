//! Server-pushed events (WebSocket `/api/ws`).

use serde::Serialize;
use tokio::sync::broadcast;
use utoipa::ToSchema;
use wpg_tx::job::StatsSnapshot;

use crate::jobs::JobStatus;

/// Capacity of the event channel; slow WebSocket clients skip what they missed.
pub const EVENT_CAPACITY: usize = 1024;

/// One message from the server to WebSocket clients.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    /// Latest statistics of a running job (every 100 ms, one per job).
    Stats {
        /// Job id.
        job_id: String,
        /// Counters and rates (`wpg_tx::job::StatsSnapshot`).
        #[schema(value_type = Object)]
        snapshot: StatsSnapshot,
    },
    /// A job changed state.
    JobState {
        /// Job id.
        job_id: String,
        /// New state.
        state: JobStatus,
    },
    /// A log line (`ts`, `level`, `target`, `message`, `fields`).
    Log {
        /// The line (flattened into the message).
        #[serde(flatten)]
        #[schema(value_type = Object)]
        line: wpg_log::LogEvent,
    },
    /// The template index changed (file watcher or API write).
    TemplatesChanged,
}

impl ServerEvent {
    /// Job the event is about, if any (for subscription filtering).
    #[must_use]
    pub fn job_id(&self) -> Option<&str> {
        match self {
            ServerEvent::Stats { job_id, .. } | ServerEvent::JobState { job_id, .. } => {
                Some(job_id)
            }
            _ => None,
        }
    }
}

/// Sender side of the event bus.
pub type EventBus = broadcast::Sender<ServerEvent>;

/// Creates an event bus.
#[must_use]
pub fn bus() -> EventBus {
    broadcast::channel(EVENT_CAPACITY).0
}
