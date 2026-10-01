//! Minimal readers for classic pcap (both byte orders, micro / nanosecond
//! timestamps) and pcapng (SHB / IDB / EPB / SPB / obsolete PB, several sections),
//! `LINKTYPE_ETHERNET` only. Used by [`crate::import`].

use std::fmt;

/// `LINKTYPE_ETHERNET`.
const LINKTYPE_ETHERNET: u32 = 1;

/// Errors of the capture readers.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CaptureError {
    /// Neither a pcap nor a pcapng file.
    UnknownFormat,
    /// The file ends inside a header or record.
    Truncated,
    /// Structurally invalid content.
    Malformed(String),
    /// The link type is not Ethernet.
    UnsupportedLinkType(u32),
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CaptureError::UnknownFormat => f.write_str("not a pcap or pcapng file"),
            CaptureError::Truncated => f.write_str("the capture file is truncated"),
            CaptureError::Malformed(m) => write!(f, "malformed capture file: {m}"),
            CaptureError::UnsupportedLinkType(t) => {
                write!(f, "unsupported link type {t} (only Ethernet is supported)")
            }
        }
    }
}

impl std::error::Error for CaptureError {}

/// One captured frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedFrame {
    /// Capture time in nanoseconds since the epoch (0 when unknown).
    pub ts_ns: u64,
    /// Captured octets (`caplen`, possibly shorter than the original frame).
    pub data: Vec<u8>,
}

/// Reads all frames of a pcap or pcapng file (detected by its magic number).
///
/// # Errors
/// [`CaptureError`] for unknown / truncated / malformed files and non-Ethernet links.
pub fn read_capture(bytes: &[u8]) -> Result<Vec<CapturedFrame>, CaptureError> {
    match bytes.get(..4) {
        Some([0x0a, 0x0d, 0x0d, 0x0a]) => read_pcapng(bytes),
        Some(_) => read_pcap(bytes),
        None => Err(CaptureError::UnknownFormat),
    }
}

#[derive(Clone, Copy)]
struct Order(bool); // true = little endian

impl Order {
    fn u16(self, b: &[u8], at: usize) -> Result<u16, CaptureError> {
        let s: [u8; 2] = b
            .get(at..at + 2)
            .and_then(|s| s.try_into().ok())
            .ok_or(CaptureError::Truncated)?;
        Ok(if self.0 {
            u16::from_le_bytes(s)
        } else {
            u16::from_be_bytes(s)
        })
    }

    fn u32(self, b: &[u8], at: usize) -> Result<u32, CaptureError> {
        let s: [u8; 4] = b
            .get(at..at + 4)
            .and_then(|s| s.try_into().ok())
            .ok_or(CaptureError::Truncated)?;
        Ok(if self.0 {
            u32::from_le_bytes(s)
        } else {
            u32::from_be_bytes(s)
        })
    }
}

/// Classic pcap.
///
/// # Errors
/// See [`read_capture`].
pub fn read_pcap(bytes: &[u8]) -> Result<Vec<CapturedFrame>, CaptureError> {
    let magic = bytes.get(..4).ok_or(CaptureError::UnknownFormat)?;
    let (le, nanos) = match magic {
        [0xd4, 0xc3, 0xb2, 0xa1] => (true, false),
        [0x4d, 0x3c, 0xb2, 0xa1] => (true, true),
        [0xa1, 0xb2, 0xc3, 0xd4] => (false, false),
        [0xa1, 0xb2, 0x3c, 0x4d] => (false, true),
        _ => return Err(CaptureError::UnknownFormat),
    };
    let o = Order(le);
    let link = o.u32(bytes, 20)? & 0x0fff_ffff; // upper bits carry FCS info
    if link != LINKTYPE_ETHERNET {
        return Err(CaptureError::UnsupportedLinkType(link));
    }
    let mut pos = 24usize;
    let mut out = Vec::new();
    while pos < bytes.len() {
        let sec = u64::from(o.u32(bytes, pos)?);
        let frac = u64::from(o.u32(bytes, pos + 4)?);
        let incl = o.u32(bytes, pos + 8)? as usize;
        let start = pos + 16;
        let data = bytes
            .get(start..start.saturating_add(incl))
            .ok_or(CaptureError::Truncated)?;
        let ts_ns = sec * 1_000_000_000 + if nanos { frac } else { frac * 1000 };
        out.push(CapturedFrame {
            ts_ns,
            data: data.to_vec(),
        });
        pos = start + incl;
    }
    Ok(out)
}

struct Interface {
    link: u32,
    /// Timestamp unit in nanoseconds per tick, as `numerator / 2^shift` style
    /// resolved to (ticks per second).
    ticks_per_sec: u64,
}

