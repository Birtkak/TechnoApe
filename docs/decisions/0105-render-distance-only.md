# ADR 0105: Render distance only: no far horizon, and the full-detail box gets cheaper instead

Status: accepted (2026-10-28). Supersedes ADR 0074 and ADR 0075 (the far layer). Keeps ADR 0073 (one
cube-shaped box of full-detail voxels, no LOD).

## Context
Every way of drawing land past the box looked wrong or glitched: the voxel LODs of ADR 0020 / 0072, the
height-field skin of ADR 0074, and the far voxels of ADR 0075. The developer's call (2026-10-28): drop the far
layer, keep one setting, the render distance, and push it out by making the box cheaper. A flatter box (full
reach sideways, about +-24 m up and down) was offered and rejected: tall mountains would show as slices, and
gas the simulation lifts would hang in the air above the player where the box ends.

What stops the box past 40 m was measured at 100 m (seed 272905368, autopilot flight, vsync off). Rendering is
cheap: about 1,000 draw calls and a GPU p99 of 2.7 ms. The cost is in streaming. The cube holds ~1 M data blocks,
the generation queue peaks at ~950 k tasks, and the main thread stalled 200-240 ms several times a second.
`voxel_patch_profile.py` (main-thread timers, `ta_*` keys in `VoxelLodTerrain.get_statistics()`, kept in the
build) traced each stall to one finished generation task. On the main thread, `apply_data_block_response`
waited ~210 ms for `loading_blocks_mutex` before erasing the block from the pending map. The update task held
that lock in `unreference_data_block_from_loading_lists`: for every pending block in the slab the box left
behind, it scanned the whole list of blocks requested in the same update. In flight at 100 m both lists hold
~10 k blocks, so that was 10^8 compares under the lock. At 40 m the slabs are 6x smaller, which makes the
scan ~40x cheaper, so it never showed.

## Decision
1. **The far layer is removed.** `scripts/visuals/far_layer.gd`, `far_colors.gd`, `tools/far_probe.gd`,
   the `far_ground` / `far_tree` / `far_water` shaders, the native `FarMesh` (`far_mesh.cpp`), its test,
   the `far_*` settings and args, the Settings > Video "Far horizon" row and the far hooks in the sky and
   terrain shaders all go. Video now has one distance row, "Render distance" (`view_distance_m`, the menu's
   24-100 m choices of the ADR 0073 amendment). The generator keeps its coarse LODs (the world map uses them).
2. **The box stays a cube,** and distance is a performance job on godot_voxel's streaming. The first fix is
   `voxel_patch_unload.py`: the dropped positions go into a set, and the request list is filtered once per
   LOD, which makes the cost linear.

## Measured (100 m box, 20 s autopilot flight, same seed)
| | before | `voxel_patch_unload.py` | 40 m box |
|---|---|---|---|
| p99 / worst frame | 13.2 / 236 ms | 15.3 / 45 ms | 8.2 / 13.7 ms |
| frames > 16.7 ms | 46 | 59 | 0 |
| main-thread wait on the loading lock (worst) | 215 ms | 14 ms | - |
| generation queue peak | 947 k | 995 k | 3.9 k |
| share of time with near detail missing | 72 % | 75 % | 0 % |
| RSS | 4.4 GB | 3.9 GB | 2.6 GB |

