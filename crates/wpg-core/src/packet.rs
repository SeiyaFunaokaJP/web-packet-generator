//! Packet instances: a stack of [`Layer`]s whose fields hold [`FieldValue`]s.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, Ipv6Addr};

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};

/// A concrete field value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldValue {
    /// Unsigned integer (also used for `enum` and `flags`).
    UInt(u64),
    /// Signed integer.
    Int(i64),
    /// Raw octets.
    Bytes(Vec<u8>),
    /// MAC address.
    Mac([u8; 6]),
    /// IPv4 address.
    Ipv4(Ipv4Addr),
    /// IPv6 address.
    Ipv6(Ipv6Addr),
    /// NSAP address octets.
    Nsap(Vec<u8>),
}

/// One protocol layer of a packet (an instance of a `ProtocolDef`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layer {
    /// Protocol id (see `ProtocolDef::id`).
    pub protocol: String,
    /// Explicitly set ("Manual") field values; unset fields use default / Auto.
    #[serde(default)]
    pub fields: BTreeMap<String, FieldValue>,
}

impl Layer {
    /// Creates an empty layer of the given protocol.
    #[must_use]
    pub fn new(protocol: impl Into<String>) -> Self {
        Self {
            protocol: protocol.into(),
            fields: BTreeMap::new(),
        }
    }

    /// Builder: sets a field value (switching the field to "Manual").
    #[must_use]
    pub fn with(mut self, field: impl Into<String>, value: FieldValue) -> Self {
        self.fields.insert(field.into(), value);
        self
    }
}

/// A packet: layers from outermost (L2) to innermost.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Packet {
    /// Layer stack.
    pub layers: Vec<Layer>,
}

impl Packet {
    /// Creates an empty packet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a layer on top of the stack.
    pub fn push(&mut self, layer: Layer) -> &mut Self {
        self.layers.push(layer);
        self
    }

    /// Serialises the packet to wire bytes.
    ///
    /// # Errors
    /// Always [`CoreError::NotImplemented`] until the builder lands in M1.
    pub fn build(&self) -> Result<Vec<u8>> {
        Err(CoreError::NotImplemented("packet builder (M1)"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_stack_and_serde() {
        let mut pkt = Packet::new();
        pkt.push(Layer::new("ethernet2").with("dst", FieldValue::Mac([0xff; 6])))
            .push(Layer::new("ipv4").with("src", FieldValue::Ipv4(Ipv4Addr::new(192, 0, 2, 1))));
        assert_eq!(pkt.layers.len(), 2);

        let json = serde_json::to_string(&pkt).unwrap();
        let back: Packet = serde_json::from_str(&json).unwrap();
        assert_eq!(back, pkt);
        assert_eq!(
            pkt.build(),
            Err(CoreError::NotImplemented("packet builder (M1)"))
        );
    }
}
