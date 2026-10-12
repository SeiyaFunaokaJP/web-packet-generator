//! BFD control packets (RFC 5880) and Micro-BFD (RFC 7130) (TODO §2.1 P2, M7).
//!
//! One [`bfd`] layer holds the mandatory section and the optional authentication
//! section. It is reached through UDP 3784 (single hop, RFC 5881), 4784 (multihop,
//! RFC 5883) and 6784 (Micro-BFD on each LAG member link, RFC 7130); the echo
//! port 3785 carries packets only the sender understands and is not decoded.
//!
//! Auto fields: `length` (the whole control packet with the authentication
//! section) and the `a` bit (set when an authentication section is present).
//! Authentication digests / hashes (keyed MD5, keyed SHA1) are Manual.

use wpg_core::{
    protocol, FieldDef, FieldType, FieldValue, Length, LengthUnit, ProtocolDef, TlvDef, TlvVariant,
};

/// Id of the BFD control packet ([`bfd`]).
pub const BFD: &str = "bfd";

/// UDP port of single-hop BFD control packets (RFC 5881).
pub const BFD_SINGLE_HOP_PORT: u64 = 3784;

/// UDP port of multihop BFD control packets (RFC 5883).
pub const BFD_MULTIHOP_PORT: u64 = 4784;

/// UDP port of Micro-BFD control packets on LAG member links (RFC 7130).
pub const MICRO_BFD_PORT: u64 = 6784;

fn u(name: &str, bits: u8) -> FieldDef {
    FieldDef::new(name, FieldType::UInt { bits })
}

fn uint(name: &str, bits: u8, default: u64) -> FieldDef {
    u(name, bits).with_default(FieldValue::UInt(default))
}

fn octets(name: &str, n: u32) -> FieldDef {
    FieldDef::new(
        name,
        FieldType::Bytes {
            len: Length::Fixed { octets: n },
        },
    )
}

/// Keyed MD5 / SHA1 bodies (RFC 5880 §4.3-4.4): key ID, reserved, sequence
/// number and a digest of `digest` octets.
fn keyed(code: u64, name: &str, digest: u32) -> TlvVariant {
    TlvVariant::new(
        code,
        name,
        vec![
            uint("key_id", 8, 1),
            uint("reserved", 8, 0),
            u("sequence_number", 32),
            octets(if digest == 16 { "digest" } else { "hash" }, digest),
        ],
    )
}

/// Authentication section (RFC 5880 §4.2-4.4): 8-bit type, 8-bit length
/// including the type and length octets. Simple password (1), keyed MD5 (2),
/// meticulous keyed MD5 (3), keyed SHA1 (4) and meticulous keyed SHA1 (5). It
/// ends at the packet length (`length`).
#[must_use]
pub fn authentication() -> FieldType {
    FieldType::Tlv(TlvDef {
        type_bits: 8,
        len_bits: 8,
        len_includes_header: true,
        len_unit: LengthUnit::Bytes,
        variants: vec![
            TlvVariant::new(
                1,
                "simple_password",
                vec![
                    uint("key_id", 8, 1),
                    FieldDef::new("password", FieldType::Bytes { len: Length::ToEnd }),
                ],
            ),
            keyed(2, "keyed_md5", 16),
            keyed(3, "meticulous_keyed_md5", 16),
            keyed(4, "keyed_sha1", 20),
            keyed(5, "meticulous_keyed_sha1", 20),
        ],
        unknown_as_raw: true,
        length: Length::FromField {
            field: "length".into(),
            unit: 1,
            adjust: -24,
            div: 1,
        },
        fill: None,
        head_octets: 0,
        align: 0,
        unpadded_last: false,
        long_length: None,
        len_first: false,
    })
}

/// BFD control packet (RFC 5880 §4.1): version, diagnostic, state, the P / F / C /
/// A / D / M bits, detect multiplier, length (Auto), discriminators, the three
/// intervals in microseconds and the optional authentication section
/// ([`authentication`]).
///
/// Default: version 1, state Down, detect multiplier 3, my discriminator 1, your
/// discriminator 0 (not yet known) and 1 s intervals (RFC 5880 §6.8.3: at least
/// 1 s while the session is not Up).
#[must_use]
pub fn bfd() -> ProtocolDef {
    protocol! {
        id: "bfd",
        name: "BFD",
        layer: L7,
        fields: {
            version: uint(3) = 1, desc = "bfd-version";
            diag: enum(5) {
                0 => no_diagnostic,
                1 => control_detection_time_expired,
                2 => echo_function_failed,
                3 => neighbor_signaled_session_down,
                4 => forwarding_plane_reset,
                5 => path_down,
                6 => concatenated_path_down,
                7 => administratively_down,
                8 => reverse_concatenated_path_down,
                9 => mis_connectivity_defect,
            } = 0, desc = "bfd-diag";
            state: enum(2) {
                0 => admin_down,
                1 => down,
                2 => init,
                3 => up,
            } = 1, desc = "bfd-state";
            p: uint(1) = 0, desc = "bfd-p";
            f: uint(1) = 0, desc = "bfd-f";
            c: uint(1) = 0, desc = "bfd-c";
            a: uint(1), auto = count(authentication), desc = "bfd-a";
            d: uint(1) = 0, desc = "bfd-d";
            m: uint(1) = 0, desc = "bfd-m";
            detect_mult: uint(8) = 3, desc = "bfd-detect-mult";
            length: uint(8), auto = length(header), desc = "bfd-length";
            my_discriminator: uint(32) = 1, desc = "bfd-my-discriminator";
            your_discriminator: uint(32) = 0, desc = "bfd-your-discriminator";
            desired_min_tx_interval: uint(32) = 1_000_000, desc = "bfd-desired-min-tx";
            required_min_rx_interval: uint(32) = 1_000_000, desc = "bfd-required-min-rx";
            required_min_echo_rx_interval: uint(32) = 0, desc = "bfd-required-min-echo-rx";
            authentication: ty(authentication()), desc = "bfd-authentication";
        },
        constraints: {
            warning: equals(version, 1);
            warning: not_equals(detect_mult, 0);
            warning: not_equals(my_discriminator, 0);
            error: min_length(header, 24);
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bfd_is_valid() {
        let d = bfd();
        d.validate().unwrap();
        assert_eq!(d.id, BFD);
    }
}
