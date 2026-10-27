//! Notification receiver: a UDP listener that decodes SNMPv1 / v2c / v3 traps
//! and informs and acknowledges informs.
//!
//! * v1 / v2c: an optional community allow list (a mismatch is still
//!   reported, with an error, so misconfigurations are visible);
//! * v3: users configured with pass phrases. Traps are authenticated with keys
//!   localised to the sender's engine id; informs to this receiver's engine id,
//!   which a sender learns from the `usmStatsUnknownEngineIDs` Report sent in
//!   answer to its discovery probe. noAuthNoPriv messages are accepted from any
//!   user name. The time window is not enforced (a test tool should show late
//!   messages, not drop them).

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use tokio::net::UdpSocket;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::oid::{known, Oid};
use crate::pcap::Tap;
use crate::pdu::{decode_community, peek_version, AnyPdu, CommunityMessage, Pdu, PduType, Version};
use crate::usm::{
    decode_v3, encode_v3, flags, open_v3, LocalKeys, MasterKeys, ScopedPdu, SecurityLevel, UsmUser,
    V3Out,
};
use crate::value::{Value, VarBind};

/// Receiver settings.
#[derive(Debug, Clone)]
pub struct ReceiverConfig {
    /// Listen address (`0.0.0.0:162`; ports below 1024 need privileges).
    pub listen: SocketAddr,
    /// Accepted communities (empty = any).
    pub communities: Vec<String>,
    /// v3 users.
    pub users: Vec<UsmUser>,
    /// This receiver's engine id (authoritative for informs).
    pub engine_id: Vec<u8>,
}

/// The v1 Trap-PDU header of a received v1 trap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V1Header {
    /// `enterprise`.
    pub enterprise: Oid,
    /// `agent-addr`.
    pub agent_addr: [u8; 4],
    /// `generic-trap`.
    pub generic: i32,
    /// `specific-trap`.
    pub specific: i32,
}

/// One received notification.
#[derive(Debug, Clone)]
pub struct Notification {
    /// Arrival time.
    pub received_at: SystemTime,
    /// Sender.
    pub source: SocketAddr,
    /// Version.
    pub version: Version,
    /// `trap_v1`, `trap_v2` or `inform` (`None` when undecodable).
    pub pdu_type: Option<PduType>,
    /// Community (v1 / v2c).
    pub community: Option<String>,
    /// User (v3).
    pub user: Option<String>,
    /// v3 security level.
    pub level: Option<SecurityLevel>,
    /// `snmpTrapOID.0` (v1 traps converted per RFC 3584).
    pub trap_oid: Option<Oid>,
    /// `sysUpTime.0` / v1 `time-stamp`.
    pub uptime: Option<u32>,
    /// The bindings (v2 notifications without the two leading ones).
    pub varbinds: Vec<VarBind>,
    /// v1 header.
    pub v1: Option<V1Header>,
    /// An inform that was acknowledged.
    pub acknowledged: bool,
    /// Why the message could not be accepted (still reported).
    pub error: Option<String>,
}

impl Notification {
    fn new(source: SocketAddr, version: Version) -> Self {
        Self {
            received_at: SystemTime::now(),
            source,
            version,
            pdu_type: None,
            community: None,
            user: None,
            level: None,
            trap_oid: None,
            uptime: None,
            varbinds: Vec::new(),
            v1: None,
            acknowledged: false,
            error: None,
        }
    }

    fn fill_from(&mut self, pdu: &AnyPdu) {
        self.pdu_type = Some(pdu.ty());
        match pdu {
            AnyPdu::TrapV1(t) => {
                self.trap_oid = Some(t.trap_oid());
                self.uptime = Some(t.timestamp);
                self.varbinds.clone_from(&t.varbinds);
                self.v1 = Some(V1Header {
                    enterprise: t.enterprise.clone(),
                    agent_addr: t.agent_addr,
                    generic: t.generic,
                    specific: t.specific,
                });
            }
            AnyPdu::Pdu(p) => {
                let mut rest = Vec::with_capacity(p.varbinds.len());
                for vb in &p.varbinds {
                    match (
                        &vb.value,
                        vb.oid == known::sys_uptime_0(),
                        vb.oid == known::snmp_trap_oid_0(),
                    ) {
                        (Value::TimeTicks(t), true, _) if self.uptime.is_none() => {
                            self.uptime = Some(*t);
                        }
                        (Value::Oid(o), _, true) if self.trap_oid.is_none() => {
                            self.trap_oid = Some(o.clone());
                        }
                        _ => rest.push(vb.clone()),
                    }
                }
                self.varbinds = rest;
            }
        }
    }
}

