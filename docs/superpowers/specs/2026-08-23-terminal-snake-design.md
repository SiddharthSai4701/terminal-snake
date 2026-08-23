# Terminal Snake — Design Spec

**Date:** 2026-08-23
**Status:** Approved for planning

## 1. Goal

A terminal snake game worth choosing over any downloadable one. Classic snake rules
underneath; the differentiation is entirely in presentation, motion feel, and
progression. Three pillars, chosen by the user:

1. **Visual spectacle** — truecolor pixel rendering, glow, particles, trails.
2. **Meta / progression** — unlockable themes, daily challenge, score tables.
3. **Polish & feel** — 60fps interpolated motion, responsive input, animated UI.

Explicitly out of scope: new gameplay mechanics that change what snake *is*
(portals, hazards, power-ups, shrinking arenas). The one exception the user
approved is golden food (Section 5.4), a scoring flourish.

## 2. Platform and stack

| Concern | Choice |
|---|---|
| Language | Rust (edition 2021), `rustc` 1.97 |
| Terminal | `crossterm` backend via `ratatui` |
| Serialization | `serde`, `serde_json` |
| RNG | `rand` with `StdRng` for seeded determinism |
| Target | Windows Terminal primary (truecolor, full Unicode); portable to any truecolor terminal |
| Distribution | single `terminal-snake.exe`, no runtime deps |

`ratatui`'s cell buffer is a grid of `{char, fg, bg}`. That is exactly the
half-block model this design needs, and it brings a double-buffered minimal-write
diff renderer, which prevents flicker on Windows Terminal. Custom drawing happens
in an owned f32 pixel layer that is quantized into that buffer once per frame.

Rejected: raw `crossterm` alone (rebuilds list/table/border UI from scratch for
menus, profile, and score tables — significant code that is not the game).
Rejected: sixel / kitty graphics protocol (Windows Terminal support is recent and
inconsistent; breaks the "runs anywhere" promise for a look half-blocks already
deliver).

## 3. Geometry

Two grids are kept deliberately separate.

| | Value | Rationale |
|---|---|---|
| Logic grid | **28 x 18 cells**, fixed | Scores stay comparable across terminal sizes |
| Pixel scale | **3–6 px per cell**, adaptive | Larger terminal renders crisper, never easier |
| Min terminal | **84 x 30** | 84 cols x 27 rows arena + 2 HUD rows + 1 hint row |

Pixel scale is computed each frame:

```
scale = clamp(min(avail_cols / 28, (avail_rows * 2) / 18), 3, 6)
```

A half-block cell holds two vertically stacked pixels, so pixels are square and
the arena is not stretched; diagonal motion reads correctly.

When the terminal is below the minimum in either dimension, the app shows a
"resize me" screen with the current and required size. It never squishes the
arena. If the terminal is resized mid-run, the run pauses and shows the same
screen; resizing back resumes it.

## 4. Rendering system

### 4.1 Pixel canvas

`render/canvas.rs` owns three same-sized `f32` RGB buffers over the arena's pixel
extent:

- **base** — cleared each frame; backdrop, arena border, snake, food.
- **glow** — additive bloom accumulator; cleared each frame.
- **trail** — persistent; multiplied by a decay constant (~0.85) each frame, then
  the snake body is added. Produces afterglow with one line of math.

Final pixel = `base + glow + trail`, tone-mapped (clamped, with a soft knee so
bright overlaps do not flatten to pure white), then quantized.

### 4.2 Quantizer

For each terminal cell, take pixel row `2y` and `2y+1`:

- Both pixels equal (within epsilon) → emit `' '` with that color as background.
- Otherwise → emit `'▀'` with the top pixel as foreground and the bottom as
  background.

Writes into the `ratatui` buffer; `ratatui` diffs against the previous frame and
emits only changed cells.

### 4.3 Snake ribbon

The snake is not drawn as discrete blocks. The body is a polyline through cell
centers in pixel space, with the head advanced by the current tick fraction
`t in [0,1)` toward its next cell, and the tail retracted by the same fraction
unless the snake is growing.

Rasterization is by signed distance to the polyline: for each pixel in the
polyline's bounding box, compute the minimum distance to any segment, then map
distance to coverage over roughly one pixel of falloff. Coverage blends the body
color into the base buffer, which yields antialiased edges and rounded caps and
joins despite the terminal having no alpha channel.

