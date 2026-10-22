# 0020 – LOD policy: detail that thins with distance, wider LOD rings, hidden LOD switches

- **Status:** accepted
- **Date:** 2026-10-22 (the user: "seamless transportation and immersion …
  chunky updates remove immersion"; "LOD quality slowly drops over distance
  so it looks like less and less foliage until far back it's just the naked
  blocks with matching colours"; "grass cuts off too quick")

## Context
ADR 0017 fixed far textures and the dirt risers. What remained: every
feature stopped at a hard edge (micro vegetation at the 64 m LOD0 edge,
trees at the LOD2 edge), lakes bloated at coarse LOD (a cell was water as
soon as its bottom voxel was under the water level), LOD rings were narrow
(godot_voxel's default `secondary_lod_distance` of 48 voxels put 2 m cubes
at 112 m and 4 m cubes at 160 m), and blocks switch LOD one parent at a time
with no fade (the fade exists but cost 30–60 ms frames).

Three of those limits live inside godot_voxel, not the game: `lod_distance`
and `secondary_lod_distance` are clamped to 128 voxels (64 m), and the LOD
fade copies every fading-out block into a new mesh instance with its own
ShaderMaterial, which godot_voxel's own comment says costs ~1 ms per instance
to free. Our shader also declared `u_lod_fade` as an *instance* uniform,
which a per-block ShaderMaterial never reaches: the earlier fade measurement
(ADR 0017) paid the full cost and showed no fade.

## Decision
**Generator (world_gen.cpp, `generate_block`), LOD 0 output unchanged:**
- lod 0: everything. lod 1: trees and micro vegetation (sampling every
  second column keeps a quarter of the tufts, drawn at twice the size, so
  cover thins instead of stopping). lod 2: trees (sampling drops most
  trunks; canopies remain as blobs). lod ≥ 3: no tree geometry; a column
  under a canopy gets a LEAVES surface cell, so forests keep their colour at
  any distance ("naked blocks with matching colours").
- Caves are carved at lod 1 too, but only within 24 voxels of the surface
  (`CAVE_MOUTH_DEPTH`): cave mouths on hillsides show to the LOD1 edge
  instead of popping in at 64 m; deep caves stay lod 0. Cave decorations
  (stalactites, pebbles, fungi) stay lod 0. lod-1 blocks: 0.18 → 0.37 ms.
- A coarse cell is water only when at least half of it is below the water
  level (lod 0 unchanged), so lakes and seas keep their level at every LOD.
- `test_world_gen_lod.gd` covers the surface rule, the soil/rock majority,
  the LOD 0 layers and the far canopy cells.

**Rings (settings.gd `lod_ring_m`, world.gd):** `secondary_lod_distance` is
set explicitly; LOD n ends at `render_distance_m + lod_ring_m * 2^n`.

**Hydrology tile cache (world_gen.cpp):** a 2 km tile (~35 ms, ~2.8 MB)
is now built outside the global lock by the first thread that asks for it
while the others wait on its future, and the cache evicts its least
recently used quarter past 96 tiles instead of clearing wholesale at 64.
First load standing still: 4.05 → 3.3 s (the builds used to serialise every
worker); flight unchanged.

**Rings:** `lod_ring_m` defaults to 48 m (2-voxel cubes to 160 m, 4-voxel to
256 m, 8-voxel to 448 m, 16-voxel to 832 m) and `terrain_threaded_update`
is on. 64 m looks better still (grass to 192 m) and holds the p99, but at
40 m/s flight the clipbox bursts give 20–27 ms frames (render-thread time
while 6,000 blocks are generated), which ADR 0009 forbids for defaults; it
stays one setting away (`lod_ring_m=64`).

**Custom godot_voxel build (prototypes/p11-lod-horizon, `voxel_patch.py`):**
the game runs our MinGW build of the same commit (045ff932) with three
patches: the LOD distance clamp raised (so the radius can be measured), no
fading copies for transition-mask changes (only transvoxel stitching needs
them), and the fade value delivered through
`instance_geometry_set_shader_parameter` when the terrain has no
ShaderMaterial. `terrain.material` is no longer set, so the block library's
per-surface materials apply: the liquid shader runs on the LOD terrain for
the first time (see-through water with a visible bottom), and blocks share
materials instead of each carrying a ShaderMaterial copy (376 vs 420 draw
calls). `lod_fade_s` defaults to 0.35 s; the shader's `u_lod_fade` is an
`instance uniform` again.

**Not adopted:** LOD0 at 128 m. Eight times the full-detail volume: 16 M
primitives, 2 GB VRAM, p99 10 ms, 9 s load. LOD0 at 96 m (3.4× the volume)
holds at 20 m/s (p50 3.0, p99 5.0, max 14 ms, 1.2 GB VRAM, load 5.8 s) but
not at 40 m/s (p99 5.8, a 19.5 ms frame). Detail further out comes from
the rings and the generator policy, not from a bigger LOD0;
`render_distance_m=96` stays a setting for a quiet walk.

## Measured
Flight at 20 m/s, 16 s after load, 1280x720, separate render thread
(prototypes/p11-lod-horizon/README.md has the full tables).

| | load s | p50 | p99 | max | VRAM MB |
|---|---|---|---|---|---|
| before (stock DLL, rings 24 m, no fade, terrain.material set) | 2.8 | 1.77 | 2.71 | 6.6 | 454 |
| stock DLL, fade 0.35 s (ADR 0017) | 2.9 | 1.89 | 9.42 | 61.6 | |
| our build, no terrain.material, fade 0.35 s, rings 24 m | 2.8 | 1.99 | 3.03 | 5.9 | 457 |
| **default now**: + rings 64 m, threaded update | 5.8 | 2.53 | 3.64 | 9.8 | 807 |

The liquid shader's transparent pass costs ~0.25 ms p50; the 64 m rings
~0.5 ms and 350 MB VRAM, and the initial load doubles (the clipbox has four
times the blocks in the first rings). The fade itself is noise.

Budget matrix (ADR 0014 scenarios, 20 s after load) on the final defaults
(our build, rings 48 m, fade 0.35 s, threaded update, no terrain.material,
2 km horizon, edit remesh rate 300/s, loading screen):

| Run | load s | p50 | p99 | max | > 8.3 | > 16.7 | RSS MB | VRAM MB | ok |
|---|---|---|---|---|---|---|---|---|---|
| fly 6 m/s | 3.4 | 2.20 | 3.12 | 4.2 | 0 | 0 | 1,197 | 488 | yes |
| fast fly 40 m/s | 4.0 | 2.73 | 4.16 | 13.2 | 2 | 0 | 1,241 | 989 | yes |
| walk + mine + place | 3.6 | 2.82 | 3.69 | 11.1 | 1 | 0 | 1,238 | 746 | yes |
| forest fire | 3.6 | 3.32 | 4.31 | 7.1 | 0 | 0 | 1,243 | 1,169 | yes |
| water spill 1.5 m / 2 s | 3.4 | 2.60 | 5.12 | 10.7 | 31 | 0 | 1,217 | 749 | yes |
| blast 8 m / 2 s | 3.6 | 2.48 | 3.23 | 10.6 | 1 | 0 | 1,218 | 738 | yes |
| 50 x 1 m craters / 2 s | 3.6 | 2.51 | 3.22 | 10.9 | 2 | 0 | 1,217 | 743 | yes |

Every scenario is within ADR 0009 for the first time (ADR 0017: fly p99 2.6,
fire 7.0, spill 8.8 over the line). Flight pays ~0.4 ms for the wider rings
and the see-through water; the fire and the spill gain from the remesh
rate. First load 2.8 → 3.4 s (more blocks in the first rings, minus the
hydrology fix).

## Open
- 40 m/s flight with 64 m rings: single 20–27 ms frames of render-thread
  time during a clipbox burst (`mesh_delta` ±8 MB, 6,000 generation tasks
  queued). The per-frame upload/remesh budget that ADR 0016 already asks for
  would cap it; with our own extension build it is now reachable.
- Walking runs show one 34 ms physics step per 20 s run, about once in
  8,000 frames, with no voxel task in flight (seen in the walk and the 8 m
  blast runs, not reproduced in two reruns). The player's `VoxelBoxMover`
  reads the terrain under its data lock; a worker holding that lock for a
  block save or cache write is the suspect. Not new to this ADR.
- "Destination block not found when cascading edits on LOD n" (1–4 per walk
  run at start-up, older than this ADR) came from the bubble pasting its first
  simulation changes before the parent LOD blocks existed. The bubble now
  holds its pushes until the loading screen finishes (`SimBubble.hold_pushes`);
  no such error in the walk and spill runs since.
- The water spill is within budget with the edit remesh rate (above).
  Measured on the way: 16-voxel mesh blocks do not help (spill p99 10.3,
  flight 80 ms frames: three times the instances) and overflow Godot's
  instance-uniform buffer; a byte cap at apply time is within noise.
- The 2 km horizon is now the default (`view_distance_m` 2048, 7 LODs):
  first load 3.9 s standing (1 km: 4.05 s), flight at 20 m/s p99 3.4 ms,
  VRAM 620 MB, 560 draw calls. With 48 m rings LOD6 reaches 3 km, so seven
  levels cover it. An eighth level (`lod_count=8`) makes the first load
  22.6 s and a 20 m/s flight never settle: lod-7 data blocks span 2 km each
  and touch more 2 km hydrology tiles than the generator's cache keeps (64,
  cleared wholesale when full, each tile rebuilt under the global mutex), so
  the workers serialize on tile rebuilds. With the tile cache fix above
  (built outside the lock, LRU past 96) an 8th level loads in 4.4 s; it is
  still not needed at 2 km.

