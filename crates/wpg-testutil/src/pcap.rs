//! Minimal pcap (classic) and pcapng writers / readers, `LINKTYPE_ETHERNET` only.

use std::io;
use std::path::Path;

/// `LINKTYPE_ETHERNET`.
pub const LINKTYPE_ETHERNET: u32 = 1;
/// Snap length written to headers.
pub const SNAPLEN: u32 = 65535;

fn bad(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_owned())
}

fn len_u32(n: usize) -> u32 {
    u32::try_from(n).expect("packet too large")
}

/// Serialises packets as a little-endian, microsecond-resolution classic pcap file.
/// Timestamps are `index` seconds after the epoch (deterministic output).
#[must_use]
pub fn pcap_bytes(packets: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0xa1b2_c3d4u32.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes()); // version major
    out.extend_from_slice(&4u16.to_le_bytes()); // version minor
    out.extend_from_slice(&0i32.to_le_bytes()); // thiszone
    out.extend_from_slice(&0u32.to_le_bytes()); // sigfigs
    out.extend_from_slice(&SNAPLEN.to_le_bytes());
    out.extend_from_slice(&LINKTYPE_ETHERNET.to_le_bytes());
    for (i, p) in packets.iter().enumerate() {
        out.extend_from_slice(&len_u32(i).to_le_bytes()); // ts_sec
        out.extend_from_slice(&0u32.to_le_bytes()); // ts_usec
        out.extend_from_slice(&len_u32(p.len()).to_le_bytes()); // incl_len
        out.extend_from_slice(&len_u32(p.len()).to_le_bytes()); // orig_len
        out.extend_from_slice(p);
    }
    out
}

/// Writes a classic pcap file (see [`pcap_bytes`]).
///
/// # Errors
/// Propagates I/O errors.
pub fn write_pcap(path: impl AsRef<Path>, packets: &[Vec<u8>]) -> io::Result<()> {
    std::fs::write(path, pcap_bytes(packets))
}

struct Rd<'a> {
    b: &'a [u8],
    le: bool,
}

impl Rd<'_> {
    fn u32(&self, o: usize) -> io::Result<u32> {
        let s: [u8; 4] = self
            .b
            .get(o..o + 4)
            .and_then(|s| s.try_into().ok())
            .ok_or_else(|| bad("truncated"))?;
        Ok(if self.le {
            u32::from_le_bytes(s)
        } else {
            u32::from_be_bytes(s)
        })
    }
}

/// Parses classic pcap bytes (either byte order, micro/nanosecond magic).
///
/// # Errors
/// `InvalidData` on a bad magic, truncated data or a non-Ethernet link type.
pub fn parse_pcap(bytes: &[u8]) -> io::Result<Vec<Vec<u8>>> {
    let magic = bytes.get(..4).ok_or_else(|| bad("truncated"))?;
    let le = match magic {
        [0xd4, 0xc3, 0xb2, 0xa1] | [0x4d, 0x3c, 0xb2, 0xa1] => true,
        [0xa1, 0xb2, 0xc3, 0xd4] | [0xa1, 0xb2, 0x3c, 0x4d] => false,
        _ => return Err(bad("not a pcap file")),
    };
    let r = Rd { b: bytes, le };
    if r.u32(20)? != LINKTYPE_ETHERNET {
        return Err(bad("unsupported link type"));
    }
    let mut pos = 24;
    let mut out = Vec::new();
    while pos < bytes.len() {
        let incl = r.u32(pos + 8)? as usize;
        let start = pos + 16;
        let data = bytes
            .get(start..start + incl)
            .ok_or_else(|| bad("truncated record"))?;
        out.push(data.to_vec());
        pos = start + incl;
    }
    Ok(out)
}

/// Reads a classic pcap file.
///
/// # Errors
/// I/O errors or malformed content (see [`parse_pcap`]).
pub fn read_pcap(path: impl AsRef<Path>) -> io::Result<Vec<Vec<u8>>> {
    parse_pcap(&std::fs::read(path)?)
}

