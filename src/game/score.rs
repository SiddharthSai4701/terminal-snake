pub const TICK_BASE_MS: f32 = 140.0;
pub const TICK_MIN_MS: f32 = 55.0;
pub const TICK_DECAY: f32 = 0.985;

/// Tick length as a function of normal food eaten. Golden food deliberately
/// does not feed this, so the curve stays a pure function of normal food and
/// daily runs remain comparable.
pub fn tick_ms(normal_food_eaten: u32) -> f32 {
    (TICK_BASE_MS * TICK_DECAY.powi(normal_food_eaten as i32)).clamp(TICK_MIN_MS, TICK_BASE_MS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_the_base_rate() {
        assert!((tick_ms(0) - 140.0).abs() < 1e-3);
    }

    #[test]
    fn is_monotonically_faster() {
        for n in 0..300 {
            assert!(tick_ms(n + 1) <= tick_ms(n));
        }
    }

    #[test]
    fn clamps_at_the_floor() {
        assert!((tick_ms(10_000) - 55.0).abs() < 1e-6);
        for n in 0..2000 {
            assert!(tick_ms(n) >= 55.0);
        }
    }
}
