//! Built-in content generators for protocols with a
//! [`Synthesizer`](crate::ir::Synthesizer) (their fields are parameters, not wire
//! fields).

use std::collections::BTreeMap;

use crate::generator::Rng;
use crate::packet::FieldValue;

/// `mode` values of the payload generator.
pub mod payload_mode {
    /// `data` verbatim, zero-extended / cut to `length` when `length` > 0.
    pub const FIXED: u64 = 0;
    /// `data` repeated to `length` octets (empty `data`: 00 01 02 ... ff 00 ...).
    pub const PATTERN: u64 = 1;
    /// `length` pseudo-random octets from `seed` (deterministic).
    pub const RANDOM: u64 = 2;
    /// `fixed` content with `counter_value` embedded big endian at `counter_offset`
    /// (`counter_width` 4 or 8 octets).
    pub const COUNTER: u64 = 3;
}

fn uint(p: &BTreeMap<String, FieldValue>, name: &str, default: u64) -> u64 {
    p.get(name).and_then(FieldValue::as_u64).unwrap_or(default)
}

/// Upper bound on generated payload sizes (a jumbo frame is far below this).
pub const MAX_PAYLOAD: usize = 1 << 20;

/// Generates payload octets from the parameters `mode`, `data`, `length`, `seed`,
/// `counter_offset`, `counter_width` and `counter_value` (missing ones default to
/// fixed / empty / 0 / 0 / 0 / 4 / 0). `length` is capped at [`MAX_PAYLOAD`].
#[must_use]
pub fn payload(p: &BTreeMap<String, FieldValue>) -> Vec<u8> {
    let data = p
        .get("data")
        .and_then(FieldValue::as_bytes)
        .unwrap_or_default();
    let length = usize::try_from(uint(p, "length", 0))
        .unwrap_or(MAX_PAYLOAD)
        .min(MAX_PAYLOAD);
    let fixed = || {
        let mut v = data.to_vec();
        if length > 0 {
            v.resize(length, 0);
        }
        v
    };
    match uint(p, "mode", payload_mode::FIXED) {
        payload_mode::PATTERN => {
            if data.is_empty() {
                #[allow(clippy::cast_possible_truncation)] // wraps on purpose
                (0..length).map(|i| i as u8).collect()
            } else {
                data.iter().copied().cycle().take(length).collect()
            }
        }
        payload_mode::RANDOM => {
            let mut rng = Rng::new(uint(p, "seed", 0));
            let mut v = Vec::with_capacity(length + 8);
            while v.len() < length {
                v.extend_from_slice(&rng.next_u64().to_be_bytes());
            }
            v.truncate(length);
            v
        }
        payload_mode::COUNTER => {
            let mut v = fixed();
            let width = if uint(p, "counter_width", 4) == 8 {
                8
            } else {
                4
            };
            let off = usize::try_from(uint(p, "counter_offset", 0))
                .unwrap_or(MAX_PAYLOAD)
                .min(MAX_PAYLOAD);
            let value = uint(p, "counter_value", 0);
            if v.len() < off + width {
                v.resize(off + width, 0);
            }
            let be = value.to_be_bytes();
            v[off..off + width].copy_from_slice(&be[8 - width..]);
            v
        }
        _ => fixed(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(kv: &[(&str, FieldValue)]) -> BTreeMap<String, FieldValue> {
        kv.iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect()
    }

    #[test]
    fn modes() {
        let d = || FieldValue::Bytes(vec![0xaa, 0xbb, 0xcc]);
        assert_eq!(payload(&params(&[("data", d())])), [0xaa, 0xbb, 0xcc]);
        assert_eq!(
            payload(&params(&[("data", d()), ("length", 5u8.into())])),
            [0xaa, 0xbb, 0xcc, 0, 0]
        );
        assert_eq!(
            payload(&params(&[
                ("mode", payload_mode::PATTERN.into()),
                ("data", d()),
                ("length", 7u8.into())
            ])),
            [0xaa, 0xbb, 0xcc, 0xaa, 0xbb, 0xcc, 0xaa]
        );
        assert_eq!(
            payload(&params(&[
                ("mode", payload_mode::PATTERN.into()),
                ("length", 3u8.into())
            ])),
            [0, 1, 2]
        );
        let r = |seed: u64| {
            payload(&params(&[
                ("mode", payload_mode::RANDOM.into()),
                ("seed", seed.into()),
                ("length", 13u8.into()),
            ]))
        };
        assert_eq!(r(1).len(), 13);
        assert_eq!(r(1), r(1));
        assert_ne!(r(1), r(2));
        assert_eq!(
            payload(&params(&[
                ("mode", payload_mode::COUNTER.into()),
                ("data", FieldValue::Bytes(vec![0xee; 8])),
                ("counter_offset", 2u8.into()),
                ("counter_value", 0x0102_0304u32.into()),
            ])),
            [0xee, 0xee, 1, 2, 3, 4, 0xee, 0xee]
        );
        assert_eq!(
            payload(&params(&[
                ("mode", payload_mode::COUNTER.into()),
                ("counter_width", 8u8.into()),
                ("counter_value", 5u8.into()),
            ])),
            [0, 0, 0, 0, 0, 0, 0, 5]
        );
    }
}
