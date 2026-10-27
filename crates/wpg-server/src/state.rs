//! Shared application state.

use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use tokio::sync::{broadcast, watch};
use wpg_protocols::Catalog;
use wpg_sys::ifaces::Iface;

use crate::auth::{Auth, AuthError};
use crate::config::Config;
use crate::db::{Db, DbError};
use crate::events::{bus, EventBus, ServerEvent};
use crate::jobs::JobManager;
use crate::rx::RxManager;
use crate::snmp::SnmpManager;
use crate::store::{StoreError, TemplateStore};
use crate::terminal::Terminal;

/// How long the interface list is cached.
const IFACE_TTL: Duration = Duration::from_secs(5);

/// Capacity of the log channel.
const LOG_CAPACITY: usize = 512;

/// The process-wide log channel fed by the `tracing` sink layer (see
/// [`crate::init_tracing`]). Bounded: slow receivers lose the oldest lines.
#[must_use]
pub fn log_channel() -> &'static broadcast::Sender<wpg_log::LogEvent> {
    static TX: OnceLock<broadcast::Sender<wpg_log::LogEvent>> = OnceLock::new();
    TX.get_or_init(|| broadcast::channel(LOG_CAPACITY).0)
}

#[derive(Debug, Default)]
struct IfaceCache {
    at: Option<Instant>,
    list: Vec<Iface>,
    error: Option<String>,
}

/// Errors of [`AppState::new`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StateError {
    /// Data / template directories.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The database.
    #[error("{path}: {source}")]
    Db {
        /// Database file.
        path: String,
        /// Underlying error.
        source: DbError,
    },
    /// The configured password.
    #[error(transparent)]
    Auth(#[from] AuthError),
}

/// Everything the handlers share (cheap to clone).
#[derive(Clone)]
pub struct AppState {
    /// Effective configuration.
    pub config: Arc<Config>,
    /// Protocol catalog.
    pub catalog: &'static Catalog,
    /// Template store.
    pub templates: TemplateStore,
    /// Job manager.
    pub jobs: JobManager,
    /// Capture manager.
    pub rx: RxManager,
    /// Web terminal.
    pub terminal: Terminal,
    /// SNMP manager tool.
    pub snmp: SnmpManager,
    /// The `SQLite` database.
    pub db: Db,
    /// Authentication.
    pub auth: Auth,
    /// Server start time.
    pub started_at: Instant,
    /// Job / template events for WebSocket clients.
    pub events: EventBus,
    /// Log lines for WebSocket clients.
    pub logs: broadcast::Sender<wpg_log::LogEvent>,
    /// Flips to `true` when the server shuts down (closes the WebSocket sessions).
    pub shutdown: watch::Receiver<bool>,
    shutdown_tx: Arc<watch::Sender<bool>>,
    ifaces: Arc<Mutex<IfaceCache>>,
    /// Development mode (`--dev`: permissive CORS for Trunk on :8081).
    pub dev: bool,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("config", &self.config)
            .field("templates", &self.templates)
            .field("jobs", &self.jobs)
            .field("dev", &self.dev)
            .finish_non_exhaustive()
    }
}

impl AppState {
    /// Builds the state: creates the data directories, opens the database
    /// (`<data_dir>/wpg.sqlite`), applies the configured password, scans the
    /// template libraries and loads the job history. Call
    /// [`AppState::watch_templates`] to enable hot reload.
    ///
    /// # Errors
    /// If the data / user template directory cannot be created, the database
    /// cannot be opened or the configured password is invalid.
    pub fn new(config: Config) -> Result<Self, StateError> {
        let config = Arc::new(config);
        let events = bus();
        let catalog = wpg_protocols::catalog();
        std::fs::create_dir_all(&config.data_dir).map_err(|source| StoreError::Io {
            path: config.data_dir.display().to_string(),
            source,
        })?;
        let db_path = config.db_path();
        let db = Db::open(&db_path).map_err(|source| StateError::Db {
            path: db_path.display().to_string(),
            source,
        })?;
        let auth = Auth::new(&config, db.clone())?;
        let templates = TemplateStore::open(
            &config.user_template_dir(),
            &config.templates.library_paths,
            config.data_dir.join("template-meta.json"),
            events.clone(),
        )?;
        let jobs = JobManager::new(
            Arc::clone(&config),
            templates.clone(),
            catalog,
            events.clone(),
            db.clone(),
        );
        let rx = RxManager::new(Arc::clone(&config), events.clone());
        let terminal = Terminal::new(&config);
        let snmp = SnmpManager::new(&config, events.clone());
        let (shutdown_tx, shutdown) = watch::channel(false);
        Ok(Self {
            config,
            catalog,
            templates,
            jobs,
            rx,
            terminal,
            snmp,
            db,
            auth,
            started_at: Instant::now(),
            events,
            logs: log_channel().clone(),
            shutdown,
            shutdown_tx: Arc::new(shutdown_tx),
            ifaces: Arc::new(Mutex::new(IfaceCache::default())),
            dev: false,
        })
    }

    /// Enables development mode (CORS for the Trunk dev server).
    #[must_use]
    pub fn with_dev(mut self, dev: bool) -> Self {
        self.dev = dev;
        self
    }

    /// Starts the template file watcher (hot reload).
    pub fn watch_templates(&self) {
        if let Err(e) = self.templates.watch(Duration::from_millis(500)) {
            tracing::warn!("template hot reload disabled: {e}");
        }
    }

    /// Signals WebSocket handlers to close.
    pub fn begin_shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }

    /// Subscribes to job / template events.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.events.subscribe()
    }

    /// Network interfaces (cached for a few seconds; `refresh` forces a re-read).
    ///
    /// # Errors
    /// The OS error message when enumeration fails.
    pub fn ifaces(&self, refresh: bool) -> Result<Vec<Iface>, String> {
        let mut c = self.ifaces.lock().unwrap_or_else(PoisonError::into_inner);
        let stale = c.at.is_none_or(|t| t.elapsed() > IFACE_TTL);
        if refresh || stale {
            match wpg_sys::ifaces::list() {
                Ok(l) => {
                    c.list = l;
                    c.error = None;
                }
                Err(e) => c.error = Some(e.to_string()),
            }
            c.at = Some(Instant::now());
        }
        match &c.error {
            Some(e) => Err(e.clone()),
            None => Ok(c.list.clone()),
        }
    }

    /// Whether the policy lets jobs send on `iface`.
    #[must_use]
    pub fn iface_allowed(&self, iface: &str) -> bool {
        let allowed = &self.config.tx.allowed_ifaces;
        allowed.is_empty() || allowed.iter().any(|a| a == iface)
    }
}
