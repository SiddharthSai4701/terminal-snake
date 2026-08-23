use crate::render::color::{nearest_256, srgb_encode, to_u8, tone_map, Rgb};
use ratatui_core::buffer::Buffer;
use ratatui_core::style::Color;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ColorTier {
    Full,
    Reduced,
}

/// A linear-light f32 pixel buffer that flushes to terminal cells as half
/// blocks: two stacked pixels per cell, which makes pixels square instead of
/// 2:1 tall.
pub struct Canvas {
    w: usize,
    h: usize,
    px: Vec<Rgb>,
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Self {
        Canvas {
            w,
            h,
            px: vec![[0.0; 3]; w * h],
        }
    }

    pub fn width(&self) -> usize {
        self.w
    }

    pub fn height(&self) -> usize {
        self.h
    }

    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            None
        } else {
            Some(y as usize * self.w + x as usize)
        }
    }

    pub fn clear(&mut self, c: Rgb) {
        self.px.iter_mut().for_each(|p| *p = c);
    }

    pub fn get(&self, x: i32, y: i32) -> Rgb {
        self.idx(x, y).map(|i| self.px[i]).unwrap_or([0.0; 3])
    }

    pub fn set(&mut self, x: i32, y: i32, c: Rgb) {
        if let Some(i) = self.idx(x, y) {
            self.px[i] = c;
        }
    }

    pub fn add(&mut self, x: i32, y: i32, c: Rgb) {
        if let Some(i) = self.idx(x, y) {
            for k in 0..3 {
                self.px[i][k] += c[k];
            }
        }
    }

    pub fn blend(&mut self, x: i32, y: i32, c: Rgb, cov: f32) {
        if let Some(i) = self.idx(x, y) {
            let a = cov.clamp(0.0, 1.0);
            for k in 0..3 {
                self.px[i][k] = self.px[i][k] * (1.0 - a) + c[k] * a;
            }
        }
    }

    fn encode(&self, p: Rgb) -> [u8; 3] {
        [
            to_u8(srgb_encode(tone_map(p[0]))),
            to_u8(srgb_encode(tone_map(p[1]))),
            to_u8(srgb_encode(tone_map(p[2]))),
        ]
    }

    /// Writes the canvas into `buf` as half-block cells starting at
    /// `origin` = (col, row).
    pub fn quantize_into(&self, buf: &mut Buffer, origin: (u16, u16), tier: ColorTier) {
        let paint = |c: [u8; 3]| match tier {
            ColorTier::Full => Color::Rgb(c[0], c[1], c[2]),
            ColorTier::Reduced => Color::Indexed(nearest_256(c)),
        };

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

                let top = self.encode(self.px[(cy * 2) * self.w + cx]);
                let bot = self.encode(self.px[(cy * 2 + 1) * self.w + cx]);

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

    #[test]
    fn two_different_pixels_become_an_upper_half_block() {
        let mut c = Canvas::new(1, 2);
        c.set(0, 0, [1.0, 0.0, 0.0]);
        c.set(0, 1, [0.0, 0.0, 1.0]);
        let mut b = buf(1, 1);
        c.quantize_into(&mut b, (0, 0), ColorTier::Full);
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
        c.quantize_into(&mut b, (0, 0), ColorTier::Full);
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
        c.quantize_into(&mut b, (0, 0), ColorTier::Full);
        assert_eq!(b[(0, 0)].symbol(), " ");
    }

    #[test]
    fn reduced_tier_emits_indexed_colour() {
        let mut c = Canvas::new(1, 2);
        c.set(0, 0, [1.0, 0.0, 0.0]);
        c.set(0, 1, [0.0, 0.0, 0.0]);
        let mut b = buf(1, 1);
        c.quantize_into(&mut b, (0, 0), ColorTier::Reduced);
        assert_eq!(b[(0, 0)].fg, Color::Indexed(196));
        assert_eq!(b[(0, 0)].bg, Color::Indexed(16));
    }

    #[test]
    fn writes_land_at_the_given_origin() {
        let mut c = Canvas::new(1, 2);
        c.set(0, 0, [1.0, 1.0, 1.0]);
        c.set(0, 1, [0.0, 0.0, 0.0]);
        let mut b = buf(4, 4);
        c.quantize_into(&mut b, (2, 3), ColorTier::Full);
        assert_eq!(b[(2, 3)].symbol(), "\u{2580}");
        assert_eq!(b[(0, 0)].symbol(), " ");
    }

    #[test]
    fn out_of_bounds_writes_are_ignored_not_panics() {
        let mut c = Canvas::new(2, 2);
        c.set(-1, 0, [1.0, 0.0, 0.0]);
        c.set(99, 0, [1.0, 0.0, 0.0]);
        c.blend(0, -5, [1.0, 0.0, 0.0], 1.0);
        c.add(-3, -3, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn quantizing_into_a_buffer_smaller_than_the_canvas_does_not_panic() {
        let mut c = Canvas::new(8, 8);
        c.clear([0.5, 0.5, 0.5]);
        let mut b = buf(2, 2);
        c.quantize_into(&mut b, (0, 0), ColorTier::Full);
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
        c.quantize_into(&mut b, (0, 0), ColorTier::Full);
        assert_eq!(b[(0, 0)].symbol(), " ");
    }
}
