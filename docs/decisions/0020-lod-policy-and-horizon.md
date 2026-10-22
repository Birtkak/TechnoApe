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
- A coarse cell is water only when at least half of it is below the water
  level (lod 0 unchanged), so lakes and seas keep their level at every LOD.
- `test_world_gen_lod.gd` covers the surface rule, the soil/rock majority,
  the LOD 0 layers and the far canopy cells.

**Rings (settings.gd `lod_ring_m`, world.gd):** `secondary_lod_distance` is
set explicitly; LOD n ends at `render_distance_m + lod_ring_m * 2^n`.

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
primitives, 2 GB VRAM, p99 10 ms, 9 s load. Detail further out comes from
the rings and the generator policy, not from a bigger LOD0.

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

Budget matrix (ADR 0014 scenarios, 20 s after load) on the new defaults
(our build, rings 48 m, fade 0.35 s, threaded update, no terrain.material):

| Run | load s | p50 | p99 | max | > 8.3 | > 16.7 | RSS MB | VRAM MB | ok |
|---|---|---|---|---|---|---|---|---|---|
| fly 6 m/s | 3.9 | 2.15 | 3.12 | 4.3 | 0 | 0 | 1,169 | 471 | yes |
| fast fly 40 m/s | 4.8 | 2.81 | 4.24 | 10.8 | 1 | 0 | 1,224 | 954 | yes |
| walk + mine + place | 4.1 | 2.80 | 3.62 | 6.1 | 0 | 0 | 1,228 | 728 | yes |
| forest fire | 4.2 | 3.21 | 6.36 | 7.9 | 0 | 0 | 1,249 | 1,149 | yes |
| water spill 1.5 m / 2 s | 4.1 | 2.60 | 8.86 | 16.4 | 103 | 0 | 1,221 | 728 | p99 over, as before |
| blast 8 m / 2 s | 4.2 | 2.44 | 3.90 | 5.8 | 0 | 0 | 1,194 | 721 | yes |
| 50 x 1 m craters / 2 s | 4.1 | 2.47 | 3.28 | 4.2 | 0 | 0 | 1,211 | 726 | yes |

Against ADR 0017's numbers (fly p99 2.6, fire 7.0, spill 8.8): the fire got
cheaper (fewer per-block materials), flight pays ~0.4 ms for the wider rings
and the see-through water, the spill is unchanged. Load time 2.8 → 4 s
(initial clipbox rings hold more blocks).

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
  run at start-up) is older than this ADR (soak logs of ADR 0016 show it):
  an edit lands before the parent LOD block exists in the data map.
- Water spill p99 8.7 ms is unchanged (ADR 0016/0017 open item).

## Consequences
- Far forests are leaf-coloured ground, not trees: a far hill reads as
  forest, and the LOD2 → LOD3 switch replaces canopy blobs with a flat
  leaf-coloured cell at the same place.
- A tree cell lookup per column at lod ≥ 3 (one extra noise column per
  generated column in forested biomes); measured in load time below.
