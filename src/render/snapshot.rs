//! Renders a frame to a PPM file so the output can be inspected as an image
//! instead of guessed at from escape codes.
//!
//! Run with: `cargo test --release snapshot -- --nocapture`

#[cfg(test)]
mod tests {
    use crate::game::types::Direction;
    use crate::game::{Game, Mode};
    use crate::input::DirQueue;
    use crate::render::arena::draw_arena;
    use crate::render::canvas::{Canvas, ColorTier};
    use crate::render::color::{srgb_encode, to_u8, tone_map};
    #[allow(unused_imports)]
    use crate::render::fx::DEATH_PARTICLES;
    use crate::render::fx::Fx;
    use crate::render::layout::Layout;
    use crate::render::theme::Theme;
    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::Rect;
    use ratatui_core::style::Color;
    use std::io::Write;

    /// One canvas pixel becomes ZOOM x ZOOM image pixels, so individual pixels
    /// are visible.
    const ZOOM: usize = 6;

    fn rgb_of(c: Color) -> [u8; 3] {
        match c {
            Color::Rgb(r, g, b) => [r, g, b],
            _ => [0, 0, 0],
        }
    }

    /// Reconstructs exactly what the terminal shows: each cell contributes its
    /// foreground colour to the upper pixel and its background to the lower.
    fn write_ppm(path: &str, buf: &Buffer, area: Rect) {
        let w = area.width as usize;
        let h = area.height as usize * 2;
        let mut img = vec![[0u8; 3]; w * h];

        for row in 0..area.height {
            for col in 0..area.width {
                let cell = &buf[(col, row)];
                let (top, bot) = if cell.symbol() == "\u{2580}" {
                    (rgb_of(cell.fg), rgb_of(cell.bg))
                } else {
                    (rgb_of(cell.bg), rgb_of(cell.bg))
                };
                img[(row as usize * 2) * w + col as usize] = top;
                img[(row as usize * 2 + 1) * w + col as usize] = bot;
            }
        }

        let mut out = Vec::with_capacity(w * h * 3 * ZOOM * ZOOM + 32);
        out.extend_from_slice(format!("P6\n{} {}\n255\n", w * ZOOM, h * ZOOM).as_bytes());
        for y in 0..h * ZOOM {
            for x in 0..w * ZOOM {
                let p = img[(y / ZOOM) * w + (x / ZOOM)];
                out.extend_from_slice(&p);
            }
        }
        std::fs::File::create(path)
            .unwrap()
            .write_all(&out)
            .unwrap();
        println!("wrote {path} ({}x{})", w * ZOOM, h * ZOOM);
    }

    fn snapshot(name: &str, food_runs: usize, kill: bool) {
        let cols = 120u16;
        let rows = 40u16;
        let l = Layout::compute(cols, rows, 4).unwrap();
        let theme = Theme::default_theme();
        let mut c = Canvas::new(l.canvas_w, l.canvas_h);
        let mut fx = Fx::new(7);
        let mut g = Game::new(Mode::Classic, 5);
        let mut q = DirQueue::new(Direction::Right);
        g.start();

        let dt = 1.0 / 60.0;
        let mut clock = 0.0f32;
        let turns = [
            Direction::Right,
            Direction::Down,
            Direction::Right,
            Direction::Up,
        ];

        // Grow the snake by feeding it, so the ribbon has real corners in it.
        for i in 0..food_runs {
            let head = g.snake().head();
            let (dx, dy) = turns[i % turns.len()].delta();
            g.force_food_at(crate::game::Pos::new(
                (head.x + dx * 2).clamp(1, 26),
                (head.y + dy * 2).clamp(1, 16),
            ));
            q.push(turns[i % turns.len()]);
            for _ in 0..14 {
                g.advance(dt, &mut q);
                fx.update(dt);
                clock += dt;
                draw_arena(&mut c, &g, &l, &theme, &fx, dt, clock);
            }
        }

        if kill {
            let path: Vec<(f32, f32)> = crate::render::ribbon::snake_path(
                g.snake(),
                0.0,
                l.scale as i32,
                false,
            );
            fx.emit_death(&path, theme.body_head);
            for _ in 0..6 {
                fx.update(dt);
                clock += dt;
                draw_arena(&mut c, &g, &l, &theme, &fx, dt, clock);
            }
        }

        let mut buf = Buffer::empty(Rect::new(0, 0, cols, rows));
        c.quantize_into(
            &mut buf,
            (l.origin_col, l.origin_row),
            ColorTier::Full,
            fx.shake_offset(),
        );
        let area = Rect::new(
            l.origin_col,
            l.origin_row,
            l.canvas_w as u16,
            l.canvas_rows(),
        );
        write_ppm(name, &buf, area);
    }

