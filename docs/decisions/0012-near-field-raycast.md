# 0012 – Core architecture: micro-voxel bricks, ray-cast near field, meshed far field

- **Status:** accepted 2026-10-22 ("let's go with option B as long as it works
  for what we want"). Gate 1 (simulation at 6.25 cm, P10 and the sim chapter)
  passed **with conditions**; gate 2 (brick store + ray-cast feed + sliding
  window, P9) passed. Step 3 (the near field in the game, from a store the
  bubble writes) landed behind `near_field=1` on 2026-10-23 (ADR 0021). Each
  remaining step still lands only after its spike meets the ADR 0009 budget.
- **Date:** 2026-10-22

## Context
The user asked, after Burkelbear Games' Grimorium video, whether ray-casting
the voxels like that developer does would be better than our meshing, and
widened it to "really research what is best for what we want, no problem if
we need to change basics". Requirements that drive it: every voxel chiselable
(ADR 0006), Noita-style simulated materials, mass edits (black holes, a
thousand TNT) that never choke, 2 km horizons, a solo developer, the ADR 0009
budget (p99 <= 8.3 ms, RAM <= 4 GB, VRAM <= 6 GB on an RTX 3070).

What was measured (P7, P8b): a 32 m cube at 6.25 cm (134 M cells) ray-cast
by a two-level DDA with brick occupancy, sun shadow and depth compositing
over meshes runs at 3.3 ms p50 / 6.1 ms p99; fed by the native simulation at
30 Hz with dirty chunks only it runs at 2.8 ms p50 / 5.6 ms p99 including
the tick, and an 8 m TNT box carve costs 10 ms with no visible spike.
Uploads are ~3 GB/s, so edits must be tracked per brick, never pushed whole.

## Options considered
1. Status quo plus: meshed 0.5 m world, meshed chisel overlays, 0.5 m sim,
   ADR 0010/0011 mitigations. Cheapest; mass edits bounded by remeshing;
   simulation stays coarse.
2. **Brickmap world: the 6.25 cm cell is the world's grid, stored as bricks
   that are today's 0.5 m voxels (uniform bricks = one byte, expanded bricks
   = 512 cells); ray-cast near field (32 m) fed from the simulation at brick
   granularity; godot_voxel meshes the uniform-brick view with LOD beyond.**
3. Full micro-voxel ray-cast world, Grimorium/Teardown style: no meshing,
   streamed LOD volumes, custom renderer for everything. A year of work, no
   literature support for runtime-edited LOD volumes, conflicts with
   building on a grid.
4. Halve the base voxel to 0.25 m and keep meshing: most of the cost, none
   of the goals.

## Decision
Option 2, in this order, each step a measured spike before it lands:
1. Simulation at micro resolution: **passed with conditions** (P10, kernel
   v5: generated-ground events 0.5-2 ms/tick at 30 Hz on 10 threads; bulk
   loose material 14-19 ms/tick worst case). The conditions are part of this
   decision: the tick runs off the main thread; a per-tick work cap with
   distance tick-rate LOD; generated ground stays static until a bounded
   rule loosens it; liquid fullness levels for spread speed; uploads by
   dirty box or brick, not whole chunks; the dense sim window is 24-32 m
   wide by 16 m tall inside the 32 m render window (6 B/cell), with a
   brick-sparse kernel (only expanded bricks resident) as the later step.
   12.5 cm stays the fallback if those conditions cannot be met.
2. Brick store and feed: **passed** (P9: 64 m of generated world = 10 MB,
   crater 0.4 ms; the caster renders the generator from bricks at 2.8-5.8
   ms p50 with a sliding window; shadows into Godot's shadow maps and
   see-through water work).
3. Next: kernel v6 (async tick, distance LOD, scanned-cell cap), then the
   game's `World` owns a `BrickStore` (generator -> bricks, godot_voxel reads
   the majority view, the near field renders from bricks, the sim bubble is
   the store's active set), then chiseling as brick expansion and the end of
   ADR 0006's overlay meshes. The 32 m seam against the meshed far field is
   the one untested piece and is measured in that integration.
Godot stays the engine: P7 crossed the only seam that mattered (depth), and
every other system we use is Godot's.

Mass-edit rule, effective immediately for every edit path: changes are
tracked per brick and uploaded/remeshed under a per-frame byte budget,
queued across frames; no single-frame whole-volume pushes.

## Consequences
- Easier: chiseling at any shape for free, instant edits in the near field,
  Noita-grade detail in the bubble, one data model for building, chisel and
  simulation, the generator and horizons untouched.
- Harder: two renderers to keep visually consistent, the simulation rewrite,
  voxel-grid collision everywhere (already done for the player and items),
  a server that simulates bricks headless and ships dirty bricks as deltas.
- Revisit: if the sim spike shows micro resolution cannot tick within budget
  even with a sparse active set, fall back to 12.5 cm cells (same cell count
  for 64 m) or to option 1 with the mass-edit rule.
