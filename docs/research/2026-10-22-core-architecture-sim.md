# Core architecture research, simulation chapter: material simulation at micro resolution

- **Date:** 2026-10-22 (player-loop session; companion to
  `2026-10-22-core-architecture.md`, gate 1 of ADR 0012)
- **Question:** can the Noita-style material simulation run at the brick
  world's 6.25 cm cells (ADR 0012 option B) under heavy activity, within the
  ADR 0009 budget on the reference PC, and what has to change?
- **Measured with:** `game/tools/micro_sim_bench.gd` (P10, README in
  `prototypes/p10-micro-sim/`), the native `SimWorld` kernel as of P5 and the
  v5 kernel built for this chapter. Reference PC, 10 worker threads, 30 Hz.

## 1. How Grimorium does it (Burkelbear Games, three devlogs)

The "3D Noita" reference that triggered ADR 0012, from its videos
("Building a 3D Noita-Style Material Simulation From Scratch", "Simulating
Hundreds of Millions of Micro-Voxels in Real-Time", "How graphics work in my
3D Noita-like game"):

- **Voxel = 16 bits:** a 12-bit material id and a 4-bit variant. Variants give
  colour variation without changing behaviour, liquid fullness (water has 8
  states and spreads while conserving mass), gas splitting and dissipation,
  and electricity flow. Over 100 M voxels fit in about 200 MB, a cube of 32
  chunks around the player.
- **Kinds:** solid, powder, liquid, gas; a 30 Hz simulation clock; lava uses
  quarter fullness steps and updates every third tick; interactions are rules
  with a probability out of 1000 per tick.
- **The simulation runs on the GPU:** compute shaders update the active
  chunks, which are handed over in groups through storage buffers; atomic
  operations resolve two threads moving into the same voxel; edits from
  spells and the generator are batched per chunk into one CPU-GPU round trip
  per frame; terrain generation is a compute shader too.
- **Chunks of 16³** with flags (in motion, contains voxels, contains point
  lights) and recursive grouping to skip whole regions; a voxel moving at a
  border activates the neighbour; unloaded chunks are walls.
- **Streaming in one fixed memory pool:** chunks are overwritten in place as
  the player moves, so the loaded region never moves in memory.
- **Explosions** cast DDA rays from the centre to the sphere's shell: each ray
  stops at a voxel harder than its remaining power, flagged voxels go in one
  pass, so the cost grows with r² rays, not r³ volume.
- **Islands:** destruction flags chunks for a once-per-frame flood fill from
  the voxels next to the hole, bounded at ~32 000 voxels (about 8 chunks).
  Reaching the bound means "attached"; anything smaller becomes a rigid body
  (marching-cubes collision mesh). The bound stops one explosion from
  collapsing a level.
- **Physics** checks trajectories at chunk level first and builds collision
  surfaces on the fly with local marching cubes; rigid bodies use Box3D.

**What we cannot copy:** a GPU simulation. Our simulation must run on the
headless Linux server as the authority (ADR 0002/0012), the server may have
no GPU, and GPU scheduling and atomics do not give a deterministic order.
We keep the CPU kernel and copy the data ideas (16-bit cells with variants,
chunk flags, bounded island checks, ray-cast explosions, a fixed pool).

## 2. P5 kernel at micro resolution (baseline)

World 32 × 16 × 32 m (67 M cells at 6.25 cm), only active 16³ chunks tick,
one cell per tick moves, whole active chunks scanned.

| event | 6.25 cm tick p50 / max | 12.5 cm tick p50 / max | dirty chunks per tick (6.25) |
|---|---|---|---|
| 8 m TNT crater in generated (static) ground | 1.7 / 2.4 ms | 0.6 / 1.2 ms | 66 |
| 2.5 m black hole, 4 m/s through a static hill | 1.6 / 2.1 ms | 0.4 / 0.8 ms | 146 |
| tree fire (6 m, canopy 6×3×6 m) | 0.6 / 0.9 ms | 0.3 / 0.8 ms | 58 |
| 6 m sand cube avalanche | 3.5 / 4.7 ms | 1.2 / 1.8 ms | 270 |
| 8×4×8 m water released | 10.5 / 12.6 ms | 2.6 / 3.9 ms | 735 |
| 8 m TNT in a hill of loose sand (worst case) | 25.6 / 36.1 ms | 4.9 / 7.6 ms | 1 642 |

Findings:
- **Cost follows the cells inside active chunks**, ~200-400 k cells scanned
  per millisecond across 10 threads, not the cells that move.
- **Typical events are cheap.** Generated ground is static until a rule
  changes it, so TNT and black holes in real terrain cost 1-2 ms per tick.
  Only bulk loose material (a whole hill of loose sand, a large flood) is
  expensive.
- **Everything is in slow motion at micro scale.** One cell per tick is
  1.9 m/s at 6.25 cm and 30 Hz: a 6 m sand drop takes 3.5 s (real: 1.1 s),
  a water front spreads ~0.45 m/s. At 12.5 cm it is 3 m/s and ~1 m/s. This
  is the biggest visible difference from the 0.5 m game, where one cell per
  tick is 15 m/s.

## 3. Kernel v5 (built for this chapter)

1. **Dirty boxes per chunk** (Noita's dirty rectangles in 3D): each chunk
   keeps the bounding box of the cells woken last tick (packed in one
   word, merged atomically from the workers); the next tick scans only that
   box grown by one cell. On by default (`set_dirty_boxes`).
2. **Fall velocity:** powders and liquids store a fall speed in the cell's
   aux byte (1/16 cell per tick) and move up to 7 cells per tick (the parity
   scheme allows < 8); landing clears it. Opt-in with `set_gravity(cells per
   tick²)`, so the 0.5 m game is unchanged (9.8 m/s² at 6.25 cm and 30 Hz is
   0.174).

| event (6.25 cm) | P5 tick p50 / max | v5 tick p50 / max |
|---|---|---|
| loose-sand TNT (worst) | 25.6 / 36.1 ms | **14.3 / 18.7 ms** |
| water release | 10.5 / 12.6 ms | **6.7 / 8.2 ms** |
| avalanche | 3.5 / 4.7 ms | **2.2 / 3.3 ms** |
| static TNT, black hole, fire | 0.6-1.7 ms | 0.5-1.6 ms |
| 6 m sand drop, first grain lands | 3.5 s | **1.5 s** (real 1.1 s) |

All 89 game tests pass with dirty boxes on (fire, shift, bubble).

## 4. Answer

**Yes, with conditions.** At 6.25 cm the CPU kernel handles the events the
game will produce most (TNT and spells in generated ground, fires, local
spills) in 0.5-2 ms per tick. Bulk loose material is the risk: the worst
case measured costs 14-19 ms of worker time per 33 ms tick after v5, which
would starve meshing and streaming if it ran unchecked, and it moves
~200 MB/s to the GPU (P7 measured ~3 GB/s, so ~2 ms of main thread per tick).

Conditions, in order of value:
1. **The tick runs off the main thread.** Today `SimBubble` calls `tick()`
   synchronously; at micro scale a bulk tick must overlap rendering (worker
   thread, dirty-chunk list handed over at the end), or a 14 ms tick lands
   in one frame.
2. **A per-tick work budget with spill-over:** when the scanned-cell count
   passes a cap, the furthest active chunks skip this tick (tick-rate level
   of detail by distance, like Grimorium's slow lava), so bulk events slow
   down far away instead of costing more.
3. **Keep generated ground static until a rule loosens it.** Whether
   disturbed ground turns loose (Noita) or keeps vertical walls (Minecraft)
   is a design choice; loosening should be local and bounded (the island
   check of §1 is the model), never "the whole hill".
4. **Liquids need faster lateral spread** (the remaining slow-motion case):
   Grimorium's fullness levels (8 states in the variant bits) let a liquid
   flatten in a few ticks instead of cell-by-cell random walks.
5. **Upload by dirty box, not by whole chunk:** the boxes v5 keeps are the
   changed region of each chunk; sending boxes (or 8³ bricks) instead of
   4 KB chunks cuts the bulk-event upload several times.
6. **Memory:** the dense window costs 6 bytes per cell (cell + move stamp):
   32 × 16 × 32 m is 400 MB, a 32 m cube 805 MB. A 24-32 m wide, 16 m tall
   bubble inside the 32 m render field fits the 4 GB budget; a brick-sparse
   kernel (only expanded bricks resident) is the later step if it does not.

**12.5 cm** would be 4-5× cheaper with twice the speeds, but it halves the
chisel resolution the brick world exists for; it stays the fallback named in
ADR 0012, not the recommendation.

## 5. Kernel v6 (built after the first answer)

- **Background tick:** `start_tick()` / `finish_tick()`; every other call that
  touches cells waits for a running tick. The game's sim bubble now starts a
  tick, lets it overlap the frame, and pastes/loads chunks only while the
  simulation is idle. In game (0.5 m, forest fire with 574 voxels lit): p99
  3.3 ms, worst frame 7.7 ms (16.7 ms before), worst main-thread bubble cost
  0.39 ms; flight at 20 m/s p99 2.1 ms. A background tick produces exactly the
  same cells as an inline tick (test).
- **Level of detail and budget:** chunks beyond a radius from the focus tick
  every n-th tick; a per-tick cell budget keeps the nearest and longest
  waiting chunks and defers the rest with their activity and dirty box, so
  work is delayed, never lost (test).
- **Static ground loosens next to changes (ADR 0013):** Noita-style collapse
  of undermined ground, broken shores and slumping sand walls, triggered only
  by fresh changes so a loaded world never collapses by itself.

| event (6.25 cm, v6, gravity on) | tick p50 / max |
|---|---|
| 8 m crater in static ground, collapsing | 2.8 / 3.4 ms |
| black hole through a static hill, collapsing | 3.6 / 5.8 ms |
| water release | 6.7 / 7.9 ms |
| loose-hill TNT, no budget | 14.1 / 18.8 ms |
| loose-hill TNT, 1.5 M-cell budget + LOD | **7.0 / 7.9 ms** |
| water release, same budget | 5.6 / 7.8 ms |

With the background tick and the budget, the worst case measured costs at
most a quarter of the 33 ms tick period on 10 threads and nothing on the main
thread except the start call and the dirty-chunk paste (budgeted, ADR 0010).
Conditions 1, 2 and 3 of section 4 are met.

## 6. Next spikes

- Liquid fullness in the 4 variant bits (16-bit cells, id + variant), the
  flood scene as the benchmark: the remaining slow-motion case.
- Upload by dirty box (or 8³ brick) instead of whole 16³ chunks.
- The island check and ray-cast explosions of section 1, on bricks.
- A brick-sparse kernel if the dense window's 6 bytes per cell outgrow the
  RAM budget.
