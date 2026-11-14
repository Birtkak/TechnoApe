# 0131 – The conformal grid, as a trial behind `warp=`

- **Status:** trial, awaiting the developer
- **Date:** 2026-11-15

## Context
The developer finds the blocks warped. On the equi-angular cube-sphere of ADR 0003 a block's top is a
1.09 x 0.77 m rectangle at the middle of a face edge and a 60 degree diamond near a cube corner. The spike in
`prototypes/square-blocks` measured the conformal cube-sphere on the 4 km planet: block tops stretched over 1.1
to 1 on 72.7 % of the ground today and on 0.0 % conformal; the price is size near the 8 corners: blocks under
0.75 m on 2.3 % of the ground (within about 450 m of a corner), under 0.5 m on 0.1 % (within about 90 m). Its
table does not resolve the last 32 columns before a corner, where blocks level off at about 0.26-0.32 m with
up to about 18 degrees of skew. The developer wants to walk on it before deciding.

## Options considered
1. **Replace the grid** – no switch to compare, every view and test moves at once, before anyone has walked on it.
2. **A trial behind a switch** (this) – `warp=0..1` on the client, the server and the bot; 0 is today's grid to
   the bit, so every existing test and view keeps its meaning.

## Decision
Option 2. The conformal grid is the equi-angular grid with its face coordinates warped first:

    a = (i - n/2) / (n/2),  b = (j - n/2) / (n/2)          // -1..1 across the face
    (a', b') = (a, b) + w * table(a, b)                     // w: 0 today's grid, 1 conformal
    dir = normalize(N + S tan(a' pi/4) + T tan(b' pi/4))    // ADR 0003's formula on (a', b')

