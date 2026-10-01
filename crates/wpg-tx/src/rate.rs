//! Rate control (TODO §2.1 "送出制御", §2.2 "レート制御").
//!
//! * [`Rate`]: what the user asked for (`1000pps`, `1.5Mpps`, `10Gbps`, `50%`,
//!   `ifg=10us`, `unlimited`) with [`Rate::parse`] and the conversion to the
//!   interval between frame starts ([`Rate::interval`]).
//! * [`Pacer`]: a token bucket in the time domain (GCRA-style "theoretical arrival
//!   time") with a configurable burst and a hybrid wait: `sleep` while the deadline is
//!   far away, then spin (optionally `yield_now`) for the last
//!   [`WaitMode::Hybrid::spin_threshold`] (~50 µs by default), so timer slack does not
//!   ruin the spacing. It also measures its own accuracy ([`RateAccuracy`]).
//!
//! Bit rates are **layer-1** rates: every frame occupies
//! `max(len, 60) + 24` octets on an Ethernet wire (8 preamble/SFD + 12 inter-frame
//! gap + 4 FCS; frames shorter than 60 octets are padded by the NIC). So `10Gbps` with
//! 60-octet frames (64 with FCS) is 14.88 Mpps, the textbook line rate.

use std::fmt;
use std::str::FromStr;
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::TxError;

/// Minimum Ethernet frame length without FCS; shorter frames are padded on the wire.
pub const MIN_FRAME_LEN: usize = 60;

/// Per-frame wire overhead in octets: 7 preamble + 1 SFD + 12 IFG + 4 FCS.
pub const WIRE_OVERHEAD: usize = 24;

/// Bits one frame of `frame_len` octets (without FCS) occupies on the wire.
#[must_use]
pub fn wire_bits(frame_len: usize) -> f64 {
    (frame_len.max(MIN_FRAME_LEN) + WIRE_OVERHEAD) as f64 * 8.0
}

/// A transmit rate.
///
/// Serialises as its string form (`"1.5Mpps"` style, see [`Rate::parse`]) so it can be
/// written directly in templates.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Rate {
    /// As fast as the backend accepts frames.
    #[default]
    Unlimited,
    /// Frames per second.
    Pps(f64),
    /// Layer-1 bits per second (see the module docs for the per-frame overhead).
    Bps(f64),
    /// Percentage of the link speed. `link_bps` is `None` until resolved with
    /// [`Rate::with_link_speed`] (parsing `50%` cannot know the NIC).
    LinkPercent {
        /// 0 < percent <= 100.
        percent: f64,
        /// Link speed in bits per second.
        link_bps: Option<f64>,
    },
    /// Fixed interval between the starts of consecutive frames (explicit IFG mode).
    /// Bursts are ignored in this mode.
    Ifg(Duration),
}

/// Error from [`Rate::parse`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum RateParseError {
    /// Empty string.
    #[error("empty rate")]
    Empty,
    /// The numeric part is missing or malformed.
    #[error("invalid number in rate `{0}`")]
    InvalidNumber(String),
    /// Unknown or missing unit.
    #[error("unknown rate unit `{unit}` in `{input}` (expected pps, kpps, Mpps, bps, kbps, Mbps, Gbps, %, ifg=<duration> or unlimited)")]
    UnknownUnit {
        /// The whole input.
        input: String,
        /// The unit part.
        unit: String,
    },
    /// The value is zero, negative, not finite, or a percentage above 100.
    #[error("rate out of range: `{0}`")]
    OutOfRange(String),
}

fn split_number(s: &str) -> (&str, &str) {
    let end = s
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_digit() || *c == '.' || *c == '_'))
        .map_or(s.len(), |(i, _)| i);
    (&s[..end], s[end..].trim())
}

fn number(input: &str, num: &str) -> Result<f64, RateParseError> {
    let cleaned: String = num.chars().filter(|c| *c != '_').collect();
    if cleaned.is_empty() {
        return Err(RateParseError::InvalidNumber(input.to_owned()));
    }
    cleaned
        .parse::<f64>()
        .map_err(|_| RateParseError::InvalidNumber(input.to_owned()))
}

/// Parses a duration like `10us`, `1.5ms`, `100ns`, `2s`, `3µs`.
///
/// # Errors
/// [`RateParseError`] on a malformed number or unit.
pub fn parse_duration(s: &str) -> Result<Duration, RateParseError> {
    let t = s.trim();
    if t.is_empty() {
        return Err(RateParseError::Empty);
    }
    let (num, unit) = split_number(t);
    let v = number(t, num)?;
    let scale = match unit.to_ascii_lowercase().as_str() {
        "ns" => 1e-9,
        "us" | "µs" | "μs" => 1e-6,
        "ms" => 1e-3,
        "s" | "" => 1.0,
        "m" | "min" => 60.0,
        "h" => 3600.0,
        _ => {
            return Err(RateParseError::UnknownUnit {
                input: t.to_owned(),
                unit: unit.to_owned(),
            })
        }
    };
    let secs = v * scale;
    Duration::try_from_secs_f64(secs).map_err(|_| RateParseError::OutOfRange(t.to_owned()))
}

