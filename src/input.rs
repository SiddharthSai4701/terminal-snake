use crate::game::types::Direction;
use std::collections::VecDeque;

pub const QUEUE_CAP: usize = 2;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Action {
    Turn(Direction),
    Start,
    #[allow(dead_code)] // wired up by the pause overlay in Phase 4
    Pause,
    Restart,
    Quit,
}

/// A short queue of pending turns.
///
/// Depth 2 is what lets a fast corner (up, then left, inside a single tick)
/// register instead of being swallowed. The reversal check is against the last
/// direction *in the queue*, not the direction currently being travelled:
/// with Right applied and Up queued, a Down press does not reverse Right, so
/// checking against Right would accept it and the next two ticks would apply
/// Up then Down — an instant self-collision.
pub struct DirQueue {
    applied: Direction,
    q: VecDeque<Direction>,
}

impl DirQueue {
    pub fn new(initial: Direction) -> Self {
        DirQueue {
            applied: initial,
            q: VecDeque::with_capacity(QUEUE_CAP),
        }
    }

    #[allow(dead_code)] // asserted by tests; read by the Phase 2 ribbon end caps
    pub fn applied(&self) -> Direction {
        self.applied
    }

    #[allow(dead_code)] // asserted by the queue tests
    pub fn len(&self) -> usize {
        self.q.len()
    }

    #[allow(dead_code)] // paired with len() for clippy's len_without_is_empty
    pub fn is_empty(&self) -> bool {
        self.q.is_empty()
    }

    fn effective(&self) -> Direction {
        *self.q.back().unwrap_or(&self.applied)
    }

    pub fn push(&mut self, d: Direction) {
        let eff = self.effective();
        // A repeat press would burn one of only two slots for no movement.
        if d == eff || d == eff.opposite() {
            return;
        }
        if self.q.len() >= QUEUE_CAP {
            return;
        }
        self.q.push_back(d);
    }

    pub fn pop(&mut self) -> Direction {
        if let Some(d) = self.q.pop_front() {
            self.applied = d;
        }
        self.applied
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::types::Direction::*;

    #[test]
    fn a_repeat_press_is_discarded_so_it_cannot_burn_a_slot() {
        let mut q = DirQueue::new(Right);
        q.push(Right);
        assert_eq!(q.len(), 0);
    }

    #[test]
    fn a_direct_reversal_is_rejected() {
        let mut q = DirQueue::new(Right);
        q.push(Left);
        assert_eq!(q.len(), 0);
    }

    #[test]
    fn a_fast_corner_is_admitted() {
        let mut q = DirQueue::new(Right);
        q.push(Up);
        q.push(Left);
        assert_eq!(q.len(), 2);
        assert_eq!(q.pop(), Up);
        assert_eq!(q.pop(), Left);
    }

    #[test]
    fn reversal_is_checked_against_the_queue_tail_not_the_applied_dir() {
        let mut q = DirQueue::new(Right);
        q.push(Up);
        q.push(Down);
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn a_full_queue_drops_the_newest_press() {
        let mut q = DirQueue::new(Right);
        q.push(Up);
        q.push(Left);
        q.push(Down);
        assert_eq!(q.len(), 2);
        assert_eq!(q.pop(), Up);
        assert_eq!(q.pop(), Left);
    }

    #[test]
    fn popping_an_empty_queue_repeats_the_applied_direction() {
        let mut q = DirQueue::new(Right);
        assert_eq!(q.pop(), Right);
        assert_eq!(q.pop(), Right);
    }

    #[test]
    fn pop_updates_the_applied_direction() {
        let mut q = DirQueue::new(Right);
        q.push(Up);
        assert_eq!(q.pop(), Up);
        assert_eq!(q.applied(), Up);
    }
}
