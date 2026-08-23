# Terminal Snake — Design Spec

**Date:** 2026-08-23
**Status:** Revised after two independent fresh-context reviews (spec quality, Rust/terminal feasibility)

## 1. Goal

A terminal snake game worth choosing over any downloadable one. Classic snake rules
underneath; the differentiation is entirely in presentation, motion feel, and
progression. Three pillars:

1. **Visual spectacle** — truecolor pixel rendering, glow, particles, trails.
2. **Meta / progression** — unlockable themes, daily challenge, score tables.
3. **Polish & feel** — 60fps interpolated motion, responsive input, animated UI.

Out of scope: new gameplay mechanics that change what snake *is* (portals,
hazards, power-ups, shrinking arenas). The one approved exception is golden food
(§5.4), a scoring flourish.

## 2. Platform and stack

| Concern | Choice |
|---|---|
| Language | Rust (edition 2021), `rustc` 1.97 |
| TUI | `ratatui` 0.30.2 — `ratatui-core` + `ratatui-widgets` always, `ratatui-crossterm` native-only |
| Terminal | `crossterm` 0.29, **only via ratatui's re-export** (`ratatui::crossterm::…`) |
| Serialization | `serde`, `serde_json`, `thiserror` |
| Config dir | `dirs` |
| RNG | **hand-rolled PCG32 in `game/`** — no `rand` dependency (see §5.7) |
| Target | Windows Terminal primary (truecolor, full Unicode); portable to any truecolor terminal |
| Distribution | single `terminal-snake.exe`, no runtime deps |

`ratatui`'s cell buffer is a grid of `{char, fg, bg}` — exactly the half-block
model this design needs — and its 0.30 workspace split lets the core compile for
`wasm32` without crossterm in the tree.

**Never add `crossterm` to `[dependencies]` directly.** `ratatui-crossterm`
selects its own crossterm by feature; a second copy links two incompatible
`KeyEvent` types.

**Verified API** (ratatui 0.30.2), used from inside `terminal.draw(|f| …)`:

```rust
let buf = f.buffer_mut();                       // Frame::buffer_mut() -> &mut Buffer
buf[(x, y)].set_char('▀')
           .set_fg(Color::Rgb(r0, g0, b0))
           .set_bg(Color::Rgb(r1, g1, b1));
```

No `Widget` impl is needed for the canvas; `buffer_mut()` is the intended escape
hatch, and `Terminal::draw` resets the back buffer each frame, so writing every
cell every frame is the expected pattern. `set_skip` is deprecated as of 0.30.1.

Rejected: raw `crossterm` alone (rebuilds list/table/border UI from scratch).
Rejected: sixel / kitty graphics (Windows Terminal support is recent and
inconsistent; breaks "runs anywhere" for a look half-blocks already deliver).

### 2.1 Build profiles

Required from the first commit — an unoptimized f32 pixel loop runs 10–30×
slower and a debug build would appear broken:

```toml
[profile.dev]
opt-level = 1
[profile.dev.package."*"]
opt-level = 3
```

## 3. Geometry

Two grids are kept deliberately separate.

| | Value | Rationale |
|---|---|---|
| Logic grid | **28 × 18 cells**, fixed | Scores stay comparable across terminal sizes |
| Border | **1 px** on every side, inside the canvas | Drawn without consuming playable cells |
| Canvas extent | `(28·s + 2) × (18·s + 2)` px | `s` = pixel scale |
| Pixel scale `s` | **integer 3–6**, default cap **4** | Larger terminal renders crisper, never easier |
| Min terminal | **86 × 31** | 86 cols × 28 rows canvas + 2 HUD rows + 1 hint row |

Scale is an **integer**, computed with floor division against the region left
after chrome:

```
chrome_rows = 3
s = clamp(floor(min((cols - 2) / 28, ((rows - chrome_rows) * 2 - 2) / 18)), 3, s_max)
s_max = 4 by default; a settings entry raises it to 6
```

At the minimum 86×31 this yields `min(3, 3) = 3` ✓. The `s_max` default of 4 is
a throughput decision, not a visual one — see §4.6.

**Scale is frozen for the duration of a run.** It is computed when the game
screen is entered and does not change until the run ends. This protects score
comparability and avoids reallocating the persistent trail buffer mid-run.

The arena is **centered** horizontally and vertically in the available region;
surplus rows and columns are filled with the theme backdrop.

