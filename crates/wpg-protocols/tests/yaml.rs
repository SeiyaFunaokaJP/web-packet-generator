//! The macro front-end and the YAML loader produce the same IR.

use wpg_core::ProtocolDef;
use wpg_protocols::{builtin, l2};

const ETHERNET2_YAML: &str = r"
id: ethernet2
name: Ethernet II
layer: l2
fields:
  - name: dst
    type: { kind: mac }
    default: { mac: [255, 255, 255, 255, 255, 255] }
  - name: src
    type: { kind: mac }
    default: { mac: [2, 0, 0, 0, 0, 1] }
  - name: ethertype
    type:
      kind: enum
      bits: 16
      variants:
        - { value: 0x0800, name: ipv4 }
        - { value: 0x0806, name: arp }
        - { value: 0x8100, name: vlan }
        - { value: 0x8809, name: slow_protocols }
        - { value: 0x86dd, name: ipv6 }
        - { value: 0x8847, name: mpls }
        - { value: 0x8848, name: mpls_multicast }
        - { value: 0x8863, name: pppoe_discovery }
        - { value: 0x8864, name: pppoe_session }
        - { value: 0x888e, name: eapol }
        - { value: 0x88a8, name: qinq }
        - { value: 0x88b5, name: local_experimental }
        - { value: 0x88cc, name: lldp }
        - { value: 0x88f7, name: ptp }
    default: { uint: 0x88b5 }
    auto: { kind: next_protocol }
next:
  field: ethertype
  table:
    - { value: 0x0800, protocol: ipv4 }
    - { value: 0x86dd, protocol: ipv6 }
    - { value: 0x0806, protocol: arp }
    - { value: 0x8100, protocol: dot1q }
    - { value: 0x88a8, protocol: dot1ad }
    - { value: 0x88cc, protocol: lldp }
    - { value: 0x8809, protocol: lacp }
    - { value: 0x888e, protocol: eapol }
    - { value: 0x8863, protocol: pppoe_discovery }
    - { value: 0x8864, protocol: pppoe_session }
    - { value: 0x8847, protocol: mpls }
    - { value: 0x8848, protocol: mpls }
    - { value: 0, max: 0x5ff, protocol: dot3, reparse: true }
min_frame_len: 60
";

/// A TLV field in YAML: the IPv4 options of `wpg_protocols::l3::ipv4_options`.
const IPV4_OPTIONS_YAML: &str = r"
kind: tlv
type_bits: 8
len_bits: 8
len_includes_header: true
variants:
  - { type_value: 0, name: eol, type_only: true }
  - { type_value: 1, name: nop, type_only: true }
  - type_value: 148
    name: router_alert
    fields:
      - { name: value, type: { kind: uint, bits: 16 } }
length: { kind: from_field, field: ihl, unit: 4, adjust: -20 }
";

#[test]
fn tlv_field_type_from_yaml() {
    let de = serde_yaml::Deserializer::from_str(IPV4_OPTIONS_YAML);
    let ty: wpg_core::FieldType =
        serde_yaml::with::singleton_map_recursive::deserialize(de).unwrap();
    assert_eq!(ty, wpg_protocols::l3::ipv4_options());
}

#[test]
fn ethernet2_macro_equals_yaml() {
    let from_yaml = ProtocolDef::from_yaml(ETHERNET2_YAML).unwrap();
    assert_eq!(from_yaml, l2::ethernet2());
}

#[test]
fn every_builtin_roundtrips_through_yaml() {
    for def in builtin() {
        let yaml = def.to_yaml().unwrap();
        let back =
            ProtocolDef::from_yaml(&yaml).unwrap_or_else(|e| panic!("{}: {e}\n{yaml}", def.id));
        assert_eq!(back, def, "{yaml}");
    }
}

#[test]
fn invalid_yaml_is_rejected() {
    assert!(ProtocolDef::from_yaml("id: [").is_err());
    let bad_width = ETHERNET2_YAML.replace("bits: 16", "bits: 65");
    assert!(ProtocolDef::from_yaml(&bad_width).is_err());
}
