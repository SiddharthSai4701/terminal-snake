//! Turns the snake's discrete cells into a smooth pixel-space polyline.
//!
//! Logic stays grid-exact; only the drawing interpolates. The head slides
//! forward into the cell it is about to occupy and the tail slides out of the
//! one it is about to leave, so the body reads as a continuous ribbon in
//! motion rather than a column of jumping squares.

use crate::game::snake::Snake;
use crate::game::types::{GRID_H, GRID_W};

/// Endpoints further apart than this many cells are a wrap seam, not a real
/// segment, and must not be stroked across the arena.
///
/// It has to clear 2.0: a fully extrapolated head sits one cell ahead of its
/// own centre, which is two cells from the next body point. A real wrap seam
/// spans most of the grid, so there is a wide margin between the two.
pub const SEG_BREAK: f32 = 3.0;

pub fn cell_centre(c: i32, scale: i32) -> f32 {
    1.0 + c as f32 * scale as f32 + scale as f32 / 2.0
}

/// Head-first polyline in canvas pixel coordinates.
///
/// `t` is the fraction of the way to the next tick, from `Game::tick_fraction`.
pub fn snake_path(snake: &Snake, t: f32, scale: i32, wrap: bool) -> Vec<(f32, f32)> {
    let t = t.clamp(0.0, 1.0);
    let s = scale as f32;
    let cells: Vec<_> = snake.iter().copied().collect();
    let n = cells.len();

    let mut pts: Vec<(f32, f32)> = cells
        .iter()
        .map(|p| (cell_centre(p.x, scale), cell_centre(p.y, scale)))
        .collect();

    // The head advances along the direction that is actually applied, which is
    // the direction the next tick will use, so the visual matches the logic.
    let (dx, dy) = snake.dir().delta();
    pts[0].0 += dx as f32 * t * s;
    pts[0].1 += dy as f32 * t * s;

    if !wrap {
        // Without wrapping the head must not poke through the border on the
        // frame before a wall death.
        let min = 1.0 + s / 2.0;
        pts[0].0 = pts[0].0.clamp(min, 1.0 + GRID_W as f32 * s - s / 2.0);
        pts[0].1 = pts[0].1.clamp(min, 1.0 + GRID_H as f32 * s - s / 2.0);
    }

    // The tail is pulled toward its predecessor unless the snake is growing,
    // in which case it stays put and the body lengthens.
    if n >= 2 && !snake.is_growing() {
        let (tx, ty) = pts[n - 1];
        let (px, py) = pts[n - 2];
        let seam = (tx - px).abs() > SEG_BREAK * s || (ty - py).abs() > SEG_BREAK * s;
        if !seam {
            pts[n - 1] = (tx + (px - tx) * t, ty + (py - ty) * t);
        }
    }

    pts
}

/// True when a pair of consecutive path points is a wrap seam rather than a
/// real body segment.
pub fn is_seam(a: (f32, f32), b: (f32, f32), scale: i32) -> bool {
    let limit = SEG_BREAK * scale as f32;
    (a.0 - b.0).abs() > limit || (a.1 - b.1).abs() > limit
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::types::{Direction, Pos};

    #[test]
    fn the_path_starts_at_the_head_and_has_one_point_per_segment() {
        let s = Snake::new(Pos::new(9, 9), 4, Direction::Right);
        let p = snake_path(&s, 0.0, 4, false);
        assert_eq!(p.len(), 4);
        assert!(
            (p[0].0 - (1.0 + 9.0 * 4.0 + 2.0)).abs() < 1e-3,
            "head should sit at its cell centre, got {:?}",
            p[0]
        );
    }

    #[test]
    fn the_head_slides_forward_with_the_tick_fraction() {
        let s = Snake::new(Pos::new(9, 9), 4, Direction::Right);
        let a = snake_path(&s, 0.0, 4, false)[0];
        let b = snake_path(&s, 0.5, 4, false)[0];
        assert!((b.0 - a.0 - 2.0).abs() < 1e-3, "half a cell at scale 4");
        assert!((b.1 - a.1).abs() < 1e-6, "no sideways drift");
    }

    #[test]
    fn the_head_slides_the_way_it_is_facing() {
        let s = Snake::new(Pos::new(9, 9), 4, Direction::Up);
        let a = snake_path(&s, 0.0, 4, false)[0];
        let b = snake_path(&s, 1.0, 4, false)[0];
        assert!((b.1 - a.1 + 4.0).abs() < 1e-3, "should rise one cell");
        assert!((b.0 - a.0).abs() < 1e-6);
    }

    #[test]
    fn the_tail_retracts_with_the_tick_fraction() {
        let s = Snake::new(Pos::new(9, 9), 4, Direction::Right);
        let a = *snake_path(&s, 0.0, 4, false).last().unwrap();
        let b = *snake_path(&s, 0.5, 4, false).last().unwrap();
        assert!(b.0 > a.0, "tail should be pulled in: {a:?} -> {b:?}");
        assert!((b.0 - a.0 - 2.0).abs() < 1e-3);
    }

    #[test]
    fn a_growing_tail_stays_put() {
        let mut s = Snake::new(Pos::new(9, 9), 4, Direction::Right);
        s.grow(1);
        let a = *snake_path(&s, 0.0, 4, false).last().unwrap();
        let b = *snake_path(&s, 0.9, 4, false).last().unwrap();
        assert!((a.0 - b.0).abs() < 1e-6, "growing tail must not retract");
    }

    #[test]
    fn the_head_never_leaves_the_arena_in_non_wrapping_modes() {
        let s = Snake::new(Pos::new(GRID_W - 1, 9), 3, Direction::Right);
        let p = snake_path(&s, 1.0, 4, false);
        let max_x = 1.0 + GRID_W as f32 * 4.0 - 2.0;
        assert!(p[0].0 <= max_x + 1e-3, "head at {} exceeded {}", p[0].0, max_x);
    }

    #[test]
    fn a_single_segment_snake_still_produces_a_path() {
        let s = Snake::new(Pos::new(5, 5), 1, Direction::Left);
        let p = snake_path(&s, 0.5, 4, false);
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn wrap_seams_are_detectable() {
        let mut s = Snake::new(Pos::new(GRID_W - 1, 5), 3, Direction::Right);
        assert_eq!(
            s.step(Direction::Right, true),
            crate::game::snake::StepOutcome::Moved
        );
        let p = snake_path(&s, 0.0, 4, true);
        assert!(
            p.windows(2).any(|w| is_seam(w[0], w[1], 4)),
            "a wrapped body should expose a seam: {p:?}"
        );
    }

    #[test]
    fn an_unwrapped_body_has_no_seams() {
        let s = Snake::new(Pos::new(9, 9), 6, Direction::Right);
        let p = snake_path(&s, 0.4, 4, false);
        assert!(!p.windows(2).any(|w| is_seam(w[0], w[1], 4)));
    }

    #[test]
    fn consecutive_points_are_close_enough_to_stroke_continuously() {
        let s = Snake::new(Pos::new(9, 9), 8, Direction::Right);
        for step in 0..=10 {
            let t = step as f32 / 10.0;
            let p = snake_path(&s, t, 4, false);
            for w in p.windows(2) {
                let d = ((w[0].0 - w[1].0).powi(2) + (w[0].1 - w[1].1).powi(2)).sqrt();
                assert!(d < 4.0 * SEG_BREAK, "t={t} gap of {d} would read as a seam");
            }
        }
    }
}
