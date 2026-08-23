use crate::render::color::{rgb_hex, Rgb};

/// A theme is pure data, so Phase 4 adds the other seven as table entries
/// rather than as code.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
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
    pub fn body_at(&self, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let mut out = [0.0; 3];
        for k in 0..3 {
            out[k] = self.body_head[k] * (1.0 - t) + self.body_tail[k] * t;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_body_ramp_runs_head_to_tail() {
        let th = Theme::default_theme();
        assert_eq!(th.body_at(0.0), th.body_head);
        assert_eq!(th.body_at(1.0), th.body_tail);
    }

    #[test]
    fn the_body_ramp_is_clamped_and_the_head_is_brighter() {
        let th = Theme::default_theme();
        assert_eq!(th.body_at(-3.0), th.body_head);
        assert_eq!(th.body_at(9.0), th.body_tail);
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
    fn trail_tau_is_a_sane_number_of_seconds() {
        let th = Theme::default_theme();
        assert!(th.trail_tau > 0.05 && th.trail_tau < 2.0);
    }
}
