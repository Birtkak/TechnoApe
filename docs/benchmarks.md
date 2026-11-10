# Benchmarks

The shared benchmark of `docs/plans/roadmap.md`, measured as ADR 0004 says. Newest first.
Run it: `bash tools/bench.sh`, then `python -I tools/summarize.py`.

## 2026-11-10: half-length series after the speed levers (ADR 0012)

Reference machine, release build, 1920x1080 off-screen, vsync off. **Half-length runs** from here on
(`length=0.5`: `stand` and `edit` 15 s, `fly` 30 s, `orbit` the same trip in 32 s, 5 s warm-up), 3 reps, 26
reps in 13 min, none disturbed by a compiler. `big` is the 1 km planet (N = 3072), `huge` the 4 km planet
(N = 12288, R = 4245 m). Not comparable one to one with the full-length rows below: `orbit` flies its trip
twice as fast, and `fly` covers half the lap.

| series | planet m | run | reps | | fps_avg | frame_p50_ms | frame_p99_ms | frame_max_ms | cpu_ms_p99 | gpu_ms_p99 | ram_mb | vram_mb | chunks_meshed_per_s | missing_chunks_max | holes_max | draws_avg | quads_avg | budget | spikes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| big | 1061 | stand | 3 | median | 2913.4 | 0.342 | 0.608 | 1.022 | 0.281 | 0.256 | 158 | 495 | 0 | 0 | 0 | 447 | 540872 | | |
| | | | | worst | 2911.1 | 0.342 | 0.611 | 1.274 | 0.285 | 0.256 | 159 | 495 | 0 | 0 | 0 | 447 | 540872 | ok | 0 |
| big | 1061 | fly | 3 | median | 2839.2 | 0.321 | 0.588 | 1.014 | 0.4 | 0.29 | 162 | 495 | 944.2 | 0 | 0 | 451 | 560768 | | |
| | | | | worst | 2823.6 | 0.325 | 0.69 | 1.181 | 0.403 | 0.293 | 162 | 495 | 943.4 | 0 | 0 | 451 | 560861 | ok | 0 |
| big | 1061 | orbit | 3 | median | 4647.3 | 0.194 | 0.454 | 1.813 | 0.303 | 0.297 | 163 | 495 | 411.2 | 0 | 0 | 79 | 133169 | | |
| | | | | worst | 4635.8 | 0.195 | 0.459 | 2.557 | 0.304 | 0.297 | 163 | 495 | 411.2 | 0 | 0 | 79 | 133208 | ok | 0 |
| big | 1061 | edit | 3 | median | 2596.4 | 0.378 | 0.691 | 2.12 | 0.358 | 0.295 | 164 | 495 | 160 | 0 | 0 | 443 | 578883 | | |
| | | | | worst | 2593.9 | 0.378 | 0.694 | 2.444 | 0.389 | 0.296 | 164 | 495 | 160 | 0 | 0 | 443 | 578886 | ok | 0 |
| huge | 4245 | stand | 3 | median | 2519.2 | 0.396 | 0.665 | 1.367 | 0.339 | 0.306 | 159 | 495 | 0 | 0 | 0 | 492 | 677876 | | |
| | | | | worst | 2517 | 0.397 | 0.667 | 1.437 | 0.339 | 0.307 | 159 | 495 | 0 | 0 | 0 | 492 | 677876 | ok | 0 |
| huge | 4245 | fly | 3 | median | 2309.9 | 0.432 | 0.704 | 1.473 | 0.437 | 0.378 | 161 | 495 | 811.5 | 0 | 0 | 530 | 730160 | | |
| | | | | worst | 2302.2 | 0.434 | 0.706 | 1.918 | 0.451 | 0.379 | 162 | 495 | 809.3 | 0 | 0 | 530 | 730173 | ok | 0 |
| huge | 4245 | orbit | 3 | median | 3610.8 | 0.245 | 0.558 | 1.214 | 0.426 | 0.423 | 162 | 495 | 1889.6 | 32 | 0 | 193 | 299612 | | |
| | | | | worst | 3609.8 | 0.245 | 0.558 | 2.622 | 0.426 | 0.424 | 162 | 495 | 1888.8 | 32 | 0 | 193 | 299681 | ok | 0 |
| huge | 4245 | edit | 3 | median | 3084.2 | 0.309 | 0.589 | 2.447 | 0.294 | 0.21 | 164 | 495 | 181.1 | 0 | 0 | 251 | 399119 | | |
| | | | | worst | 3075.9 | 0.309 | 0.6 | 2.46 | 0.315 | 0.211 | 165 | 495 | 180.3 | 0 | 0 | 251 | 399122 | ok | 0 |

