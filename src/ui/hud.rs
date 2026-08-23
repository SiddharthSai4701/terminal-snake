use crate::game::{Game, GameState};
use ratatui_core::buffer::Buffer;
use ratatui_core::style::{Color, Modifier, Style};

const LABEL: Color = Color::Rgb(0x6b, 0x7a, 0x8c);
const VALUE: Color = Color::Rgb(0x8a, 0xff, 0xc1);
const ALERT: Color = Color::Rgb(0xff, 0x9f, 0x43);

pub fn render_hud(buf: &mut Buffer, x: u16, y: u16, width: u16, game: &Game) {
    let label = Style::new().fg(LABEL);
    let value = Style::new().fg(VALUE).add_modifier(Modifier::BOLD);

    let mut cx = x;
    let mut put = |text: &str, style: Style, cx: &mut u16| {
        if *cx >= x + width {
            return;
        }
        let (nx, _) = buf.set_stringn(*cx, y, text, (x + width - *cx) as usize, style);
        *cx = nx;
    };

    put("SCORE ", label, &mut cx);
    put(&format!("{:<7}", game.score), value, &mut cx);
    put("LEN ", label, &mut cx);
    put(&format!("{:<6}", game.snake().len()), value, &mut cx);
    put("TIME ", label, &mut cx);
    put(&format!("{:<8.1}", game.elapsed), value, &mut cx);
    put("SPEED ", label, &mut cx);
    let speed = (140.0 / game.tick_ms()).max(1.0);
    put(
        &format!("{speed:.2}x"),
        if speed > 1.5 {
            Style::new().fg(ALERT).add_modifier(Modifier::BOLD)
        } else {
            value
        },
        &mut cx,
    );
}

pub fn hint_for(state: GameState) -> &'static str {
    match state {
        GameState::AwaitingStart => "press an arrow key or WASD to start",
        GameState::Running => "arrows / wasd  -  r restart  -  q quit",
        GameState::Dead => "you died  -  r to play again  -  q quit",
        GameState::Won => "you filled the board  -  r to play again  -  q quit",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Mode;
    use ratatui_core::layout::Rect;

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..buf.area().width)
            .map(|x| buf[(x, y)].symbol().to_string())
            .collect()
    }

    #[test]
    fn the_hud_shows_score_length_time_and_speed() {
        let g = Game::new(Mode::Classic, 1);
        let mut b = Buffer::empty(Rect::new(0, 0, 80, 1));
        render_hud(&mut b, 0, 0, 80, &g);
        let text = row_text(&b, 0);
        assert!(text.contains("SCORE"), "{text}");
        assert!(text.contains("LEN"), "{text}");
        assert!(text.contains("TIME"), "{text}");
        assert!(text.contains("SPEED"), "{text}");
        assert!(text.contains('4'), "length 4 should be shown: {text}");
    }

    #[test]
    fn the_hud_never_writes_past_its_width() {
        let g = Game::new(Mode::Classic, 1);
        let mut b = Buffer::empty(Rect::new(0, 0, 40, 1));
        render_hud(&mut b, 0, 0, 12, &g);
        let text = row_text(&b, 0);
        assert!(
            text[12..].chars().all(|c| c == ' '),
            "wrote past the limit: {text}"
        );
    }

    #[test]
    fn every_state_has_a_hint() {
        for s in [
            GameState::AwaitingStart,
            GameState::Running,
            GameState::Dead,
            GameState::Won,
        ] {
            assert!(!hint_for(s).is_empty());
        }
    }
}
