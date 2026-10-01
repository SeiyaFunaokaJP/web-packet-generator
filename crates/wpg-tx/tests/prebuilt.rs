//! `Prebuilt::patch` (in-place on-the-fly rendering) must produce exactly the bytes
//! of a full `Packet::build` with the same overrides, for the sample templates and
//! random override sets (fixed-size fields patched in place, everything else through
//! the rebuild fallback, invalid values rejected by both).

// Test-only PRNG plumbing: truncating casts are intended.
#![allow(clippy::cast_possible_truncation)]

use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;

use proptest::prelude::*;
use wpg_core::stream::{Expansion, FieldRef};
use wpg_core::{FieldType, FieldValue, Length, Packet, Prebuilt};
use wpg_protocols::catalog;
use wpg_template::{compile, Compiled, Template, VarOverrides};
use wpg_tx::source::{FrameSource, OnTheFly};

fn templates() -> Vec<(String, Compiled)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../templates/examples");
    let mut out = Vec::new();
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with(".wpg.yaml"))
        .collect();
    paths.sort();
    for p in paths {
        let t = Template::from_path(&p).unwrap();
        let c = compile(&t, &VarOverrides::new(), catalog()).unwrap();
        out.push((p.file_name().unwrap().to_string_lossy().into_owned(), c));
    }
    assert!(out.len() >= 5, "sample templates missing");
    out
}

/// Tiny deterministic PRNG (splitmix64) driven by the proptest seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| self.next() as u8).collect()
    }
}

/// A random value for a field type; mostly valid, sometimes out of range or of the
/// wrong length so both paths must agree on the error.
fn value_for(ty: &FieldType, rng: &mut Rng) -> Option<FieldValue> {
    let wild = rng.below(10) == 0;
    Some(match ty {
        FieldType::UInt { bits } | FieldType::Enum { bits, .. } | FieldType::Flags { bits, .. } => {
            let b = u32::from(*bits);
            let v = rng.next();
            if wild || b >= 64 {
                FieldValue::UInt(v)
            } else {
                FieldValue::UInt(v & ((1u64 << b) - 1))
            }
        }
        FieldType::Int { bits } => {
            let b = u32::from(*bits);
            let v = rng.next() as i64;
            if wild || b >= 64 {
                FieldValue::Int(v)
            } else {
                FieldValue::Int(v >> (64 - b))
            }
        }
        FieldType::Mac => FieldValue::Mac(rng.bytes(6).try_into().unwrap()),
        FieldType::Ipv4 => FieldValue::Ipv4(Ipv4Addr::from(rng.next() as u32)),
        FieldType::Ipv6 => {
            let hi = u128::from(rng.next()) << 64 | u128::from(rng.next());
            FieldValue::Ipv6(Ipv6Addr::from(hi))
        }
        FieldType::Bytes {
            len: Length::Fixed { octets },
        } => {
            let n = *octets as usize + usize::from(wild);
            FieldValue::Bytes(rng.bytes(n))
        }
        FieldType::Bytes { .. } | FieldType::Raw => {
            let n = rng.below(24) as usize;
            FieldValue::Bytes(rng.bytes(n))
        }
        _ => return None,
    })
}

/// Random overrides: 1..=4 fields of any layer (including Auto length / checksum /
/// next-protocol fields, pseudo-header addresses and generator parameters).
fn random_overrides(p: &Packet, rng: &mut Rng) -> Vec<(FieldRef, FieldValue)> {
    let n = 1 + rng.below(4);
    let mut out = Vec::new();
    for _ in 0..n {
        let li = rng.below(p.layers.len() as u64) as usize;
        let def = catalog()
            .iter()
            .find(|d| d.id == p.layers[li].protocol)
            .unwrap();
        if def.fields.is_empty() {
            continue;
        }
        let fd = &def.fields[rng.below(def.fields.len() as u64) as usize];
        if let Some(v) = value_for(&fd.ty, rng) {
            out.push((FieldRef::new(li.to_string(), fd.name.clone()), v));
        }
    }
    out
}

fn rebuild(p: &Packet, ov: &[(FieldRef, FieldValue)]) -> Result<Vec<u8>, wpg_core::CoreError> {
    let mut p = p.clone();
    for (r, v) in ov {
        let i = p.layer_index(&r.layer).unwrap();
        p.layers[i].fields.insert(r.field.clone(), v.clone());
    }
    p.build(catalog()).map(|b| b.bytes)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn patch_equals_full_rebuild(seed in any::<u64>(), with_first in any::<bool>()) {
        for (name, c) in templates() {
            let mut rng = Rng(seed);
            let mut base = c.packet.clone();
            if with_first {
                let exp = Expansion::new(&c.stream, None).unwrap();
                if !exp.is_empty() {
                    let ov = exp.nth(0).unwrap();
                    for (r, v) in ov {
                        let i = base.layer_index(&r.layer).unwrap();
                        base.layers[i].fields.insert(r.field, v);
                    }
                }
            }
            let mut pre = Prebuilt::new(&base, catalog()).unwrap();
            prop_assert_eq!(pre.patch(&[]).unwrap(), &base.build(catalog()).unwrap().bytes[..]);
            // Several patches on the same instance: each is relative to the base.
            for _ in 0..4 {
                let ov = random_overrides(&base, &mut rng);
                let want = rebuild(&base, &ov);
                let got = pre.patch(&ov).map(<[u8]>::to_vec);
                match (got, want) {
                    (Ok(g), Ok(w)) => prop_assert_eq!(g, w, "{} {:?}", name, ov),
                    (Err(_), Err(_)) => {}
                    (g, w) => prop_assert!(false, "{name} {ov:?}: patch {g:?} vs build {w:?}"),
                }
            }
        }
    }
}

/// Every sample stream rendered on the fly equals per-packet full builds, and the
/// generated fields of the samples take the in-place path.
#[test]
fn on_the_fly_samples_match_full_builds() {
    for (name, c) in templates() {
        let exp = Expansion::new(&c.stream, None).unwrap();
        let mut fly = OnTheFly::new(c.packet.clone(), exp.clone(), catalog()).unwrap();
        let n = exp.len().unwrap_or(50).min(300);
        let mut buf = Vec::new();
        for i in 0..n {
            assert!(fly.next_frame(&mut buf).is_some(), "{name} ended at {i}");
            let ov = exp.nth(i).unwrap();
            assert_eq!(buf, rebuild(&c.packet, &ov).unwrap(), "{name} packet {i}");
        }
        if !exp.targets().is_empty() {
            assert!(
                fly.is_fast(),
                "{name}: generated fields should be patched in place"
            );
        }
        assert_eq!(fly.fallbacks(), if fly.is_fast() { 0 } else { n }, "{name}");
    }
}
