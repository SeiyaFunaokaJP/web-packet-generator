//! `afpacket` backend (Linux): `AF_PACKET` / `SOCK_RAW` sockets bound to an
//! interface index.
//!
//! Two transmit paths:
//!
//! * **TPACKET_V3 TX ring** (default, [`TxOptions::tx_ring`]): `PACKET_TX_RING` is
//!   mapped into user space; frames are copied into ring slots, marked
//!   `TP_STATUS_SEND_REQUEST`, and one `send(fd, NULL, 0, MSG_DONTWAIT)` per batch
//!   makes the kernel transmit all of them. Frames larger than a ring slot go out
//!   through a second, plain socket.
//! * **`sendmmsg`**: one system call per batch. Used when the ring is disabled or
//!   cannot be set up (old kernel, memory limits); the fallback is automatic and
//!   visible through [`AfPacketBackend::mode`].
//!
//! The socket is created with protocol 0, so it never receives packets (no wasted
//! copies). `PACKET_QDISC_BYPASS` is enabled with [`TxOptions::qdisc_bypass`].
//!
//! Needs `CAP_NET_RAW`; `EPERM` / `EACCES` become [`TxError::Permission`] with a
//! `setcap` hint.
//!
//! Accounting: `packets` counts frames handed to the kernel (queued in the ring or
//! accepted by `sendmmsg`); frames the kernel later rejects (`TP_STATUS_WRONG_FORMAT`,
//! `EMSGSIZE`) or that cannot be queued (`ENOBUFS` persisting) are counted in
//! `dropped`.

use std::ffi::CString;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use crate::{Frame, TxBackend, TxError, TxOptions, TxStats};

// Constants from <linux/if_packet.h> (defined here so the backend does not depend on
// which of them a given `libc` version exports).
const PACKET_VERSION: libc::c_int = 10;
const PACKET_TX_RING: libc::c_int = 13;
const PACKET_QDISC_BYPASS: libc::c_int = 20;
const TPACKET_V3: libc::c_int = 2;
const TP_STATUS_AVAILABLE: u32 = 0;
const TP_STATUS_SEND_REQUEST: u32 = 1;
const TP_STATUS_WRONG_FORMAT: u32 = 4;
/// `sizeof(struct tpacket3_hdr)` aligned to `TPACKET_ALIGNMENT` (16): 48. For TX the
/// frame data starts at `tp_hdrlen - sizeof(struct sockaddr_ll)` = 48.
const TPACKET3_HDR: usize = 48;
const TP_LEN_OFF: usize = 16;
const TP_STATUS_OFF: usize = 20;
/// Upper bound for the ring's memory.
const RING_MAX_BYTES: usize = 16 << 20;

/// `struct tpacket_req3`.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct TpacketReq3 {
    tp_block_size: u32,
    tp_block_nr: u32,
    tp_frame_size: u32,
    tp_frame_nr: u32,
    tp_retire_blk_tov: u32,
    tp_sizeof_priv: u32,
    tp_feature_req_word: u32,
}

/// Which transmit path is in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxMode {
    /// TPACKET_V3 memory-mapped TX ring.
    Ring,
    /// `sendmmsg` batches.
    Sendmmsg,
}

const PERMISSION_HINT: &str = "AF_PACKET needs CAP_NET_RAW: run as root or grant it with \
     `sudo setcap cap_net_raw,cap_net_admin+ep $(command -v wpg)`";

#[allow(clippy::needless_pass_by_value)] // used as `map_err(|e| map_err(.., e))`
fn map_err(ctx: &str, iface: &str, e: io::Error) -> TxError {
    match e.raw_os_error() {
        Some(libc::EPERM | libc::EACCES) => {
            TxError::Permission(format!("{ctx} on `{iface}`: {e}. {PERMISSION_HINT}"))
        }
        Some(libc::ENODEV | libc::ENXIO) => TxError::Interface(iface.to_owned()),
        _ => TxError::Io(io::Error::new(e.kind(), format!("{ctx} on `{iface}`: {e}"))),
    }
}

