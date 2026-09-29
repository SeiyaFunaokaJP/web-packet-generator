//! Layer 2 protocol definitions.

use wpg_core::{
    AutoCompute, EnumVariant, FieldDef, FieldType, FieldValue, ProtocolDef, ProtocolLayer,
};

/// Well-known EtherType values offered in the GUI.
const ETHERTYPES: &[(u64, &str)] = &[
    (0x0800, "ipv4"),
    (0x0806, "arp"),
    (0x8100, "vlan"),
    (0x86DD, "ipv6"),
    (0x8847, "mpls"),
    (0x8848, "mpls_multicast"),
    (0x8863, "pppoe_discovery"),
    (0x8864, "pppoe_session"),
    (0x888E, "eapol"),
    (0x88A8, "qinq"),
    (0x88CC, "lldp"),
    (0x88F7, "ptp"),
];

/// Ethernet II (DIX) header: destination MAC, source MAC, EtherType (14 octets).
///
/// The EtherType is `Auto` by default and follows the next layer in the stack.
#[must_use]
pub fn ethernet2() -> ProtocolDef {
    ProtocolDef {
        id: "ethernet2".into(),
        name: "Ethernet II".into(),
        layer: ProtocolLayer::L2,
        fields: vec![
            FieldDef::new("dst", FieldType::Mac).with_default(FieldValue::Mac([0xff; 6])),
            FieldDef::new("src", FieldType::Mac)
                .with_default(FieldValue::Mac([0x02, 0x00, 0x00, 0x00, 0x00, 0x01])),
            FieldDef::new(
                "ethertype",
                FieldType::Enum {
                    bits: 16,
                    variants: ETHERTYPES
                        .iter()
                        .map(|&(value, name)| EnumVariant {
                            value,
                            name: name.into(),
                            description: None,
                        })
                        .collect(),
                },
            )
            .with_default(FieldValue::UInt(0x0800))
            .with_auto(AutoCompute::NextProtocol),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ethernet2_is_14_octets() {
        let eth = ethernet2();
        eth.validate().unwrap();
        assert_eq!(eth.fixed_header_bits(), Some(14 * 8));
        let names: Vec<_> = eth.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["dst", "src", "ethertype"]);
        assert_eq!(
            eth.field("ethertype").and_then(|f| f.auto.clone()),
            Some(AutoCompute::NextProtocol)
        );
    }
}
