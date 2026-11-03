//! Typed template variables: the value types of `variables[].type` and the checks
//! of `choices`, `min` / `max` (shared by lint, compile and the GUI).

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use wpg_core::notation;

use crate::Variable;

/// Value type of a template variable (the field-type names of the protocol catalog).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VarType {
    /// Unsigned integer: `64`, `0x0800`, `0b101`.
    Uint,
    /// Signed integer: `-1`, `+5`, `0x10`.
    Int,
    /// `true` / `false` (or `1` / `0`); substituted as `1` / `0`.
    Bool,
    /// Any text.
    String,
    /// IPv4 address.
    Ipv4,
    /// IPv6 address.
    Ipv6,
    /// MAC address.
    Mac,
    /// Octets in hex: `"00 01 02"`.
    Bytes,
}

impl VarType {
    /// All types, in documentation order.
    pub const ALL: [VarType; 8] = [
        VarType::Uint,
        VarType::Int,
        VarType::Bool,
        VarType::String,
        VarType::Ipv4,
        VarType::Ipv6,
        VarType::Mac,
        VarType::Bytes,
    ];

    /// The keyword used in templates (`uint`, `ipv4`, ...).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            VarType::Uint => "uint",
            VarType::Int => "int",
            VarType::Bool => "bool",
            VarType::String => "string",
            VarType::Ipv4 => "ipv4",
            VarType::Ipv6 => "ipv6",
            VarType::Mac => "mac",
            VarType::Bytes => "bytes",
        }
    }

    /// Whether `min` / `max` apply.
    #[must_use]
    pub fn is_numeric(self) -> bool {
        matches!(self, VarType::Uint | VarType::Int)
    }
}

impl fmt::Display for VarType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a variable value was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarError {
    /// The text is not a value of the declared type.
    Type(VarType),
    /// The number is outside `min..=max` (either bound may be open).
    Range {
        /// Lower bound.
        min: Option<i64>,
        /// Upper bound.
        max: Option<i64>,
    },
    /// The value is not one of `choices`.
    Choice(Vec<String>),
}

impl VarError {
    /// Fluent key of the lint issue (`lint-var-*`).
    #[must_use]
    pub fn key(&self) -> &'static str {
        match self {
            VarError::Type(_) => "lint-var-bad-type",
            VarError::Range { .. } => "lint-var-out-of-range",
            VarError::Choice(_) => "lint-var-not-a-choice",
        }
    }

    /// The expected value, for messages: `ipv4`, `1..4094`, `a, b, c`.
    #[must_use]
    pub fn expected(&self) -> String {
        match self {
            VarError::Type(t) => t.to_string(),
            VarError::Range { min, max } => format!(
                "{}..{}",
                min.map(|v| v.to_string()).unwrap_or_default(),
                max.map(|v| v.to_string()).unwrap_or_default()
            ),
            VarError::Choice(c) => c.join(", "),
        }
    }
}

