//! `dpdk` backend (Linux, feature `dpdk`): transmits through a DPDK poll-mode driver
//! (ADR-0004), linked with `dpdk-sys` (DPDK 24.11 / 23.11 LTS).
//!
//! **EAL.** DPDK's Environment Abstraction Layer is initialised once per process,
//! by the first `open`, with the arguments of [`TxOptions::eal_args`] (`wpg send
//! --eal-args`, `wpg.toml` `tx.dpdk_eal_args`) plus defaults the user did not give
//! (see [`eal_argv`]): `-l <first allowed CPU>` (only the main lcore; wpg's own TX
//! threads register as non-EAL lcores) and `--in-memory` (no shared runtime files,
//! `--no-shconf` with `--no-huge`, which `--in-memory` does not accept;
//! hugepage files unlinked). `rte_eal_init` pins the calling thread to the main
//! lcore, so it runs on a dedicated `wpg-dpdk-eal` thread that then parks; the
//! caller's thread (a CLI thread or a server runtime worker) keeps its affinity.
//! EAL cannot be initialised again: a failed init, or later different arguments,
//! need a restart of the process.
//!
//! **Ports.** The interface names a DPDK port: its number (`0`), its device name
//! (a PCI address such as `0000:00:04.0`, which EAL probed because the NIC is bound
//! to `vfio-pci` / `uio_pci_generic`), or a virtual device (`net_ring0`,
//! `net_tap0,iface=dtap0`, `net_pcap0,iface=eth0`, ...), which is hot-plugged with
//! `rte_dev_probe` when EAL has not created it yet. One job owns a port at a time.
//!
//! **Workers.** [`open_workers`] configures the port once with `queue + n` TX queues
//! (`queue` = [`TxOptions::queue`], default 0) and returns one backend per worker,
//! worker `i` on TX queue `queue + i`; they share the port, which is stopped when the
//! last of them closes. The port gets no RX queue unless the driver refuses a
//! TX-only configuration (then one, never polled).
//!
//! **Buffers.** One mbuf pool per port on the port's NUMA socket, `2048 + 128`
//! octets per mbuf (frames longer than 2048 octets are counted as dropped; jumbo
//! frames would need chained mbufs). Each frame is copied into an mbuf and handed to
//! `rte_eth_tx_burst`; when the TX ring is full the backend retries for up to 10 ms
//! before counting the rest as dropped. `RTE_ETH_TX_OFFLOAD_MBUF_FAST_FREE` is
//! enabled where the driver offers it (all mbufs come from one pool, refcount 1).
//! The pool outlives the job: a later job reuses it, or gets a larger one when it
//! needs more buffers (mbufs still in a stopped port's descriptor rings return to
//! their pool when the port is reconfigured, so pools are never freed).
//!
//! The TX threads call `rte_thread_register` on their first batch so the mempool's
//! per-lcore cache works for them (and unregister when the thread ends).
//!
//! Needs, on a real NIC: hugepages, the NIC bound to `vfio-pci` (or
//! `uio_pci_generic`) and root or the capabilities vfio needs; see
//! `docs/*/src/operations/dpdk-afxdp.md`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use dpdk_sys as sys;

use crate::{Frame, TxBackend, TxError, TxOptions, TxStats};

/// `RTE_ETH_TX_OFFLOAD_MBUF_FAST_FREE` (`RTE_BIT64(16)`, a macro bindgen cannot
/// evaluate).
const TX_OFFLOAD_MBUF_FAST_FREE: u64 = 1 << 16;
/// `LCORE_ID_ANY`.
const LCORE_ID_ANY: u32 = u32::MAX;
/// `SOCKET_ID_ANY`.
const SOCKET_ID_ANY: i32 = -1;
/// Descriptors of the RX queue some drivers insist on.
const RX_DESC: u16 = 512;
/// Per-lcore mempool cache.
const POOL_CACHE: u32 = 256;
/// Data room of an mbuf (frame octets), without the headroom.
const DATA_ROOM: u16 = 2048;
/// How long `send_batch` retries a full TX ring.
const FULL_RING_RETRY: Duration = Duration::from_millis(10);
/// How long to wait for the link after starting the port.
const LINK_WAIT: Duration = Duration::from_secs(3);