**Remesh budget for edits (`voxel_patch_remesh.py`, `edit_remesh_rate`,
default 300/s):** the open item of ADR 0012/0016/0017. The frame stalls of a
liquid spill come from mesh uploads when worker threads build the meshes, so
a cap at apply time (`mesh_upload_budget_mb`, above) does nothing; the cap
has to sit where remeshes are *scheduled*. Mesh updates that edits request
(VoxelTool pastes, generator changes; never streaming) pass through a token
bucket in `send_mesh_requests`; the rest stay pending and go out in later
updates, and a block that is edited again while pending is still remeshed
once. Water spill 20 s: p99 8.3 → 5.1 ms, frames over 8.3 ms 58 → 15, max
10 ms; 60 s of continuous flow p99 3.8 ms, max 13.7, none over 16.7 (ADR
0016 measured 17.8 / 33.7 and 484 frames over). Mining and placing update at
once (the bucket is full when idle, burst 30). The filmstrip of the spill
shows the water spreading as before.

**Loading screen (`scripts/ui/loading_screen.gd`):** the first ~4 s, while
the clipbox rings stream in, are covered by a dark panel with a progress bar
(1 − pending / peak voxel tasks) that fades once the bubble holds its chunks
and the workers are idle for 20 frames (cap 15 s). Before it the game opened
on a world assembling block by block around the player.

## Consequences
- Far forests are leaf-coloured ground, not trees: a far hill reads as
  forest, and the LOD2 → LOD3 switch replaces canopy blobs with a flat
  leaf-coloured cell at the same place.
- A tree cell lookup per column at lod ≥ 3 (one extra noise column per
  generated column in forested biomes); measured in load time below.
