//! Field value generators (TODO §2.1 "変数・インクリメント・大量生成").
//!
//! A [`Generator`] is the serialisable description (values in human notation, see
//! [`crate::notation`]). [`Generator::compile`] binds it to a [`FieldType`] and
//! yields a [`CompiledGenerator`] with O(1) random access ([`CompiledGenerator::value_at`]),
//! which is what makes multi-threaded partitioning of huge streams possible: even
//! `random` is a pure function of `(seed, index)`.
//!
//! No I/O happens here: the `file` generator reads through a [`FileProvider`].
#![allow(clippy::cast_possible_truncation)]

mod expr;
mod rng;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::error::CoreError;
use crate::ir::{FieldType, Length};
use crate::notation::{self, NotationError};
use crate::packet::FieldValue;

pub use expr::{crc32, field_value_to_i128, fnv1a64, Expr, FieldLookup, NoFields};
pub use rng::Rng;

/// Errors from compiling / evaluating generators and expanding streams.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum GeneratorError {
    /// A value in the generator is not valid for the field type.
    #[error(transparent)]
    Notation(#[from] NotationError),
    /// `step` was zero.
    #[error("step must not be zero")]
    ZeroStep,
    /// A `list` / `file` produced no values.
    #[error("generator has no values")]
    Empty,
    /// `min` is greater than `max`.
    #[error("invalid bounds: min is greater than max")]
    InvalidBounds,
    /// This generator kind cannot drive this field type.
    #[error("generator `{kind}` does not support this field type: {reason}")]
    Unsupported {
        /// Generator kind.
        kind: &'static str,
        /// Why.
        reason: &'static str,
    },
    /// NSAP range endpoints differ outside the 6-octet system id.
    #[error("NSAP range endpoints must share everything except the system id")]
    NsapMismatch,
    /// The `file` generator's path could not be resolved.
    #[error("file `{0}` is not available")]
    FileUnavailable(String),
    /// Expression syntax / evaluation error.
    #[error("expression error: {0}")]
    Expr(String),
    /// A `nested` order references a field that has no generator.
    #[error("unknown field `{0}` in nested order")]
    UnknownOrderField(String),
    /// An unbounded generator sits in a non-outermost position of a product.
    #[error("an unbounded generator can only be the outermost of a product/nested stream")]
    InfiniteInner,
}

impl From<GeneratorError> for CoreError {
    fn from(e: GeneratorError) -> Self {
        CoreError::InvalidValue {
            field: String::new(),
            reason: e.to_string(),
        }
    }
}

/// Supplies the lines of a `file` generator (the CLI / server does the I/O).
pub trait FileProvider {
    /// All lines of `path`, or `None` if unknown.
    fn lines(&self, path: &str) -> Option<Vec<String>>;
}

impl FileProvider for HashMap<String, Vec<String>> {
    fn lines(&self, path: &str) -> Option<Vec<String>> {
        self.get(path).cloned()
    }
}

impl<F: Fn(&str) -> Option<Vec<String>>> FileProvider for F {
    fn lines(&self, path: &str) -> Option<Vec<String>> {
        self(path)
    }
}

/// Inputs for [`Generator::compile`].
#[derive(Clone, Copy)]
pub struct CompileOptions<'a> {
    /// Default seed for `random` generators lacking their own.
    pub seed: u64,
    /// Mixed into the seed so identical generators on different fields differ.
    pub salt: u64,
    /// Resolver for `file` generators.
    pub files: Option<&'a dyn FileProvider>,
}

impl std::fmt::Debug for CompileOptions<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompileOptions")
            .field("seed", &self.seed)
            .field("salt", &self.salt)
            .field("files", &self.files.map(|_| "<provider>"))
            .finish()
    }
}

impl Default for CompileOptions<'_> {
    fn default() -> Self {
        CompileOptions {
            seed: 0x5EED_5EED_5EED,
            salt: 0,
            files: None,
        }
    }
}