/// `size_of::<T>()` as `socklen_t` (socket address / option structs are tiny).
#[allow(clippy::cast_possible_truncation)]
const fn socklen<T>() -> libc::socklen_t {
    size_of::<T>() as libc::socklen_t
}

fn cvt(rc: libc::c_int) -> io::Result<libc::c_int> {
    if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(rc)
    }
}

fn setsockopt_int(
    fd: RawFd,
    level: libc::c_int,
    opt: libc::c_int,
    v: libc::c_int,
) -> io::Result<()> {
    // SAFETY: passes a pointer to a live c_int with its exact size.
    cvt(unsafe {
        libc::setsockopt(
            fd,
            level,
            opt,
            (&raw const v).cast(),
            socklen::<libc::c_int>(),
        )
    })
    .map(drop)
}

/// Interface index of `iface` (`if_nametoindex`).
///
/// # Errors
/// [`TxError::Interface`] if the interface does not exist.
pub fn ifindex(iface: &str) -> Result<i32, TxError> {
    let c = CString::new(iface).map_err(|_| TxError::Interface(iface.to_owned()))?;
    // SAFETY: `c` is a valid NUL-terminated string.
    let idx = unsafe { libc::if_nametoindex(c.as_ptr()) };
    if idx == 0 {
        return Err(TxError::Interface(iface.to_owned()));
    }
    i32::try_from(idx).map_err(|_| TxError::Interface(iface.to_owned()))
}

