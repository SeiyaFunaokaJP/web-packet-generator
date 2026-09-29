//! Diagnostics produced by the builder, parser and constraint checks.
//!
//! Every diagnostic carries a Fluent message key plus named arguments (for the
//! localised UI) and an English fallback text (for logs / the CLI).

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Severity of a [`Diagnostic`] (TODO §2.1 "制約検査").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagLevel {
    /// Informational (e.g. trailing bytes after a PDU).
    Info,
    /// The packet can be built but violates an RFC or is inconsistent.
    Warning,
    /// The packet is not what was asked for (e.g. a value could not be represented)
    /// or a mandatory rule is violated.
    Error,
}

impl fmt::Display for DiagLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            DiagLevel::Info => "info",
            DiagLevel::Warning => "warning",
            DiagLevel::Error => "error",
        })
    }
}

/// Message keys used by wpg-core. Each has English fallback text built in; the UI
/// resolves the key (with [`Diagnostic::args`]) through Fluent.
pub mod keys {
    /// Integer field outside `[min, max]`. Args: `field`, `value`, `min`, `max`.
    pub const FIELD_OUT_OF_RANGE: &str = "diag-field-out-of-range";
    /// Field differs from the expected value. Args: `field`, `value`, `expected`.
    pub const FIELD_NOT_EQUAL: &str = "diag-field-not-equal";
    /// Field has a discouraged value. Args: `field`, `value`.
    pub const FIELD_FORBIDDEN_VALUE: &str = "diag-field-forbidden-value";
    /// Span shorter than required. Args: `span`, `len`, `min`.
    pub const LENGTH_TOO_SHORT: &str = "diag-length-too-short";
    /// Span longer than allowed. Args: `span`, `len`, `max`.
    pub const LENGTH_TOO_LONG: &str = "diag-length-too-long";
    /// Manual length differs from the computed one. Args: `field`, `value`, `expected`.
    pub const LENGTH_MISMATCH: &str = "diag-length-mismatch";
    /// Manual checksum differs from the computed one. Args: `field`, `value`, `expected`.
    pub const CHECKSUM_MISMATCH: &str = "diag-checksum-mismatch";
    /// Manual next-protocol value does not announce the next layer.
    /// Args: `field`, `value`, `expected`, `next`.
    pub const NEXT_PROTOCOL_MISMATCH: &str = "diag-next-protocol-mismatch";
    /// Auto value does not fit in the field. Args: `field`, `value`, `bits`.
    pub const AUTO_OVERFLOW: &str = "diag-auto-overflow";
    /// Auto length is not a whole number of units. Args: `field`, `len`, `unit`.
    pub const LENGTH_NOT_ALIGNED: &str = "diag-length-not-aligned";
    /// Pseudo-header checksum without an IP layer below. Args: `field`.
    pub const NO_PSEUDO_HEADER: &str = "diag-no-pseudo-header";
    /// Algorithm not implemented yet. Args: `field`, `what`.
    pub const NOT_IMPLEMENTED: &str = "diag-not-implemented";
    /// Layer was truncated on purpose. Args: `len`.
    pub const TRUNCATED: &str = "diag-truncated";
    /// Input ends inside a header. Args: `protocol`, `offset`.
    pub const SHORT_HEADER: &str = "diag-short-header";
    /// A length field points beyond the available data. Args: `field`, `len`, `available`.
    pub const LENGTH_EXCEEDS_DATA: &str = "diag-length-exceeds-data";
    /// Bytes after the end of the innermost PDU. Args: `len`.
    pub const TRAILING_BYTES: &str = "diag-trailing-bytes";
    /// A field type the parser cannot decode yet. Args: `protocol`, `field`.
    pub const UNSUPPORTED_FIELD: &str = "diag-unsupported-field";
}

/// One finding about a packet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Severity.
    pub level: DiagLevel,
    /// Index of the layer in the packet, if the finding is about one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<usize>,
    /// Field name, if the finding is about one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// Fluent message key (see [`keys`]).
    pub key: String,
    /// Named Fluent arguments.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, String>,
    /// English fallback text.
    pub message: String,
}

impl Diagnostic {
    /// Creates a diagnostic.
    #[must_use]
    pub fn new(level: DiagLevel, key: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            level,
            layer: None,
            field: None,
            key: key.into(),
            args: BTreeMap::new(),
            message: message.into(),
        }
    }

    /// Builder: attaches the layer index.
    #[must_use]
    pub fn at_layer(mut self, layer: usize) -> Self {
        self.layer = Some(layer);
        self
    }

    /// Builder: attaches the field name (also added as the `field` argument).
    #[must_use]
    pub fn on_field(mut self, field: impl Into<String>) -> Self {
        let field = field.into();
        self.args.insert("field".into(), field.clone());
        self.field = Some(field);
        self
    }

    /// Builder: adds a named argument.
    #[must_use]
    #[allow(clippy::needless_pass_by_value)] // by value for call-site ergonomics
    pub fn arg(mut self, name: impl Into<String>, value: impl fmt::Display) -> Self {
        self.args.insert(name.into(), value.to_string());
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.level)?;
        if let Some(l) = self.layer {
            write!(f, " [layer {l}")?;
            if let Some(field) = &self.field {
                write!(f, ".{field}")?;
            }
            f.write_str("]")?;
        }
        write!(f, ": {}", self.message)
    }
}