/// A rule that varies a field's value across the packets of a stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Generator {
    /// `start..=end` stepping by `step` (IPv4/IPv6/MAC/integers/NSAP System ID).
    /// Descends when `end < start`.
    Range {
        /// First value.
        start: String,
        /// Last value (inclusive).
        end: String,
        /// Increment, default 1.
        #[serde(default = "one")]
        step: u64,
    },
    /// Cycles through an explicit list.
    List {
        /// Values in order.
        values: Vec<String>,
    },
    /// Uniformly random value; bounds default to the field's full range.
    Random {
        /// Lower bound (inclusive).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<String>,
        /// Upper bound (inclusive).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<String>,
        /// PRNG seed (defaults to the stream seed).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seed: Option<u64>,
    },
    /// Random MAC under a fixed OUI / prefix (`00:11:22`, 0..=5 octets).
    RandomMac {
        /// Fixed leading octets.
        #[serde(default)]
        oui: String,
        /// PRNG seed (defaults to the stream seed).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seed: Option<u64>,
    },
    /// Integer counter with optional wrap-around.
    Counter {
        /// Initial value.
        #[serde(default)]
        start: u64,
        /// Increment (may be negative).
        #[serde(default = "one_i")]
        step: i64,
        /// Wrap back to `start` after reaching this value.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        wrap: Option<u64>,
    },
    /// One value per line of a file (blank lines and `#` comments skipped).
    File {
        /// Path, relative to the template.
        path: String,
    },
    /// Expression referencing other fields (`hash()`, `crc()`, arithmetic).
    Expr {
        /// Expression source.
        expr: String,
    },
}

const fn one() -> u64 {
    1
}

const fn one_i() -> i64 {
    1
}

// ---------------------------------------------------------------------------
// numeric domains
// ---------------------------------------------------------------------------

/// Numeric view of a field type: every value is an order-preserving `u128`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dom {
    UInt(u8),
    /// Offset binary (two's complement with the sign bit flipped) so order is kept.
    Int(u8),
    Ipv4,
    Ipv6,
    Mac,
    Bytes(u8),
}

impl Dom {
    fn from_type(ty: &FieldType, kind: &'static str) -> Result<Dom, GeneratorError> {
        Ok(match ty {
            FieldType::UInt { bits }
            | FieldType::Enum { bits, .. }
            | FieldType::Flags { bits, .. } => Dom::UInt(*bits),
            FieldType::Int { bits } => Dom::Int(*bits),
            FieldType::Ipv4 => Dom::Ipv4,
            FieldType::Ipv6 => Dom::Ipv6,
            FieldType::Mac => Dom::Mac,
            FieldType::Bytes {
                len: Length::Fixed { octets },
            } if (1..=16).contains(octets) => Dom::Bytes(*octets as u8),
            _ => {
                return Err(GeneratorError::Unsupported {
                    kind,
                    reason: "field type is not numeric",
                })
            }
        })
    }

    fn bits(self) -> u32 {
        match self {
            Dom::UInt(b) | Dom::Int(b) => u32::from(b),
            Dom::Ipv4 => 32,
            Dom::Ipv6 => 128,
            Dom::Mac => 48,
            Dom::Bytes(n) => u32::from(n) * 8,
        }
    }

    fn mask(self) -> u128 {
        let b = self.bits();
        if b >= 128 {
            u128::MAX
        } else {
            (1u128 << b) - 1
        }
    }

    fn sign_bit(self) -> u128 {
        match self {
            Dom::Int(b) => 1u128 << (b - 1),
            _ => 0,
        }
    }

    fn encode(self, v: &FieldValue) -> Option<u128> {
        Some(match (self, v) {
            (Dom::UInt(_), FieldValue::UInt(x)) => u128::from(*x),
            (Dom::Int(_), FieldValue::Int(x)) => {
                ((i128::from(*x) as u128) & self.mask()) ^ self.sign_bit()
            }
            (Dom::Ipv4, FieldValue::Ipv4(a)) => u128::from(u32::from(*a)),
            (Dom::Ipv6, FieldValue::Ipv6(a)) => u128::from(*a),
            (Dom::Mac, FieldValue::Mac(m)) => {
                m.iter().fold(0u128, |a, b| (a << 8) | u128::from(*b))
            }
            (Dom::Bytes(n), FieldValue::Bytes(b)) if b.len() == usize::from(n) => {
                b.iter().fold(0u128, |a, x| (a << 8) | u128::from(*x))
            }
            _ => return None,
        })
    }