/// Days since 1970-01-01 of a proleptic Gregorian date (H. Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Parses an RFC 3339 timestamp (`2026-09-29T12:00:00Z`,
/// `2026-09-29 21:00:00.5+09:00`) into seconds since the Unix epoch.
fn parse_rfc3339(s: &str) -> Option<f64> {
    let b = s.as_bytes();
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        let t = s.get(r)?;
        t.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| t.parse().ok())?
    };
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    if !matches!(b[10], b'T' | b't' | b' ') {
        return None;
    }
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, sec) = (num(11..13)?, num(14..16)?, num(17..19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || sec > 60 {
        return None;
    }
    let mut rest = &s[19..];
    let mut frac = 0.0;
    if let Some(r) = rest.strip_prefix('.') {
        let end = r.find(|c: char| !c.is_ascii_digit()).unwrap_or(r.len());
        if end == 0 {
            return None;
        }
        frac = format!("0.{}", &r[..end]).parse().ok()?;
        rest = &r[end..];
    }
    let offset = match rest {
        "Z" | "z" => 0,
        _ if rest.len() == 6 && rest.as_bytes()[3] == b':' => {
            let sign = match rest.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let oh: i64 = rest[1..3].parse().ok()?;
            let om: i64 = rest[4..6].parse().ok()?;
            sign * (oh * 3600 + om * 60)
        }
        _ => return None,
    };
    let days = days_from_civil(y, mo, d);
    let secs = days * 86_400 + h * 3600 + mi * 60 + sec - offset;
    Some(secs as f64 + frac)
}

/// Parses a scheduled start time: an RFC 3339 timestamp
/// (`2026-09-29T12:00:00Z`, `2026-09-29T21:00:00+09:00`) or a delay relative to
/// `now` (`+10s`, `+1.5m`, see [`parse_duration`]).
///
/// # Errors
/// A message for anything else.
pub fn parse_start_time(s: &str, now: SystemTime) -> Result<SystemTime, String> {
    let t = s.trim();
    if let Some(rel) = t.strip_prefix('+') {
        let d = parse_duration(rel).map_err(|e| e.to_string())?;
        return Ok(now + d);
    }
    let secs = parse_rfc3339(t).ok_or_else(|| {
        format!(
            "invalid start time `{t}` (expected RFC 3339 such as 2026-09-29T12:00:00Z, or +10s)"
        )
    })?;
    let d = Duration::try_from_secs_f64(secs.abs()).map_err(|e| e.to_string())?;
    Ok(if secs >= 0.0 {
        SystemTime::UNIX_EPOCH + d
    } else {
        SystemTime::UNIX_EPOCH - d
    })
}

impl Rate {
    /// Parses a rate string. Accepted forms (whitespace between number and unit is
    /// optional; `_` digit separators are allowed; SI prefixes `k`/`K`, `M`/`m`,
    /// `G`/`g`, `T`/`t` are decimal and case-insensitive since rates have no milli):
    ///
    /// | input | result |
    /// |---|---|
    /// | `1000pps`, `1.5Mpps`, `10 kpps` | [`Rate::Pps`] |
    /// | `10Gbps`, `100 Mbps`, `64000bps` | [`Rate::Bps`] (layer 1) |
    /// | `50%`, `line` / `linerate` (= 100 %) | [`Rate::LinkPercent`] (unresolved) |
    /// | `ifg=10us`, `ifg:1ms` | [`Rate::Ifg`] |
    /// | `unlimited`, `max`, `inf` | [`Rate::Unlimited`] |
    ///
    /// # Errors
    /// [`RateParseError`] for empty input, bad numbers, unknown units, or values that
    /// are not strictly positive (percentages must be in `(0, 100]`).
    pub fn parse(s: &str) -> Result<Rate, RateParseError> {
        let t = s.trim();
        if t.is_empty() {
            return Err(RateParseError::Empty);
        }
        let lower = t.to_ascii_lowercase();
        match lower.as_str() {
            "unlimited" | "max" | "inf" | "infinite" => return Ok(Rate::Unlimited),
            "line" | "linerate" | "line-rate" | "line_rate" => {
                return Ok(Rate::LinkPercent {
                    percent: 100.0,
                    link_bps: None,
                })
            }
            _ => {}
        }
        if let Some(rest) = lower
            .strip_prefix("ifg=")
            .or_else(|| lower.strip_prefix("ifg:"))
            .or_else(|| lower.strip_prefix("ifg "))
        {
            let d = parse_duration(rest)?;
            if d.is_zero() {
                return Err(RateParseError::OutOfRange(t.to_owned()));
            }
            return Ok(Rate::Ifg(d));
        }

        let (num, unit) = split_number(t);
        let v = number(t, num)?;
        if !v.is_finite() || v <= 0.0 {
            return Err(RateParseError::OutOfRange(t.to_owned()));
        }
        if unit == "%" {
            if v > 100.0 {
                return Err(RateParseError::OutOfRange(t.to_owned()));
            }
            return Ok(Rate::LinkPercent {
                percent: v,
                link_bps: None,
            });
        }
        let u = unit.to_ascii_lowercase();
        let unknown = || RateParseError::UnknownUnit {
            input: t.to_owned(),
            unit: unit.to_owned(),
        };
        let (prefix, base) = if let Some(p) = u.strip_suffix("pps") {
            (p, "pps")
        } else if let Some(p) = u.strip_suffix("bps") {
            (p, "bps")
        } else if let Some(p) = u.strip_suffix("bit/s") {
            (p, "bps")
        } else {
            return Err(unknown());
        };
        let mult = match prefix {
            "" => 1.0,
            "k" => 1e3,
            "m" => 1e6,
            "g" => 1e9,
            "t" => 1e12,
            _ => return Err(unknown()),
        };
        let value = v * mult;
        Ok(if base == "pps" {
            Rate::Pps(value)
        } else {
            Rate::Bps(value)
        })
    }

    /// Resolves [`Rate::LinkPercent`] against a link speed (other variants are
    /// returned unchanged).
    #[must_use]
    pub fn with_link_speed(self, link_bps: f64) -> Rate {
        match self {
            Rate::LinkPercent { percent, .. } => Rate::LinkPercent {
                percent,
                link_bps: Some(link_bps),
            },
            other => other,
        }
    }

    /// Whether this is [`Rate::Unlimited`].
    #[must_use]
    pub fn is_unlimited(&self) -> bool {
        matches!(self, Rate::Unlimited)
    }

    /// Checks that the rate is usable for pacing.
    ///
    /// # Errors
    /// A message for non-positive / non-finite values or an unresolved
    /// [`Rate::LinkPercent`].
    pub fn validate(&self) -> Result<(), String> {
        let ok = |v: f64| v.is_finite() && v > 0.0;
        match *self {
            Rate::Unlimited => Ok(()),
            Rate::Pps(v) | Rate::Bps(v) if ok(v) => Ok(()),
            Rate::Pps(_) | Rate::Bps(_) => Err(format!("rate must be positive: {self}")),
            Rate::LinkPercent { percent, link_bps } => {
                if !(ok(percent) && percent <= 100.0) {
                    return Err(format!("link percentage out of range: {percent}"));
                }
                match link_bps {
                    Some(l) if ok(l) => Ok(()),
                    _ => Err(format!(
                        "`{percent}%` needs the link speed (unknown for this interface); give an absolute rate such as `1Gbps`"
                    )),
                }
            }
            Rate::Ifg(d) if !d.is_zero() => Ok(()),
            Rate::Ifg(_) => Err("IFG must be > 0".to_owned()),
        }
    }

    /// Layer-1 bits per second this rate stands for, if it is a bit rate.
    #[must_use]
    pub fn bits_per_second(&self) -> Option<f64> {
        match *self {
            Rate::Bps(b) => Some(b),
            Rate::LinkPercent {
                percent,
                link_bps: Some(l),
            } => Some(l * percent / 100.0),
            _ => None,
        }
    }

    /// Nanoseconds between the starts of consecutive frames of `frame_len` octets;
    /// `None` for [`Rate::Unlimited`] or an unresolved [`Rate::LinkPercent`].
    #[must_use]
    pub fn interval_ns(&self, frame_len: usize) -> Option<f64> {
        match *self {
            Rate::Unlimited => None,
            Rate::Pps(p) => Some(1e9 / p),
            Rate::Ifg(d) => Some(d.as_secs_f64() * 1e9),
            Rate::Bps(_) | Rate::LinkPercent { .. } => {
                let bps = self.bits_per_second()?;
                Some(wire_bits(frame_len) / bps * 1e9)
            }
        }
    }

    /// [`Self::interval_ns`] as a [`Duration`].
    #[must_use]
    pub fn interval(&self, frame_len: usize) -> Option<Duration> {
        self.interval_ns(frame_len)
            .map(|ns| Duration::from_secs_f64(ns / 1e9))
    }

    /// This rate shared by `n` workers: each gets `1/n` of it (an IFG becomes `n`
    /// times longer). `n = 0` is treated as 1.
    #[must_use]
    pub fn split(self, n: usize) -> Rate {
        let k = n.max(1) as f64;
        match self {
            Rate::Unlimited => Rate::Unlimited,
            Rate::Pps(p) => Rate::Pps(p / k),
            Rate::Bps(b) => Rate::Bps(b / k),
            Rate::LinkPercent { percent, link_bps } => Rate::LinkPercent {
                percent: percent / k,
                link_bps,
            },
            Rate::Ifg(d) => Rate::Ifg(d.mul_f64(k)),
        }
    }

    /// Frames per second for frames of `frame_len` octets (`None` = unlimited).
    #[must_use]
    pub fn frames_per_second(&self, frame_len: usize) -> Option<f64> {
        self.interval_ns(frame_len).map(|ns| 1e9 / ns)
    }
}

impl FromStr for Rate {
    type Err = RateParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Rate::parse(s)
    }
}

