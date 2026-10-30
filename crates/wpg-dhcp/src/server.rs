//! The running server: one UDP socket per family bound to the interface
//! (`SO_BINDTODEVICE`), the raw ICMPv6 socket of the router advertisements and
//! a task that expires leases. Linux only; elsewhere [`Server::start`] fails
//! with [`std::io::ErrorKind::Unsupported`].

use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::config::Settings;
use crate::lease::Leases;

/// How often expired leases are dropped.
pub const PURGE_SECS: u64 = 10;

/// The interface the server answers on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Iface {
    /// `eth1`.
    pub name: String,
    /// OS index (multicast membership).
    pub index: u32,
    /// Hardware address (server DUID, RA source link-layer option).
    pub mac: Option<[u8; 6]>,
    /// IPv4 addresses (the first one is the default server identifier).
    pub ipv4: Vec<Ipv4Addr>,
}

/// Which part of the server an [`Activity`] is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    /// DHCPv4.
    V4,
    /// DHCPv6.
    V6,
    /// Router advertisements.
    Ra,
}

/// One exchange (or failure), for the activity log of the screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Activity {
    /// Milliseconds since the Unix epoch.
    pub ts_ms: u64,
    /// Family.
    pub family: Family,
    /// `DISCOVER`, `SOLICIT`, `RS`, `-` (periodic RA), `?`.
    pub request: String,
    /// Client MAC / DUID (or the sender address).
    pub client: String,
    /// `OFFER`, `REPLY`, `RA`, ... when something was sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply: Option<String>,
    /// Addresses / prefixes involved.
    #[serde(default)]
    pub addrs: Vec<String>,
    /// Why nothing was sent, decode / send errors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The lease table changed.
    #[serde(default)]
    pub changed: bool,
}

/// What the server reports to its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A request was handled (or an RA sent).
    Activity(Activity),
    /// This many leases expired.
    Expired(usize),
}

/// Callback receiving [`Event`]s (called from the server tasks).
pub type Hook = Arc<dyn Fn(Event) + Send + Sync>;

/// Packet counters.
#[derive(Debug, Default)]
pub struct Stats {
    rx4: AtomicU64,
    tx4: AtomicU64,
    rx6: AtomicU64,
    tx6: AtomicU64,
    ra: AtomicU64,
}

/// [`Stats`] at one moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct StatsSnapshot {
    /// DHCPv4 datagrams received.
    pub rx4: u64,
    /// DHCPv4 replies sent.
    pub tx4: u64,
    /// DHCPv6 datagrams received.
    pub rx6: u64,
    /// DHCPv6 replies sent.
    pub tx6: u64,
    /// Router advertisements sent.
    pub ra: u64,
}

impl Stats {
    /// The counters.
    #[must_use]
    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            rx4: self.rx4.load(Ordering::Relaxed),
            tx4: self.tx4.load(Ordering::Relaxed),
            rx6: self.rx6.load(Ordering::Relaxed),
            tx6: self.tx6.load(Ordering::Relaxed),
            ra: self.ra.load(Ordering::Relaxed),
        }
    }
}

#[cfg(target_os = "linux")]
fn bump(c: &AtomicU64) {
    c.fetch_add(1, Ordering::Relaxed);
}

#[cfg(target_os = "linux")]
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// A running server; dropping it stops every task.
#[derive(Debug)]
pub struct Server {
    tasks: Vec<tokio::task::JoinHandle<()>>,
    stats: Arc<Stats>,
    server_ip: Option<Ipv4Addr>,
    duid: Vec<u8>,
}

impl Drop for Server {
    fn drop(&mut self) {
        for t in &self.tasks {
            t.abort();
        }
    }
}

