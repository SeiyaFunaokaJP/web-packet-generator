//! The DHCP server tool (ADR-0012): settings and the "running at shutdown"
//! flag in `<data_dir>/dhcp.json`, leases in `<data_dir>/dhcp-leases.json`,
//! the recent activity, and the running [`wpg_dhcp::Server`]. The HTTP side
//! is [`crate::api::dhcp`].

use std::collections::VecDeque;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use wpg_dhcp::config::Problem;
use wpg_dhcp::server::{Event, Hook};
use wpg_dhcp::{Activity, Lease, Leases, Server, Settings, StatsSnapshot};

use crate::config::Config;
use crate::events::{EventBus, ServerEvent};

/// Activities kept for `GET /api/dhcp/activity`.
pub const MAX_ACTIVITY: usize = 500;

/// What `dhcp.json` holds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    /// Interface chosen on the screen (ignored when `dhcp.iface` is set).
    iface: String,
    settings: Settings,
    /// Running when the server stopped: started again at boot.
    running: bool,
}

/// `GET /api/dhcp`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DhcpStatus {
    /// `dhcp.enabled`.
    pub enabled: bool,
    /// Built for Linux (elsewhere the server cannot start).
    pub supported: bool,
    /// `dhcp.iface` (fixed at deploy time); the screen cannot change it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_iface: Option<String>,
    /// Interface used (pinned or chosen; empty: none chosen yet).
    pub iface: String,
    /// Answering.
    pub running: bool,
    /// Start time (milliseconds since the Unix epoch).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_ms: Option<u64>,
    /// DHCPv4 server identifier in use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_ip: Option<String>,
    /// DHCPv6 server DUID in use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_duid: Option<String>,
    /// Addresses this server put on the interface (removed when it stops).
    #[serde(default)]
    pub added_addresses: Vec<String>,
    /// Pools and options (`wpg_dhcp::Settings`).
    #[schema(value_type = Object)]
    pub settings: Settings,
    /// Packet counters of the running server.
    #[schema(value_type = Object)]
    pub stats: StatsSnapshot,
    /// Leases in the table.
    pub leases: usize,
    /// Why the last start (at boot) failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Errors of the manager.
#[derive(Debug, thiserror::Error)]
pub enum DhcpError {
    /// No interface pinned or chosen.
    #[error("no interface chosen")]
    NoIface,
    /// The interface does not exist.
    #[error("interface {0} not found")]
    IfaceNotFound(String),
    /// Settings that cannot work.
    #[error("{}", .0.iter().map(|p| format!("{}: {}", p.field, p.message)).collect::<Vec<_>>().join("; "))]
    Invalid(Vec<Problem>),
    /// Putting an address on the interface failed (or bringing it up).
    #[error("{addr}: {source}")]
    Address {
        /// The address (or the interface).
        addr: String,
        /// Error.
        source: std::io::Error,
    },
    /// Opening the sockets failed.
    #[error("{0}")]
    Socket(std::io::Error),
    /// Saving the settings failed.
    #[error("{path}: {source}")]
    Io {
        /// File.
        path: String,
        /// Error.
        source: std::io::Error,
    },
}

struct Running {
    server: Server,
    iface: String,
    index: u32,
    /// Addresses this server put on the interface (removed at stop).
    added: Vec<(IpAddr, u8)>,
    started_ms: u64,
}

/// Removes the addresses a start added.
fn remove_addresses(index: u32, added: &[(IpAddr, u8)]) {
    for (a, l) in added {
        match wpg_dhcp::netlink::del_address(index, *a, *l) {
            Ok(()) => tracing::info!("dhcp: removed {a}/{l}"),
            Err(e) => tracing::warn!("dhcp: removing {a}/{l}: {e}"),
        }
    }
}

/// Brings the interface up and adds the configured addresses it lacks;
/// returns those added. On a failure the ones added so far are removed.
fn add_addresses(
    os: &wpg_sys::ifaces::Iface,
    addresses: &[String],
) -> Result<Vec<(IpAddr, u8)>, DhcpError> {
    let mut added = Vec::new();
    if addresses.is_empty() {
        return Ok(added);
    }
    if !os.up {
        wpg_dhcp::netlink::set_up(os.index).map_err(|source| DhcpError::Address {
            addr: os.name.clone(),
            source,
        })?;
    }
    for text in addresses {
        let Ok((a, l)) = wpg_dhcp::config::parse_cidr(text) else {
            continue; // validated before
        };
        let present = match a {
            IpAddr::V4(v) => os.ipv4.contains(&v),
            IpAddr::V6(v) => os.ipv6.contains(&v),
        };
        if present {
            continue;
        }
        if let Err(source) = wpg_dhcp::netlink::add_address(os.index, a, l) {
            remove_addresses(os.index, &added);
            return Err(DhcpError::Address {
                addr: text.trim().to_owned(),
                source,
            });
        }
        tracing::info!("dhcp: added {a}/{l} to {}", os.name);
        added.push((a, l));
    }
    Ok(added)
}