    fn decode(self, x: u128) -> FieldValue {
        match self {
            Dom::UInt(_) => FieldValue::UInt(x as u64),
            Dom::Int(b) => {
                let tc = x ^ self.sign_bit();
                let shift = 128 - u32::from(b);
                FieldValue::Int((((tc << shift) as i128) >> shift) as i64)
            }
            Dom::Ipv4 => FieldValue::Ipv4((x as u32).into()),
            Dom::Ipv6 => FieldValue::Ipv6(x.into()),
            Dom::Mac => {
                let b = x.to_be_bytes();
                FieldValue::Mac([b[10], b[11], b[12], b[13], b[14], b[15]])
            }
            Dom::Bytes(n) => FieldValue::Bytes(x.to_be_bytes()[16 - usize::from(n)..].to_vec()),
        }
    }

    /// Wraps a signed intermediate into the domain (two's complement modulo 2^bits).
    fn wrap_i128(self, v: i128) -> u128 {
        ((v as u128) & self.mask()) ^ self.sign_bit()
    }

    /// Raw signed view of a counter start/wrap constant.
    fn raw_from_u64(self, x: u64) -> i128 {
        match self {
            Dom::Int(_) => i128::from(x as i64),
            _ => i128::from(x),
        }
    }

    fn parse(self, s: &str, ty: &FieldType) -> Result<u128, GeneratorError> {
        let v = notation::parse_value(s, ty)?;
        self.encode(&v).ok_or(GeneratorError::Unsupported {
            kind: "value",
            reason: "value does not match field type",
        })
    }
}

/// Arithmetic sequence over encoded values.
#[derive(Debug, Clone, Copy)]
struct Seq {
    start: u128,
    step: u128,
    count: u128,
    desc: bool,
}

impl Seq {
    fn new(a: u128, b: u128, step: u64) -> Result<Seq, GeneratorError> {
        if step == 0 {
            return Err(GeneratorError::ZeroStep);
        }
        let step = u128::from(step);
        let desc = b < a;
        let diff = a.abs_diff(b);
        Ok(Seq {
            start: a,
            step,
            count: (diff / step).saturating_add(1),
            desc,
        })
    }

    fn at(&self, i: u128) -> u128 {
        let off = (i % self.count).wrapping_mul(self.step);
        if self.desc {
            self.start.wrapping_sub(off)
        } else {
            self.start.wrapping_add(off)
        }
    }
}

#[derive(Debug, Clone)]
enum Kind {
    Seq(Dom, Seq),
    NsapSeq {
        template: Vec<u8>,
        seq: Seq,
    },
    List(Vec<FieldValue>),
    Random {
        dom: Dom,
        lo: u128,
        hi: u128,
        seed: u64,
    },
    Counter {
        dom: Dom,
        start: i128,
        step: i128,
        count: Option<u128>,
    },
    Expr {
        dom: Dom,
        expr: Expr,
        seed: u64,
    },
}

/// Evaluation context for [`CompiledGenerator::value_at`].
#[derive(Clone, Copy)]
pub struct EvalCtx<'a> {
    /// Packet index (what `idx` means in expressions).
    pub packet_index: u64,
    /// Field lookup for `$field` references.
    pub fields: &'a dyn FieldLookup,
}

impl std::fmt::Debug for EvalCtx<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvalCtx")
            .field("packet_index", &self.packet_index)
            .finish_non_exhaustive()
    }
}

impl EvalCtx<'_> {
    /// Context without field references.
    #[must_use]
    pub fn new(packet_index: u64) -> EvalCtx<'static> {
        EvalCtx {
            packet_index,
            fields: &NoFields,
        }
    }
}