/// pcapng (several sections; each section has its own byte order and interfaces).
///
/// # Errors
/// See [`read_capture`].
pub fn read_pcapng(bytes: &[u8]) -> Result<Vec<CapturedFrame>, CaptureError> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    let mut order = Order(true);
    let mut ifaces: Vec<Interface> = Vec::new();
    while pos < bytes.len() {
        let ty_raw = bytes.get(pos..pos + 4).ok_or(CaptureError::Truncated)?;
        let (ty, total) = if ty_raw == [0x0a, 0x0d, 0x0d, 0x0a] {
            // Section header: the byte-order magic decides how to read the length.
            let magic = bytes
                .get(pos + 8..pos + 12)
                .ok_or(CaptureError::Truncated)?;
            order = match magic {
                [0x4d, 0x3c, 0x2b, 0x1a] => Order(true),
                [0x1a, 0x2b, 0x3c, 0x4d] => Order(false),
                _ => return Err(CaptureError::Malformed("bad byte-order magic".into())),
            };
            ifaces.clear();
            (0x0a0d_0d0a, order.u32(bytes, pos + 4)? as usize)
        } else {
            (order.u32(bytes, pos)?, order.u32(bytes, pos + 4)? as usize)
        };
        if total < 12 || !total.is_multiple_of(4) {
            return Err(CaptureError::Malformed(format!("bad block length {total}")));
        }
        let block = bytes.get(pos..pos + total).ok_or(CaptureError::Truncated)?;
        let body = &block[8..total - 4];
        match ty {
            0x0000_0001 => ifaces.push(parse_idb(body, order)?),
            0x0000_0006 => {
                // Enhanced Packet Block
                let id = order.u32(body, 0)? as usize;
                let (hi, lo) = (order.u32(body, 4)?, order.u32(body, 8)?);
                let cap = order.u32(body, 12)? as usize;
                let data = body.get(20..20 + cap).ok_or(CaptureError::Truncated)?;
                out.push(frame(&ifaces, id, hi, lo, data)?);
            }
            0x0000_0002 => {
                // obsolete Packet Block
                let id = order.u16(body, 0)? as usize;
                let (hi, lo) = (order.u32(body, 4)?, order.u32(body, 8)?);
                let cap = order.u32(body, 12)? as usize;
                let data = body.get(20..20 + cap).ok_or(CaptureError::Truncated)?;
                out.push(frame(&ifaces, id, hi, lo, data)?);
            }
            0x0000_0003 => {
                // Simple Packet Block: interface 0, no timestamp
                let orig = order.u32(body, 0)? as usize;
                let rest = body.get(4..).ok_or(CaptureError::Truncated)?;
                let data = &rest[..orig.min(rest.len())];
                out.push(frame(&ifaces, 0, 0, 0, data)?);
            }
            _ => {} // name resolution, statistics, custom blocks, ...
        }
        pos += total;
    }
    Ok(out)
}

fn frame(
    ifaces: &[Interface],
    id: usize,
    hi: u32,
    lo: u32,
    data: &[u8],
) -> Result<CapturedFrame, CaptureError> {
    let iface = ifaces
        .get(id)
        .ok_or_else(|| CaptureError::Malformed(format!("packet on unknown interface {id}")))?;
    if iface.link != LINKTYPE_ETHERNET {
        return Err(CaptureError::UnsupportedLinkType(iface.link));
    }
    let ticks = (u64::from(hi) << 32) | u64::from(lo);
    let ts_ns =
        u64::try_from(u128::from(ticks) * 1_000_000_000 / u128::from(iface.ticks_per_sec.max(1)))
            .unwrap_or(u64::MAX);
    Ok(CapturedFrame {
        ts_ns,
        data: data.to_vec(),
    })
}

fn parse_idb(body: &[u8], o: Order) -> Result<Interface, CaptureError> {
    let link = u32::from(o.u16(body, 0)?);
    let mut ticks_per_sec = 1_000_000u64;
    // options start after linktype(2) reserved(2) snaplen(4)
    let mut pos = 8usize;
    while pos + 4 <= body.len() {
        let code = o.u16(body, pos)?;
        let len = usize::from(o.u16(body, pos + 2)?);
        if code == 0 {
            break;
        }
        let value = body
            .get(pos + 4..pos + 4 + len)
            .ok_or(CaptureError::Truncated)?;
        if code == 9 && len == 1 {
            // if_tsresol: bit 7 set = power of two, else power of ten
            let v = value[0];
            ticks_per_sec = if v & 0x80 == 0 {
                10u64.checked_pow(u32::from(v)).unwrap_or(u64::MAX)
            } else {
                1u64.checked_shl(u32::from(v & 0x7f)).unwrap_or(u64::MAX)
            };
        }
        pos += 4 + len.div_ceil(4) * 4;
    }
    Ok(Interface {
        link,
        ticks_per_sec,
    })
}

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)]
mod tests {
    use super::*;

    fn pcap(le: bool, nanos: bool, link: u32, frames: &[&[u8]]) -> Vec<u8> {
        let w32 = |v: u32| if le { v.to_le_bytes() } else { v.to_be_bytes() };
        let w16 = |v: u16| if le { v.to_le_bytes() } else { v.to_be_bytes() };
        let mut out = Vec::new();
        out.extend(w32(if nanos { 0xa1b2_3c4d } else { 0xa1b2_c3d4 }));
        out.extend(w16(2));
        out.extend(w16(4));
        out.extend(w32(0));
        out.extend(w32(0));
        out.extend(w32(65535));
        out.extend(w32(link));
        for (i, f) in frames.iter().enumerate() {
            out.extend(w32(i as u32 + 1));
            out.extend(w32(500));
            out.extend(w32(f.len() as u32));
            out.extend(w32(f.len() as u32));
            out.extend(*f);
        }
        out
    }

