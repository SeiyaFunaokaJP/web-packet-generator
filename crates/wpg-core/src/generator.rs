//! Field value generators (TODO §2.1 "変数・インクリメント・大量生成").
//!
//! Values are kept in the same human notation the GUI accepts (e.g. `192.168.1.1`,
//! `00:11:22:33:44:55`, `49.0001.1921.6800.1001.00`) and are parsed against the
//! target field's type when the stream is expanded.

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};

/// A rule that varies a field's value across the packets of a stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Generator {
    /// `start..=end` stepping by `step` (IPv4/IPv6/MAC/integers/NSAP System ID).
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
    /// One value per line of a file.
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

impl Generator {
    /// Number of distinct values this generator yields, when known without I/O.
    ///
    /// # Errors
    /// [`CoreError::NotImplemented`] for kinds whose cardinality needs field-type
    /// aware parsing (arrives in M1).
    pub fn cardinality(&self) -> Result<Option<u64>> {
        match self {
            Generator::List { values } => Ok(Some(values.len() as u64)),
            Generator::Counter {
                start,
                step,
                wrap: Some(wrap),
            } if *step != 0 => Ok(Some(wrap.abs_diff(*start) / step.unsigned_abs() + 1)),
            Generator::Counter { .. } | Generator::Random { .. } | Generator::Expr { .. } => {
                Ok(None)
            }
            Generator::Range { .. } | Generator::File { .. } => Err(CoreError::NotImplemented(
                "typed generator cardinality (M1)",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_defaults() {
        let g: Generator =
            serde_json::from_str(r#"{"kind":"range","start":"10.0.0.1","end":"10.0.0.9"}"#)
                .unwrap();
        assert_eq!(
            g,
            Generator::Range {
                start: "10.0.0.1".into(),
                end: "10.0.0.9".into(),
                step: 1
            }
        );
        let c: Generator = serde_json::from_str(r#"{"kind":"counter"}"#).unwrap();
        assert_eq!(
            c,
            Generator::Counter {
                start: 0,
                step: 1,
                wrap: None
            }
        );
    }

    #[test]
    fn cardinality() {
        let list = Generator::List {
            values: vec!["a".into(), "b".into()],
        };
        assert_eq!(list.cardinality(), Ok(Some(2)));
        let counter = Generator::Counter {
            start: 0,
            step: 2,
            wrap: Some(10),
        };
        assert_eq!(counter.cardinality(), Ok(Some(6)));
    }
}