    #[test]
    #[cfg_attr(debug_assertions, ignore = "run in release: cargo test --release snapshot")]
    fn snapshot_play() {
        snapshot("target/frame_play.ppm", 6, false);
    }

    #[test]
    #[cfg_attr(debug_assertions, ignore = "run in release: cargo test --release snapshot")]
    fn snapshot_death() {
        snapshot("target/frame_death.ppm", 4, true);
    }

    fn snap_bits(v: f32, bits: u32) -> u8 {
        let max = (1u32 << bits) - 1;
        let q = (v.clamp(0.0, 1.0) * max as f32 + 0.5) as u32;
        ((q * 255) / max) as u8
    }

    #[test]
    #[cfg_attr(debug_assertions, ignore = "run in release: cargo test --release snapshot")]
    fn report_what_the_colour_snap_actually_buys() {
        // Renders a moving scene and counts how many terminal cells change
        // between consecutive frames at each quantization depth. That dirty
        // count is what drives the bytes written per frame.
        let l = Layout::compute(120, 40, 4).unwrap();
        let theme = Theme::default_theme();

        for bits in [5u32, 6, 8] {
            let mut c = Canvas::new(l.canvas_w, l.canvas_h);
            let mut fx = Fx::new(7);
            let mut g = Game::new(Mode::Classic, 5);
            let mut q = DirQueue::new(Direction::Right);
            g.start();
            g.grow_for_bench(40);
            fx.emit_death(&[(40.0, 30.0)], theme.body_head);

            let dt = 1.0 / 60.0;
            let mut clock = 0.0f32;
            let mut prev: Vec<[u8; 3]> = Vec::new();
            let mut dirty_total = 0usize;
            let mut frames = 0usize;

            for _ in 0..90 {
                g.advance(dt, &mut q);
                fx.update(dt);
                clock += dt;
                draw_arena(&mut c, &g, &l, &theme, &fx, dt, clock);

                let mut cur = Vec::with_capacity(l.canvas_w * l.canvas_rows() as usize * 2);
                for cy in 0..l.canvas_rows() as i32 {
                    for cx in 0..l.canvas_w as i32 {
                        for half in 0..2 {
                            let p = c.sample(cx, cy * 2 + half);
                            cur.push([
                                snap_bits(srgb_encode(tone_map(p[0])), bits),
                                snap_bits(srgb_encode(tone_map(p[1])), bits),
                                snap_bits(srgb_encode(tone_map(p[2])), bits),
                            ]);
                        }
                    }
                }
                if !prev.is_empty() {
                    dirty_total += prev
                        .iter()
                        .zip(cur.iter())
                        .filter(|(a, b)| a != b)
                        .count();
                    frames += 1;
                }
                prev = cur;
            }

            let total_px = l.canvas_w * l.canvas_rows() as usize * 2;
            let avg = dirty_total as f64 / frames as f64;
            println!(
                "{bits}-bit: step {:>3}, {avg:.0}/{total_px} pixels change per frame ({:.1}%)",
                255 / ((1 << bits) - 1),
                100.0 * avg / total_px as f64
            );
        }
    }

    #[test]
    fn the_encoder_agrees_with_the_quantizer() {
        // Guards the snapshot pipeline itself: a known linear value must reach
        // the same byte the real quantizer would emit.
        assert_eq!(to_u8(srgb_encode(tone_map(1.0))), 255);
        assert_eq!(to_u8(srgb_encode(tone_map(0.0))), 0);
    }
}