fn pad4(n: usize) -> usize {
    (4 - n % 4) % 4
}

fn block(out: &mut Vec<u8>, ty: u32, body: &[u8]) {
    let total = len_u32(12 + body.len() + pad4(body.len()));
    out.extend_from_slice(&ty.to_le_bytes());
    out.extend_from_slice(&total.to_le_bytes());
    out.extend_from_slice(body);
    out.extend(std::iter::repeat_n(0u8, pad4(body.len())));
    out.extend_from_slice(&total.to_le_bytes());
}

/// Serialises packets as a little-endian pcapng file: one Section Header Block, one
/// Interface Description Block (Ethernet, default microsecond resolution) and one
/// Enhanced Packet Block per packet.
#[must_use]
pub fn pcapng_bytes(packets: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut shb = Vec::new();
    shb.extend_from_slice(&0x1a2b_3c4du32.to_le_bytes());
    shb.extend_from_slice(&1u16.to_le_bytes());
    shb.extend_from_slice(&0u16.to_le_bytes());
    shb.extend_from_slice(&(-1i64).to_le_bytes()); // section length unknown
    block(&mut out, 0x0a0d_0d0a, &shb);
    let mut idb = Vec::new();
    idb.extend_from_slice(&u16::try_from(LINKTYPE_ETHERNET).unwrap().to_le_bytes());
    idb.extend_from_slice(&0u16.to_le_bytes());
    idb.extend_from_slice(&SNAPLEN.to_le_bytes());
    block(&mut out, 1, &idb);
    for (i, p) in packets.iter().enumerate() {
        let mut epb = Vec::new();
        epb.extend_from_slice(&0u32.to_le_bytes()); // interface id
        epb.extend_from_slice(&0u32.to_le_bytes()); // ts high
        epb.extend_from_slice(&(len_u32(i) * 1_000_000).to_le_bytes()); // ts low (us)
        epb.extend_from_slice(&len_u32(p.len()).to_le_bytes());
        epb.extend_from_slice(&len_u32(p.len()).to_le_bytes());
        epb.extend_from_slice(p);
        block(&mut out, 6, &epb);
    }
    out
}

/// Writes a pcapng file (see [`pcapng_bytes`]).
///
/// # Errors
/// Propagates I/O errors.
pub fn write_pcapng(path: impl AsRef<Path>, packets: &[Vec<u8>]) -> io::Result<()> {
    std::fs::write(path, pcapng_bytes(packets))
}

/// Parses the Enhanced Packet Blocks of a pcapng file (either byte order; other block
/// types are skipped).
///
/// # Errors
/// `InvalidData` on malformed content.
pub fn parse_pcapng(bytes: &[u8]) -> io::Result<Vec<Vec<u8>>> {
    let mut pos = 0;
    let mut le = true;
    let mut out = Vec::new();
    while pos < bytes.len() {
        let ty_raw = bytes.get(pos..pos + 4).ok_or_else(|| bad("truncated"))?;
        if ty_raw == [0x0a, 0x0d, 0x0d, 0x0a] {
            let bom = bytes
                .get(pos + 8..pos + 12)
                .ok_or_else(|| bad("truncated"))?;
            le = match bom {
                [0x4d, 0x3c, 0x2b, 0x1a] => true,
                [0x1a, 0x2b, 0x3c, 0x4d] => false,
                _ => return Err(bad("bad byte-order magic")),
            };
        } else if pos == 0 {
            return Err(bad("not a pcapng file"));
        }
        let r = Rd { b: bytes, le };
        let ty = r.u32(pos)?;
        let total = r.u32(pos + 4)? as usize;
        if total < 12 || total & 3 != 0 || pos + total > bytes.len() {
            return Err(bad("bad block length"));
        }
        if r.u32(pos + total - 4)? as usize != total {
            return Err(bad("block length trailer mismatch"));
        }
        if ty == 6 {
            let cap = r.u32(pos + 20)? as usize;
            let data = bytes
                .get(pos + 28..pos + 28 + cap)
                .filter(|_| 28 + cap + 4 <= total)
                .ok_or_else(|| bad("truncated EPB"))?;
            out.push(data.to_vec());
        }
        pos += total;
    }
    Ok(out)
}

