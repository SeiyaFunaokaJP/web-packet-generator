//! Hex encoding helpers.

/// Decodes a hex string. ASCII whitespace and `_` are ignored; `#` starts a comment
/// that runs to the end of the line. Panics on malformed input (test helper).
///
/// ```
/// assert_eq!(wpg_testutil::hex("00 11 ff # tail\n0a"), vec![0x00, 0x11, 0xff, 0x0a]);
/// ```
#[must_use]
pub fn hex(s: &str) -> Vec<u8> {
    let mut digits = Vec::new();
    for line in s.lines() {
        let line = line.split('#').next().unwrap_or("");
        for c in line.chars() {
            if c.is_whitespace() || c == '_' {
                continue;
            }
            let d = c
                .to_digit(16)
                .unwrap_or_else(|| panic!("invalid hex digit {c:?}"));
            digits.push(u8::try_from(d).expect("hex digit fits in u8"));
        }
    }
    assert!(digits.len() & 1 == 0, "odd number of hex digits");
    digits.chunks(2).map(|p| (p[0] << 4) | p[1]).collect()
}

/// Encodes bytes as a lowercase hex string without separators.
#[must_use]
pub fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let b = hex("00 11 22\n# comment\nAA_bb");
        assert_eq!(b, [0, 0x11, 0x22, 0xaa, 0xbb]);
        assert_eq!(to_hex(&b), "001122aabb");
        assert!(hex("").is_empty());
    }
}