**Resize handling.** Resize events are **debounced 150 ms** (Windows Terminal
streams them continuously during a window drag). Only a resize that drops the
terminal *below the minimum* pauses the run and shows the resize screen; growing
an already-valid terminal changes nothing until the next run. The resize screen
states the current and required size.

**Startup check.** If `ENABLE_VIRTUAL_TERMINAL_PROCESSING` cannot be set (legacy
conhost, redirected output), crossterm silently falls back to a 16-color WinAPI
path and every color collapses to mud. Check ANSI support at startup and exit
with a clear message rather than rendering garbage.

### 3.1 Terminal capability tiers

Rust and crossterm make the game *build and run* everywhere; they do not make
every terminal capable of this design. Capability is detected once at startup and
the renderer picks a tier.

| Tier | Condition | Output |
|---|---|---|
| **Full** | truecolor available | `Color::Rgb` per cell, as designed |
| **Reduced** | 256-color only | quantizer maps each RGB to the nearest xterm-256 index (6×6×6 cube + 24 greys); a one-line banner on first run says the terminal is limited |
| **Refused** | 16-color or no ANSI | clear message naming the problem and suggesting a modern terminal; exit rather than render mud |

Truecolor is detected from `COLORTERM` (`truecolor` / `24bit`), then `TERM`
(`*-direct`, `*-256color`), then a Windows Terminal / known-emulator check, with
an explicit `--truecolor` / `--256` override for terminals that support it
without advertising. **macOS Terminal.app is the notable Reduced-tier case** —
it has never supported 24-bit color, so an unguarded build would look broken to
every default-Terminal Mac user. iTerm2, WezTerm, Alacritty, Ghostty, and kitty
are all Full tier.

The nearest-256 mapping lives in the quantizer and is a pure function, so it is
unit-tested like the rest of §4.2 rather than being a special render path.

**Multiplexers.** Under `tmux` or `screen` (`TERM` starts `tmux`/`screen`, or
`TMUX` is set), DEC 2026 synchronized output is not passed through by default and
the escape is suppressed rather than emitted blind. The adaptive 30fps fallback
in §4.6 covers the resulting tear risk.

## 4. Rendering system

### 4.1 Pixel canvas

`render/canvas.rs` owns three `f32` **linear-light** RGB buffers:

- **base** — full resolution, cleared each frame: backdrop, border, snake, food.
- **glow** — **half resolution**, cleared each frame; additive bloom accumulator.
- **trail** — full resolution, persistent; decayed each frame, then the snake is
  added. Produces afterglow.

Final pixel = `base + blur(glow)↑ + trail`, tone-mapped, sRGB-encoded, quantized.

**Color space.** Theme ramps are authored as sRGB hex. They are decoded to linear
**once at startup**; all compositing happens in linear light; the quantizer
sRGB-encodes on the way out. Additive glow over gamma-encoded values looks washed
out — this is the difference between glow that reads as lit and glow that reads
as grey.

**Tone map.** Linear below ~0.8, with a knee that lands exactly on 1.0. A
Reinhard-style `x/(1+x)` never reaches 1.0, which would make the death flash grey
instead of white.

**Trail decay is time-based, not frame-based:** `trail *= exp(-dt / tau)`, with
`tau` a per-theme parameter in seconds. A per-frame constant makes the afterglow
twice as long at 30fps and invisible at 144Hz. Shake decay and the highlight-band
phase are likewise driven by `dt`.

**Shake margin.** Screen shake offsets the sample origin, which samples outside
the buffer. The canvas allocates a few pixels of bleed and clamps at the edge.

### 4.2 Quantizer

For each terminal cell, take pixel rows `2y` and `2y+1`:

1. sRGB-encode and quantize **both pixels to `u8` first**, rounding:
   `(v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8`.
2. Compare the two `u8` triples **exactly** — not the f32s within an epsilon.
   Two f32s within epsilon can quantize differently and vice versa, which would
   emit `▀` cells whose fg equals bg.
3. Equal → emit `' '` with that color as **background and `fg = Color::Reset`**.
   Reset (not `fg = bg`) lets crossterm skip the foreground SGR entirely across
   runs of flat backdrop — about 18 bytes per cell saved.
4. Otherwise → `'▀'`, top pixel as fg, bottom as bg.

The collapse to `' '` saves only two glyph bytes; its real value is avoiding
font-fallback seams across large flat `▀` regions.

