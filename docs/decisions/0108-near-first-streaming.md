# ADR 0108: The start cover lifts on near ground; near blocks really load first

Status: accepted (2026-10-29). Follows ADR 0105 (render distance only) and ADR 0066 (teleport cover).

## Context
A new world at 100 m kept the loading cover up ~20 s (72 m: ~9 s) until every generation task was done. The
developer asked for the cover to lift as soon as the ground around the player is in, like after a teleport, and for
100 m to be fixed (2026-10-29). Lifting early made the streaming behind it the thing to measure: a spectator flight
started the moment the cover lifted (`fly_after_cover=1 load_cap_s=3.5`) found the near ground missing for 13 s,
frames up to 16 ms, and a 570 k task queue. Four causes, all in godot_voxel:

1. **Priority was flat past 39.5 m.** `PriorityDependency::evaluate` started its closest-viewer search at
   `99999` squared units; at one unit per 12.5 cm voxel that is 39.5 m, so every block farther than that had the same
   priority and a 100 m box generated in arbitrary order (a per-task wait log showed every task at d = 316 units).
2. **Newest tasks ran first.** `ThreadedTaskRunner` sorted its queue every 200 ms and took from the back, so between
   sorts the tasks staged last (the far edge of the box the update task just requested) ran first. Sorting 570 k
   items under the queue lock every 200 ms also stalled the workers.
3. **Sky blocks were 70 % of all tasks.** The ground shell keeps 64 m of air over the highest ground, water or tree of
   each column (builds up to 64 m stand, ADR 0105): 622 k of 888 k generation tasks in a 20 s flight were blocks
   whose only work was the generator's sky check.
4. **Lowering the worker count joined every worker.** The cover raises the voxel threads to cores - 2 and sets them
   back when it lifts; `set_thread_count` stopped and restarted all threads, waiting out whatever each ran (50-190
   ms frames with 180 k tasks queued; the old cover lifted on an empty queue, so this never showed).

## Decision
- `cover_until_loaded("Generating world", true)`: the start cover is a near cover (World.near_ready, ADR 0066); the
  rest of the box streams in behind it.
- `voxel_patch_task_heap.py`: the closest-viewer search starts at the float maximum; the worker queue is a max-heap
  on cached priority (a staged task gets its priority as it enters, the 200 ms refresh re-evaluates all and rebuilds
  the heap in O(n)); lowering the thread count parks the extra workers (they pick nothing, sleep 2 ms, count as
  waiting for `wait_for_all_tasks`), only raising it past the existing threads restarts them.
- `voxel_patch_sky.py`: a lod 0 block at or over its column's top bound (the shell's memo of `FineGen.ground_range`,
  the bound the generator's sky check uses) that the save never held (`may_have_block`, the key cache) is set as air
  in the update task and never queued.
- `voxel_patch_upload_order.py`: meshes waiting on the per-frame upload budget are queued nearest to the camera
  first, only as many as the budget takes (they were all requeued every frame in hash order).
- The monolith's box fills stop after 2 ms a frame: its ground now loads after the cover lifted (one 12 ms frame).
- The benchmark's ground probe samples 81 patches that follow the terrain (3 m under each patch's own ground to 1 m
  over), and counts a mesh block drawn while an edit's remesh waits (rate-limited, ADR 0020): the flat 36 m box cut
  into hillsides below the shell and reported ground that was drawn as missing.

## Measured (spectator fly, flight starts when the cover lifts, seed 272905368)
| | before | after |
|---|---|---|
| 100 m start cover | ~20 s | 3.0 s |
| 72 m start cover | ~9 s | 1.8 s |
| 100 m near ground missing in flight | 13.25 s | 1.25 s (one patch 25 m out) |
| 72 m near ground missing in flight | 4 s (old probe) | 0 |
| 100 m p99 / worst frame while streaming | 7.1 / 16.2 ms | 7.3 / 11.5 ms |
| 72 m p99 / worst frame while streaming | 5.9 / 11.5 ms | 6.0 / 10.6 ms |
| 100 m generation tasks, queue peak | 888 k, 570 k | 228 k, 183 k |
| 100 m worst update task (ta_u_data) | 207 ms | 2-3 ms |
| save path (persist=1), 100 m, two loads | 25 / 189 ms worst frame | 9.9 / 10.3 ms |

## Consequences
- The player is in the world in 2-3 s; far terrain keeps appearing for ~15 s at 100 m (5 s at 72 m) while the
  workers drain the box, at the normal worker count.
- `fly_after_cover=1` (the autopilot holds still until the start cover lifts) with `load_cap_s` just over the lift
  time measures play right after loading; the default 10 s cap still measures flight from the start.
- Open: generation throughput (~175 k ground blocks at ~330 us at 100 m) still decides how long the far box takes.