## Generation (same day)
The run JSON's `generation` key (`FineGen.gen_stats()` by result, godot_voxel's worker time per task) showed
where the workers go at 100 m: 61 % of the blocks were sky at 145 us each, 39 % full per-voxel blocks at 734 us
(74 % of the time), none uniform. Two changes, neither altering a voxel (`tools/gen_hash.gd`: the same hash over
4,000 blocks before and after; `test_fine_gen_lod`: the shortcut's blocks equal the full pass):
- **Deep rock at full detail**: a buried LOD 0 block whose cave field is negative everywhere it is read, with no
  geode, no vein lens and one rock kind, comes back as one voxel (as LOD 1+ already did).
- **Per-footprint context cache** (`FineGen::column_ctx`): the macro lattice, nearby ruins and the highest tree
  or stamp voxel at any height are built once per vertical stack of blocks (~5 KB, up to 64k footprints, FIFO;
  threads queued on the same stack wait for one build), and a block above all of it returns air at once.

| 100 m, 20 s flight | blocks generated | sky | uniform | full |
|---|---|---|---|---|
| before | 470 k | 288 k @ 145 us | 0 | 182 k @ 734 us |
| after | 665 k (+41 %) | 404 k @ 97 us (cache builds included) | 73 k @ 150 us | 188 k @ 702 us |

Full blocks are now ~70 % of the worker time: the per-voxel pass of surface blocks and of rock with a cave in it
is the next target. The 40 m box: p99 3.5 ms, no backlog.

## Generation and streaming, second pass (same day)
Stage timers in the full pass (removed again) showed a full block at ~720 us: 280 before the voxel loop (151 of it
the tree and stamp search, repeated for every block of a stack), 209 of per-column setup, 189 in the voxel loop.
- Trees are built once and shared: mature ones by trunk position, young ones by their cell (`SharedCache`,
  build once, waiting threads share the build, FIFO). The footprint context keeps its trees (pointers) and stamps,
  so a block only cuts them to its height band. The context cache is capped at 16k footprints (RAM).
- Per column: plants, tufts, sticks, leaf litter and seagrass are skipped where the block lies wholly under the
  column's top voxel; the vein and stringer lookups run once per 0.5 m column instead of per 12.5 cm column.
- godot_voxel (`voxel_patch_apply_budget.py`): finished tasks are applied for at most 4 ms a frame, the rest wait in
  order (a frame had applied 17,700 at once: 68 ms). `voxel_patch_unload.py` takes `loading_blocks_mutex` in
  chunks of 256 and yields to the main thread while it applies results: Windows mutexes are not fair, and the
  update task's 10k-block slabs starved it 7-22 ms per wait.
- Tried and reverted: slicing `VoxelData::view_area` / `unview_area` the same way (the 32 ms `try_set_block`
  wait stayed, and the update task got slower: p99 16-20 ms).

`gen_hash` unchanged throughout. 100 m, 20 s flight, three runs: p99 7.8 / 9.7 / 9.6 ms (was 13-17), frames over
16.7 ms 6 / 19 / 12 (was ~60), worst 35-38 ms; ~870 k blocks generated (was 470 k); RSS 4.6 GB. The generation
queue still does not catch up in flight (~0.9 M pending, 70-75 % of the time near detail is missing).

## Third pass (same day): what the cube costs when it is full
- Generator: each column's ground height is kept with its footprint (5 noises a column, once per stack), the
  stringer sheet's noise reuses its lattice corners up a column, and the cave field reads each lattice column's
  cave mouths and gallery offset once instead of at every height. A full block 550 -> 375 us; ~1.25 M blocks a
  20 s flight (was 870 k). `gen_hash` unchanged.
- godot_voxel: `unview_area` frees removed blocks after its locks (`voxel_patch_unload.py`). Tried and reverted:
  deferring a finished block when the map is locked (retries piled up outside the frame budget: p99 26-33 ms),
  and a writer-preferring RWLock (no change; the 70 ms `try_set_block` waits come from a holder not yet found).
- With faster generation more of the box actually loads, and the box's real cost shows. It grows with its
  volume: every cave and ore below the player is meshed, drawn and kept at full size in memory.

| box, 20 s autopilot | triangles | draw calls | VRAM | RSS | p99 | near detail missing |
|---|---|---|---|---|---|---|
| 40 m flight | 3.4 M | 918 | 0.55 GB | 2.3 GB | 3.6 ms | 0 % |
| 72 m flight (fully loaded) | 33 M | 10.7 k | 4.2 GB | 4.3 GB | 50-53 ms | 18 % |
| 100 m walk | 14.8 M | 3.5 k | 1.6 GB | 7.6 GB | 16.6 ms | 0 % |
| 100 m flight (75 % unloaded) | 14 M | 4.6 k | 1.8 GB | 6.0 GB | 15-20 ms | 75 % |

The 72 m result is the same with the generator before this pass (p99 50 ms, 33 M triangles): it is the box, not
the speed-up. Beyond ~56 m the cube needs the underground it cannot see kept out of the draw and out of memory.

## The box follows the ground (developer's choice, same day)
A flattened box was rejected (mountains would show as slices, lifted gas would hang under its lid); the box keeps its
full reach sideways and follows the ground vertically instead (`voxel_patch_shell.py`,
`VoxelLodTerrain.set_ground_shell`, Settings `ground_shell`, `shell_below_m` 24, `shell_above_m` 64, `shell_near_m`
32; `shell=0` turns it off). A drawing viewer keeps a data column from the lowest ground of its 3 x 3 columns minus
`below` to the highest ground, water or tree plus `above` (`FineGen.ground_range`, the same top bound as the
generator's sky check), plus every block within `near` of it in any direction; the simulation bubble's data-only
viewer keeps its whole box. Mountains follow, builds up to 64 m stand, caves load within 32 m of the player.

Measured in spectator flight (the developer's test from now on: no walk runs; 20 s, seed 272905368):

| box | band | p99 | frames > 16.7 | triangles | draw calls | GPU p99 | render CPU p99 | RSS | VRAM |
|---|---|---|---|---|---|---|---|---|---|
| 40 m | off | 11.7 ms | 2 | 11.2 M | 2.7 k | 4.6 ms | 5.4 ms | 2.5 GB | 1.2 GB |
| 40 m | on | 9.0 ms | 0 | 10.0 M | 2.5 k | 4.6 ms | 4.9 ms | 2.5 GB | 1.1 GB |
| 56 m | on | 21.8 ms | 196 | 15.2 M | 4.5 k | 7.5 ms | 9.4 ms | 2.8 GB | 1.6 GB |
| 72 m | on | 33.2 ms | 725 | 23.0 M | 6.8 k | 11.1 ms | 17.1 ms | 3.2 GB | 2.4 GB |
| 100 m | on | 54.4 ms | 514 | 38.6 M | 11.5 k | 12.7 ms | 23.6 ms | 5.0 GB | 4.3 GB |
| 100 m | off | 18.9 ms (81 % of near detail missing) | 102 | 9.3 M | 3.3 k | 3.6 ms | 5.0 ms | 5.5 GB | 1.5 GB |

With the band the box fills, and what is left is drawing it: render CPU grows with the draw calls (~2 ms per
1,000) and the GPU with the triangles. Streaming, generation and memory no longer stop a bigger box; the number
of mesh blocks and faces does. Next: fewer draw calls (bigger mesh blocks than godot_voxel's 32-voxel maximum)
and fewer triangles (merging coplanar faces of one material; occlusion of what hills and trees hide).

## Consequences
- The 200 ms stalls are gone; 100 m is still over budget (p99 15 ms, frames over 16.7 ms). In flight the
  generation queue never catches up: generation throughput and per-task overhead for ~1 M mostly
  all-air / all-rock blocks are next. The remaining spikes in `ta_dequeue` are ~35 ms of `apply_result` work
  per frame. Next targets: skip tasks for blocks the generator can classify as uniform from its column
  heights, cap the main-thread time spent applying results, and make the 200 ms priority re-sort of the
  task queue cheaper.
- `game/README.md` lists the two new patches; the godot_voxel DLL must be rebuilt with them.
- Removed far-horizon saves (`far_edits_<seed>`) are left on disk and ignored.