/// What the server tasks need (kept apart from [`Inner`] so that the hook
/// does not keep the manager alive).
struct Shared {
    leases: Arc<Mutex<Leases>>,
    leases_path: PathBuf,
    activity: Mutex<VecDeque<Activity>>,
    events: EventBus,
}

impl Shared {
    fn save_leases(&self) {
        let json = {
            let l = self.leases.lock().unwrap_or_else(PoisonError::into_inner);
            serde_json::to_vec_pretty(&*l)
        };
        match json {
            Ok(j) => {
                if let Err(e) = write_atomic(&self.leases_path, &j) {
                    tracing::warn!("{}: {e}", self.leases_path.display());
                }
            }
            Err(e) => tracing::warn!("dhcp leases: {e}"),
        }
    }

    fn on_event(&self, ev: Event) {
        match ev {
            Event::Activity(a) => {
                if a.changed {
                    self.save_leases();
                }
                {
                    let mut l = self.activity.lock().unwrap_or_else(PoisonError::into_inner);
                    l.push_front(a.clone());
                    l.truncate(MAX_ACTIVITY);
                }
                let _ = self.events.send(ServerEvent::DhcpActivity {
                    activity: Box::new(a),
                });
            }
            Event::Expired(_) => {
                self.save_leases();
                let _ = self.events.send(ServerEvent::DhcpState);
            }
        }
    }
}

struct Inner {
    enabled: bool,
    pinned: Option<String>,
    settings_path: PathBuf,
    saved: Mutex<Saved>,
    running: tokio::sync::Mutex<Option<Running>>,
    error: Mutex<Option<String>>,
    shared: Arc<Shared>,
}

/// Shared DHCP state (cheap to clone).
#[derive(Clone)]
pub struct DhcpManager {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for DhcpManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DhcpManager")
            .field("enabled", &self.inner.enabled)
            .field("pinned", &self.inner.pinned)
            .finish_non_exhaustive()
    }
}

fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)
}

