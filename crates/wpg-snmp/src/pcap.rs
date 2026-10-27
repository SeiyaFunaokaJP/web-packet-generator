//! Recording of the SNMP datagrams the tool sends and receives as pcap /
//! pcapng files.
//!
//! The datagrams come from ordinary UDP sockets, so the link and IP headers
//! are synthesised: Ethernet II with fixed locally administered placeholder
//! MACs ([`LOCAL_MAC`] for this host, [`PEER_MAC`] for the peer), IPv4 (or
//! IPv6) with the real addresses and a valid header checksum, and UDP with the
//! real ports and checksum. Wireshark decodes the result as SNMP.

use std::collections::VecDeque;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

/// Placeholder MAC of the wpg host (locally administered).
pub const LOCAL_MAC: [u8; 6] = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
/// Placeholder MAC of the peer (locally administered).
pub const PEER_MAC: [u8; 6] = [0x02, 0x00, 0x00, 0x00, 0x00, 0x02];

/// Default bound of a [`PcapRecorder`].
pub const DEFAULT_MAX_FRAMES: usize = 100_000;

/// One recorded frame.
#[derive(Debug, Clone)]
pub struct Frame {
    /// Nanoseconds since the Unix epoch.
    pub ts_ns: u64,
    /// The synthesised Ethernet frame.
    pub data: Vec<u8>,
}

/// File format of [`PcapRecorder::export`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PcapFormat {
    /// Classic libpcap (microsecond timestamps).
    #[default]
    Pcap,
    /// pcapng (one interface, nanosecond timestamps).
    Pcapng,
}

/// Collects frames while enabled (bounded; the oldest are dropped first).
#[derive(Debug)]
pub struct PcapRecorder {
    enabled: AtomicBool,
    frames: Mutex<VecDeque<Frame>>,
    max_frames: usize,
    dropped: Mutex<u64>,
    ip_id: AtomicU16,
}

impl Default for PcapRecorder {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_FRAMES)
    }
}

impl PcapRecorder {
    /// A disabled recorder keeping at most `max_frames`.
    #[must_use]
    pub fn new(max_frames: usize) -> Self {
        Self {
            enabled: AtomicBool::new(false),
            frames: Mutex::new(VecDeque::new()),
            max_frames: max_frames.max(1),
            dropped: Mutex::new(0),
            ip_id: AtomicU16::new(1),
        }
    }

    /// Starts / stops recording (kept frames stay).
    pub fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::Relaxed);
    }

    /// Whether recording is on.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Number of kept frames.
    #[must_use]
    pub fn len(&self) -> usize {
        self.frames
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// No frames kept.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Frames dropped because the buffer was full.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        *self.dropped.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Discards the kept frames.
    pub fn clear(&self) {
        self.frames
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        *self.dropped.lock().unwrap_or_else(PoisonError::into_inner) = 0;
    }

    /// A copy of the kept frames.
    #[must_use]
    pub fn frames(&self) -> Vec<Frame> {
        self.frames
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .cloned()
            .collect()
    }

    /// The kept frames as a file.
    #[must_use]
    pub fn export(&self, format: PcapFormat) -> Vec<u8> {
        let frames = self.frames();
        match format {
            PcapFormat::Pcap => write_pcap(&frames),
            PcapFormat::Pcapng => write_pcapng(&frames),
        }
    }

    /// Records a frame regardless of [`PcapRecorder::enabled`].
    pub fn push(&self, src: SocketAddr, dst: SocketAddr, payload: &[u8], outgoing: bool) {
        let id = self.ip_id.fetch_add(1, Ordering::Relaxed);
        let data = synth_frame(src, dst, payload, outgoing, id);
        let ts_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX));
        let mut f = self.frames.lock().unwrap_or_else(PoisonError::into_inner);
        if f.len() >= self.max_frames {
            f.pop_front();
            *self.dropped.lock().unwrap_or_else(PoisonError::into_inner) += 1;
        }
        f.push_back(Frame { ts_ns, data });
    }
}

/// What the client and the receiver hold of a shared [`PcapRecorder`]:
/// records only while it is enabled, knows which side is local (for the
/// placeholder MACs).
#[derive(Debug, Clone)]
pub struct Tap {
    rec: std::sync::Arc<PcapRecorder>,
}

impl Tap {
    /// Wraps a recorder.
    #[must_use]
    pub fn new(rec: std::sync::Arc<PcapRecorder>) -> Self {
        Self { rec }
    }

    /// Records an outgoing datagram (when enabled).
    pub fn sent(&self, local: SocketAddr, peer: SocketAddr, payload: &[u8]) {
        if self.rec.enabled() {
            self.rec.push(local, peer, payload, true);
        }
    }

    /// Records an incoming datagram (when enabled).
    pub fn received(&self, peer: SocketAddr, local: SocketAddr, payload: &[u8]) {
        if self.rec.enabled() {
            self.rec.push(peer, local, payload, false);
        }
    }
}

