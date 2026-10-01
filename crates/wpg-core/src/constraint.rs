//! Declarative constraints on protocol fields (TODO §2.1 "制約検査").
//!
//! Constraints are data (serialisable, usable from YAML); there is deliberately no
//! "custom function" escape hatch. They are evaluated by the builder on the final
//! field values and produce [`Diagnostic`]s.

use serde::{Deserialize, Serialize};

use crate::diag::{keys, DiagLevel, Diagnostic};
use crate::ir::LengthOf;
use crate::packet::FieldValue;

/// A check with a severity and an optional custom message key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Constraint {
    /// Severity of the diagnostic emitted on violation.
    pub level: DiagLevel,
    /// The rule.
    #[serde(flatten)]
    pub rule: ConstraintRule,
    /// Overrides the default Fluent key of the rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_key: Option<String>,
}

/// The kinds of constraint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "rule", rename_all = "snake_case")]
pub enum ConstraintRule {
    /// Integer field within `min..=max`.
    FieldInRange {
        /// Field name.
        field: String,
        /// Inclusive minimum.
        min: i64,
        /// Inclusive maximum.
        max: i64,
    },
    /// Integer field equal to `value`.
    FieldEquals {
        /// Field name.
        field: String,
        /// Expected value.
        value: i64,
    },
    /// Integer field different from `value`.
    FieldNotEquals {
        /// Field name.
        field: String,
        /// Discouraged value.
        value: i64,
    },
    /// Span at least `octets` long.
    MinLength {
        /// Measured span.
        of: LengthOf,
        /// Minimum.
        octets: u64,
    },
    /// Span at most `octets` long.
    MaxLength {
        /// Measured span.
        of: LengthOf,
        /// Maximum.
        octets: u64,
    },
}

impl Constraint {
    /// Creates a constraint with the rule's default message key.
    #[must_use]
    pub fn new(level: DiagLevel, rule: ConstraintRule) -> Self {
        Self {
            level,
            rule,
            message_key: None,
        }
    }
}

/// Lengths of the layer being checked, in octets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Spans {
    pub(crate) header: u64,
    pub(crate) payload: u64,
}

impl Spans {
    pub(crate) fn get(self, of: LengthOf) -> u64 {
        match of {
            LengthOf::Header => self.header,
            LengthOf::Payload => self.payload,
            LengthOf::Total => self.header + self.payload,
        }
    }
}

pub(crate) fn span_name(of: LengthOf) -> &'static str {
    match of {
        LengthOf::Header => "header",
        LengthOf::Payload => "payload",
        LengthOf::Total => "total",
    }
}

fn as_i128(v: &FieldValue) -> Option<i128> {
    match v {
        FieldValue::UInt(x) => Some(i128::from(*x)),
        FieldValue::Int(x) => Some(i128::from(*x)),
        _ => None,
    }
}

/// Evaluates `c`; `value` looks up the final value of a field of the layer.
pub(crate) fn evaluate<'a>(
    c: &Constraint,
    layer: usize,
    value: impl Fn(&str) -> Option<&'a FieldValue>,
    spans: Spans,
) -> Option<Diagnostic> {
    let d = match &c.rule {
        ConstraintRule::FieldInRange { field, min, max } => {
            let v = as_i128(value(field)?)?;
            if (i128::from(*min)..=i128::from(*max)).contains(&v) {
                return None;
            }
            Diagnostic::new(
                c.level,
                keys::FIELD_OUT_OF_RANGE,
                format!("`{field}` is {v}, expected {min}..={max}"),
            )
            .on_field(field)
            .arg("value", v)
            .arg("min", min)
            .arg("max", max)
        }
        ConstraintRule::FieldEquals { field, value: want } => {
            let v = as_i128(value(field)?)?;
            if v == i128::from(*want) {
                return None;
            }
            Diagnostic::new(
                c.level,
                keys::FIELD_NOT_EQUAL,
                format!("`{field}` is {v}, expected {want}"),
            )
            .on_field(field)
            .arg("value", v)
            .arg("expected", want)
        }
        ConstraintRule::FieldNotEquals { field, value: bad } => {
            let v = as_i128(value(field)?)?;
            if v != i128::from(*bad) {
                return None;
            }
            Diagnostic::new(
                c.level,
                keys::FIELD_FORBIDDEN_VALUE,
                format!("`{field}` should not be {v}"),
            )
            .on_field(field)
            .arg("value", v)
        }
        ConstraintRule::MinLength { of, octets } => {
            let len = spans.get(*of);
            if len >= *octets {
                return None;
            }
            let span = span_name(*of);
            Diagnostic::new(
                c.level,
                keys::LENGTH_TOO_SHORT,
                format!("{span} length is {len} octets, minimum is {octets}"),
            )
            .arg("span", span)
            .arg("len", len)
            .arg("min", octets)
        }
        ConstraintRule::MaxLength { of, octets } => {
            let len = spans.get(*of);
            if len <= *octets {
                return None;
            }
            let span = span_name(*of);
            Diagnostic::new(
                c.level,
                keys::LENGTH_TOO_LONG,
                format!("{span} length is {len} octets, maximum is {octets}"),
            )
            .arg("span", span)
            .arg("len", len)
            .arg("max", octets)
        }
    };
    let mut d = d.at_layer(layer);
    if let Some(k) = &c.message_key {
        d.key.clone_from(k);
    }
    Some(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_and_equality() {
        let c = Constraint::new(
            DiagLevel::Error,
            ConstraintRule::FieldInRange {
                field: "ihl".into(),
                min: 5,
                max: 15,
            },
        );
        let spans = Spans {
            header: 20,
            payload: 0,
        };
        let four = FieldValue::UInt(4);
        let five = FieldValue::UInt(5);
        let d = evaluate(&c, 1, |_| Some(&four), spans).unwrap();
        assert_eq!(d.level, DiagLevel::Error);
        assert_eq!(d.key, keys::FIELD_OUT_OF_RANGE);
        assert_eq!(d.layer, Some(1));
        assert_eq!(d.args["value"], "4");
        assert!(evaluate(&c, 1, |_| Some(&five), spans).is_none());

        let c = Constraint {
            message_key: Some("custom".into()),
            ..Constraint::new(
                DiagLevel::Warning,
                ConstraintRule::MinLength {
                    of: LengthOf::Total,
                    octets: 60,
                },
            )
        };
        let d = evaluate(&c, 0, |_| None, spans).unwrap();
        assert_eq!(d.key, "custom");
    }

    #[test]
    fn serde_shape() {
        let c = Constraint::new(
            DiagLevel::Warning,
            ConstraintRule::FieldNotEquals {
                field: "ttl".into(),
                value: 0,
            },
        );
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(
            json,
            r#"{"level":"warning","rule":"field_not_equals","field":"ttl","value":0}"#
        );
        assert_eq!(serde_json::from_str::<Constraint>(&json).unwrap(), c);
    }
}
