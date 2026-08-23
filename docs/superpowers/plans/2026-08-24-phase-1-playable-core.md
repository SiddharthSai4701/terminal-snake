# Phase 1 — Playable Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A real, playable Classic-mode snake game in the terminal — flat-color rendering, correct rules, correct input feel — with every rule under unit test.

**Architecture:** Pure `game/` logic (no I/O, no clock, no terminal) driven by a fixed-timestep accumulator, rendered through an f32 linear-light pixel canvas that quantizes to half-block cells in a ratatui `Buffer`. `main.rs` owns the loop; `app.rs` is a pure `update`/`render` pair so the later WASM build reuses it untouched.

**Tech Stack:** Rust 2021, ratatui 0.30 (`ratatui-core` + `ratatui-widgets` + `ratatui-crossterm`), crossterm 0.29 via ratatui's re-export only.

**Spec:** `docs/superpowers/specs/2026-08-23-terminal-snake-design.md`

## Global Constraints

Copied verbatim from the spec. Every task's requirements implicitly include these.

- Logic grid is **28 × 18 cells**, fixed. Pixel scale is an **integer 3–6**, default cap **4**. Minimum terminal **86 × 31**.
- `game/`, `render/*`, and `ui/*` must not reference `std::io`, `crossterm`, the filesystem, the system clock, or entropy. Time enters as `dt`; randomness enters as an injected RNG.
- **The loop lives in `main.rs` alone.** `app.rs` exposes `update(&mut self, dt: f32, input: &[Action])` and `render(&mut self, buf: &mut Buffer)`.
- **Never add `crossterm` to `[dependencies]`.** Use `ratatui::crossterm::…`. The crossterm-backed ratatui crate is declared under `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`.
- `input.rs` maps `KeyEvent` → `Action` at the boundary. A `KeyCode` must never appear in an `app.rs` signature.
- **Windows sends Press *and* Release for every key.** Drop everything that is not `KeyEventKind::Press`.
- No `rand` dependency. Hand-rolled PCG32 + FNV-1a-64 in `game/rng.rs`.
- Build profiles from the first commit: `[profile.dev] opt-level = 1`, `[profile.dev.package."*"] opt-level = 3`.
- `tick_ms = clamp(140.0 * 0.985^normal_food_eaten, 55.0, 140.0)`. Accumulator clamped to `5 × tick_dt`. `tick_ms` recomputed only at tick boundaries.
- Initial state: length **4**, head at cell **(9, 9)**, facing **Right**, waits for the first directional press.
- Direction queue depth **2**; reversal validated against **the last direction in the queue**, falling back to applied when empty; a repeat press is discarded; a full queue drops the newest press.

---

## File Structure

| File | Responsibility |
|---|---|
| `Cargo.toml` | deps, build profiles |
| `src/game/rng.rs` | PCG32, FNV-1a-64 |
| `src/game/types.rs` | `Pos`, `Direction`, `Mode`, grid constants |
| `src/game/snake.rs` | body deque, step, growth, collision |
| `src/game/food.rs` | free-cell enumeration, spawn |
| `src/game/score.rs` | speed curve |
| `src/game/mod.rs` | `Game`: accumulator, tick, state machine, tick fraction |
| `src/input.rs` | `Action` mapping, Press filter, `DirQueue` |
| `src/render/color.rs` | sRGB encode/decode, tone map, u8 + 5-bit snap, nearest-256 |
| `src/render/canvas.rs` | f32 linear buffer, blend, quantize into `Buffer` |
| `src/render/layout.rs` | scale + canvas rect from terminal size, min-size gate |
| `src/render/tier.rs` | truecolor capability detection |
| `src/ui/hud.rs` | score / length / speed line |
| `src/ui/resize.rs` | below-minimum screen |
| `src/app.rs` | pure state machine, `update` / `render` |
| `src/main.rs` | terminal setup, BufWriter, sync output, panic hook, loop |

---

### Task 1: Scaffold

**Files:**
- Create: `Cargo.toml`, `src/main.rs`, `rust-toolchain.toml`

**Interfaces:**
- Produces: a compiling binary crate named `terminal-snake`.

- [ ] **Step 1: Write `Cargo.toml`**

```toml
[package]
name = "terminal-snake"
version = "0.1.0"
edition = "2021"
description = "A terminal snake game with truecolor pixel rendering, glow, particles, and 60fps interpolated motion."
license = "MIT"
repository = "https://github.com/SiddharthSai4701/terminal-snake"

[dependencies]
ratatui-core = "0.30"
ratatui-widgets = "0.30"

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
ratatui = "0.30"

[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3

[profile.release]
lto = true
codegen-units = 1
```

- [ ] **Step 2: Write a placeholder `src/main.rs`**

```rust
fn main() {
    println!("terminal-snake");
}
```

- [ ] **Step 3: Verify it builds**

Run: `cargo build`
Expected: compiles clean.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs
git commit -m "chore: scaffold cargo project with build profiles"
```

---

### Task 2: RNG (`game/rng.rs`)

Deterministic forever — this is what makes the daily challenge portable across machines and versions.

**Files:**
- Create: `src/game/rng.rs`, `src/game/mod.rs`
- Modify: `src/main.rs` (add `mod game;`)

**Interfaces:**
- Produces: `Pcg32::new(seed: u64) -> Pcg32`, `Pcg32::next_u32(&mut self) -> u32`, `Pcg32::below(&mut self, bound: u32) -> u32` (unbiased), `fnv1a64(bytes: &[u8]) -> u64`.

- [ ] **Step 1: Write the failing tests**

Test vectors are the canonical PCG32 demo output for `initstate=42, initseq=54`, and the canonical FNV-1a-64 vectors. Hard-coding them means a future refactor cannot silently change historical daily seeds.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcg32_matches_reference_vectors() {
        let mut r = Pcg32::new(42);
        assert_eq!(r.next_u32(), 0xa15c02b7);
        assert_eq!(r.next_u32(), 0x7b47f409);
        assert_eq!(r.next_u32(), 0xba1d3330);
    }

    #[test]
    fn fnv1a64_matches_reference_vectors() {
        assert_eq!(fnv1a64(b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a64(b"a"), 0xaf63dc4c8601ec8c);
    }

    #[test]
    fn below_is_in_range_and_unbiased_at_edges() {
        let mut r = Pcg32::new(7);
        for _ in 0..10_000 {
            assert!(r.below(504) < 504);
        }
        assert_eq!(Pcg32::new(1).below(1), 0);
    }

    #[test]
    fn same_seed_same_sequence() {
        let a: Vec<u32> = (0..8).map(|_| Pcg32::new(99).next_u32()).collect();
        let mut b = Pcg32::new(99);
        assert_eq!(a[0], b.next_u32());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test rng`
Expected: FAIL — `Pcg32` not found.

- [ ] **Step 3: Implement**