/// Callback for every notification.
pub type Sink = Arc<dyn Fn(Notification) + Send + Sync>;

/// A running receiver.
#[derive(Debug)]
pub struct Receiver {
    local: SocketAddr,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl Receiver {
    /// Binds `cfg.listen` and starts receiving. Master keys of the v3 users
    /// are derived up front.
    ///
    /// # Errors
    /// Bind errors (permission denied for privileged ports), invalid users.
    pub async fn start(cfg: ReceiverConfig, tap: Option<Tap>, sink: Sink) -> std::io::Result<Self> {
        let mut users = HashMap::new();
        for u in &cfg.users {
            let m = u.master_keys().map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("{}: {e}", u.name))
            })?;
            users.insert(u.name.clone(), m);
        }
        let sock = UdpSocket::bind(cfg.listen).await?;
        let local = sock.local_addr()?;
        let (stop_tx, stop_rx) = oneshot::channel();
        let state = State {
            sock,
            local,
            cfg,
            users,
            keys: HashMap::new(),
            routes: HashMap::new(),
            tap,
            sink,
            started: Instant::now(),
        };
        let task = tokio::spawn(state.run(stop_rx));
        Ok(Self {
            local,
            stop: Some(stop_tx),
            task,
        })
    }

    /// The bound address.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local
    }

    /// Stops and waits for the task.
    pub async fn stop(mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(());
        }
        let _ = (&mut self.task).await;
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(());
        }
    }
}

struct State {
    sock: UdpSocket,
    local: SocketAddr,
    cfg: ReceiverConfig,
    users: HashMap<String, MasterKeys>,
    keys: HashMap<(String, Vec<u8>), LocalKeys>,
    routes: HashMap<IpAddr, IpAddr>,
    tap: Option<Tap>,
    sink: Sink,
    started: Instant,
}

impl State {
    async fn run(mut self, mut stop: oneshot::Receiver<()>) {
        let mut buf = vec![0u8; 65_535];
        loop {
            tokio::select! {
                _ = &mut stop => break,
                r = self.sock.recv_from(&mut buf) => match r {
                    Ok((n, peer)) => {
                        let data = buf[..n].to_vec();
                        self.handle(&data, peer).await;
                    }
                    Err(e) => {
                        tracing::debug!("snmp receiver: {e}");
                    }
                },
            }
        }
    }

    /// Our address towards `peer` (for the pcap frames when bound to the
    /// wildcard address).
    fn local_for(&mut self, peer: SocketAddr) -> SocketAddr {
        if !self.local.ip().is_unspecified() {
            return self.local;
        }
        let ip = *self.routes.entry(peer.ip()).or_insert_with(|| {
            let bind: SocketAddr = if peer.is_ipv4() {
                "0.0.0.0:0".parse().expect("literal")
            } else {
                "[::]:0".parse().expect("literal")
            };
            std::net::UdpSocket::bind(bind)
                .and_then(|s| {
                    s.connect(peer)?;
                    s.local_addr()
                })
                .map_or(bind.ip(), |a| a.ip())
        });
        SocketAddr::new(ip, self.local.port())
    }

    async fn reply(&mut self, peer: SocketAddr, data: &[u8]) {
        if let Some(t) = self.tap.clone() {
            let l = self.local_for(peer);
            t.sent(l, peer, data);
        }
        if let Err(e) = self.sock.send_to(data, peer).await {
            tracing::debug!("snmp receiver: reply to {peer}: {e}");
        }
    }

    async fn handle(&mut self, data: &[u8], peer: SocketAddr) {
        if let Some(t) = self.tap.clone() {
            let l = self.local_for(peer);
            t.received(peer, l, data);
        }
        let n = match peek_version(data) {
            Ok(3) => self.handle_v3(data, peer).await,
            Ok(_) => self.handle_community(data, peer).await,
            Err(e) => {
                let mut n = Notification::new(peer, Version::V2c);
                n.error = Some(format!("undecodable datagram: {e}"));
                Some(n)
            }
        };
        if let Some(n) = n {
            (self.sink)(n);
        }
    }

