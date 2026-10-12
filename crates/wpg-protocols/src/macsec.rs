//! `MACsec` (IEEE 802.1AE) frame structure (TODO §2.1 P2, M7).
//!
//! One [`macsec`] layer, reached through EtherType `0x88e5`, holds the `SecTAG`
//! (TCI / AN, short length, packet number and the optional SCI), the secure data
//! and the ICV. Only the structure is modelled: nothing is encrypted or
//! authenticated, the secure data and the ICV are opaque octets given as they
//! should appear on the wire.
//!
//! Auto field: the SC bit (set when the SCI is present). The short length (the
//! secure data length when it is below 48 octets) and the ICV are Manual. When
//! parsing, the last 16 octets of the frame are the ICV, so a short frame padded
//! up to the Ethernet minimum keeps its padding inside the secure data / ICV.

use wpg_core::{protocol, FieldDef, FieldType, FieldValue, ItemDef, Length, ProtocolDef};

/// Id of the `MACsec` layer ([`macsec`]).
pub const MACSEC: &str = "macsec";

/// EtherType of `MACsec` (IEEE 802.1AE).
pub const MACSEC_ETHERTYPE: u64 = 0x88e5;

/// Length of the ICV with the default cipher suites (GCM-AES-128 / 256).
pub const ICV_LEN: u32 = 16;

/// The optional Secure Channel Identifier: the system identifier (a MAC address)
/// and a 16-bit port identifier. Present when the SC bit is set (one element).
fn sci() -> FieldType {
    FieldType::List {
        item: ItemDef::Struct {
            fields: vec![
                FieldDef::new("system_identifier", FieldType::Mac)
                    .with_default(FieldValue::Mac([0x02, 0, 0, 0, 0, 0x01])),
                FieldDef::new("port_identifier", FieldType::UInt { bits: 16 })
                    .with_default(FieldValue::UInt(1)),
            ],
        },
        length: Length::Count { field: "sc".into() },
    }
}

/// `MACsec` frame (IEEE 802.1AE §9): the `SecTAG` (TCI: version, ES, SC, SCB, E, C;
/// association number, short length, packet number, optional SCI), the secure
/// data and the ICV.
///
/// Default: an encrypted and integrity-protected frame (E = C = 1) on association
/// number 0 with packet number 1, an explicit SCI 02:00:00:00:00:01 / port 1, 48
/// octets of secure data (short length 0) and an all-zero ICV.
#[must_use]
pub fn macsec() -> ProtocolDef {
    let data: Vec<u8> = (0..48).collect();
    protocol! {
        id: "macsec",
        name: "MACsec",
        layer: L2,
        fields: {
            v: uint(1) = 0, desc = "macsec-v";
            es: uint(1) = 0, desc = "macsec-es";
            sc: uint(1), auto = count(sci), desc = "macsec-sc";
            scb: uint(1) = 0, desc = "macsec-scb";
            e: uint(1) = 1, desc = "macsec-e";
            c: uint(1) = 1, desc = "macsec-c";
            an: uint(2) = 0, desc = "macsec-an";
            reserved: uint(2) = 0;
            sl: uint(6) = 0, desc = "macsec-sl";
            pn: uint(32) = 1, desc = "macsec-pn";
            sci: ty(sci()) = FieldValue::List(vec![FieldValue::Struct(Default::default())]), desc = "macsec-sci";
            secure_data: ty(FieldType::Bytes { len: Length::ToEndExcept { octets: ICV_LEN } }) = FieldValue::Bytes(data), desc = "macsec-secure-data";
            icv: bytes(ICV_LEN) = [0u8; ICV_LEN as usize], desc = "macsec-icv";
        },
        constraints: {
            warning: equals(v, 0);
            warning: not_equals(pn, 0);
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macsec_is_valid() {
        let d = macsec();
        d.validate().unwrap();
        assert_eq!(d.id, MACSEC);
    }
}
