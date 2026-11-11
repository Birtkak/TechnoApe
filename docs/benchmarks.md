# Benchmarks

The shared benchmark of `docs/plans/roadmap.md`, measured as ADR 0004 says. Newest first.
Run it: `bash tools/bench.sh`, then `python -I tools/summarize.py`.

## 2026-11-11: after the optimisation pass (ADR 0025)

Reference machine, release build, 1920x1080 off-screen, the default series (`bash tools/bench.sh`: the 4 km
planet, 3 reps at half length, and the filmstrips), 15 reps, none disturbed by a compiler. The pass's packages
A to J2 are in (ADR 0025), and the series gains `sim`: `stand`'s view at the spawn with 50 players walking,
digging and placing and water running (ADR 0025, S).

| series | planet m | run | reps | | fps_avg | frame_p50_ms | frame_p99_ms | frame_max_ms | cpu_ms_p99 | gpu_ms_p99 | ram_mb | vram_mb | chunks_meshed_per_s | missing_chunks_max | holes_max | draws_avg | quads_avg | budget | spikes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| small | 4245 | stand | 3 | median | 566.4 | 1.61 | 2.464 | 3.398 | 2.296 | 1.697 | 322 | 811 | 0 | 0 | 0 | 402 | 401205 | | |
| | | | | worst | 563.1 | 1.629 | 2.487 | 5.476 | 2.31 | 1.699 | 325 | 811 | 0 | 0 | 0 | 402 | 401205 | ok | 0 |
| small | 4245 | fly | 3 | median | 459.3 | 2.059 | 4.236 | 11.159 | 4.043 | 3.071 | 427 | 813 | 952.4 | 0 | 0 | 447 | 214739 | | |
| | | | | worst | 452.1 | 2.102 | 4.327 | 11.611 | 4.15 | 3.116 | 427 | 813 | 952.3 | 0 | 0 | 447 | 215567 | ok | 76 |
| small | 4245 | orbit | 3 | median | 571.4 | 1.67 | 3.182 | 5.447 | 3.035 | 2.278 | 418 | 813 | 930.1 | 201 | 0 | 80 | 92280 | | |
| | | | | worst | 555.4 | 1.718 | 3.195 | 9.955 | 3.046 | 2.293 | 425 | 813 | 928.6 | 214 | 0 | 80 | 92415 | ok | 1 |
| small | 4245 | edit | 3 | median | 253.7 | 3.993 | 4.949 | 6.333 | 4.702 | 3.278 | 390 | 811 | 956.4 | 0 | 0 | 270 | 302022 | | |
| | | | | worst | 252.8 | 4.014 | 4.951 | 10.099 | 4.714 | 3.3 | 393 | 811 | 955.8 | 0 | 0 | 270 | 302043 | ok | 1 |
| small | 4245 | sim | 3 | median | 536.1 | 1.753 | 3.04 | 4.861 | 2.801 | 1.795 | 343 | 811 | 9 | 0 | 0 | 404 | 407885 | | |
| | | | | worst | 531.9 | 1.763 | 3.119 | 6.823 | 2.887 | 1.802 | 350 | 813 | 8.7 | 0 | 0 | 404 | 408016 | ok | 0 |

| series | planet m | run | reps | | players | tick_ms_p50 | tick_ms_p99 | tick_ms_max | tick_cores | job_wait_ms_p99 | worker_cores | frame_cores |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| small | 4245 | sim | 3 | median | 51 | 3.213 | 4.322 | 4.899 | 0.065 | 0.015 | 0.002 | 0.926 |
| | | | | worst | 51 | 3.251 | 4.553 | 5.124 | 0.065 | 0.043 | 0.002 | 0.926 |

- **Budget:** every run holds ADR 0004 in every rep. The worst p99 is 4.95 ms (`edit`), the worst frame 11.6 ms
  (`fly`). The tick holds the budget ADR 0004 now gives it: p99 4.6 ms at worst against 25.
- **Against the series before the pass** (below), medians:

  | run | fps | frame p99 ms | frame max ms | CPU p99 ms | GPU p99 ms | VRAM MB |
  |---|---|---|---|---|---|---|
  | stand | 257 → 566 | 4.93 → 2.46 | 9.3 → 3.4 | 4.64 → 2.30 | 3.28 → 1.70 | 1064 → 811 |
  | fly | 185 → 459 | 7.45 → 4.24 | 11.4 → 11.2 | 7.23 → 4.04 | 6.02 → 3.07 | 1064 → 813 |
  | orbit | 293 → 571 | 5.15 → 3.18 | 10.8 → 5.4 | 4.93 → 3.04 | 3.68 → 2.28 | 1066 → 813 |
  | edit | 178 → 254 | 6.94 → 4.95 | 13.3 → 6.3 | 6.63 → 4.70 | 4.55 → 3.28 | 1832 → 811 |

  `fly` and `orbit` no longer fail: their worst frames went from 17.8 and 23.1 ms to 11.6 and 10.0 ms.
  The spikes over 8.3 ms went from 118 to 78. `fly` has 76 of them; `orbit`, `edit` and `stand` 1, 1 and 0.
