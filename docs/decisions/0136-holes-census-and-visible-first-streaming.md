# ADR 0136: A census of see-through holes, and streaming what the camera sees first

Status: accepted (2026-11-04). Builds on the task heap and sky blocks of ADR 0108 and the ground shell of ADR 0105.

## Context
Bug report 2026-11-03_23-05-16 (seed 1935341982, voxel 1148 1877 5955, creative fly, 56 m): "the world still loads
slow when exploring around quickly, lots of moments where you can see through the world". The log also had
`Out of occlusion page layers (ADR 0107)`. Before fixing anything, we needed to know why the ground in view was
not drawn.

## Decision
**Census.** `voxel_patch_holes.py` adds `VoxelLodTerrain.debug_classify_mesh_blocks(xyz)`, which returns the
pipeline stage of each lod 0 mesh block. With `holes=1`, `debug_capture.gd` samples every 0.25 s the ground mesh
blocks (from FineGen's `surface_map`, 3x3 per mesh column) that sit in the camera frustum inside the solid part of
the box, and classifies them. It counts drawn, hidden, not_in_box, generating, mesh_pending, meshing, upload_wait
and meshed_empty, and checks a meshed_empty block against the voxels before calling it a hole (a uniform block
legitimately makes no mesh). The JSON gets `holes` (by kind, by distance band, remesh gaps, series) and
`requests` (lod 0 data requests, buried ones). The census is opt-in: it costs up to ~40 ms a sample.

**Findings** (autopilot spectator fly, follow_ground 6 m, 20 m/s, 72 m, through mountains):
- 95 % of the holes are "generating": a data block of the mesh block's padded area is still queued. Meshing,
  upload and remesh gaps are each under 3 %. The rest are culling and mesher faults, which were zero.
- The occlusion page warning is not a hole cause: when the layers run out, merged faces draw unshaded.
- Demand is far above throughput: about 30 k lod 0 data requests/s against about 6 k generated/s. The mountain
  ground band is ~41 data blocks deep per column, and 18 % of requests are near-cube blocks under the band.
- Workers are only ~50 % busy. The suspect is the 200 ms heap refresh under `_tasks_mutex`, which belongs to the
  stream-throughput work.

**Fix** (`voxel_patch_visible_first.py`, `view_priority_weight` in settings.gd, default 3):
- `PriorityDependency::evaluate` measures distance from a point up to 32 m ahead of the camera (1 s of smoothed
  velocity) and multiplies it by `1 + w (1 - cos)` of the angle off the view direction. Ground in front and ahead
  goes first. The weight is 0 while a loading cover is up, because a cover waits for the ground all around.
- Lod 0 data requests take their mesh block's centre as their priority position, so the 216 data blocks one mesh
  block needs finish together instead of in distance rings.
- When the viewer is above the ground, data blocks under the ground band (near-cube filler) get +1024 voxels of
  priority distance. Blocks inside the band get their depth below the surface.
- Meshing tasks get band2 11, above generation (stock 10). A mesh block whose data is complete is drawn before
  more data is generated.
- A freshly meshed block is shown at once with one lod in clipbox. Before, it stayed hidden until the next update
  task set `visual_active` ("hidden" in the census went from ~70 to 0).

## Measurements
Medians of 3 interleaved runs, with the baseline DLL (census only) against the fix DLL. Ground mesh-block samples
in view not drawn, at 72 m:

| kind (of ~11 000 samples) | before | after |
|---|---|---|
| not drawn, total | 10309 | 7383 |
| drawn | 630 | 3082 |
| generating | 9934 | 6926 |
| mesh pending / meshing | 119 / 87 | 149 / 79 |
| hidden (meshed, not shown yet) | 86 | 0 |
| not in box (update task lagging) | 61 | 105 |
| meshed empty that were real holes | 0 | 0 |
| culled wrongly / mesher fault | 0 | 0 |

At 56 m (the report's setting), single A/B runs went from 6361 to 3853.

p99 without the census: 6.90 / 6.90 / 6.29 ms before, 6.74 / 6.63 / 65.1 ms after (the 65 ms run had another Godot process loading; an A run hit 57.8 ms the same way in an earlier set). The first-load cover lifts in 3.5 s instead of 4.25 s (median).
Evidence sheet: `game/out/holes_before_after.png` (local, not committed).

## Consequences
- Holes come down, but they do not reach zero: the box needs about five times the generation the workers deliver
  at 20 m/s over mountains. Reaching zero needs throughput: workers idle half the time, the heap refresh, and the
  deep mountain band (the stream-throughput session). A taste call is open: a streaming horizon that pulls the
  fog in to the nearest unloaded ground while streaming lags.
- Priority is now view-dependent. Turning around shows the side behind later than before. The near cube still
  loads within one refresh, because the distance is small.
- The census numbers come from any `holes=1` run, so later streaming work can be measured the same way.
