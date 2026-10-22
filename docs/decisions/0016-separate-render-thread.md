# 0016 – Separate render thread (thread_model = 2)

- **Status:** accepted
- **Date:** 2026-10-22 (the user: "adopt thread_model=2 as the default with an
  ADR and a way back to safe mode, after a soak test for crashes")

## Context
With the budget measured honestly (ADR 0014), two scenarios failed ADR 0009:
liquid spills (p99 12 ms, ~250 frames over 8.3 ms per 16 s) and, once the
visual polish turned on soft sun shadows, the forest fire (p99 10 ms, frames
up to 20 ms). Both had the same signature: 12-24 ms of main-thread
`_process` time with 3-22 MB of new mesh data, the bubble idle and the GPU at
~2 ms. Every flowing chunk is pasted and re-meshed; godot_voxel then builds
the mesh resources in `VoxelLodTerrain._process` on the main thread.

## Options considered
1. **Push each chunk less often** (`sim_push_interval_ms` 100): still p99
   10.5 ms with 104 frames over 8.3 ms, and ADR 0010 found that slower pushes
   make water and fire visibly step. Not enough on its own.
2. **Lower godot_voxel's main-thread budget**
   (`voxel/threads/main/time_budget_ms` 8 → 2): no change (p99 11.2-11.8 ms);
   the cost is not in that task queue.
3. **Cheaper shadows / fewer effects**: hard or no shadows bring the fire
   back under budget, but give up the look the polish was for, and the spill
   still fails.
4. **Separate render thread** (`rendering/driver/threads/thread_model = 2`):
   godot_voxel then builds graphics resources on its worker threads
   (`VoxelEngine.get_threaded_graphics_resource_building_enabled`). Spill p99
   6.9 ms (3-5 frames over), fire with soft shadows p99 6.1-6.4 ms. Godot
   labels the mode experimental and may crash.

## Decision
Option 4, after a soak test. `game/project.godot` sets
`driver/threads/thread_model=2`. The way back needs no file edit: launch with
`--render-thread safe` (verified: spill p99 11.7 ms and 217 frames over
8.3 ms, i.e. the old behaviour).

Measured on the dev box (1280x720, vsync off, other_godot_processes 0):

| Run | safe p99 / max | separate p99 / max |
|---|---|---|
| water spill 16 s (`blast=1.5 blast_material=water`) | 11.7-12.4 / 15.0-16.7 | 6.8-7.0 / 8.6-9.6 |
| water spill 60 s | 14.5 / 138.9 (15 frames > 16.7) | 10.0 / 14.2 (0 > 16.7) |
| forest fire 20 s, soft shadows (`fire_test=14`) | 10.1 / 20.1 * | 6.1-6.4 / 13.4-14.5 |
| fast flight 40 m/s, 20 s | 2.2 / 8.2 ** | 2.8 / 10.8 |
| walk + mine + place, 20 s | 2.4 / 12.8 ** | 3.0 / 4.6 |

\* measured by the visual-polish session on the same build. \*\* ADR 0014
matrix, before the visual polish (soft shadows etc.); the other rows are
the same build in both modes.

Steady-state p50 rises ~0.5 ms (1.0 → 1.6 ms flying): handing work to the
render thread costs a little every frame and buys the spikes away.

**Soak test** (separate thread, 3 min per scenario, 30 min in all, game at
2044002 + this setting; the stain session ran its fire and spill tests on
its own build with the same setting): no crash, exit code 0 in every run.

| Scenario (180 s) | frames | p99 | max | > 16.7 ms |
|---|---|---|---|---|
| fly 6 m/s (rerun; first run overlapped a screenshot) | 88k | 2.84 | 8.12 | 0 |
| fast fly 40 m/s | 78k | 3.39 | 18.66 | 1 |
| walk + mine + place | 69k | 2.80 | 5.83 | 0 |
| forest fire | 63k | 7.46 | 30.47 | 3 |
| blast 8 m every 2 s | 78k | 4.50 | 7.17 | 0 |
| 50 x 1 m craters every 2 s | 81k | 2.31 | 4.87 | 0 |
| water spill, a 1.5 m ball every 2 s | 38k | 17.82 | 33.71 | 484 |
| stations + furnace screen | 70k | 2.86 | 11.73 | 0 |
| cave with torch | 70k | 2.66 | 3.62 | 0 |
| third-person walk | 84k | 2.55 | 8.47 | 0 |

The fire's 3 frames came as one cluster of 30 ms frames with no process,
physics, upload or bubble time (`other_ms` only): an outside stall, not
reproduced. The 3-minute spill is real and is not a render-thread problem
(safe mode is far worse, see above): 90 balls of water keep ~54 chunks
flowing, ~890 chunk pushes per second, and godot_voxel's main-thread share
of re-meshing reaches 28-31 ms per frame. The push budget (ADR 0010) limits
paste time, not the re-meshing that follows.

## Consequences
- Godot prints the experimental warning at start-up, and on every exit an
  `ERROR: This function (finalize) can only be called from the render thread`
  plus "2 ObjectDB instances were leaked". Both also happen in an empty
  project without any extension: they are Godot's, not ours. Ignore them in
  logs; any other render-thread error is ours.
- Code must not drive the shared `RenderingDevice` from the main thread.
  Compute work (the ADR 0012 near-field port from P7, whose scripts call
  `RenderingDevice` directly) goes through
  `RenderingServer.call_on_render_thread()` or a local device from
  `RenderingServer.create_local_rendering_device()`. Texture updates through
  RenderingServer (`Texture2DArray.update_layer`, the stain map) are queued
  and safe.
- Synchronous readbacks (`Viewport.get_texture().get_image()`, filmstrips)
  still work and still stall; DebugCapture excludes them as before.
- Open: sustained liquid flow needs a re-mesh budget (a cap on chunks or
  bytes pushed per frame that adapts to the measured main-thread mesh cost),
  as ADR 0012 already requires for every edit path. The 16-20 s spill passes;
  minutes of continuous flow do not.
- DebugCapture now probes for other Godot processes every 10 s on a worker
  thread, so a run that shared the machine is flagged even if the other run
  ended first.
- If a crash shows up only with the separate thread, reproduce it with and
  without `--render-thread safe` before anything else, and record it here.
  Revisit when Godot declares the mode stable, or if it crashes in play.
