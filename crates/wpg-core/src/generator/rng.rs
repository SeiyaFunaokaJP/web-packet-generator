//! Tiny deterministic PRNG (`xoshiro256**` seeded through `SplitMix64`). No `rand` crate:
//! `wpg-core` must stay dependency-light and WASM-safe.
#![allow(clippy::cast_possible_truncation)]

/// `SplitMix64` step: also used as a cheap 64-bit mixer.
#[must_use]
pub(crate) fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// `xoshiro256**` generator.
#[derive(Debug, Clone)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    /// Seeds the generator; any seed (including 0) is fine.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        let mut sm = seed;
        Rng {
            s: [
                splitmix64(&mut sm),
                splitmix64(&mut sm),
                splitmix64(&mut sm),
                splitmix64(&mut sm),
            ],
        }
    }

    /// Counter-style constructor: the stream for element `index` of `seed`. Gives
    /// deterministic random access (`nth`) to "random" sequences.
    #[must_use]
    pub fn for_index(seed: u64, index: u64) -> Self {
        let mut sm = seed;
        let a = splitmix64(&mut sm);
        Rng::new(a ^ index.wrapping_mul(0xD6E8_FEB8_6659_FD93).rotate_left(17) ^ index)
    }

    /// Next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        let r = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        r
    }

    /// Next 128 random bits.
    pub fn next_u128(&mut self) -> u128 {
        (u128::from(self.next_u64()) << 64) | u128::from(self.next_u64())
    }

    /// Uniform value in `lo..=hi` (unbiased, rejection sampling).
    ///
    /// # Panics
    /// Never; if `lo > hi` the bounds are swapped.
    pub fn range_u128(&mut self, lo: u128, hi: u128) -> u128 {
        let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
        let span = hi - lo;
        if span == u128::MAX {
            return self.next_u128();
        }
        let n = span + 1;
        // Largest multiple of n that fits: reject the biased tail.
        let zone = u128::MAX - (u128::MAX % n + 1) % n;
        loop {
            let v = self.next_u128();
            if v <= zone {
                return lo + v % n;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_seed_sensitive() {
        let a: Vec<u64> = {
            let mut r = Rng::new(1);
            (0..4).map(|_| r.next_u64()).collect()
        };
        let b: Vec<u64> = {
            let mut r = Rng::new(1);
            (0..4).map(|_| r.next_u64()).collect()
        };
        let c: Vec<u64> = {
            let mut r = Rng::new(2);
            (0..4).map(|_| r.next_u64()).collect()
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(Rng::new(0).next_u64(), 0);
    }

    #[test]
    fn range_bounds() {
        let mut r = Rng::new(7);
        for _ in 0..1000 {
            let v = r.range_u128(10, 12);
            assert!((10..=12).contains(&v));
        }
        assert_eq!(r.range_u128(5, 5), 5);
        let _ = r.range_u128(0, u128::MAX);
    }

    #[test]
    fn for_index_is_stable() {
        assert_eq!(
            Rng::for_index(3, 10).next_u64(),
            Rng::for_index(3, 10).next_u64()
        );
        assert_ne!(
            Rng::for_index(3, 10).next_u64(),
            Rng::for_index(3, 11).next_u64()
        );
    }
}
