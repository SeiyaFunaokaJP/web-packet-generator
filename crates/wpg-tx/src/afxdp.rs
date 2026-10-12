//! `afxdp` backend (Linux 5.10+, feature `afxdp`): one AF_XDP socket bound to one
//! hardware queue, implemented directly on the kernel ABI (`<linux/if_xdp.h>`,
//! `bpf(2)`) without libbpf / libxdp.
//!
//! Per backend instance (= per TX worker):
//!
//! * a **UMEM** (anonymous mapping, `2 × TX ring` chunks of 2 or 4 KiB, at most
//!   16 MiB) registered with `XDP_UMEM_REG`; its pages are placed on the NIC's NUMA
//!   node (`mbind(MPOL_PREFERRED)`, see [`crate::pin::prefer_node`]);
//! * a **TX ring** and a **completion ring** (the fill ring is created because the
//!   kernel requires it, but never used: the socket only transmits);
//! * frames are copied into free chunks, their descriptors published on the TX
//!   ring, and the kernel is kicked with `sendto(MSG_DONTWAIT)` when it asks for it
//!   (`XDP_USE_NEED_WAKEUP`); completed chunks come back through the completion
//!   ring.
//!
//! **Bind mode** ([`XdpMode`]): `auto` first tries zero-copy (`XDP_ZEROCOPY`).
//! Drivers transmit AF_XDP zero-copy frames on their XDP rings, which most of them
//! (i40e, ice, ixgbe, igc, ...) only create while an XDP program is attached, so
//! the backend attaches a two-instruction `XDP_PASS` program in driver mode first
//! (a `bpf_link`, detached when the backend closes; an already attached program is
//! left alone). If the program cannot be loaded, the driver refuses the zero-copy
//! bind, or the first wake-up fails, the backend falls back to copy mode
//! (`XDP_COPY`, the generic path through `dev_direct_xmit`), which works on every
//! interface including `veth` and `lo`. [`AfXdpBackend::zero_copy`] and
//! [`TxBackend::detail`] report what was chosen.
//!
//! Frames longer than a UMEM chunk (2048 B, or 4096 B when the MTU is larger) are
//! counted as dropped; jumbo frames need multi-buffer AF_XDP, which is not used.
//!
//! **Busy polling** ([`TxOptions::busy_poll`], kernel 5.11+):
//! `SO_PREFER_BUSY_POLL`, `SO_BUSY_POLL` and `SO_BUSY_POLL_BUDGET` make each
//! `sendto` run the driver's NAPI poll in the sending thread.
//!
//! Needs `CAP_NET_RAW` (socket), and for zero-copy also `CAP_NET_ADMIN` +
//! `CAP_BPF` (or `CAP_SYS_ADMIN`) for the XDP program; on kernels before 5.11 the
//! UMEM is charged to `RLIMIT_MEMLOCK` (the backend raises the soft limit to the
//! hard limit; `ulimit -l` / `LimitMEMLOCK=infinity`).
//!
//! Accounting: `packets` counts frames whose descriptors were published on the TX
//! ring; `dropped` counts oversized / empty frames and the kernel's
//! `tx_invalid_descs` (`XDP_STATISTICS`).

use std::collections::HashMap;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::{Duration, Instant};

use crate::sys::{cvt, iface_mtu, ifindex, setsockopt, setsockopt_int, socklen};
use crate::{Frame, TxBackend, TxError, TxOptions, TxStats, XdpMode};

// <linux/socket.h>, <linux/if_xdp.h>
const AF_XDP: libc::c_int = 44;
const SOL_XDP: libc::c_int = 283;
const XDP_MMAP_OFFSETS: libc::c_int = 1;
const XDP_TX_RING: libc::c_int = 3;
const XDP_UMEM_REG: libc::c_int = 4;
const XDP_UMEM_FILL_RING: libc::c_int = 5;
const XDP_UMEM_COMPLETION_RING: libc::c_int = 6;
const XDP_STATISTICS: libc::c_int = 7;
const XDP_OPTIONS: libc::c_int = 8;
const XDP_COPY: u16 = 1 << 1;
const XDP_ZEROCOPY: u16 = 1 << 2;
const XDP_USE_NEED_WAKEUP: u16 = 1 << 3;
const XDP_RING_NEED_WAKEUP: u32 = 1 << 0;
const XDP_OPTIONS_ZEROCOPY: u32 = 1 << 0;
const XDP_PGOFF_TX_RING: libc::off_t = 0x8000_0000;
const XDP_UMEM_PGOFF_COMPLETION_RING: libc::off_t = 0x1_8000_0000;
// <asm-generic/socket.h>
const SO_BUSY_POLL: libc::c_int = 46;
const SO_PREFER_BUSY_POLL: libc::c_int = 69;
const SO_BUSY_POLL_BUDGET: libc::c_int = 70;
// <linux/bpf.h>
const BPF_PROG_LOAD: libc::c_int = 5;
const BPF_LINK_CREATE: libc::c_int = 28;
const BPF_PROG_TYPE_XDP: u32 = 6;
const BPF_XDP: u32 = 37;
// <linux/if_link.h>
const XDP_FLAGS_DRV_MODE: u32 = 1 << 2;

/// Upper bound for one socket's UMEM.
const UMEM_MAX_BYTES: usize = 16 << 20;
/// Fill ring entries (unused, but the kernel wants a ring).
const FILL_ENTRIES: u32 = 64;

/// `struct sockaddr_xdp`.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct SockaddrXdp {
    family: u16,
    flags: u16,
    ifindex: u32,
    queue_id: u32,
    shared_umem_fd: u32,
}

