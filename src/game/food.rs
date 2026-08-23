use crate::game::rng::Pcg32;
use crate::game::snake::Snake;
use crate::game::types::{Pos, GRID_H, GRID_W};

/// Picks a uniformly random free cell.
///
/// Enumerates free cells rather than rejection-sampling: rejection sampling
/// would spin forever on a full board, and would make the daily food sequence
/// depend on how many rejections happened.
///
/// Returns `None` when the board is full, which the caller turns into the win
/// state.
pub fn spawn(snake: &Snake, exclude: Option<Pos>, rng: &mut Pcg32) -> Option<Pos> {
    let mut free: Vec<Pos> = Vec::with_capacity((GRID_W * GRID_H) as usize);
    for y in 0..GRID_H {
        for x in 0..GRID_W {
            let p = Pos::new(x, y);
            if snake.contains(p) {
                continue;
            }
            if Some(p) == exclude {
                continue;
            }
            free.push(p);
        }
    }
    if free.is_empty() {
        return None;
    }
    Some(free[rng.below(free.len() as u32) as usize])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::types::Direction;

    fn all_cells() -> Vec<Pos> {
        (0..GRID_H)
            .flat_map(|y| (0..GRID_W).map(move |x| Pos::new(x, y)))
            .collect()
    }

    #[test]
    fn never_spawns_on_the_snake() {
        let s = Snake::new(Pos::new(9, 9), 6, Direction::Right);
        let mut r = Pcg32::new(1);
        for _ in 0..2000 {
            let p = spawn(&s, None, &mut r).unwrap();
            assert!(!s.contains(p));
        }
    }

    #[test]
    fn respects_the_exclusion_cell() {
        let s = Snake::new(Pos::new(0, 0), 1, Direction::Right);
        let mut r = Pcg32::new(2);
        let excluded = Pos::new(5, 5);
        for _ in 0..2000 {
            assert_ne!(spawn(&s, Some(excluded), &mut r).unwrap(), excluded);
        }
    }

    #[test]
    fn spawns_stay_inside_the_grid() {
        let s = Snake::new(Pos::new(9, 9), 4, Direction::Right);
        let mut r = Pcg32::new(3);
        for _ in 0..2000 {
            let p = spawn(&s, None, &mut r).unwrap();
            assert!(p.x >= 0 && p.x < GRID_W && p.y >= 0 && p.y < GRID_H);
        }
    }

    #[test]
    fn a_full_board_returns_none_instead_of_hanging() {
        let s = Snake::from_cells(all_cells(), Direction::Right);
        let mut r = Pcg32::new(4);
        assert_eq!(spawn(&s, None, &mut r), None);
    }

    #[test]
    fn the_same_seed_yields_the_same_sequence() {
        let s = Snake::new(Pos::new(9, 9), 4, Direction::Right);
        let run = || {
            let mut r = Pcg32::new(77);
            (0..20)
                .map(|_| spawn(&s, None, &mut r).unwrap())
                .collect::<Vec<Pos>>()
        };
        assert_eq!(run(), run());
    }
}