    async fn handle_community(&mut self, data: &[u8], peer: SocketAddr) -> Option<Notification> {
        let m = match decode_community(data) {
            Ok(m) => m,
            Err(e) => {
                let mut n = Notification::new(peer, Version::V2c);
                n.error = Some(format!("undecodable message: {e}"));
                return Some(n);
            }
        };
        let ty = m.pdu.ty();
        if !matches!(ty, PduType::TrapV1 | PduType::TrapV2 | PduType::Inform) {
            return None;
        }
        let mut n = Notification::new(peer, m.version);
        let community = String::from_utf8_lossy(&m.community).into_owned();
        n.community = Some(community.clone());
        n.fill_from(&m.pdu);
        if !self.cfg.communities.is_empty() && !self.cfg.communities.contains(&community) {
            n.error = Some(format!("community `{community}` is not accepted"));
            return Some(n);
        }
        if let AnyPdu::Pdu(p) = &m.pdu {
            if p.ty == PduType::Inform {
                let resp = CommunityMessage {
                    version: m.version,
                    community: m.community.clone(),
                    pdu: AnyPdu::Pdu(Pdu::new(
                        PduType::Response,
                        p.request_id,
                        p.varbinds.clone(),
                    )),
                }
                .encode();
                self.reply(peer, &resp).await;
                n.acknowledged = true;
            }
        }
        Some(n)
    }

    fn uptime_secs(&self) -> u32 {
        u32::try_from(self.started.elapsed().as_secs()).unwrap_or(u32::MAX)
    }

    async fn handle_v3(&mut self, data: &[u8], peer: SocketAddr) -> Option<Notification> {
        let mut n = Notification::new(peer, Version::V3);
        let m = match decode_v3(data) {
            Ok(m) => m,
            Err(e) => {
                n.error = Some(format!("undecodable message: {e}"));
                return Some(n);
            }
        };
        let user = String::from_utf8_lossy(&m.usm.user).into_owned();
        n.user = Some(user.clone());
        n.level = Some(match m.flags & 3 {
            0 => SecurityLevel::NoAuthNoPriv,
            1 => SecurityLevel::AuthNoPriv,
            _ => SecurityLevel::AuthPriv,
        });
        let ours = self.cfg.engine_id.clone();
        let reportable = m.flags & flags::REPORTABLE != 0;
        // Discovery of this receiver (inform senders): report our engine.
        if reportable && m.usm.engine_id != ours {
            let report = ScopedPdu {
                context_engine_id: ours.clone(),
                context_name: Vec::new(),
                pdu: AnyPdu::Pdu(Pdu::new(
                    PduType::Report,
                    request_id_of(&m.data).unwrap_or(0),
                    vec![VarBind::new(
                        known::usm_stats().child(&[4, 0]),
                        Value::Counter32(1),
                    )],
                )),
            };
            let raw = encode_v3(&V3Out {
                msg_id: m.msg_id,
                reportable: false,
                keys: &LocalKeys {
                    user: m.usm.user.clone(),
                    ..LocalKeys::default()
                },
                engine_id: &ours,
                boots: 1,
                time: self.uptime_secs(),
                scoped: &report,
            });
            self.reply(peer, &raw).await;
            return None;
        }
        let keys = if m.flags & flags::AUTH == 0 {
            LocalKeys {
                user: m.usm.user.clone(),
                ..LocalKeys::default()
            }
        } else {
            let Some(master) = self.users.get(&user) else {
                n.error = Some(format!("unknown user `{user}`"));
                return Some(n);
            };
            self.keys
                .entry((user.clone(), m.usm.engine_id.clone()))
                .or_insert_with(|| master.localize(&m.usm.engine_id))
                .clone()
        };
        let scoped = match open_v3(&m, data, &keys) {
            Ok(s) => s,
            Err(e) => {
                n.error = Some(e.to_string());
                return Some(n);
            }
        };
        let ty = scoped.pdu.ty();
        if !matches!(ty, PduType::TrapV1 | PduType::TrapV2 | PduType::Inform) {
            return None;
        }
        n.fill_from(&scoped.pdu);
        if let AnyPdu::Pdu(p) = &scoped.pdu {
            if p.ty == PduType::Inform {
                let resp = ScopedPdu {
                    context_engine_id: scoped.context_engine_id.clone(),
                    context_name: scoped.context_name.clone(),
                    pdu: AnyPdu::Pdu(Pdu::new(
                        PduType::Response,
                        p.request_id,
                        p.varbinds.clone(),
                    )),
                };
                let raw = encode_v3(&V3Out {
                    msg_id: m.msg_id,
                    reportable: false,
                    keys: &keys,
                    engine_id: &ours,
                    boots: 1,
                    time: self.uptime_secs(),
                    scoped: &resp,
                });
                self.reply(peer, &raw).await;
                n.acknowledged = true;
            }
        }
        Some(n)
    }
}

/// The request id of a plaintext scoped PDU (for the discovery report).
fn request_id_of(d: &crate::usm::MsgData) -> Option<i32> {
    match d {
        crate::usm::MsgData::Plain(s) => match &s.pdu {
            AnyPdu::Pdu(p) => Some(p.request_id),
            AnyPdu::TrapV1(_) => None,
        },
        crate::usm::MsgData::Encrypted(_) => None,
    }
}