    #[test]
    fn pcap_all_variants() {
        for (le, nanos) in [(true, false), (true, true), (false, false), (false, true)] {
            let f = read_capture(&pcap(le, nanos, 1, &[&[1, 2, 3], &[4; 9]])).unwrap();
            assert_eq!(f.len(), 2);
            assert_eq!(f[0].data, [1, 2, 3]);
            assert_eq!(f[1].data, [4; 9]);
            let frac = if nanos { 500 } else { 500_000 };
            assert_eq!(f[0].ts_ns, 1_000_000_000 + frac);
        }
    }

    #[test]
    fn pcap_errors() {
        assert_eq!(read_capture(b"xy"), Err(CaptureError::UnknownFormat));
        assert_eq!(read_capture(b"abcdefgh"), Err(CaptureError::UnknownFormat));
        assert_eq!(
            read_capture(&pcap(true, false, 105, &[&[1]])),
            Err(CaptureError::UnsupportedLinkType(105))
        );
        let mut t = pcap(true, false, 1, &[&[1, 2, 3, 4]]);
        t.pop();
        assert_eq!(read_capture(&t), Err(CaptureError::Truncated));
    }

    fn block(o: Order, ty: u32, body: &[u8]) -> Vec<u8> {
        let w32 = |v: u32| {
            if o.0 {
                v.to_le_bytes()
            } else {
                v.to_be_bytes()
            }
        };
        let pad = (4 - body.len() % 4) % 4;
        let total = (12 + body.len() + pad) as u32;
        let mut out = Vec::new();
        out.extend(w32(ty));
        out.extend(w32(total));
        out.extend(body);
        out.extend(vec![0; pad]);
        out.extend(w32(total));
        out
    }

    fn section(le: bool, link: u16, tsresol: Option<u8>, frames: &[&[u8]]) -> Vec<u8> {
        let o = Order(le);
        let w32 = |v: u32| if le { v.to_le_bytes() } else { v.to_be_bytes() };
        let w16 = |v: u16| if le { v.to_le_bytes() } else { v.to_be_bytes() };
        let mut shb = Vec::new();
        shb.extend(w32(0x1a2b_3c4d));
        shb.extend(w16(1));
        shb.extend(w16(0));
        shb.extend((-1i64).to_le_bytes());
        let mut out = block(o, 0x0a0d_0d0a, &shb);
        let mut idb = Vec::new();
        idb.extend(w16(link));
        idb.extend(w16(0));
        idb.extend(w32(65535));
        if let Some(r) = tsresol {
            idb.extend(w16(9));
            idb.extend(w16(1));
            idb.extend([r, 0, 0, 0]);
            idb.extend(w16(0));
            idb.extend(w16(0));
        }
        out.extend(block(o, 1, &idb));
        for (i, f) in frames.iter().enumerate() {
            let mut epb = Vec::new();
            epb.extend(w32(0));
            epb.extend(w32(0));
            epb.extend(w32(i as u32 + 1));
            epb.extend(w32(f.len() as u32));
            epb.extend(w32(f.len() as u32));
            epb.extend(*f);
            out.extend(block(o, 6, &epb));
        }
        out
    }

    #[test]
    fn pcapng_orders_sections_and_resolution() {
        let mut file = section(true, 1, None, &[&[1, 2, 3, 4, 5], &[6]]);
        file.extend(section(false, 1, Some(9), &[&[7, 7]]));
        let f = read_capture(&file).unwrap();
        assert_eq!(f.len(), 3);
        assert_eq!(f[0].data, [1, 2, 3, 4, 5]);
        assert_eq!(f[0].ts_ns, 1000); // 1 us
        assert_eq!(f[1].data, [6]);
        assert_eq!(f[2].data, [7, 7]);
        assert_eq!(f[2].ts_ns, 1); // 1 ns
    }

    #[test]
    fn pcapng_simple_packet_and_errors() {
        let o = Order(true);
        let mut file = section(true, 1, None, &[]);
        let mut spb = 3u32.to_le_bytes().to_vec();
        spb.extend([9, 8, 7]);
        file.extend(block(o, 3, &spb));
        assert_eq!(read_capture(&file).unwrap()[0].data, [9, 8, 7]);

        assert_eq!(
            read_capture(&section(true, 113, None, &[&[1]])),
            Err(CaptureError::UnsupportedLinkType(113))
        );
        // an unused non-Ethernet interface is fine
        assert!(read_capture(&section(true, 113, None, &[]))
            .unwrap()
            .is_empty());
        let mut bad = section(true, 1, None, &[&[1, 2]]);
        bad.truncate(bad.len() - 3);
        assert!(read_capture(&bad).is_err());
    }
}
