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

## Consequences
- The 200 ms stalls are gone; 100 m is still over budget (p99 15 ms, frames over 16.7 ms). In flight the
  generation queue never catches up: generation throughput and per-task overhead for ~1 M mostly
  all-air / all-rock blocks are next. The remaining spikes in `ta_dequeue` are ~35 ms of `apply_result` work
  per frame. Next targets: skip tasks for blocks the generator can classify as uniform from its column
  heights, cap the main-thread time spent applying results, and make the 200 ms priority re-sort of the
  task queue cheaper.
- `game/README.md` lists the two new patches; the godot_voxel DLL must be rebuilt with them.
- Removed far-horizon saves (`far_edits_<seed>`) are left on disk and ignored.
