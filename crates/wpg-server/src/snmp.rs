//! The SNMP manager tool (ADR-0010): MIB registry with uploads persisted under
//! `<data_dir>/mibs`, the trap / inform receiver with its recent
//! notifications, and the pcap recorder shared by every SNMP exchange. The
//! operations themselves are in [`crate::api::snmp`].
//!
//! The registry is built lazily (bundled modules + the files in the MIB
//! directory) on first use, so servers that never open the SNMP screen do not
//! pay for parsing.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::time::{Instant, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use wpg_snmp::mib::{format_varbind, Formatted, Registry, Source};
use wpg_snmp::pcap::{PcapRecorder, Tap};
use wpg_snmp::receiver::{Notification, Receiver, ReceiverConfig};
use wpg_snmp::usm::UsmUser;

use crate::config::Config;
use crate::events::{EventBus, ServerEvent};

/// Notifications kept for `GET /api/snmp/traps`.
pub const MAX_TRAPS: usize = 1000;

/// Largest accepted MIB file.
pub const MAX_MIB_BYTES: usize = 4 << 20;

/// File extensions accepted for MIB uploads.
pub const MIB_EXTENSIONS: &[&str] = &["mib", "txt", "my", "smi", "asn", "asn1"];

/// A rendered variable binding.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VarBindOut {
    /// OID (dotted decimal).
    pub oid: String,
    /// `IF-MIB::ifDescr.3` (numeric when no MIB knows it).
    pub name: String,
    /// Defining module.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    /// Object descriptor (`ifDescr`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    /// Instance sub-identifiers (`3`).
    pub instance: String,
    /// Wire type (`Counter32`, `OCTET STRING`, `noSuchInstance`, ...).
    #[serde(rename = "type")]
    pub value_type: String,
    /// Textual convention of the object (`DisplayString`, `InterfaceIndex`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax: Option<String>,
    /// Readable value (enum labels, display hints, units, names).
    pub value: String,
    /// The value without MIB knowledge.
    pub raw: String,
    /// Numeric value of integer types (rates, charts).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numeric: Option<u64>,
    /// UNITS of the object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<String>,
}

impl From<Formatted> for VarBindOut {
    fn from(f: Formatted) -> Self {
        Self {
            oid: f.oid,
            name: f.name,
            module: f.module,
            object: f.object,
            instance: f.instance,
            value_type: f.value_type,
            syntax: f.syntax,
            value: f.value,
            raw: f.raw,
            numeric: f.numeric,
            units: f.units,
        }
    }
}

/// A received notification.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TrapOut {
    /// Sequence number (increasing).
    pub id: u64,
    /// Arrival, milliseconds since the Unix epoch.
    pub received_ms: u64,
    /// Sender `address:port`.
    pub source: String,
    /// `v1`, `v2c`, `v3`.
    pub version: String,
    /// `trap_v1`, `trap_v2`, `inform` (absent when undecodable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pdu_type: Option<String>,
    /// Community (v1 / v2c).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub community: Option<String>,
    /// User (v3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// v3 security level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    /// `snmpTrapOID.0` (dotted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trap_oid: Option<String>,
    /// The trap OID's name (`IF-MIB::linkDown`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trap_name: Option<String>,
    /// `sysUpTime.0` / v1 time-stamp (hundredths of a second).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uptime: Option<u32>,
    /// Uptime as `1d 02:03:04.56`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uptime_text: Option<String>,
    /// v1 `enterprise` / `agent-addr` / generic / specific.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v1: Option<V1Out>,
    /// Bindings (without `sysUpTime.0` / `snmpTrapOID.0`).
    pub varbinds: Vec<VarBindOut>,
    /// An inform that was acknowledged.
    pub acknowledged: bool,
    /// Why the message was not accepted (wrong community, digest, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// SNMPv1 trap header.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct V1Out {
    /// `enterprise` (name or dotted).
    pub enterprise: String,
    /// `agent-addr`.
    pub agent_addr: String,
    /// `generic-trap`.
    pub generic: i32,
    /// `specific-trap`.
    pub specific: i32,
}

