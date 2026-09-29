//! Payload protocols.

use wpg_core::{protocol, ProtocolDef};

/// Raw payload octets (`raw`); identical to [`wpg_core::raw_protocol`], which the
/// parser uses for undecodable remainders.
#[must_use]
pub fn raw() -> ProtocolDef {
    protocol! {
        id: "raw",
        name: "Raw payload",
        layer: Payload,
        fields: {
            data: bytes(to_end) = [];
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_matches_core_builtin() {
        assert_eq!(raw(), wpg_core::raw_protocol());
    }
}