/// Reads the packets of a pcapng file.
///
/// # Errors
/// I/O errors or malformed content (see [`parse_pcapng`]).
pub fn read_pcapng(path: impl AsRef<Path>) -> io::Result<Vec<Vec<u8>>> {
    parse_pcapng(&std::fs::read(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<Vec<u8>> {
        vec![vec![1, 2, 3], vec![], (0..=255).collect(), vec![9; 4]]
    }

    #[test]
    fn pcap_header_matches_spec() {
        let b = pcap_bytes(&[vec![0xaa, 0xbb, 0xcc]]);
        assert_eq!(&b[..4], &[0xd4, 0xc3, 0xb2, 0xa1], "LE magic 0xa1b2c3d4");
        assert_eq!(&b[4..8], &[2, 0, 4, 0], "version 2.4");
        assert_eq!(&b[8..16], &[0; 8], "thiszone + sigfigs");
        assert_eq!(&b[16..20], &[0xff, 0xff, 0, 0], "snaplen 65535");
        assert_eq!(&b[20..24], &[1, 0, 0, 0], "LINKTYPE_ETHERNET");
        assert_eq!(&b[24..32], &[0; 8], "ts");
        assert_eq!(&b[32..40], &[3, 0, 0, 0, 3, 0, 0, 0], "incl/orig len");
        assert_eq!(&b[40..], &[0xaa, 0xbb, 0xcc]);
        assert_eq!(b.len(), 24 + 16 + 3);
    }

    #[test]
    fn pcap_roundtrip_and_big_endian() {
        let p = sample();
        assert_eq!(parse_pcap(&pcap_bytes(&p)).unwrap(), p);
        // Hand-built big-endian file.
        let mut be = vec![0xa1, 0xb2, 0xc3, 0xd4, 0, 2, 0, 4];
        be.extend_from_slice(&[0; 8]);
        be.extend_from_slice(&[0, 0, 0xff, 0xff, 0, 0, 0, 1]);
        be.extend_from_slice(&[0; 8]);
        be.extend_from_slice(&[0, 0, 0, 2, 0, 0, 0, 2, 7, 8]);
        assert_eq!(parse_pcap(&be).unwrap(), vec![vec![7, 8]]);
        assert!(parse_pcap(b"junk").is_err());
        assert!(parse_pcap(&pcap_bytes(&p)[..30]).is_err());
    }

    #[test]
    fn pcapng_structure_and_roundtrip() {
        let p = sample();
        let b = pcapng_bytes(&p);
        assert_eq!(&b[..4], &[0x0a, 0x0d, 0x0d, 0x0a], "SHB type");
        assert_eq!(&b[4..8], &28u32.to_le_bytes(), "SHB length");
        assert_eq!(&b[8..12], &[0x4d, 0x3c, 0x2b, 0x1a], "byte-order magic");
        assert_eq!(&b[28..32], &1u32.to_le_bytes(), "IDB type");
        assert_eq!(&b[32..36], &20u32.to_le_bytes(), "IDB length");
        assert_eq!(&b[36..38], &[1, 0], "linktype Ethernet");
        assert_eq!(&b[48..52], &6u32.to_le_bytes(), "first EPB");
        assert_eq!(b.len() % 4, 0);
        assert_eq!(parse_pcapng(&b).unwrap(), p);
        assert!(parse_pcapng(b"nope").is_err());
        assert!(parse_pcapng(&b[..b.len() - 1]).is_err());
    }

    #[test]
    fn file_io() {
        let dir = std::env::temp_dir().join(format!("wpg-testutil-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = sample();
        write_pcap(dir.join("a.pcap"), &p).unwrap();
        write_pcapng(dir.join("a.pcapng"), &p).unwrap();
        assert_eq!(read_pcap(dir.join("a.pcap")).unwrap(), p);
        assert_eq!(read_pcapng(dir.join("a.pcapng")).unwrap(), p);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
