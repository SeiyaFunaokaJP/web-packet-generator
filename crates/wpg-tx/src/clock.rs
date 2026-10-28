//! Clocks for the [`crate::rate::Pacer`].
//!
//! * [`ClockSource::Monotonic`]: `Instant` (`clock_gettime(CLOCK_MONOTONIC)`,
//!   through the vDSO on Linux: ~20 ns per read, more where the vDSO falls back to
//!   a system call).
//! * [`ClockSource::Tsc`] (`x86_64`): the time-stamp counter read with `rdtsc`
//!   (a few ns, no system call), converted to nanoseconds with a frequency
//!   calibrated once against `Instant`. Only used when the TSC is invariant
//!   (CPUID `0x8000_0007` EDX bit 8, or the Linux flags `constant_tsc` and
//!   `nonstop_tsc`), so it ticks at a constant rate across P-/C-states and cores.
//! * [`ClockSource::Auto`] (default): TSC when usable, else monotonic.
//!
//! A pacer that spins re-reads the clock in a tight loop; a cheaper clock means a
//! shorter loop, so the wake-up lands closer to the deadline.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Which clock the pacer uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClockSource {
    /// TSC when invariant, else monotonic.
    #[default]
    Auto,
    /// `Instant` / `CLOCK_MONOTONIC`.
    Monotonic,
    /// The `x86_64` TSC (falls back to monotonic where it is not usable).
    Tsc,
}

impl std::str::FromStr for ClockSource {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "auto" => Ok(ClockSource::Auto),
            "monotonic" | "mono" => Ok(ClockSource::Monotonic),
            "tsc" => Ok(ClockSource::Tsc),
            _ => Err(format!("unknown clock `{s}` (auto, monotonic, tsc)")),
        }
    }
}

/// TSC calibration: `ns = (tsc - tsc0) * ns_per_tick`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TscCalibration {
    /// Nanoseconds per TSC tick.
    pub ns_per_tick: f64,
    /// TSC frequency in Hz.
    pub hz: f64,
}

/// A started clock: nanoseconds since its creation.
#[derive(Debug, Clone, Copy)]
pub struct Clock {
    origin: Instant,
    kind: Kind,
}

#[derive(Debug, Clone, Copy)]
enum Kind {
    Monotonic,
    #[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
    Tsc {
        tsc0: u64,
        ns_per_tick: f64,
    },
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn rdtsc() -> u64 {
    // `_rdtsc` is safe in newer Rust versions and `unsafe` in older ones.
    #[allow(unused_unsafe)]
    // SAFETY: RDTSC exists on every x86_64 CPU and has no memory effects.
    unsafe {
        core::arch::x86_64::_rdtsc()
    }
}

/// Whether the CPU reports an invariant TSC.
#[must_use]
pub fn tsc_invariant() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        #[allow(unused_unsafe)]
        // SAFETY: CPUID exists on every x86_64 CPU.
        let max_ext = unsafe { core::arch::x86_64::__cpuid(0x8000_0000) }.eax;
        if max_ext >= 0x8000_0007 {
            #[allow(unused_unsafe)]
            // SAFETY: as above; leaf checked against the maximum extended leaf.
            let edx = unsafe { core::arch::x86_64::__cpuid(0x8000_0007) }.edx;
            if edx & (1 << 8) != 0 {
                return true;
            }
        }
        // Hypervisors often hide the CPUID bit but Linux still reports the flags.
        std::fs::read_to_string("/proc/cpuinfo").is_ok_and(|s| {
            s.lines().find(|l| l.starts_with("flags")).is_some_and(|l| {
                let has = |name: &str| l.split_whitespace().any(|w| w == name);
                has("constant_tsc") && has("nonstop_tsc")
            })
        })
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// The TSC calibration (measured once, ~20 ms), or `None` where the TSC is not
/// usable (not `x86_64`, not invariant, or a calibration that looks wrong).
#[must_use]
pub fn tsc_calibration() -> Option<TscCalibration> {
    static CAL: OnceLock<Option<TscCalibration>> = OnceLock::new();
    *CAL.get_or_init(calibrate)
}

fn calibrate() -> Option<TscCalibration> {
    #[cfg(target_arch = "x86_64")]
    {
        if !tsc_invariant() {
            return None;
        }
        // Two 10 ms windows; their mean is accurate to well below
        // 0.1 % (the brackets are a few tens of ns wide).
        let mut sum = 0.0;
        for _ in 0..2 {
            let t0 = Instant::now();
            let c0 = rdtsc();
            while t0.elapsed() < Duration::from_millis(10) {
                std::hint::spin_loop();
            }
            let c1 = rdtsc();
            let ns = t0.elapsed().as_secs_f64() * 1e9;
            sum += c1.wrapping_sub(c0) as f64 / ns * 1e9;
        }
        let hz = sum / 2.0;
        // Sanity: 100 MHz .. 10 GHz.
        (1e8..1e10).contains(&hz).then(|| TscCalibration {
            ns_per_tick: 1e9 / hz,
            hz,
        })
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        None
    }
}

impl Clock {
    /// Starts a clock of the given source (`Tsc` / `Auto` fall back to monotonic
    /// where the TSC is not usable).
    #[must_use]
    pub fn new(source: ClockSource) -> Clock {
        // Calibrate (first use only) before taking the origin.
        let kind = match source {
            ClockSource::Monotonic => Kind::Monotonic,
            ClockSource::Auto | ClockSource::Tsc => Self::tsc_kind().unwrap_or(Kind::Monotonic),
        };
        Clock {
            origin: Instant::now(),
            kind,
        }
    }