impl fmt::Display for Rate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rate::Unlimited => f.write_str("unlimited"),
            Rate::Pps(v) => write!(f, "{v}pps"),
            Rate::Bps(v) => write!(f, "{v}bps"),
            Rate::LinkPercent { percent, .. } => write!(f, "{percent}%"),
            Rate::Ifg(d) => write!(f, "ifg={}ns", d.as_nanos()),
        }
    }
}

impl Serialize for Rate {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Rate {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Rate::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// Link speed of `iface` in bits per second from `/sys/class/net/<iface>/speed`
/// (Linux; `None` elsewhere, for virtual interfaces that report `-1`, or when
/// unknown).
#[must_use]
pub fn link_speed_bps(iface: &str) -> Option<f64> {
    if cfg!(target_os = "linux") && !iface.contains('/') {
        let s = std::fs::read_to_string(format!("/sys/class/net/{iface}/speed")).ok()?;
        let mbps: f64 = s.trim().parse().ok()?;
        (mbps > 0.0).then_some(mbps * 1e6)
    } else {
        None
    }
}

/// How the [`Pacer`] waits for a deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "mode")]
pub enum WaitMode {
    /// `sleep` until `spin_threshold` before the deadline, then spin.
    Hybrid {
        /// Remaining time below which the pacer spins instead of sleeping.
        spin_threshold: Duration,
    },
    /// Only `sleep` (lowest CPU use, jitter = timer slack, typically 50–100 µs).
    Sleep,
    /// Only spin (a full core, lowest jitter).
    Spin,
}

impl Default for WaitMode {
    fn default() -> Self {
        WaitMode::Hybrid {
            spin_threshold: Duration::from_micros(50),
        }
    }
}

/// [`Pacer`] tunables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacerConfig {
    /// Frames sent back to back per token-bucket grant (>= 1). The average rate is
    /// unchanged; a burst of `n` is followed by a gap of `n` intervals.
    pub burst: u32,
    /// Wait strategy.
    pub wait: WaitMode,
    /// Call `std::thread::yield_now` while spinning (friendlier to other threads,
    /// slightly more jitter).
    pub yield_while_spinning: bool,
    /// How far the pacer may fall behind before it forgets the backlog (instead of
    /// sending a catch-up burst). Default 10 ms.
    pub max_backlog: Duration,
    /// Ramp-up: the rate grows linearly from 0 to the target over this much
    /// active time (TODO §2.1 "ランプアップ"). `None` = full rate from the start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ramp_up: Option<Duration>,
}

impl Default for PacerConfig {
    fn default() -> Self {
        Self {
            burst: 1,
            wait: WaitMode::default(),
            yield_while_spinning: false,
            max_backlog: Duration::from_millis(10),
            ramp_up: None,
        }
    }
}

/// Fraction of `target × active` a pacer with a linear ramp-up of `ramp` should have
/// sent after `active` time: `t / 2R` during the ramp, `1 − R / 2t` after it.
#[must_use]
pub fn ramp_fraction(active: Duration, ramp: Option<Duration>) -> f64 {
    let t = active.as_secs_f64();
    let r = ramp.map_or(0.0, |r| r.as_secs_f64());
    if t <= 0.0 {
        return 1.0;
    }
    if r <= 0.0 {
        1.0
    } else if t <= r {
        t / (2.0 * r)
    } else {
        1.0 - r / (2.0 * t)
    }
}

/// Next theoretical send time `u` (ns of active time) after a frame costing `cost`
/// ns at full rate, with a linear ramp of `ramp` ns: the frame count is
/// `u² / (2·ramp·cost)` inside the ramp and linear afterwards.
fn ramp_step(u: f64, cost: f64, ramp: f64) -> f64 {
    let u = u.max(0.0);
    if u >= ramp {
        return u + cost;
    }
    let next = (u * u + 2.0 * ramp * cost).sqrt();
    if next <= ramp {
        next
    } else {
        // The ramp ends inside this frame: the rest of it is paid at full rate.
        let used = (ramp * ramp - u * u) / (2.0 * ramp * cost);
        ramp + (1.0 - used) * cost
    }
}

/// How well the pacer met its target.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RateAccuracy {
    /// Target in `unit` (`None` when unlimited).
    pub target: Option<f64>,
    /// Achieved average over the active (not paused) time, in `unit`.
    pub achieved: f64,
    /// `pps` or `bps` (layer-1 bits per second).
    pub unit: RateUnit,
    /// `achieved / target`.
    pub ratio: Option<f64>,
    /// Largest lateness of a wake-up relative to its deadline.
    pub max_jitter: Duration,
    /// Mean lateness of wake-ups.
    pub mean_jitter: Duration,
    /// Frames charged.
    pub frames: u64,
    /// Active time (excluding pauses).
    pub active: Duration,
}

