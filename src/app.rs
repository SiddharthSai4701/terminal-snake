//! Pure application state machine.
//!
//! No loop, no terminal, no clock, no I/O - time arrives as `dt` and input as
//! already-mapped `Action`s. That is what lets the planned browser build, where
//! `requestAnimationFrame` inverts control, reuse this file untouched.

use crate::game::types::Direction;
use crate::game::{Game, GameState, Mode};
use crate::input::{Action, DirQueue};
use crate::render::arena::draw_arena;
use crate::render::canvas::{Canvas, ColorTier};
use crate::render::layout::Layout;
use crate::render::tier::Tier;
use crate::ui::hud::{hint_for, render_hud};
use crate::ui::resize::render_resize;
use ratatui_core::buffer::Buffer;
use ratatui_core::style::{Color, Style};

pub struct App {
    seed: u64,
    s_max: u32,
    game: Game,
    queue: DirQueue,
    canvas: Canvas,
    layout: Option<Layout>,
    tier: Tier,
    quit: bool,
}

impl App {
    pub fn new(seed: u64, s_max: u32) -> Self {
        App {
            seed,
            s_max,
            game: Game::new(Mode::Classic, seed),
            queue: DirQueue::new(Direction::Right),
            canvas: Canvas::new(1, 2),
            layout: None,
            tier: Tier::Full,
            quit: false,
        }
    }

    #[allow(dead_code)] // read by tests now, by the HUD and summary screens in later phases
    pub fn game(&self) -> &Game {
        &self.game
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    pub fn set_tier(&mut self, t: Tier) {
        self.tier = t;
    }

    fn restart(&mut self) {
        // Advance the seed so a replay is a new run rather than the same food
        // sequence again.
        self.seed = self
            .seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.game = Game::new(Mode::Classic, self.seed);
        self.queue = DirQueue::new(Direction::Right);
    }

    pub fn update(&mut self, dt: f32, input: &[Action]) {
        for a in input {
            match a {
                Action::Quit => self.quit = true,
                Action::Restart => {
                    if matches!(self.game.state, GameState::Dead | GameState::Won) {
                        self.restart();
                    }
                }
                Action::Turn(d) => {
                    self.game.start();
                    self.queue.push(*d);
                }
                Action::Start => self.game.start(),
                Action::Pause => {}
            }
        }
        self.game.advance(dt, &mut self.queue);
    }

    pub fn render(&mut self, buf: &mut Buffer) {
        let area = *buf.area();
        let Some(layout) = Layout::compute(area.width, area.height, self.s_max) else {
            render_resize(buf, area.width, area.height);
            self.layout = None;
            return;
        };

        if self.layout != Some(layout) {
            self.canvas = Canvas::new(layout.canvas_w, layout.canvas_h);
            self.layout = Some(layout);
        }

        draw_arena(&mut self.canvas, &self.game, &layout);
        let tier = match self.tier {
            Tier::Reduced => ColorTier::Reduced,
            _ => ColorTier::Full,
        };
        self.canvas
            .quantize_into(buf, (layout.origin_col, layout.origin_row), tier, (0.0, 0.0));

        render_hud(
            buf,
            layout.origin_col,
            0,
            layout.canvas_w as u16,
            &self.game,
        );

        let hint_row = layout.origin_row + layout.canvas_rows();
        if hint_row < area.height {
            buf.set_stringn(
                layout.origin_col,
                hint_row,
                hint_for(self.game.state),
                layout.canvas_w,
                Style::new().fg(Color::Rgb(0x6b, 0x7a, 0x8c)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::layout::Rect;

    fn buf(w: u16, h: u16) -> Buffer {
        Buffer::empty(Rect::new(0, 0, w, h))
    }

    #[test]
    fn a_turn_press_starts_a_waiting_game() {
        let mut a = App::new(1, 4);
        assert_eq!(a.game().state, GameState::AwaitingStart);
        a.update(0.016, &[Action::Turn(Direction::Up)]);
        assert_eq!(a.game().state, GameState::Running);
    }

    #[test]
    fn quit_is_recorded() {
        let mut a = App::new(1, 4);
        assert!(!a.should_quit());
        a.update(0.016, &[Action::Quit]);
        assert!(a.should_quit());
    }

    #[test]
    fn restart_is_ignored_while_the_run_is_alive() {
        let mut a = App::new(1, 4);
        a.update(0.0, &[Action::Turn(Direction::Right)]);
        a.update(0.14, &[]);
        let head = a.game().snake().head();
        a.update(0.0, &[Action::Restart]);
        assert_eq!(a.game().snake().head(), head);
    }

    #[test]
    fn restart_after_death_produces_a_fresh_run() {
        let mut a = App::new(1, 4);
        a.update(0.0, &[Action::Turn(Direction::Right)]);
        for _ in 0..200 {
            a.update(0.05, &[]);
        }
        assert_eq!(a.game().state, GameState::Dead);
        a.update(0.0, &[Action::Restart]);
        assert_eq!(a.game().state, GameState::AwaitingStart);
        assert_eq!(a.game().score, 0);
        assert_eq!(a.game().snake().len(), 4);
    }

    #[test]
    fn rendering_paints_the_arena_and_the_hud() {
        let mut a = App::new(1, 4);
        let mut b = buf(120, 40);
        a.render(&mut b);
        let painted = b.content().iter().filter(|c| c.symbol() != " ").count();
        assert!(painted > 0, "nothing was painted");
        let hud: String = (0..120).map(|x| b[(x, 0)].symbol().to_string()).collect();
        assert!(hud.contains("SCORE"), "{hud}");
    }

    #[test]
    fn a_small_buffer_renders_the_resize_screen_instead_of_panicking() {
        let mut a = App::new(1, 4);
        let mut b = buf(40, 10);
        a.render(&mut b);
        let text: String = (0..10)
            .flat_map(|y| (0..40).map(move |x| (x, y)))
            .map(|(x, y)| b[(x, y)].symbol().to_string())
            .collect();
        assert!(text.contains("too small"), "{text}");
    }

    #[test]
    fn resizing_between_frames_reallocates_the_canvas_without_panicking() {
        let mut a = App::new(1, 4);
        for (w, h) in [(120u16, 40u16), (86, 31), (200, 60), (40, 10), (90, 32)] {
            let mut b = buf(w, h);
            a.render(&mut b);
        }
    }

    #[test]
    fn a_full_run_from_start_to_death_never_panics() {
        let mut a = App::new(7, 4);
        a.update(0.0, &[Action::Turn(Direction::Right)]);
        let dirs = [
            Direction::Down,
            Direction::Left,
            Direction::Up,
            Direction::Right,
        ];
        let mut b = buf(120, 40);
        for i in 0..600 {
            let input = if i % 7 == 0 {
                vec![Action::Turn(dirs[(i / 7) % 4])]
            } else {
                vec![]
            };
            a.update(0.02, &input);
            a.render(&mut b);
        }
    }
}