/// `struct xdp_umem_reg` (with `flags` and `tx_metadata_len`; older kernels
/// accept the size because it equals their padded struct).
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct UmemReg {
    addr: u64,
    len: u64,
    chunk_size: u32,
    headroom: u32,
    flags: u32,
    tx_metadata_len: u32,
}

/// `struct xdp_ring_offset`.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct RingOffset {
    producer: u64,
    consumer: u64,
    desc: u64,
    flags: u64,
}

/// `struct xdp_mmap_offsets`.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct MmapOffsets {
    rx: RingOffset,
    tx: RingOffset,
    fr: RingOffset,
    cr: RingOffset,
}

/// `struct xdp_desc`.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct XdpDesc {
    addr: u64,
    len: u32,
    options: u32,
}

/// `struct xdp_statistics`.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct XdpStatistics {
    rx_dropped: u64,
    rx_invalid_descs: u64,
    tx_invalid_descs: u64,
    rx_ring_full: u64,
    rx_fill_ring_empty_descs: u64,
    tx_ring_empty_descs: u64,
}

const PERMISSION_HINT: &str = "AF_XDP needs CAP_NET_RAW (zero-copy also CAP_NET_ADMIN and \
     CAP_BPF): run as root or `sudo setcap cap_net_raw,cap_net_admin,cap_bpf,cap_ipc_lock+ep \
     $(command -v wpg)`";

#[allow(clippy::needless_pass_by_value)] // used as `map_err(|e| map_err(.., e))`
fn map_err(ctx: &str, iface: &str, e: io::Error) -> TxError {
    match e.raw_os_error() {
        Some(libc::EPERM | libc::EACCES) => {
            TxError::Permission(format!("{ctx} on `{iface}`: {e}. {PERMISSION_HINT}"))
        }
        Some(libc::ENODEV | libc::ENXIO) => TxError::Interface(iface.to_owned()),
        Some(libc::EBUSY) if ctx.starts_with("bind") => TxError::Config(format!(
            "{ctx} on `{iface}`: {e}: the queue already has an AF_XDP socket \
             (another wpg job or XDP application); choose another one with --queue"
        )),
        Some(libc::EAFNOSUPPORT) => TxError::Unsupported(format!(
            "{ctx} on `{iface}`: {e} (kernel without CONFIG_XDP_SOCKETS, or older than 5.10)"
        )),
        Some(libc::ENOMEM | libc::ENOBUFS) if ctx == "XDP_UMEM_REG" => TxError::Io(io::Error::new(
            e.kind(),
            format!(
                "{ctx} on `{iface}`: {e}; raise the locked-memory limit \
                 (`ulimit -l unlimited`, systemd `LimitMEMLOCK=infinity`) or lower --ring-depth"
            ),
        )),
        _ => TxError::Io(io::Error::new(e.kind(), format!("{ctx} on `{iface}`: {e}"))),
    }
}

fn getsockopt<T: Default>(fd: RawFd, level: libc::c_int, opt: libc::c_int) -> io::Result<T> {
    let mut v = T::default();
    let mut len = socklen::<T>();
    // SAFETY: `v` is a live `T` of `len` bytes; the kernel writes at most `len`.
    cvt(unsafe { libc::getsockopt(fd, level, opt, (&raw mut v).cast(), &raw mut len) })?;
    Ok(v)
}

/// An anonymous shared mapping or a ring mapping, unmapped on drop.
#[derive(Debug)]
struct Mapping {
    base: *mut u8,
    len: usize,
}

// SAFETY: the mapping is owned exclusively by this struct (and the kernel); the
// backend is used from one thread at a time (`&mut self`).
unsafe impl Send for Mapping {}

impl Mapping {
    fn new(len: usize, fd: RawFd, offset: libc::off_t) -> io::Result<Mapping> {
        let (flags, populate) = if fd < 0 {
            (libc::MAP_PRIVATE | libc::MAP_ANONYMOUS, 0)
        } else {
            (libc::MAP_SHARED, libc::MAP_POPULATE)
        };
        // SAFETY: a fresh mapping (anonymous, or a ring the kernel allocated for
        // `fd` at `offset`); nothing else refers to it.
        let base = unsafe {
            libc::mmap(
                ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                flags | populate,
                fd,
                offset,
            )
        };
        if base == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        Ok(Mapping {
            base: base.cast(),
            len,
        })
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: unmaps exactly the region mapped in `new`.
        unsafe {
            libc::munmap(self.base.cast(), self.len);
        }
    }
}

/// A mapped single-producer / single-consumer ring.
#[derive(Debug)]
struct Ring {
    map: Mapping,
    producer: usize,
    consumer: usize,
    flags: usize,
    desc: usize,
    size: u32,
    /// Our cached copy of the index we own (producer for TX, consumer for
    /// completions).
    cached: u32,
}

impl Ring {
    fn map(
        fd: RawFd,
        off: &RingOffset,
        entries: u32,
        elem: usize,
        pgoff: libc::off_t,
    ) -> io::Result<Ring> {
        let desc =
            usize::try_from(off.desc).map_err(|_| io::Error::from_raw_os_error(libc::EINVAL))?;
        let len = desc + entries as usize * elem;
        let map = Mapping::new(len, fd, pgoff)?;
        let field =
            |v: u64| usize::try_from(v).map_err(|_| io::Error::from_raw_os_error(libc::EINVAL));
        Ok(Ring {
            map,
            producer: field(off.producer)?,
            consumer: field(off.consumer)?,
            flags: field(off.flags)?,
            desc,
            size: entries,
            cached: 0,
        })
    }

    fn word(&self, off: usize) -> &AtomicU32 {
        debug_assert!(off + 4 <= self.map.len);
        // SAFETY: the kernel-provided offsets point at 4-byte aligned u32 indices
        // inside the mapping, shared with the kernel; atomic access is the protocol.
        unsafe { &*self.map.base.add(off).cast::<AtomicU32>() }
    }

