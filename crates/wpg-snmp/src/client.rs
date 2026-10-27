//! Async SNMP manager (command generator / notification originator) on a
//! tokio UDP socket: Get, GetNext, GetBulk, Walk, Set, Trap (v1 / v2c / v3),
//! Inform; timeouts with retries, request-id (and v3 msgID) matching, v3
//! engine discovery and time synchronisation.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::{Duration, Instant};

use tokio::net::UdpSocket;

use crate::ber::BerError;
use crate::oid::{known, Oid};
use crate::pcap::Tap;
use crate::pdu::{
    decode_community, error_status_name, peek_version, AnyPdu, CommunityMessage, Pdu, PduType,
    TrapV1, Version,
};
use crate::usm::{
    decode_v3, encode_v3, open_v3, LocalKeys, MasterKeys, ScopedPdu, UsmError, UsmUser, V3Out,
};
use crate::value::{Value, VarBind};

/// Largest datagram accepted.
const RECV_BUF: usize = 65_535;

/// Default rows a walk returns at most.
pub const DEFAULT_WALK_LIMIT: usize = 10_000;

/// Client errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Socket errors.
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// No (matching) response after all retries.
    #[error("no response from {target} after {attempts} attempt(s)")]
    Timeout {
        /// The agent.
        target: SocketAddr,
        /// Requests sent.
        attempts: u32,
    },
    /// An undecodable response.
    #[error("malformed response: {0}")]
    Decode(#[from] BerError),
    /// USM failure (authentication, decryption, configuration).
    #[error("{0}")]
    Usm(#[from] UsmError),
    /// The agent answered with a Report PDU (`usmStatsWrongDigests` ...).
    #[error("report from agent: {name}")]
    Report {
        /// The counter's name (`usmStatsUnknownUserNames`, ...).
        name: String,
        /// Its OID.
        oid: Oid,
    },
    /// A non-zero `error-status`.
    #[error("{name} (error-index {index})")]
    Status {
        /// `error-status`.
        status: i32,
        /// Its name.
        name: String,
        /// `error-index`.
        index: i32,
    },
    /// Invalid use (a v1 GetBulk, ...).
    #[error("{0}")]
    Invalid(String),
}

/// Client result.
pub type Result<T> = std::result::Result<T, Error>;

/// Where and how to talk.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// The agent (`host:161`) or notification receiver (`host:162`).
    pub target: SocketAddr,
    /// Protocol version.
    pub version: Version,
    /// Community (v1 / v2c).
    pub community: String,
    /// USM user (v3).
    pub user: Option<UsmUser>,
    /// v3 `contextName`.
    pub context_name: String,
    /// v3 `contextEngineID` (default: the agent's engine id).
    pub context_engine_id: Option<Vec<u8>>,
    /// Wait per attempt.
    pub timeout: Duration,
    /// Extra attempts after the first.
    pub retries: u32,
    /// Engine id of this side as the authoritative engine of v3 traps
    /// (default: a random one per client).
    pub local_engine_id: Option<Vec<u8>>,
}

impl ClientConfig {
    /// v2c with `community`, 2 s timeout, 1 retry.
    #[must_use]
    pub fn v2c(target: SocketAddr, community: &str) -> Self {
        Self {
            target,
            version: Version::V2c,
            community: community.to_owned(),
            user: None,
            context_name: String::new(),
            context_engine_id: None,
            timeout: Duration::from_secs(2),
            retries: 1,
            local_engine_id: None,
        }
    }
}

#[derive(Debug, Clone)]
struct Engine {
    id: Vec<u8>,
    boots: u32,
    time: u32,
    at: Instant,
}

impl Engine {
    fn now(&self) -> u32 {
        let secs = u32::try_from(self.at.elapsed().as_secs()).unwrap_or(u32::MAX);
        self.time.saturating_add(secs)
    }
}

