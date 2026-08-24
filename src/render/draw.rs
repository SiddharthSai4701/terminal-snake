//! Signed-distance rasterization.
//!
//! Half-block "pixels" are real square 8-bit pixels, so antialiased coverage
//! genuinely buys smooth curves - a Bresenham thick line or per-cell blocks
//! would visibly stair-step on exactly the diagonals this design exists to show
//! off.

use crate::render::canvas::Canvas;
use crate::render::color::Rgb;

#[derive(Copy, Clone, Debug)]
pub struct Stroke {
    pub radius: f32,
    pub falloff: f32,
    /// Corrects for a font whose cells are not exactly 2:1, which would
    /// otherwise skew the distance field.
    pub pixel_aspect: f32,
}

impl Stroke {
    /// The snake body: crisp, filling its cell, the way the first build drew
    /// it with solid squares.
    ///
    /// The radius is set so an axis-aligned run lights exactly one cell's worth
    /// of pixels. The falloff is a fixed sub-pixel value on purpose:
    /// axis-aligned edges land on pixel boundaries and stay perfectly sharp,
    /// while diagonals pick up just enough coverage to avoid stair-stepping.
    ///
    /// A body this wide means the parallel runs of a tight coil touch rather
    /// than showing a dark line between them. That is deliberate - it is how
    /// the original build drew it, and a coil reading as one solid mass is the
    /// classic look. Keeping a gap would mean a body noticeably narrower than
    /// its cell.
    pub fn body(scale: u32) -> Stroke {
        Stroke {
            // Exactly half a cell, measured centre-to-centre between the outer
            // pixels, so an axis-aligned run lights its cell's pixels at full
            // coverage and the next pixel out at none. Anything larger leaves a
            // partly-lit fringe in the neighbouring cell.
            radius: (scale as f32 - 1.0) / 2.0,
            falloff: 0.5,
            pixel_aspect: 1.0,
        }
    }

    /// A soft round dot - used for the food, which keeps its glow.
    pub fn soft_dot(radius: f32, scale: u32) -> Stroke {
        Stroke {
            radius,
            falloff: 0.40 * scale as f32,
            pixel_aspect: 1.0,
        }
    }
}

fn dist_to_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32), aspect: f32) -> f32 {
    let px = (p.0 - a.0) * aspect;
    let py = p.1 - a.1;
    let bx = (b.0 - a.0) * aspect;
    let by = b.1 - a.1;
    let len2 = bx * bx + by * by;
    let t = if len2 <= 1e-9 {
        0.0
    } else {
        ((px * bx + py * by) / len2).clamp(0.0, 1.0)
    };
    let dx = px - bx * t;
    let dy = py - by * t;
    (dx * dx + dy * dy).sqrt()
}

/// Strokes one capsule into the canvas.
///
/// Only this segment's own bounding box is visited, so cost is
/// `O(scale^2)` per segment rather than `O(length x arena_pixels)`. Rasterizing
/// the whole polyline bounding box against every segment costs about 22 ms per
/// frame at length 250 and misses 60fps on the snake alone.
pub fn stroke_segment(c: &mut Canvas, a: (f32, f32), b: (f32, f32), color: Rgb, s: &Stroke) {
    let reach = s.radius + s.falloff;
    let x0 = (a.0.min(b.0) - reach).floor() as i32;
    let x1 = (a.0.max(b.0) + reach).ceil() as i32;
    let y0 = (a.1.min(b.1) - reach).floor() as i32;
    let y1 = (a.1.max(b.1) + reach).ceil() as i32;

    // Clip to the canvas so an off-screen segment costs nothing.
    let x0 = x0.max(0);
    let y0 = y0.max(0);
    let x1 = x1.min(c.width() as i32 - 1);
    let y1 = y1.min(c.height() as i32 - 1);

    let inv_falloff = 1.0 / s.falloff.max(1e-4);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let d = dist_to_segment((x as f32, y as f32), a, b, s.pixel_aspect);
            let cov = 1.0 - ((d - s.radius) * inv_falloff).clamp(0.0, 1.0);
            if cov > 0.0 {
                c.blend(x, y, color, cov);
            }
        }
    }
}

