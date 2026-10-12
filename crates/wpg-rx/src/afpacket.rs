//! `afpacket` receive backend (Linux): an `AF_PACKET` / `SOCK_RAW` socket with
//! protocol `ETH_P_ALL`, bound to one interface (or to every interface for
//! `any`). `poll(2)` waits for the first frame, then `recvfrom(MSG_DONTWAIT |
//! MSG_TRUNC)` drains up to a batch. Frames the host transmits (`PACKET_OUTGOING`)
//! are skipped unless [`RxOptions::include_outgoing`] is set, so on `lo` a frame
//! counts once. Kernel drops come from `PACKET_STATISTICS`.
//!
//! Needs `CAP_NET_RAW`; `EPERM` / `EACCES` become [`RxError::Permission`].

use std::ffi::CString;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::time::Duration;

use crate::backend::{now_ns, RxBackend, RxBackendStats, RxOptions, RxPacket, RxStatus};
use crate::RxError;

// <linux/if_packet.h>
const PACKET_ADD_MEMBERSHIP: libc::c_int = 1;
const PACKET_STATISTICS: libc::c_int = 6;
const PACKET_MR_PROMISC: libc::c_ushort = 1;
const PACKET_OUTGOING: u8 = 4;
const ETH_P_ALL: u16 = 0x0003;

const PERMISSION_HINT: &str = "AF_PACKET needs CAP_NET_RAW: run as root or grant it with \
     `sudo setcap cap_net_raw,cap_net_admin+ep $(command -v wpg)`";

/// `struct packet_mreq`.
#[repr(C)]
struct PacketMreq {
    mr_ifindex: libc::c_int,
    mr_type: libc::c_ushort,
    mr_alen: libc::c_ushort,
    mr_address: [u8; 8],
}

/// `struct tpacket_stats`.
#[repr(C)]
#[derive(Default)]
struct TpacketStats {
    tp_packets: libc::c_uint,
    tp_drops: libc::c_uint,
}

#[allow(clippy::needless_pass_by_value)] // used as `map_err(|e| map_err(.., e))`
fn map_err(ctx: &str, iface: &str, e: io::Error) -> RxError {
    match e.raw_os_error() {
        Some(libc::EPERM | libc::EACCES) => {
            RxError::Permission(format!("{ctx} on `{iface}`: {e}. {PERMISSION_HINT}"))
        }
        Some(libc::ENODEV | libc::ENXIO) => RxError::Interface(iface.to_owned()),
        _ => RxError::Io(io::Error::new(e.kind(), format!("{ctx} on `{iface}`: {e}"))),
    }
}

#[allow(clippy::cast_possible_truncation)] // option / address structs are tiny
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

fn ifindex(iface: &str) -> Result<i32, RxError> {
    if iface == "any" {
        return Ok(0);
    }
    let c = CString::new(iface).map_err(|_| RxError::Interface(iface.to_owned()))?;
    // SAFETY: `c` is a valid NUL-terminated string.
    let idx = unsafe { libc::if_nametoindex(c.as_ptr()) };
    if idx == 0 {
        return Err(RxError::Interface(iface.to_owned()));
    }
    i32::try_from(idx).map_err(|_| RxError::Interface(iface.to_owned()))
}

#[derive(Debug)]
struct Open {
    fd: OwnedFd,
    iface: String,
    include_outgoing: bool,
    snaplen: usize,
    buf: Vec<u8>,
}

/// Receives through an `AF_PACKET` socket.
#[derive(Debug, Default)]
pub(crate) struct AfPacketRx {
    open: Option<Open>,
    stats: RxBackendStats,
}

