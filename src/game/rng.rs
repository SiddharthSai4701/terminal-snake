//! Deterministic RNG.
//!
//! Hand-rolled rather than pulled from `rand`: `StdRng` is documented as
//! non-portable across versions and platforms, which would silently invalidate
//! every recorded daily-challenge seed on a dependency bump.

const PCG_MULT: u64 = 6364136223846793005;
const PCG_SEQ: u64 = 54;

pub struct Pcg32 {
    state: u64,
    inc: u64,
}

impl Pcg32 {
    pub fn new(seed: u64) -> Self {
        let mut r = Pcg32 {
            state: 0,
            inc: (PCG_SEQ << 1) | 1,
        };
        r.next_u32();
        r.state = r.state.wrapping_add(seed);
        r.next_u32();
        r
    }

    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(PCG_MULT).wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Unbiased bounded draw: rejects the incomplete final block of the u32
    /// range so every value below `bound` is equally likely.
    pub fn below(&mut self, bound: u32) -> u32 {
        assert!(bound > 0, "bound must be positive");
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let r = self.next_u32();
            if r >= threshold {
                return r % bound;
            }
        }
    }
}

pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcg32_matches_reference_vectors() {
        let mut r = Pcg32::new(42);
        assert_eq!(r.next_u32(), 0xa15c02b7);
        assert_eq!(r.next_u32(), 0x7b47f409);
        assert_eq!(r.next_u32(), 0xba1d3330);
    }

    #[test]
    fn fnv1a64_matches_reference_vectors() {
        assert_eq!(fnv1a64(b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a64(b"a"), 0xaf63dc4c8601ec8c);
    }

    #[test]
    fn below_is_in_range() {
        let mut r = Pcg32::new(7);
        for _ in 0..10_000 {
            assert!(r.below(504) < 504);
        }
        assert_eq!(Pcg32::new(1).below(1), 0);
    }

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Pcg32::new(99);
        let mut b = Pcg32::new(99);
        for _ in 0..16 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }
}