**Color is snapped to 5 bits per channel** before writing the Cell. Visually
indistinguishable in a terminal, but it makes ratatui's diff actually work: a
slowly decaying trail pixel then changes its emitted value every 2–3 frames
instead of every frame, roughly halving dirty cells and bytes in the large
low-intensity regions.

### 4.3 Snake ribbon

The body is a polyline through cell centers in pixel space; the head advances by
the tick fraction `t ∈ [0,1)` toward its next cell and the tail retracts by the
same fraction unless growing. It is rasterized as a signed distance field so
edges are antialiased and caps and joins are round — half-block pixels are real
square 8-bit pixels, so coverage genuinely buys smooth curves that a Bresenham
thick line or per-cell blocks would visibly stair-step on exactly the diagonals
this design exists to show off.

**Rasterize per segment, over that segment's own bounding box only. Never loop
the whole polyline bounding box × all segments.** The naive form in the first
draft costs `length × arena_pixels` point-to-segment tests — 4.5 M tests/frame at
length 250, roughly 22 ms/frame, blowing the entire budget on the snake alone.
Each segment spans one logic cell, so its capsule bbox is `(s + 2r)²` ≈ 121 px at
scale 6:

```
length 250 → 250 × 121 ≈ 30,250 tests/frame ≈ 0.18 ms      (~125× cheaper)
```

Coverage accumulates with `max` (the union of capsules is the min of distances,
hence the max of coverages). Cost is `O(length × s²)`, independent of arena size.

**Radius and falloff scale with `s`:** `r = 0.40·s`, `falloff = 0.5 + 0.15·s`. A
fixed 1-px falloff at scale 3 makes the body *entirely* falloff — a smudge with
no solid core. Scale 4 is the floor for the intended look; scale 3 is the
degraded-but-playable fallback.

**Distance uses a `pixel_aspect: f32` multiplier.** "Half-blocks are square" holds
for Cascadia Mono at Windows Terminal's default line height, but breaks under a
different font or line-height. One multiply makes the SDF robust to that.

Body color is a head-to-tail gradient through the theme ramp with a
time-advancing highlight band, so the snake looks glossy even at rest.

### 4.4 Effects (`render/fx.rs`)

- **Particles** — `{pos, vel, life, max_life, color, drag}`. Eat: directional
  burst. Death: the body dissolves into a **fixed ~400** particles (not scaled by
  length) with drag and slight gravity. High combo: sparks trailing the head.
  Particles splat into the glow buffer.
- **Glow blur** — the glow buffer is half resolution; splat, then run a
  **separable 5-tap Gaussian (2 passes)**, then bilinear-upsample-add. *Additive
  brightness with no spatial spread is not bloom, it is just a brighter pixel* —
  without this pass the headline visual feature silently does not exist. Cost is
  ~136 K FMAs/frame, under 0.1 ms, and half resolution halves the splat cost too.
- **Screen shake** — decaying random sample offset; small on eat, large on death.
- **Flash** — full-canvas additive pulse, white on death, theme-tinted on eat.
- **Backdrop** — per-theme, low intensity: vignette, drifting starfield, or
  scanlines. *Design intent, not a testable requirement:* it must not compete
  with the snake for attention.

### 4.5 Themes

Eight themes in `render/theme.rs`, each a data record: body gradient ramp, food
color, glow tint, `tau`, border style, backdrop kind, HUD accent. Pure data, so
adding one is a table entry.

### 4.6 Frame budget and throughput

Measured extents:

| | scale 3 | scale 4 (default cap) | scale 6 |
|---|---|---|---|
| terminal cells | 2,268 | 4,032 | 9,072 |
| canvas pixels | 4,536 | 8,064 | 18,144 |
| buffers (3× f32 RGB) | 159 KB | 283 KB | 638 KB |
| worst-case ANSI bytes/frame | 88 KB | 157 KB | 346 KB |
| @60fps | 5.3 MB/s | 9.4 MB/s | 21.2 MB/s |

**Acceptance criterion:** logic + canvas + quantize completes in **under 8 ms at
scale 4 with 400 live particles**, verified by a benchmark and a toggleable debug
frame-time readout. With the §4.3 fix the per-pixel work is ~0.2 ms and the
quantizer ~50–100 µs; the terminal write dominates.

Three throughput requirements, all Windows-Terminal-driven:

1. **Buffered stdout.** Do **not** use `ratatui::init()` — its `DefaultTerminal`
   wraps a `LineWriter`, and since the ANSI stream contains no newlines a 346 KB
   frame drains as ~43 separate `WriteConsoleW` calls. Build the terminal by hand
   with a 512 KB `BufWriter` so each frame flushes as one write. This is worth
   more than any rendering micro-optimization.
2. **Synchronized output.** Wrap every frame in DEC mode 2026
   (`BeginSynchronizedUpdate` / `EndSynchronizedUpdate`). ratatui's backend does
   not emit these itself. This is the direct fix for tearing.
3. **Adaptive fallback.** Per-character truecolor SGR is the pathological case
   for Windows Terminal throughput. If measured frame time exceeds ~14 ms, drop
   the *render* rate to 30fps while logic stays at 60.

**Do not rely on ratatui's diff to reduce writes.** A decaying trail plus an
animated backdrop dirties nearly every cell every frame; the 5-bit color snap in
§4.2 is what makes the diff earn anything.

## 5. Game rules (`game/`)

`game/` has no rendering, filesystem, clock, or terminal dependencies and is
fully unit-testable.

### 5.1 Tick and speed

Fixed-timestep logic with an accumulator; rendering runs at 60fps and reads the
tick fraction for interpolation.

```
tick_ms = clamp(140.0 * 0.985^normal_food_eaten, 55.0, 140.0)
```

The upper clamp is unreachable by construction and is defensive only; tests
assert the lower clamp.

- **Accumulator is clamped** to `5 × tick_dt` per frame. Without this, a window
  drag, debugger breakpoint, or laptop sleep produces seconds of instant ticks —
  the snake teleports and dies.
- **`tick_ms` is recomputed only at tick boundaries.** Eating mid-accumulator
  otherwise changes the divisor for a fraction that was filled against the old
  value, snapping the interpolation visibly.

### 5.2 Input

Key events map to an internal `enum Action` **immediately, inside `input.rs`**.
A `crossterm::KeyCode` must never appear in an `app.rs` signature (§11).

**Windows delivers every keypress twice** — crossterm emits both
`KeyEventKind::Press` and `KeyEventKind::Release`, while macOS and Linux emit
Press only. Unfiltered, one arrow tap fills the depth-2 queue with duplicates and
the fast-corner feature never fires; it reads as dropped input. `input.rs` drops
everything that is not `KeyEventKind::Press`, and a unit test feeds a Press+Release
pair and asserts a queue depth of 1.

Direction presses push onto a **queue of depth 2**; each tick pops one.

- A press is rejected if it reverses **the last direction in the queue**, falling
  back to the last *applied* direction when the queue is empty. Validating
  against the applied direction alone is a bug: with Right applied and Up queued,
  a Down press is not a reversal of Right, so it is accepted, and the next two
  ticks apply Up then Down — an instant self-collision. The queue-tail rule still
  admits the up-then-left corner the depth-2 buffer exists for.
- A press equal to the last effective direction is **discarded**, not queued — a
  queued no-op would burn one of only two slots.
- When the queue is full the **newest press is dropped**.

### 5.3 Scoring and combo

All combo timing is in **ticks**, not wall-clock, so it does not drift as the
speed curve accelerates.

```
COMBO_WINDOW_TICKS = 25
COMBO_STEP         = 0.25
COMBO_MAX          = 5.0
COMBO_DECAY_PER_TICK = 0.05      // applies only after the window lapses
COMBO_MIN          = 1.0
```

Eating within `COMBO_WINDOW_TICKS` of the previous food: `combo = min(combo +
COMBO_STEP, COMBO_MAX)` and the window restarts. Once the window lapses, combo
decays by `COMBO_DECAY_PER_TICK` each tick, floored at `COMBO_MIN`. Score per
normal food is `round(10 × combo)`. `combo_max` for the run is recorded in the
score table.

### 5.4 Food

**Spawning enumerates free cells and indexes into that list.** Rejection sampling
would hang forever on a full board and would also make the daily-determinism test
depend on rejection count.

**Normal food:** always exactly one on the board. Eating it grows the snake by 1,
increments `normal_food_eaten` (the speed-curve input), and increments
`total_food`.

**Golden food:** on each normal-food spawn there is a **1-in-8** chance a golden
food also spawns, on a different free cell. It **coexists** with normal food,
lives **60 ticks**, and is drawn with a depleting ring showing the remaining
time. Eating it:

