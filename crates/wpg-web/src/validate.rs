//! Client-side validation of the job form (mirrors `wpg_tx::rate::Rate::parse` and
//! `parse_duration`, which cannot be linked into the WASM bundle). The server stays
//! authoritative; these checks only give instant hints.

/// A parsed rate (for the hint under the input).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum RateKind {
    /// Frames per second.
    Pps(f64),
    /// Layer-1 bits per second.
    Bps(f64),
    /// Percentage of the link speed.
    Percent(f64),
    /// Fixed inter-frame gap, in seconds.
    Ifg(f64),
    /// As fast as possible.
    Unlimited,
}

/// Why a value was rejected: an i18n key (`ui-valid-*`).
pub(crate) type Invalid = &'static str;

fn split_number(s: &str) -> (&str, &str) {
    let end = s
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_digit() || *c == '.' || *c == '_'))
        .map_or(s.len(), |(i, _)| i);
    (&s[..end], s[end..].trim())
}

fn number(num: &str) -> Result<f64, Invalid> {
    let cleaned: String = num.chars().filter(|c| *c != '_').collect();
    if cleaned.is_empty() {
        return Err("ui-valid-number");
    }
    cleaned.parse::<f64>().map_err(|_| "ui-valid-number")
}

/// Parses a duration like `10s`, `1.5m`, `100ms`, `10us` into seconds.
pub(crate) fn parse_duration(s: &str) -> Result<f64, Invalid> {
    let t = s.trim();
    if t.is_empty() {
        return Err("ui-valid-empty");
    }
    let (num, unit) = split_number(t);
    let v = number(num)?;
    let scale = match unit.to_ascii_lowercase().as_str() {
        "ns" => 1e-9,
        "us" | "µs" | "μs" => 1e-6,
        "ms" => 1e-3,
        "s" | "" => 1.0,
        "m" | "min" => 60.0,
        "h" => 3600.0,
        _ => return Err("ui-valid-duration-unit"),
    };
    let secs = v * scale;
    if secs.is_finite() && secs >= 0.0 {
        Ok(secs)
    } else {
        Err("ui-valid-range")
    }
}

/// Parses a rate: `1000pps`, `1.5Mpps`, `10Gbps`, `50%`, `line`, `ifg=10us`,
/// `unlimited`.
pub(crate) fn parse_rate(s: &str) -> Result<RateKind, Invalid> {
    let t = s.trim();
    if t.is_empty() {
        return Err("ui-valid-empty");
    }
    let lower = t.to_ascii_lowercase();
    match lower.as_str() {
        "unlimited" | "max" | "inf" | "infinite" => return Ok(RateKind::Unlimited),
        "line" | "linerate" | "line-rate" | "line_rate" => return Ok(RateKind::Percent(100.0)),
        _ => {}
    }
    if let Some(rest) = lower
        .strip_prefix("ifg=")
        .or_else(|| lower.strip_prefix("ifg:"))
        .or_else(|| lower.strip_prefix("ifg "))
    {
        let d = parse_duration(rest)?;
        return if d > 0.0 {
            Ok(RateKind::Ifg(d))
        } else {
            Err("ui-valid-range")
        };
    }
    let (num, unit) = split_number(t);
    let v = number(num)?;
    if !v.is_finite() || v <= 0.0 {
        return Err("ui-valid-range");
    }
    if unit == "%" {
        return if v > 100.0 {
            Err("ui-valid-percent")
        } else {
            Ok(RateKind::Percent(v))
        };
    }
    let u = unit.to_ascii_lowercase();
    let (prefix, pps) = if let Some(p) = u.strip_suffix("pps") {
        (p, true)
    } else if let Some(p) = u.strip_suffix("bps") {
        (p, false)
    } else if let Some(p) = u.strip_suffix("bit/s") {
        (p, false)
    } else {
        return Err("ui-valid-rate-unit");
    };
    let mult = match prefix {
        "" => 1.0,
        "k" => 1e3,
        "m" => 1e6,
        "g" => 1e9,
        "t" => 1e12,
        _ => return Err("ui-valid-rate-unit"),
    };
    Ok(if pps {
        RateKind::Pps(v * mult)
    } else {
        RateKind::Bps(v * mult)
    })
}

/// A strictly positive integer (`_` separators allowed).
pub(crate) fn parse_positive(s: &str) -> Result<u64, Invalid> {
    let cleaned: String = s.trim().chars().filter(|c| *c != '_').collect();
    if cleaned.is_empty() {
        return Err("ui-valid-empty");
    }
    match cleaned.parse::<u64>() {
        Ok(0) => Err("ui-valid-range"),
        Ok(v) => Ok(v),
        Err(_) => Err("ui-valid-integer"),
    }
}

