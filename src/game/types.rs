pub const GRID_W: i32 = 28;
pub const GRID_H: i32 = 18;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

impl Pos {
    pub const fn new(x: i32, y: i32) -> Self {
        Pos { x, y }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    pub fn delta(self) -> (i32, i32) {
        match self {
            Direction::Up => (0, -1),
            Direction::Down => (0, 1),
            Direction::Left => (-1, 0),
            Direction::Right => (1, 0),
        }
    }

    pub fn opposite(self) -> Direction {
        match self {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[allow(dead_code)] // Endless and Daily are wired up in Phase 3
pub enum Mode {
    Classic,
    Endless,
    Daily,
}

impl Mode {
    pub fn wraps(self) -> bool {
        matches!(self, Mode::Endless)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_deltas_are_screen_space() {
        assert_eq!(Direction::Up.delta(), (0, -1));
        assert_eq!(Direction::Down.delta(), (0, 1));
        assert_eq!(Direction::Left.delta(), (-1, 0));
        assert_eq!(Direction::Right.delta(), (1, 0));
    }

    #[test]
    fn opposites_round_trip() {
        for d in [
            Direction::Up,
            Direction::Down,
            Direction::Left,
            Direction::Right,
        ] {
            assert_eq!(d.opposite().opposite(), d);
            assert_ne!(d.opposite(), d);
        }
    }

    #[test]
    fn only_endless_wraps() {
        assert!(!Mode::Classic.wraps());
        assert!(Mode::Endless.wraps());
        assert!(!Mode::Daily.wraps());
    }
}
