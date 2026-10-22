# 0014 – How the frame budget is measured; mass edits through the kernel

- **Status:** accepted
- **Date:** 2026-10-22 (asked for: "iterate on the engine and find improvements")

## Context
ADR 0009 sets the reference-machine budget: after load, p99 frame <= 8.3 ms,
no frame > 16.7 ms, RAM <= 4 GB, VRAM <= 6 GB. Every system is supposed to
prove it with a benchmark, and the autopilot JSON (`DebugCapture`) is that
benchmark. Checking it against the running game showed the numbers were
optimistic:

- Frame times came from `delta`, which Godot's physics jitter fix redistributes
  between frames (a 28 ms hitch read as 20 ms, smeared over its neighbours).
- The filmstrip's synchronous GPU readback (`get_image`) landed in the
  percentiles: 3-15 ms per capture, once a second.
- "After load" began 1 s into the run, when the simulation bubble had its
  chunks but godot_voxel was still meshing the visible terrain.
- `mem_mb` (Godot's static allocator) missed the native heaps: 216 MB
  reported, 1.05 GB working set.
- Godot's `TIME_PROCESS` monitor is the maximum of the last second, so it
  cannot attribute a single slow frame.
- A per-break `print()` in the mining autopilot cost 3-15 ms on the Windows
  console and showed up as the worst frames of every mining run.
- Two sessions benchmarking on the same machine at once produced 20-32 ms
  frames that neither game caused.

ADR 0012 also requires that mass edits (explosions, a black hole, "1000 TNT")
never choke a frame. No mass-edit path existed: every edit went voxel by
voxel through GDScript.

## Decision
**Measurement (DebugCapture).**
- Frame time = wall clock between frame starts (DebugCapture processes first).
- Load ends when the bubble holds every chunk *and* godot_voxel has had no
  generation, meshing, streaming or main-thread tasks for 30 frames in a row
  (cap 10 s, then `load_settled: false`). `duration` is measured after that.
- The frame after a filmstrip readback is excluded (`capture_stalls`).
- The JSON carries `max_frame_ms`, `frames_over_8_3`, `frames_over_16_7`,
  `rss_mb` (process working set), `vram_mb`, `other_godot_processes`, and
  `budget` / `budget_ok` (ADR 0009 thresholds; unknown RSS fails).
- Every frame over 8.3 ms is listed in `spikes` with its own phase clocks:
  `_process` span (first- and last-priority nodes), physics steps, render
  CPU/GPU, `other` (driver, uploads, engine internals), mesh-memory delta,
  new pipelines, pending voxel tasks and the bubble's cost.
- Autopilot logs go into the JSON (`mined_log`), never per-event stdout.
- A run with `other_godot_processes > 0` shared the machine and does not count.

**Mass edits.** `SimWorld.fill_sphere(center, radius, material, keep)` edits
the simulation window natively, skips chunks still loading and materials in
`keep`, and merges dirty boxes and wakes once per chunk. `SimBubble.blast()`
calls it with stations and bedrock kept (block entities never lose their
voxel). The changed chunks reach the terrain through the bubble's existing
budgeted push queue, and static ground around the hole loosens (ADR 0013).
Outside the window nothing is edited: the world there is frozen.
`SimWorld.ignite_box` replaces the fire test's GDScript scan.
Autopilot `blast=R blast_count=N blast_every=S` is the stress benchmark.

**Fixes found by the new numbers.** The first waterfall spray emitter
compiled its particle shaders on the main thread (15-19 ms when the first
waterfall came into range); `WaterfallSpray` now warms one up during load.

## Measured (dev box, 1280x720, vsync off, default distances, 20 s after load)
| Run | p50 | p99 | max | frames > 8.3 | > 16.7 |
|---|---|---|---|---|---|
| fly 6 m/s | 0.98 | 1.29 | 8.99 | 1 | 0 |
| fast fly 40 m/s | 1.27 | 2.16 | 8.23 | 0 | 0 |
| walk + mine + place | 2.03 | 2.36 | 12.82 | 1 | 0 |
| forest fire (fire_test=14) | 2.13 | 6.44 | 12.09 | 35 | 0 |
| blast 4 m every 1 s | 1.57 | 1.86 | 3.64 | 0 | 0 |
| blast 8 m every 2 s | 1.55 | 2.99 | 6.34 | 0 | 0 |
| 50 x 1 m craters every 2 s | 1.59 | 1.86 | 3.38 | 0 | 0 |

All within budget; RSS ~1.0-1.1 GB, VRAM 0.3-1.0 GB. `fill_sphere`: 0.05 ms
for a 4 m crater, 0.4 ms for 8 m. Before the spray warm-up, fast flight had
a 15-19 ms frame; before the print removal, mining breaks cost 3-15 ms each.
The visual-polish settings of the same day (SSAO, soft shadows, bloom) were
measured by their session on this harness: budget_ok in all runs, ~1.2 ms
steady-state cost.

## Consequences
- Old JSON percentiles are not comparable with new ones (they were optimistic).
- Benchmarks from two sessions must not overlap; check `other_godot_processes`.
- Fire is the heaviest scenario (p99 6.4 ms, 35 frames over 8.3 ms): the
  first place to look when the budget tightens.
- Mass edits outside the simulation window still have no path; when spells
  need one, it goes through VoxelTool's native shape edits under the same
  per-frame push budget, not GDScript loops.
- Revisit: blast radii beyond the window (32 m+ black holes) need the brick
  store (ADR 0012) and its per-frame byte budget.
