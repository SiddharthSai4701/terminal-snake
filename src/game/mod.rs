pub mod food;
pub mod rng;
pub mod score;
pub mod snake;
pub mod types;

use crate::input::DirQueue;
use rng::Pcg32;
use snake::{Snake, StepOutcome};
pub use types::{Direction, Mode, Pos};

pub const START: Pos = Pos::new(9, 9);
pub const START_LEN: usize = 4;
pub const START_DIR: Direction = Direction::Right;
pub const MAX_TICKS_PER_FRAME: u32 = 5;
pub const FOOD_SCORE: u32 = 10;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum GameState {
    AwaitingStart,
    Running,
    Dead,
    Won,
}

pub struct Game {
    mode: Mode,
    snake: Snake,
    food: Pos,
    rng: Pcg32,
    acc: f32,
    tick_ms: f32,
    pub state: GameState,
    pub score: u32,
    pub normal_food_eaten: u32,
    pub elapsed: f32,
}

impl Game {
    pub fn new(mode: Mode, seed: u64) -> Self {
        let snake = Snake::new(START, START_LEN, START_DIR);
        let mut rng = Pcg32::new(seed);
        let food = food::spawn(&snake, None, &mut rng).expect("board cannot be full at start");
        Game {
            mode,
            snake,
            food,
            rng,
            acc: 0.0,
            tick_ms: score::tick_ms(0),
            state: GameState::AwaitingStart,
            score: 0,
            normal_food_eaten: 0,
            elapsed: 0.0,
        }
    }

    #[allow(dead_code)] // used by the mode-specific screens in Phase 3
    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn snake(&self) -> &Snake {
        &self.snake
    }

    pub fn food(&self) -> Pos {
        self.food
    }

    pub fn tick_ms(&self) -> f32 {
        self.tick_ms
    }

    pub fn start(&mut self) {
        if self.state == GameState::AwaitingStart {
            self.state = GameState::Running;
        }
    }

    /// How far the snake is between its current cell and the next one, for
    /// render interpolation.
    #[allow(dead_code)] // drives ribbon interpolation in Phase 2
    pub fn tick_fraction(&self) -> f32 {
        (self.acc / (self.tick_ms / 1000.0)).clamp(0.0, 1.0)
    }

    pub fn advance(&mut self, dt: f32, queue: &mut DirQueue) {
        if self.state != GameState::Running {
            return;
        }
        self.elapsed += dt;

        let tick_dt = self.tick_ms / 1000.0;
        // Clamped so a stall - a window drag, a breakpoint, a laptop sleep -
        // cannot teleport the snake across the arena in one frame.
        self.acc = (self.acc + dt).min(MAX_TICKS_PER_FRAME as f32 * tick_dt);

        loop {
            let tick_dt = self.tick_ms / 1000.0;
            if self.acc < tick_dt {
                break;
            }
            self.acc -= tick_dt;
            self.tick(queue);
            if self.state != GameState::Running {
                self.acc = 0.0;
                return;
            }
            // Recomputed only here, at a tick boundary, so a partly filled
            // accumulator is never divided by a freshly shortened tick - that
            // would make the interpolation fraction jump visibly.
            self.tick_ms = score::tick_ms(self.normal_food_eaten);
        }
    }

    fn tick(&mut self, queue: &mut DirQueue) {
        let dir = queue.pop();
        match self.snake.step(dir, self.mode.wraps()) {
            StepOutcome::HitWall | StepOutcome::HitSelf => {
                self.state = GameState::Dead;
            }
            StepOutcome::Moved => {
                if self.snake.head() == self.food {
                    self.snake.grow(1);
                    self.score += FOOD_SCORE;
                    self.normal_food_eaten += 1;
                    match food::spawn(&self.snake, None, &mut self.rng) {
                        Some(p) => self.food = p,
                        None => self.state = GameState::Won,
                    }
                }
            }
        }
    }

    #[cfg(test)]
    pub fn force_food_at(&mut self, p: Pos) {
        self.food = p;
    }

