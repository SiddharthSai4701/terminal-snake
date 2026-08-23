//! Frame-budget check for spec §4.6.
//!
//! Acceptance criterion: logic + canvas + quantize under 8 ms at scale 4 with
//! 400 live particles. Gated to release builds, because `opt-level = 1` dev
//! timings are meaningless for f32 pixel loops and would fail spuriously.
//!
//! Run with: `cargo test --release -- --ignored --nocapture`

#[cfg(test)]
mod tests {
    use crate::game::types::Direction;
    use crate::game::{Game, Mode};
    use crate::input::DirQueue;
    use crate::render::arena::draw_arena;
    use crate::render::canvas::{Canvas, ColorTier};
    use crate::render::fx::{Fx, DEATH_PARTICLES};
    use crate::render::layout::Layout;
    use crate::render::theme::Theme;
    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::Rect;
    use std::time::Instant;

    fn time_frames(scale_cols: u16, scale_rows: u16, s_max: u32, with_particles: bool) -> f64 {
        let l = Layout::compute(scale_cols, scale_rows, s_max).unwrap();
        let theme = Theme::default_theme();
        let mut c = Canvas::new(l.canvas_w, l.canvas_h);
        let mut fx = Fx::new(1);
        let mut g = Game::new(Mode::Classic, 1);
        let mut q = DirQueue::new(Direction::Right);
        g.start();
        g.grow_for_bench(60);

        if with_particles {
            fx.emit_death(&[(20.0, 20.0)], theme.body_head);
            assert_eq!(fx.live(), DEATH_PARTICLES);
        }

        let mut buf = Buffer::empty(Rect::new(0, 0, scale_cols, scale_rows));
        let frames = 200;

        // Warm the caches so the first frame's page faults do not dominate.
        for _ in 0..20 {
            draw_arena(&mut c, &g, &l, &theme, &fx, 0.016, 0.0);
            c.quantize_into(
                &mut buf,
                (l.origin_col, l.origin_row),
                ColorTier::Full,
                fx.shake_offset(),
            );
        }

        let start = Instant::now();
        for _ in 0..frames {
            g.advance(0.016, &mut q);
            if with_particles && fx.live() < 50 {
                fx.emit_death(&[(20.0, 20.0)], theme.body_head);
            }
            fx.update(0.016);
            draw_arena(&mut c, &g, &l, &theme, &fx, 0.016, 0.0);
            c.quantize_into(
                &mut buf,
                (l.origin_col, l.origin_row),
                ColorTier::Full,
                fx.shake_offset(),
            );
        }
        start.elapsed().as_secs_f64() * 1000.0 / frames as f64
    }

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "timing is only meaningful in release; run cargo test --release -- --ignored"
    )]
    fn a_worst_case_frame_fits_the_budget_at_scale_four() {
        let ms = time_frames(120, 40, 4, true);
        println!("scale 4, long snake, 400 particles: {ms:.3} ms/frame");
        assert!(ms < 8.0, "{ms:.3} ms per frame exceeds the 8 ms budget");
    }

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "timing is only meaningful in release; run cargo test --release -- --ignored"
    )]
    fn even_the_largest_scale_stays_inside_one_frame() {
        let ms = time_frames(240, 90, 6, true);
        println!("scale 6, long snake, 400 particles: {ms:.3} ms/frame");
        assert!(ms < 16.6, "{ms:.3} ms per frame misses 60fps at scale 6");
    }

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "timing is only meaningful in release; run cargo test --release -- --ignored"
    )]
    fn a_long_snake_costs_about_the_same_as_a_short_one_per_segment() {
        // Guards the per-segment SDF: the naive whole-bbox form is
        // O(length x arena), so doubling the body would more than double the
        // frame cost. Per-segment rasterization keeps it linear and cheap.
        let l = Layout::compute(120, 40, 4).unwrap();
        let theme = Theme::default_theme();
        let fx = Fx::new(1);

        let measure = |extra: u32| -> f64 {
            let mut c = Canvas::new(l.canvas_w, l.canvas_h);
            let mut g = Game::new(Mode::Classic, 1);
            g.start();
            g.grow_for_bench(extra);
            for _ in 0..20 {
                draw_arena(&mut c, &g, &l, &theme, &fx, 0.016, 0.0);
            }
            let start = Instant::now();
            for _ in 0..200 {
                draw_arena(&mut c, &g, &l, &theme, &fx, 0.016, 0.0);
            }
            start.elapsed().as_secs_f64() * 1000.0 / 200.0
        };

        let short = measure(10);
        let long = measure(200);
        println!("length 14: {short:.3} ms | length 204: {long:.3} ms");
        // The fixed per-frame work (clear, border, trail decay, blur) dominates,
        // so a 14x longer body must not be anything like 14x the cost.
        assert!(
            long < short * 4.0,
            "length scaling looks quadratic: {short:.3} -> {long:.3} ms"
        );
    }
}