/// A generator bound to a field type, ready for evaluation.
#[derive(Debug, Clone)]
pub struct CompiledGenerator {
    kind: Kind,
}

fn mix_seed(seed: u64, salt: u64) -> u64 {
    let mut s = seed ^ salt.wrapping_mul(0xA24B_AED4_963E_E407);
    rng::splitmix64(&mut s)
}

fn sat_u64(x: u128) -> u64 {
    u64::try_from(x).unwrap_or(u64::MAX)
}

impl Generator {
    /// Binds the generator to `ty`, parsing all literals.
    ///
    /// # Errors
    /// [`GeneratorError`] when values do not parse, bounds are inconsistent or the
    /// kind does not support `ty`.
    pub fn compile(
        &self,
        ty: &FieldType,
        opts: &CompileOptions<'_>,
    ) -> Result<CompiledGenerator, GeneratorError> {
        let kind = match self {
            Generator::Range { start, end, step } => {
                if matches!(ty, FieldType::Nsap) {
                    compile_nsap_range(start, end, *step)?
                } else {
                    let dom = Dom::from_type(ty, "range")?;
                    Kind::Seq(
                        dom,
                        Seq::new(dom.parse(start, ty)?, dom.parse(end, ty)?, *step)?,
                    )
                }
            }
            Generator::List { values } => {
                if values.is_empty() {
                    return Err(GeneratorError::Empty);
                }
                Kind::List(
                    values
                        .iter()
                        .map(|v| notation::parse_value(v, ty))
                        .collect::<Result<_, _>>()?,
                )
            }
            Generator::Random { min, max, seed } => {
                let dom = Dom::from_type(ty, "random")?;
                let lo = min.as_deref().map_or(Ok(0), |s| dom.parse(s, ty))?;
                let hi = max
                    .as_deref()
                    .map_or(Ok(dom.mask()), |s| dom.parse(s, ty))?;
                if lo > hi {
                    return Err(GeneratorError::InvalidBounds);
                }
                Kind::Random {
                    dom,
                    lo,
                    hi,
                    seed: mix_seed(seed.unwrap_or(opts.seed), opts.salt),
                }
            }
            Generator::RandomMac { oui, seed } => {
                if !matches!(ty, FieldType::Mac) {
                    return Err(GeneratorError::Unsupported {
                        kind: "random_mac",
                        reason: "field is not a MAC address",
                    });
                }
                let octets: Vec<u8> = oui
                    .split([':', '-', '.'])
                    .filter(|p| !p.is_empty())
                    .map(|p| {
                        u8::from_str_radix(p, 16).map_err(|_| NotationError::Invalid {
                            expected: "OUI",
                            input: oui.clone(),
                        })
                    })
                    .collect::<Result<_, _>>()?;
                if octets.len() > 6 {
                    return Err(NotationError::Invalid {
                        expected: "OUI (at most 6 octets)",
                        input: oui.clone(),
                    }
                    .into());
                }
                let free = (6 - octets.len()) * 8;
                let prefix = octets.iter().fold(0u128, |a, b| (a << 8) | u128::from(*b)) << free;
                let low = if free == 0 { 0 } else { (1u128 << free) - 1 };
                Kind::Random {
                    dom: Dom::Mac,
                    lo: prefix,
                    hi: prefix | low,
                    seed: mix_seed(seed.unwrap_or(opts.seed), opts.salt),
                }
            }
            Generator::Counter { start, step, wrap } => {
                let dom = Dom::from_type(ty, "counter")?;
                let s = dom.raw_from_u64(*start);
                let step = i128::from(*step);
                let count = wrap.map(|w| {
                    let w = dom.raw_from_u64(w);
                    let reach = if step > 0 && w >= s {
                        (w - s) / step
                    } else if step < 0 && w <= s {
                        (s - w) / -step
                    } else {
                        0
                    };
                    reach as u128 + 1
                });
                Kind::Counter {
                    dom,
                    start: s,
                    step,
                    count,
                }
            }
            Generator::File { path } => {
                let lines = opts
                    .files
                    .and_then(|p| p.lines(path))
                    .ok_or_else(|| GeneratorError::FileUnavailable(path.clone()))?;
                let values: Vec<FieldValue> = lines
                    .iter()
                    .map(|l| l.trim())
                    .filter(|l| !l.is_empty() && !l.starts_with('#'))
                    .map(|l| notation::parse_value(l, ty))
                    .collect::<Result<_, _>>()?;
                if values.is_empty() {
                    return Err(GeneratorError::Empty);
                }
                Kind::List(values)
            }
            Generator::Expr { expr } => Kind::Expr {
                dom: Dom::from_type(ty, "expr")?,
                expr: Expr::parse(expr)?,
                seed: mix_seed(opts.seed, opts.salt),
            },
        };
        Ok(CompiledGenerator { kind })
    }

