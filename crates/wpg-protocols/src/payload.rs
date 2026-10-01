//! Payload protocols.

use wpg_core::{protocol, ProtocolDef, Synthesizer};

/// Generated payload (`payload`). Its fields are generator parameters, not wire
/// fields (see [`wpg_core::synth::payload`]):
///
/// * `mode`: `fixed` (0, `data` zero-extended / cut to `length` when `length` > 0),
///   `pattern` (1, `data` repeated to `length`; empty `data` = 00 01 02 ...),
///   `random` (2, `length` octets from `seed`), `counter` (3, fixed content with
///   `counter_value` embedded big endian at `counter_offset`, `counter_width` 4 or
///   8 octets);
/// * `data`, `length`, `seed`, `counter_offset`, `counter_width`, `counter_value`.
///
/// `counter_value` is set per packet by the stream expander later; parsing a
/// `payload` layer yields `fixed` mode with the octets in `data`.
#[must_use]
pub fn payload() -> ProtocolDef {
    let mut d = protocol! {
        id: "payload",
        name: "Payload",
        layer: Payload,
        fields: {
            mode: enum(8) {
                0 => fixed,
                1 => pattern,
                2 => random,
                3 => counter,
            } = 0;
            data: bytes(to_end) = [];
            length: uint(32) = 0;
            seed: uint(64) = 0;
            counter_offset: uint(32) = 0;
            counter_width: enum(8) { 4 => u32, 8 => u64 } = 4;
            counter_value: uint(64) = 0;
        },
    };
    d.synth = Some(Synthesizer::Payload);
    d
}

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
    fn payload_is_valid() {
        payload().validate().unwrap();
    }

    #[test]
    fn raw_matches_core_builtin() {
        assert_eq!(raw(), wpg_core::raw_protocol());
    }
}
