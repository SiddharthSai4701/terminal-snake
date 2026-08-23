# terminal-snake

A snake game for the terminal that is actually worth looking at.

Classic snake rules underneath. Everything above them — truecolor pixel
rendering on a half-block canvas, 60fps interpolated motion, glow, particles,
unlockable themes, a daily seeded challenge — is the point.

> **Status: Phase 1 of 5.** The game is playable right now: Classic mode, real
> rules, responsive input, adaptive rendering. It draws in flat colour. The
> ribbon body, glow, trails, and particles land in Phase 2; modes, combo, and
> persistence in Phase 3. See [the design spec](docs/superpowers/specs/2026-08-23-terminal-snake-design.md).

## Play

```bash
git clone https://github.com/SiddharthSai4701/terminal-snake
cd terminal-snake
cargo run --release
```

Prebuilt binaries for Windows, macOS, and Linux arrive in Phase 5.

## Controls

| Key | Action |
|---|---|
| arrows / `WASD` | turn (also starts the run) |
| `R` | restart after dying |
| `Q` / `Esc` | quit |

## Requirements

- A terminal at least **86 × 30**. Below that the game shows a resize prompt
  rather than squashing the arena — the logic grid is a fixed 28 × 18 cells so
  scores stay comparable everywhere.
- **Truecolor** for the intended look. 256-colour terminals (notably macOS
  Terminal.app) fall back to the nearest indexed colours automatically.
  16-colour terminals are refused with a message instead of rendering mud.

Verified good: Windows Terminal, iTerm2, WezTerm, Alacritty, kitty, Ghostty.

## How it looks the way it does

- **Half-block rendering.** Each terminal cell holds two stacked pixels via `▀`
  with independent foreground and background colours. Terminal cells are about
  2:1 tall, so this makes the pixels square — the arena is not stretched and
  diagonal motion reads correctly.
- **Linear-light compositing.** Colours are decoded from sRGB once, composited
  in linear space, and encoded on the way out. Additive glow over
  gamma-encoded values is what makes most terminal bloom look washed out.
- **Grid logic, interpolated rendering.** Collisions, growth, and food are
  grid-exact and fair. Rendering interpolates between ticks so the snake glides
  instead of snapping.
- **Deterministic RNG.** A hand-rolled PCG32 rather than a library one: `rand`'s
  `StdRng` is explicitly non-portable across versions and platforms, which would
  silently invalidate every recorded daily-challenge seed on a dependency bump.

## Development

```bash
cargo test          # 93 tests, all logic covered
cargo run --release
```

`game/`, `render/`, and `ui/` never touch `std::io`, crossterm, the filesystem,
or the clock — time enters as `dt`, randomness as an injected RNG. The loop
lives in `main.rs` alone and `app.rs` is a pure `update`/`render` pair, so the
planned WebAssembly build (where `requestAnimationFrame` inverts control) reuses
the whole core untouched.

## Licence

MIT
