//! # wpg-core
//!
//! OS-independent heart of wpg: the declarative protocol definition IR
//! ([`ProtocolDef`] / [`FieldDef`] / [`FieldType`]), packet instances
//! ([`Packet`] / [`Layer`]), the builder ([`Packet::build`]) and parser ([`parse()`])
//! with [`Diagnostic`]s, and field value generators ([`Generator`]).
//!
//! The same IR is produced by Rust protocol definitions (`wpg-protocols`) and, later,
//! by user YAML definitions, and drives build / parse / GUI / docs generation
//! (TODO §2.1, §3 "プロトコル定義方式").
//!
//! This crate must keep building for `wasm32-unknown-unknown`.

mod bits;
pub mod build;
pub mod checksum;
pub mod constraint;
pub mod diag;
pub mod error;
pub mod generator;
pub mod ir;
mod macros;
pub mod notation;
pub mod packet;
pub mod parse;
pub mod stream;
pub mod synth;

pub use build::{Built, FieldLayout, LayerLayout, TrailerKind, TrailerLayout};
pub use constraint::{Constraint, ConstraintRule};
pub use diag::{DiagLevel, Diagnostic};
pub use error::{CoreError, Result};
pub use generator::{CompiledGenerator, Generator, GeneratorError, GeneratorState};
pub use ir::{
    raw_protocol, AutoCompute, Binding, BindingEntry, ChecksumCoverage, ChecksumKind, EnumVariant,
    FieldDef, FieldType, FlagBit, ItemDef, Length, LengthOf, LengthUnit, ProtocolDef,
    ProtocolLayer, PseudoHeader, Resolve, Synthesizer, TlvDef, TlvVariant, RAW_PROTOCOL_ID,
};
pub use packet::{FcsMode, FieldValue, Layer, Packet, TlvItem};
pub use parse::{parse, parse_with_fcs, Parsed};
