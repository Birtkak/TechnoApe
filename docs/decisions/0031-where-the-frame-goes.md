# 0031 – Where the frame goes: GPU attribution and the shadow knobs

- **Status:** accepted (measurement); the quality trade-offs below wait for the developer
- **Date:** 2026-10-23 (asked for: "do some more optimizations")

## Context
After the micro-simulation (ADR 0029) and the deeper Flint Age (ADR 0030)
landed, and the other session's housekeeping fixed the fire's pipeline
compile (`material_warmup.gd`), the question was what to optimize next.
The JSON told frame times and spikes, not where a normal frame's time goes.

## What was added
- **`frame_parts` in the autopilot JSON** (DebugCapture): p50 / p99 of the
  main thread's `_process` span and physics every frame, and of render CPU and
  GPU time every 30th frame. Reading render times waits for the render thread
  (ADR 0016): read every frame it made p50 61 ms, so the frame after each read
  is left out like a filmstrip capture.
- **Shadow knobs as run args** (Atmosphere): `shadow_splits=2|4`,
  `shadow_blend=0|1`, `shadow_filter=0..5`, `shadow_distance_m=`.
- **The progression book checks the inventory once per frame at most.** Every
  slot change used to run the full check; a shift-click craft changes slots
  hundreds of times in one frame (`quest_check_ms_max` in the JSON).

## Measured (dev box, vsync off, default settings: near field and micro sim off)
Budget matrix, 1280x720, 20 s after load, `other_godot_processes` 0 in every row:

| scenario | p50 | p99 | max | > 8.3 | > 16.7 |
|---|---|---|---|---|---|
| fly 6 m/s | 2.13 | 2.81 | 5.44 | 0 | 0 |
| fast flight 40 m/s | 2.51 | 4.04 | 8.00 | 0 | 0 |
| walk + mine | 2.94 | 3.91 | 7.01 | 0 | 0 |
| forest fire (`fire_test=14`) | 3.31 | 4.05 | 5.05 | 0 | 0 |
| water spill | 3.65 | 4.55 | 7.54 | 0 | 0 |
| 8 m blast / 2 s | 3.50 | 4.58 | 7.14 | 0 | 0 |
| 50 x 1 m craters | 3.53 | 4.52 | 6.97 | 0 | 0 |

RSS 1.38-1.42 GB, VRAM 0.55-1.13 GB. A new world loads in 3.62 s and a saved
one (`persist=1`) in 3.65 s, no frame over 7 ms while loading.

**The game is GPU-bound.** Standing 8 m up over grassland: frame p50 3.54 ms,
GPU 3.15, render CPU 0.87 (its own thread), scripts 0.16, physics ~0. One
feature off at a time:

| variant | GPU ms | triangles | draws |
|---|---|---|---|
| default | 3.15 | 10.2 M | 1,275 |
| shadows off | 1.73 | 2.2 M | 521 |
| shadows "hard" (2 splits, no penumbra) | 2.60 | 7.5 M | 1,100 |
| 2 splits, penumbra kept | 2.75 | 7.6 M | 1,110 |
| shadow distance 96 -> 64 m | 2.69 | 6.7 M | 883 |
| shadow filter quality 2 -> 1 | 2.96 | 10.2 M | 1,273 |
| no cascade blending | 3.05 | 10.2 M | 1,279 |
| render distance 64 -> 48 m | 2.64 | 6.9 M | 819 |
| SSAO off / glow off / LOD fade off | 3.05-3.15 | 10.2 M | ~1,275 |
| view distance 2 km -> 1 km | 3.16 | 10.1 M | 1,259 |

By window size (same view): GPU 2.43 ms at 640x360, 3.16 at 1280x720, 4.17
at 1920x1080 (2560x1440 is clamped to the screen). Shadows cost a fixed
1.2-1.7 ms at every size (8 M triangles into the shadow maps); the rest
scales with pixels. At 1080p p99 is 5.3 ms against the 8.3 ms budget.

## Decision
- Nothing that changes the picture is switched on its own: every remaining
  GPU saving is a quality trade-off, a taste call for the developer.
- The cheapest trade-offs, in the order they buy time: shadow distance 64 m
  (-0.44 ms), 2 cascades (-0.38), render distance 48 m (-0.51, but LOD1 cubes
  start closer), filter quality 1 (-0.17), no cascade blending (-0.08).
  A graphics preset could carry them for slower PCs.
- Not worth chasing: the 2 km horizon (free), SSAO, glow and the LOD fade
  (under 0.1 ms each), and script time (0.16 ms).

## Consequences
- The open costs are now in the opt-in features (ADR 0029): micro chunk
  expansion is 12 MB of cell writes per 8 m chunk (1.5-2.5 ms, bandwidth
  bound, already on the pool; the fix is ADR 0012's compact uniform bricks)
  and +385 MB RSS.
- `MicroMesher` already merges greedy quads, so the micro models offer no
  free triangle cut.
- Future runs should read `frame_parts` before guessing: a GPU-bound frame
  does not get faster from script work.