/// Parses `s` as an integer of the numeric type `t`.
fn parse_number(s: &str, t: VarType) -> Option<i128> {
    match t {
        VarType::Uint => notation::parse_uint(s)
            .ok()
            .filter(|v| *v <= u128::from(u64::MAX))
            .and_then(|v| i128::try_from(v).ok()),
        VarType::Int => notation::parse_int(s)
            .ok()
            .filter(|v| i64::try_from(*v).is_ok()),
        _ => None,
    }
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.trim() {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

/// Checks `value` against the type of `v` only (not `choices` / `min` / `max`).
///
/// # Errors
/// [`VarError::Type`].
pub fn check_type(t: VarType, value: &str) -> Result<(), VarError> {
    let ok = match t {
        VarType::Uint | VarType::Int => parse_number(value, t).is_some(),
        VarType::Bool => parse_bool(value).is_some(),
        VarType::String => true,
        VarType::Ipv4 => value.trim().parse::<std::net::Ipv4Addr>().is_ok(),
        VarType::Ipv6 => value.trim().parse::<std::net::Ipv6Addr>().is_ok(),
        VarType::Mac => notation::parse_mac(value).is_ok(),
        VarType::Bytes => notation::parse_bytes(value).is_ok(),
    };
    if ok {
        Ok(())
    } else {
        Err(VarError::Type(t))
    }
}

/// Checks a value of `v`: its type, `choices` and `min` / `max`.
///
/// # Errors
/// The first [`VarError`] found.
pub fn check_value(v: &Variable, value: &str) -> Result<(), VarError> {
    if let Some(t) = v.kind {
        check_type(t, value)?;
        if t.is_numeric() && (v.min.is_some() || v.max.is_some()) {
            let n = parse_number(value, t).ok_or(VarError::Type(t))?;
            if v.min.is_some_and(|m| n < i128::from(m)) || v.max.is_some_and(|m| n > i128::from(m))
            {
                return Err(VarError::Range {
                    min: v.min,
                    max: v.max,
                });
            }
        }
    }
    if !v.choices.is_empty() && !v.choices.iter().any(|c| same_value(v.kind, c, value)) {
        return Err(VarError::Choice(v.choices.clone()));
    }
    Ok(())
}

/// Whether two texts are the same value of type `t` (`0x10` = `16`, `true` = `1`).
fn same_value(t: Option<VarType>, a: &str, b: &str) -> bool {
    match t {
        Some(t @ (VarType::Uint | VarType::Int)) => {
            matches!((parse_number(a, t), parse_number(b, t)), (Some(x), Some(y)) if x == y)
        }
        Some(VarType::Bool) => {
            matches!((parse_bool(a), parse_bool(b)), (Some(x), Some(y)) if x == y)
        }
        _ => a.trim() == b.trim(),
    }
}

/// The text substituted for `${name}`: booleans become `1` / `0`, others are kept.
#[must_use]
pub fn substitution(v: &Variable, value: &str) -> String {
    match (v.kind, parse_bool(value)) {
        (Some(VarType::Bool), Some(b)) => u8::from(b).to_string(),
        _ => value.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn var(kind: Option<VarType>) -> Variable {
        Variable {
            name: "v".into(),
            kind,
            ..Variable::default()
        }
    }

    #[test]
    fn types() {
        let ok = |t, s| check_type(t, s).is_ok();
        assert!(ok(VarType::Uint, "64") && ok(VarType::Uint, "0x0800") && ok(VarType::Uint, "0b1"));
        assert!(ok(VarType::Uint, "18446744073709551615"));
        assert!(!ok(VarType::Uint, "18446744073709551616") && !ok(VarType::Uint, "-1"));
        assert!(!ok(VarType::Uint, "ten"));
        assert!(ok(VarType::Int, "-5") && ok(VarType::Int, "+5") && !ok(VarType::Int, "x"));
        assert!(!ok(VarType::Int, "9223372036854775808"));
        assert!(ok(VarType::Bool, "true") && ok(VarType::Bool, "0") && !ok(VarType::Bool, "yes"));
        assert!(ok(VarType::String, "anything at all"));
        assert!(ok(VarType::Ipv4, "192.0.2.1") && !ok(VarType::Ipv4, "192.0.2.256"));
        assert!(!ok(VarType::Ipv4, "2001:db8::1"));
        assert!(ok(VarType::Ipv6, "2001:db8::1") && !ok(VarType::Ipv6, "192.0.2.1"));
        assert!(ok(VarType::Mac, "02:00:00:00:00:01") && ok(VarType::Mac, "0200.0000.0001"));
        assert!(!ok(VarType::Mac, "02:00:00:00:00"));
        assert!(ok(VarType::Bytes, "00 01 02") && !ok(VarType::Bytes, "zz"));
        assert_eq!(
            check_type(VarType::Ipv4, "x"),
            Err(VarError::Type(VarType::Ipv4))
        );
    }

    #[test]
    fn untyped_accepts_anything() {
        assert!(check_value(&var(None), "whatever").is_ok());
    }

    #[test]
    fn range() {
        let v = Variable {
            min: Some(1),
            max: Some(4094),
            ..var(Some(VarType::Uint))
        };
        assert!(check_value(&v, "1").is_ok() && check_value(&v, "0xffe").is_ok());
        let e = check_value(&v, "4095").unwrap_err();
        assert_eq!(e.key(), "lint-var-out-of-range");
        assert_eq!(e.expected(), "1..4094");
        assert!(check_value(&v, "0").is_err());
        assert_eq!(check_value(&v, "abc"), Err(VarError::Type(VarType::Uint)));
        let open = Variable {
            min: Some(-10),
            ..var(Some(VarType::Int))
        };
        assert!(check_value(&open, "-10").is_ok() && check_value(&open, "-11").is_err());
        assert_eq!(check_value(&open, "-11").unwrap_err().expected(), "-10..");
    }

    #[test]
    fn choices() {
        let v = Variable {
            choices: vec!["tcp".into(), "udp".into()],
            ..var(Some(VarType::String))
        };
        assert!(check_value(&v, "udp").is_ok());
        let e = check_value(&v, "icmp").unwrap_err();
        assert_eq!(e.key(), "lint-var-not-a-choice");
        assert_eq!(e.expected(), "tcp, udp");
        // numbers compare by value
        let n = Variable {
            choices: vec!["16".into(), "32".into()],
            ..var(Some(VarType::Uint))
        };
        assert!(check_value(&n, "0x10").is_ok() && check_value(&n, "17").is_err());
        // untyped choices compare as text
        let u = Variable {
            choices: vec!["a".into()],
            ..var(None)
        };
        assert!(check_value(&u, "a").is_ok() && check_value(&u, "b").is_err());
    }

    #[test]
    fn bool_substitution() {
        let b = var(Some(VarType::Bool));
        assert_eq!(substitution(&b, "true"), "1");
        assert_eq!(substitution(&b, "false"), "0");
        assert_eq!(substitution(&var(None), "true"), "true");
    }

    #[test]
    fn keywords_round_trip() {
        for t in VarType::ALL {
            let y = serde_yaml::to_string(&t).unwrap();
            assert_eq!(y.trim(), t.as_str());
        }
    }
}
