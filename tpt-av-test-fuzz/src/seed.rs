//! Deterministic seed management.
//!
//! A single 64-bit seed drives every fuzzing and proptest run in the TPT AV
//! Stack. On failure, the seed of the failed case is printed and can be
//! replayed verbatim to reproduce the bug locally and in CI.

use proptest::test_runner::{Config, RngAlgorithm, RngSeed};

/// The seed used for all CI runs. Change it when the generated corpora
/// stabilize and old cases are no longer relevant.
pub const FIXED_SEED: u64 = 0x1A2B_3C4D_5E6F_7081;

/// A deterministic, replayable seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Seed(pub u64);

impl Seed {
    /// Creates a seed from a raw 64-bit value.
    pub const fn new(value: u64) -> Self {
        Seed(value)
    }

    /// Derives a seed from arbitrary bytes (e.g. a FATE-suite corpus file)
    /// using FNV-1a, so identical inputs always map to identical seeds.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
        Seed(hash)
    }

    /// Produces the next seed in a splitmix64 stream (the RNG is
    /// deterministic and seedable, mirroring proptest's own strategy).
    pub fn next(self) -> Seed {
        let mut z = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        Seed(z ^ (z >> 31))
    }

    /// Converts into proptest's own seed type so it can be installed into a
    /// [`Config`].
    pub fn proptest(self) -> RngSeed {
        RngSeed::Fixed(self.0)
    }
}

impl Default for Seed {
    fn default() -> Self {
        Seed::new(FIXED_SEED)
    }
}

/// A proptest [`Config`] pinned to `seed`, so a failing case is reproducible
/// across machines and CI runs.
pub fn determinism_config(seed: Seed) -> Config {
    Config {
        rng_seed: seed.proptest(),
        rng_algorithm: RngAlgorithm::ChaCha,
        max_shrink_iters: 4096,
        ..Config::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_bytes_is_deterministic() {
        let a = Seed::from_bytes(b"same input");
        let b = Seed::from_bytes(b"same input");
        assert_eq!(a, b);
        assert_ne!(a, Seed::from_bytes(b"different input"));
    }

    #[test]
    fn next_advances_without_collision() {
        let mut seed = Seed::default();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..10_000 {
            assert!(seen.insert(seed), "seed stream collided");
            seed = seed.next();
        }
    }
}