Body color is a head-to-tail gradient through the active theme's ramp, with a
highlight band whose phase advances with time, so the snake looks glossy and alive
while stationary.

### 4.4 Effects (`render/fx.rs`)

- **Particles** — `{pos, vel, life, max_life, color, drag}`. Eat: directional
  burst opposite the travel direction. Death: the entire body dissolves into
  ~400 particles with drag and slight gravity. Combo above a threshold: sparks
  trailing the head. Particles splat into the glow buffer.
- **Screen shake** — decaying random offset applied to the canvas sample origin.
  Triggered on eat (small) and death (large).
- **Flash** — full-canvas additive pulse, white on death, theme-colored on eat.
- **Backdrop** — per-theme, low intensity: vignette, drifting starfield, or
  scanlines. Must never compete with the snake for attention.

### 4.5 Themes

Eight themes in `render/theme.rs`, each a data record: body gradient ramp, food
color, glow tint, border style, backdrop kind, HUD accent. Themes are pure data,
so adding one is a table entry, not code.

## 5. Game rules (`game/`)

`game/` has no rendering dependencies and is fully unit-testable.

### 5.1 Tick and speed

Fixed-timestep logic with an accumulator; rendering runs at 60fps and reads the
tick fraction for interpolation.

```
tick_ms = clamp(140.0 * 0.985_f32.powi(food_eaten), 55.0, 140.0)
```

### 5.2 Input

Direction presses push onto a queue of depth 2. Each tick pops one. A press is
rejected if it reverses the **last applied** direction — not the last pressed —
which is what allows a fast double-tap corner (e.g. up-then-left within one tick)
to register instead of being swallowed.

### 5.3 Scoring and combo

Base 10 points per food, multiplied by a combo multiplier. The multiplier rises
with each eat that lands inside a decay window and falls back toward 1.0 when the
player idles. Displayed in the HUD, recorded per run as `combo_max`.

### 5.4 Food

Normal food spawns uniformly on a free cell. Golden food appears rarely, is worth
substantially more, and expires — drawn with a depleting ring so the timer is
readable at a glance. Both spawn from the mode's RNG stream.

### 5.5 Modes

| Mode | Walls | RNG | Scores |
|---|---|---|---|
| `CLASSIC` | kill | random | own top-10 table |
| `ENDLESS` | wrap | random | own top-10 table |
| `DAILY` | kill | seeded from UTC date | own table, one recorded attempt/day |

Daily seed is derived by hashing the UTC `YYYY-MM-DD` string into a `StdRng`
seed. The same date produces the same food sequence and starting position on any
machine. Replays after the first attempt are allowed but not recorded.

### 5.6 Death

Wall collision (Classic/Daily) or self-collision (all modes) ends the run. Death
triggers flash, large shake, and body dissolution, then a run-summary overlay
with score, length, duration, max combo, plus new-record and unlock notifications
when earned.

## 6. Screens and flow

```
Title (snake crawls the logo)
  └─ Menu ─┬─ Play ─┬─ Classic / Endless / Daily → Game → Summary → Menu
           │        └─ (Pause overlay via Esc)
           ├─ Themes  (gallery with live animated preview; locked entries show requirement)
           ├─ Scores  (tabbed per mode)
           ├─ Profile (lifetime stats, unlock checklist, daily status)
           └─ Quit
```

Transitions between screens are ~200ms dissolves. Controls: arrows / WASD to
move and navigate, Enter to select, Esc to pause or go back, R to restart from
the summary, Q to quit.

The main loop installs a panic hook that restores the terminal (leave alternate
screen, show cursor, disable raw mode) before printing the panic, so a crash
never leaves the user's terminal broken.

## 7. Persistence (`save.rs`)

Persistence sits behind a trait so the core never touches the filesystem
directly. This keeps a future WebAssembly build (Section 11) a small addition
rather than a rewrite.

```rust
pub trait Storage {
    fn load(&self) -> Option<String>;
    fn save(&self, data: &str) -> Result<(), StorageError>;
}
```

`FileStorage` is the desktop implementation; a `LocalStorage` implementation
backs the web build. `save.rs` handles serialization, defaults, and versioning
above the trait and is testable with an in-memory implementation.

Desktop location: `%APPDATA%\terminal-snake\profile.json` (platform config dir
elsewhere).