impl Server {
    /// Opens the sockets of the enabled parts of `settings` on `iface` and
    /// starts answering. The caller validates `settings` first.
    ///
    /// # Errors
    /// Socket errors (permission: binding ports 67 / 547 needs root or
    /// `CAP_NET_BIND_SERVICE`, `SO_BINDTODEVICE` and the RA socket
    /// `CAP_NET_RAW`), `InvalidInput` when DHCPv4 is on and no server
    /// address is known, `Unsupported` outside Linux.
    pub fn start(
        iface: &Iface,
        settings: &Settings,
        leases: &Arc<Mutex<Leases>>,
        hook: &Hook,
    ) -> std::io::Result<Self> {
        let server_ip = settings
            .v4
            .server_ip
            .or_else(|| iface.ipv4.first().copied());
        if settings.v4.enabled && server_ip.is_none() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "{} has no IPv4 address: assign one or set the server address",
                    iface.name
                ),
            ));
        }
        let duid = crate::v6::duid_ll(iface.mac.unwrap_or([2, 0, 0, 0, 0, 1]));
        let stats = Arc::new(Stats::default());
        let tasks = sys::spawn(iface, settings, server_ip, &duid, leases, &stats, hook)?;
        Ok(Self {
            tasks,
            stats,
            server_ip,
            duid,
        })
    }

    /// Packet counters.
    #[must_use]
    pub fn stats(&self) -> StatsSnapshot {
        self.stats.snapshot()
    }

    /// DHCPv4 server identifier.
    #[must_use]
    pub fn server_ip(&self) -> Option<Ipv4Addr> {
        self.server_ip
    }

    /// DHCPv6 server DUID.
    #[must_use]
    pub fn duid(&self) -> &[u8] {
        &self.duid
    }
}

#[cfg(not(target_os = "linux"))]
mod sys {
    use super::{Hook, Iface, Leases, Settings, Stats};
    use std::net::Ipv4Addr;
    use std::sync::{Arc, Mutex};

    #[allow(clippy::unnecessary_wraps)]
    pub(super) fn spawn(
        _iface: &Iface,
        _settings: &Settings,
        _server_ip: Option<Ipv4Addr>,
        _duid: &[u8],
        _leases: &Arc<Mutex<Leases>>,
        _stats: &Arc<Stats>,
        _hook: &Hook,
    ) -> std::io::Result<Vec<tokio::task::JoinHandle<()>>> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "the DHCP server runs on Linux only",
        ))
    }
}