/// Creates an `AF_PACKET` raw socket with protocol `proto` (host order; 0 = receive
/// nothing).
fn packet_socket(proto: u16) -> io::Result<OwnedFd> {
    // SAFETY: plain socket(2) call.
    let fd = cvt(unsafe {
        libc::socket(
            libc::AF_PACKET,
            libc::SOCK_RAW | libc::SOCK_CLOEXEC,
            libc::c_int::from(proto.to_be()),
        )
    })?;
    // SAFETY: `fd` was just returned by socket(2) and is owned by nobody else.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

fn bind_ll(fd: RawFd, ifindex: i32, proto: u16) -> io::Result<()> {
    // SAFETY: sockaddr_ll is plain data; zeroed is valid.
    let mut sll: libc::sockaddr_ll = unsafe { std::mem::zeroed() };
    #[allow(clippy::cast_possible_truncation)] // AF_PACKET = 17
    {
        sll.sll_family = libc::AF_PACKET as libc::c_ushort;
    }
    sll.sll_protocol = proto.to_be();
    sll.sll_ifindex = ifindex;
    // SAFETY: pointer and length describe `sll`.
    cvt(unsafe { libc::bind(fd, (&raw const sll).cast(), socklen::<libc::sockaddr_ll>()) })
        .map(drop)
}

fn iface_mtu(iface: &str) -> Option<usize> {
    std::fs::read_to_string(format!("/sys/class/net/{iface}/mtu"))
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn page_size() -> usize {
    // SAFETY: sysconf has no preconditions.
    let p = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    usize::try_from(p).ok().filter(|p| *p > 0).unwrap_or(4096)
}

/// The mapped TX ring.
#[derive(Debug)]
struct Ring {
    base: *mut u8,
    len: usize,
    frame_size: usize,
    frame_nr: usize,
    frames_per_block: usize,
    block_size: usize,
    cur: usize,
}

// SAFETY: the mapping is owned exclusively by this struct (and the kernel); the
// backend is used from one thread at a time (`&mut self`).
unsafe impl Send for Ring {}

impl Ring {
    fn setup(fd: RawFd, iface: &str, depth: usize) -> io::Result<Ring> {
        setsockopt_int(fd, libc::SOL_PACKET, PACKET_VERSION, TPACKET_V3)?;
        let mtu = iface_mtu(iface).unwrap_or(1500).min(9216);
        let need = TPACKET3_HDR + mtu + 14 + 4;
        let frame_size = need.next_power_of_two().max(2048);
        let page = page_size();
        let frames_per_block = (32 * 1024 / frame_size).max(1) * 2;
        let block_size = (frame_size * frames_per_block).div_ceil(page) * page;
        let frames_per_block = block_size / frame_size;
        let max_frames = (RING_MAX_BYTES / frame_size).max(frames_per_block);
        let want = depth.clamp(frames_per_block, max_frames);
        let block_nr = want.div_ceil(frames_per_block);
        let frame_nr = block_nr * frames_per_block;
        let u = |v: usize| u32::try_from(v).map_err(|_| io::Error::from_raw_os_error(libc::EINVAL));
        let req = TpacketReq3 {
            tp_block_size: u(block_size)?,
            tp_block_nr: u(block_nr)?,
            tp_frame_size: u(frame_size)?,
            tp_frame_nr: u(frame_nr)?,
            ..TpacketReq3::default()
        };
        // SAFETY: pointer and length describe `req`, the layout of tpacket_req3.
        cvt(unsafe {
            libc::setsockopt(
                fd,
                libc::SOL_PACKET,
                PACKET_TX_RING,
                (&raw const req).cast(),
                socklen::<TpacketReq3>(),
            )
        })?;
        let len = block_size * block_nr;
        // SAFETY: maps the ring the kernel just allocated for `fd`.
        let base = unsafe {
            libc::mmap(
                ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED | libc::MAP_LOCKED,
                fd,
                0,
            )
        };
        let base = if base == libc::MAP_FAILED {
            // MAP_LOCKED can fail under RLIMIT_MEMLOCK; retry without it.
            // SAFETY: as above.
            let b = unsafe {
                libc::mmap(
                    ptr::null_mut(),
                    len,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_SHARED,
                    fd,
                    0,
                )
            };
            if b == libc::MAP_FAILED {
                return Err(io::Error::last_os_error());
            }
            b
        } else {
            base
        };
        Ok(Ring {
            base: base.cast(),
            len,
            frame_size,
            frame_nr,
            frames_per_block,
            block_size,
            cur: 0,
        })
    }

    fn max_data(&self) -> usize {
        self.frame_size - TPACKET3_HDR
    }

    fn slot(&self, i: usize) -> *mut u8 {
        let off = (i / self.frames_per_block) * self.block_size
            + (i % self.frames_per_block) * self.frame_size;
        debug_assert!(off + self.frame_size <= self.len);
        // SAFETY: `off` is inside the mapping (see the debug assertion).
        unsafe { self.base.add(off) }
    }

    fn status(&self, i: usize) -> &AtomicU32 {
        // SAFETY: tp_status is a 4-byte aligned u32 inside the slot header, shared
        // with the kernel; accessing it atomically is the documented protocol.
        unsafe { &*self.slot(i).add(TP_STATUS_OFF).cast::<AtomicU32>() }
    }
}

impl Drop for Ring {
    fn drop(&mut self) {
        // SAFETY: unmaps exactly the region mapped in `setup`.
        unsafe {
            libc::munmap(self.base.cast(), self.len);
        }
    }
}

#[derive(Debug)]
struct Sock {
    fd: OwnedFd,
    iface: String,
    ifindex: i32,
    ring: Option<Ring>,
    /// Plain socket for frames that do not fit a ring slot (created lazily).
    big: Option<OwnedFd>,
    qdisc_bypass: bool,
}

/// `AF_PACKET` transmit backend.
#[derive(Debug, Default)]
pub struct AfPacketBackend {
    sock: Option<Sock>,
    stats: TxStats,
}

impl AfPacketBackend {
    /// Creates a closed backend.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Transmit path in use (`None` while closed).
    #[must_use]
    pub fn mode(&self) -> Option<TxMode> {
        self.sock.as_ref().map(|s| {
            if s.ring.is_some() {
                TxMode::Ring
            } else {
                TxMode::Sendmmsg
            }
        })
    }

    fn new_socket(iface: &str, qdisc_bypass: bool) -> Result<OwnedFd, TxError> {
        let fd = packet_socket(0).map_err(|e| map_err("socket(AF_PACKET)", iface, e))?;
        if qdisc_bypass {
            setsockopt_int(fd.as_raw_fd(), libc::SOL_PACKET, PACKET_QDISC_BYPASS, 1)
                .map_err(|e| map_err("PACKET_QDISC_BYPASS", iface, e))?;
        }
        Ok(fd)
    }
}

/// `sendmmsg` of `frames` on `fd`; returns (sent, dropped). Retries `ENOBUFS` /
/// `EAGAIN` for a while, drops frames the kernel rejects as malformed / oversized.
fn send_mmsg(fd: RawFd, frames: &[&[u8]]) -> io::Result<(usize, usize)> {
    const CHUNK: usize = 256;
    let mut iov: [libc::iovec; CHUNK] = [libc::iovec {
        iov_base: ptr::null_mut(),
        iov_len: 0,
    }; CHUNK];
    // SAFETY: mmsghdr is plain data; zeroed is valid (null pointers, zero lengths).
    let mut msgs: [libc::mmsghdr; CHUNK] = unsafe { std::mem::zeroed() };
    let mut sent = 0;
    let mut dropped = 0;
    let mut i = 0;
    let mut stalled_since: Option<Instant> = None;
    while i < frames.len() {
        let n = (frames.len() - i).min(CHUNK);
        for k in 0..n {
            let f = frames[i + k];
            iov[k].iov_base = f.as_ptr().cast_mut().cast();
            iov[k].iov_len = f.len();
            msgs[k].msg_hdr.msg_iov = &raw mut iov[k];
            msgs[k].msg_hdr.msg_iovlen = 1;
            msgs[k].msg_len = 0;
        }
        // SAFETY: `msgs[..n]` point at `iov[..n]`, which point at live frame data.
        #[allow(clippy::cast_possible_truncation)] // n <= CHUNK
        let rc = unsafe { libc::sendmmsg(fd, msgs.as_mut_ptr(), n.min(CHUNK) as libc::c_uint, 0) };
        if rc > 0 {
            let k = rc as usize;
            sent += k;
            i += k;
            stalled_since = None;
            continue;
        }
        let e = io::Error::last_os_error();
        match e.raw_os_error() {
            Some(libc::EINTR) => {}
            Some(libc::ENOBUFS | libc::EAGAIN) => {
                let t = *stalled_since.get_or_insert_with(Instant::now);
                if t.elapsed() > Duration::from_millis(100) {
                    dropped += frames.len() - i;
                    break;
                }
                std::thread::yield_now();
            }
            Some(libc::EMSGSIZE | libc::EINVAL) => {
                dropped += 1;
                i += 1;
            }
            _ => return Err(e),
        }
    }
    Ok((sent, dropped))
}

impl Sock {
    fn kick(&self, flags: libc::c_int) -> io::Result<()> {
        loop {
            // SAFETY: zero-length send on our own socket flushes the TX ring.
            let rc = unsafe { libc::send(self.fd.as_raw_fd(), ptr::null(), 0, flags) };
            if rc >= 0 {
                return Ok(());
            }
            let e = io::Error::last_os_error();
            if e.raw_os_error() != Some(libc::EINTR) {
                return Err(e);
            }
        }
    }

    /// Resets `TP_STATUS_WRONG_FORMAT` slots to available; returns how many.
    fn reap_wrong_format(&self) -> u64 {
        let Some(ring) = &self.ring else { return 0 };
        let mut n = 0;
        for i in 0..ring.frame_nr {
            let st = ring.status(i);
            if st.load(Ordering::Acquire) & TP_STATUS_WRONG_FORMAT != 0 {
                st.store(TP_STATUS_AVAILABLE, Ordering::Release);
                n += 1;
            }
        }
        n
    }

    /// Waits until ring slot `i` is free; returns frames found rejected.
    fn wait_slot(&self, i: usize) -> Result<u64, TxError> {
        let ring = self.ring.as_ref().expect("ring");
        let st = ring.status(i);
        let mut dropped = 0;
        let start = Instant::now();
        let mut kicked = false;
        loop {
            let s = st.load(Ordering::Acquire);
            if s == TP_STATUS_AVAILABLE {
                return Ok(dropped);
            }
            if s & TP_STATUS_WRONG_FORMAT != 0 {
                dropped += self.reap_wrong_format();
                continue;
            }
            if !kicked {
                // Blocking kick: the kernel sends everything pending and waits.
                if let Err(e) = self.kick(0) {
                    if !matches!(
                        e.raw_os_error(),
                        Some(libc::ENOBUFS | libc::EAGAIN | libc::EMSGSIZE | libc::EINVAL)
                    ) {
                        return Err(map_err("send", &self.iface, e));
                    }
                }
                kicked = true;
                continue;
            }
            if start.elapsed() > Duration::from_secs(2) {
                return Err(TxError::Io(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("AF_PACKET TX ring on `{}` is not draining", self.iface),
                )));
            }
            let mut pfd = libc::pollfd {
                fd: self.fd.as_raw_fd(),
                events: libc::POLLOUT,
                revents: 0,
            };
            // SAFETY: one valid pollfd.
            unsafe {
                libc::poll(&raw mut pfd, 1, 1);
            }
            kicked = false;
        }
    }

    fn send_big(&mut self, frame: &[u8]) -> Result<(usize, usize), TxError> {
        if self.big.is_none() {
            let fd = AfPacketBackend::new_socket(&self.iface, self.qdisc_bypass)?;
            bind_ll(fd.as_raw_fd(), self.ifindex, 0)
                .map_err(|e| map_err("bind", &self.iface, e))?;
            self.big = Some(fd);
        }
        let fd = self.big.as_ref().expect("created above").as_raw_fd();
        send_mmsg(fd, &[frame]).map_err(|e| map_err("sendmmsg", &self.iface, e))
    }

    fn send_ring(&mut self, frames: &[Frame]) -> Result<(usize, u64), TxError> {
        let mut queued = 0;
        let mut dropped = 0u64;
        for f in frames {
            let data = f.as_bytes();
            let max = self.ring.as_ref().expect("ring").max_data();
            if data.len() > max {
                let (s, d) = self.send_big(data)?;
                queued += s;
                dropped += d as u64;
                continue;
            }
            let i = self.ring.as_ref().expect("ring").cur;
            dropped += self.wait_slot(i)?;
            let ring = self.ring.as_mut().expect("ring");
            let slot = ring.slot(i);
            // SAFETY: the slot is AVAILABLE (owned by user space); the header and
            // `data.len() <= max_data()` octets after it lie inside the slot.
            unsafe {
                ptr::write_bytes(slot, 0, TP_STATUS_OFF);
                slot.add(TP_LEN_OFF)
                    .cast::<u32>()
                    .write(u32::try_from(data.len()).unwrap_or(u32::MAX));
                ptr::copy_nonoverlapping(data.as_ptr(), slot.add(TPACKET3_HDR), data.len());
            }
            ring.status(i)
                .store(TP_STATUS_SEND_REQUEST, Ordering::Release);
            ring.cur = (i + 1) % ring.frame_nr;
            queued += 1;
        }
        if let Err(e) = self.kick(libc::MSG_DONTWAIT) {
            match e.raw_os_error() {
                // Queue full: frames stay in the ring and go out with the next kick.
                Some(libc::ENOBUFS | libc::EAGAIN) => {}
                Some(libc::EMSGSIZE | libc::EINVAL) => dropped += self.reap_wrong_format(),
                _ => return Err(map_err("send", &self.iface, e)),
            }
        }
        Ok((queued, dropped))
    }
}

impl TxBackend for AfPacketBackend {
    fn name(&self) -> &'static str {
        "afpacket"
    }

    fn open(&mut self, iface: &str, opts: &TxOptions) -> Result<(), TxError> {
        if self.sock.is_some() {
            return Err(TxError::AlreadyOpen);
        }
        let idx = ifindex(iface)?;
        let mut fd = Self::new_socket(iface, opts.qdisc_bypass)?;
        let mut ring = None;
        if opts.tx_ring {
            match Ring::setup(fd.as_raw_fd(), iface, opts.ring_depth.max(1)) {
                Ok(r) => ring = Some(r),
                Err(_) => {
                    // Start over with a clean socket for the sendmmsg path.
                    fd = Self::new_socket(iface, opts.qdisc_bypass)?;
                }
            }
        }
        bind_ll(fd.as_raw_fd(), idx, 0).map_err(|e| map_err("bind", iface, e))?;
        self.sock = Some(Sock {
            fd,
            iface: iface.to_owned(),
            ifindex: idx,
            ring,
            big: None,
            qdisc_bypass: opts.qdisc_bypass,
        });
        self.stats = TxStats::default();
        Ok(())
    }

    fn send_batch(&mut self, frames: &[Frame]) -> Result<usize, TxError> {
        let sock = self.sock.as_mut().ok_or(TxError::NotOpen)?;
        self.stats.batches += 1;
        let (sent, dropped) = if sock.ring.is_some() {
            sock.send_ring(frames)?
        } else {
            let mut refs: [&[u8]; 64] = [&[]; 64];
            let mut sent = 0;
            let mut dropped = 0u64;
            for chunk in frames.chunks(64) {
                for (r, f) in refs.iter_mut().zip(chunk) {
                    *r = f.as_bytes();
                }
                let (s, d) = send_mmsg(sock.fd.as_raw_fd(), &refs[..chunk.len()])
                    .map_err(|e| map_err("sendmmsg", &sock.iface, e))?;
                sent += s;
                dropped += d as u64;
            }
            (sent, dropped)
        };
        self.stats.packets += sent as u64;
        self.stats.dropped += dropped;
        // Bytes of the accepted frames (rejections are rare; approximate by order).
        self.stats.bytes += frames
            .iter()
            .take(sent)
            .map(|f| f.len() as u64)
            .sum::<u64>();
        Ok(sent)
    }

    fn stats(&self) -> TxStats {
        self.stats
    }

    fn detail(&self) -> Option<String> {
        let sock = self.sock.as_ref()?;
        Some(match &sock.ring {
            Some(r) => format!(
                "TPACKET_V3 TX ring, {} slots x {} B{}",
                r.frame_nr,
                r.frame_size,
                if sock.qdisc_bypass {
                    ", qdisc bypass"
                } else {
                    ""
                }
            ),
            None => format!(
                "sendmmsg{}",
                if sock.qdisc_bypass {
                    ", qdisc bypass"
                } else {
                    ""
                }
            ),
        })
    }

    fn close(&mut self) -> Result<(), TxError> {
        if let Some(sock) = self.sock.take() {
            if sock.ring.is_some() {
                // Flush what is still queued, then account late rejections.
                let _ = sock.kick(0);
                self.stats.dropped += sock.reap_wrong_format();
            }
            drop(sock);
        }
        Ok(())
    }
}

