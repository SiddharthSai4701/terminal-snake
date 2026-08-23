use crate::game::types::{GRID_H, GRID_W};

pub const MIN_COLS: u16 = 86;
pub const MIN_ROWS: u16 = 30;
/// 1 HUD row + 1 hint row. Kept at 2 so the 120x30 Windows Terminal
/// default is playable out of the box.
pub const CHROME_ROWS: u16 = 2;
/// One border pixel on each side.
pub const BORDER_PX: usize = 2;
pub const MIN_SCALE: u32 = 3;
/// Capped at 4 by default: scale 6 pushes ~21 MB/s of per-cell truecolour SGR,
/// which is at or past what Windows Terminal sustains.
pub const DEFAULT_MAX_SCALE: u32 = 4;
pub const MAX_SCALE: u32 = 6;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    pub scale: u32,
    pub canvas_w: usize,
    pub canvas_h: usize,
    pub origin_col: u16,
    pub origin_row: u16,
}

impl Layout {
    /// Integer pixel scale for a terminal of `cols` x `rows`, or `None` when
    /// the terminal is below the playable minimum.
    ///
    /// The logic grid never changes size, so a bigger terminal renders crisper
    /// but never easier.
    pub fn compute(cols: u16, rows: u16, s_max: u32) -> Option<Layout> {
        if cols < MIN_COLS || rows < MIN_ROWS {
            return None;
        }

        let avail_px_w = (cols as usize).saturating_sub(BORDER_PX);
        let avail_px_h = ((rows - CHROME_ROWS) as usize * 2).saturating_sub(BORDER_PX);

        let by_w = avail_px_w / GRID_W as usize;
        let by_h = avail_px_h / GRID_H as usize;
        let s_max = s_max.clamp(MIN_SCALE, MAX_SCALE);
        let scale = (by_w.min(by_h) as u32).clamp(MIN_SCALE, s_max);

        let canvas_w = GRID_W as usize * scale as usize + BORDER_PX;
        let canvas_h = GRID_H as usize * scale as usize + BORDER_PX;

        let used_rows = canvas_h.div_ceil(2) as u16 + CHROME_ROWS;
        if canvas_w > cols as usize || used_rows > rows {
            return None;
        }

        let origin_col = (cols - canvas_w as u16) / 2;
        let origin_row = (CHROME_ROWS - 1) + (rows - used_rows) / 2;

        Some(Layout {
            scale,
            canvas_w,
            canvas_h,
            origin_col,
            origin_row,
        })
    }

    pub fn canvas_rows(&self) -> u16 {
        self.canvas_h.div_ceil(2) as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn below_the_minimum_there_is_no_layout() {
        assert!(Layout::compute(85, 31, 4).is_none());
        assert!(Layout::compute(86, 29, 4).is_none());
        assert!(Layout::compute(20, 10, 4).is_none());
        assert!(Layout::compute(0, 0, 4).is_none());
    }

    #[test]
    fn the_documented_minimum_yields_scale_three() {
        let l = Layout::compute(86, 30, 4).unwrap();
        assert_eq!(l.scale, 3);
        assert_eq!(l.canvas_w, 28 * 3 + 2);
        assert_eq!(l.canvas_h, 18 * 3 + 2);
    }

    #[test]
    fn the_canvas_always_fits_the_terminal() {
        for cols in MIN_COLS..240u16 {
            for rows in MIN_ROWS..90u16 {
                let l = Layout::compute(cols, rows, MAX_SCALE)
                    .unwrap_or_else(|| panic!("no layout for {cols}x{rows}"));
                assert!(l.canvas_w <= cols as usize, "{cols}x{rows} overflowed width");
                let used = l.canvas_rows() + CHROME_ROWS;
                assert!(used <= rows, "{cols}x{rows} overflowed height");
                assert!(l.origin_col as usize + l.canvas_w <= cols as usize);
                assert!(
                    l.origin_row + l.canvas_rows() < rows,
                    "no room for the hint row at {cols}x{rows}"
                );
            }
        }
    }

    #[test]
    fn scale_is_capped_by_the_argument() {
        assert_eq!(Layout::compute(400, 200, 4).unwrap().scale, 4);
        assert_eq!(Layout::compute(400, 200, 6).unwrap().scale, 6);
        assert_eq!(Layout::compute(400, 200, 3).unwrap().scale, 3);
    }

    #[test]
    fn scale_never_drops_below_the_minimum_or_exceeds_the_maximum() {
        assert_eq!(Layout::compute(86, 30, 1).unwrap().scale, MIN_SCALE);
        assert_eq!(Layout::compute(999, 400, 99).unwrap().scale, MAX_SCALE);
    }

    #[test]
    fn the_arena_is_centred() {
        let l = Layout::compute(200, 80, 4).unwrap();
        assert_eq!(l.origin_col, (200 - l.canvas_w as u16) / 2);
    }
}
