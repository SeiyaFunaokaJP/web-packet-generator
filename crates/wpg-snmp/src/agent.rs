//! A small in-memory SNMP agent (command responder): v1 / v2c communities and
//! v3 USM users, Get / GetNext / GetBulk / Set over a sorted OID → value map.
//!
//! It exists for tests and demos (the wpg integration tests run the client
//! and the HTTP API against it on 127.0.0.1), not as a production agent: no
//! views, no row creation, no time-window enforcement.

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use tokio::net::UdpSocket;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::oid::{known, Oid};
use crate::pdu::{decode_community, peek_version, AnyPdu, CommunityMessage, Pdu, PduType, Version};
use crate::usm::{decode_v3, encode_v3, flags, open_v3, LocalKeys, ScopedPdu, UsmUser, V3Out};
use crate::value::{Value, VarBind};

/// The agent's MIB view.
pub type Store = Arc<Mutex<BTreeMap<Oid, Value>>>;

/// Agent settings.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Listen address (`127.0.0.1:0` for a random port).
    pub listen: SocketAddr,
    /// Read-only community.
    pub community: String,
    /// Read-write community.
    pub write_community: String,
    /// v3 users.
    pub users: Vec<UsmUser>,
    /// Engine id.
    pub engine_id: Vec<u8>,
}

/// A running agent.
#[derive(Debug)]
pub struct Agent {
    local: SocketAddr,
    store: Store,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl Agent {
    /// Starts the agent with `data`.
    ///
    /// # Errors
    /// Bind errors, invalid users.
    pub async fn start(cfg: AgentConfig, data: BTreeMap<Oid, Value>) -> std::io::Result<Self> {
        let mut keys = HashMap::new();
        for u in &cfg.users {
            let m = u.master_keys().map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string())
            })?;
            keys.insert(u.name.clone(), m.localize(&cfg.engine_id));
        }
        let sock = UdpSocket::bind(cfg.listen).await?;
        let local = sock.local_addr()?;
        let store: Store = Arc::new(Mutex::new(data));
        let (tx, rx) = oneshot::channel();
        let st = AgentState {
            sock,
            cfg,
            keys,
            store: Arc::clone(&store),
            started: Instant::now(),
        };
        let task = tokio::spawn(st.run(rx));
        Ok(Self {
            local,
            store,
            stop: Some(tx),
            task,
        })
    }

    /// The bound address.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local
    }

    /// The data (inspect / change while running).
    #[must_use]
    pub fn store(&self) -> Store {
        Arc::clone(&self.store)
    }

    /// Stops the agent.
    pub async fn stop(mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(());
        }
        let _ = (&mut self.task).await;
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(());
        }
    }
}

struct AgentState {
    sock: UdpSocket,
    cfg: AgentConfig,
    keys: HashMap<String, LocalKeys>,
    store: Store,
    started: Instant,
}

impl AgentState {
    async fn run(self, mut stop: oneshot::Receiver<()>) {
        let mut buf = vec![0u8; 65_535];
        loop {
            tokio::select! {
                _ = &mut stop => break,
                r = self.sock.recv_from(&mut buf) => {
                    let Ok((n, peer)) = r else { continue };
                    let data = buf[..n].to_vec();
                    if let Some(reply) = self.handle(&data) {
                        let _ = self.sock.send_to(&reply, peer).await;
                    }
                }
            }
        }
    }

    fn time(&self) -> u32 {
        u32::try_from(self.started.elapsed().as_secs()).unwrap_or(u32::MAX)
    }

    fn handle(&self, data: &[u8]) -> Option<Vec<u8>> {
        match peek_version(data).ok()? {
            3 => self.handle_v3(data),
            _ => {
                let m = decode_community(data).ok()?;
                let community = String::from_utf8_lossy(&m.community).into_owned();
                let write = community == self.cfg.write_community;
                if !write && community != self.cfg.community {
                    return None;
                }
                let AnyPdu::Pdu(p) = m.pdu else { return None };
                let resp = self.respond(&p, m.version, write)?;
                Some(
                    CommunityMessage {
                        version: m.version,
                        community: m.community,
                        pdu: AnyPdu::Pdu(resp),
                    }
                    .encode(),
                )
            }
        }
    }

    fn report(&self, msg_id: i32, request_id: i32, counter: u32, keys: &LocalKeys) -> Vec<u8> {
        let scoped = ScopedPdu {
            context_engine_id: self.cfg.engine_id.clone(),
            context_name: Vec::new(),
            pdu: AnyPdu::Pdu(Pdu::new(
                PduType::Report,
                request_id,
                vec![VarBind::new(
                    known::usm_stats().child(&[counter, 0]),
                    Value::Counter32(1),
                )],
            )),
        };
        encode_v3(&V3Out {
            msg_id,
            reportable: false,
            keys,
            engine_id: &self.cfg.engine_id,
            boots: 1,
            time: self.time(),
            scoped: &scoped,
        })
    }