/// Dry-run capture file name: relative, no `..`, no absolute path.
pub(crate) fn check_capture_name(s: &str) -> Result<(), Invalid> {
    let t = s.trim();
    if t.is_empty() {
        return Err("ui-valid-empty");
    }
    let bad = t.starts_with('/')
        || t.starts_with('\\')
        || t.contains(':')
        || t.split(['/', '\\'])
            .any(|c| c.is_empty() || c == "." || c == "..");
    if bad {
        Err("ui-valid-capture-name")
    } else {
        Ok(())
    }
}

/// Validates an optional field: empty is fine (server default).
pub(crate) fn optional<T>(s: &str, f: impl Fn(&str) -> Result<T, Invalid>) -> Result<(), Invalid> {
    if s.trim().is_empty() {
        Ok(())
    } else {
        f(s).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates() {
        assert_eq!(parse_rate("1000pps"), Ok(RateKind::Pps(1000.0)));
        assert_eq!(parse_rate("1Mpps"), Ok(RateKind::Pps(1e6)));
        assert_eq!(parse_rate(" 1.5 kpps "), Ok(RateKind::Pps(1500.0)));
        assert_eq!(parse_rate("10Gbps"), Ok(RateKind::Bps(1e10)));
        assert_eq!(parse_rate("100 Mbit/s"), Ok(RateKind::Bps(1e8)));
        assert_eq!(parse_rate("1_000pps"), Ok(RateKind::Pps(1000.0)));
        assert_eq!(parse_rate("50%"), Ok(RateKind::Percent(50.0)));
        assert_eq!(parse_rate("line"), Ok(RateKind::Percent(100.0)));
        assert_eq!(parse_rate("unlimited"), Ok(RateKind::Unlimited));
        assert_eq!(parse_rate("MAX"), Ok(RateKind::Unlimited));
        assert!(matches!(parse_rate("ifg=10us"), Ok(RateKind::Ifg(d)) if (d - 1e-5).abs() < 1e-12));
        assert_eq!(parse_rate(""), Err("ui-valid-empty"));
        assert_eq!(parse_rate("fast"), Err("ui-valid-number"));
        assert_eq!(parse_rate("10"), Err("ui-valid-rate-unit"));
        assert_eq!(parse_rate("10 Xpps"), Err("ui-valid-rate-unit"));
        assert_eq!(parse_rate("0pps"), Err("ui-valid-range"));
        assert_eq!(parse_rate("150%"), Err("ui-valid-percent"));
        assert_eq!(parse_rate("ifg=0s"), Err("ui-valid-range"));
        assert_eq!(parse_rate("ifg=3 parsecs"), Err("ui-valid-duration-unit"));
    }

    #[test]
    fn durations() {
        assert_eq!(parse_duration("10s"), Ok(10.0));
        assert_eq!(parse_duration("2m"), Ok(120.0));
        assert_eq!(parse_duration("1h"), Ok(3600.0));
        assert_eq!(parse_duration("250ms"), Ok(0.25));
        assert_eq!(parse_duration("5"), Ok(5.0));
        assert_eq!(parse_duration("5 days"), Err("ui-valid-duration-unit"));
        assert_eq!(parse_duration(""), Err("ui-valid-empty"));
    }

    #[test]
    fn integers_and_names() {
        assert_eq!(parse_positive("1_000"), Ok(1000));
        assert_eq!(parse_positive("0"), Err("ui-valid-range"));
        assert_eq!(parse_positive("-3"), Err("ui-valid-integer"));
        assert_eq!(parse_positive("1.5"), Err("ui-valid-integer"));
        assert_eq!(check_capture_name("run1.pcapng"), Ok(()));
        assert_eq!(check_capture_name("lab/run1"), Ok(()));
        assert_eq!(check_capture_name("../x"), Err("ui-valid-capture-name"));
        assert_eq!(check_capture_name("/etc/x"), Err("ui-valid-capture-name"));
        assert_eq!(check_capture_name("C:\\x"), Err("ui-valid-capture-name"));
        assert_eq!(check_capture_name("a//b"), Err("ui-valid-capture-name"));
        assert_eq!(optional("", parse_positive), Ok(()));
        assert_eq!(optional("x", parse_positive), Err("ui-valid-integer"));
    }
}
