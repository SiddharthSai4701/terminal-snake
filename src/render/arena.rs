//! Phase 1 arena drawing: flat colour, one filled square per logic cell.
//!
//! The SDF ribbon, glow, particles, and trail arrive in Phase 2 and replace the
//! body drawing here; the canvas contract does not change.

use crate::game::{Game, GameState};
use crate::render::canvas::Canvas;
use crate::render::color::{rgb_hex, Rgb};
use crate::render::layout::Layout;

const BG: u32 = 0x0b0f14;
const BORDER: u32 = 0x1e2a38;
const BODY: u32 = 0x27c26b;
const HEAD: u32 = 0x8affc1;
const FOOD: u32 = 0xff4d5a;

fn scale_rgb(c: Rgb, k: f32) -> Rgb {
    [c[0] * k, c[1] * k, c[2] * k]
}

fn fill_cell(c: &mut Canvas, cell_x: i32, cell_y: i32, scale: i32, col: Rgb, inset: i32) {
    let ox = 1 + cell_x * scale;
    let oy = 1 + cell_y * scale;
    for dy in inset..(scale - inset) {
        for dx in inset..(scale - inset) {
            c.set(ox + dx, oy + dy, col);
        }
    }
}

pub fn draw_arena(c: &mut Canvas, game: &Game, layout: &Layout) {
    let s = layout.scale as i32;
    c.clear_base(rgb_hex(BG));

    let border = rgb_hex(BORDER);
    let w = c.width() as i32;
    let h = c.height() as i32;
    for x in 0..w {
        c.set(x, 0, border);
        c.set(x, h - 1, border);
    }
    for y in 0..h {
        c.set(0, y, border);
        c.set(w - 1, y, border);
    }

    let inset = if s >= 4 { 1 } else { 0 };
    fill_cell(c, game.food().x, game.food().y, s, rgb_hex(FOOD), inset);

    // Dimmed once dead, so the final frame reads as an ending rather than a
    // freeze.
    let dim = if game.state == GameState::Dead { 0.35 } else { 1.0 };
    let body = scale_rgb(rgb_hex(BODY), dim);
    let head = scale_rgb(rgb_hex(HEAD), dim);
    for (i, p) in game.snake().iter().enumerate() {
        let col = if i == 0 { head } else { body };
        fill_cell(c, p.x, p.y, s, col, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Game, Mode};

    fn setup(seed: u64) -> (Game, Layout, Canvas) {
        let g = Game::new(Mode::Classic, seed);
        let l = Layout::compute(120, 40, 4).unwrap();
        let c = Canvas::new(l.canvas_w, l.canvas_h);
        (g, l, c)
    }

    fn cell_centre(l: &Layout, cx: i32, cy: i32) -> (i32, i32) {
        let s = l.scale as i32;
        (1 + cx * s + s / 2, 1 + cy * s + s / 2)
    }

    #[test]
    fn the_head_cell_is_lit() {
        let (g, l, mut c) = setup(1);
        draw_arena(&mut c, &g, &l);
        let (x, y) = cell_centre(&l, g.snake().head().x, g.snake().head().y);
        assert!(c.get(x, y)[1] > 0.05, "head cell should be lit");
    }

    #[test]
    fn every_body_cell_is_lit() {
        let (g, l, mut c) = setup(1);
        draw_arena(&mut c, &g, &l);
        for p in g.snake().iter() {
            let (x, y) = cell_centre(&l, p.x, p.y);
            assert!(c.get(x, y)[1] > 0.05, "body cell {p:?} should be lit");
        }
    }

    #[test]
    fn the_food_cell_is_lit() {
        let (g, l, mut c) = setup(9);
        draw_arena(&mut c, &g, &l);
        let (x, y) = cell_centre(&l, g.food().x, g.food().y);
        assert!(c.get(x, y)[0] > 0.1, "food cell should be lit");
    }

    #[test]
    fn empty_cells_stay_near_the_background() {
        let (g, l, mut c) = setup(1);
        draw_arena(&mut c, &g, &l);
        let mut checked = 0;
        for cy in 0..18 {
            for cx in 0..28 {
                let p = crate::game::Pos::new(cx, cy);
                if g.snake().contains(p) || g.food() == p {
                    continue;
                }
                let (x, y) = cell_centre(&l, cx, cy);
                assert!(c.get(x, y).iter().sum::<f32>() < 0.05, "cell {p:?} is lit");
                checked += 1;
            }
        }
        assert!(checked > 400);
    }

    #[test]
    fn the_border_is_drawn_on_every_edge() {
        let (g, l, mut c) = setup(1);
        draw_arena(&mut c, &g, &l);
        for x in 0..l.canvas_w as i32 {
            assert!(c.get(x, 0).iter().sum::<f32>() > 0.0, "top gap at {x}");
            assert!(c.get(x, l.canvas_h as i32 - 1).iter().sum::<f32>() > 0.0);
        }
        for y in 0..l.canvas_h as i32 {
            assert!(c.get(0, y).iter().sum::<f32>() > 0.0, "left gap at {y}");
            assert!(c.get(l.canvas_w as i32 - 1, y).iter().sum::<f32>() > 0.0);
        }
    }

    #[test]
    fn drawing_at_the_minimum_scale_does_not_panic() {
        let g = Game::new(Mode::Classic, 2);
        let l = Layout::compute(86, 31, 4).unwrap();
        let mut c = Canvas::new(l.canvas_w, l.canvas_h);
        draw_arena(&mut c, &g, &l);
        assert_eq!(l.scale, 3);
    }
}
