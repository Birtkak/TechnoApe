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

## What changes from the Godot game (developer, 2026-11-09)
Asked before any engine code, to fix what did not work: liquids, the sim bubble, LOD, and textures and
models of mismatched sizes.
- **Gameplay in Rust, content as data files.** No script language; items, recipes, machines, materials and
  biomes stay data + schema + tests (rule 6).
- **Multiplayer from day one.** Singleplayer runs a local server; every system is server-authoritative from
  its first version (rule 7), no retrofit.
- **No sim bubble.** The whole loaded area keeps simulating on worker threads, full rate near the player and
  lower rates farther out; no window edge, no shift on the frame thread.
- **Liquids as fill levels plus fine splashes.** Each 0.5 m block holds a fill level (smooth surface, flows,
  cheap everywhere); near the player it breaks into 12.5 cm cells for splashes and pouring, and settles back.
- **LOD visible but tidy.** Far terrain is honestly drawn in bigger blocks with clean seams, no cross-fade;
  the look is part of the style. Revises ADR 0170's "hidden" wish.
- **One pixel grid for every block, item and model:** one texel size everywhere (16 px per 0.5 m block, so
  3.125 cm per texel unless the look spike changes the number); every model is built on that grid, so sizes
  never mismatch.
- **First playable:** one planet and its moon, radial gravity, walk, dig and place blocks, water as fill
  levels, saving. No machines yet.
- **Checks:** `cargo test` plus a headless run that prints JSON stats and a filmstrip in under a minute; long
  benchmark series only for performance decisions.

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
