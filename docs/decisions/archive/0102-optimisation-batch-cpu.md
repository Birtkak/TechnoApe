# 0102 – Optimisation batch: CPU

- **Status:** accepted
- **Date:** 2026-11-13

## Context
The CPU half of the first optimisation batch (CLAUDE.md principle 14, ADR 0096): win back worker and
frame-thread time the 11-13 features cost (`docs/perf-debt.md`) without changing what the game looks like or does.
World generation stays bit-identical: saves and bakes stay valid, `made_hash` unchanged. Every fast path keeps
the slow one as its test's reference and was seen failing when broken on purpose.

## Options considered
1. A coarser canopy (one sample a coarse column at levels 3-4, ADR 0100's idea): cheaper, but the far forest
   would look different. Not taken: this batch keeps the look.
2. A cache of tree spots across tiles (`Baked::spots`): a level-4 tile asks some 19k spots, the cache holds 65k
   and is cleared when full; tiles of one level share only their margins. Not taken.
3. Doing less work per spot and per sample, the same answer: taken.

## Decision
The wins, each measured on the play build with the ignored cost tests (`far_tile_cost`, `tree_chunk_cost`, one
thread, on a PC shared with other agents, so a few tens of percent of noise) and the fly run (median of three):

| Win | Before | After | How measured |
|---|---|---|---|
| Far tiles ask only for trees: `tree_candidate` turns a spot down before its ground (non-tree plants) and before its biome (a clearing, or a roll that picks a tree in no biome: `Rows::tree_rolls`) | see the fourth row | | |
| The far canopy's sample walks each tree's column down from its top and stops at its first block (`plant_top_with`), not every block of the column | | | |
| A tile's spots in a dense grid, not a hash map (the lookup cost more than a far sample) | | | |
| Far tile over forest, all three, median µs a tile, levels 1/2/3/4 | 1152 / 2535 / 4678 / 8261 | 771 / 1409 / 1931 / 4418 | `far_tile_cost` |
| A tree's limbs and clumps built once a column (`Plant::shape_of`, `column`), not once a block | forest chunk's first generation 1481 µs | 1043 µs (with the spot grid) | `tree_chunk_cost` |
| Fly, all of them: worker job | `job_us_avg` 1459 µs, after load 901 | 920 µs, after load 638; frame p99 9.34 -> 9.39 ms and the frame thread's stages (table 0.175 -> 0.178, submit 0.42 -> 0.50, rt 0.12 -> 0.12 ms p50) unchanged within noise | `run=fly quick=1 seed=1337 save=0`, play build, median of 3 |

Tests: `biomes::plants_are_the_same_block_for_block` pins a hash of every block of every plant at every height
and 256 hashes, taken from the code before the change, and holds `column` to `block` per layer (seen failing with
a limb's elbow moved a layer); `world::the_fast_canopy_matches_the_plants` holds `tree_candidate` to
`candidate(..).filter(trunk)` at every spot, `plant_top_with` to `plants_with(..).last()` over seam strips and a
band and on made-up ties of wood and leaves, `tree_rolls` to every biome's rolls, and whole tiles of levels 0-4
from the grid to the slow path (plants, trunks, canopy heights; seen failing with the wood-over-wood tie or the
roll bound broken).

Left, and why:
- Plant rolls in the mesh jobs (+47 µs a level-0 chunk, ADR 0091), the ore halo (+81 µs a chunk near a deep body,
  0095) and the cracks' corner tops (+8 % of a mesh, 0094): each under 5 % of an average job now; not worth a
  batch's risk.
- Frame thread: `table` and `submit` are wgpu's encoding and validation of a few thousand chunks' draws and
  writes; `rt`'s spikes are BLAS creation and the TLAS rebuild (wgpu walks every instance on a rebuild). Both
  belong to the ray-tracing side (`rt.rs`), owned by another agent this batch. Ideas: on a TLAS rebuild set
  only the instances whose (offset, BLAS) changed instead of every one; spread BLAS creation over more frames
  (`BUILDS_MAX` 256 builds a frame at most); grow the TLAS by `update` rather than a full build when only a few
  instances came.
- The streamer's walk (`select`, p99 1.65 ms on the frames that walk, the look-ahead's every fourth frame
  included) is the frame thread's largest spike and was not touched by the 11-13 features; a candidate for the
  next batch (walk on a worker, or keep the look-ahead's walk to the chunks the real walk left pending).

Review (a fresh reviewer given the diff and the task): no output-changing bug found. Taken: the round reach checked
before a crown is built for a corner column in `plant_top_with`; the shapes listed in `shape_of`, not `_`;
`Crown`, `shape_of` and `cell` crate-private; a needless derive dropped; `may_tree` also tested at each interval's
ends and the floats beside them. Left: the spot grid is allocated per tile (some 0.6 MB zeroed at level 4, inside
the numbers above).

## Consequences
A plant shape now has a build step (`shape_of`) and a per-cell step (`cell`); a new shape with limbs or clumps
puts what does not depend on the cell in its `Crown`. `tree_rolls` is derived from `data/biomes.toml` at load.