- **Budget:** all 24 reps hold ADR 0004 with no frame over 8.3 ms; worst p99 0.71 ms, worst frame 2.6 ms.
- **The 4 km planet** costs about 15% of the frame rate (`stand` 2,519 against 2,913 fps, 25% more quads) and
  holds the budget by a factor of 12. `orbit` there shows up to 32 missing chunks while it crosses 3.5 km
  between bodies at twice the full run's speed; `fly` has none.
- **Levers** (ablation in ADR 0012): the soak 2.8x faster; mimalloc 17% less meshing time; the frame numbers
  a few percent up, within the noise of a calm machine.

## 2026-11-10: engine base on the cube-sphere grid

Reference machine (RTX 3070, Vulkan), release build, 1920x1080 off-screen, vsync off, 10 s warm-up, 3 reps per
run, 24 reps, none disturbed by a compiler. `base` is the 531 m planet (N = 1536), `big` the 1 km planet of
the first playable (N = 3072, R = 1061 m); the moon is 177 m.

| series | planet m | run | reps | | fps_avg | frame_p50_ms | frame_p99_ms | frame_max_ms | cpu_ms_p99 | gpu_ms_p99 | ram_mb | vram_mb | chunks_meshed_per_s | missing_chunks_max | holes_max | draws_avg | quads_avg | budget | spikes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| base | 531 | stand | 3 | median | 2326.6 | 0.324 | 0.925 | 2.199 | 0.803 | 0.28 | 158 | 495 | 0 | 0 | 0 | 366 | 398716 | | |
| | | | | worst | 2132.2 | 0.44 | 0.941 | 2.257 | 0.82 | 0.28 | 158 | 495 | 0 | 0 | 0 | 366 | 398716 | ok | 0 |
| base | 531 | fly | 3 | median | 2389 | 0.346 | 0.914 | 2.011 | 0.804 | 0.285 | 161 | 495 | 1163.4 | 0 | 0 | 380 | 402965 | | |
| | | | | worst | 2153.1 | 0.443 | 0.924 | 3.778 | 0.805 | 0.286 | 162 | 495 | 1148.6 | 0 | 0 | 381 | 403158 | ok | 0 |
| base | 531 | orbit | 3 | median | 3530.3 | 0.239 | 0.752 | 3.976 | 0.573 | 0.254 | 162 | 495 | 182.2 | 0 | 0 | 67 | 97747 | | |
| | | | | worst | 3142.7 | 0.272 | 0.787 | 19.764 | 0.581 | 0.261 | 162 | 495 | 182.2 | 0 | 0 | 69 | 100147 | FAIL | 1 |
| base | 531 | edit | 3 | median | 1219.4 | 0.814 | 1.499 | 3.061 | 1.347 | 0.348 | 160 | 495 | 2032.2 | 0 | 0 | 401 | 465117 | | |
| | | | | worst | 1203.8 | 0.832 | 1.503 | 3.229 | 1.358 | 0.348 | 160 | 495 | 2022.7 | 0 | 0 | 401 | 465132 | ok | 0 |
| big | 1061 | stand | 3 | median | 2090.8 | 0.44 | 0.973 | 2.535 | 0.795 | 0.323 | 157 | 495 | 0 | 0 | 0 | 445 | 471932 | | |
| | | | | worst | 2087.5 | 0.44 | 0.976 | 2.556 | 0.871 | 0.328 | 158 | 495 | 0 | 0 | 0 | 445 | 471932 | ok | 0 |
| big | 1061 | fly | 3 | median | 2167.1 | 0.435 | 0.993 | 1.88 | 0.871 | 0.341 | 163 | 495 | 942.5 | 0 | 0 | 450 | 486474 | | |
| | | | | worst | 1853 | 0.499 | 0.999 | 9.036 | 0.878 | 0.342 | 163 | 495 | 918.7 | 0 | 0 | 450 | 486650 | ok | 1 |
| big | 1061 | orbit | 3 | median | 3145.4 | 0.27 | 0.786 | 1.885 | 0.591 | 0.262 | 162 | 495 | 182.9 | 0 | 0 | 81 | 119134 | | |
| | | | | worst | 3111 | 0.274 | 0.823 | 3.32 | 0.603 | 0.265 | 163 | 495 | 182.9 | 0 | 0 | 82 | 120136 | ok | 0 |
| big | 1061 | edit | 3 | median | 1175.6 | 0.857 | 1.506 | 2.444 | 1.365 | 0.353 | 159 | 495 | 2249.1 | 0 | 0 | 441 | 512814 | | |
| | | | | worst | 1175.3 | 0.858 | 1.524 | 2.486 | 1.383 | 0.355 | 159 | 495 | 2248.8 | 0 | 0 | 441 | 512820 | ok | 0 |