/// Unit of [`RateAccuracy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateUnit {
    /// Frames per second.
    Pps,
    /// Layer-1 bits per second.
    Bps,
}

/// Token-bucket pacer. Usage per iteration of a TX loop:
///
/// 1. `let grant = pacer.poll(max_block)` — waits (at most `max_block`) until the next
///    burst may start; `None` means "not yet", so the caller can do housekeeping and
///    poll again;
/// 2. send up to `grant` frames, calling [`Pacer::charge`] for each.
///
/// When the caller is slower than the rate, a grant covers the whole backlog (bounded
/// by [`PacerConfig::max_backlog`]), so batching backends keep up.
#[derive(Debug)]
pub struct Pacer {
    rate: Rate,
    cfg: PacerConfig,
    origin: Instant,
    /// Theoretical time (ns since `origin`) at which the next frame may start.
    tat: f64,
    last_cost: f64,
    frames: u64,
    wire_bits: f64,
    waits: u64,
    jitter_sum: f64,
    jitter_max: f64,
    paused_at: Option<Instant>,
    paused_total: Duration,
}

impl Pacer {
    /// Creates a pacer starting now.
    ///
    /// # Errors
    /// [`TxError::Config`] if the rate is invalid (see [`Rate::validate`]).
    pub fn new(rate: Rate, mut cfg: PacerConfig) -> Result<Pacer, TxError> {
        rate.validate().map_err(TxError::Config)?;
        cfg.burst = cfg.burst.max(1);
        if matches!(rate, Rate::Ifg(_)) {
            cfg.burst = 1;
        }
        Ok(Pacer {
            rate,
            cfg,
            origin: Instant::now(),
            tat: 0.0,
            last_cost: rate.interval_ns(MIN_FRAME_LEN).unwrap_or(0.0),
            frames: 0,
            wire_bits: 0.0,
            waits: 0,
            jitter_sum: 0.0,
            jitter_max: 0.0,
            paused_at: None,
            paused_total: Duration::ZERO,
        })
    }

