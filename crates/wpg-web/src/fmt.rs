//! Number / date formatting. Locale-sensitive parts go through the browser's `Intl`
//! (grouping, dates); SI-prefixed rates are locale independent (always `.` decimals,
//! like the CLI output, in every UI language) and pure, so they are unit tested natively.

use wpg_i18n::Locale;

/// `1234567.0, "pps"` → `1.23 Mpps`; three significant digits, SI prefixes.
pub(crate) fn si(value: f64, unit: &str) -> String {
    if !value.is_finite() {
        return format!("– {unit}");
    }
    let (v, prefix) = scale(value.abs());
    let sign = if value < 0.0 { "-" } else { "" };
    let digits = if prefix.is_empty() && v.fract() == 0.0 {
        0
    } else if v < 10.0 {
        2
    } else if v < 100.0 {
        1
    } else {
        0
    };
    format!("{sign}{v:.digits$} {prefix}{unit}")
}

fn scale(v: f64) -> (f64, &'static str) {
    const PREFIXES: [(f64, &str); 4] = [(1e12, "T"), (1e9, "G"), (1e6, "M"), (1e3, "k")];
    for (div, p) in PREFIXES {
        // Round first so 999_999 shows as `1.00 M`, not `1000 k`.
        if v >= div * 0.9995 {
            return (v / div, p);
        }
    }
    (v, "")
}

/// Octets with SI prefixes (`1.50 MB`).
pub(crate) fn bytes(n: u64) -> String {
    #[allow(clippy::cast_precision_loss)] // display only
    si(n as f64, "B")
}

/// `1234567` → `1,234,567` (the native fallback of [`int`]).
pub(crate) fn group_thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Seconds as `1h 02m 03s`, `4m 05s`, `12.3 s`.
pub(crate) fn duration(secs: f64) -> String {
    if !secs.is_finite() || secs < 0.0 {
        return "–".to_owned();
    }
    if secs < 60.0 {
        return format!("{secs:.1} s");
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // finite, >= 60
    let total = secs.round() as u64;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}h {m:02}m {s:02}s")
    } else {
        format!("{m}m {s:02}s")
    }
}

/// A ratio as a percentage with one decimal (`0.9987` → `99.9 %`).
pub(crate) fn percent(ratio: f64) -> String {
    format!("{:.1} %", ratio * 100.0)
}

/// A signed nanosecond interval (latency, jitter) as `850 ns`, `12.3 µs`,
/// `1.50 ms`, `2.00 s` (three significant digits).
pub(crate) fn nanos(ns: i64) -> String {
    const UNITS: [(f64, &str); 3] = [(1e9, "s"), (1e6, "ms"), (1e3, "µs")];
    let sign = if ns < 0 { "-" } else { "" };
    #[allow(clippy::cast_precision_loss)] // display only
    let a = ns.unsigned_abs() as f64;
    for (div, unit) in UNITS {
        // Round first so 999 999 ns shows as `1.00 ms`, not `1000 µs`.
        if a >= div * 0.9995 {
            let v = a / div;
            let digits = if v < 10.0 {
                2
            } else if v < 100.0 {
                1
            } else {
                0
            };
            return format!("{sign}{v:.digits$} {unit}");
        }
    }
    format!("{sign}{a} ns")
}

/// Integer with the locale's grouping (`Intl.NumberFormat`).
pub(crate) fn int(locale: Locale, n: u64) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        #[allow(clippy::cast_precision_loss)] // display only
        let v = n as f64;
        if let Some(s) = intl::number(locale, v) {
            return s;
        }
    }
    let _ = locale;
    group_thousands(n)
}

/// Unix milliseconds as a local date + time (`Intl.DateTimeFormat`, short / medium).
pub(crate) fn datetime(locale: Locale, unix_ms: u64) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        #[allow(clippy::cast_precision_loss)] // ms since 1970 fit in f64
        let v = unix_ms as f64;
        if let Some(s) = intl::datetime(locale, v) {
            return s;
        }
    }
    let _ = locale;
    format!("@{unix_ms}")
}

/// An RFC 3339 timestamp (log lines) as a local time of day.
pub(crate) fn time_of(locale: Locale, rfc3339: &str) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(rfc3339));
        let ms = d.get_time();
        if ms.is_finite() {
            if let Some(s) = intl::time(locale, ms) {
                return s;
            }
        }
    }
    let _ = locale;
    rfc3339
        .split_once('T')
        .map_or(rfc3339, |(_, t)| t.trim_end_matches('Z'))
        .to_owned()
}