    /// Number of distinct values for `ty` (`None` = unbounded / random / expression).
    ///
    /// # Errors
    /// Same as [`Generator::compile`].
    pub fn cardinality_for(
        &self,
        ty: &FieldType,
        opts: &CompileOptions<'_>,
    ) -> Result<Option<u64>, GeneratorError> {
        Ok(self.compile(ty, opts)?.cardinality())
    }

    /// Iterator over the values for `ty`.
    ///
    /// # Errors
    /// Same as [`Generator::compile`].
    pub fn iter(
        &self,
        ty: &FieldType,
        opts: &CompileOptions<'_>,
    ) -> Result<GeneratorState, GeneratorError> {
        Ok(self.compile(ty, opts)?.into_state())
    }

    /// Type-less cardinality: infers the type of `range` endpoints from their text.
    ///
    /// # Errors
    /// [`CoreError::NotImplemented`] for `file` (needs I/O) and for endpoints whose
    /// type cannot be inferred; other errors as [`CoreError::InvalidValue`].
    pub fn cardinality(&self) -> crate::error::Result<Option<u64>> {
        match self {
            Generator::List { values } => Ok(Some(values.len() as u64)),
            Generator::Counter {
                wrap: Some(_),
                step,
                ..
            } if *step != 0 => {
                let g = self.compile(&FieldType::UInt { bits: 64 }, &CompileOptions::default())?;
                Ok(g.cardinality())
            }
            Generator::Counter { .. }
            | Generator::Random { .. }
            | Generator::RandomMac { .. }
            | Generator::Expr { .. } => Ok(None),
            Generator::Range { start, .. } => {
                let ty = notation::infer_type(start).ok_or(CoreError::NotImplemented(
                    "cardinality of a range with untyped endpoints",
                ))?;
                Ok(self.cardinality_for(&ty, &CompileOptions::default())?)
            }
            Generator::File { .. } => Err(CoreError::NotImplemented(
                "file generator cardinality needs a FileProvider",
            )),
        }
    }
}

fn compile_nsap_range(start: &str, end: &str, step: u64) -> Result<Kind, GeneratorError> {
    let a = notation::parse_nsap(start)?;
    let b = notation::parse_nsap(end)?;
    if a.len() != b.len() || a.len() < 7 {
        return Err(GeneratorError::NsapMismatch);
    }
    let n = a.len();
    if a[..n - 7] != b[..n - 7] || a[n - 1] != b[n - 1] {
        return Err(GeneratorError::NsapMismatch);
    }
    let sys = |v: &[u8]| {
        v[n - 7..n - 1]
            .iter()
            .fold(0u128, |x, y| (x << 8) | u128::from(*y))
    };
    Ok(Kind::NsapSeq {
        seq: Seq::new(sys(&a), sys(&b), step)?,
        template: a,
    })
}

impl CompiledGenerator {
    /// Exact number of distinct values; `None` when unbounded (random, expression,
    /// counter without wrap). Saturates at `u128::MAX`.
    #[must_use]
    pub fn cardinality_exact(&self) -> Option<u128> {
        match &self.kind {
            Kind::Seq(_, s) | Kind::NsapSeq { seq: s, .. } => Some(s.count),
            Kind::List(v) => Some(v.len() as u128),
            Kind::Counter { count, .. } => *count,
            Kind::Random { .. } | Kind::Expr { .. } => None,
        }
    }