    #[cfg(target_arch = "x86_64")]
    fn tsc_kind() -> Option<Kind> {
        let cal = tsc_calibration()?;
        Some(Kind::Tsc {
            tsc0: rdtsc(),
            ns_per_tick: cal.ns_per_tick,
        })
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn tsc_kind() -> Option<Kind> {
        None
    }

    /// Whether this clock reads the TSC.
    #[must_use]
    pub fn is_tsc(&self) -> bool {
        matches!(self.kind, Kind::Tsc { .. })
    }

    /// Nanoseconds since the clock was started.
    #[inline]
    #[must_use]
    pub fn now_ns(&self) -> f64 {
        match self.kind {
            Kind::Monotonic => self.origin.elapsed().as_secs_f64() * 1e9,
            #[cfg(target_arch = "x86_64")]
            Kind::Tsc { tsc0, ns_per_tick } => rdtsc().wrapping_sub(tsc0) as f64 * ns_per_tick,
            #[cfg(not(target_arch = "x86_64"))]
            Kind::Tsc { .. } => self.origin.elapsed().as_secs_f64() * 1e9,
        }
    }

    /// The `Instant` at `ns` on this clock (for sleeping).
    #[must_use]
    pub fn instant_at(&self, ns: f64) -> Instant {
        self.origin + Duration::from_secs_f64(ns.max(0.0) / 1e9)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sources() {
        assert_eq!("auto".parse::<ClockSource>(), Ok(ClockSource::Auto));
        assert_eq!("TSC".parse::<ClockSource>(), Ok(ClockSource::Tsc));
        assert_eq!(
            "monotonic".parse::<ClockSource>(),
            Ok(ClockSource::Monotonic)
        );
        assert!("rtc".parse::<ClockSource>().is_err());
    }

    #[test]
    fn clocks_track_instant() {
        for src in [ClockSource::Monotonic, ClockSource::Auto] {
            let c = Clock::new(src);
            // Locally one measurement must be within tolerance; on CI (`CI`
            // set) a descheduled thread may skew one run, so allow retries.
            let attempts = if std::env::var_os("CI").is_some() {
                5
            } else {
                1
            };
            let mut a = 0.0;
            let mut last = String::new();
            let mut ok = false;
            for _ in 0..attempts {
                let t = Instant::now();
                std::thread::sleep(Duration::from_millis(20));
                a = c.now_ns();
                let b = t.elapsed().as_secs_f64() * 1e9;
                // Within 2 ms + 2 % of each other (calibration error, scheduling).
                if (a - b).abs() < 2e6 + b * 0.02 {
                    ok = true;
                    break;
                }
                last = format!("{src:?}: clock {a} vs instant {b}");
            }
            assert!(ok, "{last}");
            assert!(c.now_ns() >= a);
        }
        if let Some(cal) = tsc_calibration() {
            assert!(cal.hz > 1e8, "{cal:?}");
            assert!(Clock::new(ClockSource::Tsc).is_tsc());
        }
        assert!(!Clock::new(ClockSource::Monotonic).is_tsc());
    }
}