    fn handle_v3(&self, data: &[u8]) -> Option<Vec<u8>> {
        let m = decode_v3(data).ok()?;
        let plain_id = match &m.data {
            crate::usm::MsgData::Plain(s) => match &s.pdu {
                AnyPdu::Pdu(p) => p.request_id,
                AnyPdu::TrapV1(_) => 0,
            },
            crate::usm::MsgData::Encrypted(_) => 0,
        };
        let none = LocalKeys::discovery();
        if m.usm.engine_id != self.cfg.engine_id {
            return Some(self.report(m.msg_id, plain_id, 4, &none));
        }
        let user = String::from_utf8_lossy(&m.usm.user).into_owned();
        let Some(keys) = self.keys.get(&user) else {
            return Some(self.report(m.msg_id, plain_id, 3, &none));
        };
        if m.flags & 3 != keys.level().flags() {
            return Some(self.report(m.msg_id, plain_id, 1, &none));
        }
        let scoped = match open_v3(&m, data, keys) {
            Ok(s) => s,
            Err(crate::usm::UsmError::WrongDigest) => {
                return Some(self.report(m.msg_id, plain_id, 5, &none))
            }
            Err(_) => return Some(self.report(m.msg_id, plain_id, 6, &none)),
        };
        let AnyPdu::Pdu(p) = &scoped.pdu else {
            return None;
        };
        if m.flags & flags::REPORTABLE == 0 {
            return None;
        }
        let resp = self.respond(p, Version::V3, true)?;
        let out = ScopedPdu {
            context_engine_id: scoped.context_engine_id.clone(),
            context_name: scoped.context_name.clone(),
            pdu: AnyPdu::Pdu(resp),
        };
        Some(encode_v3(&V3Out {
            msg_id: m.msg_id,
            reportable: false,
            keys,
            engine_id: &self.cfg.engine_id,
            boots: 1,
            time: self.time(),
            scoped: &out,
        }))
    }

    fn respond(&self, p: &Pdu, version: Version, write: bool) -> Option<Pdu> {
        let mut store = self.store.lock().unwrap_or_else(PoisonError::into_inner);
        let v1 = version == Version::V1;
        let mut resp = Pdu::new(PduType::Response, p.request_id, Vec::new());
        let err = |resp: &mut Pdu, status: i32, i: usize, vbs: &[VarBind]| {
            resp.error_status = status;
            resp.error_index = i32::try_from(i + 1).unwrap_or(0);
            resp.varbinds = vbs.to_vec();
        };
        let next = |store: &BTreeMap<Oid, Value>, o: &Oid| {
            store
                .range(o.clone()..)
                .find(|(k, _)| *k > o)
                .map(|(k, v)| (k.clone(), v.clone()))
        };
        match p.ty {
            PduType::Get => {
                for (i, vb) in p.varbinds.iter().enumerate() {
                    match store.get(&vb.oid) {
                        Some(v) => resp.varbinds.push(VarBind::new(vb.oid.clone(), v.clone())),
                        None if v1 => {
                            err(&mut resp, 2, i, &p.varbinds);
                            break;
                        }
                        None => {
                            let exists = store
                                .keys()
                                .any(|k| k.parent().as_ref() == vb.oid.parent().as_ref());
                            resp.varbinds.push(VarBind::new(
                                vb.oid.clone(),
                                if exists {
                                    Value::NoSuchInstance
                                } else {
                                    Value::NoSuchObject
                                },
                            ));
                        }
                    }
                }
            }
            PduType::GetNext => {
                for (i, vb) in p.varbinds.iter().enumerate() {
                    match next(&store, &vb.oid) {
                        Some((k, v)) => resp.varbinds.push(VarBind::new(k, v)),
                        None if v1 => {
                            err(&mut resp, 2, i, &p.varbinds);
                            break;
                        }
                        None => resp
                            .varbinds
                            .push(VarBind::new(vb.oid.clone(), Value::EndOfMibView)),
                    }
                }
            }
            PduType::GetBulk => {
                let nr = usize::try_from(p.error_status.max(0))
                    .unwrap_or(0)
                    .min(p.varbinds.len());
                let reps = usize::try_from(p.error_index.max(0)).unwrap_or(0);
                for vb in &p.varbinds[..nr] {
                    match next(&store, &vb.oid) {
                        Some((k, v)) => resp.varbinds.push(VarBind::new(k, v)),
                        None => resp
                            .varbinds
                            .push(VarBind::new(vb.oid.clone(), Value::EndOfMibView)),
                    }
                }
                let mut cursors: Vec<Oid> =
                    p.varbinds[nr..].iter().map(|v| v.oid.clone()).collect();
                for _ in 0..reps {
                    for c in &mut cursors {
                        match next(&store, c) {
                            Some((k, v)) => {
                                *c = k.clone();
                                resp.varbinds.push(VarBind::new(k, v));
                            }
                            None => resp
                                .varbinds
                                .push(VarBind::new(c.clone(), Value::EndOfMibView)),
                        }
                    }
                }
            }
            PduType::Set => {
                if !write {
                    err(&mut resp, if v1 { 2 } else { 6 }, 0, &p.varbinds);
                    return Some(resp);
                }
                for (i, vb) in p.varbinds.iter().enumerate() {
                    match store.get(&vb.oid) {
                        None => {
                            err(&mut resp, if v1 { 2 } else { 17 }, i, &p.varbinds);
                            return Some(resp);
                        }
                        Some(old)
                            if std::mem::discriminant(old) != std::mem::discriminant(&vb.value) =>
                        {
                            err(&mut resp, if v1 { 3 } else { 7 }, i, &p.varbinds);
                            return Some(resp);
                        }
                        Some(_) => {}
                    }
                }
                for vb in &p.varbinds {
                    store.insert(vb.oid.clone(), vb.value.clone());
                }
                resp.varbinds.clone_from(&p.varbinds);
            }
            _ => return None,
        }
        Some(resp)
    }
}
