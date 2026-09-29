//! `pcapfile` backend: writes frames to a capture file instead of transmitting
//! (dry runs, sharing, CI tests).
//!
//! The `iface` argument of [`TxBackend::open`] is the output path. Files ending in
//! `.pcap` get the classic nanosecond pcap format (magic `a1b23c4d`); everything else
//! gets pcapng (SHB + one Ethernet IDB with `if_tsresol` = 10^-9 + one EPB per frame).
//! Timestamps are the wall-clock time taken at `open` plus the monotonic time elapsed
//! since then, so they are strictly ordered even if the system clock jumps.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::{Frame, TxBackend, TxError, TxOptions, TxStats};

/// `LINKTYPE_ETHERNET`.
const LINKTYPE_ETHERNET: u16 = 1;
/// Snap length written to the headers.
const SNAPLEN: u32 = 262_144;

/// Output file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcapFormat {
    /// pcapng (default).
    PcapNg,
    /// Classic pcap, nanosecond timestamps.
    Pcap,
}

impl PcapFormat {
    /// Format implied by the file extension (`.pcap` → classic, else pcapng).
    #[must_use]
    pub fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|e| e.to_str()) {
            Some(e) if e.eq_ignore_ascii_case("pcap") => PcapFormat::Pcap,
            _ => PcapFormat::PcapNg,
        }
    }
}

#[derive(Debug)]
struct Open {
    out: BufWriter<File>,
    path: PathBuf,
    format: PcapFormat,
    wall0_ns: u128,
    mono0: Instant,
}

/// Writes frames to a pcapng / pcap file.
#[derive(Debug, Default)]
pub struct PcapFileBackend {
    open: Option<Open>,
    stats: TxStats,
}

fn len_u32(n: usize) -> Result<u32, TxError> {
    u32::try_from(n).map_err(|_| TxError::Config(format!("frame of {n} octets is too large")))
}

fn pad4(n: usize) -> usize {
    (4 - n % 4) % 4
}

impl PcapFileBackend {
    /// Creates a closed backend.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates the backend already open on `path`.
    ///
    /// # Errors
    /// I/O errors creating the file.
    pub fn create(path: impl AsRef<Path>) -> Result<Self, TxError> {
        let mut b = Self::new();
        b.open_path(path.as_ref())?;
        Ok(b)
    }

    /// Output path while open.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.open.as_ref().map(|o| o.path.as_path())
    }

    fn open_path(&mut self, path: &Path) -> Result<(), TxError> {
        if self.open.is_some() {
            return Err(TxError::AlreadyOpen);
        }
        let format = PcapFormat::from_path(path);
        let file = File::create(path)?;
        let mut out = BufWriter::with_capacity(1 << 20, file);
        match format {
            PcapFormat::Pcap => {
                out.write_all(&0xa1b2_3c4du32.to_le_bytes())?;
                out.write_all(&2u16.to_le_bytes())?;
                out.write_all(&4u16.to_le_bytes())?;
                out.write_all(&0i32.to_le_bytes())?;
                out.write_all(&0u32.to_le_bytes())?;
                out.write_all(&SNAPLEN.to_le_bytes())?;
                out.write_all(&u32::from(LINKTYPE_ETHERNET).to_le_bytes())?;
            }
            PcapFormat::PcapNg => {
                // Section Header Block.
                let mut shb = Vec::with_capacity(16);
                shb.extend_from_slice(&0x1a2b_3c4du32.to_le_bytes());
                shb.extend_from_slice(&1u16.to_le_bytes());
                shb.extend_from_slice(&0u16.to_le_bytes());
                shb.extend_from_slice(&(-1i64).to_le_bytes());
                write_block(&mut out, 0x0a0d_0d0a, &shb)?;
                // Interface Description Block with if_tsresol = 9 (nanoseconds).
                let mut idb = Vec::with_capacity(20);
                idb.extend_from_slice(&LINKTYPE_ETHERNET.to_le_bytes());
                idb.extend_from_slice(&0u16.to_le_bytes());
                idb.extend_from_slice(&SNAPLEN.to_le_bytes());
                idb.extend_from_slice(&9u16.to_le_bytes()); // if_tsresol
                idb.extend_from_slice(&1u16.to_le_bytes());
                idb.extend_from_slice(&[9, 0, 0, 0]);
                idb.extend_from_slice(&[0, 0, 0, 0]); // opt_endofopt
                write_block(&mut out, 1, &idb)?;
            }
        }
        let wall0_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        self.open = Some(Open {
            out,
            path: path.to_owned(),
            format,
            wall0_ns,
            mono0: Instant::now(),
        });
        self.stats = TxStats::default();
        Ok(())
    }
}

fn write_block(out: &mut impl Write, ty: u32, body: &[u8]) -> std::io::Result<()> {
    let total = u32::try_from(12 + body.len() + pad4(body.len())).unwrap_or(u32::MAX);
    out.write_all(&ty.to_le_bytes())?;
    out.write_all(&total.to_le_bytes())?;
    out.write_all(body)?;
    out.write_all(&[0u8; 3][..pad4(body.len())])?;
    out.write_all(&total.to_le_bytes())
}