- **`stand` ends at 566 fps**, not the 800 of the pass's first target, which the developer dropped
  (2026-11-11).
- **`fly`'s 76 spikes** come in one burst in each rep, about 1.4 s of 9.5-11.6 ms frames 500-580 m into the
  flight; their place follows the flight's speed, not the clock.
  - The frame thread spends 7-8 ms of each such frame in `get_current_texture`, with 0.8 ms of CPU, while the
    GPU's timed stages stay at their usual 1.6 ms. So it is a wait in the driver, not our work.
  - It came with ADR 0025's J2: J1's build has none.
  - It depends on how video memory happens to be laid out, not on J2's code: J2 with the quad buffer at 12, 20
    or 24 MB (`quad_cap_m=`) instead of 16 has none, and standing still at that spot shows none either.
  - Which allocation triggers it is open; see ADR 0025's J2 row.
- **`orbit`'s missing chunks** went up, at most 214 against 162 before: with the far levels loading on the same
  workers but more of them asked for in time, this was not looked into.
- **`sim`:** 51 players cost the frame p99 0.6 ms against `stand` (3.04 against 2.46). The tick takes 3.2 ms of
  its 50 at p50 (0.07 of a core), and the meshing workers are nearly idle. The frame thread holds 0.93 of a
  core in every run: nothing caps the frame rate, and the driver's wait for the screen keeps it busy.

## 2026-11-10: before the optimisation pass (ADR 0025), with the traced light

Reference machine, release build, 1920x1080 off-screen, the default series (`bash tools/bench.sh`: the 4 km
planet, 3 reps at half length, and the filmstrips), 12 reps, none disturbed by a compiler. The first series with
the ray-traced lighting of ADR 0022; the baseline the optimisation pass (ADR 0025) is measured against.

| series | planet m | run | reps | | fps_avg | frame_p50_ms | frame_p99_ms | frame_max_ms | cpu_ms_p99 | gpu_ms_p99 | ram_mb | vram_mb | chunks_meshed_per_s | missing_chunks_max | holes_max | draws_avg | quads_avg | budget | spikes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| small | 4245 | stand | 3 | median | 257 | 3.866 | 4.929 | 9.346 | 4.642 | 3.282 | 309 | 1064 | 0 | 0 | 0 | 402 | 588734 | | |
| | | | | worst | 255.4 | 3.9 | 5.357 | 15.962 | 4.94 | 3.302 | 340 | 1064 | 0 | 0 | 0 | 402 | 588734 | ok | 23 |
| small | 4245 | fly | 3 | median | 185.4 | 5.26 | 7.446 | 11.363 | 7.231 | 6.018 | 357 | 1064 | 951.8 | 0 | 0 | 450 | 535072 | | |
| | | | | worst | 183.5 | 5.295 | 7.603 | 17.75 | 7.353 | 6.13 | 371 | 1064 | 951.8 | 0 | 0 | 450 | 535804 | FAIL | 26 |
| small | 4245 | orbit | 3 | median | 293.4 | 3.455 | 5.148 | 10.799 | 4.931 | 3.684 | 357 | 1066 | 674.1 | 134 | 0 | 80 | 159722 | | |
| | | | | worst | 287.9 | 3.481 | 5.443 | 23.077 | 5.051 | 3.749 | 363 | 1066 | 662.5 | 162 | 0 | 80 | 159808 | FAIL | 25 |
| small | 4245 | edit | 3 | median | 177.7 | 5.603 | 6.94 | 13.276 | 6.634 | 4.552 | 333 | 1832 | 757.9 | 0 | 0 | 270 | 446613 | | |
| | | | | worst | 176.5 | 5.617 | 8.109 | 13.552 | 7.525 | 4.612 | 343 | 1832 | 755.7 | 0 | 0 | 270 | 446618 | ok | 44 |

- **Budget:** p99 holds everywhere (worst 8.1 ms in `edit`), but `fly` and `orbit` each have a rep with a frame
  over 16.7 ms (17.8 and 23.1 ms), so they fail ADR 0004. 118 spikes over 8.3 ms in 12 reps.
- **Against the series below:** the frame went from about 0.4 to 3.5-5.6 ms (frame rate a factor 10 down), VRAM
  from 495 to 1064 MB (1832 MB in `edit`), the mesh jobs from about 45 to 600-1000 µs.

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
