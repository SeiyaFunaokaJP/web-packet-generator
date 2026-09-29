//! Frame sources: where the TX loop gets its bytes from.
//!
//! * [`PreRendered`]: frames rendered up front, replayed (optionally cycled);
//! * [`OnTheFly`]: a base [`Packet`] plus a stream [`Expansion`]; each frame applies
//!   the next override set to a working copy of the packet and builds it;
//! * [`Rendered`]: picks one of the two from a [`Plan`] (TODO §2.1 "事前展開と
//!   オンザフライ生成の自動選択");
//! * [`Mixer`]: several sources mixed weighted / sequentially / round-robin (TODO
//!   §2.1 "ジョブは複数ストリームを重み付き・順次・ラウンドロビンで混合").
//!
//! # Override targets
//!
//! [`FieldRef::layer`] selects a layer of the base packet by
//! * protocol id: the first layer with that protocol (`ipv4`),
//! * protocol id with an occurrence index: `ipv4[1]` or `ipv4#1` (0-based, the second
//!   IPv4 layer, e.g. the inner header of a tunnel),
//! * plain layer index: `0`, `1`, ...

use std::collections::HashMap;
use std::sync::Arc;

use thiserror::Error;
use wpg_core::generator::{field_value_to_i128, GeneratorError};
use wpg_core::stream::{Expansion, FieldRef, Plan, PlanConfig, Strategy};
use wpg_core::{CoreError, FieldValue, Packet, Resolve};

/// Errors from building frame sources.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SourceError {
    /// A stream target names a layer that is not in the packet.
    #[error("stream target `{0}` does not match any layer of the packet")]
    UnknownLayer(String),
    /// Generator / expression evaluation failed.
    #[error(transparent)]
    Generator(#[from] GeneratorError),
    /// Building a packet failed.
    #[error(transparent)]
    Build(#[from] CoreError),
    /// The stream yields no packets.
    #[error("stream is empty")]
    Empty,
}

/// A producer of frames for the TX loop.
pub trait FrameSource: Send {
    /// Writes the next frame into `buf` (replacing its contents, reusing its
    /// allocation where possible) and returns its length; `None` when exhausted
    /// (or failed, see [`FrameSource::take_error`]).
    fn next_frame(&mut self, buf: &mut Vec<u8>) -> Option<usize>;

    /// Frames left, `None` when unbounded / unknown.
    fn remaining(&self) -> Option<u64>;

    /// Index of the stream that produced the last frame (for per-stream counters;
    /// `0` for single-stream sources).
    fn stream_index(&self) -> usize {
        0
    }

    /// Number of streams (per-stream counter slots).
    fn stream_count(&self) -> usize {
        1
    }

    /// The error that ended the source, if `next_frame` returned `None` because of
    /// a failure rather than exhaustion.
    fn take_error(&mut self) -> Option<SourceError> {
        None
    }
}

impl<T: FrameSource + ?Sized> FrameSource for Box<T> {
    fn next_frame(&mut self, buf: &mut Vec<u8>) -> Option<usize> {
        (**self).next_frame(buf)
    }
    fn remaining(&self) -> Option<u64> {
        (**self).remaining()
    }
    fn stream_index(&self) -> usize {
        (**self).stream_index()
    }
    fn stream_count(&self) -> usize {
        (**self).stream_count()
    }
    fn take_error(&mut self) -> Option<SourceError> {
        (**self).take_error()
    }
}

/// Pre-rendered frames, replayed in order.
#[derive(Debug, Clone)]
pub struct PreRendered {
    frames: Vec<Vec<u8>>,
    /// Frames still to emit (`None` = forever).
    left: Option<u64>,
    pos: usize,
}

impl PreRendered {
    /// Emits `frames` `repeat` times in order (`None` = loop forever).
    #[must_use]
    pub fn new(frames: Vec<Vec<u8>>, repeat: Option<u64>) -> Self {
        let n = frames.len() as u64;
        let left = if n == 0 {
            Some(0)
        } else {
            repeat.map(|r| r.saturating_mul(n))
        };
        Self {
            frames,
            left,
            pos: 0,
        }
    }

    /// Emits exactly `total` frames cycling through `frames` (`None` = forever).
    #[must_use]
    pub fn cycled(frames: Vec<Vec<u8>>, total: Option<u64>) -> Self {
        let left = if frames.is_empty() { Some(0) } else { total };
        Self {
            frames,
            left,
            pos: 0,
        }
    }

    /// The distinct frames.
    #[must_use]
    pub fn frames(&self) -> &[Vec<u8>] {
        &self.frames
    }

    /// Consumes the source, returning the distinct frames.
    #[must_use]
    pub fn into_frames(self) -> Vec<Vec<u8>> {
        self.frames
    }
}

impl FrameSource for PreRendered {
    fn next_frame(&mut self, buf: &mut Vec<u8>) -> Option<usize> {
        match &mut self.left {
            Some(0) => return None,
            Some(n) => *n -= 1,
            None => {}
        }
        let f = &self.frames[self.pos];
        self.pos += 1;
        if self.pos == self.frames.len() {
            self.pos = 0;
        }
        buf.clear();
        buf.extend_from_slice(f);
        Some(f.len())
    }

    fn remaining(&self) -> Option<u64> {
        self.left
    }
}

/// A protocol catalog shared with the TX thread.
#[derive(Clone)]
pub enum CatalogRef {
    /// A `'static` catalog such as `wpg_protocols::catalog()`.
    Static(&'static (dyn Resolve + Send + Sync)),
    /// A reference-counted catalog (e.g. built-ins plus user definitions).
    Shared(Arc<dyn Resolve + Send + Sync>),
}

impl CatalogRef {
    /// The catalog.
    #[must_use]
    pub fn get(&self) -> &dyn Resolve {
        match self {
            CatalogRef::Static(c) => *c,
            CatalogRef::Shared(c) => &**c,
        }
    }
}

impl std::fmt::Debug for CatalogRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CatalogRef::Static(_) => "CatalogRef::Static",
            CatalogRef::Shared(_) => "CatalogRef::Shared",
        })
    }
}

