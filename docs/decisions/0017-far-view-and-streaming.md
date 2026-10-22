# 0017 – Far view: constant texels, mipmaps, grass hillsides; terrain streaming

- **Status:** accepted
- **Date:** 2026-10-22 (the user: "looks ugly far away", then "it loads in
  slow ugly visible chunks … gets more detailed in different chunks at
  different intervals")

## Context
Beyond the full-detail radius godot_voxel draws each LOD ring with blocks of
2, 4, 8 … voxels, and `VoxelMesherBlocky` keeps one atlas tile per face, so
a 16 m cube stretched one 16-px tile across its side. The atlas is imported
without mipmaps (its `.import` is generated, not tracked), so every far pixel
sampled a full-resolution texel: pixel noise on mountains and sea. At LOD
the generator samples every 2^lod-th voxel, so hillsides became bands of
dirt risers under grass/snow tops. `Settings.render_distance_m` said 96 m,
but this godot_voxel build clamps `lod_distance` to 128 voxels = 64 m, so
the value was silently cut.

Streaming: blocks switch LOD the moment their own mesh is ready, so
neighbours flip at different times. Measured while flying at 20 m/s: the
voxel workers (6 threads) are idle 87 % of the time but get bursts of up to
3,500 queued blocks when a clipbox ring moves, and those bursts are what
pops in piece by piece. The CPU peaks at ~60 % of 12 threads for the load
second, then runs at 5–17 % while flying; standing still the game uses 9 %
of one core and 21 % of the GPU with vsync on.

## Decision
**Far view (terrain_common.gdshaderinc, world.gd, settings.gd).**
- The atlas gets mipmaps at load (`Image.generate_mipmaps`, world.gd).
- `terrain_sample` measures voxels-per-tile from the derivatives of world
  position against tile UV. Above 1.5 (a far LOD block) the tile repeats per
  world voxel (constant texel size), grass side tiles show the grass top,
  and past `far_fade_start_m` (150 m) the mip gradients widen so the sample
  blends toward the tile's average by `far_fade_end_m` (600 m). Gradients
  are capped at one tile per pixel, since coarser mips blend neighbouring
  atlas tiles. Near faces (voxel, micro cell, item cube) are unchanged.
- `render_distance_m` defaults to the real cap, 64 m, with the clamp noted.
- Frame cost: none measurable (fly p50 1.70 vs 1.69 ms, p99 2.55 vs 2.54).
- Dirt risers, where the LOD sampler hit a dirt or stone voxel under a thin
  grass/snow layer (at step 8 only one surface cell in eight was grass), are
  fixed in the generator: a coarse cell is classified by what it contains.
  The cell holding the surface voxel gets the top material, a buried cell is
  soil or rock by majority (`world_gen.cpp`, `test_world_gen_lod.gd`). LOD 0
  output is unchanged, so `GEN_VERSION` stays and saves still load.
- Still visible: the staircase silhouette of far LOD blocks.

**LOD cross-fade: supported, off by default.** The shader implements
godot_voxel's `u_lod_fade` with an ordered dither (`lod_fade_discard`) and
`Settings.lod_fade_s` sets `lod_fade_duration`. Measured at 0.35 s while
flying at 20 m/s: p99 2.6 → 10.0 ms, frames up to 47–61 ms, because a
3,000-block burst draws both LODs and godot_voxel updates a per-block
instance uniform on the main thread every frame. The default is 0.

**Streaming (world.gd, project.godot).** The slow loading the user sees in
play, but no benchmark showed, was the save file: autopilot runs never
persist, the game does, and `VoxelLodTerrain` asks the stream for every
block of every LOD before generating it. Each miss was a database query on
one thread. Measured flying at 20 m/s (`persist=1 seed=<unused>`):

| | load settled | backlog mean / max | workers busy |
|---|---|---|---|
| no save file (every earlier benchmark) | 2.5 s | 170 / 3,800 | 11 % |
| save file, as the game runs | never (10 s cap) | 5,600 / 12,700 | 97 % |
| save file + `set_key_cache_enabled(true)` | 2.7 s | 187 / 2,700–4,200 | 11 % |
| same, 9 worker threads | 2.3 s | 123 / 2,900 | 8 % |

So: the SQLite stream's key cache is on (misses never reach the database).
Worker threads stay at godot_voxel's default (6 of 12): 9 threads shortened
the load (2.75 → 2.4 s) and the bursts, but in the remesh-heavy scenes they
finish more meshes per frame and applying them lands on the main thread:
fire p99 7.0 → 9.1 ms, water spill 8.8 → 12.5 ms (same build, 6 vs 9
workers; trimming the simulation's threads changed nothing). Fewer workers
are the only remesh budget we have until the push path gets a real one
(ADR 0016 open item). Blocks still switch LOD one by one, but they arrive
in a quarter second instead of trickling for a minute.

After this change (HEAD 32c98e0 + this ADR, 20 s runs, 6 workers): fly p99
2.6 / max 7.2 ms, walk + mine 3.7 / 6.6, fire 7.0 / 15.7 (ADR 0016: 6.1–6.4),
water spill 8.8 / 13.9 (ADR 0016 at 16 s: 6.9; 20 s drops two more balls).
The spill at 20 s is over the p99 line; the fire and the walk are within it.

## Consequences
- `tools/anim_probe.gd` still passes (water/lava/fire animate, stone does
  not) and the budget matrix is unchanged.
- Far blocks ignore the mesher's UV orientation; textures that depend on
  their orientation (none today) would need the near path.
- The staircase horizon is geometry: a smooth far-field renderer beyond
  ~400 m (Distant Horizons style, from the generator's heightmap) is the
  next step if the user wants more, and is a spike of its own.
- The fire and spill p99s are a little higher than ADR 0016's; the far-view
  sampling and the chat/creative work (ADR 0018) landed together, so which
  one costs what is not isolated. Watch it in the next budget matrix.
- Autopilot: `voxel_threads=N`, `persist=1 seed=<unused>` (save-file load
  path), `lod_fade_s=`, `far_fade_start_m=`, `far_fade_end_m=`; the JSON
  carries `voxel_backlog` (pending voxel tasks while moving: max, mean,
  busy share, longest busy streak, thread count).
