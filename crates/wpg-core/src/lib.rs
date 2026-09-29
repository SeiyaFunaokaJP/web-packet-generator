//! # wpg-core
//!
//! OS-independent heart of wpg: the declarative protocol definition IR
//! ([`ProtocolDef`] / [`FieldDef`] / [`FieldType`]), packet instances
//! ([`Packet`] / [`Layer`]) and field value generators ([`Generator`]).
//!
//! The same IR is produced by Rust protocol definitions (`wpg-protocols`) and, later,
//! by user YAML definitions, and drives build / parse / GUI / docs generation
//! (TODO §2.1, §3 "プロトコル定義方式").
//!
//! This crate must keep building for `wasm32-unknown-unknown`.

pub mod error;
pub mod generator;
pub mod ir;
pub mod packet;

pub use error::{CoreError, Result};
pub use generator::Generator;
pub use ir::{
    AutoCompute, ChecksumKind, EnumVariant, FieldDef, FieldType, FlagBit, Length, ProtocolDef,
    ProtocolLayer,
};
pub use packet::{FieldValue, Layer, Packet};
