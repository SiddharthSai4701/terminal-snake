//! Arena drawing: interpolated ribbon body, glowing food, afterglow trail,
//! particles, and flash, composited in linear light.

use crate::game::{Game, GameState};
use crate::render::canvas::Canvas;
use crate::render::color::Rgb;
use crate::render::draw::{disc, stroke_segment, Stroke};
use crate::render::fx::Fx;
use crate::render::layout::Layout;
use crate::render::ribbon::{cell_centre, is_seam, snake_path};
use crate::render::theme::Theme;

/// How fast the gloss band travels along the body, in body-lengths per second.
const HIGHLIGHT_SPEED: f32 = 0.55;
const HIGHLIGHT_WIDTH: f32 = 0.13;

fn scale_rgb(c: Rgb, k: f32) -> Rgb {
    [c[0] * k, c[1] * k, c[2] * k]
}

fn add_rgb(a: Rgb, b: Rgb) -> Rgb {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn draw_border(c: &mut Canvas, col: Rgb) {
    let w = c.width() as i32;
    let h = c.height() as i32;
    for x in 0..w {
        c.set(x, 0, col);
        c.set(x, h - 1, col);
    }
    for y in 0..h {
        c.set(0, y, col);
        c.set(w - 1, y, col);
    }
}

/// `clock` is the run's elapsed time, used for the breathing food and the
/// travelling gloss band; both must move with wall-clock time, not frames.
pub fn draw_arena(
    c: &mut Canvas,
    game: &Game,
    layout: &Layout,
    theme: &Theme,
    fx: &Fx,
    dt: f32,
    clock: f32,
) {
    let s = layout.scale as i32;
    let dead = game.state == GameState::Dead;

    c.set_gains(theme.glow_gain, theme.trail_gain);
    c.decay_trail(dt, theme.trail_tau);
    c.clear_base(theme.bg);
    draw_border(c, theme.border);

    // Food breathes so an idle board is never completely static.
    let pulse = 0.75 + 0.25 * (clock * 3.4).sin();
    let fx_pos = (
        cell_centre(game.food().x, s),
        cell_centre(game.food().y, s),
    );
    disc(
        c,
        fx_pos,
        0.34 * s as f32 + 0.10 * s as f32 * pulse,
        theme.food,
        1.0,
    );
    c.add_glow(
        fx_pos.0,
        fx_pos.1,
        scale_rgb(theme.food, 0.55 * pulse),
    );

    // The body: one capsule per segment, each with its own ramp colour, so the
    // cost is O(length x scale^2) rather than O(length x arena).
    let stroke = Stroke::for_scale(layout.scale);
    let path = snake_path(game.snake(), game.tick_fraction(), s, game.mode().wraps());
    let n = path.len().max(1);
    let dim = if dead { 0.30 } else { 1.0 };
    let phase = clock * HIGHLIGHT_SPEED;

    if !dead {
        for i in (1..path.len()).rev() {
            let a = path[i - 1];
            let b = path[i];
            if is_seam(a, b, s) {
                continue;
            }
            let t = i as f32 / n as f32;

            let mut col = theme.body_at(t);
            // A gloss band that travels tail-ward, so the snake looks lit even
            // while it is standing still.
            let band = ((t + phase).fract() - 0.5).abs();
            if band < HIGHLIGHT_WIDTH {
                let k = 1.0 - band / HIGHLIGHT_WIDTH;
                col = add_rgb(col, scale_rgb(theme.highlight, 0.22 * k * k));
            }
            stroke_segment(c, a, b, col, &stroke);
        }

        let head = path[0];
        disc(c, head, stroke.radius * 1.05, theme.body_head, 1.0);
        c.add_glow(head.0, head.1, scale_rgb(theme.glow_tint, 0.85));
        c.add_trail(
            head.0.round() as i32,
            head.1.round() as i32,
            scale_rgb(theme.glow_tint, 0.7),
        );
    } else {
        // Once dead the body has been handed to the particle system; leave a
        // dim ghost of where it was.
        for i in 1..path.len() {
            let (a, b) = (path[i - 1], path[i]);
            if is_seam(a, b, s) {
                continue;
            }
            let col = scale_rgb(theme.body_at(i as f32 / n as f32), dim);
            stroke_segment(c, a, b, col, &stroke);
        }
    }

    fx.draw(c);

    let flash = fx.flash();
    if flash > 0.0 {
        let w = c.width() as i32;
        let h = c.height() as i32;
        let add = [flash * 0.9, flash * 0.9, flash * 0.9];
        for y in 0..h {
            for x in 0..w {
                let base = c.get(x, y);
                c.set(x, y, add_rgb(base, add));
            }
        }
    }

    c.blur_glow();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::types::Direction;
    use crate::game::{Game, Mode};
    use crate::input::DirQueue;

    fn setup(seed: u64) -> (Game, Layout, Canvas, Fx, Theme) {
        let g = Game::new(Mode::Classic, seed);
        let l = Layout::compute(120, 40, 4).unwrap();
        let c = Canvas::new(l.canvas_w, l.canvas_h);
        (g, l, c, Fx::new(seed), Theme::default_theme())
    }

    fn cell_px(l: &Layout, cx: i32, cy: i32) -> (i32, i32) {
        let s = l.scale as i32;
        (cell_centre(cx, s) as i32, cell_centre(cy, s) as i32)
    }

    #[test]
    fn the_head_cell_is_lit() {
        let (g, l, mut c, fx, th) = setup(1);
        draw_arena(&mut c, &g, &l, &th, &fx, 0.016, 0.0);
        let (x, y) = cell_px(&l, g.snake().head().x, g.snake().head().y);
        assert!(c.sample(x, y)[1] > 0.05, "head cell should be lit");
    }

    #[test]
    fn every_body_cell_is_lit() {
        let (g, l, mut c, fx, th) = setup(1);
        draw_arena(&mut c, &g, &l, &th, &fx, 0.016, 0.0);
        for p in g.snake().iter() {
            let (x, y) = cell_px(&l, p.x, p.y);
            assert!(c.sample(x, y)[1] > 0.03, "body cell {p:?} should be lit");
        }
    }

    #[test]
    fn the_ribbon_has_no_gap_between_consecutive_cells() {
        // This is the whole point of a ribbon over flat squares: the midpoint
        // between two cell centres must be lit too.
        let (g, l, mut c, fx, th) = setup(1);
        draw_arena(&mut c, &g, &l, &th, &fx, 0.016, 0.0);
        let s = l.scale as i32;
        let head = g.snake().head();
        let mid_x = (cell_centre(head.x, s) + cell_centre(head.x - 1, s)) / 2.0;
        let mid_y = cell_centre(head.y, s);
        assert!(
            c.sample(mid_x as i32, mid_y as i32)[1] > 0.05,
            "gap between segments"
        );
    }

    #[test]
    fn the_food_cell_is_lit() {
        let (g, l, mut c, fx, th) = setup(9);
        draw_arena(&mut c, &g, &l, &th, &fx, 0.016, 0.0);
        let (x, y) = cell_px(&l, g.food().x, g.food().y);
        assert!(c.sample(x, y)[0] > 0.1, "food cell should be lit");
    }

    #[test]
    fn the_border_is_drawn_on_every_edge() {
        let (g, l, mut c, fx, th) = setup(1);
        draw_arena(&mut c, &g, &l, &th, &fx, 0.016, 0.0);
        for x in 0..l.canvas_w as i32 {
            assert!(c.get(x, 0).iter().sum::<f32>() > 0.0, "top gap at {x}");
            assert!(c.get(x, l.canvas_h as i32 - 1).iter().sum::<f32>() > 0.0);
        }
        for y in 0..l.canvas_h as i32 {
            assert!(c.get(0, y).iter().sum::<f32>() > 0.0, "left gap at {y}");
            assert!(c.get(l.canvas_w as i32 - 1, y).iter().sum::<f32>() > 0.0);
        }
    }

    /// Runs the snake far enough right that its starting cell is both trailed
    /// and vacated, and returns that cell.
    fn run_past_start(g: &mut Game, l: &Layout, c: &mut Canvas, th: &Theme, fx: &Fx) -> (i32, i32) {
        let mut q = DirQueue::new(Direction::Right);
        g.start();
        let start = g.snake().head();
        // Drawn first, so the head lays a trail on the starting cell itself.
        draw_arena(c, g, l, th, fx, 0.016, 0.0);
        for _ in 0..8 {
            g.advance(0.14, &mut q);
            draw_arena(c, g, l, th, fx, 0.016, 0.0);
        }
        assert!(!g.snake().contains(start), "the start cell should be vacated");
        cell_px(l, start.x, start.y)
    }

    #[test]
    fn the_snake_leaves_an_afterglow_behind_it() {
        let (mut g, l, mut c, fx, th) = setup(3);
        let (bx, by) = cell_px(&l, 2, 15);
        let (x, y) = run_past_start(&mut g, &l, &mut c, &th, &fx);
        let backdrop = c.sample(bx, by).iter().sum::<f32>();
        assert!(
            c.sample(x, y).iter().sum::<f32>() > backdrop + 0.01,
            "a vacated cell should still glow"
        );
    }

    #[test]
    fn the_trail_fades_out_over_time() {
        let (mut g, l, mut c, fx, th) = setup(3);
        let (bx, by) = cell_px(&l, 2, 15);
        let (x, y) = run_past_start(&mut g, &l, &mut c, &th, &fx);
        // Measured against an untouched cell rather than zero: the backdrop is
        // not black, so it would otherwise dominate the ratio.
        let backdrop = c.sample(bx, by).iter().sum::<f32>();
        let lit = c.sample(x, y).iter().sum::<f32>();
        assert!(lit > backdrop + 0.01, "no trail was laid down");

        for _ in 0..40 {
            draw_arena(&mut c, &g, &l, &th, &fx, 0.05, 0.0);
        }
        let faded = c.sample(x, y).iter().sum::<f32>();
        assert!(
            faded - backdrop < (lit - backdrop) * 0.05,
            "the trail should have faded: lit {lit}, faded {faded}, backdrop {backdrop}"
        );
    }

    #[test]
    fn a_flash_brightens_the_whole_canvas() {
        let (g, l, mut c, mut fx, th) = setup(1);
        draw_arena(&mut c, &g, &l, &th, &fx, 0.016, 0.0);
        let corner_before = c.sample(l.canvas_w as i32 / 2, 4).iter().sum::<f32>();
        fx.emit_death(&[(10.0, 10.0)], th.body_head);
        draw_arena(&mut c, &g, &l, &th, &fx, 0.016, 0.0);
        let corner_after = c.sample(l.canvas_w as i32 / 2, 4).iter().sum::<f32>();
        assert!(
            corner_after > corner_before + 0.5,
            "{corner_before} -> {corner_after}"
        );
    }

    #[test]
    fn drawing_at_every_scale_does_not_panic() {
        for (cols, rows, expect) in [(86u16, 30u16, 3u32), (120, 40, 4), (240, 90, 4)] {
            let g = Game::new(Mode::Classic, 2);
            let l = Layout::compute(cols, rows, 4).unwrap();
            assert_eq!(l.scale, expect, "{cols}x{rows}");
            let mut c = Canvas::new(l.canvas_w, l.canvas_h);
            let fx = Fx::new(1);
            draw_arena(&mut c, &g, &l, &Theme::default_theme(), &fx, 0.016, 1.5);
        }
    }

    #[test]
    fn the_gloss_band_moves_with_the_clock() {
        let (g, l, mut a, fx, th) = setup(1);
        draw_arena(&mut a, &g, &l, &th, &fx, 0.016, 0.0);
        let sum_a: f32 = (0..a.width() as i32)
            .map(|x| a.sample(x, l.canvas_h as i32 / 2).iter().sum::<f32>())
            .sum();
        let mut b = Canvas::new(l.canvas_w, l.canvas_h);
        draw_arena(&mut b, &g, &l, &th, &fx, 0.016, 0.9);
        let sum_b: f32 = (0..b.width() as i32)
            .map(|x| b.sample(x, l.canvas_h as i32 / 2).iter().sum::<f32>())
            .sum();
        assert!((sum_a - sum_b).abs() > 1e-4, "the frame is static");
    }
}
