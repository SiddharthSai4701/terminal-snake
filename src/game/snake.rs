use crate::game::types::{Direction, Pos, GRID_H, GRID_W};
use std::collections::VecDeque;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum StepOutcome {
    Moved,
    HitWall,
    HitSelf,
}

pub struct Snake {
    body: VecDeque<Pos>,
    dir: Direction,
    grow: u32,
}

impl Snake {
    /// `body[0]` is the head; the remaining segments trail behind it opposite
    /// `dir`.
    pub fn new(head: Pos, len: usize, dir: Direction) -> Self {
        let (dx, dy) = dir.delta();
        let mut body = VecDeque::with_capacity(len.max(8));
        for i in 0..len as i32 {
            body.push_back(Pos::new(head.x - dx * i, head.y - dy * i));
        }
        Snake {
            body,
            dir,
            grow: 0,
        }
    }

    pub fn head(&self) -> Pos {
        self.body[0]
    }

    pub fn len(&self) -> usize {
        self.body.len()
    }

    #[allow(dead_code)] // the Phase 2 ribbon needs the heading for its end caps
    pub fn dir(&self) -> Direction {
        self.dir
    }

    pub fn iter(&self) -> impl Iterator<Item = &Pos> {
        self.body.iter()
    }

    pub fn contains(&self, p: Pos) -> bool {
        self.body.contains(&p)
    }

    pub fn grow(&mut self, n: u32) {
        self.grow += n;
    }

    pub fn step(&mut self, dir: Direction, wrap: bool) -> StepOutcome {
        self.dir = dir;
        let (dx, dy) = dir.delta();
        let mut nx = self.head().x + dx;
        let mut ny = self.head().y + dy;

        if nx < 0 || nx >= GRID_W || ny < 0 || ny >= GRID_H {
            if !wrap {
                return StepOutcome::HitWall;
            }
            nx = nx.rem_euclid(GRID_W);
            ny = ny.rem_euclid(GRID_H);
        }
        let next = Pos::new(nx, ny);

        // The tail vacates on this same tick unless we are growing, so it is
        // not an obstacle in that case.
        let growing = self.grow > 0;
        let occupied = if growing {
            self.body.iter().any(|&p| p == next)
        } else {
            let n = self.body.len();
            self.body.iter().take(n - 1).any(|&p| p == next)
        };
        if occupied {
            return StepOutcome::HitSelf;
        }

        self.body.push_front(next);
        if growing {
            self.grow -= 1;
        } else {
            self.body.pop_back();
        }
        StepOutcome::Moved
    }

    #[cfg(test)]
    pub fn from_cells(cells: Vec<Pos>, dir: Direction) -> Self {
        Snake {
            body: cells.into(),
            dir,
            grow: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snake() -> Snake {
        Snake::new(Pos::new(9, 9), 4, Direction::Right)
    }

    #[test]
    fn new_lays_body_behind_the_head() {
        let s = snake();
        assert_eq!(s.head(), Pos::new(9, 9));
        assert_eq!(s.len(), 4);
        assert!(s.contains(Pos::new(8, 9)));
        assert!(s.contains(Pos::new(6, 9)));
        assert!(!s.contains(Pos::new(5, 9)));
    }

    #[test]
    fn step_moves_head_and_drops_tail() {
        let mut s = snake();
        assert_eq!(s.step(Direction::Right, false), StepOutcome::Moved);
        assert_eq!(s.head(), Pos::new(10, 9));
        assert_eq!(s.len(), 4);
        assert!(!s.contains(Pos::new(6, 9)));
    }

    #[test]
    fn growth_holds_the_tail_and_adds_exactly_one_per_unit() {
        let mut s = snake();
        s.grow(1);
        s.step(Direction::Right, false);
        assert_eq!(s.len(), 5);
        assert!(s.contains(Pos::new(6, 9)));
        s.step(Direction::Right, false);
        assert_eq!(s.len(), 5);
    }

    #[test]
    fn wall_kills_when_not_wrapping() {
        let mut s = Snake::new(Pos::new(GRID_W - 1, 9), 2, Direction::Right);
        assert_eq!(s.step(Direction::Right, false), StepOutcome::HitWall);
    }

    #[test]
    fn wall_wraps_when_wrapping() {
        let mut s = Snake::new(Pos::new(GRID_W - 1, 9), 2, Direction::Right);
        assert_eq!(s.step(Direction::Right, true), StepOutcome::Moved);
        assert_eq!(s.head(), Pos::new(0, 9));
    }

    #[test]
    fn wrapping_covers_the_other_edges() {
        let mut up = Snake::new(Pos::new(4, 0), 2, Direction::Up);
        assert_eq!(up.step(Direction::Up, true), StepOutcome::Moved);
        assert_eq!(up.head(), Pos::new(4, GRID_H - 1));

        let mut left = Snake::new(Pos::new(0, 4), 2, Direction::Left);
        assert_eq!(left.step(Direction::Left, true), StepOutcome::Moved);
        assert_eq!(left.head(), Pos::new(GRID_W - 1, 4));
    }

    #[test]
    fn entering_the_vacating_tail_cell_is_legal() {
        let mut s = Snake::new(Pos::new(5, 5), 4, Direction::Right);
        s.step(Direction::Down, false);
        s.step(Direction::Left, false);
        assert_eq!(s.step(Direction::Up, false), StepOutcome::Moved);
    }

    #[test]
    fn entering_the_tail_cell_while_growing_is_a_collision() {
        let mut s = Snake::new(Pos::new(5, 5), 4, Direction::Right);
        s.step(Direction::Down, false);
        s.step(Direction::Left, false);
        s.grow(1);
        assert_eq!(s.step(Direction::Up, false), StepOutcome::HitSelf);
    }

    #[test]
    fn running_into_the_middle_of_the_body_is_a_collision() {
        let mut s = Snake::new(Pos::new(5, 5), 6, Direction::Right);
        s.step(Direction::Down, false);
        s.step(Direction::Left, false);
        assert_eq!(s.step(Direction::Up, false), StepOutcome::HitSelf);
    }
}
