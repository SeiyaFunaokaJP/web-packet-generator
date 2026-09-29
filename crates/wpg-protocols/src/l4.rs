//! Layer 4 protocol definitions.

use wpg_core::{protocol, ProtocolDef};

/// UDP (RFC 768): `length` and `checksum` (with IPv4/IPv6 pseudo header) are Auto.
/// A computed checksum of zero is sent as `0xffff`.
#[must_use]
pub fn udp() -> ProtocolDef {
    protocol! {
        id: "udp",
        name: "UDP",
        layer: L4,
        fields: {
            src_port: uint(16) = 49152;
            dst_port: uint(16) = 9;
            length: uint(16), auto = length(total);
            checksum: uint(16), auto = checksum(internet_pseudo, header_payload, nonzero);
        },
        constraints: {
            warning: min_length(total, 8);
        },
    }
}

/// TCP (RFC 9293) with options as raw octets (typed options come with the P0
/// protocol phase). `data_offset` (including options and automatic zero padding)
/// and `checksum` (with pseudo header) are Auto.
#[must_use]
pub fn tcp() -> ProtocolDef {
    protocol! {
        id: "tcp",
        name: "TCP",
        layer: L4,
        fields: {
            src_port: uint(16) = 49152;
            dst_port: uint(16) = 80;
            seq: uint(32) = 0;
            ack: uint(32) = 0;
            data_offset: uint(4), auto = length(header, unit = words32);
            reserved: uint(4) = 0;
            flags: flags(8) {
                7 => cwr,
                6 => ece,
                5 => urg,
                4 => ack,
                3 => psh,
                2 => rst,
                1 => syn,
                0 => fin,
            } = 0x02;
            window: uint(16) = 65535;
            checksum: uint(16), auto = checksum(internet_pseudo, header_payload);
            urgent_ptr: uint(16) = 0;
            options: bytes(from = data_offset, unit = 4, adjust = -20);
            padding: padding(4);
        },
        constraints: {
            error: in_range(data_offset, 5, 15);
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l4_defs_are_valid() {
        udp().validate().unwrap();
        tcp().validate().unwrap();
        assert_eq!(udp().fixed_header_bits(), Some(64));
    }
}