```rust
const PCG_MULT: u64 = 6364136223846793005;
const PCG_SEQ: u64 = 54;

pub struct Pcg32 { state: u64, inc: u64 }

impl Pcg32 {
    pub fn new(seed: u64) -> Self {
        let mut r = Pcg32 { state: 0, inc: (PCG_SEQ << 1) | 1 };
        r.next_u32();
        r.state = r.state.wrapping_add(seed);
        r.next_u32();
        r
    }

    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(PCG_MULT).wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Unbiased bounded draw via rejection of the incomplete final block.
    pub fn below(&mut self, bound: u32) -> u32 {
        assert!(bound > 0);
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let r = self.next_u32();
            if r >= threshold { return r % bound; }
        }
    }
}

pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test rng`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add src/game/
git commit -m "feat(game): add deterministic PCG32 and FNV-1a-64"
```

---

### Task 3: Types (`game/types.rs`)

**Files:**
- Create: `src/game/types.rs`

**Interfaces:**
- Produces: `GRID_W: i32 = 28`, `GRID_H: i32 = 18`, `Pos { x: i32, y: i32 }`, `Direction { Up, Down, Left, Right }` with `delta()` and `opposite()`, `Mode { Classic, Endless, Daily }` with `wraps()`.

- [ ] **Step 1: Write the failing tests**

```rust
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
        for d in [Direction::Up, Direction::Down, Direction::Left, Direction::Right] {
            assert_eq!(d.opposite().opposite(), d);
            assert_ne!(d.opposite(), d);
        }
    }

    #[test]
    fn only_classic_and_daily_kill_on_walls() {
        assert!(!Mode::Classic.wraps());
        assert!(Mode::Endless.wraps());
        assert!(!Mode::Daily.wraps());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test types`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement**

```rust
pub const GRID_W: i32 = 28;
pub const GRID_H: i32 = 18;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct Pos { pub x: i32, pub y: i32 }

impl Pos {
    pub fn new(x: i32, y: i32) -> Self { Pos { x, y } }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Direction { Up, Down, Left, Right }

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
pub enum Mode { Classic, Endless, Daily }

impl Mode {
    pub fn wraps(self) -> bool { matches!(self, Mode::Endless) }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test types`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add src/game/types.rs src/game/mod.rs
git commit -m "feat(game): add grid types and direction primitives"
```

---

### Task 4: Snake (`game/snake.rs`)

The tail-vacates rule is the subtle one: entering the cell the tail is about to leave is legal when not growing.

**Files:**
- Create: `src/game/snake.rs`

**Interfaces:**
- Produces: `Snake::new(head: Pos, len: usize, dir: Direction)`, `head()`, `len()`, `dir()`, `contains(Pos) -> bool`, `iter()`, `grow(n: u32)`, `step(dir: Direction, wrap: bool) -> StepOutcome`, `enum StepOutcome { Moved, HitWall, HitSelf }`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::types::*;

    fn snake() -> Snake { Snake::new(Pos::new(9, 9), 4, Direction::Right) }

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
    fn wrapping_covers_all_four_edges() {
        let mut up = Snake::new(Pos::new(4, 0), 2, Direction::Up);
        assert_eq!(up.step(Direction::Up, true), StepOutcome::Moved);
        assert_eq!(up.head(), Pos::new(4, GRID_H - 1));

        let mut left = Snake::new(Pos::new(0, 4), 2, Direction::Left);
        assert_eq!(left.step(Direction::Left, true), StepOutcome::Moved);
        assert_eq!(left.head(), Pos::new(GRID_W - 1, 4));
    }

    #[test]
    fn entering_the_vacating_tail_cell_is_legal() {
        // A 4-long snake turned into a tight box returns to where the tail is
        // leaving on the same tick. That must not be a collision.
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test snake`
Expected: FAIL — `Snake` not found.

- [ ] **Step 3: Implement**

```rust
use std::collections::VecDeque;
use crate::game::types::{Direction, Pos, GRID_H, GRID_W};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum StepOutcome { Moved, HitWall, HitSelf }

pub struct Snake { body: VecDeque<Pos>, dir: Direction, grow: u32 }

impl Snake {
    /// `body[0]` is the head; the rest trails behind opposite `dir`.
    pub fn new(head: Pos, len: usize, dir: Direction) -> Self {
        let (dx, dy) = dir.delta();
        let mut body = VecDeque::with_capacity(len.max(8));
        for i in 0..len as i32 {
            body.push_back(Pos::new(head.x - dx * i, head.y - dy * i));
        }
        Snake { body, dir, grow: 0 }
    }

    pub fn head(&self) -> Pos { self.body[0] }
    pub fn len(&self) -> usize { self.body.len() }
    pub fn dir(&self) -> Direction { self.dir }
    pub fn iter(&self) -> impl Iterator<Item = &Pos> { self.body.iter() }
    pub fn contains(&self, p: Pos) -> bool { self.body.contains(&p) }
    pub fn grow(&mut self, n: u32) { self.grow += n; }

    pub fn step(&mut self, dir: Direction, wrap: bool) -> StepOutcome {
        self.dir = dir;
        let (dx, dy) = dir.delta();
        let mut nx = self.head().x + dx;
        let mut ny = self.head().y + dy;

        if nx < 0 || nx >= GRID_W || ny < 0 || ny >= GRID_H {
            if !wrap { return StepOutcome::HitWall; }
            nx = nx.rem_euclid(GRID_W);
            ny = ny.rem_euclid(GRID_H);
        }
        let next = Pos::new(nx, ny);

        // The tail vacates this tick unless we are growing, so it is not an
        // obstacle in that case.
        let growing = self.grow > 0;
        let occupied = if growing {
            self.body.iter().any(|&p| p == next)
        } else {
            let n = self.body.len();
            self.body.iter().take(n - 1).any(|&p| p == next)
        };
        if occupied { return StepOutcome::HitSelf; }

        self.body.push_front(next);
        if growing { self.grow -= 1; } else { self.body.pop_back(); }
        StepOutcome::Moved
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test snake`
Expected: 9 passed.

- [ ] **Step 5: Commit**

```bash
git add src/game/snake.rs src/game/mod.rs
git commit -m "feat(game): add snake body with tail-vacates collision rule"
```

---

### Task 5: Food (`game/food.rs`)

Enumerate free cells and index into them. Rejection sampling would hang on a full board and would make the daily sequence depend on rejection count.

**Files:**
- Create: `src/game/food.rs`

**Interfaces:**
- Produces: `spawn(snake: &Snake, exclude: Option<Pos>, rng: &mut Pcg32) -> Option<Pos>` — `None` means the board is full.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::rng::Pcg32;
    use crate::game::snake::Snake;
    use crate::game::types::*;

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
        let s = Snake::new(Pos::new(0, 0), (GRID_W * GRID_H) as usize, Direction::Right);
        let mut r = Pcg32::new(4);
        assert_eq!(spawn(&s, None, &mut r), None);
    }

    #[test]
    fn the_same_seed_yields_the_same_sequence() {
        let s = Snake::new(Pos::new(9, 9), 4, Direction::Right);
        let a: Vec<Pos> = { let mut r = Pcg32::new(77); (0..20).map(|_| spawn(&s, None, &mut r).unwrap()).collect() };
        let b: Vec<Pos> = { let mut r = Pcg32::new(77); (0..20).map(|_| spawn(&s, None, &mut r).unwrap()).collect() };
        assert_eq!(a, b);
    }
}
```

Note: the full-board test relies on `Snake::new` laying a body longer than the grid; it wraps negative coordinates out of range, so instead build the occupancy directly — see the implementation note in Step 3.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test food`
Expected: FAIL — `spawn` not found.

- [ ] **Step 3: Implement**

```rust
use crate::game::rng::Pcg32;
use crate::game::snake::Snake;
use crate::game::types::{Pos, GRID_H, GRID_W};

/// Picks a uniformly random free cell. Returns `None` when the board is full,
/// which the caller turns into the win state.
pub fn spawn(snake: &Snake, exclude: Option<Pos>, rng: &mut Pcg32) -> Option<Pos> {
    let mut free: Vec<Pos> = Vec::with_capacity((GRID_W * GRID_H) as usize);
    for y in 0..GRID_H {
        for x in 0..GRID_W {
            let p = Pos::new(x, y);
            if snake.contains(p) { continue; }
            if Some(p) == exclude { continue; }
            free.push(p);
        }
    }
    if free.is_empty() { return None; }
    Some(free[rng.below(free.len() as u32) as usize])
}
```

For the full-board test, construct the snake from an explicit body covering
every cell rather than `Snake::new`; add this helper to `snake.rs` behind
`#[cfg(test)]`:

```rust
#[cfg(test)]
impl Snake {
    pub fn from_cells(cells: Vec<Pos>, dir: Direction) -> Self {
        Snake { body: cells.into(), dir, grow: 0 }
    }
}
```

and build it in the test with:

```rust
let all: Vec<Pos> = (0..GRID_H).flat_map(|y| (0..GRID_W).map(move |x| Pos::new(x, y))).collect();
let s = Snake::from_cells(all, Direction::Right);
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test food`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add src/game/food.rs src/game/snake.rs src/game/mod.rs
git commit -m "feat(game): add free-cell food spawning with full-board terminal case"
```

---

### Task 6: Speed curve (`game/score.rs`)

**Files:**
- Create: `src/game/score.rs`

**Interfaces:**
- Produces: `tick_ms(normal_food_eaten: u32) -> f32`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_the_base_rate() {
        assert!((tick_ms(0) - 140.0).abs() < 1e-3);
    }

    #[test]
    fn is_monotonically_faster() {
        for n in 0..300 { assert!(tick_ms(n + 1) <= tick_ms(n)); }
    }

    #[test]
    fn clamps_at_the_floor() {
        assert!((tick_ms(10_000) - 55.0).abs() < 1e-6);
        for n in 0..2000 { assert!(tick_ms(n) >= 55.0); }
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test score`
Expected: FAIL.

- [ ] **Step 3: Implement**

```rust
pub const TICK_BASE_MS: f32 = 140.0;
pub const TICK_MIN_MS: f32 = 55.0;
pub const TICK_DECAY: f32 = 0.985;

pub fn tick_ms(normal_food_eaten: u32) -> f32 {
    (TICK_BASE_MS * TICK_DECAY.powi(normal_food_eaten as i32))
        .clamp(TICK_MIN_MS, TICK_BASE_MS)
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test score`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add src/game/score.rs src/game/mod.rs
git commit -m "feat(game): add speed curve"
```

---

### Task 7: Direction queue (`input.rs`, logic half)

The depth-2 queue is what makes fast corners register. The reversal check must be against the **queue tail**, not the applied direction — otherwise Right-applied + Up-queued accepts Down and kills the player next tick.

**Files:**
- Create: `src/input.rs`

**Interfaces:**
- Produces: `enum Action { Turn(Direction), Start, Pause, Restart, Quit }`, `DirQueue::new(initial: Direction)`, `push(Direction)`, `pop() -> Direction`, `applied() -> Direction`, `len() -> usize`.

- [ ] **Step 1: Write the failing tests**

```rust
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
        // Right applied, Up queued. Down does not reverse Right, but it does
        // reverse Up — accepting it would self-collide on the next tick.
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test input`
Expected: FAIL.

- [ ] **Step 3: Implement**

```rust
use std::collections::VecDeque;
use crate::game::types::Direction;

pub const QUEUE_CAP: usize = 2;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Action { Turn(Direction), Start, Pause, Restart, Quit }

pub struct DirQueue { applied: Direction, q: VecDeque<Direction> }

impl DirQueue {
    pub fn new(initial: Direction) -> Self {
        DirQueue { applied: initial, q: VecDeque::with_capacity(QUEUE_CAP) }
    }

    pub fn applied(&self) -> Direction { self.applied }
    pub fn len(&self) -> usize { self.q.len() }

    /// The direction a new press is validated against: the last queued turn if
    /// there is one, otherwise the direction currently being travelled.
    fn effective(&self) -> Direction {
        *self.q.back().unwrap_or(&self.applied)
    }

    pub fn push(&mut self, d: Direction) {
        let eff = self.effective();
        if d == eff || d == eff.opposite() { return; }
        if self.q.len() >= QUEUE_CAP { return; }
        self.q.push_back(d);
    }

    pub fn pop(&mut self) -> Direction {
        if let Some(d) = self.q.pop_front() { self.applied = d; }
        self.applied
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test input`
Expected: 7 passed.

- [ ] **Step 5: Commit**

```bash
git add src/input.rs src/main.rs
git commit -m "feat(input): add depth-2 direction queue with queue-tail reversal rule"
```

---

### Task 8: Game loop logic (`game/mod.rs`)

**Files:**
- Modify: `src/game/mod.rs`

**Interfaces:**
- Produces: `enum GameState { AwaitingStart, Running, Dead, Won }`, `Game::new(mode: Mode, seed: u64)`, `Game::advance(&mut self, dt: f32, queue: &mut DirQueue)`, `Game::tick_fraction() -> f32`, and public fields `state`, `score`, `normal_food_eaten`, `elapsed`, plus `snake()` and `food()` accessors.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::DirQueue;
    use crate::game::types::Direction;

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
        g.advance(10.0, &mut q);
        g.advance(10.0, &mut q);
        g.advance(10.0, &mut q);
        g.advance(10.0, &mut q);
        assert_eq!(g.state, GameState::Dead);
    }

    #[test]
    fn eating_scores_grows_and_speeds_up() {
        let mut g = Game::new(Mode::Classic, 1);
        let mut q = DirQueue::new(Direction::Right);
        g.start();
        g.force_food_at(Pos::new(10, 9));
        g.advance(0.140, &mut q);
        assert_eq!(g.score, 10);
        assert_eq!(g.normal_food_eaten, 1);
        g.advance(0.140, &mut q);
        assert_eq!(g.snake().len(), 5);
    }

    #[test]
    fn food_never_spawns_under_the_snake() {
        let mut g = Game::new(Mode::Classic, 5);
        assert!(!g.snake().contains(g.food()));
        let mut q = DirQueue::new(Direction::Right);
        g.start();
        for _ in 0..40 { g.advance(0.140, &mut q); if g.state != GameState::Running { break; } }
    }

    #[test]
    fn the_same_seed_replays_identically() {
        let run = |seed: u64| {
            let mut g = Game::new(Mode::Classic, seed);
            let mut q = DirQueue::new(Direction::Right);
            g.start();
            let mut path = vec![];
            for _ in 0..12 { g.advance(0.140, &mut q); path.push(g.food()); }
            path
        };
        assert_eq!(run(1234), run(1234));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test game::tests`
Expected: FAIL.

- [ ] **Step 3: Implement**

```rust
pub mod food;
pub mod rng;
pub mod score;
pub mod snake;
pub mod types;

use crate::input::DirQueue;
use rng::Pcg32;
use snake::{Snake, StepOutcome};
pub use types::{Direction, Mode, Pos, GRID_H, GRID_W};

pub const START: Pos = Pos { x: 9, y: 9 };
pub const START_LEN: usize = 4;
pub const START_DIR: Direction = Direction::Right;
pub const MAX_TICKS_PER_FRAME: u32 = 5;
pub const FOOD_SCORE: u32 = 10;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum GameState { AwaitingStart, Running, Dead, Won }

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
        let food = food::spawn(&snake, None, &mut rng).expect("empty board at start");
        Game {
            mode, snake, food, rng,
            acc: 0.0,
            tick_ms: score::tick_ms(0),
            state: GameState::AwaitingStart,
            score: 0,
            normal_food_eaten: 0,
            elapsed: 0.0,
        }
    }

    pub fn snake(&self) -> &Snake { &self.snake }
    pub fn food(&self) -> Pos { self.food }
    pub fn tick_ms(&self) -> f32 { self.tick_ms }

    pub fn start(&mut self) {
        if self.state == GameState::AwaitingStart { self.state = GameState::Running; }
    }

    /// Fraction of the way to the next tick, for render interpolation.
    pub fn tick_fraction(&self) -> f32 {
        (self.acc / (self.tick_ms / 1000.0)).clamp(0.0, 1.0)
    }

    pub fn advance(&mut self, dt: f32, queue: &mut DirQueue) {
        if self.state != GameState::Running { return; }
        self.elapsed += dt;

        let tick_dt = self.tick_ms / 1000.0;
        // Clamp so a stall (window drag, sleep, breakpoint) cannot teleport the
        // snake through the arena.
        self.acc = (self.acc + dt).min(MAX_TICKS_PER_FRAME as f32 * tick_dt);

        while self.acc >= self.tick_ms / 1000.0 {
            self.acc -= self.tick_ms / 1000.0;
            self.tick(queue);
            if self.state != GameState::Running { self.acc = 0.0; return; }
            // Recompute only at a tick boundary, so the interpolation fraction
            // never divides a partly-filled accumulator by a new tick length.
            self.tick_ms = score::tick_ms(self.normal_food_eaten);
        }
    }

    fn tick(&mut self, queue: &mut DirQueue) {
        let dir = queue.pop();
        match self.snake.step(dir, self.mode.wraps()) {
            StepOutcome::HitWall | StepOutcome::HitSelf => { self.state = GameState::Dead; }
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
    pub fn force_food_at(&mut self, p: Pos) { self.food = p; }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/game/mod.rs
git commit -m "feat(game): add tick loop, accumulator clamp, and Classic rules"
```

---

### Task 9: Color (`render/color.rs`)

**Files:**
- Create: `src/render/color.rs`, `src/render/mod.rs`

**Interfaces:**
- Produces: `srgb_decode(f32) -> f32`, `srgb_encode(f32) -> f32`, `tone_map(f32) -> f32`, `to_u8(f32) -> u8` (5-bit snapped), `nearest_256([u8;3]) -> u8`, `Rgb = [f32; 3]`, `rgb_hex(u32) -> Rgb` (decodes to linear).

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_round_trips() {
        for i in 0..=255u32 {
            let v = i as f32 / 255.0;
            assert!((srgb_encode(srgb_decode(v)) - v).abs() < 1e-4, "failed at {i}");
        }
    }

    #[test]
    fn tone_map_reaches_exactly_one_so_the_death_flash_is_white() {
        assert_eq!(tone_map(1.0), 1.0);
        assert!(tone_map(4.0) <= 1.0);
        assert!((tone_map(0.5) - 0.5).abs() < 1e-6, "linear below the knee");
    }

    #[test]
    fn to_u8_snaps_to_five_bits() {
        // 32 distinct levels, so the diff renderer has something to skip.
        let levels: std::collections::BTreeSet<u8> =
            (0..=255u32).map(|i| to_u8(i as f32 / 255.0)).collect();
        assert_eq!(levels.len(), 32);
        assert_eq!(to_u8(0.0), 0);
        assert_eq!(to_u8(1.0), 255);
    }

    #[test]
    fn nearest_256_maps_greys_and_primaries() {
        assert_eq!(nearest_256([0, 0, 0]), 16);
        assert_eq!(nearest_256([255, 255, 255]), 231);
        assert_eq!(nearest_256([255, 0, 0]), 196);
    }

    #[test]
    fn rgb_hex_decodes_to_linear() {
        let white = rgb_hex(0xffffff);
        assert!((white[0] - 1.0).abs() < 1e-4);
        let mid = rgb_hex(0x808080);
        assert!(mid[0] < 0.3, "sRGB 0.5 is about 0.21 in linear, got {}", mid[0]);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test color`
Expected: FAIL.

- [ ] **Step 3: Implement**

```rust
pub type Rgb = [f32; 3];

pub fn srgb_decode(v: f32) -> f32 {
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

pub fn srgb_encode(v: f32) -> f32 {
    if v <= 0.0031308 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}

/// Linear below the knee, then a curve that lands exactly on 1.0 so a full
/// white flash is actually white (Reinhard would asymptote to grey).
pub const KNEE: f32 = 0.8;

pub fn tone_map(v: f32) -> f32 {
    if v <= KNEE { v } else {
        let over = v - KNEE;
        (KNEE + (1.0 - KNEE) * (over / (over + (1.0 - KNEE)))).min(1.0)
    }
}

/// Quantize to 5 bits per channel. Visually indistinguishable in a terminal,
/// but it lets ratatui's diff skip slowly-decaying trail pixels.
pub fn to_u8(v: f32) -> u8 {
    let q = (v.clamp(0.0, 1.0) * 31.0 + 0.5) as u32;
    ((q * 255) / 31) as u8
}

pub fn rgb_hex(hex: u32) -> Rgb {
    [
        srgb_decode(((hex >> 16) & 0xff) as f32 / 255.0),
        srgb_decode(((hex >> 8) & 0xff) as f32 / 255.0),
        srgb_decode((hex & 0xff) as f32 / 255.0),
    ]
}

const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];

fn cube_index(v: u8) -> (usize, i32) {
    let mut best = 0usize;
    let mut best_err = i32::MAX;
    for (i, &c) in CUBE.iter().enumerate() {
        let e = (c as i32 - v as i32).abs();
        if e < best_err { best_err = e; best = i; }
    }
    (best, best_err)
}

/// Nearest xterm-256 index: 6x6x6 colour cube (16..231) or the 24-step grey
/// ramp (232..255), whichever is closer. Used on 256-colour terminals such as
/// macOS Terminal.app.
pub fn nearest_256(c: [u8; 3]) -> u8 {
    let (ri, _) = cube_index(c[0]);
    let (gi, _) = cube_index(c[1]);
    let (bi, _) = cube_index(c[2]);
    let cube = [CUBE[ri], CUBE[gi], CUBE[bi]];
    let cube_err = dist2(c, cube);

    let avg = ((c[0] as u32 + c[1] as u32 + c[2] as u32) / 3) as i32;
    let gi = (((avg - 8) as f32 / 10.0).round() as i32).clamp(0, 23);
    let g = (8 + gi * 10) as u8;
    let grey_err = dist2(c, [g, g, g]);

    if grey_err < cube_err { (232 + gi) as u8 }
    else { (16 + 36 * ri + 6 * gi_cube(gi_of(c[1])) + bi) as u8 }
}
```

Note for the implementer: the final index expression above is deliberately
written out in full during implementation as
`(16 + 36 * ri + 6 * gi + bi) as u8` using the cube indices `ri`, `gi`, `bi`
computed at the top; shadowing `gi` with the grey index is a bug. Rename the
grey index to `grey_i` when implementing.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test color`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add src/render/
git commit -m "feat(render): add linear-light colour, tone map, and 256-colour fallback"
```

---

### Task 10: Canvas and quantizer (`render/canvas.rs`)

**Files:**
- Create: `src/render/canvas.rs`

**Interfaces:**
- Produces: `Canvas::new(w: usize, h: usize)`, `clear(Rgb)`, `set(x: i32, y: i32, Rgb)`, `blend(x: i32, y: i32, Rgb, cov: f32)`, `add(x: i32, y: i32, Rgb)`, `quantize_into(&self, buf: &mut Buffer, origin: (u16, u16), tier: ColorTier)`, `enum ColorTier { Full, Reduced }`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::Rect;
    use ratatui_core::style::Color;

    fn buf(w: u16, h: u16) -> Buffer { Buffer::empty(Rect::new(0, 0, w, h)) }

    #[test]
    fn two_different_pixels_become_an_upper_half_block() {
        let mut c = Canvas::new(1, 2);
        c.set(0, 0, [1.0, 0.0, 0.0]);
        c.set(0, 1, [0.0, 0.0, 1.0]);
        let mut b = buf(1, 1);
        c.quantize_into(&mut b, (0, 0), ColorTier::Full);
        let cell = &b[(0, 0)];
        assert_eq!(cell.symbol(), "▀");
        assert_eq!(cell.fg, Color::Rgb(255, 0, 0));
        assert_eq!(cell.bg, Color::Rgb(0, 0, 255));
    }

    #[test]
    fn an_equal_pair_collapses_to_a_space_with_reset_fg() {
        let mut c = Canvas::new(1, 2);
        c.set(0, 0, [0.25, 0.25, 0.25]);
        c.set(0, 1, [0.25, 0.25, 0.25]);
        let mut b = buf(1, 1);
        c.quantize_into(&mut b, (0, 0), ColorTier::Full);
        let cell = &b[(0, 0)];
        assert_eq!(cell.symbol(), " ");
        assert_eq!(cell.fg, Color::Reset);
        assert!(matches!(cell.bg, Color::Rgb(..)));
    }

    #[test]
    fn near_equal_pixels_that_quantize_alike_also_collapse() {
        // The comparison must happen after quantizing to u8, not on the floats.
        let mut c = Canvas::new(1, 2);
        c.set(0, 0, [0.5000, 0.5, 0.5]);
        c.set(0, 1, [0.5001, 0.5, 0.5]);
        let mut b = buf(1, 1);
        c.quantize_into(&mut b, (0, 0), ColorTier::Full);
        assert_eq!(b[(0, 0)].symbol(), " ");
    }

    #[test]
    fn reduced_tier_emits_indexed_colour() {
        let mut c = Canvas::new(1, 2);
        c.set(0, 0, [1.0, 0.0, 0.0]);
        c.set(0, 1, [0.0, 0.0, 0.0]);
        let mut b = buf(1, 1);
        c.quantize_into(&mut b, (0, 0), ColorTier::Reduced);
        assert_eq!(b[(0, 0)].fg, Color::Indexed(196));
        assert_eq!(b[(0, 0)].bg, Color::Indexed(16));
    }

    #[test]
    fn writes_land_at_the_given_origin() {
        let mut c = Canvas::new(1, 2);
        c.set(0, 0, [1.0, 1.0, 1.0]);
        c.set(0, 1, [0.0, 0.0, 0.0]);
        let mut b = buf(4, 4);
        c.quantize_into(&mut b, (2, 3), ColorTier::Full);
        assert_eq!(b[(2, 3)].symbol(), "▀");
        assert_eq!(b[(0, 0)].symbol(), " ");
    }

    #[test]
    fn out_of_bounds_writes_are_ignored_not_panics() {
        let mut c = Canvas::new(2, 2);
        c.set(-1, 0, [1.0, 0.0, 0.0]);
        c.set(99, 0, [1.0, 0.0, 0.0]);
        c.blend(0, -5, [1.0, 0.0, 0.0], 1.0);
    }

    #[test]
    fn blend_interpolates_by_coverage() {
        let mut c = Canvas::new(1, 1);
        c.set(0, 0, [0.0, 0.0, 0.0]);
        c.blend(0, 0, [1.0, 1.0, 1.0], 0.5);
        assert!((c.get(0, 0)[0] - 0.5).abs() < 1e-6);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test canvas`
Expected: FAIL.

- [ ] **Step 3: Implement**

```rust
use ratatui_core::buffer::Buffer;
use ratatui_core::style::Color;
use crate::render::color::{nearest_256, srgb_encode, to_u8, tone_map, Rgb};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ColorTier { Full, Reduced }

pub struct Canvas { w: usize, h: usize, px: Vec<Rgb> }

impl Canvas {
    pub fn new(w: usize, h: usize) -> Self {
        Canvas { w, h, px: vec![[0.0; 3]; w * h] }
    }

    pub fn width(&self) -> usize { self.w }
    pub fn height(&self) -> usize { self.h }

    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h { None }
        else { Some(y as usize * self.w + x as usize) }
    }

    pub fn clear(&mut self, c: Rgb) { self.px.iter_mut().for_each(|p| *p = c); }

    pub fn get(&self, x: i32, y: i32) -> Rgb {
        self.idx(x, y).map(|i| self.px[i]).unwrap_or([0.0; 3])
    }

    pub fn set(&mut self, x: i32, y: i32, c: Rgb) {
        if let Some(i) = self.idx(x, y) { self.px[i] = c; }
    }

    pub fn add(&mut self, x: i32, y: i32, c: Rgb) {
        if let Some(i) = self.idx(x, y) {
            for k in 0..3 { self.px[i][k] += c[k]; }
        }
    }

    pub fn blend(&mut self, x: i32, y: i32, c: Rgb, cov: f32) {
        if let Some(i) = self.idx(x, y) {
            let a = cov.clamp(0.0, 1.0);
            for k in 0..3 { self.px[i][k] = self.px[i][k] * (1.0 - a) + c[k] * a; }
        }
    }

    fn encode(&self, p: Rgb) -> [u8; 3] {
        [
            to_u8(srgb_encode(tone_map(p[0].max(0.0)))),
            to_u8(srgb_encode(tone_map(p[1].max(0.0)))),
            to_u8(srgb_encode(tone_map(p[2].max(0.0)))),
        ]
    }

    /// Writes the canvas into `buf` as half-block cells, two pixel rows per
    /// terminal row, starting at `origin` = (col, row).
    pub fn quantize_into(&self, buf: &mut Buffer, origin: (u16, u16), tier: ColorTier) {
        let rows = self.h / 2;
        for cy in 0..rows {
            for cx in 0..self.w {
                let top = self.encode(self.px[(cy * 2) * self.w + cx]);
                let bot = self.encode(self.px[(cy * 2 + 1) * self.w + cx]);

                let col = origin.0 as usize + cx;
                let row = origin.1 as usize + cy;
                if col > u16::MAX as usize || row > u16::MAX as usize { continue; }
                let Some(cell) = buf.cell_mut((col as u16, row as u16)) else { continue };

                let paint = |c: [u8; 3]| match tier {
                    ColorTier::Full => Color::Rgb(c[0], c[1], c[2]),
                    ColorTier::Reduced => Color::Indexed(nearest_256(c)),
                };

                if top == bot {
                    // Reset (rather than fg = bg) lets the backend skip the
                    // foreground SGR across runs of flat backdrop.
                    cell.set_char(' ').set_fg(Color::Reset).set_bg(paint(bot));
                } else {
                    cell.set_char('▀').set_fg(paint(top)).set_bg(paint(bot));
                }
            }
        }
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test canvas`
Expected: 7 passed.

- [ ] **Step 5: Commit**

```bash
git add src/render/canvas.rs src/render/mod.rs
git commit -m "feat(render): add f32 pixel canvas with half-block quantizer"
```

---

### Task 11: Layout (`render/layout.rs`)

**Files:**
- Create: `src/render/layout.rs`

**Interfaces:**
- Produces: `MIN_COLS: u16 = 86`, `MIN_ROWS: u16 = 31`, `DEFAULT_MAX_SCALE: u32 = 4`, `struct Layout { scale: u32, canvas_w: usize, canvas_h: usize, origin_col: u16, origin_row: u16 }`, `Layout::compute(cols: u16, rows: u16, s_max: u32) -> Option<Layout>`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn below_the_minimum_there_is_no_layout() {
        assert!(Layout::compute(85, 31, 4).is_none());
        assert!(Layout::compute(86, 30, 4).is_none());
        assert!(Layout::compute(20, 10, 4).is_none());
    }

    #[test]
    fn the_documented_minimum_yields_scale_three() {
        let l = Layout::compute(86, 31, 4).unwrap();
        assert_eq!(l.scale, 3);
        assert_eq!(l.canvas_w, 28 * 3 + 2);
        assert_eq!(l.canvas_h, 18 * 3 + 2);
    }

    #[test]
    fn the_canvas_always_fits_the_terminal() {
        for cols in 86..200u16 {
            for rows in 31..80u16 {
                let l = Layout::compute(cols, rows, 6).unwrap();
                assert!(l.canvas_w <= cols as usize, "{cols}x{rows} overflowed width");
                let used_rows = l.canvas_h.div_ceil(2) + CHROME_ROWS as usize;
                assert!(used_rows <= rows as usize, "{cols}x{rows} overflowed height");
            }
        }
    }

    #[test]
    fn scale_is_capped_by_the_argument() {
        let l = Layout::compute(400, 200, 4).unwrap();
        assert_eq!(l.scale, 4);
        let l6 = Layout::compute(400, 200, 6).unwrap();
        assert_eq!(l6.scale, 6);
    }

    #[test]
    fn the_arena_is_centred() {
        let l = Layout::compute(200, 80, 4).unwrap();
        let slack = 200 - l.canvas_w as u16;
        assert_eq!(l.origin_col, slack / 2);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test layout`
Expected: FAIL.

- [ ] **Step 3: Implement**

```rust
use crate::game::types::{GRID_H, GRID_W};

pub const MIN_COLS: u16 = 86;
pub const MIN_ROWS: u16 = 31;
pub const CHROME_ROWS: u16 = 3; // 2 HUD rows + 1 hint row
pub const BORDER_PX: usize = 2; // one pixel each side
pub const MIN_SCALE: u32 = 3;
pub const DEFAULT_MAX_SCALE: u32 = 4;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    pub scale: u32,
    pub canvas_w: usize,
    pub canvas_h: usize,
    pub origin_col: u16,
    pub origin_row: u16,
}

impl Layout {
    pub fn compute(cols: u16, rows: u16, s_max: u32) -> Option<Layout> {
        if cols < MIN_COLS || rows < MIN_ROWS { return None; }

        let avail_px_w = (cols as usize).saturating_sub(BORDER_PX);
        let avail_px_h = ((rows - CHROME_ROWS) as usize * 2).saturating_sub(BORDER_PX);

        let by_w = avail_px_w / GRID_W as usize;
        let by_h = avail_px_h / GRID_H as usize;
        let scale = (by_w.min(by_h) as u32).clamp(MIN_SCALE, s_max.max(MIN_SCALE));

        let canvas_w = GRID_W as usize * scale as usize + BORDER_PX;
        let canvas_h = GRID_H as usize * scale as usize + BORDER_PX;

        let used_rows = canvas_h.div_ceil(2) as u16 + CHROME_ROWS;
        if canvas_w > cols as usize || used_rows > rows { return None; }

        let origin_col = (cols - canvas_w as u16) / 2;
        let origin_row = CHROME_ROWS - 1 + (rows - used_rows) / 2;

        Some(Layout { scale, canvas_w, canvas_h, origin_col, origin_row })
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test layout`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add src/render/layout.rs src/render/mod.rs
git commit -m "feat(render): add adaptive scale layout with minimum-size gate"
```

---

### Task 12: Capability tier (`render/tier.rs`)

macOS Terminal.app is the case that matters — it has never supported 24-bit colour.

**Files:**
- Create: `src/render/tier.rs`

**Interfaces:**
- Produces: `detect(colorterm: Option<&str>, term: Option<&str>, term_program: Option<&str>, wt: bool) -> Tier`, `enum Tier { Full, Reduced, Refused }`, `fn suppress_sync(term: Option<&str>, tmux: bool) -> bool`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorterm_truecolor_is_full() {
        assert_eq!(detect(Some("truecolor"), None, None, false), Tier::Full);
        assert_eq!(detect(Some("24bit"), None, None, false), Tier::Full);
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
        for p in ["iTerm.app", "WezTerm", "ghostty"] {
            assert_eq!(detect(None, Some("xterm-256color"), Some(p), false), Tier::Full, "{p}");
        }
    }

    #[test]
    fn plain_256color_is_reduced_and_bare_terminals_are_refused() {
        assert_eq!(detect(None, Some("xterm-256color"), None, false), Tier::Reduced);
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
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test tier`
Expected: FAIL.

- [ ] **Step 3: Implement**

```rust
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Tier { Full, Reduced, Refused }

const TRUECOLOR_PROGRAMS: [&str; 6] =
    ["iTerm.app", "WezTerm", "ghostty", "alacritty", "kitty", "vscode"];

pub fn detect(
    colorterm: Option<&str>,
    term: Option<&str>,
    term_program: Option<&str>,
    windows_terminal: bool,
) -> Tier {
    if let Some(ct) = colorterm {
        let ct = ct.to_ascii_lowercase();
        if ct.contains("truecolor") || ct.contains("24bit") { return Tier::Full; }
    }
    if windows_terminal { return Tier::Full; }

    if let Some(p) = term_program {
        if TRUECOLOR_PROGRAMS.iter().any(|k| k.eq_ignore_ascii_case(p)) { return Tier::Full; }
    }

    match term {
        Some(t) if t.contains("direct") => Tier::Full,
        Some(t) if t.contains("256color") => Tier::Reduced,
        _ => Tier::Refused,
    }
}

/// tmux and screen do not pass DEC 2026 through by default, so the escape is
/// suppressed rather than emitted blind.
pub fn suppress_sync(term: Option<&str>, tmux_env: bool) -> bool {
    if tmux_env { return true; }
    matches!(term, Some(t) if t.starts_with("tmux") || t.starts_with("screen"))
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test tier`
Expected: 7 passed.

- [ ] **Step 5: Commit**

```bash
git add src/render/tier.rs src/render/mod.rs
git commit -m "feat(render): detect truecolor capability with 256-colour fallback tier"
```

---

### Task 13: Arena drawing and HUD (`render/arena.rs`, `ui/hud.rs`)

Phase 1 draws flat colour. The SDF ribbon, glow, and particles arrive in Phase 2.

**Files:**
- Create: `src/render/arena.rs`, `src/ui/hud.rs`, `src/ui/mod.rs`, `src/ui/resize.rs`

**Interfaces:**
- Produces: `draw_arena(canvas: &mut Canvas, game: &Game, layout: &Layout)`, `render_hud(buf: &mut Buffer, area: Rect, game: &Game)`, `render_resize(buf: &mut Buffer, area: Rect, cols: u16, rows: u16)`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Game, Mode};
    use crate::render::layout::Layout;

    #[test]
    fn the_snake_occupies_its_cells_and_the_background_stays_dark() {
        let g = Game::new(Mode::Classic, 1);
        let l = Layout::compute(120, 40, 4).unwrap();
        let mut c = Canvas::new(l.canvas_w, l.canvas_h);
        draw_arena(&mut c, &g, &l);

        let head = g.snake().head();
        let s = l.scale as i32;
        let px = 1 + head.x * s + s / 2;
        let py = 1 + head.y * s + s / 2;
        assert!(c.get(px, py)[1] > 0.05, "head cell should be lit");

        // A cell far from the snake and the food stays near background.
        let empty = c.get(1 + 27 * s, 1 + 17 * s);
        assert!(empty.iter().sum::<f32>() < 0.3);
    }

    #[test]
    fn the_border_is_drawn_on_every_edge() {
        let g = Game::new(Mode::Classic, 1);
        let l = Layout::compute(120, 40, 4).unwrap();
        let mut c = Canvas::new(l.canvas_w, l.canvas_h);
        draw_arena(&mut c, &g, &l);
        for x in 0..l.canvas_w as i32 {
            assert!(c.get(x, 0).iter().sum::<f32>() > 0.0, "top border gap at {x}");
            assert!(c.get(x, l.canvas_h as i32 - 1).iter().sum::<f32>() > 0.0);
        }
    }

    #[test]
    fn the_food_cell_is_lit() {
        let g = Game::new(Mode::Classic, 9);
        let l = Layout::compute(120, 40, 4).unwrap();
        let mut c = Canvas::new(l.canvas_w, l.canvas_h);
        draw_arena(&mut c, &g, &l);
        let s = l.scale as i32;
        let f = g.food();
        assert!(c.get(1 + f.x * s + s / 2, 1 + f.y * s + s / 2)[0] > 0.1);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test arena`
Expected: FAIL.

- [ ] **Step 3: Implement**

```rust
// src/render/arena.rs
use crate::game::{Game, GameState};
use crate::render::canvas::Canvas;
use crate::render::color::{rgb_hex, Rgb};
use crate::render::layout::Layout;

const BG: u32 = 0x0b0f14;
const BORDER: u32 = 0x1e2a38;
const BODY: u32 = 0x27c26b;
const HEAD: u32 = 0x8affc1;
const FOOD: u32 = 0xff4d5a;

fn fill_cell(c: &mut Canvas, cell_x: i32, cell_y: i32, scale: i32, col: Rgb, inset: i32) {
    let ox = 1 + cell_x * scale;
    let oy = 1 + cell_y * scale;
    for dy in inset..(scale - inset) {
        for dx in inset..(scale - inset) {
            c.set(ox + dx, oy + dy, col);
        }
    }
}

pub fn draw_arena(c: &mut Canvas, game: &Game, layout: &Layout) {
    let s = layout.scale as i32;
    c.clear(rgb_hex(BG));

    let border = rgb_hex(BORDER);
    let w = c.width() as i32;
    let h = c.height() as i32;
    for x in 0..w {
        c.set(x, 0, border);
        c.set(x, h - 1, border);
    }
    for y in 0..h {
        c.set(0, y, border);
        c.set(w - 1, y, border);
    }

    fill_cell(c, game.food().x, game.food().y, s, rgb_hex(FOOD), if s >= 4 { 1 } else { 0 });

    let body = rgb_hex(BODY);
    let head = rgb_hex(HEAD);
    let dim = if game.state == GameState::Dead { 0.35 } else { 1.0 };
    for (i, p) in game.snake().iter().enumerate() {
        let col = if i == 0 { head } else { body };
        let col = [col[0] * dim, col[1] * dim, col[2] * dim];
        fill_cell(c, p.x, p.y, s, col, 0);
    }
}
```

```rust
// src/ui/hud.rs
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Style};
use ratatui_core::text::{Line, Span};
use ratatui_core::widgets::Widget;
use crate::game::{Game, GameState};

pub fn render_hud(buf: &mut Buffer, area: Rect, game: &Game) {
    let accent = Style::new().fg(Color::Rgb(0x8a, 0xff, 0xc1));
    let dim = Style::new().fg(Color::Rgb(0x6b, 0x7a, 0x8c));

    let line = Line::from(vec![
        Span::styled(" SCORE ", dim),
        Span::styled(format!("{:>5}", game.score), accent),
        Span::styled("   LEN ", dim),
        Span::styled(format!("{:>3}", game.snake().len()), accent),
        Span::styled("   TIME ", dim),
        Span::styled(format!("{:>5.1}s", game.elapsed), accent),
    ]);
    line.render(area, buf);
}

pub fn hint_for(state: GameState) -> &'static str {
    match state {
        GameState::AwaitingStart => " press an arrow key or WASD to start ",
        GameState::Running => " arrows / wasd  ·  esc pause  ·  q quit ",
        GameState::Dead => " r restart  ·  q quit ",
        GameState::Won => " you filled the board  ·  r restart  ·  q quit ",
    }
}
```

```rust
// src/ui/resize.rs
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Style};
use ratatui_core::text::Line;
use ratatui_core::widgets::Widget;
use crate::render::layout::{MIN_COLS, MIN_ROWS};

pub fn render_resize(buf: &mut Buffer, area: Rect, cols: u16, rows: u16) {
    let warn = Style::new().fg(Color::Rgb(0xff, 0x9f, 0x43));
    let dim = Style::new().fg(Color::Rgb(0x6b, 0x7a, 0x8c));
    let mid = area.height / 2;
    Line::styled(format!("  terminal too small: {cols}x{rows}"), warn)
        .render(Rect { y: area.y + mid.saturating_sub(1), height: 1, ..area }, buf);
    Line::styled(format!("  resize to at least {MIN_COLS}x{MIN_ROWS} to play"), dim)
        .render(Rect { y: area.y + mid, height: 1, ..area }, buf);
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/render/arena.rs src/ui/ src/render/mod.rs
git commit -m "feat(render): draw flat-colour arena, HUD, and resize screen"
```

---

### Task 14: App state machine (`app.rs`)

Pure — no loop, no I/O. The browser build reuses this file as-is.

**Files:**
- Create: `src/app.rs`

**Interfaces:**
- Produces: `App::new(seed: u64, s_max: u32)`, `update(&mut self, dt: f32, input: &[Action])`, `render(&mut self, buf: &mut Buffer)`, `should_quit() -> bool`, `set_tier(Tier)`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::types::Direction;
    use crate::input::Action;
    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::Rect;

    fn buf() -> Buffer { Buffer::empty(Rect::new(0, 0, 120, 40)) }

    #[test]
    fn a_turn_press_starts_a_waiting_game() {
        let mut a = App::new(1, 4);
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
    fn restart_after_death_produces_a_fresh_run() {
        let mut a = App::new(1, 4);
        a.update(0.0, &[Action::Turn(Direction::Right)]);
        for _ in 0..200 { a.update(0.05, &[]); }
        assert_eq!(a.game().state, GameState::Dead);
        a.update(0.0, &[Action::Restart]);
        assert_eq!(a.game().state, GameState::AwaitingStart);
        assert_eq!(a.game().score, 0);
    }

    #[test]
    fn rendering_a_large_buffer_fills_cells_without_panicking() {
        let mut a = App::new(1, 4);
        let mut b = buf();
        a.render(&mut b);
        let painted = b.content().iter().filter(|c| c.symbol() != " ").count();
        assert!(painted > 0);
    }

    #[test]
    fn a_small_buffer_renders_the_resize_screen_instead_of_panicking() {
        let mut a = App::new(1, 4);
        let mut b = Buffer::empty(Rect::new(0, 0, 40, 10));
        a.render(&mut b);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test app`
Expected: FAIL.

- [ ] **Step 3: Implement**

```rust
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::text::Line;
use ratatui_core::style::{Color, Style};
use ratatui_core::widgets::Widget;

use crate::game::types::Direction;
use crate::game::{Game, GameState, Mode};
use crate::input::{Action, DirQueue};
use crate::render::arena::draw_arena;
use crate::render::canvas::{Canvas, ColorTier};
use crate::render::layout::Layout;
use crate::render::tier::Tier;
use crate::ui::hud::{hint_for, render_hud};
use crate::ui::resize::render_resize;

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

    pub fn game(&self) -> &Game { &self.game }
    pub fn should_quit(&self) -> bool { self.quit }
    pub fn set_tier(&mut self, t: Tier) { self.tier = t; }

    fn restart(&mut self) {
        self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1);
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
                Action::Start | Action::Pause => {}
            }
        }
        self.game.advance(dt, &mut self.queue);
    }

    pub fn render(&mut self, buf: &mut Buffer) {
        let area = *buf.area();
        let Some(layout) = Layout::compute(area.width, area.height, self.s_max) else {
            render_resize(buf, area, area.width, area.height);
            self.layout = None;
            return;
        };

        if self.layout != Some(layout) {
            self.canvas = Canvas::new(layout.canvas_w, layout.canvas_h);
            self.layout = Some(layout);
        }

        draw_arena(&mut self.canvas, &self.game, &layout);
        let tier = match self.tier { Tier::Reduced => ColorTier::Reduced, _ => ColorTier::Full };
        self.canvas.quantize_into(buf, (layout.origin_col, layout.origin_row), tier);

        render_hud(buf, Rect { x: layout.origin_col, y: 0, width: layout.canvas_w as u16, height: 1 }, &self.game);

        let hint_row = layout.origin_row + layout.canvas_h.div_ceil(2) as u16;
        if hint_row < area.height {
            Line::styled(hint_for(self.game.state), Style::new().fg(Color::Rgb(0x6b, 0x7a, 0x8c)))
                .render(Rect { x: layout.origin_col, y: hint_row, width: layout.canvas_w as u16, height: 1 }, buf);
        }
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/app.rs src/main.rs
git commit -m "feat(app): add pure update/render state machine"
```

---

### Task 15: Terminal loop (`main.rs`)

The only file allowed to touch crossterm, stdout, or the clock.

**Files:**
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: everything above.

- [ ] **Step 1: Implement**

Key requirements, each from the spec:

- 512 KB `BufWriter` — **not** `ratatui::init()`, whose `LineWriter` splits a frame into dozens of console writes.
- DEC 2026 synchronized output around each draw, suppressed under tmux/screen.
- `poll(Duration::ZERO)` drain loop; pace with `sleep` against an `Instant` deadline. A real timeout on `poll` quantizes to the ~15.6 ms Windows timer tick.
- `KeyEventKind::Press` filter — Windows sends Press and Release for every key.
- Panic hook restores the terminal first.
- Refuse to run on a `Tier::Refused` terminal with a clear message.

```rust
mod app; mod game; mod input; mod render; mod ui;

use std::io::{BufWriter, Write};
use std::time::{Duration, Instant};

use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::crossterm::{execute, terminal};
use ratatui::Terminal;

use app::App;
use game::types::Direction;
use input::Action;
use render::layout::DEFAULT_MAX_SCALE;
use render::tier::{detect, suppress_sync, Tier};

const FRAME: Duration = Duration::from_micros(16_667);

fn env(k: &str) -> Option<String> { std::env::var(k).ok() }

fn map_key(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('W') => Some(Action::Turn(Direction::Up)),
        KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('S') => Some(Action::Turn(Direction::Down)),
        KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A') => Some(Action::Turn(Direction::Left)),
        KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D') => Some(Action::Turn(Direction::Right)),
        KeyCode::Char('r') | KeyCode::Char('R') => Some(Action::Restart),
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => Some(Action::Quit),
        _ => None,
    }
}

fn main() -> std::io::Result<()> {
    let tier = detect(
        env("COLORTERM").as_deref(),
        env("TERM").as_deref(),
        env("TERM_PROGRAM").as_deref(),
        env("WT_SESSION").is_some(),
    );
    if tier == Tier::Refused {
        eprintln!("terminal-snake needs a 256-colour or truecolor terminal.");
        eprintln!("Try Windows Terminal, iTerm2, WezTerm, Alacritty, kitty, or Ghostty.");
        return Ok(());
    }
    let sync = !suppress_sync(env("TERM").as_deref(), env("TMUX").is_some());

    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(std::io::stdout(), terminal::LeaveAlternateScreen,
                         ratatui::crossterm::cursor::Show);
        prev(info);
    }));

    terminal::enable_raw_mode()?;
    let mut out = BufWriter::with_capacity(1 << 19, std::io::stdout());
    execute!(out, terminal::EnterAlternateScreen, ratatui::crossterm::cursor::Hide)?;
    let mut term = Terminal::new(CrosstermBackend::new(out))?;

    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x2545F4914F6CDD1D);

    let mut app = App::new(seed, DEFAULT_MAX_SCALE);
    app.set_tier(tier);

    let mut last = Instant::now();
    let mut deadline = Instant::now();

    while !app.should_quit() {
        let mut actions = Vec::new();
        while event::poll(Duration::ZERO)? {
            if let Event::Key(k) = event::read()? {
                if k.kind != KeyEventKind::Press { continue; }
                if let Some(a) = map_key(k.code) { actions.push(a); }
            }
        }

        let now = Instant::now();
        let dt = (now - last).as_secs_f32().min(0.25);
        last = now;
        app.update(dt, &actions);

        if sync { let _ = execute!(term.backend_mut(), terminal::BeginSynchronizedUpdate); }
        term.draw(|f| app.render(f.buffer_mut()))?;
        if sync { let _ = execute!(term.backend_mut(), terminal::EndSynchronizedUpdate); }
        term.backend_mut().writer_mut().flush()?;

        deadline += FRAME;
        let now = Instant::now();
        if deadline > now { std::thread::sleep(deadline - now); } else { deadline = now; }
    }

    terminal::disable_raw_mode()?;
    execute!(term.backend_mut(), terminal::LeaveAlternateScreen,
             ratatui::crossterm::cursor::Show)?;
    Ok(())
}
```

- [ ] **Step 2: Verify it builds and the suite is green**

Run: `cargo build && cargo test`
Expected: compiles; all tests pass.

- [ ] **Step 3: Play it**

Run: `cargo run --release`
Confirm by hand: the snake starts still, the first arrow key starts it, it moves and grows, walls kill, `r` restarts, `q` quits, the terminal is restored on exit, and shrinking the window below 86×31 shows the resize screen.

- [ ] **Step 4: Commit and push**

```bash
git add src/main.rs
git commit -m "feat: add terminal loop with buffered output and synchronized updates"
git push origin main
```

---

## Self-Review

**Spec coverage for Phase 1:** §2 stack and profiles → Task 1. §3 geometry and gate → Task 11. §3.1 tiers → Tasks 9, 12, 15. §4.1 linear light → Task 9. §4.2 quantizer → Tasks 9, 10. §4.6 buffered stdout and sync output → Task 15. §5.1 tick and clamp → Tasks 6, 8. §5.2 input → Tasks 7, 15. §5.4 food → Task 5. §5.5 initial state → Task 8. §5.6 death and win → Tasks 4, 8. §5.7 RNG → Task 2. §6 pacing and panic hook → Task 15. §11 constraints → enforced by the module layout; `game/`, `render/`, and `ui/` import no `std::io` or crossterm.

Deferred to later phases by design: combo and golden food (Phase 3), SDF ribbon, glow, particles, trail (Phase 2), themes, menus, persistence (Phases 3–4), CI and releases (Phase 5).

**Type consistency:** `Pos`, `Direction`, `Mode`, `StepOutcome`, `GameState`, `Action`, `DirQueue`, `Canvas`, `ColorTier`, `Tier`, and `Layout` are each defined once and referenced with the same names and signatures throughout.
