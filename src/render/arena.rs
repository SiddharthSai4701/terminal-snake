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
/// Steady-state trail brightness under a stationary head, as a multiple of the
/// theme's glow tint.
const TRAIL_STRENGTH: f32 = 0.9;
/// Radius of the death flash as a fraction of the canvas width.
const FLASH_SIGMA: f32 = 0.16;
/// How much of the flash reaches the far corners. Small on purpose: a uniform
/// additive flash lifts pure black to mid grey and hides the whole arena.
const FLASH_AMBIENT: f32 = 0.06;

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
    let pulse = 0.85 + 0.15 * (clock * 3.4).sin();
    let fx_pos = (
        cell_centre(game.food().x, s),
        cell_centre(game.food().y, s),
    );
    // Radius and softness both in cell units, so the dot reads the same at any
    // scale instead of turning into a hard square at small ones.
    // The food keeps its glow - it is the thing you are looking for.
    stroke_segment(
        c,
        fx_pos,
        fx_pos,
        theme.food,
        &Stroke::soft_dot(0.20 * s as f32 * pulse, layout.scale),
    );
    c.add_glow(
        fx_pos.0,
        fx_pos.1,
        scale_rgb(theme.food, 0.34 * pulse),
    );

    // The body: one capsule per segment, each with its own ramp colour, so the
    // cost is O(length x scale^2) rather than O(length x arena).
    let stroke = Stroke::body(layout.scale);
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

        // The snake itself carries no glow and lays no trail: crisp body,
        // the way the first build drew it. Glow is reserved for the food and
        // for dying, so it means something when it appears.
        let head = path[0];
        stroke_segment(c, head, head, theme.body_head, &stroke);
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
        let (cx, cy) = fx.flash_at();
        let sigma = c.width() as f32 * FLASH_SIGMA;
        let inv = 1.0 / (2.0 * sigma * sigma);
        for y in 0..h {
            for x in 0..w {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let radial = (-(dx * dx + dy * dy) * inv).exp();
                let k = flash * (FLASH_AMBIENT + (1.0 - FLASH_AMBIENT) * radial);
                let base = c.get(x, y);
                c.set(x, y, add_rgb(base, [k, k, k]));
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

    #[test]
    fn the_snake_lays_no_trail_and_carries_no_glow() {
        // Glow is reserved for the food and for dying. A cell the snake has
        // left must return to the backdrop, and the space just outside the
        // body must not be lit by a halo.
        let (mut g, l, mut c, fx, th) = setup(3);
        let mut q = DirQueue::new(Direction::Right);
        g.start();
        let start = g.snake().head();
        draw_arena(&mut c, &g, &l, &th, &fx, 0.016, 0.0);
        while g.snake().contains(start) {
            g.advance(0.14, &mut q);
            draw_arena(&mut c, &g, &l, &th, &fx, 0.016, 0.0);
        }

        let (bx, by) = cell_px(&l, 2, 15);
        let backdrop = c.sample(bx, by).iter().sum::<f32>();
        let (x, y) = cell_px(&l, start.x, start.y);
        assert!(
            (c.sample(x, y).iter().sum::<f32>() - backdrop).abs() < 1e-3,
            "a vacated cell still glows"
        );

        let head = g.snake().head();
        let (hx, hy) = cell_px(&l, head.x, head.y);
        let beside = c.sample(hx, hy + l.scale as i32).iter().sum::<f32>();
        assert!(
            (beside - backdrop).abs() < 0.02,
            "there is a halo beside the body: {beside} against {backdrop}"
        );
    }

    #[test]
    fn the_food_still_glows() {
        let (g, l, mut c, fx, th) = setup(9);
        draw_arena(&mut c, &g, &l, &th, &fx, 0.016, 0.0);
        let (fx_x, fy) = cell_px(&l, g.food().x, g.food().y);
        let (bx, by) = cell_px(&l, 2, 15);
        let backdrop = c.sample(bx, by).iter().sum::<f32>();
        // One cell away from the food there should still be light.
        let halo = c.sample(fx_x + l.scale as i32, fy).iter().sum::<f32>();
        assert!(
            halo > backdrop + 0.01,
            "the food lost its glow: {halo} against {backdrop}"
        );
    }

    #[test]
    fn a_flash_is_centred_on_the_death_rather_than_washing_the_screen() {
        let (g, l, mut c, mut fx, th) = setup(1);
        let centre = (
            cell_centre(g.snake().head().x, l.scale as i32),
            cell_centre(g.snake().head().y, l.scale as i32),
        );
        draw_arena(&mut c, &g, &l, &th, &fx, 0.016, 0.0);
        let far_before = c.sample(l.canvas_w as i32 - 4, 4).iter().sum::<f32>();

        fx.emit_death(&[centre], th.body_head);
        draw_arena(&mut c, &g, &l, &th, &fx, 0.016, 0.0);

        let near = c.sample(centre.0 as i32, centre.1 as i32).iter().sum::<f32>();
        let far = c.sample(l.canvas_w as i32 - 4, 4).iter().sum::<f32>();
        assert!(near > 1.5, "the flash should be bright at the impact: {near}");
        assert!(
            far < far_before + 0.35,
            "the far corner washed out to {far} from {far_before}"
        );
    }

    #[test]
    fn a_flash_is_over_quickly() {
        let (g, l, mut c, mut fx, th) = setup(1);
        fx.emit_death(&[(20.0, 20.0)], th.body_head);
        // A quarter of a second later it must be gone, not a lingering grey veil.
        for _ in 0..15 {
            fx.update(1.0 / 60.0);
        }
        draw_arena(&mut c, &g, &l, &th, &fx, 0.016, 0.0);
        let corner = c.sample(l.canvas_w as i32 - 4, 4).iter().sum::<f32>();
        assert!(corner < 0.2, "still washed out after 250ms: {corner}");
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
        // Sample the row the body actually occupies, not the middle of the
        // canvas - the ribbon is narrower than a cell.
        let row = cell_centre(g.snake().head().y, l.scale as i32) as i32;
        let brightness = |c: &Canvas| -> f32 {
            (0..c.width() as i32)
                .map(|x| c.sample(x, row).iter().sum::<f32>())
                .sum()
        };
        draw_arena(&mut a, &g, &l, &th, &fx, 0.016, 0.0);
        let at_zero = brightness(&a);
        let mut b = Canvas::new(l.canvas_w, l.canvas_h);
        draw_arena(&mut b, &g, &l, &th, &fx, 0.016, 0.9);
        let later = brightness(&b);
        assert!(at_zero > 0.1, "the sampled row missed the body entirely");
        assert!(
            (at_zero - later).abs() > 1e-4,
            "the frame is static: {at_zero} vs {later}"
        );
    }
}
