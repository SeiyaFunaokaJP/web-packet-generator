use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};

use proptest::prelude::*;

use super::*;
use crate::notation;

fn opts() -> CompileOptions<'static> {
    CompileOptions::default()
}

fn u(bits: u8) -> FieldType {
    FieldType::UInt { bits }
}

fn vals(g: &Generator, ty: &FieldType) -> Vec<FieldValue> {
    g.iter(ty, &opts()).unwrap().collect()
}

fn range(start: &str, end: &str, step: u64) -> Generator {
    Generator::Range {
        start: start.into(),
        end: end.into(),
        step,
    }
}

fn uints(v: &[FieldValue]) -> Vec<u64> {
    v.iter()
        .map(|x| match x {
            FieldValue::UInt(n) => *n,
            other => panic!("{other:?}"),
        })
        .collect()
}

#[test]
fn serde_defaults() {
    let g: Generator =
        serde_json::from_str(r#"{"kind":"range","start":"10.0.0.1","end":"10.0.0.9"}"#).unwrap();
    assert_eq!(g, range("10.0.0.1", "10.0.0.9", 1));
    let c: Generator = serde_json::from_str(r#"{"kind":"counter"}"#).unwrap();
    assert_eq!(
        c,
        Generator::Counter {
            start: 0,
            step: 1,
            wrap: None
        }
    );
    let r: Generator = serde_json::from_str(r#"{"kind":"random_mac","oui":"00:11:22"}"#).unwrap();
    assert_eq!(
        r,
        Generator::RandomMac {
            oui: "00:11:22".into(),
            seed: None
        }
    );
    let r: Generator = serde_json::from_str(r#"{"kind":"random"}"#).unwrap();
    assert!(matches!(r, Generator::Random { min: None, .. }));
}

#[test]
fn untyped_cardinality() {
    let list = Generator::List {
        values: vec!["a".into(), "b".into()],
    };
    assert_eq!(list.cardinality(), Ok(Some(2)));
    let counter = Generator::Counter {
        start: 0,
        step: 2,
        wrap: Some(10),
    };
    assert_eq!(counter.cardinality(), Ok(Some(6)));
    assert_eq!(
        range("10.0.0.1", "10.0.0.10", 1).cardinality(),
        Ok(Some(10))
    );
    assert_eq!(
        Generator::Random {
            min: None,
            max: None,
            seed: None
        }
        .cardinality(),
        Ok(None)
    );
    assert!(Generator::File { path: "x".into() }.cardinality().is_err());
}

#[test]
fn integer_range_step_and_descending() {
    let ty = u(16);
    assert_eq!(uints(&vals(&range("1", "10", 3), &ty)), vec![1, 4, 7, 10]);
    assert_eq!(uints(&vals(&range("1", "9", 3), &ty)), vec![1, 4, 7]);
    assert_eq!(uints(&vals(&range("5", "1", 2), &ty)), vec![5, 3, 1]);
    assert_eq!(uints(&vals(&range("7", "7", 1), &ty)), vec![7]);
    assert_eq!(
        range("1", "1", 0).compile(&ty, &opts()).unwrap_err(),
        GeneratorError::ZeroStep
    );
    assert!(range("1", "70000", 1).compile(&ty, &opts()).is_err());
    let s = FieldType::Int { bits: 8 };
    assert_eq!(
        vals(&range("-2", "2", 2), &s),
        vec![FieldValue::Int(-2), FieldValue::Int(0), FieldValue::Int(2)]
    );
    assert_eq!(vals(&range("127", "126", 1), &s).len(), 2);
    let full = range("0", "0xffffffffffffffff", 1)
        .compile(&u(64), &opts())
        .unwrap();
    assert_eq!(full.cardinality_exact(), Some(1u128 << 64));
    assert_eq!(full.cardinality(), Some(u64::MAX));
}

#[test]
fn ipv4_range_and_cidr() {
    let g = range("192.168.1.250", "192.168.2.2", 1);
    let v = vals(&g, &FieldType::Ipv4);
    assert_eq!(v.len(), 9);
    assert_eq!(v[6], FieldValue::Ipv4(Ipv4Addr::new(192, 168, 2, 0)));
    let cidr = notation::parse_generator("10.0.0.0/30", &FieldType::Ipv4).unwrap();
    assert_eq!(
        vals(&cidr, &FieldType::Ipv4),
        vec![
            FieldValue::Ipv4(Ipv4Addr::new(10, 0, 0, 1)),
            FieldValue::Ipv4(Ipv4Addr::new(10, 0, 0, 2))
        ]
    );
    let all = notation::parse_generator_with(
        "10.0.0.0/24",
        &FieldType::Ipv4,
        notation::CidrOptions {
            include_network_broadcast: true,
        },
    )
    .unwrap();
    assert_eq!(
        all.cardinality_for(&FieldType::Ipv4, &opts()),
        Ok(Some(256))
    );
}

#[test]
fn ipv6_range_crosses_64_bit_boundary() {
    let g = range("2001:db8::ffff:ffff:ffff:fffe", "2001:db8:0:1::1", 1);
    let v = vals(&g, &FieldType::Ipv6);
    assert_eq!(v.len(), 4);
    assert_eq!(
        v[1],
        FieldValue::Ipv6("2001:db8::ffff:ffff:ffff:ffff".parse().unwrap())
    );
    assert_eq!(
        v[2],
        FieldValue::Ipv6("2001:db8:0:1::".parse::<Ipv6Addr>().unwrap())
    );
    // Whole IPv6 space saturates the u64 cardinality but stays exact in u128.
    let all = range("::", "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", 1)
        .compile(&FieldType::Ipv6, &opts())
        .unwrap();
    assert_eq!(all.cardinality(), Some(u64::MAX));
    assert_eq!(all.cardinality_exact(), Some(u128::MAX));
    assert_eq!(
        all.value_at_simple(5).unwrap(),
        FieldValue::Ipv6(Ipv6Addr::from(5u128))
    );
}

#[test]
fn mac_range() {
    let g = range("00:00:00:00:00:fe", "00:00:00:00:01:01", 1);
    let v = vals(&g, &FieldType::Mac);
    assert_eq!(v.len(), 4);
    assert_eq!(v[1], FieldValue::Mac([0, 0, 0, 0, 0, 0xff]));
    assert_eq!(v[2], FieldValue::Mac([0, 0, 0, 0, 1, 0]));
}

#[test]
fn nsap_system_id_increment() {
    let g = range("49.0001.1921.6800.1001.00", "49.0001.1921.6800.1004.00", 1);
    let v = vals(&g, &FieldType::Nsap);
    assert_eq!(v.len(), 4);
    // System id is the 6 octets before the NSEL: 1921.6800.1001 .. 1921.6800.1004.
    let FieldValue::Nsap(last) = v.last().unwrap() else {
        panic!()
    };
    assert_eq!(notation::format_nsap(last), "49.0001.1921.6800.1004.00");
    // Carry across octets inside the system id.
    let g = range("49.0001.0000.0000.00ff.00", "49.0001.0000.0000.0101.00", 1);
    let v = vals(&g, &FieldType::Nsap);
    let FieldValue::Nsap(mid) = &v[1] else {
        panic!()
    };
    assert_eq!(notation::format_nsap(mid), "49.0001.0000.0000.0100.00");
    // Prefix / NSEL must match.
    assert_eq!(
        range("49.0001.1921.6800.1001.00", "49.0002.1921.6800.1001.00", 1)
            .compile(&FieldType::Nsap, &opts())
            .unwrap_err(),
        GeneratorError::NsapMismatch
    );
}

#[test]
fn list_cycles_and_validates() {
    let g = Generator::List {
        values: vec!["1".into(), "2".into(), "3".into()],
    };
    let c = g.compile(&u(8), &opts()).unwrap();
    assert_eq!(c.cardinality(), Some(3));
    assert_eq!(c.value_at_simple(4).unwrap(), FieldValue::UInt(2));
    assert!(Generator::List { values: vec![] }
        .compile(&u(8), &opts())
        .is_err());
    assert!(Generator::List {
        values: vec!["300".into()]
    }
    .compile(&u(8), &opts())
    .is_err());
}

#[test]
fn random_is_seeded_bounded_and_random_access() {
    let g = Generator::Random {
        min: Some("10".into()),
        max: Some("20".into()),
        seed: Some(42),
    };
    let c = g.compile(&u(8), &opts()).unwrap();
    assert_eq!(c.cardinality(), None);
    let a: Vec<u64> = (0..200)
        .map(|i| uints(&[c.value_at_simple(i).unwrap()])[0])
        .collect();
    assert!(a.iter().all(|x| (10..=20).contains(x)));
    assert!(a.iter().collect::<std::collections::HashSet<_>>().len() > 5);
    let c2 = g.compile(&u(8), &opts()).unwrap();
    assert_eq!(
        c2.value_at_simple(77).unwrap(),
        c.value_at_simple(77).unwrap()
    );
    let other = Generator::Random {
        min: Some("10".into()),
        max: Some("20".into()),
        seed: Some(43),
    }
    .compile(&u(8), &opts())
    .unwrap();
    assert!((0..50).any(|i| other.value_at_simple(i).unwrap() != c.value_at_simple(i).unwrap()));
    // Signed bounds and IPv4 default range.
    let s = Generator::Random {
        min: Some("-3".into()),
        max: Some("3".into()),
        seed: None,
    }
    .compile(&FieldType::Int { bits: 8 }, &opts())
    .unwrap();
    for i in 0..100 {
        let FieldValue::Int(x) = s.value_at_simple(i).unwrap() else {
            panic!()
        };
        assert!((-3..=3).contains(&x));
    }
    assert!(Generator::Random {
        min: Some("5".into()),
        max: Some("1".into()),
        seed: None
    }
    .compile(&u(8), &opts())
    .is_err());
    let full = Generator::Random {
        min: None,
        max: None,
        seed: None,
    }
    .compile(&FieldType::Ipv6, &opts())
    .unwrap();
    assert!(matches!(
        full.value_at_simple(0).unwrap(),
        FieldValue::Ipv6(_)
    ));
}

#[test]
fn random_mac_keeps_oui() {
    let g = Generator::RandomMac {
        oui: "00:11:22".into(),
        seed: Some(1),
    };
    let c = g.compile(&FieldType::Mac, &opts()).unwrap();
    let mut distinct = std::collections::HashSet::new();
    for i in 0..100 {
        let FieldValue::Mac(m) = c.value_at_simple(i).unwrap() else {
            panic!()
        };
        assert_eq!(&m[..3], &[0, 0x11, 0x22]);
        distinct.insert(m);
    }
    assert!(distinct.len() > 90);
    assert!(g.compile(&FieldType::Ipv4, &opts()).is_err());
    let full = Generator::RandomMac {
        oui: "01:02:03:04:05:06".into(),
        seed: None,
    }
    .compile(&FieldType::Mac, &opts())
    .unwrap();
    assert_eq!(
        full.value_at_simple(3).unwrap(),
        FieldValue::Mac([1, 2, 3, 4, 5, 6])
    );
    assert!(Generator::RandomMac {
        oui: "zz".into(),
        seed: None
    }
    .compile(&FieldType::Mac, &opts())
    .is_err());
}

#[test]
fn counter_wrap_negative_and_masking() {
    let c = |start, step, wrap| Generator::Counter { start, step, wrap };
    assert_eq!(uints(&vals(&c(5, 1, Some(7)), &u(8))), vec![5, 6, 7]);
    // Wrapping counter cycles when driven past its cardinality.
    let cg = c(0, 3, Some(9)).compile(&u(8), &opts()).unwrap();
    assert_eq!(cg.cardinality(), Some(4));
    let cyc: Vec<u64> = (0..9)
        .map(|i| uints(&[cg.value_at_simple(i).unwrap()])[0])
        .collect();
    assert_eq!(cyc, vec![0, 3, 6, 9, 0, 3, 6, 9, 0]);
    // Negative step counts down to wrap.
    assert_eq!(uints(&vals(&c(10, -5, Some(0)), &u(8))), vec![10, 5, 0]);
    // Unwrapped counters are unbounded and wrap at the field width.
    let inf = c(254, 1, None).compile(&u(8), &opts()).unwrap();
    assert_eq!(inf.cardinality(), None);
    assert_eq!(inf.value_at_simple(2).unwrap(), FieldValue::UInt(0));
    let down = c(1, -1, None).compile(&u(8), &opts()).unwrap();
    assert_eq!(down.value_at_simple(2).unwrap(), FieldValue::UInt(255));
    // Zero step is a constant.
    assert_eq!(uints(&vals(&c(9, 0, Some(9)), &u(8))), vec![9]);
    // Counter over IPv4.
    let ip = c(0, 1, None).compile(&FieldType::Ipv4, &opts()).unwrap();
    assert_eq!(
        ip.value_at_simple(3).unwrap(),
        FieldValue::Ipv4(Ipv4Addr::new(0, 0, 0, 3))
    );
}

#[test]
fn file_lines_via_provider() {
    let mut files = HashMap::new();
    files.insert(
        "ips.txt".to_owned(),
        vec![
            "# comment".to_owned(),
            "10.0.0.1".to_owned(),
            String::new(),
            "  10.0.0.2 ".to_owned(),
        ],
    );
    let o = CompileOptions {
        files: Some(&files),
        ..CompileOptions::default()
    };
    let g = Generator::File {
        path: "ips.txt".into(),
    };
    let c = g.compile(&FieldType::Ipv4, &o).unwrap();
    assert_eq!(c.cardinality(), Some(2));
    assert_eq!(
        c.value_at_simple(1).unwrap(),
        FieldValue::Ipv4(Ipv4Addr::new(10, 0, 0, 2))
    );
    assert_eq!(
        g.compile(&FieldType::Ipv4, &opts()).unwrap_err(),
        GeneratorError::FileUnavailable("ips.txt".into())
    );
    let closure = |p: &str| (p == "ips.txt").then(|| vec!["1.1.1.1".to_owned()]);
    let o2 = CompileOptions {
        files: Some(&closure),
        ..CompileOptions::default()
    };
    assert!(g.compile(&FieldType::Ipv4, &o2).is_ok());
    files.insert("bad.txt".into(), vec!["nope".into()]);
    let o3 = CompileOptions {
        files: Some(&files),
        ..CompileOptions::default()
    };
    assert!(Generator::File {
        path: "bad.txt".into()
    }
    .compile(&FieldType::Ipv4, &o3)
    .is_err());
}

#[test]
fn expr_generator() {
    let g = Generator::Expr {
        expr: "($a + idx) * 2".into(),
    };
    let c = g.compile(&u(8), &opts()).unwrap();
    assert!(c.is_expr());
    assert_eq!(c.references(), vec!["a".to_owned()]);
    let lk = |n: &str| (n == "a").then_some(3);
    let v = c
        .value_at(
            0,
            &EvalCtx {
                packet_index: 4,
                fields: &lk,
            },
        )
        .unwrap();
    assert_eq!(v, FieldValue::UInt(14));
    // Result wraps into the field width.
    let w = Generator::Expr {
        expr: "300 + idx".into(),
    }
    .compile(&u(8), &opts())
    .unwrap();
    assert_eq!(w.value_at_simple(0).unwrap(), FieldValue::UInt(44));
    let neg = Generator::Expr {
        expr: "0 - 1".into(),
    }
    .compile(&FieldType::Int { bits: 8 }, &opts())
    .unwrap();
    assert_eq!(neg.value_at_simple(0).unwrap(), FieldValue::Int(-1));
    assert!(Generator::Expr { expr: "1 +".into() }
        .compile(&u(8), &opts())
        .is_err());
    assert!(Generator::Expr { expr: "1".into() }
        .compile(&FieldType::Nsap, &opts())
        .is_err());
}

#[test]
fn state_seek_and_size_hint() {
    let mut it = range("1", "100", 1).iter(&u(8), &opts()).unwrap();
    assert_eq!(it.size_hint(), (100, Some(100)));
    it.seek(98);
    assert_eq!(uints(&it.collect::<Vec<_>>()), vec![99, 100]);
    let inf = Generator::Counter {
        start: 0,
        step: 1,
        wrap: None,
    };
    assert_eq!(inf.iter(&u(8), &opts()).unwrap().take(3).count(), 3);
}

#[test]
fn core_error_conversion() {
    let e: CoreError = GeneratorError::ZeroStep.into();
    assert!(matches!(e, CoreError::InvalidValue { .. }));
}

#[test]
fn ipv4_million_values_quickly() {
    let c = range("10.0.0.0", "10.15.66.63", 1)
        .compile(&FieldType::Ipv4, &opts())
        .unwrap();
    assert_eq!(c.cardinality(), Some(1_000_000));
    let t = std::time::Instant::now();
    let n = c.into_state().count();
    assert_eq!(n, 1_000_000);
    assert!(
        t.elapsed().as_secs_f64() < 5.0,
        "too slow: {:?}",
        t.elapsed()
    );
}

proptest! {
    #[test]
    fn range_len_matches_cardinality(a in 0u64..2000, b in 0u64..2000, step in 1u64..50) {
        let ty = u(16);
        let g = range(&a.to_string(), &b.to_string(), step);
        let c = g.compile(&ty, &opts()).unwrap();
        let card = c.cardinality().unwrap();
        let v: Vec<FieldValue> = c.clone().into_state().collect();
        prop_assert_eq!(v.len() as u64, card);
        let want = a.abs_diff(b) / step + 1;
        prop_assert_eq!(card, want);
        // Every value stays within bounds and hits the expected stride.
        for (i, x) in uints(&v).into_iter().enumerate() {
            let off = i as u64 * step;
            let e = if b < a { a - off } else { a + off };
            prop_assert_eq!(x, e);
        }
    }

    #[test]
    fn ipv4_range_len_matches(a in any::<u32>(), b in any::<u32>(), step in 1u64..100_000) {
        let g = range(&Ipv4Addr::from(a).to_string(), &Ipv4Addr::from(b).to_string(), step);
        let c = g.compile(&FieldType::Ipv4, &opts()).unwrap();
        let card = c.cardinality().unwrap();
        prop_assert_eq!(card, u64::from(a.abs_diff(b)) / step + 1);
        // Spot check the last element instead of iterating billions.
        let last = c.value_at_simple(card - 1).unwrap();
        let FieldValue::Ipv4(l) = last else { panic!() };
        let l = u32::from(l);
        let (lo, hi) = (a.min(b), a.max(b));
        prop_assert!(l >= lo && l <= hi);
    }

    #[test]
    fn counter_wrap_len(start in 0u64..1000, span in 0u64..1000, step in 1i64..20) {
        let g = Generator::Counter { start, step, wrap: Some(start + span) };
        let n = g.iter(&u(16), &opts()).unwrap().count() as u64;
        prop_assert_eq!(n, span / step.unsigned_abs() + 1);
    }
}