/// The name of a Report counter OID (`usmStatsNotInTimeWindows`, ...).
#[must_use]
pub fn report_name(oid: &Oid) -> String {
    let a = oid.arcs();
    let usm = known::usm_stats();
    if oid.starts_with(&usm) {
        let n = a.get(usm.len()).copied().unwrap_or(0);
        return match n {
            1 => "usmStatsUnsupportedSecLevels",
            2 => "usmStatsNotInTimeWindows",
            3 => "usmStatsUnknownUserNames",
            4 => "usmStatsUnknownEngineIDs",
            5 => "usmStatsWrongDigests",
            6 => "usmStatsDecryptionErrors",
            _ => "usmStats",
        }
        .to_owned();
    }
    let mpd = Oid::from([1, 3, 6, 1, 6, 3, 11, 2, 1]);
    if oid.starts_with(&mpd) {
        return match a.get(mpd.len()).copied().unwrap_or(0) {
            1 => "snmpUnknownSecurityModels",
            2 => "snmpInvalidMsgs",
            3 => "snmpUnknownPDUHandlers",
            _ => "snmpMPDStats",
        }
        .to_owned();
    }
    oid.to_string()
}

fn report_is(oid: &Oid, n: u32) -> bool {
    oid.starts_with(&known::usm_stats().child(&[n]))
}

/// An SNMP session with one peer.
#[derive(Debug)]
pub struct Client {
    sock: UdpSocket,
    cfg: ClientConfig,
    local: SocketAddr,
    tap: Option<Tap>,
    next_id: i32,
    engine: Option<Engine>,
    master: Option<MasterKeys>,
    keys: Option<LocalKeys>,
}

fn random_i31() -> i32 {
    let mut b = [0u8; 4];
    let _ = getrandom::fill(&mut b);
    i32::from_be_bytes(b) & 0x3fff_ffff
}

impl Client {
    /// Opens a UDP socket "connected" to the target (only its datagrams are
    /// received; the local address is the real source address). For v3 the
    /// user's master keys are derived here (pass phrase checks fail early).
    ///
    /// # Errors
    /// Socket errors, invalid USM configuration.
    pub async fn connect(cfg: ClientConfig, tap: Option<Tap>) -> Result<Self> {
        let bind: SocketAddr = if cfg.target.is_ipv4() {
            (Ipv4Addr::UNSPECIFIED, 0).into()
        } else {
            (Ipv6Addr::UNSPECIFIED, 0).into()
        };
        let master = match (&cfg.version, &cfg.user) {
            (Version::V3, Some(u)) => Some(u.master_keys()?),
            (Version::V3, None) => {
                return Err(Error::Invalid("SNMPv3 needs a user".into()));
            }
            _ => None,
        };
        let sock = UdpSocket::bind(bind).await?;
        sock.connect(cfg.target).await?;
        let local = sock.local_addr()?;
        Ok(Self {
            sock,
            cfg,
            local,
            tap,
            next_id: random_i31(),
            engine: None,
            master,
            keys: None,
        })
    }

