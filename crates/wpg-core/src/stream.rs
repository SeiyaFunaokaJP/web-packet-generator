//! Stream expansion: combines per-field [`Generator`]s into per-packet override sets.
//!
//! Overrides are plain data (`Vec<(FieldRef, FieldValue)>`); this module does not
//! depend on the packet build API. Every mode supports O(1) random access
//! ([`Expansion::nth`]) so a multi-threaded TX can split `0..len` with
//! [`partition`].

use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::generator::{
    CompileOptions, CompiledGenerator, EvalCtx, FieldLookup, FileProvider, Generator,
    GeneratorError, NoFields,
};
use crate::ir::FieldType;
use crate::packet::FieldValue;

/// Addresses one field of one layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FieldRef {
    /// Layer (protocol id or layer name).
    pub layer: String,
    /// Field name.
    pub field: String,
}

impl FieldRef {
    /// Creates a reference.
    #[must_use]
    pub fn new(layer: impl Into<String>, field: impl Into<String>) -> Self {
        FieldRef {
            layer: layer.into(),
            field: field.into(),
        }
    }

    /// Parses `layer.field` (split at the last dot).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let (l, f) = s.rsplit_once('.')?;
        (!l.is_empty() && !f.is_empty()).then(|| FieldRef::new(l, f))
    }
}

impl std::fmt::Display for FieldRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.layer, self.field)
    }
}

/// One generator assigned to one field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assignment {
    /// Target field.
    pub target: FieldRef,
    /// Type of the target field (from the protocol definition).
    pub ty: FieldType,
    /// The generator.
    pub generator: Generator,
}

/// How several generators are combined.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum Mode {
    /// Advance all in lockstep; shorter generators cycle. Length = longest.
    #[default]
    Zip,
    /// Cartesian product; the last assignment varies fastest (odometer order).
    Product,
    /// Odometer with explicit priority: `order` lists fields from the slowest
    /// (outermost) to the fastest (innermost). Unlisted fields become the fastest,
    /// in declaration order.
    Nested {
        /// Slowest to fastest.
        order: Vec<FieldRef>,
    },
}

/// Declarative description of a stream's variation.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct StreamSpec {
    /// Field generators.
    pub assignments: Vec<Assignment>,
    /// Combination mode.
    #[serde(default)]
    pub mode: Mode,
    /// Explicit packet count; overrides the natural length (finite generators cycle,
    /// unbounded ones make this mandatory).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    /// Seed for `random` / `rand()`.
    #[serde(default)]
    pub seed: u64,
}

/// Overrides for one packet.
pub type Overrides = Vec<(FieldRef, FieldValue)>;

/// A compiled stream: random-access packet-override generator.
#[derive(Debug, Clone)]
pub struct Expansion {
    targets: Vec<FieldRef>,
    gens: Vec<CompiledGenerator>,
    cards: Vec<Option<u128>>,
    /// Indices into `gens`, slowest first (product / nested only).
    order: Vec<usize>,
    zip: bool,
    count: Option<u64>,
    total: Option<u64>,
}

fn sat(x: u128) -> u64 {
    u64::try_from(x).unwrap_or(u64::MAX)
}