| | Effect |
|---|---|
| Score | `round(50 × combo)` |
| Growth | +3 |
| `normal_food_eaten` (speed) | **not incremented** — speed tracks normal food only |
| `total_food` | +1 |
| Combo | restarts the window; does **not** advance the multiplier |

Keeping golden food out of the speed curve keeps the curve a pure function of
normal food count, which keeps daily runs comparable.

### 5.5 Modes and initial state

**Initial state is fixed and identical in all modes:** length **4**, head at cell
**(9, 9)** facing **Right**, body extending left. The snake **waits for the first
directional press** before the clock and the tick accumulator start.

Nothing about the start is randomized. The only seeded quantity is the food
sequence, which makes the daily guarantee simple to state and simple to test.

| Mode | Walls | RNG | Scores |
|---|---|---|---|
| `CLASSIC` | kill | entropy-seeded | own top-10 table |
| `ENDLESS` | wrap | entropy-seeded | own top-10 table |
| `DAILY` | kill | seeded from UTC date | own table, one recorded attempt/day |

**Daily determinism:** the same UTC date produces the same food sequence on any
machine, on any build. Replays after the first attempt are allowed but not
recorded.

### 5.6 Death and win

Wall collision (Classic/Daily) or self-collision (all modes) ends the run. The
tail cell is legal to enter when not growing. Death triggers flash, large shake,
and body dissolution, then a run-summary overlay with score, length, duration,
max combo, plus new-record and unlock notifications.

**Win state:** filling all 504 cells ends the run as a win, with its own screen.
This is almost certainly unreachable, but it is defined so the free-cell
enumerator has a terminal case instead of an empty-list panic.

### 5.7 Random number generation

**`StdRng` cannot be used.** `rand`'s own documentation states it is
"non-portable… even with a fixed seed, output is not portable" and that a future
version may replace the algorithm — which would silently invalidate every past
daily seed and make the daily leaderboard incomparable across game versions.
`DefaultHasher` carries the same non-guarantee for the seed derivation.

`game/` therefore contains a **hand-written ~20-line PCG32** and an inline
**FNV-1a-64** for deriving the daily seed from the `YYYY-MM-DD` string. This
guarantees determinism permanently, drops the `rand` dependency entirely, and
removes the WASM `getrandom` problem (§11) outright.

Entropy for Classic and Endless comes from a `fn seed() -> u64` in the platform
layer — OS RNG natively, `crypto.getRandomValues` on web.

## 6. Screens and flow

```
Title (snake crawls the logo)
  └─ Menu ─┬─ Play ─┬─ Classic / Endless / Daily → Game → Summary → Menu
           │        └─ Pause overlay (Esc)
           ├─ Themes  (gallery, live animated preview, locked entries show requirement)
           ├─ Scores  (tabbed per mode)
           ├─ Profile (lifetime stats, unlock checklist, daily status)
           └─ Quit
```

Transitions are ~200 ms dissolves.

**Controls.** Arrows / WASD to move and navigate, Enter to select, Esc to pause
or go back, R to restart from the summary, Q to quit. Letter keys are
case-insensitive. **Q mid-run and Esc→Menu from pause both abandon the run
without recording a score** (a Daily attempt is still consumed — see §7).

**Frame pacing.** Use `poll(Duration::ZERO)` in a drain-all input loop and pace
frames separately with `thread::sleep` against an `Instant` deadline. Passing a
real timeout to `poll` is the trap: crossterm's Windows event source waits on the
console handle, and waitable-object timeouts round to the ~15.6 ms system tick,
quantizing frame times and producing judder that looks like a rendering bug.
(`thread::sleep` itself is fine — Rust std uses a high-resolution waitable timer
on Windows 10 1803+.)

The main loop installs a panic hook that restores the terminal (leave alternate
screen, show cursor, disable raw mode) before printing the panic, so a crash
never leaves the user's terminal broken.

## 7. Persistence (`save.rs`)

Persistence and time both sit behind traits so the core never touches the
filesystem or the system clock:

```rust
pub trait Storage {
    fn load(&self) -> Option<String>;
    fn save(&self, data: &str) -> Result<(), StorageError>;
}

pub trait Clock {
    fn utc_date(&self) -> Date;      // YYYY-MM-DD
    fn now(&self) -> Instant;
}
```

`FileStorage` + a system clock are the desktop implementations; `LocalStorage`
and a JS clock back the web build. Both traits have in-memory test
implementations. `SystemTime::now()` and `Instant::now()` panic on
`wasm32-unknown-unknown`, which is why the clock is injected rather than called.

