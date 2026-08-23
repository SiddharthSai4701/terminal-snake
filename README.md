# terminal-snake

A snake game for the terminal that is actually worth looking at.

Classic snake rules underneath. Everything above them — truecolor pixel
rendering on a half-block canvas, 60fps interpolated motion, glow, particles,
unlockable themes, a daily seeded challenge — is the point.

> **Status: Phase 2 of 5 complete.** Classic mode is playable and it now looks
> the way it was meant to: a glossy interpolated ribbon with glow, afterglow
> trails, particle bursts, screen shake, and a death dissolve. Endless and Daily
> modes, combo scoring, and saved profiles land in Phase 3; themes and menus in
> Phase 4. See [the design spec](docs/superpowers/specs/2026-08-23-terminal-snake-design.md).

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
  instead of snapping: the head slides into the cell it is about to enter and
  the tail slides out of the one it is leaving.
- **Per-segment signed distance fields.** The body is a chain of antialiased
  capsules, each rasterized over its own bounding box only. The obvious
  implementation - the whole polyline's bounding box against every segment -
  costs about 22 ms per frame at length 250 and cannot hold 60fps. This one
  costs 0.08 ms and barely changes as the snake grows.
- **Real bloom.** Glow accumulates into a half-resolution buffer that gets a
  separable Gaussian blur before compositing. Additive brightness with no
  spatial spread is not glow, it is just a brighter pixel.
- **Deterministic RNG.** A hand-rolled PCG32 rather than a library one: `rand`'s
  `StdRng` is explicitly non-portable across versions and platforms, which would
  silently invalidate every recorded daily-challenge seed on a dependency bump.

## Development

```bash
cargo test                  # 141 tests
cargo test --release bench -- --nocapture   # frame-budget measurements
cargo run --release
```

`game/`, `render/`, and `ui/` never touch `std::io`, crossterm, the filesystem,
or the clock — time enters as `dt`, randomness as an injected RNG. The loop
lives in `main.rs` alone and `app.rs` is a pure `update`/`render` pair, so the
planned WebAssembly build (where `requestAnimationFrame` inverts control) reuses
the whole core untouched.

## Performance

Measured in release with a 64-segment snake and 400 live particles:

| | per frame | budget |
|---|---|---|
| scale 4 (default) | **0.98 ms** | 8.0 ms |
| scale 6 (maximum) | **1.99 ms** | 16.6 ms |

Growing the body from 14 segments to 204 moves a frame from 0.075 ms to
0.080 ms. These are asserted by tests, not just measured once.

## Licence

MIT
