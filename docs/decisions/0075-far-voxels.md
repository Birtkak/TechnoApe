# ADR 0075: The far horizon is voxels: our own cube meshes from the coarse data, player builds included

Status: superseded by ADR 0105 (2026-10-28: the far layer is removed; render distance only). Was: proposed (2026-10-27). Supersedes the height-field skin of ADR 0074 as the far layer's mesher; the
compositing of ADR 0074 (own SubViewport, per-pixel blend at the box's edge, far fog) stays. The 40 m box
(ADR 0073) stays the default and the far horizon stays a Settings > Video option (`far=1`) until the
developer prefers it in play.

## Context
ADR 0074 drew the land past the box as a smooth column height-field skin with instanced blob trees. Shown
the result against Lay of the Land's footage, the developer's verdict (2026-10-27): the smooth skin and
crowns "do not match the voxel vibe", and "big player structures will not show up right": a skin carries
one height per column, so a tower is a grass-coloured bump and a bridge a wall, which no tuning fixes.
The direction given: the same far look as Lay of the Land, rework and raise the budget if needed, done
right.

What the earlier voxel LODs (ADR 0020, 0072, 0073; `out/ring_64`, `out/horizon/after_pose`) got wrong was
never the voxels: 1 m cubes started at ~100 m (10 px stairs on every slope), a one-voxel water step became
a full-height dark face, the ordered dither read as stitching, and clipbox streaming hid LOD 0 under its
parents. The research note (`docs/research/2026-10-26-lay-of-the-land-distance.md`) says what to keep: a
far cell under about a pixel or three, flat water at its level, trees as blobs to the horizon, the voxel
colour speckle kept, haze doing the rest.

## Decision
1. **The far layer is voxel cubes from the voxel data.** `FarMesh::build_voxel_tile` (native,
   `prototypes/p5-sim-spike/src/far_mesh.cpp`) takes `FineGen::generate_block(lod = level + 1)` for the
   tile's volume (the generator already has lumpy voxel trees to lod 3 and a per-tree leaf dome beyond)
   and emits one quad per run of exposed solid faces of the same material (greedy along one axis), with
   the material's atlas-average colour; the far ground shader adds the per-25 cm speckle. Caves, overhangs,
   arches and undersides are what the data says.
2. **Cells never over ~3 px.** Level L cells are 2^(L + 1) voxels and ring L ends at 80 m x 2^L: 25 cm
   cubes to 80 m, 50 cm to 160 m, 1 m to 320 m, 2 m to 640 m, 4 m to 1.3 km, 8 m beyond (`R0_M` in
   `far_layer.gd`, 48 m for the skin). Rings switch hard (no morph, no dither): a cube's stairs are
   invisible at that size.
3. **Water is a flat top sheet per liquid column**, coloured by the bed under it and its depth, with no
   side faces: a shore or a river step is never a trench. Hidden solid faces under the sheet are skipped.
4. **Player builds show.** Edited chunk columns are re-read from the real voxels (as ADR 0074's edit scan)
   and kept at lod 1 (`FarMesh::edit_column`: 2-voxel cells, the highest solid voxel's material, else
   liquid, else air, so a thin wall stays); `build_voxel_tile` lays them over the generated cells at every
   level before meshing. Saved with the world (`far_edits_<seed>` JSON, base64 columns) and restored before
   the first tiles build, so a reload keeps craters and towers far off.
5. **Kept from ADR 0074:** the quadtree ring selection, stale tiles until their replacement is drawn, the
   node pool, two low-priority worker threads, the own-world SubViewport, the per-pixel blend of the near
   box into the far texture, the far fog at `far_fog` = 0.35 of the near world's, the near fluid's water
   tint. The skin (`far_mesher=field`) and surface nets (`far_mesher=nets`) stay for comparison until
   this ADR is accepted, then go.

## Measured (seed 272905368, 1280x720, vsync off, far_m 1536, ADR 0014 method)
| run | 40 m box (ADR 0073) | + skin (ADR 0074) | + far voxels |
|---|---|---|---|
| report pose (alt 40): p99 / max / VRAM / load | 3.5 / 14.7 ms / 0.56 GB | 4.8 / 9.7 ms / 0.71 GB / 2.1 s | 4.2 / 5.9 ms / 0.93 GB / 7.6 s |
| far tiles / triangles / tile build avg / max | - | 259 / 3.0 M / 14 / 240 ms | 502 / 6.2 M / 30 / 237 ms |
| 100 m up: p99 / max / VRAM | 2.9 / 7.4 ms | 3.4 / 7.4 ms | 3.5 / 12.5 ms / 0.63 GB |
| standing 20 s: p99 / max / > 16.7 / VRAM | 6.2 / 8.8 ms / 0 / 0.9 GB | 7.2-7.6 / 8.3-12.4 / 0 / 1.04 | 7.0 / 8.4 ms / 0 / 1.27 GB, `budget_ok` |
| 20 m/s flight 16 s: p99 / max / > 16.7 | 5.2 / 5.7 ms / 0 (today 4.9 / 16 / 0) | 8-11 / 50-90 / 14-25 | 10.8 / 18.8 ms / 3 |
| far tiles in flight / triangles / VRAM | - | - | 505 / 8.2 M / 1.41 GB |

What the flight took: tile meshes are built on the worker threads (an `ArrayMesh` per tile, the
upload cost the main thread 3-10 ms a tile before: 15 frames over), one tile is handed over per frame
in play (a burst of vertex buffers stalled the render thread), stale meshes are released on a worker,
and the stale-tile scan walks up the quadtree instead of down (11 -> 6 ms worst update). The three
frames left are the box's own streaming burst at a forest edge (`render_cpu` 15 ms with a mesh delta,
ADR 0074's open point; the control flight without the far layer has a 16 ms frame too) plus the
far layer's general load (p99 10.8 against 4.9).

Screens: `out/far/vpose1` / `vhigh1` / `vseam1` / `vtaiga1` (the voxel far layer) against
`out/far/pose4` / `high4` / `seam4` / `taiga4` (the skin); `out/far/vtower2/shots/far_tower2.png`, the
beach and river from 40 m up. A built tower seen through the far layer is a playtest item
(`docs/plans/to-test.md`): the scripted `/fill` + `/tp` runs (`vtower3`, `vtower4`: 16-19 edited
columns, 6-7 tile rebuilds) land the player in terrain instead of a vantage point; the data path is
`test_far_mesh.test_voxel_tile_shows_a_placed_cube_from_an_edited_column`.

## Consequences
- On: +0.8 ms p99 standing, +0.35 GB VRAM, first load 3-10 s instead of 2-4 (the loading screen waits
  for the rings to 640 m; the two far ones fill in behind the haze), the horizon at 1.5 km in voxels,
  player builds and craters visible from anywhere, trees as the generator's own voxel trees to 320 m
  and its leaf domes beyond.
- Open: the 20 m/s flight has 3 frames over 16.7 ms (above); the first load's length; felled far
  trees stand until their column is touched (the edit scan reads chunk columns, a felled tree is one).
- The far shaders' morph and relief are off for voxel tiles (`morph_start` 1e9, `relief` 0); the
  speckle stays.
- Tests: `test_far_mesh` (`edit_column`, the voxel tile's closed cubes and water sheets, a placed cube from
  an edited column, the save round trip).