impl Open {
    fn write_frame(&mut self, data: &[u8]) -> Result<(), TxError> {
        let ts = self.wall0_ns + self.mono0.elapsed().as_nanos();
        let len = len_u32(data.len())?;
        match self.format {
            PcapFormat::Pcap => {
                let sec = u32::try_from(ts / 1_000_000_000).unwrap_or(u32::MAX);
                let nsec = (ts % 1_000_000_000) as u32;
                let o = &mut self.out;
                o.write_all(&sec.to_le_bytes())?;
                o.write_all(&nsec.to_le_bytes())?;
                o.write_all(&len.to_le_bytes())?;
                o.write_all(&len.to_le_bytes())?;
                o.write_all(data)?;
            }
            PcapFormat::PcapNg => {
                let ts = u64::try_from(ts).unwrap_or(u64::MAX);
                let pad = pad4(data.len());
                let total = len_u32(32 + data.len() + pad)?;
                let o = &mut self.out;
                o.write_all(&6u32.to_le_bytes())?;
                o.write_all(&total.to_le_bytes())?;
                o.write_all(&0u32.to_le_bytes())?; // interface id
                o.write_all(&((ts >> 32) as u32).to_le_bytes())?;
                o.write_all(&((ts & 0xffff_ffff) as u32).to_le_bytes())?;
                o.write_all(&len.to_le_bytes())?; // captured
                o.write_all(&len.to_le_bytes())?; // original
                o.write_all(data)?;
                o.write_all(&[0u8; 3][..pad])?;
                o.write_all(&total.to_le_bytes())?;
            }
        }
        Ok(())
    }
}

impl TxBackend for PcapFileBackend {
    fn name(&self) -> &'static str {
        "pcapfile"
    }

    fn open(&mut self, iface: &str, _opts: &TxOptions) -> Result<(), TxError> {
        if iface.is_empty() {
            return Err(TxError::Interface(
                "pcapfile needs an output path as its interface".to_owned(),
            ));
        }
        self.open_path(Path::new(iface))
    }

    fn send_batch(&mut self, frames: &[Frame]) -> Result<usize, TxError> {
        let open = self.open.as_mut().ok_or(TxError::NotOpen)?;
        for f in frames {
            open.write_frame(f.as_bytes())?;
            self.stats.packets += 1;
            self.stats.bytes += f.len() as u64;
        }
        self.stats.batches += 1;
        Ok(frames.len())
    }

    fn stats(&self) -> TxStats {
        self.stats
    }

    fn close(&mut self) -> Result<(), TxError> {
        if let Some(mut o) = self.open.take() {
            o.out.flush()?;
        }
        Ok(())
    }
}

impl Drop for PcapFileBackend {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wpg-tx-pcapfile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    fn frames() -> Vec<Frame> {
        vec![
            Frame::new((0..60u8).collect()),
            Frame::new(vec![0xab; 61]),
            Frame::new(vec![1; 1514]),
        ]
    }

    #[test]
    fn pcapng_readable_back() {
        let path = tmp("out.pcapng");
        let mut tx = PcapFileBackend::new();
        tx.open(path.to_str().unwrap(), &TxOptions::default())
            .unwrap();
        let f = frames();
        assert_eq!(tx.send_batch(&f).unwrap(), 3);
        assert_eq!(tx.send_batch(&f[..1]).unwrap(), 1);
        assert_eq!(tx.stats().packets, 4);
        tx.close().unwrap();
        let back = wpg_testutil::pcap::read_pcapng(&path).unwrap();
        assert_eq!(back.len(), 4);
        assert_eq!(back[0], f[0].as_bytes());
        assert_eq!(back[1], f[1].as_bytes());
        assert_eq!(back[2], f[2].as_bytes());
        assert_eq!(back[3], f[0].as_bytes());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn classic_pcap_by_extension_with_ordered_timestamps() {
        let path = tmp("out.pcap");
        let mut tx = PcapFileBackend::create(&path).unwrap();
        assert_eq!(tx.path(), Some(path.as_path()));
        let f = frames();
        tx.send_batch(&f).unwrap();
        drop(tx); // Drop flushes.
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..4], &0xa1b2_3c4du32.to_le_bytes());
        let back = wpg_testutil::pcap::parse_pcap(&bytes).unwrap();
        assert_eq!(back.len(), 3);
        assert_eq!(back[2], f[2].as_bytes());
        // Timestamps: record 0 at 24, record 1 at 24 + 16 + 60.
        let ts = |o: usize| {
            let s = u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
            let n = u32::from_le_bytes(bytes[o + 4..o + 8].try_into().unwrap());
            u64::from(s) * 1_000_000_000 + u64::from(n)
        };
        let t0 = ts(24);
        let t1 = ts(24 + 16 + 60);
        assert!(t1 >= t0);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let now = u64::try_from(now).unwrap();
        assert!(now >= t0 && now - t0 < 60_000_000_000);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn not_open_and_empty_path() {
        let mut tx = PcapFileBackend::new();
        assert!(matches!(tx.send_batch(&frames()), Err(TxError::NotOpen)));
        assert!(matches!(
            tx.open("", &TxOptions::default()),
            Err(TxError::Interface(_))
        ));
        tx.close().unwrap();
    }
}