/// Internet checksum of `data` with an initial sum.
fn checksum(init: u32, data: &[u8]) -> u16 {
    let mut sum = init;
    let (pairs, rest) = data.as_chunks::<2>();
    for c in pairs {
        sum += u32::from(u16::from_be_bytes(*c));
    }
    if let [b] = rest {
        sum += u32::from(*b) << 8;
    }
    while sum > 0xffff {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    #[allow(clippy::cast_possible_truncation)] // folded
    let s = sum as u16;
    !s
}

fn sum16(data: &[u8]) -> u32 {
    data.chunks(2)
        .map(|c| u32::from(u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)])))
        .sum()
}

/// One Ethernet / IP / UDP frame carrying `payload` from `src` to `dst`.
/// Mixed address families (an IPv4-mapped peer) are mapped to IPv6.
#[must_use]
pub fn synth_frame(
    src: SocketAddr,
    dst: SocketAddr,
    payload: &[u8],
    outgoing: bool,
    ip_id: u16,
) -> Vec<u8> {
    let (smac, dmac) = if outgoing {
        (LOCAL_MAC, PEER_MAC)
    } else {
        (PEER_MAC, LOCAL_MAC)
    };
    let udp_len = 8 + payload.len();
    let mut udp = Vec::with_capacity(udp_len);
    udp.extend_from_slice(&src.port().to_be_bytes());
    udp.extend_from_slice(&dst.port().to_be_bytes());
    #[allow(clippy::cast_possible_truncation)] // UDP payloads are < 64 KiB
    udp.extend_from_slice(&(udp_len as u16).to_be_bytes());
    udp.extend_from_slice(&[0, 0]);
    udp.extend_from_slice(payload);
    let mut frame = Vec::with_capacity(14 + 40 + udp_len);
    frame.extend_from_slice(&dmac);
    frame.extend_from_slice(&smac);
    match (src.ip(), dst.ip()) {
        (IpAddr::V4(s), IpAddr::V4(d)) => {
            frame.extend_from_slice(&[0x08, 0x00]);
            let total = 20 + udp_len;
            let mut ip = Vec::with_capacity(20);
            ip.extend_from_slice(&[0x45, 0x00]);
            #[allow(clippy::cast_possible_truncation)]
            ip.extend_from_slice(&(total as u16).to_be_bytes());
            ip.extend_from_slice(&ip_id.to_be_bytes());
            ip.extend_from_slice(&[0x40, 0x00, 64, 17, 0, 0]);
            ip.extend_from_slice(&s.octets());
            ip.extend_from_slice(&d.octets());
            let c = checksum(0, &ip);
            ip[10..12].copy_from_slice(&c.to_be_bytes());
            #[allow(clippy::cast_possible_truncation)]
            let pseudo = sum16(&s.octets()) + sum16(&d.octets()) + 17 + udp_len as u32;
            let mut c = checksum(pseudo, &udp);
            if c == 0 {
                c = 0xffff;
            }
            udp[6..8].copy_from_slice(&c.to_be_bytes());
            frame.extend_from_slice(&ip);
        }
        (s, d) => {
            let to6 = |a: IpAddr| match a {
                IpAddr::V4(x) => x.to_ipv6_mapped(),
                IpAddr::V6(x) => x,
            };
            let (s, d) = (to6(s), to6(d));
            frame.extend_from_slice(&[0x86, 0xdd]);
            frame.extend_from_slice(&[0x60, 0, 0, 0]);
            #[allow(clippy::cast_possible_truncation)]
            frame.extend_from_slice(&(udp_len as u16).to_be_bytes());
            frame.extend_from_slice(&[17, 64]);
            frame.extend_from_slice(&s.octets());
            frame.extend_from_slice(&d.octets());
            #[allow(clippy::cast_possible_truncation)]
            let pseudo = sum16(&s.octets()) + sum16(&d.octets()) + 17 + udp_len as u32;
            let mut c = checksum(pseudo, &udp);
            if c == 0 {
                c = 0xffff;
            }
            udp[6..8].copy_from_slice(&c.to_be_bytes());
        }
    }
    frame.extend_from_slice(&udp);
    frame
}