    fn producer(&self) -> &AtomicU32 {
        self.word(self.producer)
    }

    fn consumer(&self) -> &AtomicU32 {
        self.word(self.consumer)
    }

    fn needs_wakeup(&self) -> bool {
        self.word(self.flags).load(Ordering::Acquire) & XDP_RING_NEED_WAKEUP != 0
    }

    fn entry(&self, idx: u32, elem: usize) -> *mut u8 {
        let i = (idx & (self.size - 1)) as usize;
        // SAFETY: `i < size`, and the descriptor area holds `size` entries.
        unsafe { self.map.base.add(self.desc + i * elem) }
    }
}

/// How the socket ended up bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Bound {
    zero_copy: bool,
    need_wakeup: bool,
}

/// One AF_XDP socket with its UMEM and rings. Fields drop in order: rings, then
/// the socket, then the UMEM, then the XDP program link.
#[derive(Debug)]
struct Xsk {
    tx: Ring,
    comp: Ring,
    fd: OwnedFd,
    umem: Mapping,
    chunk: usize,
    /// Free UMEM chunk addresses.
    free: Vec<u64>,
    /// Descriptors published but not yet completed.
    outstanding: u32,
    bound: Bound,
    busy_poll: bool,
    numa: Option<usize>,
    link: Option<Arc<XdpLink>>,
}

/// An attached `XDP_PASS` program (`bpf_link`), detached on drop.
#[derive(Debug)]
struct XdpLink {
    _link: OwnedFd,
    _prog: OwnedFd,
}

#[repr(C, align(8))]
struct BpfAttr([u8; 128]);

impl BpfAttr {
    fn put_u32(&mut self, off: usize, v: u32) {
        self.0[off..off + 4].copy_from_slice(&v.to_ne_bytes());
    }

    fn put_u64(&mut self, off: usize, v: u64) {
        self.0[off..off + 8].copy_from_slice(&v.to_ne_bytes());
    }
}