#[cfg(target_os = "linux")]
mod sys {
    use std::io::Read;
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV6};
    use std::sync::{Arc, Mutex, PoisonError};
    use std::time::{Duration, Instant};

    use socket2::{Domain, Protocol, SockAddr, Socket, Type};
    use tokio::io::unix::AsyncFd;
    use tokio::net::UdpSocket;
    use tokio::task::JoinHandle;

    use super::{
        bump, now_ms, Activity, Event, Family, Hook, Iface, Leases, Settings, Stats, PURGE_SECS,
    };
    use crate::config::{V4Config, V6Config};
    use crate::server4::Dest;
    use crate::{now_secs, ra, server4, server6, v4, v6};

    /// Least time between two solicited advertisements.
    const RA_MIN_GAP: Duration = Duration::from_secs(3);
    /// Delay before resending an advertisement that could not be sent.
    const RA_RETRY: Duration = Duration::from_secs(1);

    fn udp(domain: Domain, iface: &str, addr: SocketAddr) -> std::io::Result<UdpSocket> {
        let s = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;
        if domain == Domain::IPV6 {
            s.set_only_v6(true)?;
        } else {
            s.set_broadcast(true)?;
        }
        s.set_reuse_address(true)?;
        s.bind_device(Some(iface.as_bytes()))?;
        s.set_nonblocking(true)?;
        s.bind(&addr.into())?;
        UdpSocket::from_std(s.into())
    }

    pub(super) fn spawn(
        iface: &Iface,
        settings: &Settings,
        server_ip: Option<Ipv4Addr>,
        duid: &[u8],
        leases: &Arc<Mutex<Leases>>,
        stats: &Arc<Stats>,
        hook: &Hook,
    ) -> std::io::Result<Vec<JoinHandle<()>>> {
        // Open everything first so that a failure leaves nothing running.
        let s4 = match (settings.v4.enabled, server_ip) {
            (true, Some(ip)) => Some((
                udp(
                    Domain::IPV4,
                    &iface.name,
                    SocketAddr::from(([0, 0, 0, 0], v4::SERVER_PORT)),
                )?,
                ip,
            )),
            _ => None,
        };
        let s6 = if settings.v6.enabled {
            let s = udp(
                Domain::IPV6,
                &iface.name,
                SocketAddr::from((Ipv6Addr::UNSPECIFIED, v6::SERVER_PORT)),
            )?;
            s.join_multicast_v6(&v6::ALL_SERVERS, iface.index)?;
            Some(s)
        } else {
            None
        };
        let sra = if settings.v6.ra_enabled {
            let s = Socket::new(Domain::IPV6, Type::RAW, Some(Protocol::ICMPV6))?;
            s.bind_device(Some(iface.name.as_bytes()))?;
            s.set_multicast_if_v6(iface.index)?;
            s.set_multicast_hops_v6(255)?;
            s.set_unicast_hops_v6(255)?;
            s.set_multicast_loop_v6(false)?;
            s.join_multicast_v6(&ra::ALL_ROUTERS, iface.index)?;
            s.set_nonblocking(true)?;
            // SAFETY: the socket owns its descriptor, which is neither
            // replaced nor closed while the AsyncFd holds it.
            Some(unsafe { AsyncFd::register(s) }?)
        } else {
            None
        };
        let mut tasks = Vec::new();
        if let Some((sock, ip)) = s4 {
            tasks.push(tokio::spawn(run_v4(
                sock,
                settings.v4.clone(),
                ip,
                Arc::clone(leases),
                Arc::clone(stats),
                Arc::clone(hook),
            )));
        }
        if let Some(sock) = s6 {
            tasks.push(tokio::spawn(run_v6(
                sock,
                settings.v6.clone(),
                duid.to_vec(),
                Arc::clone(leases),
                Arc::clone(stats),
                Arc::clone(hook),
            )));
        }
        if let Some(fd) = sra {
            tasks.push(tokio::spawn(run_ra(
                fd,
                settings.v6.clone(),
                iface.index,
                ra::build(&settings.v6, iface.mac),
                Arc::clone(stats),
                Arc::clone(hook),
            )));
        }
        tasks.push(tokio::spawn(purge(Arc::clone(leases), Arc::clone(hook))));
        Ok(tasks)
    }

    fn activity(family: Family, request: &str, client: String) -> Activity {
        Activity {
            ts_ms: now_ms(),
            family,
            request: request.to_owned(),
            client,
            reply: None,
            addrs: Vec::new(),
            note: None,
            changed: false,
        }
    }

    async fn run_v4(
        sock: UdpSocket,
        cfg: V4Config,
        server_ip: Ipv4Addr,
        leases: Arc<Mutex<Leases>>,
        stats: Arc<Stats>,
        hook: Hook,
    ) {
        let mut buf = vec![0u8; 4096];
        loop {
            let (n, src) = match sock.recv_from(&mut buf).await {
                Ok(x) => x,
                Err(e) => {
                    tracing::warn!("dhcpv4 receive: {e}");
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    continue;
                }
            };
            bump(&stats.rx4);
            let req = match v4::Message::decode(&buf[..n]) {
                Ok(m) => m,
                Err(e) => {
                    let mut a = activity(Family::V4, "?", src.to_string());
                    a.note = Some(e.to_string());
                    hook(Event::Activity(a));
                    continue;
                }
            };
            let out = {
                let mut l = leases.lock().unwrap_or_else(PoisonError::into_inner);
                server4::handle(&cfg, server_ip, &mut l, now_secs(), &req)
            };
            let mut a = activity(Family::V4, out.request, out.mac);
            a.addrs = out.addr.map(|ip| ip.to_string()).into_iter().collect();
            a.note = out.note;
            a.changed = out.changed;
            if let Some(r) = out.reply {
                let to = match r.dest {
                    Dest::Broadcast => SocketAddr::from((Ipv4Addr::BROADCAST, v4::CLIENT_PORT)),
                    Dest::Unicast(ip) => SocketAddr::from((ip, v4::CLIENT_PORT)),
                    Dest::Relay(ip) => SocketAddr::from((ip, v4::SERVER_PORT)),
                };
                match sock.send_to(&r.msg.encode(), to).await {
                    Ok(_) => {
                        bump(&stats.tx4);
                        a.reply = r.msg.message_type().map(|t| t.name().to_owned());
                    }
                    Err(e) => a.note = Some(format!("send to {to}: {e}")),
                }
            }
            hook(Event::Activity(a));
        }
    }

    async fn run_v6(
        sock: UdpSocket,
        cfg: V6Config,
        duid: Vec<u8>,
        leases: Arc<Mutex<Leases>>,
        stats: Arc<Stats>,
        hook: Hook,
    ) {
        let mut buf = vec![0u8; 4096];
        loop {
            let (n, src) = match sock.recv_from(&mut buf).await {
                Ok(x) => x,
                Err(e) => {
                    tracing::warn!("dhcpv6 receive: {e}");
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    continue;
                }
            };
            bump(&stats.rx6);
            let req = match v6::Message::decode(&buf[..n]) {
                Ok(m) => m,
                Err(e) => {
                    let mut a = activity(Family::V6, "?", src.to_string());
                    a.note = Some(e.to_string());
                    hook(Event::Activity(a));
                    continue;
                }
            };
            let out = {
                let mut l = leases.lock().unwrap_or_else(PoisonError::into_inner);
                server6::handle(&cfg, &duid, &mut l, now_secs(), &req)
            };
            let mut a = activity(Family::V6, out.request, out.client);
            a.addrs = out.addrs;
            a.note = out.note;
            a.changed = out.changed;
            if let Some(r) = out.reply {
                match sock.send_to(&r.encode(), src).await {
                    Ok(_) => {
                        bump(&stats.tx6);
                        a.reply = Some(v6::msg_name(r.msg_type).to_owned());
                    }
                    Err(e) => a.note = Some(format!("send to {src}: {e}")),
                }
            }
            hook(Event::Activity(a));
        }
    }

    async fn run_ra(
        fd: AsyncFd<Socket>,
        cfg: V6Config,
        index: u32,
        packet: Vec<u8>,
        stats: Arc<Stats>,
        hook: Hook,
    ) {
        let to = SockAddr::from(SocketAddrV6::new(ra::ALL_NODES, 0, 0, index));
        let mut tick =
            tokio::time::interval(Duration::from_secs(u64::from(cfg.ra_interval_secs.max(4))));
        let mut last: Option<Instant> = None;
        let mut buf = [0u8; 1500];
        loop {
            let request = tokio::select! {
                _ = tick.tick() => "-",
                r = fd.readable() => {
                    let Ok(mut guard) = r else {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        continue;
                    };
                    match guard.try_io(|s| s.get_ref().read(&mut buf)) {
                        Ok(Ok(n)) if n > 0 && buf[0] == ra::ROUTER_SOLICITATION => {
                            if last.is_some_and(|t| t.elapsed() < RA_MIN_GAP) {
                                continue;
                            }
                            "RS"
                        }
                        _ => continue,
                    }
                }
            };
            last = Some(Instant::now());
            let mut a = activity(Family::Ra, request, String::new());
            match fd.get_ref().send_to(&packet, &to) {
                Ok(_) => {
                    bump(&stats.ra);
                    a.reply = Some("RA".to_owned());
                }
                Err(e) => {
                    // Typically right after the link came up: no link-local
                    // address yet (DAD). Try again soon, not a whole interval later.
                    tick.reset_after(RA_RETRY);
                    a.note = Some(format!("send RA: {e}"));
                }
            }
            hook(Event::Activity(a));
        }
    }

    async fn purge(leases: Arc<Mutex<Leases>>, hook: Hook) {
        let mut tick = tokio::time::interval(Duration::from_secs(PURGE_SECS));
        loop {
            tick.tick().await;
            let n = {
                let mut l = leases.lock().unwrap_or_else(PoisonError::into_inner);
                let before = l.list.len();
                l.purge(now_secs());
                before - l.list.len()
            };
            if n > 0 {
                hook(Event::Expired(n));
            }
        }
    }
}