- **Budget:** 23 of 24 reps hold ADR 0004 with a wide margin (worst p99 1.5 ms). One `orbit` rep on the small
  planet had a single 19.8 ms frame; the engine's own work in that frame was 0.5 ms, so the time was outside
  it (presenting or the OS). Three more `orbit` reps with the present call timed did not repeat it (worst
  frame 2.1 ms). One `fly` rep on the big planet had a 9.0 ms frame of the same kind (0.3 ms of engine work).
- **Streaming:** no missing chunks and no holes in any run, on either planet, across the seams the `fly` lap
  crosses.
- **Against the spike** (flat grid, 500 m, greedy quads): about 5x the quads per frame (400,000 against
  84,000: one quad per block face, and no per-direction ranges) at the same or higher frame rate
  (`stand` 2,327 against 2,033 fps); `edit` is slower (1,219 against 1,587 fps) and still 5x inside budget.
- **Looked at:** filmstrips of `fly` and `orbit`, and full-size stills at a seam and at a cube corner
  (`run=stand at=0,1,1` and `at=1,1,1`): the sphere is round, the grid turns at the seams, no cracks.
- **LOD seams (2026-11-10):** skirts close them: every chunk draws its own border walls two cells under
  the surface, so a coarser neighbour's different surface shows as a step, never a hole (`docs/images/
  lod-skirts.png`). Quick runs after: `fly` 2,253 fps / p99 0.95 ms, `edit` 1,963 fps / p99 1.16 ms,
  `orbit` 3,497 fps; about 15% more quads at the worst view. The full series above predates skirts.
- **With player movement (2026-11-09, ADR 0007, full series, after skirts):** every run holds p99 (worst 1.2 ms,
  `edit` on the big planet) and streaming (no missing chunks, no holes); `stand` 2,786 / 2,485 fps, `edit`
  2,257 / 2,196 fps (base / big). Three of six `orbit` reps had one frame over 16.7 ms (16.5, 29.5 and
  31.3 ms) with under 1 ms of engine work and 0.15 ms presenting: the same outside-the-frame stall as above,
  now more often. `orbit` flies with `Move`, so the movement code does not step in it.
- **Chasing that stall (2026-11-10):** it did not come back in 8 full `orbit` reps on the same build (worst
  frame 2.3 ms, 4,600 fps against the series' 3,900, so the PC was busier that night), nor with the bench's
  `tasklist` sampler running alongside. Ruled out: the RAM/VRAM probe on the frame thread (worst 0.74 ms);
  plain CPU load (16 busy processes: worst frame 9.7 ms, never 30). The stalls are one and two Windows time
  slices long (about 15.6 ms each), outside the engine's work. Spikes now also report `prev_tail_ms` (the
  previous frame's work after present) and `between_ms` (time in winit's loop between frames), so every
  millisecond of a spike is accounted for; under forced load the lost time lands in `between_ms`. The next
  spike in a series names its place.
- **Not built yet:** textures from images, lighting beyond one sun, shadows, transparency.