The UTC date is computed with a ~15-line days-from-civil conversion off
`SystemTime`, keeping `daily.rs` dependency-free.

Desktop location: `dirs::config_dir()` + `terminal-snake/profile.json`
(Roaming `%APPDATA%` on Windows).

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

Every field carries `#[serde(default)]` so adding a stat later cannot nuke saves.

**Writes are atomic:** serialize to a temp file in the same directory,
`File::sync_all()`, then rename over the target. `fs::rename` does overwrite on
Windows (`MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`), so the pattern is
sound — but **retry the rename 3× with a short backoff**, because antivirus and
the search indexer transiently hold handles on newly created files in AppData and
return `ERROR_ACCESS_DENIED`.

**Version handling** (never silently discard a player's history):

| Case | Behavior |
|---|---|
| `version == current` | load |
| `version < current` | migrate forward |
| `version > current` | rename the file aside, start fresh, tell the player |
| unparseable | rename aside, start fresh |

### 7.1 Daily bookkeeping

The date is captured **at run start**, and the attempt is marked consumed **at
run start** — this prevents farming by crashing or quitting, and fixes the
midnight-crossing case: a run begun at 23:58 UTC records against that date, and
the next day's attempt re-arms on the new date regardless of when the run ended.
`scores.daily` accumulates one entry per recorded day, kept top-10 by score,
while `profile.daily` holds only today's attempt status.

### 7.2 Unlocks

Eight themes. `ember` is the always-available starter; the other seven are gated
by exact predicates, evaluated **at run end** (including abandoned runs, since
lifetime stats still advance):

| Theme | Unlock predicate |
|---|---|
| `ember` | starter, always unlocked |
| `neon` | best score in any mode ≥ 500 |
| `frost` | best score in any mode ≥ 1500 |
| `toxic` | 10 lifetime runs |
| `solar` | 40 lifetime runs |
| `void` | best score in any mode ≥ 2500 |
| `abyss` | reach length 40 in a single run |
| `mirage` | survive 3 minutes in a single run |

Predicates are declarative data so a newly satisfied one surfaces immediately in
the summary overlay.

## 8. Module layout

```
src/
  main.rs           terminal setup/teardown, panic hook, THE LOOP (native only)
  app.rs            state machine + screen routing — pure, no loop, no I/O
  input.rs          KeyEvent → Action, Press-only filter, direction queue
  game/
    mod.rs          Game struct, tick, rules orchestration
    snake.rs        body deque, growth, collision
    food.rs         free-cell enumeration, normal + golden spawn/lifetime
    score.rs        combo, scoring, speed curve
    rng.rs          PCG32 + FNV-1a-64
  render/
    canvas.rs       linear f32 buffers, blur, tone map, sRGB encode, quantize
    draw.rs         per-segment SDF ribbon, circles, sprites
    fx.rs           particles, shake, flash, trail decay
    theme.rs        eight themes as data
  ui/
    hud.rs menu.rs scores.rs profile.rs themes.rs
    title.rs pause.rs summary.rs resize.rs
  platform/
    storage.rs      FileStorage
    clock.rs        system clock, days-from-civil
    entropy.rs      seed()
  save.rs           profile schema, defaults, versioning, migration
  daily.rs          date → seed derivation
```

`app.rs` exposes a pure surface so the browser build — where
`requestAnimationFrame` inverts control — reuses it untouched:

```rust
impl App {
    pub fn update(&mut self, dt: f32, input: &[Action]);
    pub fn render(&mut self, buf: &mut Buffer);
}
```

## 9. Testing

Test-driven, logic first. `game/`, `save.rs`, and `daily.rs` carry the bulk.

- **Collision** — wall death in Classic/Daily; wrap in Endless; self-collision in
  all modes; the tail cell is legal to enter when not growing.
- **Growth** — +1 per normal food, +3 per golden; the tail holds position on the
  growth tick.
- **Speed curve** — monotonically decreasing in `normal_food_eaten`, clamped at
  55 ms; unaffected by golden food.
- **Accumulator** — a 3-second stall produces at most 5 ticks.
- **Input** — a Press+Release pair yields queue depth 1; the queue-tail reversal
  rule rejects up-then-down while admitting up-then-left; a repeat press is
  discarded; a third press onto a full queue is dropped.
- **Combo** — rises by `COMBO_STEP` inside the window, caps at 5.0, decays after
  the window lapses, never below 1.0; golden food restarts the window without
  advancing the multiplier.
- **PCG32 / FNV-1a** — fixed vectors, asserted against hard-coded expected output
  so a refactor cannot silently change historical daily seeds.
- **Daily determinism** — the same date seed produces an identical food sequence
  across fresh instances.
- **Food spawn** — never returns an occupied cell; a full board yields the win
  state rather than looping.
- **Save round-trip** — write then read yields an equal profile; corrupt, missing,
  and newer-version files each fall back to defaults *and preserve the original*;
  an older version migrates forward.
- **Unlocks** — each of the seven predicates fires exactly when its threshold is
  crossed, and not before.
- **Quantizer** — known pixel pairs map to expected `{char, fg, bg}` cells,
  including the equal-pair collapse to a background-only space with
  `fg = Color::Reset`, and the 5-bit color snap.
- **Frame budget** — a benchmark asserts logic + canvas + quantize under 8 ms at
  scale 4 with 400 live particles.

Rendering beyond the quantizer and the benchmark is verified by eye.

## 10. Repository

Public GitHub repository `SiddharthSai4701/terminal-snake`. Commit at each
completed phase and push after each commit.

## 11. Distribution

**Shipping in v1:**

- **GitHub Releases with prebuilt binaries.** A GitHub Actions workflow builds
  Windows x86_64, macOS x86_64 and aarch64, and Linux x86_64 on each pushed tag
  and attaches the artifacts.
- **crates.io** — `cargo install terminal-snake`.
- **README** with an animated capture of real gameplay, per-platform install
  instructions, and controls.

**Planned follow-up, designed for but not built in v1:** compile the core to
WebAssembly and drive `xterm.js` on a GitHub Pages site, so the game is playable
from a link.

**Portability constraints, binding from the first commit:**

1. `game/`, `render/*`, **and `ui/*`** must not reference `std::io`, `crossterm`,
   the filesystem, the system clock, or entropy. Time enters as `dt`; the date
   enters via `Clock`; randomness enters as an injected RNG. `ui/` is included
   because those nine files are portable *only if* they touch `Buffer`/`Widget`
   and never a `Backend` — omitting them means rewriting half the UI.
2. Persistence goes through `Storage`, the date through `Clock`, entropy through
   `platform/entropy.rs`. No direct file, clock, or OS-RNG access elsewhere.
3. **The loop lives in `main.rs` alone.** `app.rs` is pure (`update`/`render`) so
   the browser's inverted control flow reuses it as-is.
4. `crossterm` and the crossterm-backed ratatui crate are declared under
   `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`. A `#[cfg]` on the
   module is not enough — a `crossterm` entry in plain `[dependencies]` fails to
   build for `wasm32-unknown-unknown` regardless.
5. `input.rs` maps to `Action` at the boundary; a `KeyCode` in an `app.rs`
   signature already breaks constraint 3 and nothing else will catch it.

**Scoped, not free:** an xterm.js `Backend` impl is ~150–200 lines under ratatui
0.30, which requires an associated `Error` type and a `clear_region` impl (no
default any more).

**Rejected: a VS Code extension.** A VS Code user already has an integrated
terminal, so the extension would only shell out to the same binary while
requiring per-platform VSIX packages. Substantial plumbing, negligible reach.

## 12. Phasing

This is more than one implementation plan's worth of work. Each phase gets its
own plan; §11's constraints are enforced from Phase 1.

| Phase | Contents | Milestone |
|---|---|---|
| **1 — Playable core** | `game/` for Classic (tick, speed, input queue, growth, collision, normal food, PCG32), `canvas.rs` + quantizer + flat-color snake, minimal HUD, geometry/resize gate, buffered stdout, sync output, build profiles | **A real, playable snake game** |
| **2 — Feel** | Per-segment SDF ribbon, gradient + highlight, trail buffer, glow + blur, particles, shake, flash, 60fps interpolation, frame-budget benchmark | Pillar 3 proved |
| **3 — Modes & persistence** | Endless wrap, Daily + `daily.rs`, combo, golden food, `Storage`/`Clock` traits, `save.rs`, score tables | Runs are recorded |
| **4 — Meta & shell** | 8 themes + gallery, unlock engine, profile and scores screens, title animation, transitions | Pillar 2 complete |
| **5 — Distribution** | CI matrix, tagged releases, crates.io, README capture | Other people can play |
