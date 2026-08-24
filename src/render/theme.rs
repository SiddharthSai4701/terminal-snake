use crate::render::color::{rgb_hex, srgb_decode, srgb_encode, Rgb};

/// A theme is pure data, so Phase 4 adds the other seven as table entries
/// rather than as code.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    #[allow(dead_code)] // shown by the theme gallery in Phase 4
    pub name: &'static str,
    pub bg: Rgb,
    pub border: Rgb,
    pub body_head: Rgb,
    pub body_tail: Rgb,
    pub food: Rgb,
    pub glow_tint: Rgb,
    pub highlight: Rgb,
    /// Afterglow e-fold time in seconds.
    pub trail_tau: f32,
    pub trail_gain: f32,
    pub glow_gain: f32,
}

impl Theme {
    pub fn default_theme() -> Theme {
        Theme {
            name: "ember",
            bg: rgb_hex(0x080c11),
            border: rgb_hex(0x1b2734),
            body_head: rgb_hex(0xa8ffd4),
            body_tail: rgb_hex(0x0d5c37),
            food: rgb_hex(0xff5566),
            glow_tint: rgb_hex(0x2bffa0),
            highlight: rgb_hex(0xffffff),
            trail_tau: 0.20,
            trail_gain: 0.35,
            glow_gain: 0.75,
        }
    }

    /// `t` runs 0 at the head to 1 at the tail.
    ///
    /// Interpolated in sRGB space and decoded back, not blended in linear
    /// light. A linear-light blend spends almost its whole length near the
    /// bright end - the gradient stops reading as a gradient at all.
    pub fn body_at(&self, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let mut out = [0.0; 3];
        for k in 0..3 {
            let a = srgb_encode(self.body_head[k]);
            let b = srgb_encode(self.body_tail[k]);
            out[k] = srgb_decode(a * (1.0 - t) + b * t);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Rgb, b: Rgb) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() < 1e-4)
    }

    #[test]
    fn the_body_ramp_runs_head_to_tail() {
        let th = Theme::default_theme();
        // Approximate: the ramp round-trips through sRGB, which is not exact.
        assert!(close(th.body_at(0.0), th.body_head), "{:?}", th.body_at(0.0));
        assert!(close(th.body_at(1.0), th.body_tail), "{:?}", th.body_at(1.0));
    }

    #[test]
    fn the_body_ramp_is_clamped_and_the_head_is_brighter() {
        let th = Theme::default_theme();
        assert!(close(th.body_at(-3.0), th.body_head));
        assert!(close(th.body_at(9.0), th.body_tail));
        let lum = |c: Rgb| c[0] + c[1] + c[2];
        assert!(
            lum(th.body_at(0.0)) > lum(th.body_at(1.0)),
            "head should be brighter than the tail"
        );
    }

    #[test]
    fn the_ramp_is_monotonic_along_its_length() {
        let th = Theme::default_theme();
        let lum = |c: Rgb| c[0] + c[1] + c[2];
        let mut prev = f32::INFINITY;
        for i in 0..=20 {
            let v = lum(th.body_at(i as f32 / 20.0));
            assert!(v <= prev + 1e-6, "ramp brightened at {i}");
            prev = v;
        }
    }

    #[test]
    fn the_ramp_is_perceptually_even_not_linear_in_light() {
        // A ramp interpolated in linear light looks almost entirely like the
        // bright end, because sRGB encoding compresses the highlights. The
        // midpoint must land near the perceptual midpoint of the endpoints.
        let th = Theme::default_theme();
        let mid = th.body_at(0.5);
        for k in 0..3 {
            let want = (srgb_encode(th.body_head[k]) + srgb_encode(th.body_tail[k])) / 2.0;
            let got = srgb_encode(mid[k]);
            assert!(
                (got - want).abs() < 0.06,
                "channel {k}: midpoint encodes to {got}, perceptual middle is {want}"
            );
        }
    }

    #[test]
    fn trail_tau_is_a_sane_number_of_seconds() {
        let th = Theme::default_theme();
        assert!(th.trail_tau > 0.05 && th.trail_tau < 2.0);
    }
}