/// The current `rte_errno` as text.
fn errno_text() -> String {
    // SAFETY: rte_errno / rte_strerror work on any thread; rte_strerror returns a
    // NUL-terminated string in static (per-thread) storage.
    unsafe { CStr::from_ptr(sys::rte_strerror(sys::wpg_rte_errno())) }
        .to_string_lossy()
        .into_owned()
}

/// Text of a negative errno returned by an ethdev function.
fn ret_text(ret: i32) -> String {
    std::io::Error::from_raw_os_error(-ret).to_string()
}

/// The linked DPDK version, e.g. `DPDK 24.11.3` (works without EAL).
#[must_use]
pub fn version() -> String {
    // SAFETY: rte_version returns a static NUL-terminated string.
    unsafe { CStr::from_ptr(sys::rte_version()) }
        .to_string_lossy()
        .into_owned()
}

// ---- EAL --------------------------------------------------------------------------

/// The initialised EAL.
#[derive(Debug)]
struct Eal {
    /// The user's arguments ([`TxOptions::eal_args`]) the EAL was started with.
    user_args: Vec<String>,
    /// The full argument vector (with defaults).
    argv: Vec<String>,
}

static EAL: OnceLock<Result<Eal, String>> = OnceLock::new();

/// Whether `args` already selects lcores (`-l`, `-c`, `--lcores`).
fn has_lcore_arg(args: &[String]) -> bool {
    args.iter().any(|a| {
        a == "--lcores"
            || a.starts_with("--lcores=")
            || (a.starts_with("-l") || a.starts_with("-c")) && !a.starts_with("--")
    })
}

/// The full EAL argument vector for the user's `args`: program name, `args`, and
/// the defaults they do not override: `-l <first allowed CPU>` unless lcores are
/// given, `--in-memory` (`--no-shconf` with `--no-huge`) unless a shared-config option (`--in-memory`,
/// `--file-prefix`, `--proc-type`, `--no-shconf`) is given.
#[must_use]
pub fn eal_argv(args: &[String]) -> Vec<String> {
    let mut v = vec!["wpg".to_owned()];
    if !has_lcore_arg(args) {
        let cpu = crate::pin::allowed_cpus().first().copied().unwrap_or(0);
        v.push("-l".to_owned());
        v.push(cpu.to_string());
    }
    let shconf = ["--in-memory", "--file-prefix", "--proc-type", "--no-shconf"];
    if !args.iter().any(|a| {
        shconf
            .iter()
            .any(|o| a == o || a.starts_with(&format!("{o}=")))
    }) {
        // `--no-huge` implies legacy memory, which `--in-memory` refuses.
        let no_huge = args.iter().any(|a| a == "--no-huge");
        v.push(
            if no_huge {
                "--no-shconf"
            } else {
                "--in-memory"
            }
            .to_owned(),
        );
    }
    v.extend(args.iter().cloned());
    v
}

/// Initialises EAL with `args` on the first call; later calls check that `args` is
/// empty or the same.
fn eal(args: &[String]) -> Result<&'static Eal, TxError> {
    let r = EAL.get_or_init(|| init_eal(args));
    match r {
        Ok(e) if args.is_empty() || args == e.user_args.as_slice() => Ok(e),
        Ok(e) => Err(TxError::Config(format!(
            "DPDK EAL is already running with arguments `{}`; EAL can be initialised \
             once per process, restart wpg to use `{}`",
            e.user_args.join(" "),
            args.join(" ")
        ))),
        Err(m) => Err(TxError::Unsupported(format!(
            "DPDK EAL initialisation failed earlier in this process ({m}); fix the \
             EAL arguments / host setup and restart wpg"
        ))),
    }
}