impl Drop for AfPacketBackend {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::{synthetic_frame, BENCH_ETHERTYPE};

    /// Opens on `lo`, or returns `None` (test skipped) without `CAP_NET_RAW`.
    fn open_lo(tx_ring: bool) -> Option<AfPacketBackend> {
        let mut b = AfPacketBackend::new();
        let opts = TxOptions {
            tx_ring,
            ring_depth: 256,
            ..TxOptions::default()
        };
        match b.open("lo", &opts) {
            Ok(()) => Some(b),
            Err(TxError::Permission(m)) => {
                assert!(m.contains("setcap"));
                eprintln!("skipping AF_PACKET test: {m}");
                None
            }
            Err(e) => panic!("open lo: {e}"),
        }
    }

    /// A receiver for EtherType 0x88b5 on `lo`.
    fn receiver() -> OwnedFd {
        let idx = ifindex("lo").unwrap();
        let fd = packet_socket(BENCH_ETHERTYPE).unwrap();
        bind_ll(fd.as_raw_fd(), idx, BENCH_ETHERTYPE).unwrap();
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
        // SAFETY: pointer and length describe `tv`.
        unsafe {
            libc::setsockopt(
                fd.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_RCVTIMEO,
                (&raw const tv).cast(),
                socklen::<libc::timeval>(),
            );
        }
        fd
    }