impl AfPacketRx {
    fn setup(iface: &str, opts: &RxOptions) -> Result<OwnedFd, RxError> {
        let idx = ifindex(iface)?;
        // SAFETY: plain socket(2) call.
        let fd = cvt(unsafe {
            libc::socket(
                libc::AF_PACKET,
                libc::SOCK_RAW | libc::SOCK_CLOEXEC,
                libc::c_int::from(ETH_P_ALL.to_be()),
            )
        })
        .map_err(|e| map_err("socket", iface, e))?;
        // SAFETY: `fd` was just returned by socket(2) and is owned by nobody else.
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        // SAFETY: sockaddr_ll is plain data; zeroed is valid.
        let mut sll: libc::sockaddr_ll = unsafe { std::mem::zeroed() };
        #[allow(clippy::cast_possible_truncation)] // AF_PACKET = 17
        {
            sll.sll_family = libc::AF_PACKET as libc::c_ushort;
        }
        sll.sll_protocol = ETH_P_ALL.to_be();
        sll.sll_ifindex = idx;
        // SAFETY: pointer and length describe `sll`.
        cvt(unsafe {
            libc::bind(
                fd.as_raw_fd(),
                (&raw const sll).cast(),
                socklen::<libc::sockaddr_ll>(),
            )
        })
        .map_err(|e| map_err("bind", iface, e))?;
        if opts.promisc && idx != 0 {
            let mreq = PacketMreq {
                mr_ifindex: idx,
                mr_type: PACKET_MR_PROMISC,
                mr_alen: 0,
                mr_address: [0; 8],
            };
            // SAFETY: pointer and length describe `mreq`.
            cvt(unsafe {
                libc::setsockopt(
                    fd.as_raw_fd(),
                    libc::SOL_PACKET,
                    PACKET_ADD_MEMBERSHIP,
                    (&raw const mreq).cast(),
                    socklen::<PacketMreq>(),
                )
            })
            .map_err(|e| map_err("promiscuous mode", iface, e))?;
        }
        Ok(fd)
    }

    /// Reads (and resets) the kernel counters.
    fn kernel_drops(fd: RawFd) -> u64 {
        let mut st = TpacketStats::default();
        let mut len = socklen::<TpacketStats>();
        // SAFETY: pointer and length describe `st`.
        let rc = unsafe {
            libc::getsockopt(
                fd,
                libc::SOL_PACKET,
                PACKET_STATISTICS,
                (&raw mut st).cast(),
                &raw mut len,
            )
        };
        if rc == 0 {
            u64::from(st.tp_drops)
        } else {
            0
        }
    }
}

