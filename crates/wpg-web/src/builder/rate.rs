//! Send-time estimate for the builder (TODO §4 M4 "所要時間の見積表示").
//!
//! `wpg-tx` (which owns `Rate`) is not compiled to WASM, so this is a small mirror
//! of its rate notation and layer-1 arithmetic: `1000pps`, `1.5Mpps`, `10Gbps`,
//! `ifg=10us`, `unlimited`, `50%` / `line`. Only what the estimate needs is
//! reproduced; a native test checks it against `wpg_tx::Rate`.

use wpg_template::TxProfile;

/// Minimum Ethernet frame length without FCS (as `wpg_tx::rate::MIN_FRAME_LEN`).
const MIN_FRAME_LEN: usize = 60;
/// Preamble + SFD + IFG + FCS octets per frame (as `wpg_tx::rate::WIRE_OVERHEAD`).
const WIRE_OVERHEAD: usize = 24;

/// A parsed rate, reduced to what a duration estimate can use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum EstRate {
    /// Frames per second.
    Pps(f64),
    /// Layer-1 bits per second.
    Bps(f64),
    /// Seconds between frame starts.
    Ifg(f64),
    /// As fast as possible, or a link percentage (the NIC speed is unknown here).
    Unknown,
}

impl EstRate {
    /// Parses the template's `tx.rate` notation. `None` on anything `wpg-tx` would
    /// reject.
    pub(crate) fn parse(s: &str) -> Option<EstRate> {
        let t = s.trim();
        let lower = t.to_ascii_lowercase();
        match lower.as_str() {
            "" => return None,
            "unlimited" | "max" | "inf" | "infinite" | "line" | "linerate" | "line-rate"
            | "line_rate" => return Some(EstRate::Unknown),
            _ => {}
        }
        if let Some(rest) = ["ifg=", "ifg:", "ifg "]
            .iter()
            .find_map(|p| lower.strip_prefix(p))
        {
            let (v, unit) = number_unit(rest.trim())?;
            let scale = match unit {
                "ns" => 1e-9,
                "us" | "µs" | "μs" => 1e-6,
                "ms" => 1e-3,
                "s" | "" => 1.0,
                "m" | "min" => 60.0,
                "h" => 3600.0,
                _ => return None,
            };
            let secs = v * scale;
            return (secs > 0.0 && secs.is_finite()).then_some(EstRate::Ifg(secs));
        }
        let (v, unit) = number_unit(&lower)?;
        if !(v.is_finite() && v > 0.0) {
            return None;
        }
        if unit == "%" {
            return (v <= 100.0).then_some(EstRate::Unknown);
        }
        let (prefix, pps) = match unit.strip_suffix("pps") {
            Some(p) => (p, true),
            None => (
                unit.strip_suffix("bps")
                    .or_else(|| unit.strip_suffix("bit/s"))?,
                false,
            ),
        };
        let mult = match prefix {
            "" => 1.0,
            "k" => 1e3,
            "m" => 1e6,
            "g" => 1e9,
            "t" => 1e12,
            _ => return None,
        };
        Some(if pps {
            EstRate::Pps(v * mult)
        } else {
            EstRate::Bps(v * mult)
        })
    }

    /// Frames per second for frames of `frame_len` octets (`None` = unknown).
    pub(crate) fn frames_per_second(self, frame_len: usize) -> Option<f64> {
        match self {
            EstRate::Pps(p) => Some(p),
            EstRate::Ifg(secs) => Some(1.0 / secs),
            #[allow(clippy::cast_precision_loss)] // frame lengths are small
            EstRate::Bps(b) => {
                Some(b / ((frame_len.max(MIN_FRAME_LEN) + WIRE_OVERHEAD) as f64 * 8.0))
            }
            EstRate::Unknown => None,
        }
    }
}

/// Splits `12.5 kpps` into `(12.5, "kpps")` (digits, `.` and `_` separators).
fn number_unit(s: &str) -> Option<(f64, &str)> {
    let end = s
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_digit() || *c == '.' || *c == '_'))
        .map_or(s.len(), |(i, _)| i);
    let num: String = s[..end].chars().filter(|c| *c != '_').collect();
    if num.is_empty() {
        return None;
    }
    Some((num.parse().ok()?, s[end..].trim()))
}

