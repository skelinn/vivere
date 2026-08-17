//! vivere owns its RNG. Determinism is a core promise, so the generator's
//! exact behavior must never drift with an external crate's implementation
//! details, and its full state must serialize into snapshots.

use serde::{Deserialize, Serialize};

/// xoshiro256++ seeded via splitmix64.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    pub fn from_seed(seed: u64) -> Self {
        let mut x = seed;
        let mut next = || {
            x = x.wrapping_add(0x9E3779B97F4A7C15);
            let mut z = x;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
            z ^ (z >> 31)
        };
        Self {
            s: [next(), next(), next(), next()],
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[0]
            .wrapping_add(self.s[3])
            .rotate_left(23)
            .wrapping_add(self.s[0]);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Uniform in [0, 1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 * (1.0 / (1u64 << 24) as f32)
    }

    /// Uniform in [0, 1).
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform in [lo, hi).
    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }

    /// Uniform integer in [0, n). `n` must be > 0.
    /// Modulo bias is negligible at the ranges the simulation uses.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }

    /// Log-uniform in [lo, hi]: uniform in log space, so scale-like genes
    /// spanning a 10–30× range don't over-sample their high end.
    pub fn log_range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        libm::expf(self.range_f32(libm::logf(lo), libm::logf(hi))).clamp(lo, hi)
    }

    /// Standard normal via Box–Muller. Generates a fresh pair every call and
    /// discards the second value: a cached "spare" would be hidden generator
    /// state, which would break snapshot exactness.
    pub fn gaussian(&mut self) -> f32 {
        let u1 = (1.0 - self.f64()) as f32; // in (0, 1], so log never sees 0
        let u2 = self.f32();
        let r = libm::sqrtf(-2.0 * libm::logf(u1));
        r * libm::cosf(core::f32::consts::TAU * u2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_stream() {
        let mut a = Rng::from_seed(42);
        let mut b = Rng::from_seed(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn gaussian_is_roughly_standard() {
        let mut rng = Rng::from_seed(7);
        let n = 20_000;
        let (mut sum, mut sq) = (0.0f64, 0.0f64);
        for _ in 0..n {
            let g = rng.gaussian() as f64;
            sum += g;
            sq += g * g;
        }
        let mean = sum / n as f64;
        let var = sq / n as f64 - mean * mean;
        assert!(mean.abs() < 0.05, "mean {mean}");
        assert!((var - 1.0).abs() < 0.1, "var {var}");
    }

    #[test]
    fn uniform_bounds() {
        let mut rng = Rng::from_seed(3);
        for _ in 0..10_000 {
            let x = rng.f32();
            assert!((0.0..1.0).contains(&x));
            let y = rng.range_f32(-2.0, 3.0);
            assert!((-2.0..3.0).contains(&y));
            assert!(rng.below(7) < 7);
        }
    }
}