#[cfg(target_arch = "wasm32")]
mod intl {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use js_sys::{Array, Function, Intl, Object, Reflect};
    use wasm_bindgen::JsValue;
    use wpg_i18n::Locale;

    thread_local! {
        static CACHE: RefCell<HashMap<(Locale, u8), Function>> = RefCell::new(HashMap::new());
    }

    fn formatter(locale: Locale, kind: u8) -> Function {
        CACHE.with(|c| {
            c.borrow_mut()
                .entry((locale, kind))
                .or_insert_with(|| {
                    let locales = Array::of1(&JsValue::from_str(locale.as_str()));
                    let opts = Object::new();
                    let set = |k: &str, v: &str| {
                        let _ = Reflect::set(&opts, &k.into(), &v.into());
                    };
                    match kind {
                        0 => Intl::NumberFormat::new(&locales, &opts).format(),
                        1 => {
                            set("dateStyle", "short");
                            set("timeStyle", "medium");
                            Intl::DateTimeFormat::new(&locales, &opts).format()
                        }
                        _ => {
                            set("timeStyle", "medium");
                            Intl::DateTimeFormat::new(&locales, &opts).format()
                        }
                    }
                })
                .clone()
        })
    }

    fn call(f: &Function, arg: &JsValue) -> Option<String> {
        f.call1(&JsValue::NULL, arg).ok()?.as_string()
    }

    pub(super) fn number(locale: Locale, v: f64) -> Option<String> {
        call(&formatter(locale, 0), &JsValue::from_f64(v))
    }

    pub(super) fn datetime(locale: Locale, ms: f64) -> Option<String> {
        let d = js_sys::Date::new(&JsValue::from_f64(ms));
        call(&formatter(locale, 1), &d.into())
    }

    pub(super) fn time(locale: Locale, ms: f64) -> Option<String> {
        let d = js_sys::Date::new(&JsValue::from_f64(ms));
        call(&formatter(locale, 2), &d.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn si_prefixes() {
        assert_eq!(si(0.0, "pps"), "0 pps");
        assert_eq!(si(999.0, "pps"), "999 pps");
        assert_eq!(si(12.5, "pps"), "12.5 pps");
        assert_eq!(si(1_000.0, "pps"), "1.00 kpps");
        assert_eq!(si(1_234_567.0, "pps"), "1.23 Mpps");
        assert_eq!(si(999_999.0, "bps"), "1.00 Mbps");
        assert_eq!(si(10e9, "bps"), "10.0 Gbps");
        assert_eq!(si(148_809_523.0, "pps"), "149 Mpps");
        assert_eq!(si(2.5e12, "bps"), "2.50 Tbps");
        assert_eq!(si(f64::NAN, "pps"), "– pps");
        assert_eq!(bytes(1_500_000), "1.50 MB");
    }

    #[test]
    fn grouping_and_durations() {
        assert_eq!(group_thousands(0), "0");
        assert_eq!(group_thousands(999), "999");
        assert_eq!(group_thousands(1000), "1,000");
        assert_eq!(group_thousands(1_234_567), "1,234,567");
        assert_eq!(int(Locale::Ja, 12_345), "12,345");
        assert_eq!(duration(12.34), "12.3 s");
        assert_eq!(duration(245.0), "4m 05s");
        assert_eq!(duration(3723.0), "1h 02m 03s");
        assert_eq!(duration(-1.0), "–");
        assert_eq!(percent(0.99871), "99.9 %");
        assert_eq!(nanos(0), "0 ns");
        assert_eq!(nanos(850), "850 ns");
        assert_eq!(nanos(-850), "-850 ns");
        assert_eq!(nanos(12_345), "12.3 µs");
        assert_eq!(nanos(999_999), "1.00 ms");
        assert_eq!(nanos(1_500_000), "1.50 ms");
        assert_eq!(nanos(2_000_000_000), "2.00 s");
        assert_eq!(nanos(i64::MIN), "-9223372037 s");
        assert_eq!(
            time_of(Locale::En, "2026-09-29T12:00:01.123Z"),
            "12:00:01.123"
        );
    }
}
