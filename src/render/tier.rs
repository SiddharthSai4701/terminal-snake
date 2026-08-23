//! Terminal colour capability.
//!
//! Rust and crossterm make the game build and run everywhere; they do not make
//! every terminal capable of 24-bit colour. macOS Terminal.app is the case that
//! matters - it has never supported truecolour, so an unguarded build would
//! look broken to every default-Terminal Mac user.

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Tier {
    Full,
    Reduced,
    Refused,
}

const TRUECOLOR_PROGRAMS: [&str; 6] = [
    "iTerm.app",
    "WezTerm",
    "ghostty",
    "alacritty",
    "kitty",
    "vscode",
];

pub fn detect(
    colorterm: Option<&str>,
    term: Option<&str>,
    term_program: Option<&str>,
    windows_terminal: bool,
) -> Tier {
    if let Some(ct) = colorterm {
        let ct = ct.to_ascii_lowercase();
        if ct.contains("truecolor") || ct.contains("24bit") {
            return Tier::Full;
        }
    }
    if windows_terminal {
        return Tier::Full;
    }
    if let Some(p) = term_program {
        if TRUECOLOR_PROGRAMS.iter().any(|k| k.eq_ignore_ascii_case(p)) {
            return Tier::Full;
        }
    }
    match term {
        Some(t) if t.contains("direct") => Tier::Full,
        Some(t) if t.contains("256color") => Tier::Reduced,
        _ => Tier::Refused,
    }
}

/// tmux and screen do not pass DEC mode 2026 through by default, so the
/// synchronized-output escape is suppressed rather than emitted blind.
pub fn suppress_sync(term: Option<&str>, tmux_env: bool) -> bool {
    if tmux_env {
        return true;
    }
    matches!(term, Some(t) if t.starts_with("tmux") || t.starts_with("screen"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorterm_truecolor_is_full() {
        assert_eq!(detect(Some("truecolor"), None, None, false), Tier::Full);
        assert_eq!(detect(Some("24bit"), None, None, false), Tier::Full);
        assert_eq!(detect(Some("TRUECOLOR"), None, None, false), Tier::Full);
    }

    #[test]
    fn windows_terminal_is_full_even_without_colorterm() {
        assert_eq!(detect(None, None, None, true), Tier::Full);
    }

    #[test]
    fn apple_terminal_is_reduced_not_full() {
        assert_eq!(
            detect(None, Some("xterm-256color"), Some("Apple_Terminal"), false),
            Tier::Reduced
        );
    }

    #[test]
    fn known_truecolor_emulators_are_full() {
        for p in ["iTerm.app", "WezTerm", "ghostty", "kitty", "alacritty"] {
            assert_eq!(
                detect(None, Some("xterm-256color"), Some(p), false),
                Tier::Full,
                "{p}"
            );
        }
    }

    #[test]
    fn plain_256color_is_reduced_and_bare_terminals_are_refused() {
        assert_eq!(
            detect(None, Some("xterm-256color"), None, false),
            Tier::Reduced
        );
        assert_eq!(detect(None, Some("xterm"), None, false), Tier::Refused);
        assert_eq!(detect(None, Some("dumb"), None, false), Tier::Refused);
        assert_eq!(detect(None, None, None, false), Tier::Refused);
    }

    #[test]
    fn direct_colour_terminfo_is_full() {
        assert_eq!(detect(None, Some("xterm-direct"), None, false), Tier::Full);
    }

    #[test]
    fn multiplexers_suppress_synchronized_output() {
        assert!(suppress_sync(Some("tmux-256color"), false));
        assert!(suppress_sync(Some("screen"), false));
        assert!(suppress_sync(Some("xterm-256color"), true));
        assert!(!suppress_sync(Some("xterm-256color"), false));
        assert!(!suppress_sync(None, false));
    }
}
