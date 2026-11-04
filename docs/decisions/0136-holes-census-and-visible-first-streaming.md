# ADR 0136: A census of see-through holes, and streaming what the camera sees first

Status: accepted (2026-11-04). Builds on the task heap and sky blocks of ADR 0108 and the ground shell of ADR 0105.
Measured together with ADR 0135 (stream-throughput), whose patches stack under these.

## Context
Bug report 2026-11-03_23-05-16 (seed 1935341982, voxel 1148 1877 5955, creative fly, 56 m): "the world still loads
slow when exploring around quickly, lots of moments where you can see through the world". The log also had
`Out of occlusion page layers (ADR 0107)`. Before fixing anything, we needed to know why ground in view was not drawn.

## Decision
**Census** (`voxel_patch_holes.py`, `holes=1`). `VoxelLodTerrain.debug_classify_mesh_blocks(xyz)` returns the
pipeline stage of each lod 0 mesh block. Every 0.25 s, `debug_capture.gd` takes the ground mesh blocks (FineGen
`surface_map`, 3x3 per mesh column) in the camera frustum inside the solid part of the box and sorts them by why
they are not drawn:
- hidden, not_in_box, generating, mesh_pending, meshing, upload_wait;
- meshed_empty: checked against the voxels, because a uniform block legitimately makes no mesh.

It also counts the **visible** holes: those the camera has a clear line to over the true ground heights (a hole
behind a hill shows nothing). The JSON `holes` gives:
- by kind, visible by kind, distance bands of 0 / 16 / 32+ m;
- remesh gaps (drawn, then not);
- why data-complete blocks wait (pending_state, pending_wait_ms);
- `requests`: lod 0 data requests, buried ones, and how many of each were generated.

The census costs up to ~40 ms a sample, so it is opt-in.

**Findings** (spectator fly, follow_ground 6 m, 20 m/s, 72 m, mountains):
- 95 % of the holes are "generating": a data block of the mesh block's padded area is still queued. The census
  found no culling or mesher faults, no remesh gaps, and no meshed_empty block that was really a hole.
- The occlusion page warning is not a hole cause: when the layers run out, merged faces draw unshaded.
- Demand is far above throughput: about 30 k lod 0 data requests/s against about 6 k generated/s. That part
  belongs to ADR 0135.
- Data-complete meshes waited in MESH_NEVER_UPDATED for the next update task. The update task's data pass
  (`ta_u_data`) took 110-145 ms on every box step. Each column the box gained asked FineGen.ground_range
  (~0.5 ms, it builds the column's context) one by one on the update thread, about 80 per 2 m step.

**Fix** (`voxel_patch_visible_first.py`, `view_priority_weight` in settings.gd, default 3):
- `PriorityDependency::evaluate` ranks ground in view and ahead first.
  - The distance is the nearer of two: to the camera, or to a point up to 32 m ahead (1 s of smoothed velocity).
  - It is multiplied by `1 + w (1 - cos)` of the angle between the view axis and the block's nearest edge. A mesh
    block's radius is ~56 voxels, so a big block under the camera counts as in view; measuring to the centre had
    doubled the holes within 16 m.
  - Only cosines and square roots are used, because the heap refresh evaluates every task.
  - The weight is 0 while a loading cover is up: a cover waits for the ground all around.
- Lod 0 data requests take their mesh block's centre as their priority position. The 216 data blocks one mesh
  block needs then finish together instead of in distance rings.
- With the viewer above the ground, blocks under the ground band (near-cube filler) get +1024 voxels of priority
  distance. Blocks in the band get their depth under their own column's ground.
- Meshing gets band2 11, above generation (10). A newly meshed block is shown at once (one lod, clipbox) instead of
  after the next update task ("hidden" ~80 -> 0).
- The update task fetches the missing ground of the columns a box step adds on up to 6 threads before the pass. The
  values and the shell are the same. The view phase went from p50 111 ms to 10 ms, and visible mesh_pending holes
  from 139 to 22.

**Tried and dropped:**
- Skipping the buried requests while the viewer is above ground. Only 0.2 % of generated blocks were buried ones
  (the +1024 already keeps them last), and the sim bubble reads the near cube (`world.read_column`, the first
  cover waits on `bubble.is_loaded(4)`).
- Writer preference for `SpatialLock3D`, for main-thread pastes starved by mesh-task readers. On `fluid_lag=river`
  neither version starved, and it raised the worst paste from 0.2-0.4 to 1.2-2.2 ms, because blocked readers eat
  the semaphore posts. If the 296-512 ms paste frames come back, use writer preference plus a waiter count (post
  once per waiter).
- Pulling the horizon in while streaming lags: the developer said no, the horizon stays fixed.

## Measurements
Medians of 3 interleaved runs, at 72 m. A machine shared with 0-4 other Godot runs inflates the p99 of runs with
the census. The three DLLs:
- **baseline**: census patch only;
- **fix**: this ADR;
- **combined**: ADR 0135's `voxel_patch_mesh_speed.py` and `voxel_patch_throughput.py`, then these two, in
  README order.

| ground mesh-block samples in view (of ~11 000) | baseline | fix | combined |
|---|---|---|---|
| not drawn, total | 9463 | 4613 | 3779 |
| drawn | 1348 | 5367 | 6316 |
| generating | 8971 | 4318 | 3567 |
| mesh pending / meshing | 177 / 171 | 52 / 73 | 60 / 59 |
| hidden (meshed, not shown yet) | 94 | 0 | 0 |
| not in box (update task lagging) | 58 | 131 | 113 |
| remesh gaps, meshed-empty holes, culling or mesher faults | 0 | 0 | 0 |
| **visible** holes (clear line of sight) | 4130 | 1813 | 1477 |
| visible within 0-16 / 16-32 / 32+ m | 85 / 629 / 3458 | 77 / 229 / 1490 | 86 / 194 / 1197 |

Without the census (same fly, medians of 3, 1-3 other Godot runs beside them):

| | baseline | fix | combined |
|---|---|---|---|
| p99 frame | 6.39 ms | 6.39 ms | 6.76 ms |
| worst frame per run | 23 / 30 / 147 ms | 19 / 15 / 22 ms | 16 / 145 / 16 ms |
| first loading cover lifts after | 4.66 s | 2.75 s | 2.64 s |
| mem / VRAM (more is drawn) | 0.71 / 0.61 GB | 0.91 / 0.75 GB | 1.01 / 0.84 GB |

The ~145 ms single frames happen with and without the fix (physics, bubble column loads, the render thread on a
shared machine); they are not this ADR's.

Evidence sheet: `game/out/holes_before_after.png` (local, not committed). `shell_test` at the default render distance
(run alone) gives 0 unloaded, 0 unmeshed and 0 air-where-solid near the player with both DLLs. It still reports
ok=false on far cavern walls (`far_undrawn_8m` 137-428 on the baseline, 186-484 with the fix), which predate this ADR.

## Consequences
- Not zero yet. What is left is about 96 % generation at 20 m/s over mountains, and holes past 32 m are most of it.
  Reaching zero needs generation throughput (ADR 0135 and on): the queue still wants about five times what the
  workers deliver in mountains.
- Priority now depends on the view. Turning around shows the side that was behind later than before. The ground
  within reach and under the camera still comes first.
- Any later streaming work can be measured the same way with `holes=1`.