    /// The rate.
    #[must_use]
    pub fn rate(&self) -> Rate {
        self.rate
    }

    fn now_ns(&self) -> f64 {
        self.origin.elapsed().as_secs_f64() * 1e9
    }

    fn ns_to_instant(&self, ns: f64) -> Instant {
        self.origin + Duration::from_secs_f64(ns.max(0.0) / 1e9)
    }

    /// Waits until `deadline` according to the [`WaitMode`].
    pub fn wait_until(&self, deadline: Instant) {
        loop {
            let now = Instant::now();
            if now >= deadline {
                return;
            }
            let rem = deadline - now;
            let spin = match self.cfg.wait {
                WaitMode::Sleep => false,
                WaitMode::Spin => true,
                WaitMode::Hybrid { spin_threshold } => {
                    if rem > spin_threshold {
                        std::thread::sleep(rem - spin_threshold);
                        continue;
                    }
                    true
                }
            };
            if spin {
                if self.cfg.yield_while_spinning {
                    std::thread::yield_now();
                } else {
                    std::hint::spin_loop();
                }
            } else {
                std::thread::sleep(rem);
            }
        }
    }

    /// Waits at most `max_block` for the next grant. Returns the number of frames
    /// that may be sent now (`u32::MAX` when unlimited), or `None` if the deadline is
    /// further away than `max_block`.
    pub fn poll(&mut self, max_block: Duration) -> Option<u32> {
        if self.rate.is_unlimited() {
            return Some(u32::MAX);
        }
        let mut now = self.now_ns();
        if self.tat > now {
            let cap = now + max_block.as_secs_f64() * 1e9;
            let target = self.tat.min(cap);
            self.wait_until(self.ns_to_instant(target));
            now = self.now_ns();
            if now < self.tat {
                return None;
            }
            let late = now - self.tat;
            self.waits += 1;
            self.jitter_sum += late;
            self.jitter_max = self.jitter_max.max(late);
        }
        let backlog_cap = self.cfg.max_backlog.as_secs_f64() * 1e9;
        if now - self.tat > backlog_cap {
            self.tat = now - backlog_cap;
        }
        let behind = if self.last_cost > 0.0 {
            ((now - self.tat) / self.last_cost).floor()
        } else {
            0.0
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // clamped
        let extra = behind.clamp(0.0, f64::from(u32::MAX)) as u32;
        Some(self.cfg.burst.saturating_add(extra))
    }

    /// Accounts one frame of `frame_len` octets.
    pub fn charge(&mut self, frame_len: usize) {
        self.frames += 1;
        self.wire_bits += wire_bits(frame_len);
        if let Some(cost) = self.rate.interval_ns(frame_len) {
            let before = self.tat;
            self.tat = match self.cfg.ramp_up {
                Some(r) if !r.is_zero() => {
                    let paused = self.paused_total.as_secs_f64() * 1e9;
                    ramp_step(self.tat - paused, cost, r.as_secs_f64() * 1e9) + paused
                }
                _ => self.tat + cost,
            };
            self.last_cost = self.tat - before;
        }
    }

    /// Marks the start of a pause (pause time is excluded from the accuracy).
    pub fn pause(&mut self) {
        if self.paused_at.is_none() {
            self.paused_at = Some(Instant::now());
        }
    }

    /// Ends a pause; the schedule restarts from now (no catch-up burst).
    pub fn resume(&mut self) {
        if let Some(p) = self.paused_at.take() {
            self.paused_total += p.elapsed();
        }
        self.tat = self.tat.max(self.now_ns());
    }

    /// Active time so far (excluding pauses).
    #[must_use]
    pub fn active(&self) -> Duration {
        let paused = self.paused_total + self.paused_at.map_or(Duration::ZERO, |p| p.elapsed());
        self.origin.elapsed().saturating_sub(paused)
    }

    /// Accuracy so far.
    #[must_use]
    pub fn accuracy(&self) -> RateAccuracy {
        let active = self.active();
        let secs = active.as_secs_f64().max(1e-9);
        let (unit, target, achieved) = match self.rate.bits_per_second() {
            Some(bps) => (RateUnit::Bps, Some(bps), self.wire_bits / secs),
            None => (
                RateUnit::Pps,
                self.rate.frames_per_second(MIN_FRAME_LEN),
                self.frames as f64 / secs,
            ),
        };
        let mean = if self.waits == 0 {
            0.0
        } else {
            self.jitter_sum / self.waits as f64
        };
        RateAccuracy {
            target,
            achieved,
            unit,
            ratio: target.map(|t| achieved / (t * ramp_fraction(active, self.cfg.ramp_up))),
            max_jitter: Duration::from_secs_f64(self.jitter_max / 1e9),
            mean_jitter: Duration::from_secs_f64(mean / 1e9),
            frames: self.frames,
            active,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_table() {
        let cases: &[(&str, Rate)] = &[
            ("1000pps", Rate::Pps(1000.0)),
            ("1.5Mpps", Rate::Pps(1.5e6)),
            ("10 kpps", Rate::Pps(10_000.0)),
            ("2Kpps", Rate::Pps(2000.0)),
            ("1_000_000 pps", Rate::Pps(1e6)),
            ("10Gbps", Rate::Bps(1e10)),
            ("100 Mbps", Rate::Bps(1e8)),
            ("64000bps", Rate::Bps(64_000.0)),
            ("1 Tbps", Rate::Bps(1e12)),
            ("2.5gbit/s", Rate::Bps(2.5e9)),
            (
                "50%",
                Rate::LinkPercent {
                    percent: 50.0,
                    link_bps: None,
                },
            ),
            (
                " 12.5 % ",
                Rate::LinkPercent {
                    percent: 12.5,
                    link_bps: None,
                },
            ),
            (
                "line",
                Rate::LinkPercent {
                    percent: 100.0,
                    link_bps: None,
                },
            ),
            ("unlimited", Rate::Unlimited),
            ("MAX", Rate::Unlimited),
            ("ifg=10us", Rate::Ifg(Duration::from_micros(10))),
            ("ifg:1.5ms", Rate::Ifg(Duration::from_micros(1500))),
            ("IFG=100ns", Rate::Ifg(Duration::from_nanos(100))),
        ];
        for (s, want) in cases {
            assert_eq!(Rate::parse(s).unwrap(), *want, "{s}");
            assert_eq!(s.parse::<Rate>().unwrap(), *want, "{s}");
        }
    }

    #[test]
    fn parse_errors() {
        assert_eq!(Rate::parse(""), Err(RateParseError::Empty));
        assert_eq!(Rate::parse("  "), Err(RateParseError::Empty));
        assert!(matches!(
            Rate::parse("pps"),
            Err(RateParseError::InvalidNumber(_))
        ));
        assert!(matches!(
            Rate::parse("1.2.3pps"),
            Err(RateParseError::InvalidNumber(_))
        ));
        assert!(matches!(
            Rate::parse("100"),
            Err(RateParseError::UnknownUnit { .. })
        ));
        assert!(matches!(
            Rate::parse("100 xps"),
            Err(RateParseError::UnknownUnit { .. })
        ));
        assert!(matches!(
            Rate::parse("100 Xbps"),
            Err(RateParseError::UnknownUnit { .. })
        ));
        assert!(matches!(
            Rate::parse("0pps"),
            Err(RateParseError::OutOfRange(_))
        ));
        assert!(matches!(
            Rate::parse("150%"),
            Err(RateParseError::OutOfRange(_))
        ));
        assert!(matches!(
            Rate::parse("ifg=0us"),
            Err(RateParseError::OutOfRange(_))
        ));
    }

    #[test]
    fn display_round_trips_and_serde() {
        for s in [
            "1500000pps",
            "10000000000bps",
            "50%",
            "ifg=10000ns",
            "unlimited",
        ] {
            let r = Rate::parse(s).unwrap();
            assert_eq!(r.to_string(), s);
            assert_eq!(Rate::parse(&r.to_string()).unwrap(), r);
        }
        let json = serde_json::to_string(&Rate::Pps(1.5e6)).unwrap();
        assert_eq!(json, "\"1500000pps\"");
        let back: Rate = serde_json::from_str("\"10 Gbps\"").unwrap();
        assert_eq!(back, Rate::Bps(1e10));
        assert!(serde_json::from_str::<Rate>("\"fast\"").is_err());
    }

    #[test]
    fn bps_to_interval_math() {
        // 60-octet frame (64 with FCS) at 10 Gbps: 84 octets * 8 = 672 bits -> 67.2 ns.
        let r = Rate::Bps(10e9);
        assert!((r.interval_ns(60).unwrap() - 67.2).abs() < 1e-9);
        // Shorter frames are padded to 60 on the wire.
        assert!((r.interval_ns(42).unwrap() - 67.2).abs() < 1e-9);
        // 14.88 Mpps line rate.
        assert!((r.frames_per_second(60).unwrap() - 14_880_952.38).abs() < 1.0);
        // 1514-octet frames at 1 Gbps: 1538 * 8 = 12304 ns -> 81 274 pps.
        let g = Rate::Bps(1e9);
        assert!((g.interval_ns(1514).unwrap() - 12_304.0).abs() < 1e-6);
        // Percent of a 1 Gbps link.
        let p = Rate::parse("50%").unwrap();
        assert_eq!(p.interval_ns(60), None);
        assert!(p.validate().is_err());
        let p = p.with_link_speed(1e9);
        assert!(p.validate().is_ok());
        assert!((p.interval_ns(60).unwrap() - 1344.0).abs() < 1e-9);
        // pps and IFG ignore the length.
        assert!((Rate::Pps(1000.0).interval_ns(1500).unwrap() - 1e6).abs() < 1e-9);
        assert_eq!(
            Rate::Ifg(Duration::from_micros(3)).interval(9000),
            Some(Duration::from_micros(3))
        );
        assert_eq!(Rate::Unlimited.interval(60), None);
    }

    #[test]
    fn durations() {
        assert_eq!(parse_duration("10s").unwrap(), Duration::from_secs(10));
        assert_eq!(parse_duration("2m").unwrap(), Duration::from_secs(120));
        assert_eq!(parse_duration("1.5").unwrap(), Duration::from_millis(1500));
        assert_eq!(parse_duration("250ms").unwrap(), Duration::from_millis(250));
        assert!(parse_duration("3 fortnights").is_err());
    }

    #[test]
    fn token_bucket_burst() {
        // 100 pps, burst 5: the first grant covers 5 frames immediately, then the
        // pacer waits 5 intervals (50 ms) for the next burst.
        let cfg = PacerConfig {
            burst: 5,
            ..PacerConfig::default()
        };
        let mut p = Pacer::new(Rate::Pps(100.0), cfg).unwrap();
        let g = p.poll(Duration::from_millis(1)).unwrap();
        assert_eq!(g, 5);
        for _ in 0..5 {
            p.charge(60);
        }
        // Not ready within 1 ms.
        assert_eq!(p.poll(Duration::from_millis(1)), None);
        let t0 = Instant::now();
        let g = loop {
            if let Some(g) = p.poll(Duration::from_millis(100)) {
                break g;
            }
        };
        let waited = t0.elapsed();
        assert_eq!(g, 5);
        assert!(
            waited >= Duration::from_millis(40) && waited < Duration::from_millis(100),
            "{waited:?}"
        );
    }

    #[test]
    fn start_times() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        assert_eq!(
            parse_start_time("+10s", now).unwrap(),
            now + Duration::from_secs(10)
        );
        let at = |s: &str| {
            parse_start_time(s, now)
                .unwrap()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs_f64()
        };
        assert_eq!(at("1970-01-01T00:00:00Z"), 0.0);
        assert_eq!(at("2026-09-29T12:00:00Z"), 1_790_683_200.0);
        assert_eq!(at("2026-09-29T21:00:00+09:00"), 1_790_683_200.0);
        assert_eq!(at("2026-09-29 11:30:00.25-00:30"), 1_790_683_200.25);
        for bad in [
            "",
            "tomorrow",
            "2026-13-01T00:00:00Z",
            "2026-09-29T12:00:00",
            "+x",
        ] {
            assert!(parse_start_time(bad, now).is_err(), "{bad}");
        }
    }

    #[test]
    fn ramp_math_and_split() {
        // Inside the ramp the n-th frame is due at sqrt(2 R n c).
        let (r, c) = (1e9, 1e6); // 1 s ramp, 1000 pps
        let mut u = 0.0;
        for _ in 0..50 {
            u = ramp_step(u, c, r);
        }
        assert!((u - (2.0 * r * 50.0 * c).sqrt()).abs() < 1.0, "{u}");
        // 500 frames fill the ramp exactly (1000 pps × 1 s / 2), then linear.
        for _ in 50..500 {
            u = ramp_step(u, c, r);
        }
        assert!((u - r).abs() < 1e3, "{u}");
        assert!((ramp_step(u, c, r) - (u + c)).abs() < 1e-3);
        assert!(
            (ramp_fraction(Duration::from_secs(1), Some(Duration::from_secs(2))) - 0.25).abs()
                < 1e-9
        );
        assert!(
            (ramp_fraction(Duration::from_secs(4), Some(Duration::from_secs(2))) - 0.75).abs()
                < 1e-9
        );
        assert_eq!(ramp_fraction(Duration::from_secs(4), None), 1.0);

        assert_eq!(Rate::Pps(1000.0).split(4), Rate::Pps(250.0));
        assert_eq!(Rate::Bps(1e9).split(2), Rate::Bps(5e8));
        assert_eq!(
            Rate::Ifg(Duration::from_micros(10)).split(3),
            Rate::Ifg(Duration::from_micros(30))
        );
        assert_eq!(Rate::Unlimited.split(8), Rate::Unlimited);
        assert_eq!(Rate::Pps(10.0).split(0), Rate::Pps(10.0));
    }

    #[test]
    fn unlimited_grants_everything() {
        let mut p = Pacer::new(Rate::Unlimited, PacerConfig::default()).unwrap();
        assert_eq!(p.poll(Duration::ZERO), Some(u32::MAX));
        p.charge(100);
        let acc = p.accuracy();
        assert_eq!(acc.target, None);
        assert_eq!(acc.frames, 1);
    }

    #[test]
    fn ifg_forces_burst_one() {
        let cfg = PacerConfig {
            burst: 10,
            ..PacerConfig::default()
        };
        let mut p = Pacer::new(Rate::Ifg(Duration::from_millis(5)), cfg).unwrap();
        assert_eq!(p.poll(Duration::ZERO), Some(1));
    }

    #[test]
    fn unresolved_percent_is_a_config_error() {
        let r = Rate::parse("10%").unwrap();
        assert!(matches!(
            Pacer::new(r, PacerConfig::default()),
            Err(TxError::Config(_))
        ));
    }

    /// 1000 pps for 0.5 s must land within ±5 % (timing-sensitive, so it can be
    /// skipped on overloaded CI machines with `--skip pacer_accuracy`).
    #[test]
    fn pacer_accuracy_1000pps() {
        let mut p = Pacer::new(Rate::Pps(1000.0), PacerConfig::default()).unwrap();
        let end = Instant::now() + Duration::from_millis(500);
        let mut sent = 0u64;
        while Instant::now() < end {
            if let Some(g) = p.poll(Duration::from_millis(5)) {
                for _ in 0..g.min(1000) {
                    p.charge(60);
                    sent += 1;
                }
            }
        }
        let acc = p.accuracy();
        let ratio = acc.ratio.unwrap();
        assert!((0.95..=1.05).contains(&ratio), "{acc:?} sent={sent}");
        assert!((475..=525).contains(&sent), "sent={sent}");
    }
}
