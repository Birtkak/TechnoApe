# Speed debt

What features cost that has not been won back yet (CLAUDE.md principle 14, ADR 0096). One row per feature,
written when it is merged, from one measurement on a quiet PC (`play` build, quick runs, the median of three).
An optimisation batch works down this list, most expensive first, CPU before GPU, and strikes rows out.

Budget (ADR 0004): frame p99 <= 8.3 ms, no frame > 16.7 ms. Main on 2026-11-13: fly GPU p50 2.9 ms,
frame p99 6.3 ms, frame-thread work 0.58 ms; sim work 0.59 ms.

| Feature (ADR) | Run | CPU: frame thread, jobs | GPU | Frame p99 | Ideas to make it cheaper |
|---|---|---|---|---|---|
| Trees with branches, clumped crowns, denser forests (0092) | `tree_chunk_cost`, 621 forest chunks, one thread, median of 3 | frame thread none; a forest chunk's first generation 1408 us (main 509-585), cached 141 us (same), mesh 130 us (same) | quads per forest chunk 3112 fast, 6435 fancy (main 1920, 2248) | not measured | build each tree's limbs and clumps once per tile, not per cell; skip cells under the fork that are not trunk; look up only the trees whose crown reaches a column (REACH 6 is the oak's); the river check in `candidate` (16 `water_at`) only near rivers; merge a crown's inner leaf faces in fancy mode |
| Rubble in mines (0093) | `fall::rubble_mine_cost` (engine, play build; server tick, no frame run) | server tick: 5,766 resting rubble 9 ns a tick; a drop of 25,473 moves 306 ns a visit, worst tick 0.66 ms; client: at most 2 chunk mesh jobs a tick during a drop, none at rest | none beyond the blocks in the mesh | not measured (no frame run; resting rubble is ordinary blocks) | a drop over many chunks remeshes each chunk once a tick: batch a chunk's moves over 2-3 ticks; a fallen column could move as one edit of n cells; the budget (1,024 visits) visits lowest first, so a big drop holds others back a few ticks |
| Ore halo (0095) | `tests/gen_cost.rs`: the 6 705 level-0 chunks within reach of a deep body, 4 km planet, seed 1337 | frame thread none; jobs: `generate` 214 µs a chunk against 133 (+81 µs, +61 %), 0.54 s of worker CPU for all of them once | none | not measured (no run changes) | the halo pass costs cells that are not rock or lie outside every grown body: cut each column to its grown body's z span from (u, v) instead of the box; keep `Gen::Solid` for a chunk wholly inside one grade; a sin/cos table per body |
| Cracks in rock (0094) | `run=fly quick=1` x3 (play build); `crack::drop_cost` (engine, 4 km planet) | frame thread none; jobs: mesh 359 us a chunk against 332 (+8 %); server: a drop's search 4.7 us a dig, 57.7 us at worst, a crack lookup 0.66 us | none measurable (p50 2.86 ms) | 6.2 ms, unchanged | the corner tops call the fold noise once a corner: interpolate them from the tile's column tops; skip the crack arrays for a chunk whose exposed rock does not crack (a bit from generation); the drop's lookups could share a column-top cache |
