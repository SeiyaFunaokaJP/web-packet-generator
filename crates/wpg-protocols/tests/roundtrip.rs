//! Property test: build → parse → build is the identity on bytes (and parse is
//! stable) for random field values of the reference protocols.

use std::net::Ipv4Addr;

use proptest::prelude::*;
use wpg_core::{parse, DiagLevel, FieldValue, Layer, Packet, TlvItem};
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

/// One typed TCP option (or an unknown kind with value octets) and its size.
fn tcp_option() -> impl Strategy<Value = (TlvItem, usize)> {
    prop_oneof![
        Just((TlvItem::named("nop"), 1)),
        any::<u16>().prop_map(|v| (TlvItem::named("mss").with("value", v), 4)),
        any::<u8>().prop_map(|v| (TlvItem::named("ws").with("shift", v), 3)),
        Just((TlvItem::named("sack_permitted"), 2)),
        proptest::collection::vec((any::<u32>(), any::<u32>()), 0..=4).prop_map(|b| {
            let n = 2 + 8 * b.len();
            let blocks = b
                .into_iter()
                .map(|(l, r)| {
                    FieldValue::Struct(
                        [
                            ("left".to_owned(), FieldValue::UInt(l.into())),
                            ("right".to_owned(), FieldValue::UInt(r.into())),
                        ]
                        .into(),
                    )
                })
                .collect::<Vec<_>>();
            (TlvItem::named("sack").with("blocks", blocks), n)
        }),
        (any::<u32>(), any::<u32>()).prop_map(|(a, b)| {
            (
                TlvItem::named("timestamps")
                    .with("tsval", a)
                    .with("tsecr", b),
                10,
            )
        }),
        (9u64..=253, proptest::collection::vec(any::<u8>(), 0..6))
            .prop_map(|(k, d)| (TlvItem::with_type(k, d.clone()), 2 + d.len())),
    ]
}

/// Typed options fitting the 40-octet option space.
fn tcp_options() -> impl Strategy<Value = Vec<TlvItem>> {
    proptest::collection::vec(tcp_option(), 0..8).prop_map(|opts| {
        let mut used = 0;
        opts.into_iter()
            .filter(|(_, n)| {
                used += n;
                used <= 40
            })
            .map(|(o, _)| o)
            .collect()
    })
}

fn ipv4() -> impl Strategy<Value = Layer> {
    (
        (0u8..64, 0u8..4, any::<u16>(), 0u8..8, 0u16..8192, 1u8..=255),
        (any::<[u8; 4]>(), any::<[u8; 4]>()),
        proptest::collection::vec(
            prop_oneof![
                Just(TlvItem::named("nop")),
                any::<u16>().prop_map(|v| TlvItem::named("router_alert").with("value", v)),
                (2u64..=255, proptest::collection::vec(any::<u8>(), 0..6))
                    .prop_filter("typed kinds", |(k, _)| *k != 148)
                    .prop_map(|(k, d)| TlvItem::with_type(k, d)),
            ],
            0..5,
        ),
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
        tcp_options(),
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
            Packet::from_layers(layers)
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

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Arbitrary (mostly malformed) option octets survive parse → build unchanged,
    /// and well-formed typed options parse back to the same items.
    #[test]
    fn tcp_option_octets_roundtrip(
        raw in proptest::collection::vec(any::<u8>(), 0..=40),
        typed in tcp_options(),
    ) {
        for options in [FieldValue::Bytes(raw), FieldValue::Tlv(typed.clone())] {
            let p = Packet::from_layers(vec![
                Layer::new("ipv4"),
                Layer::new("tcp").with("options", options),
            ]);
            let built = p.build(catalog()).unwrap();
            let parsed = parse(&built.bytes, "ipv4", catalog()).unwrap();
            prop_assert_eq!(&parsed.packet.build(catalog()).unwrap().bytes, &built.bytes);
        }
        let p = Packet::from_layers(vec![
            Layer::new("ipv4"),
            Layer::new("tcp").with("options", typed.clone()),
        ]);
        let built = p.build(catalog()).unwrap();
        let parsed = parse(&built.bytes, "ipv4", catalog()).unwrap();
        let FieldValue::Tlv(items) = &parsed.packet.layers[1].fields["options"] else {
            panic!("options not typed: {:?}", parsed.packet.layers[1]);
        };
        // Parsed items carry both type and name; trailing EOLs pad to 32 bits.
        let names: Vec<_> = items.iter().map(|i| i.name.clone()).collect();
        let mut want: Vec<_> = typed.iter().map(|i| i.name.clone()).collect();
        while want.len() < names.len() {
            want.push(Some("eol".to_owned()));
        }
        prop_assert_eq!(names, want);
        for (got, sent) in items.iter().zip(&typed) {
            prop_assert_eq!(&got.fields, &sent.fields);
            prop_assert_eq!(&got.data, &sent.data);
            prop_assert_eq!(got.length, None);
        }
    }
}