impl Expansion {
    /// Compiles `spec`. `files` resolves `file` generators.
    ///
    /// # Errors
    /// Any [`GeneratorError`] from compiling generators, an unknown field in a nested
    /// order, or an unbounded generator in an inner product position.
    pub fn new(
        spec: &StreamSpec,
        files: Option<&dyn FileProvider>,
    ) -> Result<Expansion, GeneratorError> {
        let mut targets = Vec::new();
        let mut gens = Vec::new();
        for (i, a) in spec.assignments.iter().enumerate() {
            let opts = CompileOptions {
                seed: spec.seed,
                salt: i as u64 + 1,
                files,
            };
            gens.push(a.generator.compile(&a.ty, &opts)?);
            targets.push(a.target.clone());
        }
        let cards: Vec<Option<u128>> = gens
            .iter()
            .map(CompiledGenerator::cardinality_exact)
            .collect();
        let n = gens.len();

        let (zip, order) = match &spec.mode {
            Mode::Zip => (true, Vec::new()),
            Mode::Product => (false, (0..n).collect()),
            Mode::Nested { order } => {
                let mut o = Vec::with_capacity(n);
                for r in order {
                    let idx = targets
                        .iter()
                        .position(|t| t == r)
                        .ok_or_else(|| GeneratorError::UnknownOrderField(r.to_string()))?;
                    if !o.contains(&idx) {
                        o.push(idx);
                    }
                }
                let rest: Vec<usize> = (0..n).filter(|i| !o.contains(i)).collect();
                o.extend(rest);
                (false, o)
            }
        };
        if !zip {
            // Only the outermost digit may be unbounded.
            if order.iter().skip(1).any(|&g| cards[g].is_none()) {
                return Err(GeneratorError::InfiniteInner);
            }
        }

        let total = if zip {
            if n == 0 {
                Some(1)
            } else {
                cards.iter().flatten().max().map(|c| sat(*c))
            }
        } else if cards.iter().any(Option::is_none) {
            None
        } else {
            Some(sat(cards
                .iter()
                .flatten()
                .fold(1u128, |a, c| a.saturating_mul(*c))))
        };

        Ok(Expansion {
            targets,
            gens,
            cards,
            order,
            zip,
            count: spec.count,
            total,
        })
    }

    /// Targets in assignment order (matches [`Self::values_at`] output order).
    #[must_use]
    pub fn targets(&self) -> &[FieldRef] {
        &self.targets
    }

    /// Natural size ignoring `count`: `None` when unbounded.
    #[must_use]
    pub fn total_cardinality(&self) -> Option<u64> {
        self.total
    }

    /// Number of packets: `count` if set, else [`Self::total_cardinality`]; `None` =
    /// unbounded.
    #[must_use]
    pub fn len(&self) -> Option<u64> {
        self.count.or(self.total)
    }

    /// True if the stream yields no packets.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == Some(0)
    }

    /// Writes the values of packet `index` into `out` (assignment order); no
    /// allocation for numeric fields once `out` has capacity. `base` resolves
    /// `$field` references to fields that are not generated by this stream.
    ///
    /// # Errors
    /// Expression evaluation errors.
    pub fn values_at(
        &self,
        index: u64,
        base: &dyn FieldLookup,
        out: &mut Vec<FieldValue>,
    ) -> Result<(), GeneratorError> {
        let n = self.gens.len();
        out.clear();
        out.resize(n, FieldValue::UInt(0));
        // Per-generator indices; on the stack for the usual handful of generators
        // (this runs once per packet on the on-the-fly TX path).
        let mut small = [0u64; 8];
        let mut heap = Vec::new();
        let local: &mut [u64] = if n <= small.len() {
            &mut small[..n]
        } else {
            heap.resize(n, 0);
            &mut heap
        };
        if self.zip {
            local.iter_mut().for_each(|l| *l = index);
        } else {
            let mut rem = u128::from(index);
            for &g in self.order.iter().rev() {
                match self.cards[g] {
                    Some(c) => {
                        local[g] = sat(rem % c);
                        rem /= c;
                    }
                    None => local[g] = sat(rem),
                }
            }
        }
        for (i, g) in self.gens.iter().enumerate() {
            if !g.is_expr() {
                out[i] = g.value_at(local[i], &EvalCtx::new(index))?;
            }
        }
        for (i, g) in self.gens.iter().enumerate() {
            if g.is_expr() {
                let v = {
                    let lk = StreamLookup {
                        targets: &self.targets,
                        gens: &self.gens,
                        values: out,
                        current: i,
                        base,
                    };
                    g.value_at(
                        local[i],
                        &EvalCtx {
                            packet_index: index,
                            fields: &lk,
                        },
                    )?
                };
                out[i] = v;
            }
        }
        Ok(())
    }

    /// Overrides of packet `index` (no external field lookup).
    ///
    /// # Errors
    /// See [`Self::values_at`].
    pub fn nth(&self, index: u64) -> Result<Overrides, GeneratorError> {
        self.nth_with(index, &NoFields)
    }

    /// Overrides of packet `index` with an external field lookup.
    ///
    /// # Errors
    /// See [`Self::values_at`].
    pub fn nth_with(
        &self,
        index: u64,
        base: &dyn FieldLookup,
    ) -> Result<Overrides, GeneratorError> {
        let mut vals = Vec::new();
        self.values_at(index, base, &mut vals)?;
        Ok(self.targets.iter().cloned().zip(vals).collect())
    }

    /// Iterates all packets (endless when unbounded).
    #[must_use]
    pub fn iter(&self) -> ExpansionIter<'_> {
        self.iter_range(0..self.len().unwrap_or(u64::MAX))
    }

    /// Iterates packets `range` (for partitioned workers).
    #[must_use]
    pub fn iter_range(&self, range: Range<u64>) -> ExpansionIter<'_> {
        ExpansionIter {
            exp: self,
            pos: range.start,
            end: range.end,
        }
    }

    /// Chooses `PreRender` vs `OnTheFly` for frames of `frame_len` octets.
    #[must_use]
    pub fn plan(&self, frame_len: usize, cfg: &PlanConfig) -> Plan {
        Plan::decide(self.len(), frame_len, cfg)
    }
}