    /// The local address.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local
    }

    /// The configuration.
    #[must_use]
    pub fn config(&self) -> &ClientConfig {
        &self.cfg
    }

    /// The agent's engine id (v3, after the first request).
    #[must_use]
    pub fn engine_id(&self) -> Option<&[u8]> {
        self.engine.as_ref().map(|e| e.id.as_slice())
    }

    fn take_id(&mut self) -> i32 {
        self.next_id = self.next_id.wrapping_add(1) & 0x7fff_ffff;
        self.next_id
    }

    async fn send(&self, data: &[u8]) -> Result<()> {
        if let Some(t) = &self.tap {
            t.sent(self.local, self.cfg.target, data);
        }
        self.sock.send(data).await?;
        Ok(())
    }

    /// Receives until `deadline`; `None` on timeout.
    async fn recv(&self, buf: &mut [u8], deadline: tokio::time::Instant) -> Result<Option<usize>> {
        match tokio::time::timeout_at(deadline, self.sock.recv(buf)).await {
            Err(_) => Ok(None),
            Ok(Err(e)) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                // ICMP port unreachable from an earlier datagram: keep waiting
                // (the retry may still be answered).
                tokio::time::sleep_until(deadline).await;
                Ok(None)
            }
            Ok(Err(e)) => Err(e.into()),
            Ok(Ok(n)) => {
                if let Some(t) = &self.tap {
                    t.received(self.cfg.target, self.local, &buf[..n]);
                }
                Ok(Some(n))
            }
        }
    }

    /// Sends a request PDU and waits for the matching response PDU (Report
    /// PDUs of v3 become [`Error::Report`]). The `error-status` is left to the
    /// caller.
    ///
    /// # Errors
    /// Timeouts, socket, decoding and USM errors.
    pub async fn request(&mut self, pdu: Pdu) -> Result<Pdu> {
        match self.cfg.version {
            Version::V1 | Version::V2c => self.exchange_community(pdu).await,
            Version::V3 => self.exchange_v3(pdu).await,
        }
    }

    async fn exchange_community(&mut self, pdu: Pdu) -> Result<Pdu> {
        let id = pdu.request_id;
        let msg = CommunityMessage {
            version: self.cfg.version,
            community: self.cfg.community.as_bytes().to_vec(),
            pdu: AnyPdu::Pdu(pdu),
        }
        .encode();
        let mut buf = vec![0u8; RECV_BUF];
        let attempts = self.cfg.retries + 1;
        for _ in 0..attempts {
            self.send(&msg).await?;
            let deadline = tokio::time::Instant::now() + self.cfg.timeout;
            while let Some(n) = self.recv(&mut buf, deadline).await? {
                let Ok(m) = decode_community(&buf[..n]) else {
                    tracing::debug!("snmp: ignoring undecodable datagram");
                    continue;
                };
                if let AnyPdu::Pdu(p) = m.pdu {
                    if p.request_id == id && p.ty == PduType::Response {
                        return Ok(p);
                    }
                }
            }
        }
        Err(Error::Timeout {
            target: self.cfg.target,
            attempts,
        })
    }

    /// One v3 round trip with the given keys / engine values. Returns the
    /// response PDU and the responder's USM parameters (boots, time).
    async fn round_trip_v3(
        &mut self,
        pdu: &Pdu,
        keys: &LocalKeys,
        engine_id: &[u8],
        boots: u32,
        time: u32,
    ) -> Result<(Pdu, Vec<u8>, u32, u32)> {
        let msg_id = self.take_id();
        let scoped = ScopedPdu {
            context_engine_id: self
                .cfg
                .context_engine_id
                .clone()
                .unwrap_or_else(|| engine_id.to_vec()),
            context_name: self.cfg.context_name.as_bytes().to_vec(),
            pdu: AnyPdu::Pdu(pdu.clone()),
        };
        let raw = encode_v3(&V3Out {
            msg_id,
            reportable: pdu.ty.is_confirmed(),
            keys,
            engine_id,
            boots,
            time,
            scoped: &scoped,
        });
        let mut buf = vec![0u8; RECV_BUF];
        let attempts = self.cfg.retries + 1;
        for _ in 0..attempts {
            self.send(&raw).await?;
            let deadline = tokio::time::Instant::now() + self.cfg.timeout;
            while let Some(n) = self.recv(&mut buf, deadline).await? {
                let data = &buf[..n];
                if peek_version(data).ok() != Some(3) {
                    continue;
                }
                let Ok(m) = decode_v3(data) else { continue };
                if m.msg_id != msg_id {
                    continue;
                }
                // Reports of discovery / time sync come unauthenticated.
                let open_keys = if m.flags & crate::usm::flags::AUTH == 0 {
                    LocalKeys::discovery()
                } else {
                    keys.clone()
                };
                let s = open_v3(&m, data, &open_keys)?;
                let AnyPdu::Pdu(p) = s.pdu else { continue };
                return Ok((p, m.usm.engine_id, m.usm.boots, m.usm.time));
            }
        }
        Err(Error::Timeout {
            target: self.cfg.target,
            attempts,
        })
    }

    /// Engine id discovery (RFC 3414 §4): an empty noAuthNoPriv Get whose
    /// Report carries the agent's engine id, boots and time.
    async fn discover(&mut self) -> Result<()> {
        let probe = Pdu::new(PduType::Get, self.take_id(), Vec::new());
        let (_, id, boots, time) = self
            .round_trip_v3(&probe, &LocalKeys::discovery(), &[], 0, 0)
            .await?;
        if id.is_empty() {
            return Err(Error::Invalid(
                "engine discovery returned no engine id".into(),
            ));
        }
        self.set_engine(id, boots, time);
        Ok(())
    }

    fn set_engine(&mut self, id: Vec<u8>, boots: u32, time: u32) {
        let relocalize = self.engine.as_ref().is_none_or(|e| e.id != id);
        if relocalize {
            self.keys = self.master.as_ref().map(|m| m.localize(&id));
        }
        self.engine = Some(Engine {
            id,
            boots,
            time,
            at: Instant::now(),
        });
    }

    async fn exchange_v3(&mut self, pdu: Pdu) -> Result<Pdu> {
        if self.engine.is_none() {
            self.discover().await?;
        }
        let mut synced = false;
        loop {
            let e = self.engine.clone().expect("engine discovered");
            let keys = self.keys.clone().unwrap_or_default();
            let (resp, id, boots, time) = self
                .round_trip_v3(&pdu, &keys, &e.id, e.boots, e.now())
                .await?;
            if resp.ty == PduType::Report {
                let oid = resp
                    .varbinds
                    .first()
                    .map(|v| v.oid.clone())
                    .unwrap_or_default();
                // Out of the time window / a new engine: adopt the reported
                // values and try once more.
                if !synced && (report_is(&oid, 2) || report_is(&oid, 4)) {
                    synced = true;
                    self.set_engine(id, boots, time);
                    continue;
                }
                return Err(Error::Report {
                    name: report_name(&oid),
                    oid,
                });
            }
            if keys.auth.is_some() {
                self.set_engine(id, boots, time);
            }
            return Ok(resp);
        }
    }

    /// `GetRequest`.
    ///
    /// # Errors
    /// See [`Client::request`].
    pub async fn get(&mut self, oids: &[Oid]) -> Result<Pdu> {
        let id = self.take_id();
        self.request(Pdu::new(
            PduType::Get,
            id,
            oids.iter().cloned().map(VarBind::null).collect(),
        ))
        .await
    }

    /// `GetNextRequest`.
    ///
    /// # Errors
    /// See [`Client::request`].
    pub async fn get_next(&mut self, oids: &[Oid]) -> Result<Pdu> {
        let id = self.take_id();
        self.request(Pdu::new(
            PduType::GetNext,
            id,
            oids.iter().cloned().map(VarBind::null).collect(),
        ))
        .await
    }

    /// `GetBulkRequest` (v2c / v3).
    ///
    /// # Errors
    /// [`Error::Invalid`] on v1, else see [`Client::request`].
    pub async fn get_bulk(
        &mut self,
        non_repeaters: i32,
        max_repetitions: i32,
        oids: &[Oid],
    ) -> Result<Pdu> {
        if self.cfg.version == Version::V1 {
            return Err(Error::Invalid("GetBulk needs SNMPv2c or v3".into()));
        }
        let id = self.take_id();
        self.request(Pdu::bulk(id, non_repeaters, max_repetitions, oids))
            .await
    }

    /// `SetRequest`.
    ///
    /// # Errors
    /// See [`Client::request`].
    pub async fn set(&mut self, varbinds: Vec<VarBind>) -> Result<Pdu> {
        let id = self.take_id();
        self.request(Pdu::new(PduType::Set, id, varbinds)).await
    }

    /// Walks the subtree under `root`: GetBulk (`max_repetitions` per request)
    /// on v2c / v3, GetNext on v1. Stops at the end of the subtree, at
    /// `endOfMibView`, at a non-increasing OID (broken agents) or after
    /// `limit` bindings (`truncated` = true). When the subtree is empty the
    /// root itself is fetched with a Get (a scalar instance like
    /// `sysUpTime.0`).
    ///
    /// # Errors
    /// See [`Client::request`]; a non-zero `error-status` other than v1's
    /// `noSuchName` (the v1 end of the MIB).
    pub async fn walk(&mut self, root: &Oid, max_repetitions: i32, limit: usize) -> Result<Walk> {
        let mut out: Vec<VarBind> = Vec::new();
        let mut cur = root.clone();
        let mut truncated = false;
        let mut requests = 0u32;
        'outer: loop {
            let resp = if self.cfg.version == Version::V1 {
                self.get_next(std::slice::from_ref(&cur)).await?
            } else {
                self.get_bulk(0, max_repetitions.max(1), std::slice::from_ref(&cur))
                    .await?
            };
            requests += 1;
            if resp.error_status != 0 {
                if self.cfg.version == Version::V1 && resp.error_status == 2 {
                    break;
                }
                return Err(status_error(&resp));
            }
            if resp.varbinds.is_empty() {
                break;
            }
            for vb in resp.varbinds {
                if vb.value == Value::EndOfMibView || !vb.oid.starts_with(root) || vb.oid <= cur {
                    break 'outer;
                }
                cur = vb.oid.clone();
                out.push(vb);
                if out.len() >= limit {
                    truncated = true;
                    break 'outer;
                }
            }
        }
        if out.is_empty() {
            let resp = self.get(std::slice::from_ref(root)).await?;
            requests += 1;
            if resp.error_status == 0 {
                out.extend(
                    resp.varbinds
                        .into_iter()
                        .filter(|v| !v.value.is_exception()),
                );
            }
        }
        Ok(Walk {
            varbinds: out,
            truncated,
            requests,
        })
    }

    /// Sends an SNMPv1 Trap (v1 only; see [`Client::notify`] for the others).
    ///
    /// # Errors
    /// Socket errors, [`Error::Invalid`] on v2c / v3.
    pub async fn trap_v1(&mut self, trap: TrapV1) -> Result<()> {
        if self.cfg.version != Version::V1 {
            return Err(Error::Invalid("Trap-PDU is SNMPv1 only".into()));
        }
        let msg = CommunityMessage {
            version: Version::V1,
            community: self.cfg.community.as_bytes().to_vec(),
            pdu: AnyPdu::TrapV1(trap),
        }
        .encode();
        self.send(&msg).await
    }

    /// Sends a notification: an SNMPv2-Trap (or, `inform`, an `InformRequest`
    /// that is acknowledged by a Response). On v1 the notification is
    /// converted to a Trap-PDU (RFC 3584 §3.2). Returns the acknowledgement
    /// of an inform.
    ///
    /// # Errors
    /// Socket / timeout / USM errors; informs need v2c or v3.
    pub async fn notify(
        &mut self,
        trap_oid: &Oid,
        uptime: u32,
        varbinds: &[VarBind],
        inform: bool,
    ) -> Result<Option<Pdu>> {
        match self.cfg.version {
            Version::V1 => {
                if inform {
                    return Err(Error::Invalid("InformRequest needs SNMPv2c or v3".into()));
                }
                let agent_addr = match self.local.ip() {
                    IpAddr::V4(a) => a.octets(),
                    IpAddr::V6(_) => [0, 0, 0, 0],
                };
                let t = v1_trap_from(trap_oid, uptime, varbinds, agent_addr);
                self.trap_v1(t).await?;
                Ok(None)
            }
            Version::V2c => {
                let ty = if inform {
                    PduType::Inform
                } else {
                    PduType::TrapV2
                };
                let id = self.take_id();
                let pdu = Pdu::notification(ty, id, uptime, trap_oid, varbinds);
                if inform {
                    return self.exchange_community(pdu).await.map(Some);
                }
                let msg = CommunityMessage {
                    version: Version::V2c,
                    community: self.cfg.community.as_bytes().to_vec(),
                    pdu: AnyPdu::Pdu(pdu),
                }
                .encode();
                self.send(&msg).await?;
                Ok(None)
            }
            Version::V3 => {
                let id = self.take_id();
                if inform {
                    let pdu = Pdu::notification(PduType::Inform, id, uptime, trap_oid, varbinds);
                    return self.exchange_v3(pdu).await.map(Some);
                }
                // A trap's authoritative engine is the sender (this side).
                let engine_id = self
                    .cfg
                    .local_engine_id
                    .clone()
                    .unwrap_or_else(crate::usm::random_engine_id);
                let keys = self
                    .master
                    .as_ref()
                    .map(|m| m.localize(&engine_id))
                    .unwrap_or_default();
                let pdu = Pdu::notification(PduType::TrapV2, id, uptime, trap_oid, varbinds);
                let scoped = ScopedPdu {
                    context_engine_id: self
                        .cfg
                        .context_engine_id
                        .clone()
                        .unwrap_or_else(|| engine_id.clone()),
                    context_name: self.cfg.context_name.as_bytes().to_vec(),
                    pdu: AnyPdu::Pdu(pdu),
                };
                let raw = encode_v3(&V3Out {
                    msg_id: self.take_id(),
                    reportable: false,
                    keys: &keys,
                    engine_id: &engine_id,
                    boots: 1,
                    time: uptime / 100,
                    scoped: &scoped,
                });
                self.send(&raw).await?;
                Ok(None)
            }
        }
    }
}