impl<T: Resolve + Send + Sync + 'static> From<&'static T> for CatalogRef {
    fn from(c: &'static T) -> Self {
        CatalogRef::Static(c)
    }
}

impl<T: Resolve + Send + Sync + 'static> From<Arc<T>> for CatalogRef {
    fn from(c: Arc<T>) -> Self {
        CatalogRef::Shared(c)
    }
}

/// Resolves a [`FieldRef::layer`] to a layer index (see the module docs).
#[must_use]
pub fn resolve_layer(packet: &Packet, layer: &str) -> Option<usize> {
    if let Ok(i) = layer.parse::<usize>() {
        return (i < packet.layers.len()).then_some(i);
    }
    let (proto, nth) = match layer
        .strip_suffix(']')
        .and_then(|s| s.rsplit_once('['))
        .or_else(|| layer.rsplit_once('#'))
    {
        Some((p, n)) => (p, n.parse::<usize>().ok()?),
        None => (layer, 0),
    };
    packet
        .layers
        .iter()
        .enumerate()
        .filter(|(_, l)| l.protocol == proto)
        .nth(nth)
        .map(|(i, _)| i)
}

/// Applies `targets[i] = values[i]` to `packet` (Manual values), resolving layers as
/// described in the module docs.
///
/// # Errors
/// [`SourceError::UnknownLayer`] for a target that matches no layer.
pub fn apply_overrides(
    packet: &mut Packet,
    targets: &[FieldRef],
    values: &[FieldValue],
) -> Result<(), SourceError> {
    for (t, v) in targets.iter().zip(values) {
        let li = resolve_layer(packet, &t.layer)
            .ok_or_else(|| SourceError::UnknownLayer(t.to_string()))?;
        set_field(&mut packet.layers[li].fields, &t.field, v.clone());
    }
    Ok(())
}

fn set_field(
    fields: &mut std::collections::BTreeMap<String, FieldValue>,
    name: &str,
    v: FieldValue,
) {
    // `get_mut` first: after the first frame the key exists, so no String allocation.
    if let Some(slot) = fields.get_mut(name) {
        *slot = v;
    } else {
        fields.insert(name.to_owned(), v);
    }
}