    /// [`Self::cardinality_exact`] saturated to `u64`.
    #[must_use]
    pub fn cardinality(&self) -> Option<u64> {
        self.cardinality_exact().map(sat_u64)
    }

    /// True for expression generators (they need field lookups).
    #[must_use]
    pub fn is_expr(&self) -> bool {
        matches!(self.kind, Kind::Expr { .. })
    }

    /// Fields referenced by an expression generator (empty otherwise).
    #[must_use]
    pub fn references(&self) -> Vec<String> {
        match &self.kind {
            Kind::Expr { expr, .. } => expr.references(),
            _ => Vec::new(),
        }
    }

    /// The `index`-th value (finite generators cycle, i.e. index modulo cardinality).
    ///
    /// # Errors
    /// Only expression generators can fail (unknown field, division by zero).
    pub fn value_at(&self, index: u64, ctx: &EvalCtx<'_>) -> Result<FieldValue, GeneratorError> {
        let i = u128::from(index);
        Ok(match &self.kind {
            Kind::Seq(dom, s) => dom.decode(s.at(i)),
            Kind::NsapSeq { template, seq } => {
                let n = template.len();
                let mut out = template.clone();
                let v = seq.at(i).to_be_bytes();
                out[n - 7..n - 1].copy_from_slice(&v[10..]);
                FieldValue::Nsap(out)
            }
            Kind::List(v) => v[(i % v.len() as u128) as usize].clone(),
            Kind::Random { dom, lo, hi, seed } => {
                dom.decode(Rng::for_index(*seed, index).range_u128(*lo, *hi))
            }
            Kind::Counter {
                dom,
                start,
                step,
                count,
            } => {
                let k = count.map_or(i, |c| i % c);
                let v = start.wrapping_add((k as i128).wrapping_mul(*step));
                dom.decode(dom.wrap_i128(v))
            }
            Kind::Expr { dom, expr, seed } => {
                let v = expr.eval_env(&expr::Env {
                    index: ctx.packet_index,
                    seed: *seed,
                    fields: ctx.fields,
                })?;
                dom.decode(dom.wrap_i128(v))
            }
        })
    }

    /// Convenience for generators without field references.
    ///
    /// # Errors
    /// See [`Self::value_at`].
    pub fn value_at_simple(&self, index: u64) -> Result<FieldValue, GeneratorError> {
        self.value_at(index, &EvalCtx::new(index))
    }

    /// Consumes `self` into a sequential iterator.
    #[must_use]
    pub fn into_state(self) -> GeneratorState {
        let end = self.cardinality();
        GeneratorState {
            gen: self,
            pos: 0,
            end,
        }
    }
}

/// Sequential iteration over a [`CompiledGenerator`]: one pass for finite generators,
/// endless for unbounded ones. Expression generators evaluate without field access
/// (use [`CompiledGenerator::value_at`] / streams for those); evaluation errors end
/// the iteration.
#[derive(Debug, Clone)]
pub struct GeneratorState {
    gen: CompiledGenerator,
    pos: u64,
    end: Option<u64>,
}

impl GeneratorState {
    /// Repositions to `index` (random access for partitioned iteration).
    pub fn seek(&mut self, index: u64) {
        self.pos = index;
    }
}

impl Iterator for GeneratorState {
    type Item = FieldValue;

    fn next(&mut self) -> Option<FieldValue> {
        if self.end.is_some_and(|e| self.pos >= e) {
            return None;
        }
        let v = self.gen.value_at_simple(self.pos).ok()?;
        self.pos += 1;
        Some(v)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.end {
            Some(e) => {
                let n = usize::try_from(e.saturating_sub(self.pos)).unwrap_or(usize::MAX);
                (n, Some(n))
            }
            None => (usize::MAX, None),
        }
    }
}

#[cfg(test)]
mod tests;