/// Result of [`Client::walk`].
#[derive(Debug, Clone, Default)]
pub struct Walk {
    /// Bindings in agent order.
    pub varbinds: Vec<VarBind>,
    /// Stopped at the limit.
    pub truncated: bool,
    /// Requests sent.
    pub requests: u32,
}

/// [`Error::Status`] from a response.
#[must_use]
pub fn status_error(p: &Pdu) -> Error {
    Error::Status {
        status: p.error_status,
        name: error_status_name(p.error_status),
        index: p.error_index,
    }
}

/// RFC 3584 §3.2: an SNMPv2 notification as a v1 Trap-PDU.
#[must_use]
pub fn v1_trap_from(
    trap_oid: &Oid,
    uptime: u32,
    varbinds: &[VarBind],
    agent_addr: [u8; 4],
) -> TrapV1 {
    let traps = known::snmp_traps();
    let enterprise_vb = varbinds
        .iter()
        .find(|v| v.oid == known::snmp_trap_enterprise_0())
        .and_then(|v| match &v.value {
            Value::Oid(o) => Some(o.clone()),
            _ => None,
        });
    let rest: Vec<VarBind> = varbinds
        .iter()
        .filter(|v| v.oid != known::snmp_trap_enterprise_0())
        .cloned()
        .collect();
    let a = trap_oid.arcs();
    let (enterprise, generic, specific) = if trap_oid.starts_with(&traps)
        && a.len() == traps.len() + 1
        && (1..=6).contains(&a[a.len() - 1])
    {
        let g = i32::try_from(a[a.len() - 1] - 1).unwrap_or(0);
        (enterprise_vb.unwrap_or_else(|| traps.clone()), g, 0)
    } else if a.len() >= 2 && a[a.len() - 2] == 0 {
        (
            Oid::from(&a[..a.len() - 2]),
            6,
            i32::try_from(a[a.len() - 1]).unwrap_or(0),
        )
    } else {
        (
            trap_oid.parent().unwrap_or_default(),
            6,
            i32::try_from(a.last().copied().unwrap_or(0)).unwrap_or(0),
        )
    };
    TrapV1 {
        enterprise,
        agent_addr,
        generic,
        specific,
        timestamp: uptime,
        varbinds: rest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3584_conversion() {
        let t = v1_trap_from(
            &"1.3.6.1.6.3.1.1.5.3".parse().unwrap(),
            5,
            &[],
            [1, 2, 3, 4],
        );
        assert_eq!(t.generic, 2);
        assert_eq!(t.trap_oid().to_string(), "1.3.6.1.6.3.1.1.5.3");
        let t = v1_trap_from(&"1.3.6.1.4.1.9.9.41.2.0.1".parse().unwrap(), 5, &[], [0; 4]);
        assert_eq!((t.generic, t.specific), (6, 1));
        assert_eq!(t.enterprise.to_string(), "1.3.6.1.4.1.9.9.41.2");
        assert_eq!(t.trap_oid().to_string(), "1.3.6.1.4.1.9.9.41.2.0.1");
        assert_eq!(
            report_name(&"1.3.6.1.6.3.15.1.1.5.0".parse().unwrap()),
            "usmStatsWrongDigests"
        );
    }
}
