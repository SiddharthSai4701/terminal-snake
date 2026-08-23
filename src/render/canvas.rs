use crate::render::color::{nearest_256, srgb_encode, to_u8, tone_map, Rgb};
use ratatui_core::buffer::Buffer;
use ratatui_core::style::Color;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ColorTier {
    Full,
    Reduced,
}

/// A linear-light pixel canvas that flushes to terminal cells as half blocks:
/// two stacked pixels per cell, which makes pixels square instead of 2:1 tall.
///
/// Three layers composite at sample time:
///
/// - `base`   full resolution, cleared every frame - backdrop, border, ribbon, food
/// - `glow`   half resolution, cleared every frame, blurred before use
/// - `trail`  full resolution, persistent, decayed by wall-clock time
pub struct Canvas {
    w: usize,
    h: usize,
    gw: usize,
    gh: usize,
    base: Vec<Rgb>,
    glow: Vec<Rgb>,
    trail: Vec<Rgb>,
    glow_gain: f32,
    trail_gain: f32,
    blur_scratch: Vec<Rgb>,
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Self {
        let gw = w.div_ceil(2).max(1);
        let gh = h.div_ceil(2).max(1);
        Canvas {
            w,
            h,
            gw,
            gh,
            base: vec![[0.0; 3]; w * h],
            glow: vec![[0.0; 3]; gw * gh],
            trail: vec![[0.0; 3]; w * h],
            glow_gain: 1.0,
            trail_gain: 1.0,
            blur_scratch: vec![[0.0; 3]; gw * gh],
        }
    }

    pub fn width(&self) -> usize {
        self.w
    }

    pub fn height(&self) -> usize {
        self.h
    }