    /// Counts received frames whose payload carries `tag` at offset 14.
    fn count_received(fd: &OwnedFd, tag: u8, len: usize) -> usize {
        let mut buf = vec![0u8; 70_000];
        let mut n = 0;
        loop {
            // SAFETY: `buf` is valid for `buf.len()` bytes.
            let rc = unsafe { libc::recv(fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len(), 0) };
            if rc < 0 {
                return n;
            }
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

    fn roundtrip(tx_ring: bool, tag: u8, len: usize) {
        let Some(mut b) = open_lo(tx_ring) else {
            return;
        };
        if tx_ring {
            assert_eq!(b.mode(), Some(TxMode::Ring));
        } else {
            assert_eq!(b.mode(), Some(TxMode::Sendmmsg));
        }
        let rx = receiver();
        let frames: Vec<Frame> = (0..50).map(|_| tagged(tag, len)).collect();
        let mut sent = 0;
        for _ in 0..4 {
            sent += b.send_batch(&frames).unwrap();
        }
        b.close().unwrap();
        assert_eq!(sent, 200);
        let st = b.stats();
        assert_eq!(st.packets, 200);
        assert_eq!(st.dropped, 0);
        assert_eq!(st.bytes, 200 * len as u64);
        let got = count_received(&rx, tag, len);
        assert_eq!(got, 200, "frames received on lo");
    }

    #[test]
    fn lo_sendmmsg_frames_arrive() {
        roundtrip(false, 0xa1, 64);
    }

    #[test]
    fn lo_tx_ring_frames_arrive() {
        roundtrip(true, 0xa2, 128);
    }

    #[test]
    fn lo_tx_ring_oversized_frames_use_plain_socket() {
        // Ring slots are sized from min(MTU, 9216) (16 KiB slots on lo, whose MTU
        // is 64 KiB), so a 20 000-octet frame takes the plain-socket path.
        roundtrip(true, 0xa3, 20_000);
    }

    #[test]
    fn unknown_interface() {
        let mut b = AfPacketBackend::new();
        assert!(matches!(
            b.open("nonexistent0", &TxOptions::default()),
            Err(TxError::Interface(_))
        ));
        assert!(matches!(b.send_batch(&[]), Err(TxError::NotOpen)));
    }

    #[test]
    fn job_on_lo() {
        use crate::job::{JobConfig, TxJob};
        use crate::source::PreRendered;
        let Some(b) = open_lo(true) else { return };
        let src = PreRendered::new(vec![synthetic_frame(60)], Some(1000));
        let r = TxJob::start(Box::new(b), Box::new(src), JobConfig::default())
            .unwrap()
            .wait();
        assert_eq!(r.stats.packets, 1000);
        assert_eq!(r.backend.packets, 1000);
    }
}