/// Seconds a send of this template takes: the packet count (`tx.count`, else the
/// stream length `stream_len`) at `tx.rate`, capped by `tx.duration_secs`.
/// `None` when it cannot be told (no / unlimited / percentage rate on an unbounded
/// or uncapped send, or an invalid rate).
pub(crate) fn estimate_secs(
    tx: &TxProfile,
    stream_len: Option<u64>,
    frame_len: usize,
) -> Option<f64> {
    let cap = tx.duration_secs.filter(|d| d.is_finite() && *d > 0.0);
    let packets = tx.count.or(stream_len);
    let fps = tx
        .rate
        .as_deref()
        .and_then(EstRate::parse)
        .and_then(|r| r.frames_per_second(frame_len));
    #[allow(clippy::cast_precision_loss)] // display only
    let at_rate = packets.zip(fps).map(|(n, f)| n as f64 / f);
    match (at_rate, cap) {
        (Some(a), Some(c)) => Some(a.min(c)),
        (a, c) => a.or(c),
    }
}

/// A send time: milliseconds below one second (`< 1 ms`, `254 ms`), else
/// [`crate::fmt::duration`].
pub(crate) fn format_secs(secs: f64) -> String {
    if secs < 0.001 {
        "< 1 ms".to_owned()
    } else if secs < 1.0 {
        format!("{:.0} ms", secs * 1000.0)
    } else {
        crate::fmt::duration(secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_wpg_tx_rate() {
        for s in [
            "1000pps", "1.5Mpps", "10 kpps", "10Gbps", "100 Mbps", "64000bps", "1_000pps",
            "2Tbit/s", "ifg=10us", "ifg:1ms", "ifg 2s",
        ] {
            let ours = EstRate::parse(s).unwrap_or_else(|| panic!("{s}"));
            let theirs = wpg_tx::rate::Rate::parse(s).unwrap();
            for len in [14usize, 60, 64, 1500, 9000] {
                let a = ours.frames_per_second(len).unwrap();
                let b = theirs.frames_per_second(len).unwrap();
                assert!((a - b).abs() / b < 1e-9, "{s} @ {len}: {a} vs {b}");
            }
        }
        for s in ["unlimited", "max", "50%", "line"] {
            assert_eq!(EstRate::parse(s), Some(EstRate::Unknown), "{s}");
            assert!(wpg_tx::rate::Rate::parse(s).is_ok());
        }
        for s in ["", "abc", "0pps", "-5pps", "101%", "10xps", "ifg=0s", "pps"] {
            assert_eq!(EstRate::parse(s), None, "{s}");
            assert!(wpg_tx::rate::Rate::parse(s).is_err(), "{s}");
        }
    }

    #[test]
    fn estimates() {
        let tx = |rate: Option<&str>, count: Option<u64>, dur: Option<f64>| TxProfile {
            count,
            duration_secs: dur,
            rate: rate.map(str::to_owned),
            burst: None,
        };
        // 254 packets at 1000 pps.
        assert_eq!(
            estimate_secs(&tx(Some("1000pps"), None, None), Some(254), 64),
            Some(0.254)
        );
        // tx.count wins over the stream length.
        assert_eq!(
            estimate_secs(&tx(Some("10pps"), Some(100), None), Some(5), 64),
            Some(10.0)
        );
        // Capped by the duration.
        assert_eq!(
            estimate_secs(&tx(Some("1pps"), None, Some(3.0)), Some(10), 64),
            Some(3.0)
        );
        // Unbounded stream with a duration.
        assert_eq!(
            estimate_secs(&tx(Some("1pps"), None, Some(3.0)), None, 64),
            Some(3.0)
        );
        // 1 Gbps with 60-octet frames: 1e9 / 672 bits = 1,488,095 fps.
        let s = estimate_secs(&tx(Some("1Gbps"), None, None), Some(1_488_095), 60).unwrap();
        assert!((s - 1.0).abs() < 1e-6, "{s}");
        // Unknown cases.
        assert_eq!(estimate_secs(&tx(None, None, None), Some(10), 64), None);
        assert_eq!(
            estimate_secs(&tx(Some("unlimited"), None, None), Some(10), 64),
            None
        );
        assert_eq!(
            estimate_secs(&tx(Some("1000pps"), None, None), None, 64),
            None
        );
        assert_eq!(
            estimate_secs(&tx(Some("bogus"), None, None), Some(10), 64),
            None
        );
    }

    #[test]
    fn secs_formatting() {
        assert_eq!(format_secs(0.000_5), "< 1 ms");
        assert_eq!(format_secs(0.254), "254 ms");
        assert_eq!(format_secs(12.34), "12.3 s");
    }
}