struct StreamLookup<'a> {
    targets: &'a [FieldRef],
    gens: &'a [CompiledGenerator],
    values: &'a [FieldValue],
    current: usize,
    base: &'a dyn FieldLookup,
}

impl FieldLookup for StreamLookup<'_> {
    fn lookup(&self, name: &str) -> Option<i128> {
        let ready = |j: usize| !self.gens[j].is_expr() || j < self.current;
        let hit = self.targets.iter().enumerate().position(|(j, t)| {
            ready(j) && (t.field == name || name == format!("{}.{}", t.layer, t.field))
        });
        match hit {
            Some(j) => crate::generator::field_value_to_i128(&self.values[j]),
            None => self.base.lookup(name),
        }
    }
}

/// Iterator returned by [`Expansion::iter`]. Yields `Err` (once, then stops) if an
/// expression fails.
#[derive(Debug)]
pub struct ExpansionIter<'a> {
    exp: &'a Expansion,
    pos: u64,
    end: u64,
}

impl Iterator for ExpansionIter<'_> {
    type Item = Result<Overrides, GeneratorError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.end {
            return None;
        }
        let r = self.exp.nth(self.pos);
        self.pos = if r.is_err() { self.end } else { self.pos + 1 };
        Some(r)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = usize::try_from(self.end.saturating_sub(self.pos)).unwrap_or(usize::MAX);
        (n, Some(n))
    }
}

/// Splits `0..total` into at most `parts` contiguous, balanced ranges.
#[must_use]
pub fn partition(total: u64, parts: usize) -> Vec<Range<u64>> {
    let parts = (parts.max(1) as u64).min(total.max(1));
    let (q, r) = (total / parts, total % parts);
    let mut out = Vec::new();
    let mut at = 0;
    for i in 0..parts {
        let len = q + u64::from(i < r);
        if len > 0 {
            out.push(at..at + len);
        }
        at += len;
    }
    out
}

/// User hint for the expansion strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanHint {
    /// Decide from the estimate.
    #[default]
    Auto,
    /// Prefer pre-rendering.
    PreRender,
    /// Prefer on-the-fly generation.
    OnTheFly,
}

/// Expansion strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    /// Render every packet before sending.
    PreRender,
    /// Render while sending.
    OnTheFly,
}