/// On-the-fly frame generation from a base packet and a stream expansion.
#[derive(Debug)]
pub struct OnTheFly {
    work: Packet,
    exp: Expansion,
    catalog: CatalogRef,
    /// Layer index per expansion target.
    slots: Vec<usize>,
    values: Vec<FieldValue>,
    /// Numeric Manual fields of the base packet for `$field` references in `expr`
    /// generators (`field` for the first occurrence and `layer.field`).
    base_fields: HashMap<String, i128>,
    index: u64,
    end: Option<u64>,
    /// Stream length when cycling.
    cycle: Option<u64>,
    error: Option<SourceError>,
}

impl OnTheFly {
    /// Creates the source. Every packet of the expansion is produced once, in order.
    ///
    /// # Errors
    /// [`SourceError::UnknownLayer`] if a stream target does not match a layer.
    pub fn new(
        packet: Packet,
        exp: Expansion,
        catalog: impl Into<CatalogRef>,
    ) -> Result<Self, SourceError> {
        let slots = exp
            .targets()
            .iter()
            .map(|t| {
                resolve_layer(&packet, &t.layer)
                    .ok_or_else(|| SourceError::UnknownLayer(t.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut base_fields = HashMap::new();
        for layer in &packet.layers {
            for (name, v) in &layer.fields {
                if let Some(x) = field_value_to_i128(v) {
                    base_fields.entry(name.clone()).or_insert(x);
                    base_fields
                        .entry(format!("{}.{name}", layer.protocol))
                        .or_insert(x);
                }
            }
        }
        let end = exp.len();
        Ok(Self {
            work: packet,
            exp,
            catalog: catalog.into(),
            slots,
            values: Vec::new(),
            base_fields,
            index: 0,
            end,
            cycle: None,
            error: None,
        })
    }

    /// Restricts the source to packet indices `range` (for partitioned workers).
    #[must_use]
    pub fn with_range(mut self, range: std::ops::Range<u64>) -> Self {
        self.index = range.start;
        self.end = Some(self.end.map_or(range.end, |e| e.min(range.end)));
        self
    }

    /// Loops over the stream forever (packet index modulo the stream length; an
    /// unbounded stream is unchanged). Use with a job count / duration limit.
    #[must_use]
    pub fn cycling(mut self) -> Self {
        self.cycle = self.exp.len().filter(|n| *n > 0);
        self.end = None;
        self
    }

    /// Builds packet `index` into `buf`.
    ///
    /// # Errors
    /// Generator or build errors.
    pub fn render(&mut self, index: u64, buf: &mut Vec<u8>) -> Result<usize, SourceError> {
        let base = &self.base_fields;
        let lookup = |name: &str| base.get(name).copied();
        self.exp.values_at(index, &lookup, &mut self.values)?;
        for ((slot, target), v) in self
            .slots
            .iter()
            .zip(self.exp.targets())
            .zip(self.values.drain(..))
        {
            set_field(&mut self.work.layers[*slot].fields, &target.field, v);
        }
        let mut built = self.work.build(self.catalog.get())?;
        std::mem::swap(buf, &mut built.bytes);
        Ok(buf.len())
    }

    /// Renders all packets (for pre-rendering).
    ///
    /// # Errors
    /// [`SourceError::Empty`] for an unbounded stream; generator / build errors.
    pub fn render_all(&mut self) -> Result<Vec<Vec<u8>>, SourceError> {
        let n = self.end.ok_or(SourceError::Empty)?;
        let mut out = Vec::with_capacity(usize::try_from(n - self.index).unwrap_or(0));
        for i in self.index..n {
            let mut b = Vec::new();
            self.render(i, &mut b)?;
            out.push(b);
        }
        Ok(out)
    }
}

impl FrameSource for OnTheFly {
    fn next_frame(&mut self, buf: &mut Vec<u8>) -> Option<usize> {
        if self.error.is_some() || self.end.is_some_and(|e| self.index >= e) {
            return None;
        }
        let idx = self.cycle.map_or(self.index, |n| self.index % n);
        match self.render(idx, buf) {
            Ok(n) => {
                self.index += 1;
                Some(n)
            }
            Err(e) => {
                self.error = Some(e);
                None
            }
        }
    }

    fn remaining(&self) -> Option<u64> {
        self.end.map(|e| e.saturating_sub(self.index))
    }

    fn take_error(&mut self) -> Option<SourceError> {
        self.error.take()
    }
}

/// A single stream rendered per a [`Plan`].
#[derive(Debug)]
pub enum Rendered {
    /// Pre-rendered frames.
    Pre(PreRendered),
    /// Generated while sending.
    Fly(Box<OnTheFly>),
}

impl Rendered {
    /// Creates the source following `plan.strategy`.
    ///
    /// # Errors
    /// Layer resolution, generator or build errors (pre-rendering builds every
    /// frame immediately).
    pub fn from_plan(
        packet: Packet,
        exp: Expansion,
        catalog: impl Into<CatalogRef>,
        plan: &Plan,
    ) -> Result<Self, SourceError> {
        let mut fly = OnTheFly::new(packet, exp, catalog)?;
        match plan.strategy {
            Strategy::PreRender if fly.end.is_some() => {
                let frames = fly.render_all()?;
                Ok(Rendered::Pre(PreRendered::new(frames, Some(1))))
            }
            _ => Ok(Rendered::Fly(Box::new(fly))),
        }
    }

    /// Renders the first frame to learn the frame length, decides with
    /// [`Expansion::plan`] and creates the source. Returns the plan too.
    ///
    /// # Errors
    /// See [`Self::from_plan`].
    pub fn auto(
        packet: Packet,
        exp: Expansion,
        catalog: impl Into<CatalogRef>,
        cfg: &PlanConfig,
    ) -> Result<(Self, Plan), SourceError> {
        let catalog = catalog.into();
        let mut probe = OnTheFly::new(packet.clone(), exp.clone(), catalog.clone())?;
        let mut buf = Vec::new();
        let frame_len = if exp.is_empty() {
            0
        } else {
            probe.render(0, &mut buf)?
        };
        let plan = exp.plan(frame_len, cfg);
        Ok((Self::from_plan(packet, exp, catalog, &plan)?, plan))
    }
}

impl FrameSource for Rendered {
    fn next_frame(&mut self, buf: &mut Vec<u8>) -> Option<usize> {
        match self {
            Rendered::Pre(p) => p.next_frame(buf),
            Rendered::Fly(f) => f.next_frame(buf),
        }
    }

    fn remaining(&self) -> Option<u64> {
        match self {
            Rendered::Pre(p) => p.remaining(),
            Rendered::Fly(f) => f.remaining(),
        }
    }

    fn take_error(&mut self) -> Option<SourceError> {
        match self {
            Rendered::Pre(p) => p.take_error(),
            Rendered::Fly(f) => f.take_error(),
        }
    }
}

/// How a [`Mixer`] interleaves its streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MixMode {
    /// Smooth weighted round-robin: stream `i` gets `weight_i / Σ weight` of the
    /// frames, interleaved as evenly as possible (deterministic).
    #[default]
    Weighted,
    /// Exhaust stream 0, then stream 1, ...
    Sequential,
    /// One frame from each stream in turn (weights ignored).
    RoundRobin,
}

struct MixEntry {
    source: Box<dyn FrameSource>,
    weight: u32,
    current: i64,
    done: bool,
}

/// Mixes several frame sources into one.
pub struct Mixer {
    streams: Vec<MixEntry>,
    mode: MixMode,
    cursor: usize,
    last: usize,
    error: Option<SourceError>,
}

impl std::fmt::Debug for Mixer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mixer")
            .field("streams", &self.streams.len())
            .field("mode", &self.mode)
            .field("cursor", &self.cursor)
            .finish_non_exhaustive()
    }
}

impl Mixer {
    /// An empty mixer.
    #[must_use]
    pub fn new(mode: MixMode) -> Self {
        Self {
            streams: Vec::new(),
            mode,
            cursor: 0,
            last: 0,
            error: None,
        }
    }