`table` is the spike's 193 x 193 lattice of (da, db) f32 pairs (`crates/engine/data/warp_table.bin`, made by
`prototypes/square-blocks/warp_table.py`), bilinear between its points, the same for every face and planet
size. It is symmetric under the square's 8 symmetries with da = 0 on a = +-1, so two faces lay a shared edge on
the same points. CPU and GPU evaluate it on |a|, |b| and put the signs back, so mirrored points take the same
texels (`warp.rs::at`, the shader's `warp_at`).

**What changed**
- `engine::warp` (new): the table, `at` (value and the bilinear's own derivatives), `warp`, `unwarp` (Newton
  from one fixed-point step, to 1e-14 of a face).
- `Body::warp: f64` (0 by default; not a process global, tests run in parallel), set by
  `World::with_maps_warp` / `World::baked_warp` before the body gets its map (its spawn, caves and ores are
  laid out on the grid). `Body::dir` warps first and `Body::locate` unwarps after, both skipping the table at 0.
  `Body::warp_col` / `unwarp_col` convert between block columns and the equi-angular grid's columns.
- **The land stays where it is.** It did not by itself: the bake's map was read by face column (`i * m / n`),
  not by direction, so the warp would have slid the land by up to 0.13 of a face (about 400 columns). Every map
  lookup (`height_at`, `water_tide`, `biome_at`, `climate_at`, `year_air`, `heat`, `rock_at`,
  `ground_biome_at`, `bed`, `ice_sheet`, `river_quads`' bounds) now reads the map at `warp_col` of the column;
  map cells turned into block columns (Luna's lava tubes, the river gravel ores) go through `unwarp_col`. The
  noise detail was already by direction. What moves with the block lattice: plant spots, biome patches,
  clutter, caves and ore bodies (all laid out in block columns), so the same seed has other trees in other
  places, on the same land.
- `World::grid_at` uses `Body::locate` (the same bits at 0); the height tile builder works out each warped
  direction (the warped grid is not separable); the player's box (`phys::cells`) spans its 0.6 m in the local
  columns' width on the warped grid (near a corner a column is a third of a metre).
- The shader: the table as an rg32float texture at group 0 binding 18 (`textureLoad`, bilinear by hand), the
  strength in the body uniform's `rot0.w`. `frame_at`, `place`, `tx_point`, `tx_grid_of` (via `tx_grid_on`) and
  `lamp_target` take the warp; the side faces' normals (planes through the centre on the old grid, used in 7
  places) come from the frame as `si` / `sj` (the cross product of the direction and the grid line's tangent on
  the warped grid). `lamp_target` clamps in the block grid's coordinates on the warped grid. The inverse in the
  shader takes one fixed-point step and **3 Newton steps**: two leave 1.4 columns near a corner (the start is
  up to about 110 columns off), three under 0.005 of a column, a twelfth of a light texel
  (`warp::tests::the_shaders_three_newton_steps_suffice`).
- `warp=` on `technoape` (every run; the local server too), `technoape-server` and `technoape-bot`. It needs
  `save=0` (it is in no save; asserted).

**Left unwarped on purpose:** the bake's own grid (`bake/grid.rs`, `bake/rivers.rs`), the clouds
(`clouds.wgsl` `cl_locate`), the weather and air grid (`weather.rs`, `bake/air.rs`): each has its own
equi-angular map by direction. The globe map (`globe.rs`) reads the bake grid. `Body::lattice` still sizes the
trilinear lattice from the equi-angular column width (the warped grid's widest is 1.12 m against 1.09).
River and lake surface slopes (`vs_sea`, `vs_water`: `lnq / theta`) still take a column as `r * theta` wide,
as they already did where columns were 0.77 m.

**Tests** (each seen failing first, or by a mutation where the property holds today too):
`warp::tests::a_block_at_a_face_edge_is_square` (1.05 x 1.05 m within 3 %; failed at 1.085 x 0.768),
`a_block_near_a_corner_is_small_and_square` (0.43 m within 0.02, corner within 4 degrees; failed at 1.015 m),
`locate_undoes_dir_on_the_warped_grid` (1e-6 columns, all faces, by edges and corners),
`faces_lay_a_shared_edge_on_the_same_points` (1e-12, pairs found with `fold`),
`with_no_warp_the_grid_is_todays_to_the_bit` (the old formulas written out), `the_table_inverts`,
`the_shaders_three_newton_steps_suffice`, `world::baked_tests::the_warp_moves_the_blocks_not_the_land`
(ground along a column's direction within 1e-6 m of the unwarped world's; failed by 32.5 m without the map
fix), and in the client `gi::tests::the_warped_grid_on_the_gpu_is_the_engines` (the shader's own `tx_point`
and `tx_grid_of` on the GPU against `Body::dir`, 3 mm and 0.006 columns).

**Speed** (`play` build, quick runs, one each, `slot.sh measure`, warp 0 -> 1; row "Conformal grid trial" in
`docs/perf-debt.md`): frame p50 / p99 stand 4.32 / 4.91 -> 4.24 / 4.82 ms, fly 4.07 / 5.49 -> 4.15 / 5.60;
GPU p50 stand 3.91 -> 3.76, fly 3.55 -> 3.61 (noise); frame thread's own work level; chunk jobs +24-26 %
(stand 910 -> 1127 us, fly 595 -> 749); a world's start-up about 1.5 x.

**Seen** (`out/warp/`, not committed): checkerboards at a face's middle, edge and corner (warp 0: squares, tall
rectangles, diamonds; warp 1: squares everywhere, small at the corner); eye-level views at an edge and a corner
(the same mountain in the same place at 0 and 1); `seam` and `corner` views, cracks 0; lamps at night near a
corner (the walls lit, the posts' shadows on them); a bot at a corner on a warp=1 server landed, dug, placed
and dug sideways (3 edits applied, 0 refused).

## Consequences
- The developer can walk on it: `technoape.exe run=play onscreen=1 warp=1 save=0`.
- Limits of the trial: a client or bot joining a remote server must be given the same `warp=` by hand (it is in
  no message); no save keeps it; the bake's planet places (spawn scoring, rivers) were chosen on the old grid
  and are read through the warp, so a river is as wide in metres but crosses more or fewer columns; caves and
  ore bodies are laid out in block columns, so near a corner they shrink with the blocks; blocks within 32
  columns of a corner are 0.26-0.32 m and skewed up to 18 degrees.
- If it is kept: make it the default, drop the old path or keep it for old saves, put the warp in the save and
  the welcome, and fold the inverse's cost into the optimisation batch.