fn bpf(cmd: libc::c_int, attr: &mut BpfAttr) -> io::Result<OwnedFd> {
    // SAFETY: bpf(2) reads at most `size_of::<BpfAttr>()` bytes from `attr`; the
    // pointers inside it (instructions, license) outlive the call.
    let rc = unsafe {
        libc::syscall(
            libc::SYS_bpf,
            cmd,
            attr.0.as_mut_ptr(),
            size_of::<BpfAttr>(),
        )
    };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    let fd = RawFd::try_from(rc).map_err(|_| io::Error::from_raw_os_error(libc::EBADF))?;
    // SAFETY: bpf(2) returned a new file descriptor that nobody else owns.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// `r0 = XDP_PASS; exit` as raw eBPF instructions.
const XDP_PASS_PROG: [u64; 2] = [
    // BPF_ALU64 | BPF_MOV | BPF_K, dst r0, imm 2 (XDP_PASS)
    u64::from_le_bytes([0xb7, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00]),
    // BPF_JMP | BPF_EXIT
    u64::from_le_bytes([0x95, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
];

/// The `XDP_PASS` link of `ifindex`, shared by every worker on that interface so
/// it stays attached until the last of them closes. `Ok(None)`: another program
/// is already attached (fine for zero-copy).
fn shared_pass_prog(ifindex: u32) -> io::Result<Option<Arc<XdpLink>>> {
    static LINKS: Mutex<Option<HashMap<u32, Weak<XdpLink>>>> = Mutex::new(None);
    let mut links = LINKS.lock().unwrap_or_else(PoisonError::into_inner);
    let map = links.get_or_insert_with(HashMap::new);
    if let Some(l) = map.get(&ifindex).and_then(Weak::upgrade) {
        return Ok(Some(l));
    }
    let link = attach_pass_prog(ifindex)?.map(Arc::new);
    match &link {
        Some(l) => {
            map.insert(ifindex, Arc::downgrade(l));
        }
        None => {
            map.remove(&ifindex);
        }
    }
    Ok(link)
}

/// Loads the `XDP_PASS` program and attaches it to `ifindex` in driver mode.
/// `Ok(None)`: another program is already attached.
fn attach_pass_prog(ifindex: u32) -> io::Result<Option<XdpLink>> {
    let license = c"GPL";
    let mut a = BpfAttr([0; 128]);
    a.put_u32(0, BPF_PROG_TYPE_XDP);
    a.put_u32(4, 2); // insn_cnt
    a.put_u64(8, XDP_PASS_PROG.as_ptr() as u64);
    a.put_u64(16, license.as_ptr() as u64);
    a.0[48..48 + 12].copy_from_slice(b"wpg_xdp_pass");
    a.put_u32(68, BPF_XDP); // expected_attach_type
    let prog = bpf(BPF_PROG_LOAD, &mut a)?;
    let mut l = BpfAttr([0; 128]);
    #[allow(clippy::cast_sign_loss)] // file descriptors are non-negative
    l.put_u32(0, prog.as_raw_fd() as u32);
    l.put_u32(4, ifindex);
    l.put_u32(8, BPF_XDP);
    l.put_u32(12, XDP_FLAGS_DRV_MODE);
    match bpf(BPF_LINK_CREATE, &mut l) {
        Ok(link) => Ok(Some(XdpLink {
            _link: link,
            _prog: prog,
        })),
        Err(e)
            if e.raw_os_error() == Some(libc::EBUSY) || e.raw_os_error() == Some(libc::EEXIST) =>
        {
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

/// Raises the soft `RLIMIT_MEMLOCK` to the hard limit (UMEM pinning is charged to
/// it before Linux 5.11).
fn raise_memlock() {
    // SAFETY: getrlimit / setrlimit with a valid struct.
    unsafe {
        let mut r: libc::rlimit = std::mem::zeroed();
        if libc::getrlimit(libc::RLIMIT_MEMLOCK, &raw mut r) == 0 && r.rlim_cur < r.rlim_max {
            r.rlim_cur = r.rlim_max;
            libc::setrlimit(libc::RLIMIT_MEMLOCK, &raw const r);
        }
    }
}

/// Socket parameters derived from the options.
#[derive(Debug, Clone, Copy)]
struct Geometry {
    tx_entries: u32,
    chunks: usize,
    chunk: usize,
}

impl Geometry {
    fn new(depth: usize, mtu: Option<usize>) -> Geometry {
        let chunk = if mtu.unwrap_or(1500) + 14 + 4 <= 2048 {
            2048
        } else {
            4096
        };
        // Powers of two: 4096 (2 KiB chunks) or 2048 (4 KiB chunks).
        let max_tx = UMEM_MAX_BYTES / chunk / 2;
        let tx = depth.clamp(64, max_tx).next_power_of_two().min(max_tx);
        #[allow(clippy::cast_possible_truncation)] // <= UMEM_MAX_BYTES / 2048
        Geometry {
            tx_entries: tx as u32,
            chunks: tx * 2,
            chunk,
        }
    }
}

impl Xsk {
    #[allow(clippy::too_many_lines)]
    fn create(
        iface: &str,
        ifindex: u32,
        queue: u32,
        geo: Geometry,
        zero_copy: bool,
        opts: &TxOptions,
    ) -> Result<Xsk, TxError> {
        let e = |ctx: &'static str| move |err: io::Error| map_err(ctx, iface, err);
        // SAFETY: plain socket(2) call.
        let raw = cvt(unsafe { libc::socket(AF_XDP, libc::SOCK_RAW | libc::SOCK_CLOEXEC, 0) })
            .map_err(e("socket(AF_XDP)"))?;
        // SAFETY: `raw` was just returned by socket(2) and is owned by nobody else.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        let sfd = fd.as_raw_fd();

        // UMEM on the NIC's NUMA node.
        let umem_len = geo.chunks * geo.chunk;
        let umem = Mapping::new(umem_len, -1, 0).map_err(e("mmap(UMEM)"))?;
        let numa = crate::pin::iface_numa_node(iface)
            .filter(|&n| crate::pin::prefer_node(umem.base, umem_len, n));
        // Touch every page so it is allocated now (on the preferred node).
        // SAFETY: writes inside the private mapping.
        unsafe { ptr::write_bytes(umem.base, 0, umem_len) };
        raise_memlock();
        let reg = UmemReg {
            addr: umem.base as u64,
            len: umem_len as u64,
            #[allow(clippy::cast_possible_truncation)] // 2048 or 4096
            chunk_size: geo.chunk as u32,
            ..UmemReg::default()
        };
        setsockopt(sfd, SOL_XDP, XDP_UMEM_REG, &reg).map_err(e("XDP_UMEM_REG"))?;
        #[allow(clippy::cast_possible_truncation)] // <= 8192
        let comp_entries = geo.chunks as u32;
        setsockopt(sfd, SOL_XDP, XDP_UMEM_FILL_RING, &FILL_ENTRIES)
            .map_err(e("XDP_UMEM_FILL_RING"))?;
        setsockopt(sfd, SOL_XDP, XDP_UMEM_COMPLETION_RING, &comp_entries)
            .map_err(e("XDP_UMEM_COMPLETION_RING"))?;
        setsockopt(sfd, SOL_XDP, XDP_TX_RING, &geo.tx_entries).map_err(e("XDP_TX_RING"))?;
        let off: MmapOffsets =
            getsockopt(sfd, SOL_XDP, XDP_MMAP_OFFSETS).map_err(e("XDP_MMAP_OFFSETS"))?;
        let tx = Ring::map(
            sfd,
            &off.tx,
            geo.tx_entries,
            size_of::<XdpDesc>(),
            XDP_PGOFF_TX_RING,
        )
        .map_err(e("mmap(TX ring)"))?;
        let comp = Ring::map(
            sfd,
            &off.cr,
            comp_entries,
            size_of::<u64>(),
            XDP_UMEM_PGOFF_COMPLETION_RING,
        )
        .map_err(e("mmap(completion ring)"))?;

        let mut busy_poll = false;
        if opts.busy_poll {
            let budget = libc::c_int::try_from(opts.batch_size.clamp(1, 1024)).unwrap_or(64);
            busy_poll = setsockopt_int(sfd, libc::SOL_SOCKET, SO_PREFER_BUSY_POLL, 1).is_ok()
                && setsockopt_int(sfd, libc::SOL_SOCKET, SO_BUSY_POLL, 20).is_ok()
                && setsockopt_int(sfd, libc::SOL_SOCKET, SO_BUSY_POLL_BUDGET, budget).is_ok();
        }

        let link = if zero_copy {
            match shared_pass_prog(ifindex) {
                Ok(l) => l,
                Err(err) => return Err(map_err("XDP program (zero-copy)", iface, err)),
            }
        } else {
            None
        };
        let mode = if zero_copy { XDP_ZEROCOPY } else { XDP_COPY };
        let mut bound = Bound {
            zero_copy,
            need_wakeup: true,
        };
        let bind = |flags: u16| {
            let sa = SockaddrXdp {
                #[allow(clippy::cast_possible_truncation)] // 44
                family: AF_XDP as u16,
                flags,
                ifindex,
                queue_id: queue,
                shared_umem_fd: 0,
            };
            // A socket that was just closed on this queue releases it from a kernel
            // work queue, so EBUSY right after another job ended is retried briefly.
            let start = Instant::now();
            loop {
                // SAFETY: pointer and length describe `sa`, a `struct sockaddr_xdp`.
                let r = cvt(unsafe {
                    libc::bind(sfd, (&raw const sa).cast(), socklen::<SockaddrXdp>())
                });
                match r {
                    Err(e)
                        if e.raw_os_error() == Some(libc::EBUSY)
                            && start.elapsed() < Duration::from_secs(1) =>
                    {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    r => return r,
                }
            }
        };
        if let Err(err) = bind(mode | XDP_USE_NEED_WAKEUP) {
            if err.raw_os_error() == Some(libc::EINVAL) && !zero_copy {
                // Kernels without need_wakeup support (not expected on 5.10+).
                bind(mode).map_err(e("bind(AF_XDP)"))?;
                bound.need_wakeup = false;
            } else {
                return Err(map_err("bind(AF_XDP)", iface, err));
            }
        }
        if let Ok(o) = getsockopt::<u32>(sfd, SOL_XDP, XDP_OPTIONS) {
            bound.zero_copy = o & XDP_OPTIONS_ZEROCOPY != 0;
        }
        let free = (0..geo.chunks as u64)
            .rev()
            .map(|i| i * geo.chunk as u64)
            .collect();
        let xsk = Xsk {
            tx,
            comp,
            fd,
            umem,
            chunk: geo.chunk,
            free,
            outstanding: 0,
            bound,
            busy_poll,
            numa,
            link,
        };
        if zero_copy {
            // Drivers without XDP rings refuse the first wake-up.
            xsk.wakeup().map_err(e("sendto(AF_XDP wake-up)"))?;
        }
        Ok(xsk)
    }

    /// Kicks the kernel (`sendto(MSG_DONTWAIT)`); "try again" errors are fine.
    fn wakeup(&self) -> io::Result<()> {
        loop {
            // SAFETY: zero-length sendto on our own socket starts transmission.
            let rc = unsafe {
                libc::sendto(
                    self.fd.as_raw_fd(),
                    ptr::null(),
                    0,
                    libc::MSG_DONTWAIT,
                    ptr::null(),
                    0,
                )
            };
            if rc >= 0 {
                return Ok(());
            }
            let err = io::Error::last_os_error();
            match err.raw_os_error() {
                Some(libc::EINTR) => {}
                Some(libc::EAGAIN | libc::EBUSY | libc::ENOBUFS) => return Ok(()),
                _ => return Err(err),
            }
        }
    }

    fn kick(&self) -> io::Result<()> {
        if !self.bound.need_wakeup || self.tx.needs_wakeup() || self.busy_poll {
            self.wakeup()
        } else {
            Ok(())
        }
    }

    /// Returns completed chunks to the free list.
    fn reap(&mut self) -> u32 {
        let prod = self.comp.producer().load(Ordering::Acquire);
        let n = prod.wrapping_sub(self.comp.cached);
        if n == 0 {
            return 0;
        }
        for k in 0..n {
            let idx = self.comp.cached.wrapping_add(k);
            // SAFETY: entries between our consumer index and the producer were
            // written by the kernel (Acquire above) and are 8-byte aligned u64s.
            let addr = unsafe { self.comp.entry(idx, size_of::<u64>()).cast::<u64>().read() };
            self.free.push(addr);
        }
        self.comp.cached = prod;
        self.comp.consumer().store(prod, Ordering::Release);
        self.outstanding = self.outstanding.saturating_sub(n);
        n
    }

    fn tx_free(&self) -> u32 {
        let cons = self.tx.consumer().load(Ordering::Acquire);
        self.tx.size - self.tx.cached.wrapping_sub(cons)
    }

    fn tx_pending(&self) -> bool {
        self.tx.consumer().load(Ordering::Acquire) != self.tx.cached
    }

    fn publish(&self) {
        self.tx.producer().store(self.tx.cached, Ordering::Release);
    }

    /// Waits for a free chunk and a free TX slot.
    fn reserve(&mut self, iface: &str) -> Result<u64, TxError> {
        let start = Instant::now();
        loop {
            if self.free.is_empty() {
                self.reap();
            }
            if !self.free.is_empty() && self.tx_free() > 0 {
                return Ok(self.free.pop().expect("checked"));
            }
            self.publish();
            self.wakeup()
                .map_err(|e| map_err("sendto(AF_XDP)", iface, e))?;
            if self.reap() > 0 {
                continue;
            }
            if start.elapsed() > Duration::from_secs(2) {
                return Err(TxError::Io(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("AF_XDP TX ring on `{iface}` is not draining (link down?)"),
                )));
            }
            if start.elapsed() > Duration::from_micros(200) {
                std::thread::yield_now();
            } else {
                std::hint::spin_loop();
            }
        }
    }

    /// Copy mode transmits at most a few dozen frames per `sendto`: keep kicking
    /// while the kernel makes progress.
    fn drain(&self, iface: &str, budget: Duration) -> Result<(), TxError> {
        let start = Instant::now();
        while self.tx_pending() {
            let before = self.tx.consumer().load(Ordering::Acquire);
            self.wakeup()
                .map_err(|e| map_err("sendto(AF_XDP)", iface, e))?;
            if self.tx.consumer().load(Ordering::Acquire) == before {
                if start.elapsed() > budget {
                    break;
                }
                std::thread::yield_now();
            }
        }
        Ok(())
    }

    fn send(&mut self, iface: &str, frames: &[Frame]) -> Result<(usize, u64), TxError> {
        let mut queued = 0;
        let mut dropped = 0u64;
        for f in frames {
            let data = f.as_bytes();
            if data.is_empty() || data.len() > self.chunk {
                dropped += 1;
                continue;
            }
            let addr = self.reserve(iface)?;
            // SAFETY: `addr` is a free chunk (owned by user space) of `chunk` octets
            // inside the UMEM, and `data.len() <= chunk`.
            unsafe {
                ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    self.umem.base.add(usize::try_from(addr).unwrap_or(0)),
                    data.len(),
                );
            }
            let desc = XdpDesc {
                addr,
                len: u32::try_from(data.len()).unwrap_or(u32::MAX),
                options: 0,
            };
            // SAFETY: the slot at our producer index is free (`tx_free() > 0`) and
            // 16-byte aligned.
            unsafe {
                self.tx
                    .entry(self.tx.cached, size_of::<XdpDesc>())
                    .cast::<XdpDesc>()
                    .write(desc);
            };
            self.tx.cached = self.tx.cached.wrapping_add(1);
            self.outstanding += 1;
            queued += 1;
        }
        self.publish();
        if self.bound.zero_copy {
            self.kick()
                .map_err(|e| map_err("sendto(AF_XDP)", iface, e))?;
        } else {
            self.drain(iface, Duration::from_millis(10))?;
        }
        self.reap();
        Ok((queued, dropped))
    }

    fn invalid_descs(&self) -> u64 {
        getsockopt::<XdpStatistics>(self.fd.as_raw_fd(), SOL_XDP, XDP_STATISTICS)
            .map_or(0, |s| s.tx_invalid_descs)
    }
}

/// AF_XDP transmit backend (one socket on one queue).
#[derive(Debug, Default)]
pub struct AfXdpBackend {
    xsk: Option<Xsk>,
    iface: String,
    queue: u32,
    stats: TxStats,
    /// Why zero-copy was not used (`auto` mode).
    fallback: Option<String>,
}

impl AfXdpBackend {
    /// Creates a closed backend.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the socket is bound in zero-copy mode (`None` while closed).
    #[must_use]
    pub fn zero_copy(&self) -> Option<bool> {
        self.xsk.as_ref().map(|x| x.bound.zero_copy)
    }

    /// Why `auto` mode fell back to copy mode.
    #[must_use]
    pub fn fallback_reason(&self) -> Option<&str> {
        self.fallback.as_deref()
    }

    /// Queue the socket is bound to.
    #[must_use]
    pub fn queue(&self) -> u32 {
        self.queue
    }
}

fn check_queue(iface: &str, queue: u32) -> Result<(), TxError> {
    let q = queue as usize;
    let tx = crate::pin::tx_queue_count(iface);
    let rx = crate::pin::rx_queue_count(iface);
    if let (Some(tx), Some(rx)) = (tx, rx) {
        let n = tx.min(rx);
        if q >= n {
            return Err(TxError::Config(format!(
                "AF_XDP queue {queue} does not exist on `{iface}` ({n} combined queues; \
                 add some with `ethtool -L {iface} combined <N>`)"
            )));
        }
    }
    Ok(())
}

impl TxBackend for AfXdpBackend {
    fn name(&self) -> &'static str {
        "afxdp"
    }

    fn open(&mut self, iface: &str, opts: &TxOptions) -> Result<(), TxError> {
        if self.xsk.is_some() {
            return Err(TxError::AlreadyOpen);
        }
        let idx =
            u32::try_from(ifindex(iface)?).map_err(|_| TxError::Interface(iface.to_owned()))?;
        let queue = opts.queue.unwrap_or(0);
        check_queue(iface, queue)?;
        let geo = Geometry::new(opts.ring_depth.max(1), iface_mtu(iface));
        self.fallback = None;
        let xsk = match opts.xdp_mode {
            XdpMode::Copy => Xsk::create(iface, idx, queue, geo, false, opts)?,
            XdpMode::ZeroCopy => Xsk::create(iface, idx, queue, geo, true, opts)?,
            XdpMode::Auto => match Xsk::create(iface, idx, queue, geo, true, opts) {
                Ok(x) => x,
                // Interface / socket-level problems are not about zero-copy.
                Err(e @ (TxError::Interface(_) | TxError::Config(_) | TxError::Unsupported(_))) => {
                    return Err(e)
                }
                Err(e) => {
                    self.fallback = Some(e.to_string());
                    Xsk::create(iface, idx, queue, geo, false, opts)?
                }
            },
        };
        self.xsk = Some(xsk);
        self.iface = iface.to_owned();
        self.queue = queue;
        self.stats = TxStats::default();
        Ok(())
    }

    fn send_batch(&mut self, frames: &[Frame]) -> Result<usize, TxError> {
        let xsk = self.xsk.as_mut().ok_or(TxError::NotOpen)?;
        self.stats.batches += 1;
        let (sent, dropped) = xsk.send(&self.iface, frames)?;
        self.stats.packets += sent as u64;
        self.stats.dropped += dropped;
        self.stats.bytes += frames
            .iter()
            .filter(|f| !f.is_empty() && f.len() <= xsk.chunk)
            .map(|f| f.len() as u64)
            .sum::<u64>();
        Ok(sent)
    }

    fn service(&mut self) -> Result<(), TxError> {
        if let Some(x) = self.xsk.as_mut() {
            x.reap();
            if x.tx_pending() {
                x.kick()
                    .map_err(|e| map_err("sendto(AF_XDP)", &self.iface, e))?;
            }
        }
        Ok(())
    }

    fn stats(&self) -> TxStats {
        self.stats
    }

    fn detail(&self) -> Option<String> {
        let x = self.xsk.as_ref()?;
        let mut d = format!(
            "AF_XDP {}, queue {}, {} TX slots, UMEM {} x {} B",
            if x.bound.zero_copy {
                "zero-copy"
            } else {
                "copy mode"
            },
            self.queue,
            x.tx.size,
            x.free.capacity(),
            x.chunk,
        );
        if x.link.is_some() {
            d.push_str(", XDP_PASS program attached");
        }
        if x.busy_poll {
            d.push_str(", busy-poll");
        }
        if let Some(n) = x.numa {
            d.push_str(&format!(", NUMA node {n}"));
        }
        if let Some(why) = &self.fallback {
            d.push_str(&format!(", zero-copy unavailable: {why}"));
        }
        Some(d)
    }

    fn close(&mut self) -> Result<(), TxError> {
        if let Some(mut x) = self.xsk.take() {
            // Flush what is still queued and wait (briefly) for completions.
            let start = Instant::now();
            let _ = x.drain(&self.iface, Duration::from_millis(500));
            while x.outstanding > 0 && start.elapsed() < Duration::from_secs(1) {
                if x.reap() == 0 {
                    let _ = x.wakeup();
                    std::thread::yield_now();
                }
            }
            self.stats.dropped += x.invalid_descs();
            drop(x);
        }
        Ok(())
    }
}

impl Drop for AfXdpBackend {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// Opens the backend (registry entry point).
///
/// # Errors
/// See [`AfXdpBackend::open`] ([`TxBackend::open`]).
pub fn open(iface: &str, opts: &TxOptions) -> Result<Box<dyn TxBackend>, TxError> {
    let mut b = AfXdpBackend::new();
    b.open(iface, opts)?;
    Ok(Box::new(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::{synthetic_frame, BENCH_ETHERTYPE};

    #[test]
    fn abi_sizes() {
        assert_eq!(size_of::<SockaddrXdp>(), 16);
        assert_eq!(size_of::<UmemReg>(), 32);
        assert_eq!(size_of::<RingOffset>(), 32);
        assert_eq!(size_of::<MmapOffsets>(), 128);
        assert_eq!(size_of::<XdpDesc>(), 16);
        assert_eq!(size_of::<XdpStatistics>(), 48);
    }

    #[test]
    fn geometry() {
        let g = Geometry::new(4096, Some(1500));
        assert_eq!((g.chunk, g.tx_entries, g.chunks), (2048, 4096, 8192));
        assert!(g.chunks * g.chunk <= UMEM_MAX_BYTES);
        let g = Geometry::new(4096, Some(65536));
        assert_eq!((g.chunk, g.tx_entries, g.chunks), (4096, 2048, 4096));
        let g = Geometry::new(1, None);
        assert_eq!((g.tx_entries, g.chunks), (64, 128));
        let g = Geometry::new(1000, Some(1500));
        assert_eq!(g.tx_entries, 1024);
        assert!(g.tx_entries.is_power_of_two());
    }

    #[test]
    fn missing_queue_is_a_config_error() {
        assert!(
            matches!(check_queue("lo", 4096), Err(TxError::Config(m)) if m.contains("ethtool -L"))
        );
        assert!(check_queue("lo", 0).is_ok());
    }

    #[test]
    fn unknown_interface_and_closed_backend() {
        let mut b = AfXdpBackend::new();
        assert!(matches!(
            b.open("nonexistent0", &TxOptions::default()),
            Err(TxError::Interface(_))
        ));
        assert!(matches!(b.send_batch(&[]), Err(TxError::NotOpen)));
        assert!(b.detail().is_none());
        assert!(b.close().is_ok());
    }

    /// One AF_XDP socket per queue: tests that bind `lo` queue 0 run one at a time.
    static LO: Mutex<()> = Mutex::new(());

    fn lo_lock() -> std::sync::MutexGuard<'static, ()> {
        LO.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Opens on `iface`, or `None` (test skipped) where AF_XDP is not usable.
    fn open_on(iface: &str, opts: &TxOptions) -> Option<AfXdpBackend> {
        let mut b = AfXdpBackend::new();
        match b.open(iface, opts) {
            Ok(()) => Some(b),
            Err(e @ (TxError::Permission(_) | TxError::Unsupported(_))) => {
                eprintln!("skipping AF_XDP test: {e}");
                None
            }
            Err(e) => panic!("open {iface}: {e}"),
        }
    }

    /// AF_PACKET receiver for the benchmark EtherType on `iface`.
    fn receiver(iface: &str) -> OwnedFd {
        let idx = ifindex(iface).unwrap();
        let proto = BENCH_ETHERTYPE.to_be();
        // SAFETY: plain socket(2) call.
        let fd = unsafe {
            OwnedFd::from_raw_fd(
                cvt(libc::socket(
                    libc::AF_PACKET,
                    libc::SOCK_RAW | libc::SOCK_CLOEXEC,
                    libc::c_int::from(proto),
                ))
                .unwrap(),
            )
        };
        // SAFETY: sockaddr_ll is plain data.
        let mut sll: libc::sockaddr_ll = unsafe { std::mem::zeroed() };
        #[allow(clippy::cast_possible_truncation)]
        {
            sll.sll_family = libc::AF_PACKET as u16;
        }
        sll.sll_protocol = proto;
        sll.sll_ifindex = idx;
        // SAFETY: pointer and length describe `sll`.
        cvt(unsafe {
            libc::bind(
                fd.as_raw_fd(),
                (&raw const sll).cast(),
                socklen::<libc::sockaddr_ll>(),
            )
        })
        .unwrap();
        let _ = setsockopt_int(
            fd.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_RCVBUFFORCE,
            8 << 20,
        );
        let tv = libc::timeval {
            tv_sec: 0,
            tv_usec: 200_000,
        };
        let _ = setsockopt(fd.as_raw_fd(), libc::SOL_SOCKET, libc::SO_RCVTIMEO, &tv);
        fd
    }

    fn count_received(fd: &OwnedFd, tag: u8, len: usize) -> usize {
        let mut buf = vec![0u8; 70_000];
        let mut n = 0;
        loop {
            // SAFETY: `buf` is valid for `buf.len()` bytes.
            let rc = unsafe { libc::recv(fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len(), 0) };
            if rc < 0 {
                return n;
            }
            #[allow(clippy::cast_sign_loss)]
            let got = rc as usize;
            if got == len && buf[14] == tag && buf[12..14] == BENCH_ETHERTYPE.to_be_bytes() {
                n += 1;
            }
        }
    }

    fn tagged(tag: u8, len: usize) -> Frame {
        let mut f = synthetic_frame(len);
        f[14] = tag;
        Frame::new(f)
    }

    /// Copy mode on `lo` (generic XDP path): frames must arrive on an AF_PACKET
    /// receiver; auto mode must fall back to copy (lo has no native XDP).
    #[test]
    fn lo_copy_mode_frames_arrive() {
        let _lo = lo_lock();
        for (mode, tag) in [(XdpMode::Copy, 0xb1), (XdpMode::Auto, 0xb2)] {
            let opts = TxOptions {
                xdp_mode: mode,
                ring_depth: 256,
                ..TxOptions::default()
            };
            let Some(mut b) = open_on("lo", &opts) else {
                return;
            };
            assert_eq!(b.zero_copy(), Some(false));
            let d = b.detail().unwrap();
            assert!(d.contains("copy mode") && d.contains("queue 0"), "{d}");
            let rx = receiver("lo");
            // 4 x 300 frames: more than the 256-slot ring, so chunks are recycled.
            let frames: Vec<Frame> = (0..300).map(|_| tagged(tag, 64)).collect();
            let mut sent = 0;
            for _ in 0..4 {
                sent += b.send_batch(&frames).unwrap();
            }
            b.close().unwrap();
            assert_eq!(sent, 1200);
            let st = b.stats();
            assert_eq!((st.packets, st.dropped, st.bytes), (1200, 0, 1200 * 64));
            assert_eq!(count_received(&rx, tag, 64), 1200, "{mode:?}");
        }
    }

    #[test]
    fn oversized_frames_are_dropped() {
        let _lo = lo_lock();
        let opts = TxOptions {
            xdp_mode: XdpMode::Copy,
            ..TxOptions::default()
        };
        let Some(mut b) = open_on("lo", &opts) else {
            return;
        };
        let frames = vec![
            tagged(0xb3, 64),
            Frame::new(vec![0; 5000]),
            Frame::default(),
        ];
        assert_eq!(b.send_batch(&frames).unwrap(), 1);
        b.close().unwrap();
        assert_eq!((b.stats().packets, b.stats().dropped), (1, 2));
    }

    #[test]
    fn job_on_lo_with_busy_poll() {
        let _lo = lo_lock();
        use crate::job::{JobConfig, TxJob};
        use crate::rate::{PacerConfig, Rate, WaitMode};
        use crate::source::PreRendered;
        let opts = TxOptions {
            xdp_mode: XdpMode::Copy,
            busy_poll: true,
            ..TxOptions::default()
        };
        let Some(b) = open_on("lo", &opts) else {
            return;
        };
        let src = PreRendered::new(vec![synthetic_frame(60)], Some(2000));
        let r = TxJob::start(
            Box::new(b),
            Box::new(src),
            JobConfig {
                rate: Rate::Pps(100_000.0),
                pacer: PacerConfig {
                    wait: WaitMode::BusyPoll,
                    ..PacerConfig::default()
                },
                ..JobConfig::default()
            },
        )
        .unwrap()
        .wait();
        assert_eq!(r.stats.packets, 2000, "{r:?}");
        assert_eq!(r.backend.packets, 2000);
        assert!(r.backend_detail.unwrap().contains("AF_XDP"));
    }

    /// Multi-queue check on a veth pair given as `WPG_AFXDP_VETH=<tx>,<rx>` (both
    /// up, at least 2 queues: `ip link add vxa numtxqueues 4 numrxqueues 4 type
    /// veth peer name vxb numtxqueues 4 numrxqueues 4`). Exercises the native
    /// `XDP_PASS` attach, the zero-copy refusal and two workers on queues 0 and 1.
    #[test]
    fn veth_two_queues_from_env() {
        let Ok(pair) = std::env::var("WPG_AFXDP_VETH") else {
            eprintln!("skipping: set WPG_AFXDP_VETH=<tx>,<rx>");
            return;
        };
        let (txi, rxi) = pair.split_once(',').expect("WPG_AFXDP_VETH=<tx>,<rx>");
        let idx = u32::try_from(ifindex(txi).unwrap()).unwrap();
        assert!(
            attach_pass_prog(idx).unwrap().is_some(),
            "native XDP on veth"
        );
        let rx = receiver(rxi);
        let opts = TxOptions::default();
        let mut workers = crate::backend::open_workers("afxdp", txi, &opts, 2).unwrap();
        for (q, w) in workers.iter_mut().enumerate() {
            let d = w.detail().unwrap();
            assert!(
                d.contains(&format!("queue {q}")) && d.contains("copy mode"),
                "{d}"
            );
            let frames: Vec<Frame> = (0..500)
                .map(|_| tagged(0xc0 + u8::try_from(q).unwrap(), 100))
                .collect();
            assert_eq!(w.send_batch(&frames).unwrap(), 500);
            w.close().unwrap();
        }
        let mut buf = vec![0u8; 2048];
        let mut per_tag = [0usize; 2];
        loop {
            // SAFETY: `buf` is valid for `buf.len()` bytes.
            let rc = unsafe { libc::recv(rx.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len(), 0) };
            if rc < 0 {
                break;
            }
            if rc == 100 && (0xc0..0xc2).contains(&buf[14]) {
                per_tag[usize::from(buf[14] - 0xc0)] += 1;
            }
        }
        assert_eq!(per_tag, [500, 500]);
    }
}