/// Trap receiver state.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReceiverStatus {
    /// Listening.
    pub running: bool,
    /// Bound address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen: Option<String>,
    /// Accepted communities (empty = any).
    pub communities: Vec<String>,
    /// Configured v3 user names.
    pub users: Vec<String>,
    /// This receiver's engine id (hex), for v3 informs.
    pub engine_id: String,
    /// Notifications received since the start.
    pub received: u64,
    /// Started at boot from `snmp.trap_listen`.
    pub from_config: bool,
}

/// pcap recording state.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CaptureStatus {
    /// Recording.
    pub enabled: bool,
    /// Kept frames.
    pub frames: usize,
    /// Frames dropped because the buffer was full.
    pub dropped: u64,
}

struct Running {
    rx: Receiver,
    communities: Vec<String>,
    users: Vec<String>,
    from_config: bool,
}

struct Inner {
    enabled: bool,
    trap_listen: String,
    mib_dir: PathBuf,
    registry: RwLock<Option<Registry>>,
    /// Uploaded file → modules it defines.
    files: Mutex<BTreeMap<String, Vec<String>>>,
    recorder: Arc<PcapRecorder>,
    receiver: tokio::sync::Mutex<Option<Running>>,
    traps: Mutex<VecDeque<TrapOut>>,
    seq: AtomicU64,
    received: AtomicU64,
    events: EventBus,
    engine_id: Vec<u8>,
    started: Instant,
}

/// Shared SNMP state (cheap to clone).
#[derive(Clone)]
pub struct SnmpManager {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for SnmpManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SnmpManager")
            .field("enabled", &self.inner.enabled)
            .field("mib_dir", &self.inner.mib_dir)
            .finish_non_exhaustive()
    }
}

/// Errors of MIB uploads / deletion.
#[derive(Debug, thiserror::Error)]
pub enum MibError {
    /// File name or extension not acceptable.
    #[error("{0}")]
    BadName(String),
    /// Too large.
    #[error("{0}: larger than {MAX_MIB_BYTES} bytes")]
    TooLarge(String),
    /// Bundled modules cannot be deleted.
    #[error("{0} is bundled")]
    Bundled(String),
    /// Unknown module.
    #[error("{0}")]
    NotFound(String),
    /// Disk errors.
    #[error("{path}: {source}")]
    Io {
        /// File.
        path: String,
        /// Error.
        source: std::io::Error,
    },
}

/// Result of one uploaded file.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UploadedFile {
    /// Stored file name.
    pub file: String,
    /// Modules found in it.
    pub modules: Vec<String>,
    /// Problems (no module found: the file was not stored).
    pub errors: Vec<String>,
    /// Whether the file was stored.
    pub stored: bool,
}

/// Keeps `[A-Za-z0-9._-]` of a file name and requires a MIB extension (a
/// name without an extension gets `.mib`).
///
/// # Errors
/// [`MibError::BadName`].
pub fn sanitize_file_name(name: &str) -> Result<String, MibError> {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name).trim();
    let clean: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let clean = clean.trim_start_matches('.').to_owned();
    if clean.is_empty() || clean.len() > 128 {
        return Err(MibError::BadName(name.to_owned()));
    }
    match clean.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => {
            if MIB_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()) {
                Ok(clean)
            } else {
                Err(MibError::BadName(name.to_owned()))
            }
        }
        _ => Ok(format!("{clean}.mib")),
    }
}

impl SnmpManager {
    /// The manager for `config` (nothing is loaded or started yet).
    #[must_use]
    pub fn new(config: &Config, events: EventBus) -> Self {
        Self {
            inner: Arc::new(Inner {
                enabled: config.snmp.enabled,
                trap_listen: config.snmp.trap_listen.trim().to_owned(),
                mib_dir: config.mib_dir(),
                registry: RwLock::new(None),
                files: Mutex::new(BTreeMap::new()),
                recorder: Arc::new(PcapRecorder::default()),
                receiver: tokio::sync::Mutex::new(None),
                traps: Mutex::new(VecDeque::new()),
                seq: AtomicU64::new(0),
                received: AtomicU64::new(0),
                events,
                engine_id: wpg_snmp::usm::random_engine_id(),
                started: Instant::now(),
            }),
        }
    }

