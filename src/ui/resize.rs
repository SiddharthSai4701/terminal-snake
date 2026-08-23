use crate::render::layout::{MIN_COLS, MIN_ROWS};
use ratatui_core::buffer::Buffer;
use ratatui_core::style::{Color, Modifier, Style};

/// Shown instead of a squished arena when the terminal is below the playable
/// minimum. The logic grid is fixed so scores stay comparable, which means
/// there is nothing to shrink.
pub fn render_resize(buf: &mut Buffer, cols: u16, rows: u16) {
    let warn = Style::new()
        .fg(Color::Rgb(0xff, 0x9f, 0x43))
        .add_modifier(Modifier::BOLD);
    let dim = Style::new().fg(Color::Rgb(0x6b, 0x7a, 0x8c));

    let mid = rows / 2;
    let lines = [
        (format!("terminal too small: {cols}x{rows}"), warn),
        (format!("resize to at least {MIN_COLS}x{MIN_ROWS}"), dim),
    ];

    for (i, (text, style)) in lines.iter().enumerate() {
        let y = mid.saturating_sub(1) + i as u16;
        if y >= rows {
            continue;
        }
        let x = cols.saturating_sub(text.len() as u16) / 2;
        buf.set_stringn(x, y, text, cols as usize, *style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::layout::Rect;

    fn text_of(buf: &Buffer) -> String {
        let a = *buf.area();
        (0..a.height)
            .map(|y| {
                (0..a.width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn it_names_the_current_and_required_size() {
        let mut b = Buffer::empty(Rect::new(0, 0, 40, 10));
        render_resize(&mut b, 40, 10);
        let t = text_of(&b);
        assert!(t.contains("40x10"), "{t}");
        assert!(t.contains("86x30"), "{t}");
    }

    #[test]
    fn a_tiny_terminal_does_not_panic() {
        for (w, h) in [(1u16, 1u16), (2, 1), (10, 2), (5, 3)] {
            let mut b = Buffer::empty(Rect::new(0, 0, w, h));
            render_resize(&mut b, w, h);
        }
    }
}