    /// Adds a stream (`weight` 0 is treated as 1).
    #[must_use]
    pub fn with(mut self, source: Box<dyn FrameSource>, weight: u32) -> Self {
        self.push(source, weight);
        self
    }

    /// Adds a stream (`weight` 0 is treated as 1).
    pub fn push(&mut self, source: Box<dyn FrameSource>, weight: u32) {
        self.streams.push(MixEntry {
            source,
            weight: weight.max(1),
            current: 0,
            done: false,
        });
    }

    fn pull(&mut self, i: usize, buf: &mut Vec<u8>) -> Option<usize> {
        let e = &mut self.streams[i];
        match e.source.next_frame(buf) {
            Some(n) => {
                self.last = i;
                Some(n)
            }
            None => {
                e.done = true;
                if let Some(err) = e.source.take_error() {
                    self.error.get_or_insert(err);
                }
                None
            }
        }
    }
}

impl FrameSource for Mixer {
    fn next_frame(&mut self, buf: &mut Vec<u8>) -> Option<usize> {
        if self.error.is_some() {
            return None;
        }
        let n = self.streams.len();
        loop {
            if self.error.is_some() || self.streams.iter().all(|e| e.done) {
                return None;
            }
            let pick = match self.mode {
                MixMode::Sequential => {
                    while self.cursor < n && self.streams[self.cursor].done {
                        self.cursor += 1;
                    }
                    self.cursor
                }
                MixMode::RoundRobin => {
                    let mut i = self.cursor % n;
                    while self.streams[i].done {
                        i = (i + 1) % n;
                    }
                    self.cursor = (i + 1) % n;
                    i
                }
                MixMode::Weighted => {
                    let total: i64 = self
                        .streams
                        .iter()
                        .filter(|e| !e.done)
                        .map(|e| i64::from(e.weight))
                        .sum();
                    let mut best = usize::MAX;
                    let mut best_current = i64::MIN;
                    for (i, e) in self.streams.iter_mut().enumerate() {
                        if e.done {
                            continue;
                        }
                        e.current += i64::from(e.weight);
                        if best == usize::MAX || e.current > best_current {
                            best = i;
                            best_current = e.current;
                        }
                    }
                    self.streams[best].current -= total;
                    best
                }
            };
            if let Some(len) = self.pull(pick, buf) {
                return Some(len);
            }
        }
    }

