//! # wpg-snmp
//!
//! The SNMP manager behind wpg's SNMP tool screen (ADR-0010): unlike the
//! `snmp` protocol of `wpg-protocols`, which only *crafts* packets, this crate
//! talks to agents and receives notifications.
//!
//! * [`ber`], [`value`], [`pdu`]: BER codec (long-form lengths), SMI values,
//!   all PDU types, v1 / v2c messages;
//! * [`usm`]: SNMPv3 messages and the User-based Security Model (MD5, SHA-1,
//!   SHA-2 authentication; DES, AES-128/192/256 privacy);
//! * [`client`]: async Get / GetNext / GetBulk / Walk / Set / Trap / Inform;
//! * [`receiver`]: trap / inform listener;
//! * [`mib`]: SMIv1 / SMIv2 MIB parser, the OID ⇄ name registry with the
//!   bundled IETF modules, and readable value formatting;
//! * [`pcap`]: recording of the exchanged datagrams as pcap / pcapng;
//! * [`agent`]: a tiny in-memory agent for tests and demos.

pub mod agent;
pub mod ber;
pub mod client;
pub mod mib;
pub mod oid;
pub mod pcap;
pub mod pdu;
pub mod receiver;
pub mod usm;
pub mod value;

pub use client::{Client, ClientConfig, Error, Walk};
pub use oid::Oid;
pub use pdu::{Pdu, PduType, Version};
pub use value::{Value, VarBind};
