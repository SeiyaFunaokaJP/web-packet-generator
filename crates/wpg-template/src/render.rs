//! Rendering a compiled template to packets / a pcapng file (`wpg render`).

use std::io::Write;

use wpg_core::stream::Expansion;
use wpg_core::Resolve;

use crate::compile::{apply_overrides, Compiled};
use crate::pcapng::PcapngWriter;
use crate::TemplateError;

/// Receives `(packet index, frame bytes)` from [`render_with`].
pub type Sink<'a> = &'a mut dyn FnMut(u64, &[u8]) -> Result<(), TemplateError>;

/// Number of packets a render will produce: `count` if given, else the stream's own
/// count (`stream.count` / `tx.count`), else its natural length.
///
/// # Errors
/// [`TemplateError::Generator`] if the stream does not compile, and
/// [`TemplateError::Unbounded`] when it is unbounded and no count is known.
pub fn packet_count(c: &Compiled, count: Option<u64>) -> Result<u64, TemplateError> {
    let mut spec = c.stream.clone();
    if count.is_some() {
        spec.count = count;
    }
    Expansion::new(&spec, None)?
        .len()
        .ok_or(TemplateError::Unbounded)
}

/// Builds every packet of the stream and passes `(index, bytes)` to `sink`.
/// Returns the number of packets.
///
/// # Errors
/// See [`packet_count`]; also build errors and errors returned by `sink`.
pub fn render_with(
    c: &Compiled,
    resolve: &dyn Resolve,
    count: Option<u64>,
    sink: Sink<'_>,
) -> Result<u64, TemplateError> {
    let n = packet_count(c, count)?;
    let mut spec = c.stream.clone();
    spec.count = Some(n);
    let exp = Expansion::new(&spec, None)?;
    let mut packet = c.packet.clone();
    for i in 0..n {
        apply_overrides(&mut packet, &exp.nth(i)?)?;
        let built = packet.build(resolve)?;
        sink(i, &built.bytes)?;
    }
    Ok(n)
}

/// Renders the stream as pcapng (timestamps = packet index in microseconds).
/// Returns the number of packets.
///
/// # Errors
/// See [`render_with`].
pub fn render_pcapng<W: Write>(
    c: &Compiled,
    resolve: &dyn Resolve,
    count: Option<u64>,
    out: W,
) -> Result<u64, TemplateError> {
    let mut w = PcapngWriter::new(out)?;
    let n = render_with(c, resolve, count, &mut |i, bytes| {
        w.write_packet(i, bytes).map_err(TemplateError::Write)
    })?;
    w.finish()?;
    Ok(n)
}