    fn remaining(&self) -> Option<u64> {
        self.streams
            .iter()
            .filter(|e| !e.done)
            .try_fold(0u64, |a, e| {
                e.source.remaining().map(|r| a.saturating_add(r))
            })
    }

    fn stream_index(&self) -> usize {
        self.last
    }

    fn stream_count(&self) -> usize {
        self.streams.len().max(1)
    }

    fn take_error(&mut self) -> Option<SourceError> {
        self.error.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use wpg_core::stream::{Assignment, Mode, StreamSpec};
    use wpg_core::{FieldType, Generator, Layer};

    fn drain(src: &mut dyn FrameSource, max: usize) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let mut buf = Vec::new();
        while out.len() < max {
            match src.next_frame(&mut buf) {
                Some(n) => {
                    assert_eq!(n, buf.len());
                    out.push(buf.clone());
                }
                None => break,
            }
        }
        out
    }

    fn base_packet() -> Packet {
        Packet::from_layers(vec![
            Layer::new("ethernet2")
                .with("dst", FieldValue::Mac([0xff; 6]))
                .with("src", FieldValue::Mac([2, 0, 0, 0, 0, 1])),
            Layer::new("ipv4")
                .with("src", FieldValue::Ipv4(Ipv4Addr::new(192, 0, 2, 1)))
                .with("dst", FieldValue::Ipv4(Ipv4Addr::new(198, 51, 100, 1))),
            Layer::new("udp").with("dst_port", FieldValue::UInt(9)),
        ])
    }

