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

/// Input that maps to pure white.
///
/// The curve is deliberately the identity on [0, 1] rather than a soft
/// shoulder: authored theme colours must round-trip to their exact bytes, and
/// a shoulder dims every one of them by a few percent. Additive glow above 1.0
/// clips to white, which is how bloom is supposed to look.
pub const WHITE: f32 = 1.0;

pub fn tone_map(v: f32) -> f32 {
    v.clamp(0.0, WHITE)
}

/// Bits kept per channel.
///
/// This was 5 to help the diff renderer skip slowly decaying trail pixels, but
/// measurement showed the saving is not worth the cost. Across a moving scene
/// with a death burst at scale 4, the share of pixels changing per frame is
/// 24.5% at 5 bits, 30.0% at 6, and 41.5% at 8 - about 2.3, 2.8 and 3.9 MB/s
/// of escape codes at 60fps, against a Windows Terminal ceiling nearer 20.
/// Five bits puts a visible step of 8 into every smooth gradient, which shows
/// up as contour rings around the glow and the death flash. Full depth costs
/// 1.6 MB/s more and removes the banding entirely.
pub const QUANT_BITS: u32 = 8;

pub fn to_u8(v: f32) -> u8 {
    let max = ((1u32 << QUANT_BITS) - 1) as f32;
    let q = (v.clamp(0.0, 1.0) * max + 0.5) as u32;
    ((q * 255) / max as u32) as u8
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
        assert_eq!(tone_map(1.0), 1.0);
        assert_eq!(tone_map(4.0), 1.0);
    }

    #[test]
    fn tone_map_is_the_identity_so_authored_colours_round_trip() {
        for i in 0..=255u32 {
            let v = i as f32 / 255.0;
            assert!((tone_map(v) - v).abs() < 1e-9, "dimmed at {i}");
        }
        assert_eq!(to_u8(srgb_encode(srgb_decode(1.0))), 255);
    }

    #[test]
    fn tone_map_clamps_negative_light_to_black() {
        assert_eq!(tone_map(-3.0), 0.0);
    }

    #[test]
    fn to_u8_keeps_the_configured_depth() {
        let levels: std::collections::BTreeSet<u8> =
            (0..=255u32).map(|i| to_u8(i as f32 / 255.0)).collect();
        assert_eq!(levels.len(), 1 << QUANT_BITS);
        assert_eq!(to_u8(0.0), 0);
        assert_eq!(to_u8(1.0), 255);
    }

    #[test]
    fn the_quantization_step_is_small_enough_not_to_band() {
        // A step above about 4 puts visible contour rings into the wide, smooth
        // gradients that the glow and the death flash are made of.
        let step = 255 / ((1u32 << QUANT_BITS) - 1);
        assert!(step <= 4, "quantization step of {step} will band");
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
