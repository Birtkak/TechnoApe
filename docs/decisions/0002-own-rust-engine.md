# 0002 – Our own engine in Rust + wgpu, and what it does differently

- **Status:** accepted
- **Date:** 2026-11-09

## Context
ADR 0001 left the engine to two spikes on one shared benchmark (a planet and moon, `docs/plans/roadmap.md`):
p14 drew it with our own renderer inside Godot 4.7.2, p15 in a small engine of our own in Rust + wgpu. The rule
was "our own engine unless Godot proves itself". Both spikes live on their own branches with README and results:
p14 on `claude/suspicious-taussig-3df52e`, p15 on `claude/p15-engine-spike-251147`.

Measured on the reference machine (ADR 0004), release builds, median of 3 reps:

| | p14 Godot + own renderer | p15 Rust + wgpu |
|---|---|---|
| stand fps / p99 ms | 1,588 / 1.15 | 2,033 / 0.81 |
| fly fps / missing chunks | 1,497 / 232 (**fails the gate**) | 1,650 / 0 |
| edit fps / CPU p99 ms | 575 / 4.80 | 1,587 / 0.84 |
| RAM / VRAM MB | 695 / 146 | 148 / 495 |

Both held the frame budget and neither showed holes, but Godot failed the flight gate and spent 5x the CPU on
edits. Most of the renderer's speed comes from its design (binary greedy meshing, 8-byte quads in one buffer,
front-to-back order, horizon culling). A profile of the old Godot game showed its worst frames (62-79 ms) came
from moving the simulation window on the main thread, not from rendering: the new design must keep such work
off the frame from the start.

## Decision
- **The game is built on our own engine in Rust with wgpu**, starting from the p15 spike's renderer and
  streamer.
- **Everything is new.** No code, data or content from the old Godot game is carried over or ported; it
  stays in git history only.
- **Gameplay is Rust; content is data.** No script language. Items, recipes, machines, materials and biomes
  are data files with a schema and tests.
- **Multiplayer from day one.** Singleplayer runs a local server; every system is server-authoritative from
  its first version and sends deltas.
- **No frame stalls by design.** Streaming, meshing, simulation and saving run on worker threads; the frame
  thread only swaps finished results in, under a per-frame budget. Every system ships with its benchmark.
- **No simulation bubble.** The whole loaded area keeps simulating, full rate near the player and lower rates
  farther out.
- **Liquids are fill levels plus fine splashes.** Each 0.5 m block holds a fill level (smooth surface, flows,
  cheap everywhere); near the player it breaks into 12.5 cm cells for splashes and pouring, and settles back.
- **One pixel grid for every block, item and model:** one texel size everywhere, **16 px per 0.5 m block**
  (3.125 cm per texel), 16x16 pixel textures rather than flat colours. Settled 2026-11-10 by the block look
  spike (`prototypes/block-look/`). Every model is built on that grid, so sizes never mismatch.
- **Lighting:** block light from torches and lamps, the sun from its real direction around the planet, corner
  occlusion, dark caves, and real-time shadow maps for the sun and moving lights.
- **A day is about 20 minutes**; the sun really circles the planet, so the night side is dark.
- **Movement:** snappy first-person walking and jumping in radial gravity; a jetpack or ship unlocks the climb
  to orbit.
- **Saves hold only what changed:** untouched terrain is rebuilt from the seed; saves keep edits, entities and
  simulation state. Generator changes therefore alter untouched ground in old worlds.
- **UI:** pixel-art panels and slot grids on the same pixel grid as the world, keyboard and mouse, drag and
  drop, shift-click, search.
- **Content is designed fresh for planets.** The first planet is Earth-like with varied biomes (oceans,
  forests, deserts, mountains, caves, ores); the moon is barren with other ores.
- **Rich audio early:** 3D sound with cave reverb, material-based footsteps and digging, wind and water from
  the first playable.
- **Bots are clients.** A bot is a windowless client on the same protocol as a player (move, look, dig, place,
  use, craft in; world state out). Tests drive bots from a script or live over a control channel ("walk to X,
  dig, pour water, wait, report") and get JSON state and filmstrips back; many bots load-test the server. In
  the game, **AI players play on their own** (gather, build, settle): a brain (goals, pathfinding, utility AI)
  on top of the same client. Test bots come with the first playable, AI players after the core loop.
- **Checks:** `cargo test`, plus a headless run that prints JSON stats and a filmstrip in under a minute. Long
  benchmark series only for performance decisions.
- **First playable:** one planet and its moon, radial gravity, walk, dig and place blocks, water as fill
  levels, sound, saving, test bots. No machines yet.
- Libraries are picked per system when that system is built, each in its own ADR (UI, audio, physics,
  networking).

## Consequences
- The renderer is the small part. UI, audio, shadows, particles, transparency, tools and all gameplay are
  written by us. No time estimate: nothing measures the speed of that work yet.
- No editor and no hot reload: tests are `cargo test`, scenes are code and data files.