    fn spec() -> StreamSpec {
        StreamSpec {
            assignments: vec![
                Assignment {
                    target: FieldRef::new("ipv4", "src"),
                    ty: FieldType::Ipv4,
                    generator: Generator::List {
                        values: vec!["10.0.0.1".into(), "10.0.0.2".into(), "10.0.0.3".into()],
                    },
                },
                Assignment {
                    target: FieldRef::new("udp", "src_port"),
                    ty: FieldType::UInt { bits: 16 },
                    generator: Generator::List {
                        values: vec!["1000".into(), "2000".into()],
                    },
                },
            ],
            mode: Mode::Product,
            count: None,
            seed: 1,
        }
    }

    fn catalog() -> &'static wpg_protocols::Catalog {
        wpg_protocols::catalog()
    }

    #[test]
    fn prerendered_repeat_and_cycle() {
        let frames = vec![vec![1u8; 60], vec![2u8; 61]];
        let mut p = PreRendered::new(frames.clone(), Some(2));
        assert_eq!(p.remaining(), Some(4));
        let out = drain(&mut p, 100);
        assert_eq!(
            out,
            vec![
                frames[0].clone(),
                frames[1].clone(),
                frames[0].clone(),
                frames[1].clone()
            ]
        );
        let mut c = PreRendered::cycled(frames.clone(), Some(3));
        assert_eq!(drain(&mut c, 100).len(), 3);
        let mut inf = PreRendered::new(frames, None);
        assert_eq!(inf.remaining(), None);
        assert_eq!(drain(&mut inf, 1000).len(), 1000);
        assert_eq!(drain(&mut PreRendered::new(Vec::new(), None), 10).len(), 0);
    }

    #[test]
    fn on_the_fly_matches_prerender_and_manual_build() {
        let exp = Expansion::new(&spec(), None).unwrap();
        assert_eq!(exp.len(), Some(6));
        let mut fly = OnTheFly::new(base_packet(), exp.clone(), catalog()).unwrap();
        let a = drain(&mut fly, 100);
        assert_eq!(a.len(), 6);
        assert!(fly.take_error().is_none());

        let plan = Plan::decide(Some(6), 60, &PlanConfig::default());
        assert_eq!(plan.strategy, Strategy::PreRender);
        let mut pre = Rendered::from_plan(base_packet(), exp.clone(), catalog(), &plan).unwrap();
        assert!(matches!(pre, Rendered::Pre(_)));
        let b = drain(&mut pre, 100);
        assert_eq!(a, b);

        let forced = Plan::decide(
            Some(6),
            60,
            &PlanConfig {
                hint: wpg_core::stream::PlanHint::OnTheFly,
                ..PlanConfig::default()
            },
        );
        let mut f2 = Rendered::from_plan(base_packet(), exp.clone(), catalog(), &forced).unwrap();
        assert!(matches!(f2, Rendered::Fly(_)));
        assert_eq!(drain(&mut f2, 100), a);

        // Reference: apply overrides by hand and build.
        for (i, frame) in a.iter().enumerate() {
            let ov = exp.nth(i as u64).unwrap();
            let mut p = base_packet();
            let (t, v): (Vec<_>, Vec<_>) = ov.into_iter().unzip();
            apply_overrides(&mut p, &t, &v).unwrap();
            assert_eq!(&p.build(catalog()).unwrap().bytes, frame);
        }
        // Frames differ in the IPv4 source (offset 14 + 12).
        assert_eq!(&a[0][26..30], &[10, 0, 0, 1]);
        assert_eq!(&a[2][26..30], &[10, 0, 0, 2]);
        assert_ne!(a[0], a[1]);

        let (auto, plan) =
            Rendered::auto(base_packet(), exp, catalog(), &PlanConfig::default()).unwrap();
        assert_eq!(plan.strategy, Strategy::PreRender);
        assert_eq!(auto.remaining(), Some(6));
    }

