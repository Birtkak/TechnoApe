# 0171: A custom engine in Rust + wgpu for the planets game

Status: accepted (2026-11-09, developer). Replaces Godot (ADR 0001) and godot_voxel (ADR 0003) for the planets
game of ADR 0170. The current Godot game in `game/` stays as it is: no new work, kept as the reference for porting.

## Context
ADR 0170 left the engine to two spikes on one shared benchmark (`docs/plans/planet-rebuild.md`): p14 draws the
planet with our own renderer inside Godot 4.7.2, p15 draws it in a small engine of our own in Rust + wgpu. The
plan's rule was "a lean rewrite unless Godot proves itself".

Measured on the dev box, 2026-11-09, release builds, median of 3 reps
(`prototypes/p14-godot-planet/README.md`, `prototypes/p15-engine-spike/README.md`):

| | p14 Godot + own renderer | p15 Rust + wgpu |
|---|---|---|
| stand fps / p99 ms | 1,588 / 1.15 | 2,033 / 0.81 |
| fly fps / missing chunks | 1,497 / 232 (**fails the gate**) | 1,650 / 0 (after `opt`) |
| edit fps / CPU p99 ms | 575 / 4.80 | 1,587 / 0.84 |
| RAM / VRAM MB | 695 / 146 | 148 / 495 |

Both hold the ADR 0009 budget with room to spare and neither shows holes. Godot did not prove itself: it fails
the `fly` gate and spends 5x the CPU on edits. Most of the renderer's speed comes from its design (binary
greedy meshing, 8-byte quads in one buffer, front-to-back order, horizon culling), which works in either engine.

A spectator flight in today's Godot game (2026-11-09) showed its frame spikes (62-79 ms) all come from the sim
bubble shifting on the main thread, not from rendering (render CPU p99 2.9 ms). A rewrite only helps there if
the new design keeps such work off the frame from the start.

## Decision
- **The planets game is built on our own engine in Rust with wgpu**, starting from `prototypes/p15-engine-spike`.
- **Native C++ is kept**, not ported: the simulation (`prototypes/p5-sim-spike/src`) and the generator
  (`src/`) are built as a C library and called from Rust over FFI; only the Godot binding layer goes. A port
  to Rust can come later, one system at a time, if it pays.
- **Gameplay (70,300 lines GDScript) is ported by hand**, in the order of wave 3 in the plan, on top of the new
  world rather than translated line for line.
- **No per-frame stalls by design:** streaming, meshing, sim window moves and saves run on worker threads; the
  frame thread only swaps finished results in, under a per-frame budget. Every system ships with its benchmark
  (rule 9).
- **Debug reach stays** (rule 10): JSON stats on stdout, logs, state dumps, filmstrips, headless runs.
- Libraries are picked per system when that system is built, each recorded in its own ADR (UI, audio,
  physics for rigid pieces, networking).

## Consequences
- The renderer is the small part. UI, audio, lights and shadows, particles, transparency, tooling and all
  gameplay are written or ported by us; p15's README lists each part. No time estimate: nothing measures the
  porting speed yet.
- No editor and no hot reload; tests move from gdUnit4 to `cargo test`, scenes to code and data files.
- CLAUDE.md's stack, run and test instructions change when the first Rust game code lands; until then they
  describe `game/`.
- Data stays data (rule 6): items, recipes, materials and biomes in `game/data` carry over as files.
- Supersedes ADR 0001 and 0003 for new work, and the Godot-specific ADRs built on them (for example 0016,
  Godot's render thread).
