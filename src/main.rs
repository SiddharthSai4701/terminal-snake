mod app;
mod game;
mod input;
mod render;
mod ui;

use std::io::BufWriter;
use std::time::{Duration, Instant};

use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::crossterm::cursor;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::crossterm::{execute, terminal};
use ratatui::Terminal;

use app::App;
use game::types::Direction;
use input::Action;
use render::layout::DEFAULT_MAX_SCALE;
use render::tier::{detect, suppress_sync, Tier};

const FRAME: Duration = Duration::from_micros(16_667);
/// Large enough to absorb a whole worst-case frame, so the ANSI stream flushes
/// as one write instead of dozens of console calls.
const OUT_BUF: usize = 1 << 19;

fn env(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

fn map_key(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('W') => Some(Action::Turn(Direction::Up)),
        KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('S') => {
            Some(Action::Turn(Direction::Down))
        }
        KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A') => {
            Some(Action::Turn(Direction::Left))
        }
        KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D') => {
            Some(Action::Turn(Direction::Right))
        }
        KeyCode::Char('r') | KeyCode::Char('R') => Some(Action::Restart),
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => Some(Action::Quit),
        KeyCode::Enter | KeyCode::Char(' ') => Some(Action::Start),
        _ => None,
    }
}

fn entropy_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x2545_F491_4F6C_DD1D)
}

fn main() -> std::io::Result<()> {
    let tier = detect(
        env("COLORTERM").as_deref(),
        env("TERM").as_deref(),
        env("TERM_PROGRAM").as_deref(),
        env("WT_SESSION").is_some(),
    );
    if tier == Tier::Refused {
        eprintln!("terminal-snake needs a 256-colour or truecolour terminal.");
        eprintln!("Try Windows Terminal, iTerm2, WezTerm, Alacritty, kitty, or Ghostty.");
        return Ok(());
    }
    let sync = !suppress_sync(env("TERM").as_deref(), env("TMUX").is_some());

    // Restore the terminal before the panic message prints, so a crash never
    // leaves the user with a broken shell.
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(
            std::io::stdout(),
            terminal::LeaveAlternateScreen,
            cursor::Show
        );
        previous_hook(info);
    }));

    terminal::enable_raw_mode()?;
    let mut out = BufWriter::with_capacity(OUT_BUF, std::io::stdout());
    execute!(out, terminal::EnterAlternateScreen, cursor::Hide)?;
    let mut term = Terminal::new(CrosstermBackend::new(out))?;
    term.clear()?;

    let mut app = App::new(entropy_seed(), DEFAULT_MAX_SCALE);
    app.set_tier(tier);

    let mut last = Instant::now();
    let mut deadline = Instant::now();

    while !app.should_quit() {
        // Drain with a zero timeout and pace separately. Handing crossterm a
        // real timeout waits on the console handle, whose timeouts round to the
        // ~15.6 ms system tick - that quantizes frame times into visible judder
        // that looks like a rendering bug.
        let mut actions = Vec::new();
        while event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(k) => {
                    // Windows reports both Press and Release for every key;
                    // unfiltered, one tap fills the depth-2 queue with
                    // duplicates and fast corners stop working.
                    if k.kind != KeyEventKind::Press {
                        continue;
                    }
                    if let Some(a) = map_key(k.code) {
                        actions.push(a);
                    }
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        let now = Instant::now();
        let dt = (now - last).as_secs_f32().min(0.25);
        last = now;
        app.update(dt, &actions);

        if sync {
            let _ = execute!(term.backend_mut(), terminal::BeginSynchronizedUpdate);
        }
        term.draw(|f| app.render(f.buffer_mut()))?;
        if sync {
            let _ = execute!(term.backend_mut(), terminal::EndSynchronizedUpdate);
        }
        Backend::flush(term.backend_mut())?;

        deadline += FRAME;
        let now = Instant::now();
        if deadline > now {
            std::thread::sleep(deadline - now);
        } else {
            deadline = now;
        }
    }

    terminal::disable_raw_mode()?;
    execute!(
        term.backend_mut(),
        terminal::LeaveAlternateScreen,
        cursor::Show
    )?;
    Backend::flush(term.backend_mut())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_and_wasd_map_to_the_same_turns() {
        assert_eq!(map_key(KeyCode::Up), Some(Action::Turn(Direction::Up)));
        assert_eq!(map_key(KeyCode::Char('w')), map_key(KeyCode::Up));
        assert_eq!(map_key(KeyCode::Char('a')), map_key(KeyCode::Left));
        assert_eq!(map_key(KeyCode::Char('s')), map_key(KeyCode::Down));
        assert_eq!(map_key(KeyCode::Char('d')), map_key(KeyCode::Right));
    }

    #[test]
    fn letter_keys_are_case_insensitive() {
        assert_eq!(map_key(KeyCode::Char('W')), map_key(KeyCode::Char('w')));
        assert_eq!(map_key(KeyCode::Char('Q')), Some(Action::Quit));
        assert_eq!(map_key(KeyCode::Char('R')), Some(Action::Restart));
    }

    #[test]
    fn unmapped_keys_produce_nothing() {
        assert_eq!(map_key(KeyCode::Char('z')), None);
        assert_eq!(map_key(KeyCode::F(5)), None);
    }
}