    pub fn set_gains(&mut self, glow_gain: f32, trail_gain: f32) {
        self.glow_gain = glow_gain;
        self.trail_gain = trail_gain;
    }

    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            None
        } else {
            Some(y as usize * self.w + x as usize)
        }
    }

    fn gidx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x as usize >= self.gw || y as usize >= self.gh {
            None
        } else {
            Some(y as usize * self.gw + x as usize)
        }
    }

    // --- base layer -------------------------------------------------------

    pub fn clear_base(&mut self, c: Rgb) {
        self.base.iter_mut().for_each(|p| *p = c);
        self.glow.iter_mut().for_each(|p| *p = [0.0; 3]);
    }

    #[allow(dead_code)] // read by the canvas, draw, and arena tests
    pub fn get(&self, x: i32, y: i32) -> Rgb {
        self.idx(x, y).map(|i| self.base[i]).unwrap_or([0.0; 3])
    }

    pub fn set(&mut self, x: i32, y: i32, c: Rgb) {
        if let Some(i) = self.idx(x, y) {
            self.base[i] = c;
        }
    }

    pub fn blend(&mut self, x: i32, y: i32, c: Rgb, cov: f32) {
        if let Some(i) = self.idx(x, y) {
            let a = cov.clamp(0.0, 1.0);
            for k in 0..3 {
                self.base[i][k] = self.base[i][k] * (1.0 - a) + c[k] * a;
            }
        }
    }

    // --- glow layer -------------------------------------------------------

    /// Splats light into the half-resolution glow accumulator. Coordinates are
    /// in full-resolution pixel space.
    pub fn add_glow(&mut self, x: f32, y: f32, c: Rgb) {
        let gx = (x * 0.5).round() as i32;
        let gy = (y * 0.5).round() as i32;
        if let Some(i) = self.gidx(gx, gy) {
            for k in 0..3 {
                self.glow[i][k] += c[k];
            }
        }
    }

    /// Separable 5-tap Gaussian, two passes, at half resolution.
    ///
    /// Without this the glow buffer is not bloom at all - additive brightness
    /// with no spatial spread is just a brighter pixel.
    pub fn blur_glow(&mut self) {
        const K: [f32; 5] = [0.0625, 0.25, 0.375, 0.25, 0.0625];
        let (gw, gh) = (self.gw, self.gh);

        for y in 0..gh {
            for x in 0..gw {
                let mut acc = [0.0f32; 3];
                for (t, &weight) in K.iter().enumerate() {
                    let sx = (x as i32 + t as i32 - 2).clamp(0, gw as i32 - 1) as usize;
                    let s = self.glow[y * gw + sx];
                    for k in 0..3 {
                        acc[k] += s[k] * weight;
                    }
                }
                self.blur_scratch[y * gw + x] = acc;
            }
        }
        for y in 0..gh {
            for x in 0..gw {
                let mut acc = [0.0f32; 3];
                for (t, &weight) in K.iter().enumerate() {
                    let sy = (y as i32 + t as i32 - 2).clamp(0, gh as i32 - 1) as usize;
                    let s = self.blur_scratch[sy * gw + x];
                    for k in 0..3 {
                        acc[k] += s[k] * weight;
                    }
                }
                self.glow[y * gw + x] = acc;
            }
        }
    }

    fn glow_at(&self, x: i32, y: i32) -> Rgb {
        let gx = (x / 2).clamp(0, self.gw as i32 - 1) as usize;
        let gy = (y / 2).clamp(0, self.gh as i32 - 1) as usize;
        self.glow[gy * self.gw + gx]
    }

    // --- trail layer ------------------------------------------------------

    pub fn add_trail(&mut self, x: i32, y: i32, c: Rgb) {
        if let Some(i) = self.idx(x, y) {
            for k in 0..3 {
                self.trail[i][k] += c[k];
            }
        }
    }

    /// Decay by wall-clock time, not per frame. A per-frame constant makes the
    /// afterglow twice as long at 30fps and invisible at 144Hz.
    pub fn decay_trail(&mut self, dt: f32, tau: f32) {
        let k = (-dt / tau.max(1e-4)).exp();
        self.trail.iter_mut().for_each(|p| {
            for c in p.iter_mut() {
                *c *= k;
            }
        });
    }

    // --- composite --------------------------------------------------------

    /// The composited linear-light value at a pixel, with edge clamping so
    /// screen shake never samples outside the buffer.
    pub fn sample(&self, x: i32, y: i32) -> Rgb {
        let cx = x.clamp(0, self.w as i32 - 1);
        let cy = y.clamp(0, self.h as i32 - 1);
        let i = cy as usize * self.w + cx as usize;
        let g = self.glow_at(cx, cy);
        let t = self.trail[i];
        let b = self.base[i];
        [
            b[0] + g[0] * self.glow_gain + t[0] * self.trail_gain,
            b[1] + g[1] * self.glow_gain + t[1] * self.trail_gain,
            b[2] + g[2] * self.glow_gain + t[2] * self.trail_gain,
        ]
    }

    fn encode(&self, p: Rgb) -> [u8; 3] {
        [
            to_u8(srgb_encode(tone_map(p[0]))),
            to_u8(srgb_encode(tone_map(p[1]))),
            to_u8(srgb_encode(tone_map(p[2]))),
        ]
    }

    /// Writes the canvas into `buf` as half-block cells starting at
    /// `origin` = (col, row). `shake` offsets the sample origin in pixels.
    pub fn quantize_into(
        &self,
        buf: &mut Buffer,
        origin: (u16, u16),
        tier: ColorTier,
        shake: (f32, f32),
    ) {
        let paint = |c: [u8; 3]| match tier {
            ColorTier::Full => Color::Rgb(c[0], c[1], c[2]),
            ColorTier::Reduced => Color::Indexed(nearest_256(c)),
        };
        let sx = shake.0.round() as i32;
        let sy = shake.1.round() as i32;

        let rows = self.h / 2;
        for cy in 0..rows {
            let row = origin.1 as usize + cy;
            if row > u16::MAX as usize {
                continue;
            }
            for cx in 0..self.w {
                let col = origin.0 as usize + cx;
                if col > u16::MAX as usize {
                    continue;
                }
                let Some(cell) = buf.cell_mut((col as u16, row as u16)) else {
                    continue;
                };

                let top = self.encode(self.sample(cx as i32 + sx, (cy * 2) as i32 + sy));
                let bot = self.encode(self.sample(cx as i32 + sx, (cy * 2 + 1) as i32 + sy));

                // Compared after quantizing to u8, not as floats: two floats
                // within an epsilon can still land on different bytes, and two
                // outside it can land on the same one.
                if top == bot {
                    // Reset rather than fg == bg, so the backend can skip the
                    // foreground SGR across runs of flat backdrop.
                    cell.set_char(' ').set_fg(Color::Reset).set_bg(paint(bot));
                } else {
                    cell.set_char('\u{2580}')
                        .set_fg(paint(top))
                        .set_bg(paint(bot));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::layout::Rect;

    fn buf(w: u16, h: u16) -> Buffer {
        Buffer::empty(Rect::new(0, 0, w, h))
    }

    fn total_light(c: &Canvas) -> f32 {
        (0..c.height() as i32)
            .flat_map(|y| (0..c.width() as i32).map(move |x| (x, y)))
            .map(|(x, y)| c.sample(x, y).iter().sum::<f32>())
            .sum()
    }

    #[test]
    fn two_different_pixels_become_an_upper_half_block() {
        let mut c = Canvas::new(1, 2);
        c.set(0, 0, [1.0, 0.0, 0.0]);
        c.set(0, 1, [0.0, 0.0, 1.0]);
        let mut b = buf(1, 1);
        c.quantize_into(&mut b, (0, 0), ColorTier::Full, (0.0, 0.0));
        let cell = &b[(0, 0)];
        assert_eq!(cell.symbol(), "\u{2580}");
        assert_eq!(cell.fg, Color::Rgb(255, 0, 0));
        assert_eq!(cell.bg, Color::Rgb(0, 0, 255));
    }

    #[test]
    fn an_equal_pair_collapses_to_a_space_with_reset_fg() {
        let mut c = Canvas::new(1, 2);
        c.set(0, 0, [0.25, 0.25, 0.25]);
        c.set(0, 1, [0.25, 0.25, 0.25]);
        let mut b = buf(1, 1);
        c.quantize_into(&mut b, (0, 0), ColorTier::Full, (0.0, 0.0));
        let cell = &b[(0, 0)];
        assert_eq!(cell.symbol(), " ");
        assert_eq!(cell.fg, Color::Reset);
        assert!(matches!(cell.bg, Color::Rgb(..)));
    }

    #[test]
    fn near_equal_pixels_that_quantize_alike_also_collapse() {
        let mut c = Canvas::new(1, 2);
        c.set(0, 0, [0.5000, 0.5, 0.5]);
        c.set(0, 1, [0.5001, 0.5, 0.5]);
        let mut b = buf(1, 1);
        c.quantize_into(&mut b, (0, 0), ColorTier::Full, (0.0, 0.0));
        assert_eq!(b[(0, 0)].symbol(), " ");
    }

    #[test]
    fn reduced_tier_emits_indexed_colour() {
        let mut c = Canvas::new(1, 2);
        c.set(0, 0, [1.0, 0.0, 0.0]);
        c.set(0, 1, [0.0, 0.0, 0.0]);
        let mut b = buf(1, 1);
        c.quantize_into(&mut b, (0, 0), ColorTier::Reduced, (0.0, 0.0));
        assert_eq!(b[(0, 0)].fg, Color::Indexed(196));
        assert_eq!(b[(0, 0)].bg, Color::Indexed(16));
    }

    #[test]
    fn writes_land_at_the_given_origin() {
        let mut c = Canvas::new(1, 2);
        c.set(0, 0, [1.0, 1.0, 1.0]);
        c.set(0, 1, [0.0, 0.0, 0.0]);
        let mut b = buf(4, 4);
        c.quantize_into(&mut b, (2, 3), ColorTier::Full, (0.0, 0.0));
        assert_eq!(b[(2, 3)].symbol(), "\u{2580}");
        assert_eq!(b[(0, 0)].symbol(), " ");
    }

    #[test]
    fn out_of_bounds_writes_are_ignored_not_panics() {
        let mut c = Canvas::new(2, 2);
        c.set(-1, 0, [1.0, 0.0, 0.0]);
        c.set(99, 0, [1.0, 0.0, 0.0]);
        c.blend(0, -5, [1.0, 0.0, 0.0], 1.0);
        c.add_trail(-3, -3, [1.0, 0.0, 0.0]);
        c.add_glow(-40.0, -40.0, [1.0, 0.0, 0.0]);
        c.add_glow(400.0, 400.0, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn quantizing_into_a_buffer_smaller_than_the_canvas_does_not_panic() {
        let mut c = Canvas::new(8, 8);
        c.clear_base([0.5, 0.5, 0.5]);
        let mut b = buf(2, 2);
        c.quantize_into(&mut b, (0, 0), ColorTier::Full, (0.0, 0.0));
    }

    #[test]
    fn blend_interpolates_by_coverage() {
        let mut c = Canvas::new(1, 1);
        c.set(0, 0, [0.0, 0.0, 0.0]);
        c.blend(0, 0, [1.0, 1.0, 1.0], 0.5);
        assert!((c.get(0, 0)[0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn negative_light_cannot_darken_below_black() {
        let mut c = Canvas::new(1, 2);
        c.set(0, 0, [-5.0, -5.0, -5.0]);
        c.set(0, 1, [0.0, 0.0, 0.0]);
        let mut b = buf(1, 1);
        c.quantize_into(&mut b, (0, 0), ColorTier::Full, (0.0, 0.0));
        assert_eq!(b[(0, 0)].symbol(), " ");
    }

    #[test]
    fn glow_spreads_light_to_neighbouring_pixels() {
        let mut c = Canvas::new(16, 16);
        c.clear_base([0.0; 3]);
        c.add_glow(8.0, 8.0, [1.0, 1.0, 1.0]);
        let before = c.sample(13, 8)[0];
        c.blur_glow();
        let after = c.sample(13, 8)[0];
        assert!(
            after > before,
            "blur should carry light outward: {before} -> {after}"
        );
    }

    #[test]
    fn glow_roughly_conserves_the_light_it_was_given() {
        let mut c = Canvas::new(32, 32);
        c.clear_base([0.0; 3]);
        c.add_glow(16.0, 16.0, [1.0, 0.0, 0.0]);
        let before = total_light(&c);
        c.blur_glow();
        let after = total_light(&c);
        assert!(
            (after - before).abs() / before < 0.25,
            "before {before}, after {after}"
        );
    }

    #[test]
    fn the_trail_decays_by_wall_clock_not_by_frame() {
        let mut a = Canvas::new(2, 2);
        a.add_trail(0, 0, [1.0, 1.0, 1.0]);
        a.decay_trail(0.1, 0.1);
        let one_step = a.sample(0, 0)[0];

        let mut b = Canvas::new(2, 2);
        b.add_trail(0, 0, [1.0, 1.0, 1.0]);
        for _ in 0..10 {
            b.decay_trail(0.01, 0.1);
        }
        let ten_steps = b.sample(0, 0)[0];

        assert!(
            (one_step - ten_steps).abs() < 1e-3,
            "{one_step} vs {ten_steps}"
        );
    }

    #[test]
    fn one_tau_of_decay_leaves_about_a_third() {
        let mut c = Canvas::new(2, 2);
        c.add_trail(0, 0, [1.0, 1.0, 1.0]);
        c.decay_trail(0.2, 0.2);
        let v = c.sample(0, 0)[0];
        assert!(
            (v - std::f32::consts::E.recip()).abs() < 1e-3,
            "got {v}, expected 1/e"
        );
    }

    #[test]
    fn clearing_the_base_leaves_the_trail_alone_but_wipes_the_glow() {
        let mut c = Canvas::new(2, 2);
        c.add_trail(0, 0, [0.5, 0.5, 0.5]);
        c.add_glow(0.0, 0.0, [0.5, 0.5, 0.5]);
        c.clear_base([0.0; 3]);
        assert!(c.sample(0, 0)[0] > 0.4, "trail should survive");
        c.add_trail(0, 0, [-0.5, -0.5, -0.5]);
        assert!(c.sample(0, 0)[0].abs() < 1e-6, "glow should not have survived");
    }

    #[test]
    fn shake_offsets_the_sampled_image() {
        let mut c = Canvas::new(4, 4);
        c.clear_base([0.0; 3]);
        c.set(0, 0, [1.0, 1.0, 1.0]);
        let mut plain = buf(4, 2);
        c.quantize_into(&mut plain, (0, 0), ColorTier::Full, (0.0, 0.0));
        let mut shaken = buf(4, 2);
        c.quantize_into(&mut shaken, (0, 0), ColorTier::Full, (1.0, 0.0));
        // The lit pixel is the top half of cell (0,0); shifting the sample
        // origin one pixel right moves it out of that cell entirely.
        assert_eq!(plain[(0, 0)].symbol(), "\u{2580}");
        assert_eq!(plain[(0, 0)].fg, Color::Rgb(255, 255, 255));
        assert_eq!(shaken[(0, 0)].symbol(), " ");
    }

    #[test]
    fn shake_clamps_at_the_edges_instead_of_sampling_black() {
        let mut c = Canvas::new(4, 4);
        c.clear_base([0.3, 0.3, 0.3]);
        let mut b = buf(4, 2);
        c.quantize_into(&mut b, (0, 0), ColorTier::Full, (-99.0, -99.0));
        assert!(matches!(b[(0, 0)].bg, Color::Rgb(r, _, _) if r > 0));
    }

    #[test]
    fn gains_scale_the_glow_and_trail_contributions() {
        let mut c = Canvas::new(4, 4);
        c.clear_base([0.0; 3]);
        c.add_trail(1, 1, [1.0, 1.0, 1.0]);
        c.set_gains(1.0, 0.5);
        assert!((c.sample(1, 1)[0] - 0.5).abs() < 1e-6);
        c.set_gains(1.0, 0.0);
        assert!(c.sample(1, 1)[0].abs() < 1e-6);
    }
}