fn init_eal(user: &[String]) -> Result<Eal, String> {
    let argv = eal_argv(user);
    let c_args: Vec<CString> = argv
        .iter()
        .map(|a| CString::new(a.as_str()).map_err(|_| format!("EAL argument `{a}` has a NUL")))
        .collect::<Result<_, _>>()?;
    let (tx, rx) = mpsc::channel::<Result<(), String>>();
    std::thread::Builder::new()
        .name("wpg-dpdk-eal".into())
        .spawn(move || {
            // EAL keeps pointers into argv for the life of the process.
            let c_args: &'static [CString] = Box::leak(c_args.into_boxed_slice());
            let ptrs: Vec<*mut c_char> = c_args.iter().map(|a| a.as_ptr().cast_mut()).collect();
            let ptrs: &'static mut [*mut c_char] = Box::leak(ptrs.into_boxed_slice());
            let argc = i32::try_from(ptrs.len()).unwrap_or(i32::MAX);
            // SAFETY: argv holds argc valid NUL-terminated strings that live for the
            // rest of the process (leaked above).
            let ret = unsafe { sys::rte_eal_init(argc, ptrs.as_mut_ptr()) };
            if ret < 0 {
                let _ = tx.send(Err(errno_text()));
                return;
            }
            let _ = tx.send(Ok(()));
            // This thread is the main lcore: keep it (idle) for the life of the process.
            loop {
                std::thread::park();
            }
        })
        .map_err(|e| format!("spawning the EAL thread: {e}"))?;
    match rx.recv() {
        Ok(Ok(())) => Ok(Eal {
            user_args: user.to_vec(),
            argv,
        }),
        Ok(Err(e)) => Err(format!(
            "rte_eal_init({}): {e}. Check hugepages (`wpg doctor`), privileges (root or \
             vfio access) and the EAL arguments (--eal-args / tx.dpdk_eal_args)",
            argv.join(" ")
        )),
        Err(_) => Err("the EAL thread ended without a result".to_owned()),
    }
}

/// Registers the calling thread as a non-EAL lcore (once per thread) so mempool
/// caches work; unregisters when the thread ends.
fn register_thread() {
    struct Registered;
    impl Drop for Registered {
        fn drop(&mut self) {
            // SAFETY: called on the thread that registered.
            unsafe { sys::rte_thread_unregister() };
        }
    }
    thread_local! {
        static REG: RefCell<Option<Option<Registered>>> = const { RefCell::new(None) };
    }
    REG.with(|r| {
        let mut r = r.borrow_mut();
        if r.is_none() {
            // SAFETY: EAL is initialised (only called by an open backend).
            let reg = unsafe {
                sys::wpg_rte_lcore_id() == LCORE_ID_ANY && sys::rte_thread_register() == 0
            };
            *r = Some(reg.then_some(Registered));
        }
    });
}

// ---- Ports ------------------------------------------------------------------------

/// An mbuf pool. Pools are never freed (see the module docs).
#[derive(Debug, Clone, Copy)]
struct Pool {
    ptr: *mut sys::rte_mempool,
    size: u32,
}

// SAFETY: a DPDK mempool is a thread-safe shared object.
unsafe impl Send for Pool {}
// SAFETY: as above.
unsafe impl Sync for Pool {}

#[derive(Debug, Default)]
struct PortSlot {
    busy: bool,
    pool: Option<Pool>,
}

fn ports() -> std::sync::MutexGuard<'static, HashMap<u16, PortSlot>> {
    static PORTS: OnceLock<Mutex<HashMap<u16, PortSlot>>> = OnceLock::new();
    PORTS
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// A started port, shared by the workers; stopped when the last one drops it.
#[derive(Debug)]
struct Port {
    id: u16,
    name: String,
    driver: String,
    socket: i32,
    pool: Pool,
    tx_queues: u16,
    tx_desc: u16,
    fast_free: bool,
    link: String,
}

impl Drop for Port {
    fn drop(&mut self) {
        // Give the NIC a moment to put the frames still in its TX rings on the wire
        // (4096 descriptors of 1518 octets take 5 ms at 10 Gbit/s).
        std::thread::sleep(Duration::from_millis(20));
        // SAFETY: the port was started by `open_workers`; nobody transmits on it any
        // more (every backend holding it is gone).
        unsafe {
            sys::rte_eth_dev_stop(self.id);
        }
        if let Some(s) = ports().get_mut(&self.id) {
            s.busy = false;
        }
    }
}

