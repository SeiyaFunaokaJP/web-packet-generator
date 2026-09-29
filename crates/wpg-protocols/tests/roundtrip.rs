//! Property test: build → parse → build is the identity on bytes (and parse is
//! stable) for random field values of the reference protocols.

use std::net::Ipv4Addr;

use proptest::prelude::*;
use wpg_core::{parse, DiagLevel, FieldValue, Layer, Packet};
use wpg_protocols::catalog;

fn ethernet() -> impl Strategy<Value = Layer> {
    (any::<[u8; 6]>(), any::<[u8; 6]>()).prop_map(|(d, s)| {
        Layer::new("ethernet2")
            .with("dst", FieldValue::Mac(d))
            .with("src", FieldValue::Mac(s))
    })
}

fn vlan() -> impl Strategy<Value = Layer> {
    (0u8..8, 0u8..2, 0u16..4095).prop_map(|(pcp, dei, vid)| {
        Layer::new("dot1q")
            .with("pcp", pcp)
            .with("dei", dei)
            .with("vid", vid)
    })
}

fn ipv4() -> impl Strategy<Value = Layer> {
    (
        (0u8..64, 0u8..4, any::<u16>(), 0u8..8, 0u16..8192, 1u8..=255),
        (any::<[u8; 4]>(), any::<[u8; 4]>()),
        proptest::collection::vec(any::<u8>(), 0..=40),
    )
        .prop_map(|((dscp, ecn, id, flags, frag, ttl), (src, dst), options)| {
            Layer::new("ipv4")
                .with("dscp", dscp)
                .with("ecn", ecn)
                .with("identification", id)
                .with("flags", flags)
                .with("frag_offset", frag)
                .with("ttl", ttl)
                .with("src", Ipv4Addr::from(src))
                .with("dst", Ipv4Addr::from(dst))
                .with("options", options)
        })
}

fn udp() -> impl Strategy<Value = Layer> {
    (any::<u16>(), any::<u16>())
        .prop_map(|(s, d)| Layer::new("udp").with("src_port", s).with("dst_port", d))
}

fn tcp() -> impl Strategy<Value = Layer> {
    (
        (any::<u16>(), any::<u16>(), any::<u32>(), any::<u32>()),
        (0u8..16, any::<u8>(), any::<u16>(), any::<u16>()),
        proptest::collection::vec(any::<u8>(), 0..=40),
    )
        .prop_map(|((sp, dp, seq, ack), (res, flags, win, urg), options)| {
            Layer::new("tcp")
                .with("src_port", sp)
                .with("dst_port", dp)
                .with("seq", seq)
                .with("ack", ack)
                .with("reserved", res)
                .with("flags", flags)
                .with("window", win)
                .with("urgent_ptr", urg)
                .with("options", options)
        })
}

fn packet() -> impl Strategy<Value = Packet> {
    (
        ethernet(),
        proptest::collection::vec(vlan(), 0..3),
        prop_oneof![udp(), tcp()],
        proptest::collection::vec(any::<u8>(), 0..64),
        ipv4(),
    )
        .prop_map(|(eth, vlans, l4, payload, ip)| {
            let mut layers = vec![eth];
            layers.extend(vlans);
            layers.push(ip);
            layers.push(l4);
            if !payload.is_empty() {
                layers.push(Layer::new("raw").with("data", payload));
            }
            Packet { layers }
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn build_parse_build(p in packet()) {
        let built = p.build(catalog()).unwrap();
        prop_assert!(
            !built.diagnostics.iter().any(|d| d.level == DiagLevel::Error),
            "{:?}", built.diagnostics
        );
        let parsed = parse(&built.bytes, "ethernet2", catalog()).unwrap();
        prop_assert!(
            !parsed.diagnostics.iter().any(|d| d.level >= DiagLevel::Warning),
            "{:?}", parsed.diagnostics
        );
        let protos: Vec<_> = parsed.packet.layers.iter().map(|l| l.protocol.clone()).collect();
        let want: Vec<_> = p.layers.iter().map(|l| l.protocol.clone()).collect();
        prop_assert_eq!(protos, want);
        let rebuilt = parsed.packet.build(catalog()).unwrap();
        prop_assert_eq!(&rebuilt.bytes, &built.bytes);
        prop_assert_eq!(&rebuilt.layout, &parsed.layout);
        // Parsing is stable.
        let again = parse(&rebuilt.bytes, "ethernet2", catalog()).unwrap();
        prop_assert_eq!(again.packet, parsed.packet);
    }
}