/// Why a strategy was chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum PlanReason {
    /// The user's hint was followed.
    HintForced,
    /// Unbounded stream cannot be pre-rendered (also overrides a pre-render hint).
    Unbounded,
    /// More packets than the pre-render packet limit.
    ExceedsPacketLimit {
        /// The limit.
        limit: u64,
    },
    /// Estimated memory above the threshold.
    ExceedsMemoryThreshold {
        /// The threshold in bytes.
        threshold_bytes: u64,
    },
    /// Fits comfortably in memory.
    WithinMemoryThreshold {
        /// The threshold in bytes.
        threshold_bytes: u64,
    },
}

/// Tunables for [`Plan::decide`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanConfig {
    /// Pre-render only if `count * frame_len` is at most this (default 256 MiB).
    pub threshold_bytes: u64,
    /// Hard packet limit for pre-rendering (default 100M, TODO §2.1).
    pub max_prerender_packets: u64,
    /// User override.
    pub hint: PlanHint,
}

impl Default for PlanConfig {
    fn default() -> Self {
        PlanConfig {
            threshold_bytes: 256 * 1024 * 1024,
            max_prerender_packets: 100_000_000,
            hint: PlanHint::Auto,
        }
    }
}

/// The decision, as data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    /// Chosen strategy.
    pub strategy: Strategy,
    /// Why.
    pub reason: PlanReason,
    /// Packet count, `None` if unbounded.
    pub packets: Option<u64>,
    /// `packets * frame_len`, `None` if unbounded.
    pub estimated_bytes: Option<u128>,
}