/// Classic pcap (`LINKTYPE_ETHERNET`, microseconds).
#[must_use]
pub fn write_pcap(frames: &[Frame]) -> Vec<u8> {
    let mut out = Vec::with_capacity(24 + frames.iter().map(|f| f.data.len() + 16).sum::<usize>());
    out.extend_from_slice(&0xa1b2_c3d4u32.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&65_535u32.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    for f in frames {
        #[allow(clippy::cast_possible_truncation)] // seconds fit until 2106
        let sec = (f.ts_ns / 1_000_000_000) as u32;
        #[allow(clippy::cast_possible_truncation)]
        let usec = ((f.ts_ns % 1_000_000_000) / 1000) as u32;
        #[allow(clippy::cast_possible_truncation)]
        let len = f.data.len() as u32;
        out.extend_from_slice(&sec.to_le_bytes());
        out.extend_from_slice(&usec.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&f.data);
    }
    out
}

fn pcapng_block(out: &mut Vec<u8>, ty: u32, body: &[u8]) {
    let padded = body.len().div_ceil(4) * 4;
    #[allow(clippy::cast_possible_truncation)]
    let total = (12 + padded) as u32;
    out.extend_from_slice(&ty.to_le_bytes());
    out.extend_from_slice(&total.to_le_bytes());
    out.extend_from_slice(body);
    out.resize(out.len() + padded - body.len(), 0);
    out.extend_from_slice(&total.to_le_bytes());
}

/// pcapng: section header, one Ethernet interface with nanosecond
/// resolution, enhanced packet blocks.
#[must_use]
pub fn write_pcapng(frames: &[Frame]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut shb = Vec::new();
    shb.extend_from_slice(&0x1a2b_3c4du32.to_le_bytes());
    shb.extend_from_slice(&1u16.to_le_bytes());
    shb.extend_from_slice(&0u16.to_le_bytes());
    shb.extend_from_slice(&(-1i64).to_le_bytes());
    pcapng_block(&mut out, 0x0a0d_0d0a, &shb);
    let mut idb = Vec::new();
    idb.extend_from_slice(&1u16.to_le_bytes());
    idb.extend_from_slice(&0u16.to_le_bytes());
    idb.extend_from_slice(&0u32.to_le_bytes());
    // if_tsresol = 9 (nanoseconds), then opt_endofopt.
    idb.extend_from_slice(&9u16.to_le_bytes());
    idb.extend_from_slice(&1u16.to_le_bytes());
    idb.extend_from_slice(&[9, 0, 0, 0]);
    idb.extend_from_slice(&[0, 0, 0, 0]);
    pcapng_block(&mut out, 1, &idb);
    for f in frames {
        let mut epb = Vec::with_capacity(20 + f.data.len());
        epb.extend_from_slice(&0u32.to_le_bytes());
        #[allow(clippy::cast_possible_truncation)]
        let (hi, lo) = ((f.ts_ns >> 32) as u32, f.ts_ns as u32);
        epb.extend_from_slice(&hi.to_le_bytes());
        epb.extend_from_slice(&lo.to_le_bytes());
        #[allow(clippy::cast_possible_truncation)]
        let len = f.data.len() as u32;
        epb.extend_from_slice(&len.to_le_bytes());
        epb.extend_from_slice(&len.to_le_bytes());
        epb.extend_from_slice(&f.data);
        pcapng_block(&mut out, 6, &epb);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_verify() {
        let f = synth_frame(
            "192.0.2.1:40000".parse().unwrap(),
            "192.0.2.2:161".parse().unwrap(),
            b"\x30\x00",
            true,
            7,
        );
        assert_eq!(&f[..6], &PEER_MAC);
        let ip = &f[14..34];
        assert_eq!(checksum(0, ip), 0, "IPv4 header checksum");
        let udp = &f[34..];
        let pseudo = sum16(&[192, 0, 2, 1]) + sum16(&[192, 0, 2, 2]) + 17 + 10;
        assert_eq!(checksum(pseudo, udp), 0, "UDP checksum");
        let f6 = synth_frame(
            "[2001:db8::1]:40000".parse().unwrap(),
            "192.0.2.2:162".parse().unwrap(),
            b"abc",
            false,
            1,
        );
        assert_eq!(&f6[12..14], &[0x86, 0xdd]);
        assert_eq!(f6.len(), 14 + 40 + 8 + 3);
    }

    #[test]
    fn files() {
        let r = PcapRecorder::new(2);
        let a: SocketAddr = "127.0.0.1:1".parse().unwrap();
        let b: SocketAddr = "127.0.0.1:2".parse().unwrap();
        for _ in 0..3 {
            r.push(a, b, b"x", true);
        }
        assert_eq!(r.len(), 2);
        assert_eq!(r.dropped(), 1);
        let p = r.export(PcapFormat::Pcap);
        assert_eq!(&p[..4], &[0xd4, 0xc3, 0xb2, 0xa1]);
        assert_eq!(p.len(), 24 + 2 * (16 + 14 + 20 + 8 + 1));
        let n = r.export(PcapFormat::Pcapng);
        assert_eq!(&n[..4], &[0x0a, 0x0d, 0x0d, 0x0a]);
        // Block lengths chain to the end.
        let mut off = 0;
        let mut blocks = 0;
        while off < n.len() {
            let len = u32::from_le_bytes(n[off + 4..off + 8].try_into().unwrap()) as usize;
            assert_eq!(len % 4, 0);
            off += len;
            blocks += 1;
        }
        assert_eq!(off, n.len());
        assert_eq!(blocks, 4);
        r.clear();
        assert!(r.is_empty());
    }
}
