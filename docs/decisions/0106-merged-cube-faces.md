# ADR 0106: The blocky mesher merges cube faces that shade alike

Status: accepted (2026-10-29). Follows ADR 0105 (the full-detail box gets cheaper instead of a far layer).

## Context
After ADR 0105 the GPU's triangle count set the floor of a 72-100 m box: with shadows off a 72 m flight still drew
5.8 M triangles, and godot_voxel's `VoxelMesherBlocky` draws two triangles for every visible voxel face. At
12.5 cm voxels a field, a cliff or a forest floor is thousands of faces in one plane with one material.

Merging faces into rectangles (greedy meshing) is only exact if the picture does not change:
- **Texture:** the opaque terrain shader (`voxel_sample` in `terrain_common.gdshaderinc`) uses the UV only to find
  the material's atlas tile and picks each voxel's colour from its world position, so a rectangle whose four
  vertices carry the tile's corner UVs draws the same voxels as the single faces. Animated tiles (water, lava,
  fire) are fluids or transparent and do not merge.
- **Ambient occlusion:** godot_voxel bakes it into vertex colours per face corner. Merging only faces without any
  occlusion cut a probe of 30 surface blocks from 452 k to 396 k triangles (-12 %); with occlusion off it would
  have been 125 k. Occlusion is everywhere on a 12.5 cm hillside (every step's foot), so it decides the gain.

## Decision
`voxel_patch_merge_faces.py` (godot_voxel): faces of plain cubes (one surface, nothing inside, not a fluid, not
transparent, a full side whose four vertices are the unit square) are not drawn in the voxel loop but marked per
voxel and side with a code for their corner occlusion, then merged per side and slice:
- the same occlusion on all four corners: grows into a rectangle;
- occlusion that changes only across one axis (a step's foot, a wall's base): grows into a strip along the other
  axis. A colour that is linear across one axis is interpolated exactly by any triangle, so this is exact too;
- anything else is drawn as before.
The merged vertex colour is the single face's (1 - darkness x occlusion at that corner, times the model colour).
A tint mode turns merging off (colours per voxel); `VoxelMesherBlocky.set_merge_faces(false)` and the
`merge_faces=0` arg turn it off for A/B runs (`Settings.merge_faces`, default on).

## Measurements
Probe (30 surface blocks, 64 voxels + padding): 452 k triangles -> 199 k (-56 %).

Spectator autopilot flights (seed 272905368, 20 s, vsync off, the same run with `merge_faces=0` / `=1`):

| box | p99 frame | frames > 16.7 ms | primitives | GPU p99 | VRAM |
|---|---|---|---|---|---|
| 40 m, merged | 4.4 ms | 0 | 8.2 M | 3.6 ms | 0.9 GB |
| 72 m, off | 10.5 ms | 0 | 13.4 M | 7.8 ms | 2.5 GB |
| 72 m, merged | 7.4 ms | 0 | 9.2 M | 6.2 ms | 1.7 GB |
| 100 m, off | 49.0 ms | 239 | 21.8 M | 14.1 ms | 4.6 GB |
| 100 m, merged | 32.3 ms | 99 | 13.9 M | 9.8 ms | 3.2 GB |

72 m is inside the frame budget (ADR 0009) with shadows as they are. 100 m is not: its slow frames are the main
thread while the box streams in (process p99 31 ms; the loading cover lifts at its 10 s cap before the box is
full, and 28 % of near detail is missing in flight), not rendering. That is the next job.

A still frame at 72 m (`speed=0 pitch=-0.5`) with merging off and on differs only where animals moved and in a
handful of single pixels; the T-junctions merging creates between rectangles may show as such pixels.
`test_merge_faces.gd` checks a slab (6 rectangles) and a step (strips, the same vertex colours).

## Consequences
- An edit remeshes as before; the merge pass is one pass over a byte per voxel and side (2.6 MB per mesher
  thread at 64-voxel blocks).
- Faces under a non-uniform corner pattern (inner corners, single-voxel bumps) still draw singly.
- Moving occlusion into the shader (a per-voxel light/occlusion volume) would let every same-material face merge
  (the probe's -72 %); not done.