```jsonc
{
  "version": 1,
  "stats": { "runs": 0, "total_food": 0, "total_time_secs": 0 },
  "scores": {
    "classic": [ { "score": 0, "length": 0, "duration_secs": 0,
                   "combo_max": 1.0, "date": "2026-08-23" } ],
    "endless": [ ], "daily": [ ]
  },
  "unlocks": ["ember"],
  "daily": { "date": "2026-08-23", "played": true, "score": 0 }
}
```

Writes are atomic: serialize to a temp file in the same directory, then rename
over the target. A missing, unreadable, or version-mismatched file yields a fresh
default profile rather than an error — the game must always start.

Unlock conditions are declarative data evaluated against the profile after each
run, so a newly satisfied condition surfaces immediately in the summary overlay.
Eight themes gated on a mix of: best score thresholds, lifetime run counts, and
milestones such as surviving three minutes or reaching length 40.

## 8. Module layout

```
src/
  main.rs           terminal setup/teardown, panic hook, main loop
  app.rs            app state machine, screen routing, transitions
  input.rs          key mapping, direction buffer
  game/
    mod.rs          Game struct, tick, rules orchestration
    snake.rs        body deque, growth, collision
    food.rs         spawn, seeded RNG, golden food lifetime
    score.rs        combo, scoring, speed curve
  render/
    canvas.rs       f32 buffers, blend, tone-map, quantize to ratatui
    draw.rs         SDF polyline ribbon, circles, sprites
    fx.rs           particles, shake, flash, trail decay
    theme.rs        eight themes as data
  ui/
    hud.rs menu.rs scores.rs profile.rs themes.rs
  save.rs           profile load/save, atomic write, defaults
  daily.rs          date → seed derivation
```

## 9. Testing

Test-driven, logic first. `game/`, `save.rs`, and `daily.rs` carry the bulk.

- **Collision** — wall death in Classic/Daily; wrap in Endless; self-collision in
  all modes; the tail cell is legal to enter when not growing.
- **Growth** — length increases exactly one per food; the tail holds position on
  the growth tick.
- **Speed curve** — monotonically decreasing, clamped at both ends.
- **Combo** — rises inside the window, decays outside, never below 1.0.
- **Daily determinism** — the same date seed produces an identical food sequence
  and start state across fresh instances.
- **Save round-trip** — write then read yields an equal profile; corrupt and
  missing files fall back to defaults; version mismatch falls back to defaults.
- **Unlocks** — each condition fires exactly when its threshold is crossed.
- **Quantizer** — known pixel pairs map to the expected `{char, fg, bg}` cells,
  including the equal-pair collapse to a background-only space.

Rendering beyond the quantizer is verified by eye, not by test.

## 10. Repository

Public GitHub repository `SiddharthSai4701/terminal-snake`. Commit at each
completed phase and push after each commit.

## 11. Distribution

The game is meant to be easy for other people to play.

**Shipping in v1:**

- **GitHub Releases with prebuilt binaries.** A GitHub Actions workflow builds
  Windows x86_64, macOS x86_64 and aarch64, and Linux x86_64 on every pushed tag
  and attaches the artifacts to the release. A player downloads one file and
  runs it.
- **crates.io.** `cargo install terminal-snake` for anyone with a Rust
  toolchain.
- **README** with an animated capture of real gameplay, install instructions per
  platform, and controls.

**Planned follow-up, designed for but not built in v1:**

- **Browser build.** Compile the core to WebAssembly and drive `xterm.js` on a
  GitHub Pages site, so the game is playable from a link with no download.

**Portability constraints this imposes on v1**, which must hold from the first
commit:

1. `game/`, `render/canvas.rs`, `render/draw.rs`, `render/fx.rs`, and
   `render/theme.rs` must not reference `std::io`, `crossterm`, the filesystem,
   or the system clock. Time enters as a `dt` parameter; randomness enters as an
   injected RNG.
2. Persistence goes through the `Storage` trait (Section 7). No direct file
   access outside `FileStorage`.
3. The main loop's terminal setup, input polling, and frame flush are confined
   to `main.rs`, `app.rs`, and `input.rs` — the layer a web build replaces.

**Rejected: a VS Code extension.** A VS Code user already has an integrated
terminal, so the extension would only shell out to the same binary, while
requiring separate per-platform VSIX packages. Substantial plumbing for
negligible added reach.
