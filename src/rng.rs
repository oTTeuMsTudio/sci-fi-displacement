use rand::Rng as _;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// Deterministic RNG so a seed always rebuilds the same map.
pub struct Rng(ChaCha8Rng);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(ChaCha8Rng::seed_from_u64(seed))
    }

    pub fn f32(&mut self) -> f32 {
        self.0.gen()
    }

    pub fn bool(&mut self) -> bool {
        self.0.gen()
    }

    /// Inclusive on both ends. Returns `lo` when `lo >= hi`.
    pub fn u32(&mut self, lo: u32, hi: u32) -> u32 {
        if lo >= hi {
            return lo;
        }
        self.0.gen_range(lo..=hi)
    }

    pub fn i32(&mut self, lo: i32, hi: i32) -> i32 {
        if lo >= hi {
            return lo;
        }
        self.0.gen_range(lo..=hi)
    }

    pub fn u64(&mut self, lo: u64, hi: u64) -> u64 {
        if lo >= hi {
            return lo;
        }
        self.0.gen_range(lo..=hi)
    }

    pub fn chance(&mut self, probability: f32) -> bool {
        self.f32() < probability
    }

    pub fn index(&mut self, len: usize) -> usize {
        if len <= 1 {
            0
        } else {
            self.0.gen_range(0..len)
        }
    }

    /// Pick an index using non-negative weights. Empty input returns 0.
    pub fn weighted(&mut self, weights: &[u32]) -> usize {
        let total: u32 = weights.iter().sum();
        if total == 0 || weights.is_empty() {
            return 0;
        }
        let mut ticket = self.u32(0, total - 1);
        for (index, weight) in weights.iter().enumerate() {
            if ticket < *weight {
                return index;
            }
            ticket -= *weight;
        }
        weights.len() - 1
    }
}
