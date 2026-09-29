//! Error types for wpg-core.

use thiserror::Error;

/// Convenience alias used throughout wpg-core.
pub type Result<T, E = CoreError> = core::result::Result<T, E>;

/// Errors produced while defining, building or parsing packets.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum CoreError {
    /// A protocol id was referenced that is not in the catalog.
    #[error("unknown protocol `{0}`")]
    UnknownProtocol(String),
    /// A field name was referenced that the protocol does not define.
    #[error("protocol `{protocol}` has no field `{field}`")]
    UnknownField {
        /// Protocol id.
        protocol: String,
        /// Field name.
        field: String,
    },
    /// A field's bit width is outside what its type supports.
    #[error("field `{field}`: invalid bit width {bits} (allowed {min}..={max})")]
    InvalidWidth {
        /// Field name.
        field: String,
        /// Requested width.
        bits: u16,
        /// Minimum allowed width.
        min: u16,
        /// Maximum allowed width.
        max: u16,
    },
    /// A value does not fit / does not match the field type.
    #[error("field `{field}`: invalid value: {reason}")]
    InvalidValue {
        /// Field name.
        field: String,
        /// Human readable reason.
        reason: String,
    },
    /// Two fields in one protocol share a name.
    #[error("protocol `{protocol}` defines field `{field}` more than once")]
    DuplicateField {
        /// Protocol id.
        protocol: String,
        /// Field name.
        field: String,
    },
    /// Placeholder for functionality that is planned but not written yet.
    #[error("not implemented yet: {0}")]
    NotImplemented(&'static str),
}