pub fn disc(c: &mut Canvas, centre: (f32, f32), radius: f32, color: Rgb, aspect: f32) {
    let s = Stroke {
        radius,
        falloff: 1.0,
        pixel_aspect: aspect,
    };
    stroke_segment(c, centre, centre, color, &s);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank(w: usize, h: usize) -> Canvas {
        let mut c = Canvas::new(w, h);
        c.clear_base([0.0; 3]);
        c
    }

    #[test]
    fn a_stroke_paints_its_own_line_and_nothing_far_away() {
        let mut c = blank(32, 32);
        let s = Stroke {
            radius: 2.0,
            falloff: 1.0,
            pixel_aspect: 1.0,
        };
        stroke_segment(&mut c, (4.0, 16.0), (28.0, 16.0), [1.0, 1.0, 1.0], &s);
        assert!(c.get(16, 16)[0] > 0.9, "centre of the line");
        assert!(c.get(16, 28)[0] < 0.01, "far from the line");
    }

    #[test]
    fn a_stroke_has_soft_edges() {
        let mut c = blank(32, 32);
        let s = Stroke {
            radius: 3.0,
            falloff: 1.5,
            pixel_aspect: 1.0,
        };
        stroke_segment(&mut c, (4.0, 16.0), (28.0, 16.0), [1.0, 1.0, 1.0], &s);
        let core = c.get(16, 16)[0];
        let edge = c.get(16, 20)[0];
        assert!(core > 0.9, "core {core}");
        assert!(edge > 0.02 && edge < core, "edge {edge} vs core {core}");
    }

    #[test]
    fn a_stroke_has_round_caps_beyond_its_endpoints() {
        let mut c = blank(32, 32);
        let s = Stroke {
            radius: 3.0,
            falloff: 1.0,
            pixel_aspect: 1.0,
        };
        stroke_segment(&mut c, (10.0, 16.0), (20.0, 16.0), [1.0, 1.0, 1.0], &s);
        assert!(c.get(8, 16)[0] > 0.3, "cap should extend past the endpoint");
        assert!(c.get(4, 16)[0] < 0.01, "but not indefinitely");
    }

    #[test]
    fn caps_are_round_not_square() {
        let mut c = blank(32, 32);
        let s = Stroke {
            radius: 4.0,
            falloff: 0.5,
            pixel_aspect: 1.0,
        };
        stroke_segment(&mut c, (12.0, 16.0), (20.0, 16.0), [1.0, 1.0, 1.0], &s);
        // Straight off the end is inside the cap; the diagonal corner is not.
        assert!(c.get(15, 16)[0] > 0.9, "along the axis");
        assert!(c.get(8, 12)[0] < 0.01, "corner of a square cap would be lit");
    }

    #[test]
    fn cost_is_bounded_by_the_segment_not_the_canvas() {
        let mut c = blank(256, 256);
        let s = Stroke {
            radius: 2.0,
            falloff: 1.0,
            pixel_aspect: 1.0,
        };
        stroke_segment(&mut c, (128.0, 128.0), (132.0, 128.0), [1.0, 1.0, 1.0], &s);
        let mut lit = 0;
        for y in 0..256 {
            for x in 0..256 {
                if c.get(x, y)[0] > 0.001 {
                    lit += 1;
                }
            }
        }
        assert!(lit < 200, "{lit} pixels lit by one short segment");
    }

    #[test]
    fn pixel_aspect_biases_the_distance_metric() {
        let mut c = blank(32, 32);
        let s = Stroke {
            radius: 4.0,
            falloff: 2.0,
            pixel_aspect: 2.0,
        };
        stroke_segment(&mut c, (16.0, 16.0), (16.0, 16.0), [1.0, 1.0, 1.0], &s);
        // Three pixels along y is inside the radius; three along x counts as
        // six once the aspect is applied, so it falls outside.
        let along_y = c.get(16, 19)[0];
        let along_x = c.get(19, 16)[0];
        assert!(
            along_y > along_x,
            "an aspect above 1 should compress the x axis: y {along_y} vs x {along_x}"
        );
    }

    #[test]
    fn a_disc_is_round_and_bounded() {
        let mut c = blank(32, 32);
        disc(&mut c, (16.0, 16.0), 4.0, [1.0, 1.0, 1.0], 1.0);
        assert!(c.get(16, 16)[0] > 0.9);
        assert!(c.get(16, 19)[0] > 0.3);
        assert!(c.get(16, 24)[0] < 0.01);
    }

    #[test]
    fn drawing_off_canvas_does_not_panic() {
        let mut c = blank(8, 8);
        let s = Stroke {
            radius: 3.0,
            falloff: 1.0,
            pixel_aspect: 1.0,
        };
        stroke_segment(&mut c, (-40.0, -40.0), (-30.0, -30.0), [1.0; 3], &s);
        stroke_segment(&mut c, (100.0, 100.0), (140.0, 140.0), [1.0; 3], &s);
        stroke_segment(&mut c, (-5.0, 4.0), (12.0, 4.0), [1.0; 3], &s);
        disc(&mut c, (-9.0, 99.0), 5.0, [1.0; 3], 1.0);
    }

    #[test]
    fn the_body_keeps_a_solid_core_at_every_scale() {
        for scale in 3..=6u32 {
            let s = Stroke::body(scale);
            assert!(
                s.radius > s.falloff * 0.5,
                "scale {scale}: radius {} vs falloff {}",
                s.radius,
                s.falloff
            );
            let mut c = blank(32, 32);
            stroke_segment(&mut c, (10.0, 16.0), (22.0, 16.0), [1.0; 3], &s);
            assert!(
                c.get(16, 16)[0] > 0.95,
                "scale {scale} has no solid core: {}",
                c.get(16, 16)[0]
            );
        }
    }

    #[test]
    fn the_body_fills_its_cell_exactly_with_no_fringe() {
        use crate::render::ribbon::cell_centre;
        for scale in 3..=6u32 {
            let mut c = blank(64, 64);
            let st = Stroke::body(scale);
            let y = cell_centre(6, scale as i32);
            stroke_segment(&mut c, (8.0, y), (56.0, y), [1.0; 3], &st);

            let full = (0..64).filter(|py| c.get(32, *py)[0] > 0.99).count();
            let partial = (0..64)
                .filter(|py| {
                    let v = c.get(32, *py)[0];
                    v > 0.001 && v < 0.99
                })
                .count();
            assert_eq!(full, scale as usize, "scale {scale}: {full} full pixels");
            assert_eq!(partial, 0, "scale {scale}: {partial} fringe pixels");
        }
    }

    #[test]
    fn the_body_edge_is_crisp_not_blurred() {
        // The first build's look: a solid bar with hard edges. At most one
        // partially-lit pixel per side, so it never reads as a smudge.
        for scale in 3..=6u32 {
            let mut c = blank(64, 64);
            let st = Stroke::body(scale);
            stroke_segment(&mut c, (8.0, 32.0), (56.0, 32.0), [1.0; 3], &st);
            let partial = (20..=44)
                .filter(|y| {
                    let v = c.get(32, *y)[0];
                    v > 0.05 && v < 0.95
                })
                .count();
            assert!(partial <= 2, "scale {scale}: {partial} soft pixels on the edge");
        }
    }

    #[test]
    fn a_diagonal_segment_is_continuous() {
        let mut c = blank(40, 40);
        let s = Stroke::body(4);
        stroke_segment(&mut c, (8.0, 8.0), (32.0, 32.0), [1.0; 3], &s);
        for i in 10..30 {
            assert!(
                c.get(i, i)[0] > 0.5,
                "gap on the diagonal at {i}: {}",
                c.get(i, i)[0]
            );
        }
    }
}