fn read_json<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> T {
    match std::fs::read(path) {
        Ok(b) => serde_json::from_slice(&b).unwrap_or_else(|e| {
            tracing::warn!("{}: {e} (ignored)", path.display());
            T::default()
        }),
        Err(_) => T::default(),
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn os_iface(name: &str) -> Result<wpg_sys::ifaces::Iface, DhcpError> {
    wpg_sys::ifaces::list()
        .unwrap_or_default()
        .into_iter()
        .find(|i| i.name == name)
        .ok_or_else(|| DhcpError::IfaceNotFound(name.to_owned()))
}

/// Looks up `name` among the OS interfaces.
///
/// # Errors
/// [`DhcpError::IfaceNotFound`].
pub fn resolve_iface(name: &str) -> Result<wpg_dhcp::Iface, DhcpError> {
    let i = os_iface(name)?;
    Ok(wpg_dhcp::Iface {
        name: i.name,
        index: i.index,
        mac: i.mac,
        ipv4: i.ipv4,
    })
}

impl DhcpManager {
    /// The manager for `config` (loads `dhcp.json` and the leases; nothing
    /// runs yet).
    #[must_use]
    pub fn new(config: &Config, events: EventBus) -> Self {
        let settings_path = config.data_dir.join("dhcp.json");
        let leases_path = config.data_dir.join("dhcp-leases.json");
        let saved: Saved = read_json(&settings_path);
        let mut leases: Leases = read_json(&leases_path);
        leases.purge(wpg_dhcp::now_secs());
        let pinned = Some(config.dhcp.iface.trim().to_owned()).filter(|s| !s.is_empty());
        Self {
            inner: Arc::new(Inner {
                enabled: config.dhcp.enabled,
                pinned,
                settings_path,
                saved: Mutex::new(saved),
                running: tokio::sync::Mutex::new(None),
                error: Mutex::new(None),
                shared: Arc::new(Shared {
                    leases: Arc::new(Mutex::new(leases)),
                    leases_path,
                    activity: Mutex::new(VecDeque::new()),
                    events,
                }),
            }),
        }
    }

    /// `dhcp.enabled`.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.inner.enabled
    }

    /// `dhcp.iface`, when set.
    #[must_use]
    pub fn pinned_iface(&self) -> Option<&str> {
        self.inner.pinned.as_deref()
    }

    fn saved(&self) -> Saved {
        self.inner
            .saved
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn iface_of(&self, saved: &Saved) -> String {
        self.inner
            .pinned
            .clone()
            .unwrap_or_else(|| saved.iface.trim().to_owned())
    }

    fn store(&self, saved: Saved) -> Result<(), DhcpError> {
        let json = serde_json::to_vec_pretty(&saved).map_err(|e| DhcpError::Io {
            path: self.inner.settings_path.display().to_string(),
            source: std::io::Error::other(e),
        })?;
        write_atomic(&self.inner.settings_path, &json).map_err(|source| DhcpError::Io {
            path: self.inner.settings_path.display().to_string(),
            source,
        })?;
        *self
            .inner
            .saved
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = saved;
        Ok(())
    }

    /// Current state.
    pub async fn status(&self) -> DhcpStatus {
        let saved = self.saved();
        let g = self.inner.running.lock().await;
        DhcpStatus {
            enabled: self.inner.enabled,
            supported: cfg!(target_os = "linux"),
            pinned_iface: self.inner.pinned.clone(),
            iface: g
                .as_ref()
                .map_or_else(|| self.iface_of(&saved), |r| r.iface.clone()),
            running: g.is_some(),
            started_ms: g.as_ref().map(|r| r.started_ms),
            server_ip: g
                .as_ref()
                .and_then(|r| r.server.server_ip())
                .filter(|_| saved.settings.v4.enabled)
                .map(|ip| ip.to_string()),
            server_duid: g
                .as_ref()
                .filter(|_| saved.settings.v6.enabled)
                .map(|r| wpg_dhcp::v6::fmt_duid(r.server.duid())),
            added_addresses: g
                .as_ref()
                .map(|r| r.added.iter().map(|(a, l)| format!("{a}/{l}")).collect())
                .unwrap_or_default(),
            settings: saved.settings,
            stats: g.as_ref().map(|r| r.server.stats()).unwrap_or_default(),
            leases: self
                .inner
                .shared
                .leases
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .list
                .len(),
            error: self
                .inner
                .error
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone(),
        }
    }

    /// Saves new settings (and the chosen interface unless pinned); a running
    /// server is restarted with them.
    ///
    /// # Errors
    /// [`DhcpError::Invalid`], disk errors, and those of [`DhcpManager::start`].
    pub async fn update(
        &self,
        iface: Option<String>,
        settings: Settings,
    ) -> Result<DhcpStatus, DhcpError> {
        let problems = settings.validate();
        if !problems.is_empty() {
            return Err(DhcpError::Invalid(problems));
        }
        let mut saved = self.saved();
        if let Some(i) = iface {
            saved.iface = i.trim().to_owned();
        }
        saved.settings = settings;
        let was_running = self.inner.running.lock().await.is_some();
        self.store(saved)?;
        if was_running {
            self.start().await
        } else {
            let _ = self.inner.shared.events.send(ServerEvent::DhcpState);
            Ok(self.status().await)
        }
    }

    /// (Re)starts the server with the saved settings.
    ///
    /// # Errors
    /// [`DhcpError`].
    pub async fn start(&self) -> Result<DhcpStatus, DhcpError> {
        let mut saved = self.saved();
        let name = self.iface_of(&saved);
        if name.is_empty() {
            return Err(DhcpError::NoIface);
        }
        let problems = saved.settings.validate();
        if !problems.is_empty() {
            return Err(DhcpError::Invalid(problems));
        }
        os_iface(&name)?;
        let mut g = self.inner.running.lock().await;
        // Close the old sockets before binding the ports again.
        if let Some(old) = g.take() {
            let Running {
                server,
                index,
                added,
                ..
            } = old;
            drop(server);
            remove_addresses(index, &added);
        }
        let os = os_iface(&name)?;
        let added = add_addresses(&os, &saved.settings.addresses)?;
        // Read again: the server identifier comes from the new addresses.
        let iface = resolve_iface(&name)?;
        let shared = Arc::clone(&self.inner.shared);
        let hook: Hook = Arc::new(move |ev: Event| shared.on_event(ev));
        let server = Server::start(&iface, &saved.settings, &self.inner.shared.leases, &hook)
            .map_err(|e| {
                remove_addresses(iface.index, &added);
                DhcpError::Socket(e)
            })?;
        tracing::info!(
            iface = %name,
            v4 = saved.settings.v4.enabled,
            v6 = saved.settings.v6.enabled,
            ra = saved.settings.v6.ra_enabled,
            "DHCP server started"
        );
        *g = Some(Running {
            server,
            iface: name,
            index: iface.index,
            added,
            started_ms: now_ms(),
        });
        drop(g);
        *self
            .inner
            .error
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
        if !saved.running {
            saved.running = true;
            self.store(saved)?;
        }
        let _ = self.inner.shared.events.send(ServerEvent::DhcpState);
        Ok(self.status().await)
    }

    /// Stops the server. `remember`: clear the "start at boot" flag (a stop
    /// from the screen; not at shutdown).
    pub async fn stop(&self, remember: bool) {
        if let Some(r) = self.inner.running.lock().await.take() {
            let Running {
                server,
                index,
                added,
                ..
            } = r;
            drop(server);
            remove_addresses(index, &added);
            tracing::info!("DHCP server stopped");
        }
        if remember {
            let mut saved = self.saved();
            if saved.running {
                saved.running = false;
                if let Err(e) = self.store(saved) {
                    tracing::warn!("dhcp: {e}");
                }
            }
            let _ = self.inner.shared.events.send(ServerEvent::DhcpState);
        }
    }

    /// Starts the server at boot when it was running at shutdown. Failures
    /// are logged and shown on the screen, not fatal.
    pub async fn start_from_config(&self) {
        if !self.inner.enabled || !self.saved().running {
            return;
        }
        if let Err(e) = self.start().await {
            tracing::error!("DHCP server: {e}");
            *self
                .inner
                .error
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Some(e.to_string());
        }
    }

    /// Leases, sorted by kind and address.
    #[must_use]
    pub fn leases(&self) -> Vec<Lease> {
        let mut l = self
            .inner
            .shared
            .leases
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .list
            .clone();
        l.sort_by_key(|a| (a.kind as u8, a.addr));
        l
    }

    /// Removes the lease with this key (`Lease::key`); `None`: all of them.
    /// True when something was removed.
    pub fn delete_leases(&self, key: Option<&str>) -> bool {
        let removed = {
            let mut l = self
                .inner
                .shared
                .leases
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            match key {
                Some(k) => l.remove_key(k),
                None => {
                    let any = !l.list.is_empty();
                    l.list.clear();
                    any
                }
            }
        };
        if removed {
            self.inner.shared.save_leases();
            let _ = self.inner.shared.events.send(ServerEvent::DhcpState);
        }
        removed
    }

    /// Recent activity, newest first.
    #[must_use]
    pub fn activity(&self, limit: usize) -> Vec<Activity> {
        self.inner
            .shared
            .activity
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .take(limit)
            .cloned()
            .collect()
    }

    /// Forgets the activity log.
    pub fn clear_activity(&self) {
        self.inner
            .shared
            .activity
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager(dir: &Path, iface: &str) -> DhcpManager {
        let mut c = Config {
            data_dir: dir.to_path_buf(),
            ..Config::default()
        };
        c.dhcp.iface = iface.into();
        DhcpManager::new(&c, crate::events::bus())
    }

    #[tokio::test]
    async fn settings_are_saved_and_validated() {
        let dir = tempfile::tempdir().unwrap();
        let m = manager(dir.path(), "");
        let st = m.status().await;
        assert!(!st.running && st.iface.is_empty() && st.pinned_iface.is_none());
        assert!(matches!(m.start().await, Err(DhcpError::NoIface)));
        let mut s = Settings::default();
        s.v4.lease_secs = 1;
        assert!(matches!(
            m.update(None, s.clone()).await,
            Err(DhcpError::Invalid(_))
        ));
        s.v4.lease_secs = 120;
        let st = m.update(Some(" nosuch0 ".into()), s.clone()).await.unwrap();
        assert_eq!(st.iface, "nosuch0");
        assert_eq!(st.settings.v4.lease_secs, 120);
        assert!(matches!(m.start().await, Err(DhcpError::IfaceNotFound(_))));
        // Reloaded from dhcp.json.
        let m = manager(dir.path(), "");
        assert_eq!(m.status().await.settings, s);
        // A pinned interface wins over the chosen one.
        let m = manager(dir.path(), "pinned0");
        let st = m.status().await;
        assert_eq!(
            (st.iface.as_str(), st.pinned_iface.as_deref()),
            ("pinned0", Some("pinned0"))
        );
    }

    #[test]
    fn leases_are_loaded_and_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let lease = |addr: &str| Lease {
            kind: wpg_dhcp::LeaseKind::V4,
            addr: addr.parse().unwrap(),
            plen: 32,
            client: addr.into(),
            iaid: 0,
            mac: None,
            hostname: None,
            state: wpg_dhcp::LeaseState::Bound,
            expires: u64::MAX / 2,
            updated: 0,
        };
        let leases = Leases {
            list: vec![lease("10.0.0.2"), lease("10.0.0.1")],
        };
        std::fs::write(
            dir.path().join("dhcp-leases.json"),
            serde_json::to_vec(&leases).unwrap(),
        )
        .unwrap();
        let m = manager(dir.path(), "");
        let keys: Vec<_> = m.leases().iter().map(Lease::key).collect();
        assert_eq!(keys, ["10.0.0.1", "10.0.0.2"]);
        assert!(m.delete_leases(Some("10.0.0.1")));
        assert!(!m.delete_leases(Some("10.0.0.1")));
        let m = manager(dir.path(), "");
        assert_eq!(m.leases().len(), 1);
        assert!(m.delete_leases(None));
        assert!(m.leases().is_empty());
    }
}
