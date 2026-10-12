//! `nsap` fields: OSI encoding (length indicator + address octets) when building
//! and parsing (M7 "NSAP 型").

use wpg_core::{parse, protocol, FieldValue, Layer, Packet, ProtocolDef};

fn esh_like() -> ProtocolDef {
    let d = protocol! {
        id: "esh",
        name: "ESH-like",
        layer: L3,
        fields: {
            kind: uint(8) = 2;
            net: nsap = [0x49, 0x00, 0x01, 0x19, 0x21, 0x68, 0x00, 0x10, 0x01, 0x00];
            rest: bytes(to_end) = [];
        },
    };
    d.validate().unwrap();
    d
}

#[test]
fn nsap_is_length_prefixed() {
    let defs = vec![esh_like()];
    let built = Packet::from_layers(vec![Layer::new("esh").with("rest", vec![0xaau8])])
        .build(&defs)
        .unwrap();
    assert_eq!(
        built.bytes,
        [2, 10, 0x49, 0, 1, 0x19, 0x21, 0x68, 0, 0x10, 1, 0, 0xaa]
    );
    let net = &built.layout[0].fields[1];
    assert_eq!((net.bit_offset, net.bit_len), (8, 11 * 8));

    let p = parse(&built.bytes, "esh", &defs).unwrap();
    assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
    assert_eq!(
        p.packet.layers[0].fields["net"],
        FieldValue::Nsap(vec![0x49, 0, 1, 0x19, 0x21, 0x68, 0, 0x10, 1, 0])
    );
    assert_eq!(
        p.packet.layers[0].fields["rest"],
        FieldValue::Bytes(vec![0xaa])
    );
    assert_eq!(p.packet.build(&defs).unwrap().bytes, built.bytes);
}

#[test]
fn nsap_manual_values_and_truncation() {
    let defs = vec![esh_like()];
    let short = FieldValue::Nsap(vec![0x47, 0x00, 0x05]);
    let built = Packet::from_layers(vec![Layer::new("esh").with("net", short.clone())])
        .build(&defs)
        .unwrap();
    assert_eq!(built.bytes, [2, 3, 0x47, 0, 5]);
    let p = parse(&built.bytes, "esh", &defs).unwrap();
    assert_eq!(p.packet.layers[0].fields["net"], short);

    // An empty address is just the zero length indicator.
    let built = Packet::from_layers(vec![Layer::new("esh").with("net", FieldValue::Nsap(vec![]))])
        .build(&defs)
        .unwrap();
    assert_eq!(built.bytes, [2, 0]);

    // More than 255 octets cannot be encoded.
    assert!(Packet::from_layers(vec![
        Layer::new("esh").with("net", FieldValue::Nsap(vec![0; 256]))
    ])
    .build(&defs)
    .is_err());

    // A length indicator running past the data does not decode as `esh`.
    let p = parse(&[2, 10, 0x49, 0], "esh", &defs).unwrap();
    assert!(p.has_errors());
}