fn port_name(id: u16) -> String {
    let mut buf = [0 as c_char; 64];
    // SAFETY: the buffer is RTE_ETH_NAME_MAX_LEN (64) bytes, as the API requires.
    if unsafe { sys::rte_eth_dev_get_name_by_port(id, buf.as_mut_ptr()) } != 0 {
        return String::new();
    }
    // SAFETY: NUL-terminated by DPDK.
    unsafe { CStr::from_ptr(buf.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

/// The available ports (number and name) for error messages.
fn list_ports() -> String {
    let mut v = Vec::new();
    // SAFETY: rte_eth_find_next iterates valid ports after EAL init.
    let mut p = unsafe { sys::rte_eth_find_next(0) };
    while u32::from(p) < sys::RTE_MAX_ETHPORTS {
        v.push(format!("{p} `{}`", port_name(p)));
        // SAFETY: as above.
        p = unsafe { sys::rte_eth_find_next(p + 1) };
    }
    if v.is_empty() {
        "none".to_owned()
    } else {
        v.join(", ")
    }
}

fn lookup_port(name: &str) -> Option<u16> {
    let c = CString::new(name).ok()?;
    let mut id = 0u16;
    // SAFETY: valid C string and out pointer.
    (unsafe { sys::rte_eth_dev_get_port_by_name(c.as_ptr(), &raw mut id) } == 0).then_some(id)
}

/// Resolves `iface` (port number, device name, or virtual device with devargs) to a
/// port, hot-plugging virtual devices.
fn resolve_port(iface: &str) -> Result<u16, TxError> {
    if let Ok(id) = iface.parse::<u16>() {
        // SAFETY: plain query.
        if unsafe { sys::rte_eth_dev_is_valid_port(id) } == 1 {
            return Ok(id);
        }
    }
    let name = iface.split(',').next().unwrap_or(iface);
    if let Some(id) = lookup_port(name) {
        return Ok(id);
    }
    if name.starts_with("net_") {
        let c = CString::new(iface).map_err(|_| TxError::Interface(iface.to_owned()))?;
        // SAFETY: valid devargs string.
        let ret = unsafe { sys::rte_dev_probe(c.as_ptr()) };
        if ret != 0 {
            return Err(TxError::Config(format!(
                "DPDK: adding virtual device `{iface}` failed: {}",
                ret_text(ret)
            )));
        }
        if let Some(id) = lookup_port(name) {
            return Ok(id);
        }
    }
    Err(TxError::Config(format!(
        "DPDK port `{iface}` not found (ports: {}). Give a port number, a PCI address \
         of a NIC bound to vfio-pci / uio_pci_generic (dpdk-devbind.py --status; \
         EAL `-a <pci>` limits probing) or a virtual device such as `net_tap0,iface=dtap0`",
        list_ports()
    )))
}

fn link_text(id: u16, wait: Duration) -> String {
    let start = Instant::now();
    loop {
        let mut link = sys::rte_eth_link::default();
        let mut buf = [0 as c_char; 128];
        // SAFETY: valid port and out pointers; the buffer length is passed.
        let text = unsafe {
            if sys::rte_eth_link_get_nowait(id, &raw mut link) != 0
                || sys::rte_eth_link_to_str(buf.as_mut_ptr(), buf.len(), &raw const link) < 0
            {
                return "link unknown".to_owned();
            }
            CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned()
        };
        if text.starts_with("Link up") || start.elapsed() >= wait {
            return text.replacen("Link", "link", 1);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Configures and starts the port behind `iface` with `queue + n` TX queues.
fn start_port(iface: &str, opts: &TxOptions, n: u16) -> Result<Arc<Port>, TxError> {
    let id = resolve_port(iface)?;
    let mut slots = ports();
    let slot = slots.entry(id).or_default();
    if slot.busy {
        return Err(TxError::Config(format!(
            "DPDK port {id} (`{iface}`) is already used by another job"
        )));
    }

    let mut info = sys::rte_eth_dev_info::default();
    // SAFETY: valid port and out pointer.
    let ret = unsafe { sys::rte_eth_dev_info_get(id, &raw mut info) };
    if ret != 0 {
        return Err(TxError::Config(format!(
            "DPDK port {id}: rte_eth_dev_info_get: {}",
            ret_text(ret)
        )));
    }
    let driver = if info.driver_name.is_null() {
        String::new()
    } else {
        // SAFETY: the driver name is a static NUL-terminated string.
        unsafe { CStr::from_ptr(info.driver_name) }
            .to_string_lossy()
            .into_owned()
    };
    let base = u16::try_from(opts.queue.unwrap_or(0)).unwrap_or(u16::MAX);
    let nq = base.saturating_add(n);
    if nq > info.max_tx_queues {
        return Err(TxError::Config(format!(
            "DPDK port {id} ({driver}) has {} TX queues; {n} worker(s) from queue {base} \
             need {nq}",
            info.max_tx_queues
        )));
    }

    let mut conf = sys::rte_eth_conf::default();
    let fast_free = info.tx_offload_capa & TX_OFFLOAD_MBUF_FAST_FREE != 0;
    if fast_free {
        conf.txmode.offloads |= TX_OFFLOAD_MBUF_FAST_FREE;
    }
    // TX only; some drivers need at least one RX queue.
    let mut rxq = 0u16;
    // SAFETY: valid port and configuration.
    let mut ret = unsafe { sys::rte_eth_dev_configure(id, 0, nq, &raw const conf) };
    if ret != 0 {
        rxq = 1;
        // SAFETY: as above.
        ret = unsafe { sys::rte_eth_dev_configure(id, 1, nq, &raw const conf) };
    }
    if ret != 0 {
        return Err(TxError::Config(format!(
            "DPDK port {id} ({driver}): rte_eth_dev_configure({rxq} RX, {nq} TX queues): {}",
            ret_text(ret)
        )));
    }
    let mut rx_desc = RX_DESC;
    let mut tx_desc = u16::try_from(opts.ring_depth.max(1)).unwrap_or(u16::MAX);
    // SAFETY: valid port and out pointers.
    unsafe { sys::rte_eth_dev_adjust_nb_rx_tx_desc(id, &raw mut rx_desc, &raw mut tx_desc) };

    // SAFETY: plain query.
    let socket = unsafe { sys::rte_eth_dev_socket_id(id) };
    let batch = u32::try_from(opts.batch_size).unwrap_or(u32::MAX).min(4096);
    let need = u32::from(nq) * (u32::from(tx_desc) + 2 * batch)
        + u32::from(rxq) * u32::from(rx_desc)
        + 2 * POOL_CACHE * (u32::from(n) + 1)
        + 1024;
    let pool = match slot.pool {
        Some(p) if p.size >= need => p,
        _ => {
            static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let size = need.next_power_of_two() - 1;
            let name = CString::new(format!("wpg_tx{id}_{seq}")).unwrap_or_default();
            let room = DATA_ROOM + u16::try_from(sys::RTE_PKTMBUF_HEADROOM).unwrap_or(128);
            // SAFETY: valid name; DPDK validates the sizes.
            let ptr = unsafe {
                sys::rte_pktmbuf_pool_create(
                    name.as_ptr(),
                    size,
                    POOL_CACHE,
                    0,
                    room,
                    if socket < 0 { SOCKET_ID_ANY } else { socket },
                )
            };
            if ptr.is_null() {
                return Err(TxError::Config(format!(
                    "DPDK: creating a pool of {size} mbufs on socket {socket}: {} \
                     (not enough hugepage memory? see `wpg doctor`, or lower --ring-depth)",
                    errno_text()
                )));
            }
            let p = Pool { ptr, size };
            slot.pool = Some(p);
            p
        }
    };

    // `socket_id` is unsigned in the API; SOCKET_ID_ANY (-1) wraps as in C.
    #[allow(clippy::cast_sign_loss)]
    let qsocket = socket as u32;
    let mut txconf = info.default_txconf;
    txconf.offloads = conf.txmode.offloads;
    for q in 0..nq {
        // SAFETY: valid port, queue index below the configured count, and txconf.
        let ret =
            unsafe { sys::rte_eth_tx_queue_setup(id, q, tx_desc, qsocket, &raw const txconf) };
        if ret != 0 {
            return Err(TxError::Config(format!(
                "DPDK port {id} ({driver}): rte_eth_tx_queue_setup({q}, {tx_desc} descriptors): {}",
                ret_text(ret)
            )));
        }
    }
    if rxq == 1 {
        // SAFETY: valid port, queue 0 configured, pool alive for the process.
        let ret =
            unsafe { sys::rte_eth_rx_queue_setup(id, 0, rx_desc, qsocket, ptr::null(), pool.ptr) };
        if ret != 0 {
            return Err(TxError::Config(format!(
                "DPDK port {id} ({driver}): rte_eth_rx_queue_setup: {}",
                ret_text(ret)
            )));
        }
    }
    // SAFETY: configured port with all queues set up.
    let ret = unsafe { sys::rte_eth_dev_start(id) };
    if ret != 0 {
        return Err(TxError::Config(format!(
            "DPDK port {id} ({driver}): rte_eth_dev_start: {}",
            ret_text(ret)
        )));
    }
    slot.busy = true;
    drop(slots);
    Ok(Arc::new(Port {
        id,
        name: port_name(id),
        driver,
        socket,
        pool,
        tx_queues: nq,
        tx_desc,
        fast_free,
        link: link_text(id, LINK_WAIT),
    }))
}

// ---- Backend ----------------------------------------------------------------------

#[derive(Debug)]
struct Inner {
    port: Arc<Port>,
    queue: u16,
    mbufs: Vec<*mut sys::rte_mbuf>,
    lens: Vec<u64>,
}

// SAFETY: the mbuf pointers are scratch space, empty between calls; the port is
// shared through an Arc and DPDK allows one thread per TX queue.
unsafe impl Send for Inner {}

/// DPDK transmit backend: one TX queue of a DPDK port.
#[derive(Debug, Default)]
pub struct DpdkBackend {
    inner: Option<Inner>,
    stats: TxStats,
}

impl DpdkBackend {
    /// A closed backend.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The DPDK port id (once open).
    #[must_use]
    pub fn port_id(&self) -> Option<u16> {
        self.inner.as_ref().map(|i| i.port.id)
    }

    /// The TX queue (once open).
    #[must_use]
    pub fn queue(&self) -> Option<u16> {
        self.inner.as_ref().map(|i| i.queue)
    }
}

/// Opens `n` backends on the DPDK port `iface`, worker `i` on TX queue
/// `opts.queue + i` (see the module docs).
///
/// # Errors
/// [`TxError::Unsupported`] when EAL cannot be initialised; [`TxError::Config`] for an
/// unknown or busy port, too few queues, or a failed port setup.
pub fn open_workers(
    iface: &str,
    opts: &TxOptions,
    n: usize,
) -> Result<Vec<Box<dyn TxBackend>>, TxError> {
    eal(&opts.eal_args)?;
    let n = u16::try_from(n.max(1))
        .map_err(|_| TxError::Config(format!("too many DPDK workers ({n})")))?;
    let port = start_port(iface, opts, n)?;
    let base = port.tx_queues - n;
    let batch = opts.batch_size.max(1);
    Ok((0..n)
        .map(|i| {
            Box::new(DpdkBackend {
                inner: Some(Inner {
                    port: Arc::clone(&port),
                    queue: base + i,
                    mbufs: Vec::with_capacity(batch),
                    lens: Vec::with_capacity(batch),
                }),
                stats: TxStats::default(),
            }) as Box<dyn TxBackend>
        })
        .collect())
}

/// Opens one backend (registry entry point).
///
/// # Errors
/// See [`open_workers`].
pub fn open(iface: &str, opts: &TxOptions) -> Result<Box<dyn TxBackend>, TxError> {
    let mut b = DpdkBackend::new();
    b.open(iface, opts)?;
    Ok(Box::new(b))
}

impl TxBackend for DpdkBackend {
    fn name(&self) -> &'static str {
        "dpdk"
    }

    fn open(&mut self, iface: &str, opts: &TxOptions) -> Result<(), TxError> {
        if self.inner.is_some() {
            return Err(TxError::AlreadyOpen);
        }
        eal(&opts.eal_args)?;
        let port = start_port(iface, opts, 1)?;
        let batch = opts.batch_size.max(1);
        self.inner = Some(Inner {
            queue: port.tx_queues - 1,
            port,
            mbufs: Vec::with_capacity(batch),
            lens: Vec::with_capacity(batch),
        });
        self.stats = TxStats::default();
        Ok(())
    }

    fn send_batch(&mut self, frames: &[Frame]) -> Result<usize, TxError> {
        let inner = self.inner.as_mut().ok_or(TxError::NotOpen)?;
        register_thread();
        self.stats.batches += 1;
        let (port, queue, pool) = (inner.port.id, inner.queue, inner.port.pool.ptr);
        inner.mbufs.clear();
        inner.lens.clear();
        let mut dropped = 0u64;
        for f in frames {
            let data = f.as_bytes();
            let Ok(len) = u16::try_from(data.len()) else {
                dropped += 1;
                continue;
            };
            if len == 0 || len > DATA_ROOM {
                dropped += 1;
                continue;
            }
            // SAFETY: the pool lives for the process.
            let mut m = unsafe { sys::wpg_rte_pktmbuf_alloc(pool) };
            if m.is_null() {
                // Buffers still sit in the TX ring; ask the driver to free sent ones.
                // SAFETY: our queue on a started port.
                unsafe { sys::rte_eth_tx_done_cleanup(port, queue, 0) };
                // SAFETY: as above.
                m = unsafe { sys::wpg_rte_pktmbuf_alloc(pool) };
                if m.is_null() {
                    dropped += 1;
                    continue;
                }
            }
            // SAFETY: `m` is a fresh mbuf; `data` is `len` readable octets and
            // `len <= DATA_ROOM`, the mbuf's data room.
            unsafe { sys::wpg_rte_pktmbuf_copy_in(m, data.as_ptr().cast(), len) };
            inner.mbufs.push(m);
            inner.lens.push(u64::from(len));
        }

        let total = inner.mbufs.len();
        let mut sent = 0usize;
        let mut deadline: Option<Instant> = None;
        while sent < total {
            let left = u16::try_from(total - sent).unwrap_or(u16::MAX);
            // SAFETY: the slice holds `left` valid mbufs from `sent` on; tx_burst takes
            // ownership of the ones it accepts. One thread per queue.
            let k = usize::from(unsafe {
                sys::wpg_rte_eth_tx_burst(port, queue, inner.mbufs.as_mut_ptr().add(sent), left)
            });
            sent += k;
            if k == 0 {
                let d = *deadline.get_or_insert_with(|| Instant::now() + FULL_RING_RETRY);
                if Instant::now() >= d {
                    break;
                }
                std::hint::spin_loop();
            } else {
                deadline = None;
            }
        }
        for &m in &inner.mbufs[sent..] {
            // SAFETY: not accepted by the driver, still ours.
            unsafe { sys::wpg_rte_pktmbuf_free(m) };
        }
        dropped += (total - sent) as u64;
        self.stats.packets += sent as u64;
        self.stats.bytes += inner.lens[..sent].iter().sum::<u64>();
        self.stats.dropped += dropped;
        inner.mbufs.clear();
        Ok(sent)
    }

    fn stats(&self) -> TxStats {
        self.stats
    }

    fn detail(&self) -> Option<String> {
        let i = self.inner.as_ref()?;
        let p = &i.port;
        let mut d = format!(
            "{}, port {} `{}` ({}), TX queue {} of {}, {} descriptors, pool {} x {} B",
            version(),
            p.id,
            p.name,
            p.driver,
            i.queue,
            p.tx_queues,
            p.tx_desc,
            p.pool.size,
            DATA_ROOM,
        );
        if p.socket >= 0 {
            d.push_str(&format!(", socket {}", p.socket));
        }
        if p.fast_free {
            d.push_str(", mbuf fast free");
        }
        d.push_str(&format!(", {}", p.link));
        if let Some(Ok(e)) = EAL.get() {
            d.push_str(&format!(", EAL `{}`", e.argv[1..].join(" ")));
        }
        Some(d)
    }

    fn close(&mut self) -> Result<(), TxError> {
        self.inner = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|&a| a.to_owned()).collect()
    }

    #[test]
    fn eal_defaults() {
        let v = eal_argv(&[]);
        assert_eq!(v[0], "wpg");
        assert_eq!(v[1], "-l");
        assert!(v.contains(&"--in-memory".to_owned()));

        let v = eal_argv(&s(&[
            "-l",
            "2-3",
            "--file-prefix=wpg",
            "-a",
            "0000:00:04.0",
        ]));
        assert_eq!(
            v,
            s(&[
                "wpg",
                "-l",
                "2-3",
                "--file-prefix=wpg",
                "-a",
                "0000:00:04.0"
            ])
        );
        let v = eal_argv(&s(&["--lcores=1", "--no-huge"]));
        assert_eq!(v, s(&["wpg", "--no-shconf", "--lcores=1", "--no-huge"]));
        assert!(has_lcore_arg(&s(&["-c0x3"])));
        assert!(!has_lcore_arg(&s(&["--log-level=8"])));
    }

    /// Records of a classic pcap file.
    fn pcap_records(path: &std::path::Path) -> Vec<Vec<u8>> {
        let b = std::fs::read(path).unwrap();
        let mut v = Vec::new();
        let mut off = 24;
        while off + 16 <= b.len() {
            let len = u32::from_le_bytes(b[off + 8..off + 12].try_into().unwrap()) as usize;
            v.push(b[off + 16..off + 16 + len].to_vec());
            off += 16 + len;
        }
        v
    }

    /// EAL is per process, so everything that needs it is in this one test: EAL
    /// without hugepages or PCI, a `net_ring` port created by EAL, a hot-plugged
    /// `net_pcap` port writing a pcap file to check the bytes, and `net_null` ports
    /// for workers.
    #[test]
    fn virtual_ports() {
        let args = s(&[
            "--no-huge",
            "-m",
            "256",
            "--no-pci",
            "--no-telemetry",
            "--vdev=net_ring0",
        ]);
        let opts = TxOptions {
            batch_size: 8,
            ring_depth: 256,
            eal_args: args.clone(),
            ..TxOptions::default()
        };

        // net_ring0, created by EAL from --vdev.
        let mut b = DpdkBackend::new();
        b.open("net_ring0", &opts).unwrap();
        assert!(matches!(
            b.open("net_ring0", &opts),
            Err(TxError::AlreadyOpen)
        ));
        let detail = b.detail().unwrap();
        assert!(detail.contains("`net_ring0` (net_ring)"), "{detail}");
        assert!(detail.contains("--no-shconf --no-huge"), "{detail}");
        let port = b.port_id().unwrap();
        // The port is busy while a job holds it.
        assert!(matches!(open("net_ring0", &opts), Err(TxError::Config(_))));
        assert!(matches!(
            open(&port.to_string(), &opts),
            Err(TxError::Config(_))
        ));

        let frames: Vec<Frame> = (0..3u8)
            .map(|i| Frame::new((0..60).map(|j| j ^ i).collect()))
            .chain([Frame::new(vec![0; 3000]), Frame::new(Vec::new())])
            .collect();
        assert_eq!(b.send_batch(&frames).unwrap(), 3);
        let st = b.stats();
        assert_eq!(
            (st.packets, st.bytes, st.dropped, st.batches),
            (3, 180, 2, 1)
        );
        b.close().unwrap();
        assert!(matches!(b.send_batch(&frames), Err(TxError::NotOpen)));
        // Closed: the port can be opened again.
        open("net_ring0", &opts).unwrap();

        // net_pcap0 (hot-plugged with its devargs) writes what it sends to a file.
        let dir = std::env::temp_dir().join(format!("wpg-dpdk-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("tx.pcap");
        let mut p = open(&format!("net_pcap0,tx_pcap={}", file.display()), &opts).unwrap();
        assert_eq!(p.send_batch(&frames).unwrap(), 3);
        p.close().unwrap();
        let got = pcap_records(&file);
        let want: Vec<Vec<u8>> = frames[..3].iter().map(|f| f.as_bytes().to_vec()).collect();
        assert_eq!(got, want);
        // The hot-plugged port is found by name from now on.
        drop(open("net_pcap0", &opts).unwrap());
        let _ = std::fs::remove_dir_all(&dir);

        // Different EAL arguments are refused, none are fine.
        let other = TxOptions {
            eal_args: s(&["--no-huge"]),
            ..opts.clone()
        };
        assert!(matches!(open("net_ring0", &other), Err(TxError::Config(_))));
        let none = TxOptions {
            eal_args: Vec::new(),
            ..opts.clone()
        };

        // net_null0 is hot-plugged; three workers on queues 1..=3.
        let q = TxOptions {
            queue: Some(1),
            ..none.clone()
        };
        let mut ws = open_workers("net_null0", &q, 3).unwrap();
        let handles: Vec<_> = ws
            .drain(..)
            .map(|mut w| {
                std::thread::spawn(move || {
                    let f = vec![Frame::new(vec![0xab; 64]); 8];
                    for _ in 0..100 {
                        assert_eq!(w.send_batch(&f).unwrap(), 8);
                    }
                    (w.stats(), w.detail().unwrap())
                })
            })
            .collect();
        let mut queues = Vec::new();
        for h in handles {
            let (st, d) = h.join().unwrap();
            assert_eq!((st.packets, st.dropped), (800, 0));
            assert!(d.contains("(net_null)") && d.contains("of 4"), "{d}");
            queues.push(d);
        }
        for qn in 1..=3 {
            assert!(
                queues
                    .iter()
                    .any(|d| d.contains(&format!("TX queue {qn} of"))),
                "{queues:?}"
            );
        }

        // Unknown ports name the existing ones.
        match open("0000:99:00.0", &none) {
            Err(TxError::Config(m)) => assert!(m.contains("net_ring0"), "{m}"),
            other => panic!("{:?}", other.map(|b| b.name())),
        }
        assert!(matches!(
            open_workers(
                "net_null1",
                &TxOptions {
                    queue: Some(60_000),
                    ..none
                },
                1
            ),
            Err(TxError::Config(_))
        ));
    }
}