impl RxBackend for AfPacketRx {
    fn name(&self) -> &'static str {
        "afpacket"
    }

    fn open(&mut self, iface: &str, opts: &RxOptions) -> Result<(), RxError> {
        if self.open.is_some() {
            return Err(RxError::AlreadyOpen);
        }
        let fd = Self::setup(iface, opts)?;
        // Discard what the socket queued between socket(2) and bind(2) (frames of
        // other interfaces) and reset the kernel counters.
        Self::kernel_drops(fd.as_raw_fd());
        let mut scratch = [0u8; 1];
        // SAFETY: `scratch` is a valid buffer of the given length.
        while unsafe {
            libc::recv(
                fd.as_raw_fd(),
                scratch.as_mut_ptr().cast(),
                1,
                libc::MSG_DONTWAIT | libc::MSG_TRUNC,
            )
        } >= 0
        {}
        self.open = Some(Open {
            fd,
            iface: iface.to_owned(),
            include_outgoing: opts.include_outgoing,
            snaplen: opts.snaplen.max(64),
            buf: vec![0; opts.snaplen.max(64)],
        });
        self.stats = RxBackendStats::default();
        Ok(())
    }

    fn recv(
        &mut self,
        out: &mut Vec<RxPacket>,
        max: usize,
        timeout: Duration,
    ) -> Result<RxStatus, RxError> {
        let o = self.open.as_mut().ok_or(RxError::NotOpen)?;
        let fd = o.fd.as_raw_fd();
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let ms = libc::c_int::try_from(timeout.as_millis()).unwrap_or(libc::c_int::MAX);
        // SAFETY: one valid pollfd.
        let rc = unsafe { libc::poll(&raw mut pfd, 1, ms) };
        if rc < 0 {
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                return Ok(RxStatus::Ok);
            }
            return Err(map_err("poll", &o.iface, e));
        }
        let mut got = 0;
        while got < max {
            // SAFETY: sockaddr_ll is plain data; zeroed is valid.
            let mut sll: libc::sockaddr_ll = unsafe { std::mem::zeroed() };
            let mut sl = socklen::<libc::sockaddr_ll>();
            // SAFETY: buffer and address pointers / lengths describe live objects.
            let n = unsafe {
                libc::recvfrom(
                    fd,
                    o.buf.as_mut_ptr().cast(),
                    o.buf.len(),
                    libc::MSG_DONTWAIT | libc::MSG_TRUNC,
                    (&raw mut sll).cast(),
                    &raw mut sl,
                )
            };
            if n < 0 {
                let e = io::Error::last_os_error();
                match e.kind() {
                    io::ErrorKind::WouldBlock => break,
                    io::ErrorKind::Interrupted => continue,
                    _ => return Err(map_err("recvfrom", &o.iface, e)),
                }
            }
            if sll.sll_pkttype == PACKET_OUTGOING && !o.include_outgoing {
                continue;
            }
            let len = usize::try_from(n).unwrap_or(0);
            let cap = len.min(o.snaplen);
            out.push(RxPacket {
                ts_ns: now_ns(),
                len,
                data: o.buf[..cap].to_vec(),
            });
            got += 1;
        }
        self.stats.received += got as u64;
        self.stats.dropped += Self::kernel_drops(fd);
        Ok(RxStatus::Ok)
    }

    fn stats(&self) -> RxBackendStats {
        self.stats
    }

    fn close(&mut self) -> Result<(), RxError> {
        self.open = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_interface() {
        let mut b = AfPacketRx::default();
        assert!(matches!(
            b.open("wpgnope0", &RxOptions::default()),
            Err(RxError::Interface(_))
        ));
        let mut out = Vec::new();
        assert!(matches!(
            b.recv(&mut out, 1, Duration::ZERO),
            Err(RxError::NotOpen)
        ));
    }

    /// Sends frames on `lo` with a plain packet socket and receives them.
    #[test]
    fn receives_on_loopback() {
        let mut rx = AfPacketRx::default();
        match rx.open("lo", &RxOptions::default()) {
            Ok(()) => {}
            Err(RxError::Permission(m)) => {
                eprintln!("skipping: {m}");
                return;
            }
            Err(e) => panic!("{e}"),
        }
        // A socket never sees its own transmissions, so send from a third one.
        let mut both = AfPacketRx::default();
        both.open(
            "lo",
            &RxOptions {
                include_outgoing: true,
                ..RxOptions::default()
            },
        )
        .unwrap();
        let mut tx = AfPacketRx::default();
        tx.open("lo", &RxOptions::default()).unwrap();
        let tx_fd = tx.open.as_ref().unwrap().fd.as_raw_fd();
        let mut frame = vec![0u8; 6];
        frame.extend_from_slice(&[0x02, 0x77, 0x70, 0x67, 0, 1]);
        frame.extend_from_slice(&[0x88, 0xb5]);
        frame.extend_from_slice(b"wpg-rx afpacket test");
        frame.resize(80, 0xee);
        for _ in 0..3 {
            // SAFETY: valid buffer; the socket is bound to `lo`.
            let n = unsafe { libc::send(tx_fd, frame.as_ptr().cast(), frame.len(), 0) };
            assert_eq!(usize::try_from(n).unwrap(), frame.len());
        }
        let mut out = Vec::new();
        let mut outgoing = Vec::new();
        for _ in 0..50 {
            rx.recv(&mut out, 64, Duration::from_millis(20)).unwrap();
            both.recv(&mut outgoing, 64, Duration::ZERO).unwrap();
            if out.iter().filter(|p| p.data == frame).count() >= 3
                && outgoing.iter().filter(|p| p.data == frame).count() >= 6
            {
                break;
            }
        }
        // Each frame once without outgoing copies, twice with them.
        assert_eq!(out.iter().filter(|p| p.data == frame).count(), 3);
        assert_eq!(outgoing.iter().filter(|p| p.data == frame).count(), 6);
        assert!(out.iter().all(|p| p.ts_ns > 0));
        assert!(rx.stats().received >= 3);
        rx.close().unwrap();
        rx.close().unwrap();
    }
}