impl Plan {
    /// Decides for `packets` frames of `frame_len` octets.
    #[must_use]
    pub fn decide(packets: Option<u64>, frame_len: usize, cfg: &PlanConfig) -> Plan {
        let estimated_bytes = packets.map(|p| u128::from(p) * frame_len as u128);
        let (strategy, reason) = match (packets, estimated_bytes) {
            (None, _) | (_, None) => (Strategy::OnTheFly, PlanReason::Unbounded),
            (Some(_), Some(_)) if cfg.hint == PlanHint::PreRender => {
                (Strategy::PreRender, PlanReason::HintForced)
            }
            (Some(_), Some(_)) if cfg.hint == PlanHint::OnTheFly => {
                (Strategy::OnTheFly, PlanReason::HintForced)
            }
            (Some(p), Some(_)) if p > cfg.max_prerender_packets => (
                Strategy::OnTheFly,
                PlanReason::ExceedsPacketLimit {
                    limit: cfg.max_prerender_packets,
                },
            ),
            (Some(_), Some(b)) if b > u128::from(cfg.threshold_bytes) => (
                Strategy::OnTheFly,
                PlanReason::ExceedsMemoryThreshold {
                    threshold_bytes: cfg.threshold_bytes,
                },
            ),
            _ => (
                Strategy::PreRender,
                PlanReason::WithinMemoryThreshold {
                    threshold_bytes: cfg.threshold_bytes,
                },
            ),
        };
        Plan {
            strategy,
            reason,
            packets,
            estimated_bytes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn asg(layer: &str, field: &str, ty: FieldType, g: Generator) -> Assignment {
        Assignment {
            target: FieldRef::new(layer, field),
            ty,
            generator: g,
        }
    }

    fn list(vals: &[&str]) -> Generator {
        Generator::List {
            values: vals.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    fn u8t() -> FieldType {
        FieldType::UInt { bits: 8 }
    }

    fn spec(mode: Mode) -> StreamSpec {
        StreamSpec {
            assignments: vec![
                asg("ip", "a", u8t(), list(&["1", "2"])),
                asg("ip", "b", u8t(), list(&["10", "20", "30"])),
            ],
            mode,
            count: None,
            seed: 1,
        }
    }

    fn collect(e: &Expansion) -> Vec<Vec<u64>> {
        e.iter()
            .map(|r| {
                r.unwrap()
                    .into_iter()
                    .map(|(_, v)| match v {
                        FieldValue::UInt(x) => x,
                        other => panic!("{other:?}"),
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn zip_cycles_shorter() {
        let e = Expansion::new(&spec(Mode::Zip), None).unwrap();
        assert_eq!(e.len(), Some(3));
        assert_eq!(collect(&e), vec![vec![1, 10], vec![2, 20], vec![1, 30]]);
    }

    #[test]
    fn product_odometer() {
        let e = Expansion::new(&spec(Mode::Product), None).unwrap();
        assert_eq!(e.total_cardinality(), Some(6));
        assert_eq!(
            collect(&e),
            vec![
                vec![1, 10],
                vec![1, 20],
                vec![1, 30],
                vec![2, 10],
                vec![2, 20],
                vec![2, 30]
            ]
        );
    }

    #[test]
    fn nested_priority_and_unknown() {
        let mode = Mode::Nested {
            order: vec![FieldRef::new("ip", "b"), FieldRef::new("ip", "a")],
        };
        let e = Expansion::new(&spec(mode), None).unwrap();
        assert_eq!(
            collect(&e),
            vec![
                vec![1, 10],
                vec![2, 10],
                vec![1, 20],
                vec![2, 20],
                vec![1, 30],
                vec![2, 30]
            ]
        );
        let bad = Mode::Nested {
            order: vec![FieldRef::new("x", "y")],
        };
        assert_eq!(
            Expansion::new(&spec(bad), None).unwrap_err(),
            GeneratorError::UnknownOrderField("x.y".into())
        );
        // Unlisted fields become fastest.
        let part = Mode::Nested {
            order: vec![FieldRef::new("ip", "b")],
        };
        let e = Expansion::new(&spec(part), None).unwrap();
        assert_eq!(collect(&e)[1], vec![2, 10]);
    }

    #[test]
    fn count_override_and_random_access() {
        let mut sp = spec(Mode::Product);
        sp.count = Some(8);
        let e = Expansion::new(&sp, None).unwrap();
        assert_eq!(e.len(), Some(8));
        assert_eq!(e.total_cardinality(), Some(6));
        let all = collect(&e);
        assert_eq!(all.len(), 8);
        assert_eq!(all[6], all[0]); // wraps
        for (i, row) in all.iter().enumerate() {
            let nth: Vec<u64> = e
                .nth(i as u64)
                .unwrap()
                .into_iter()
                .map(|(_, v)| if let FieldValue::UInt(x) = v { x } else { 0 })
                .collect();
            assert_eq!(&nth, row);
        }
    }

    #[test]
    fn unbounded_outermost_only() {
        let rnd = Generator::Random {
            min: None,
            max: None,
            seed: None,
        };
        let sp = StreamSpec {
            assignments: vec![
                asg("l", "r", u8t(), rnd.clone()),
                asg("l", "f", u8t(), list(&["1", "2"])),
            ],
            mode: Mode::Product,
            count: Some(5),
            seed: 0,
        };
        let e = Expansion::new(&sp, None).unwrap();
        assert_eq!(e.total_cardinality(), None);
        assert_eq!(e.len(), Some(5));
        let mut bad = sp.clone();
        bad.assignments.reverse();
        assert_eq!(
            Expansion::new(&bad, None).unwrap_err(),
            GeneratorError::InfiniteInner
        );
        // Zip of only-random needs a count.
        let z = StreamSpec {
            assignments: vec![asg("l", "r", u8t(), rnd)],
            mode: Mode::Zip,
            count: None,
            seed: 0,
        };
        assert_eq!(Expansion::new(&z, None).unwrap().len(), None);
    }

    #[test]
    fn zip_ipv4_and_mac_ranges_with_expr() {
        let sp = StreamSpec {
            assignments: vec![
                asg(
                    "ipv4",
                    "src",
                    FieldType::Ipv4,
                    Generator::Range {
                        start: "10.0.0.1".into(),
                        end: "10.0.0.3".into(),
                        step: 1,
                    },
                ),
                asg(
                    "eth",
                    "src",
                    FieldType::Mac,
                    Generator::Range {
                        start: "00:00:00:00:00:01".into(),
                        end: "00:00:00:00:00:03".into(),
                        step: 1,
                    },
                ),
                asg(
                    "ipv4",
                    "id",
                    FieldType::UInt { bits: 16 },
                    Generator::Expr {
                        expr: "$ipv4.src & 0xff + idx * 100 + $base".into(),
                    },
                ),
            ],
            mode: Mode::Zip,
            count: None,
            seed: 0,
        };
        let e = Expansion::new(&sp, None).unwrap();
        assert_eq!(e.len(), Some(3));
        let base = |n: &str| (n == "base").then_some(1000);
        let p = e.nth_with(2, &base).unwrap();
        assert_eq!(p[0].1, FieldValue::Ipv4(Ipv4Addr::new(10, 0, 0, 3)));
        assert_eq!(p[1].1, FieldValue::Mac([0, 0, 0, 0, 0, 3]));
        // `&` binds looser than `+`: $ipv4.src & (0xff + 200 + 1000)
        let src = i128::from(u32::from(Ipv4Addr::new(10, 0, 0, 3)));
        assert_eq!(
            p[2].1,
            FieldValue::UInt(((src & (0xff + 200 + 1000)) & 0xffff) as u64)
        );
        // Missing base field is an error.
        assert!(e.nth(0).is_err());
    }

    #[test]
    fn partition_covers_exactly() {
        let parts = partition(10, 3);
        assert_eq!(parts, vec![0..4, 4..7, 7..10]);
        assert_eq!(partition(2, 8), vec![0..1, 1..2]);
        assert!(partition(0, 4).is_empty());
        let e = Expansion::new(&spec(Mode::Product), None).unwrap();
        let mut joined = Vec::new();
        for r in partition(e.len().unwrap(), 4) {
            joined.extend(e.iter_range(r).map(Result::unwrap));
        }
        let whole: Vec<_> = e.iter().map(Result::unwrap).collect();
        assert_eq!(joined, whole);
    }

    #[test]
    fn plan_selection() {
        let cfg = PlanConfig::default();
        let p = Plan::decide(Some(1000), 64, &cfg);
        assert_eq!(p.strategy, Strategy::PreRender);
        assert_eq!(p.estimated_bytes, Some(64_000));
        assert!(matches!(p.reason, PlanReason::WithinMemoryThreshold { .. }));
        let p = Plan::decide(Some(10_000_000), 1500, &cfg);
        assert_eq!(p.strategy, Strategy::OnTheFly);
        assert!(matches!(
            p.reason,
            PlanReason::ExceedsMemoryThreshold { .. }
        ));
        let p = Plan::decide(Some(200_000_000), 1, &cfg);
        assert!(matches!(p.reason, PlanReason::ExceedsPacketLimit { .. }));
        let p = Plan::decide(None, 64, &cfg);
        assert_eq!(
            (p.strategy, p.reason),
            (Strategy::OnTheFly, PlanReason::Unbounded)
        );
        let force = PlanConfig {
            hint: PlanHint::PreRender,
            ..cfg
        };
        let p = Plan::decide(Some(10_000_000), 1500, &force);
        assert_eq!(
            (p.strategy, p.reason),
            (Strategy::PreRender, PlanReason::HintForced)
        );
        assert_eq!(Plan::decide(None, 64, &force).strategy, Strategy::OnTheFly);
        let otf = PlanConfig {
            hint: PlanHint::OnTheFly,
            ..cfg
        };
        assert_eq!(Plan::decide(Some(1), 64, &otf).strategy, Strategy::OnTheFly);
    }

    #[test]
    fn field_ref_parse_display_and_serde() {
        let r = FieldRef::parse("ipv4.src").unwrap();
        assert_eq!(r.to_string(), "ipv4.src");
        assert!(FieldRef::parse("nodot").is_none());
        let sp = spec(Mode::Nested {
            order: vec![FieldRef::new("ip", "a")],
        });
        let j = serde_json::to_string(&sp).unwrap();
        assert_eq!(serde_json::from_str::<StreamSpec>(&j).unwrap(), sp);
    }
}