    /// Whether the tool is enabled (`snmp.enabled`).
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.inner.enabled
    }

    /// The MIB directory.
    #[must_use]
    pub fn mib_dir(&self) -> &Path {
        &self.inner.mib_dir
    }

    /// Hundredths of a second since the server started (default `sysUpTime`
    /// of sent notifications).
    #[must_use]
    pub fn uptime_ticks(&self) -> u32 {
        u32::try_from(self.inner.started.elapsed().as_millis() / 10).unwrap_or(u32::MAX)
    }

    /// This side's engine id (v3 traps sent, informs received).
    #[must_use]
    pub fn engine_id(&self) -> &[u8] {
        &self.inner.engine_id
    }

    /// A tap for clients (records while recording is on).
    #[must_use]
    pub fn tap(&self) -> Tap {
        Tap::new(Arc::clone(&self.inner.recorder))
    }

    /// The recorder.
    #[must_use]
    pub fn recorder(&self) -> &PcapRecorder {
        &self.inner.recorder
    }

    /// Recording state.
    #[must_use]
    pub fn capture_status(&self) -> CaptureStatus {
        let r = &self.inner.recorder;
        CaptureStatus {
            enabled: r.enabled(),
            frames: r.len(),
            dropped: r.dropped(),
        }
    }

    /// Runs `f` with the registry (built on first use).
    pub fn with_registry<R>(&self, f: impl FnOnce(&Registry) -> R) -> R {
        {
            let g = self
                .inner
                .registry
                .read()
                .unwrap_or_else(PoisonError::into_inner);
            if let Some(r) = g.as_ref() {
                return f(r);
            }
        }
        self.reload();
        let g = self
            .inner
            .registry
            .read()
            .unwrap_or_else(PoisonError::into_inner);
        f(g.as_ref().expect("registry loaded"))
    }

    /// Rebuilds the registry: bundled modules, then every MIB file in the MIB
    /// directory (file name order; a later file replaces a module of the same
    /// name).
    pub fn reload(&self) {
        let mut reg = Registry::new();
        for (_, text) in wpg_snmp::mib::BUNDLED {
            reg.add(text, &Source::Bundled);
        }
        let mut files = BTreeMap::new();
        if let Ok(rd) = std::fs::read_dir(&self.inner.mib_dir) {
            let mut paths: Vec<PathBuf> = rd.filter_map(Result::ok).map(|e| e.path()).collect();
            paths.sort();
            for p in paths {
                if !p.is_file() {
                    continue;
                }
                let Some(name) = p.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                    continue;
                };
                match std::fs::read(&p) {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        let r = reg.add(&text, &Source::File(name.clone()));
                        files.insert(name, r.modules);
                    }
                    Err(e) => tracing::warn!("MIB {}: {e}", p.display()),
                }
            }
        }
        reg.rebuild();
        *self
            .inner
            .files
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = files;
        *self
            .inner
            .registry
            .write()
            .unwrap_or_else(PoisonError::into_inner) = Some(reg);
    }

    /// Uploaded files and their modules.
    #[must_use]
    pub fn files(&self) -> BTreeMap<String, Vec<String>> {
        self.with_registry(|_| ());
        self.inner
            .files
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Stores uploaded MIB files and rebuilds the registry. A file without any
    /// module is reported and not stored; an earlier upload whose modules are
    /// all redefined by a new file is replaced.
    ///
    /// # Errors
    /// Bad names, oversized files, disk errors.
    pub fn upload(&self, uploads: &[(String, String)]) -> Result<Vec<UploadedFile>, MibError> {
        let io = |path: &Path, source| MibError::Io {
            path: path.display().to_string(),
            source,
        };
        let existing = self.files();
        let mut out = Vec::new();
        let mut checked = Vec::new();
        for (name, content) in uploads {
            let file = sanitize_file_name(name)?;
            if content.len() > MAX_MIB_BYTES {
                return Err(MibError::TooLarge(file));
            }
            checked.push((file, content));
        }
        std::fs::create_dir_all(&self.inner.mib_dir).map_err(|e| io(&self.inner.mib_dir, e))?;
        for (file, content) in checked {
            let parsed = wpg_snmp::mib::parser::parse(content);
            let modules: Vec<String> = parsed.modules.iter().map(|m| m.name.clone()).collect();
            if modules.is_empty() {
                out.push(UploadedFile {
                    file,
                    modules,
                    errors: parsed.errors,
                    stored: false,
                });
                continue;
            }
            for (old, old_modules) in &existing {
                if old != &file
                    && !old_modules.is_empty()
                    && old_modules.iter().all(|m| modules.contains(m))
                {
                    let p = self.inner.mib_dir.join(old);
                    if let Err(e) = std::fs::remove_file(&p) {
                        tracing::warn!("replacing {}: {e}", p.display());
                    }
                }
            }
            let path = self.inner.mib_dir.join(&file);
            std::fs::write(&path, content).map_err(|e| io(&path, e))?;
            out.push(UploadedFile {
                file,
                modules,
                errors: parsed.errors,
                stored: true,
            });
        }
        self.reload();
        Ok(out)
    }

    /// Deletes the uploaded file that defines `module` (with every module in
    /// it) and rebuilds. Returns the removed modules.
    ///
    /// # Errors
    /// Unknown module, bundled module, disk errors.
    pub fn delete_module(&self, module: &str) -> Result<Vec<String>, MibError> {
        let source = self.with_registry(|r| r.source_of(module).cloned());
        match source {
            None => Err(MibError::NotFound(module.to_owned())),
            Some(Source::Bundled) => Err(MibError::Bundled(module.to_owned())),
            Some(Source::File(f)) => {
                let path = self.inner.mib_dir.join(&f);
                let removed = self.files().get(&f).cloned().unwrap_or_default();
                std::fs::remove_file(&path).map_err(|source| MibError::Io {
                    path: path.display().to_string(),
                    source,
                })?;
                self.reload();
                Ok(removed)
            }
        }
    }

    /// Renders a received notification and keeps / broadcasts it.
    fn on_notification(&self, n: &Notification) {
        let id = self.inner.seq.fetch_add(1, Ordering::Relaxed) + 1;
        self.inner.received.fetch_add(1, Ordering::Relaxed);
        let out = self.with_registry(|reg| TrapOut {
            id,
            received_ms: n
                .received_at
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX)),
            source: n.source.to_string(),
            version: n.version.name().to_owned(),
            pdu_type: n.pdu_type.map(|t| t.name().to_owned()),
            community: n.community.clone(),
            user: n.user.clone(),
            level: n.level.map(|l| {
                match l {
                    wpg_snmp::usm::SecurityLevel::NoAuthNoPriv => "noAuthNoPriv",
                    wpg_snmp::usm::SecurityLevel::AuthNoPriv => "authNoPriv",
                    wpg_snmp::usm::SecurityLevel::AuthPriv => "authPriv",
                }
                .to_owned()
            }),
            trap_oid: n.trap_oid.as_ref().map(ToString::to_string),
            trap_name: n.trap_oid.as_ref().map(|o| reg.name_of(o)),
            uptime: n.uptime,
            uptime_text: n.uptime.map(wpg_snmp::mib::format::timeticks),
            v1: n.v1.as_ref().map(|h| V1Out {
                enterprise: reg.name_of(&h.enterprise),
                agent_addr: std::net::Ipv4Addr::from(h.agent_addr).to_string(),
                generic: h.generic,
                specific: h.specific,
            }),
            varbinds: n
                .varbinds
                .iter()
                .map(|vb| format_varbind(reg, vb).into())
                .collect(),
            acknowledged: n.acknowledged,
            error: n.error.clone(),
        });
        {
            let mut t = self
                .inner
                .traps
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            t.push_front(out.clone());
            t.truncate(MAX_TRAPS);
        }
        let _ = self.inner.events.send(ServerEvent::SnmpTrap {
            trap: Box::new(out),
        });
    }

    /// Recent notifications, newest first.
    #[must_use]
    pub fn traps(&self, limit: usize) -> Vec<TrapOut> {
        self.inner
            .traps
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .take(limit)
            .cloned()
            .collect()
    }

    /// Forgets the kept notifications.
    pub fn clear_traps(&self) {
        self.inner
            .traps
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }

    /// The receiver state.
    pub async fn receiver_status(&self) -> ReceiverStatus {
        let g = self.inner.receiver.lock().await;
        ReceiverStatus {
            running: g.is_some(),
            listen: g.as_ref().map(|r| r.rx.local_addr().to_string()),
            communities: g
                .as_ref()
                .map(|r| r.communities.clone())
                .unwrap_or_default(),
            users: g.as_ref().map(|r| r.users.clone()).unwrap_or_default(),
            engine_id: wpg_snmp::usm::hex(&self.inner.engine_id),
            received: self.inner.received.load(Ordering::Relaxed),
            from_config: g.as_ref().is_some_and(|r| r.from_config),
        }
    }

    /// (Re)starts the receiver on `listen`.
    ///
    /// # Errors
    /// Bind errors (`PermissionDenied` for privileged ports), invalid users.
    pub async fn start_receiver(
        &self,
        listen: std::net::SocketAddr,
        communities: Vec<String>,
        users: Vec<UsmUser>,
        from_config: bool,
    ) -> std::io::Result<ReceiverStatus> {
        let mut g = self.inner.receiver.lock().await;
        if let Some(old) = g.take() {
            old.rx.stop().await;
        }
        // Weak: the receiver lives inside `Inner`; a strong reference would
        // keep both alive forever.
        let weak = Arc::downgrade(&self.inner);
        let names = users.iter().map(|u| u.name.clone()).collect();
        let rx = Receiver::start(
            ReceiverConfig {
                listen,
                communities: communities.clone(),
                users,
                engine_id: self.inner.engine_id.clone(),
            },
            Some(self.tap()),
            Arc::new(move |n: Notification| {
                if let Some(inner) = weak.upgrade() {
                    SnmpManager { inner }.on_notification(&n);
                }
            }),
        )
        .await?;
        tracing::info!("SNMP trap receiver listening on {}", rx.local_addr());
        *g = Some(Running {
            rx,
            communities,
            users: names,
            from_config,
        });
        drop(g);
        Ok(self.receiver_status().await)
    }

    /// Stops the receiver (no-op when not running).
    pub async fn stop_receiver(&self) {
        if let Some(r) = self.inner.receiver.lock().await.take() {
            r.rx.stop().await;
            tracing::info!("SNMP trap receiver stopped");
        }
    }

    /// Starts the receiver from `snmp.trap_listen` (startup). Failures are
    /// logged, not fatal: the UI can start it later.
    pub async fn start_from_config(&self) {
        if !self.inner.enabled || self.inner.trap_listen.is_empty() {
            return;
        }
        let addr = match self.inner.trap_listen.parse() {
            Ok(a) => a,
            Err(e) => {
                tracing::error!("snmp.trap_listen `{}`: {e}", self.inner.trap_listen);
                return;
            }
        };
        if let Err(e) = self
            .start_receiver(addr, Vec::new(), Vec::new(), true)
            .await
        {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                tracing::error!(
                    "SNMP trap receiver: cannot bind {addr}: {e} (ports below 1024 need root \
                     or CAP_NET_BIND_SERVICE: sudo setcap cap_net_bind_service=+ep $(which wpg))"
                );
            } else {
                tracing::error!("SNMP trap receiver: cannot bind {addr}: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names() {
        assert_eq!(sanitize_file_name("CISCO-SMI.my").unwrap(), "CISCO-SMI.my");
        assert_eq!(sanitize_file_name("../../etc/x.txt").unwrap(), "x.txt");
        assert_eq!(sanitize_file_name("C:\\mibs\\A B.mib").unwrap(), "A_B.mib");
        assert_eq!(
            sanitize_file_name("JUNIPER-MIB").unwrap(),
            "JUNIPER-MIB.mib"
        );
        assert!(sanitize_file_name("evil.exe").is_err());
        assert!(sanitize_file_name("...").is_err());
        assert!(sanitize_file_name("").is_err());
    }
}
