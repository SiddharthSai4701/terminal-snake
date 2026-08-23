//! Colour pipeline.
//!
//! Everything composites in *linear light*; theme colours are authored as sRGB
//! hex and decoded once. Adding glow to gamma-encoded values is what makes
//! terminal bloom look washed out instead of lit.

pub type Rgb = [f32; 3];

pub fn srgb_decode(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

pub fn srgb_encode(v: f32) -> f32 {
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// Where the highlight roll-off begins.
pub const KNEE: f32 = 0.8;
/// Input that maps to pure white. A Reinhard curve only approaches 1.0, which
/// would render the death flash grey.
pub const WHITE: f32 = 2.0;

/// Linear below `KNEE`, then a curve that meets it with matching slope and
/// lands exactly on 1.0 at `WHITE`.
pub fn tone_map(v: f32) -> f32 {
    if v <= KNEE {
        v.max(0.0)
    } else if v >= WHITE {
        1.0
    } else {
        let t = (v - KNEE) / (WHITE - KNEE);
        // Exponent chosen so the derivative is continuous at the knee.
        let n = (WHITE - KNEE) / (1.0 - KNEE);
        KNEE + (1.0 - KNEE) * (1.0 - (1.0 - t).powf(n))
    }
}

/// Quantize to 5 bits per channel. Indistinguishable on screen, but it lets
/// the diff renderer skip slowly decaying trail pixels instead of rewriting
/// every cell every frame.
pub fn to_u8(v: f32) -> u8 {
    let q = (v.clamp(0.0, 1.0) * 31.0 + 0.5) as u32;
    ((q * 255) / 31) as u8
}

pub fn rgb_hex(hex: u32) -> Rgb {
    [
        srgb_decode(((hex >> 16) & 0xff) as f32 / 255.0),
        srgb_decode(((hex >> 8) & 0xff) as f32 / 255.0),
        srgb_decode((hex & 0xff) as f32 / 255.0),
    ]
}

const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];

fn cube_index(v: u8) -> usize {
    let mut best = 0usize;
    let mut best_err = i32::MAX;
    for (i, &c) in CUBE.iter().enumerate() {
        let e = (c as i32 - v as i32).abs();
        if e < best_err {
            best_err = e;
            best = i;
        }
    }
    best
}

fn dist2(a: [u8; 3], b: [u8; 3]) -> i32 {
    (0..3)
        .map(|i| {
            let d = a[i] as i32 - b[i] as i32;
            d * d
        })
        .sum()
}

/// Nearest xterm-256 index: the 6x6x6 colour cube (16..=231) or the 24-step
/// grey ramp (232..=255), whichever is closer. Used on 256-colour-only
/// terminals such as macOS Terminal.app.
pub fn nearest_256(c: [u8; 3]) -> u8 {
    let ri = cube_index(c[0]);
    let gi = cube_index(c[1]);
    let bi = cube_index(c[2]);
    let cube_err = dist2(c, [CUBE[ri], CUBE[gi], CUBE[bi]]);

    let avg = (c[0] as i32 + c[1] as i32 + c[2] as i32) / 3;
    let grey_i = (((avg - 8) as f32 / 10.0).round() as i32).clamp(0, 23);
    let g = (8 + grey_i * 10) as u8;
    let grey_err = dist2(c, [g, g, g]);

    if grey_err < cube_err {
        (232 + grey_i) as u8
    } else {
        (16 + 36 * ri + 6 * gi + bi) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_round_trips() {
        for i in 0..=255u32 {
            let v = i as f32 / 255.0;
            assert!(
                (srgb_encode(srgb_decode(v)) - v).abs() < 1e-4,
                "failed at {i}"
            );
        }
    }

    #[test]
    fn tone_map_reaches_exactly_one_so_the_death_flash_is_white() {
        assert_eq!(tone_map(WHITE), 1.0);
        assert_eq!(tone_map(4.0), 1.0);
        assert!((tone_map(0.5) - 0.5).abs() < 1e-6, "linear below the knee");
        assert!((tone_map(KNEE) - KNEE).abs() < 1e-6, "continuous at the knee");
    }

    #[test]
    fn tone_map_is_monotonic_and_bounded() {
        let mut prev = -1.0f32;
        for i in 0..=400 {
            let v = tone_map(i as f32 / 100.0);
            assert!(v >= prev - 1e-6, "not monotonic at {i}");
            assert!((0.0..=1.0).contains(&v));
            prev = v;
        }
    }

    #[test]
    fn to_u8_snaps_to_five_bits() {
        let levels: std::collections::BTreeSet<u8> =
            (0..=255u32).map(|i| to_u8(i as f32 / 255.0)).collect();
        assert_eq!(levels.len(), 32);
        assert_eq!(to_u8(0.0), 0);
        assert_eq!(to_u8(1.0), 255);
    }

    #[test]
    fn nearest_256_maps_greys_and_primaries() {
        assert_eq!(nearest_256([0, 0, 0]), 16);
        assert_eq!(nearest_256([255, 255, 255]), 231);
        assert_eq!(nearest_256([255, 0, 0]), 196);
        assert_eq!(nearest_256([0, 255, 0]), 46);
        assert_eq!(nearest_256([0, 0, 255]), 21);
    }

    #[test]
    fn nearest_256_prefers_the_grey_ramp_for_off_cube_greys() {
        // 118 is nearer grey step 11 (118) than any cube step (95 or 135).
        assert_eq!(nearest_256([118, 118, 118]), 232 + 11);
    }

    #[test]
    fn rgb_hex_decodes_to_linear() {
        let white = rgb_hex(0xffffff);
        assert!((white[0] - 1.0).abs() < 1e-4);
        let mid = rgb_hex(0x808080);
        assert!(
            mid[0] < 0.3,
            "sRGB 0.5 is about 0.21 in linear, got {}",
            mid[0]
        );
    }
}
