//! Minimal pcapng writer (SHB + IDB + EPB), microsecond timestamps.

use std::io::{self, Write};

/// Streaming pcapng writer for Ethernet frames.
#[derive(Debug)]
pub struct PcapngWriter<W: Write> {
    out: W,
}

const LINKTYPE_ETHERNET: u16 = 1;
const SNAPLEN: u32 = 262_144;

impl<W: Write> PcapngWriter<W> {
    /// Writes the section header and the interface description block.
    ///
    /// # Errors
    /// Errors of the underlying writer.
    pub fn new(mut out: W) -> io::Result<Self> {
        // Section Header Block
        out.write_all(&0x0A0D_0D0Au32.to_le_bytes())?;
        out.write_all(&28u32.to_le_bytes())?;
        out.write_all(&0x1A2B_3C4Du32.to_le_bytes())?;
        out.write_all(&1u16.to_le_bytes())?; // major
        out.write_all(&0u16.to_le_bytes())?; // minor
        out.write_all(&(-1i64).to_le_bytes())?; // section length unknown
        out.write_all(&28u32.to_le_bytes())?;
        // Interface Description Block (default if_tsresol = microseconds)
        out.write_all(&1u32.to_le_bytes())?;
        out.write_all(&20u32.to_le_bytes())?;
        out.write_all(&LINKTYPE_ETHERNET.to_le_bytes())?;
        out.write_all(&0u16.to_le_bytes())?;
        out.write_all(&SNAPLEN.to_le_bytes())?;
        out.write_all(&20u32.to_le_bytes())?;
        Ok(Self { out })
    }

    /// Appends one Enhanced Packet Block with timestamp `ts_us` (microseconds).
    ///
    /// # Errors
    /// Errors of the underlying writer, or a frame longer than 4 GiB.
    #[allow(clippy::cast_possible_truncation)] // the 64-bit timestamp is split on purpose
    pub fn write_packet(&mut self, ts_us: u64, data: &[u8]) -> io::Result<()> {
        let len = u32::try_from(data.len())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "frame too long"))?;
        let padded = data.len().div_ceil(4) * 4;
        let total = u32::try_from(32 + padded)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "frame too long"))?;
        self.out.write_all(&6u32.to_le_bytes())?;
        self.out.write_all(&total.to_le_bytes())?;
        self.out.write_all(&0u32.to_le_bytes())?; // interface 0
        let (hi, lo) = ((ts_us >> 32) as u32, (ts_us & 0xffff_ffff) as u32);
        self.out.write_all(&hi.to_le_bytes())?;
        self.out.write_all(&lo.to_le_bytes())?;
        self.out.write_all(&len.to_le_bytes())?;
        self.out.write_all(&len.to_le_bytes())?;
        self.out.write_all(data)?;
        self.out.write_all(&[0u8; 3][..padded - data.len()])?;
        self.out.write_all(&total.to_le_bytes())
    }

    /// Flushes and returns the writer.
    ///
    /// # Errors
    /// Errors of the underlying writer.
    pub fn finish(mut self) -> io::Result<W> {
        self.out.flush()?;
        Ok(self.out)
    }
}