    /// Lengthens the snake in place, for the frame-budget benchmark.
    #[cfg(test)]
    pub fn grow_for_bench(&mut self, n: u32) {
        self.snake.grow(n);
        let mut q = crate::input::DirQueue::new(self.snake.dir());
        for _ in 0..n {
            self.snake.step(q.pop(), true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::DirQueue;

    fn game() -> (Game, DirQueue) {
        (Game::new(Mode::Classic, 1), DirQueue::new(Direction::Right))
    }

    #[test]
    fn starts_at_the_spec_position_and_waits_for_input() {
        let (g, _) = game();
        assert_eq!(g.state, GameState::AwaitingStart);
        assert_eq!(g.snake().head(), Pos::new(9, 9));
        assert_eq!(g.snake().len(), 4);
        assert_eq!(g.snake().dir(), Direction::Right);
    }

    #[test]
    fn does_not_advance_before_the_first_press() {
        let (mut g, mut q) = game();
        g.advance(5.0, &mut q);
        assert_eq!(g.snake().head(), Pos::new(9, 9));
        assert_eq!(g.elapsed, 0.0);
    }

    #[test]
    fn one_tick_of_time_moves_one_cell() {
        let (mut g, mut q) = game();
        g.start();
        g.advance(0.140, &mut q);
        assert_eq!(g.snake().head(), Pos::new(10, 9));
    }

    #[test]
    fn a_long_stall_is_clamped_to_five_ticks() {
        let (mut g, mut q) = game();
        g.start();
        g.advance(3.0, &mut q);
        assert_eq!(g.snake().head(), Pos::new(14, 9));
    }

    #[test]
    fn tick_fraction_interpolates_between_zero_and_one() {
        let (mut g, mut q) = game();
        g.start();
        g.advance(0.070, &mut q);
        let f = g.tick_fraction();
        assert!(f > 0.4 && f < 0.6, "fraction was {f}");
    }

    #[test]
    fn hitting_a_wall_in_classic_kills() {
        let (mut g, mut q) = game();
        g.start();
        for _ in 0..4 {
            g.advance(10.0, &mut q);
        }
        assert_eq!(g.state, GameState::Dead);
    }

    #[test]
    fn eating_scores_grows_and_counts_food() {
        let mut g = Game::new(Mode::Classic, 1);
        let mut q = DirQueue::new(Direction::Right);
        g.start();
        g.force_food_at(Pos::new(10, 9));
        g.advance(0.140, &mut q);
        assert_eq!(g.score, 10);
        assert_eq!(g.normal_food_eaten, 1);
        assert!(g.tick_ms() < 140.0, "speed should have increased");
        g.advance(0.140, &mut q);
        assert_eq!(g.snake().len(), 5);
    }

    #[test]
    fn food_never_spawns_under_the_snake() {
        let mut g = Game::new(Mode::Classic, 5);
        assert!(!g.snake().contains(g.food()));
        let mut q = DirQueue::new(Direction::Right);
        g.start();
        for _ in 0..40 {
            g.advance(0.140, &mut q);
            assert!(!g.snake().contains(g.food()));
            if g.state != GameState::Running {
                break;
            }
        }
    }

    #[test]
    fn the_same_seed_replays_identically() {
        let run = |seed: u64| {
            let mut g = Game::new(Mode::Classic, seed);
            let mut q = DirQueue::new(Direction::Right);
            g.start();
            let mut path = vec![];
            for _ in 0..12 {
                g.advance(0.140, &mut q);
                path.push(g.food());
            }
            path
        };
        assert_eq!(run(1234), run(1234));
    }

    #[test]
    fn endless_mode_wraps_instead_of_dying() {
        let mut g = Game::new(Mode::Endless, 3);
        let mut q = DirQueue::new(Direction::Right);
        g.start();
        for _ in 0..8 {
            g.advance(10.0, &mut q);
        }
        assert_eq!(g.state, GameState::Running);
    }
}