    #[test]
    fn on_the_fly_range_and_unknown_layer() {
        let exp = Expansion::new(&spec(), None).unwrap();
        let mut fly = OnTheFly::new(base_packet(), exp.clone(), catalog())
            .unwrap()
            .with_range(2..4);
        assert_eq!(fly.remaining(), Some(2));
        assert_eq!(drain(&mut fly, 10).len(), 2);

        let all = drain(
            &mut OnTheFly::new(base_packet(), exp.clone(), catalog()).unwrap(),
            100,
        );
        let mut cyc = OnTheFly::new(base_packet(), exp.clone(), catalog())
            .unwrap()
            .cycling();
        assert_eq!(cyc.remaining(), None);
        let looped = drain(&mut cyc, 14);
        assert_eq!(&looped[..6], &all[..]);
        assert_eq!(&looped[6..12], &all[..]);

        let mut s = spec();
        s.assignments[0].target = FieldRef::new("ipv6", "src");
        let exp = Expansion::new(&s, None).unwrap();
        assert!(matches!(
            OnTheFly::new(base_packet(), exp, catalog()),
            Err(SourceError::UnknownLayer(_))
        ));
    }

    #[test]
    fn layer_resolution() {
        let p = Packet::from_layers(vec![
            Layer::new("ethernet2"),
            Layer::new("ipv4"),
            Layer::new("ipv4"),
        ]);
        assert_eq!(resolve_layer(&p, "ipv4"), Some(1));
        assert_eq!(resolve_layer(&p, "ipv4[1]"), Some(2));
        assert_eq!(resolve_layer(&p, "ipv4#1"), Some(2));
        assert_eq!(resolve_layer(&p, "ipv4[2]"), None);
        assert_eq!(resolve_layer(&p, "0"), Some(0));
        assert_eq!(resolve_layer(&p, "3"), None);
        assert_eq!(resolve_layer(&p, "tcp"), None);
    }

    fn tagged(tag: u8, n: u64) -> Box<dyn FrameSource> {
        Box::new(PreRendered::new(vec![vec![tag; 60]], Some(n)))
    }

    fn tags(m: &mut Mixer, max: usize) -> Vec<(u8, usize)> {
        let mut out = Vec::new();
        let mut buf = Vec::new();
        while out.len() < max {
            if m.next_frame(&mut buf).is_none() {
                break;
            }
            out.push((buf[0], m.stream_index()));
        }
        out
    }

    #[test]
    fn mixer_sequential() {
        let mut m = Mixer::new(MixMode::Sequential)
            .with(tagged(1, 2), 1)
            .with(tagged(2, 3), 1);
        assert_eq!(m.remaining(), Some(5));
        assert_eq!(m.stream_count(), 2);
        let t: Vec<u8> = tags(&mut m, 100).into_iter().map(|x| x.0).collect();
        assert_eq!(t, vec![1, 1, 2, 2, 2]);
    }

    #[test]
    fn mixer_round_robin_skips_exhausted() {
        let mut m = Mixer::new(MixMode::RoundRobin)
            .with(tagged(1, 1), 5)
            .with(tagged(2, 3), 1)
            .with(tagged(3, 2), 1);
        let t = tags(&mut m, 100);
        let only: Vec<u8> = t.iter().map(|x| x.0).collect();
        assert_eq!(only, vec![1, 2, 3, 2, 3, 2]);
        // stream_index matches the tag.
        assert!(t.iter().all(|(tag, idx)| usize::from(*tag) == idx + 1));
    }

    #[test]
    fn mixer_weighted_is_proportional_and_smooth() {
        let mut m = Mixer::new(MixMode::Weighted)
            .with(Box::new(PreRendered::new(vec![vec![1; 60]], None)), 3)
            .with(Box::new(PreRendered::new(vec![vec![2; 60]], None)), 1);
        assert_eq!(m.remaining(), None);
        let t: Vec<u8> = tags(&mut m, 400).into_iter().map(|x| x.0).collect();
        assert_eq!(t.iter().filter(|&&x| x == 1).count(), 300);
        // Smooth: never more than 3 of stream 1 in a row.
        assert!(!t.windows(4).any(|w| w.iter().all(|&x| x == 1)));
        assert_eq!(&t[..4], &[1, 1, 2, 1]);

        // Exhausted streams drop out.
        let mut m = Mixer::new(MixMode::Weighted)
            .with(tagged(1, 2), 1)
            .with(tagged(2, 5), 1);
        let t: Vec<u8> = tags(&mut m, 100).into_iter().map(|x| x.0).collect();
        assert_eq!(t.len(), 7);
        assert_eq!(t.iter().filter(|&&x| x == 2).count(), 5);
    }
}
