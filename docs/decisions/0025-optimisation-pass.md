# 0025 – The optimisation pass before world generation

- **Status:** accepted
- **Date:** 2026-11-10

## Context
The ray-traced lighting (ADR 0022) took the frame from about 0.4 ms to 3.9-5.4 ms and mesh jobs from 42 µs
to about 950 µs; `edit` had 5 % of the 8.3 ms p99 budget left (ADR 0004). `docs/plans/optimization-handoff.md`
lists nine work packages, A to I, in order; each is measured before and after on the same quick runs
(`run=stand|fly|edit|orbit quick=1`, 4 km planet, 1080p, off-screen), kept only if its number moved, and
committed on its own. The soak's final hash for seed 1 must not change in any package.

The baseline series before the pass is in `docs/benchmarks.md` (2026-11-10, "before the optimisation pass").

## Options considered
One row per package, in the order done.

| Package | Number it had to move | Before | After | Verdict |
|---|---|---|---|---|
| A. Account for every millisecond | GPU and CPU stages sum to within 10 % of the frame in all four runs | `gpu_ms` covered 2.9 of 3.9 ms (`stand`); `edit`'s 12 ms frames 9 ms unnamed | GPU stages + idle gap = frame p50 within 1 % in all four (stand 3.10 + 0.51 vs 3.64; fly 4.03 + 0.55 vs 4.56; edit 4.80 + 0.56 vs 5.36; orbit 2.76 + 0.55 vs 3.30); CPU stretches back to back | keep |
| A2. Submit everything before the screen pass ahead of `get_current_texture` (two submits a frame) | the GPU's idle gap between frames | gap p50 0.51 / 0.55 / 0.56 / 0.55 ms (stand / fly / edit / orbit) | gap 0.45 / 0.46 / 0.47 / 0.46, but the screen stage grew 0.15 -> 0.36 ms in `stand`: the idle moved between the two command buffers; frames within run-to-run noise | revert: the gap is not the CPU recording late; most likely the present's own copy of the swapchain image on Windows, outside our passes |
| B. Identity instance transforms, rays turned into the body's frame in `ray()` / `seen()`, a persistent chunk table (an index per mesh offset), the TLAS rebuilt only when its instances change | CPU `rt.record` + `submit`, GPU `rt` (A showed 1.1 ms CPU, 0.19 ms GPU, over the 0.15 ms threshold) | `stand` rt.record 0.56, submit 0.57, GPU rt 0.19; `fly` 0.60 / 0.57 / 0.17; `orbit` 0.47 / 0.43 / 0.08; frame p50 3.64 / 4.56 / 3.30 (stand / fly / orbit) | `stand` 0.05 / 0.22 / 0.001; `fly` 0.05 / 0.24 / 0.001; `orbit` ~0 / 0.21 / 0.002; frame p50 3.43-3.54 / 4.16 / 2.88. `edit` unchanged (0.69 / 0.62 / 0.77): its remeshes build new BLAS every frame. `views.sh` ok (0 cracks, diffs 0-0.25 %), the `orbit` filmstrip the same frame for frame across the change of frame | keep |
| C. History and smoothing at half resolution, guided by the pixel each 2x2 traced this frame; three `atrous` passes 1, 2, 4 half-pixels apart; the composite upsamples from the four nearest half-res values weighted by normal and plane against the full-res G-buffer; `point_at` from three interpolated corner rays instead of a mat4 per tap | `temporal` + `atrous` from about 1.3 ms to under 0.4 ms | denoise 1.24 / 1.14 / 1.47 / 0.97 ms (stand / fly / edit / orbit); frame p50 3.54 / 4.16 / 5.26 / 2.88, p99 4.47 / 6.42 / 7.99 / 5.12; VRAM 1066 (`edit` 1834) | denoise 0.32 / 0.30 / 0.35 / 0.28; screen pass +0.04-0.10 for the upsampling; frame p50 2.48 / 3.43 / 4.18 / 2.37, p99 3.04 / 5.40 / 5.39 / 4.02; VRAM 810 (`edit` 1578). Lamp room and sunlit slope stills: mean difference 0.1-0.2 of 255, no pixel over 8; `views.sh` 0 cracks, diffs under 0.25 % | keep (the developer's taste call: yes; viewpoints accepted) |
| D1. Visible draws sorted near to far by the distance `select` already computes (`DrawChunk::dist`) | the G-buffer stage; under 10 % less would mean overdraw is no problem | G-buffer p50 1.35 / 1.92 / 2.06 / 0.66 ms (stand / fly / edit / orbit), frame p50 2.48 / 3.43 / 4.18 / 2.37 | G-buffer 0.82 / 1.30 / 1.75 / 0.52 (-40 / -32 / -15 / -21 %), frame p50 2.18 / 2.63 / 4.00 / 2.25; the sort costs nothing measurable on the CPU (`table` +0.01 ms); `views.sh` 0 cracks | keep |
| D2. `light()` out of the fragment shader into a compute pass over the G-buffer | - | - | not done: after D1 the worst p99 of the four runs is 5.1 ms of 8.3 (`edit`), so it does not matter yet; what overdraw is left is inside a chunk. The next lever when the G-buffer stage counts again | later |
| E. A height tile per chunk footprint (34 x 34 `f64` plus min and max, keyed face, level, x, y) in a bounded cache inside `Body` (4096 tiles, the older half evicted), built on the workers; `generate` and `sea_quads` take it, `column_height` and `gen_floor` read a cached level-0 tile and compute directly on a miss, never building one; tans once per row and column through the same `dir` code | `job_us_avg`; a remesh in `edit` under 100 µs; first-time jobs 30 % cheaper where chunks stack; `orbit` without holes at the same speed; heights to the bit | `job_us_avg` 938 / 740 / 854 / 616 (stand / fly / edit / orbit), an `edit` remesh about 790 µs (from the totals) | 363 / 246 / 352 / 182 (-61 / -67 / -59 / -70 %); a remesh after loading 349 µs in `edit`, 222 in `fly`; `orbit` meshes 1266 chunks/s (1168), 0 holes; RAM +5 to +65 MB. A new test checks tiles against `Body::height` to the bit, seams included; the soak's hash for seed 1 (3 min) is the same, 90e4ed04d6628c96; `views.sh` unchanged | keep. The 100 µs remesh is not reached and not a height problem: a probe split an `edit` remesh into 163 µs generating (39,304 cells and 1,508 edits from the tile) and 188 µs meshing 5,419 quads, ten times a normal chunk, from the run's 2,000 random toggles a second |
| F. `select` keeps its last LOD walk (the draw list with bounds, missing and holes, the camera in each body's frame) and walks again only when a job changed a slot's state, a slot was evicted, or the camera moved 4 m (plus 1 % of its height over a body, so the moon's orbit alone does not force a walk); other frames cull the cached list for the look. A remesh that stays drawn updates its entry in place; touched blocks request their cached leaves directly; eviction ages slots from the last walk | `select` under 0.1 ms on a still camera, under 0.5 ms (with dispatch) in `edit` | select p50 0.31 / 0.35 / 0.38 / 0.011 ms, p99 0.45 / 0.76 / 0.49 / 0.75 (stand / fly / edit / orbit); `edit` select + dispatch 0.74 | p50 0.038 / 0.049 / 0.043 / 0.007, p99 0.06 / 0.58 / 0.06 / 0.51; walks on 4 / 25 / 7 / 22 % of frames; `edit` select + dispatch 0.40; missing, holes and chunks meshed per second unchanged; `views.sh` unchanged | keep. `dispatch` (0.36 ms in `edit`) still snapshots `edits_for` on the frame thread: moving it into the job needs `World::edits` as shared `Arc` maps, which the server uses too; not needed for the target, left for later |

### A: where the time goes (quick runs, p50 ms)

| run | rt | gbuffer | trace | temporal | atrous | screen | GPU idle | CPU: select | dispatch | rt.record | submit | waits in acquire |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| stand | 0.19 | 1.36 | 0.14 | 0.20 | 1.04 | 0.15 | 0.51 | 0.35 | 0.00 | 0.56 | 0.57 | 1.93 |
| fly | 0.17 | 1.91 | 0.15 | 0.17 | 0.96 | 0.34 | 0.55 | 0.40 | 0.00 | 0.60 | 0.57 | 2.63 |
| edit | 0.79 | 2.09 | 0.27 | 0.25 | 1.23 | 0.15 | 0.56 | 0.43 | 0.51 | 0.70 | 0.69 | 2.67 |
| orbit | 0.08 | 0.67 | 0.09 | 0.13 | 0.85 | 0.87 | 0.55 | 0.01 | 0.00 | 0.47 | 0.43 | 2.16 |

- Every run is GPU-bound: the frame thread spends half the frame waiting in `get_current_texture`.
- The G-buffer pass (the forward fragment shader that traces the sun, lamp and reflection rays) is the
  largest stage, 1.4-2.1 ms; the smoothing (`temporal` + `atrous`) is next, 1.1-1.5 ms.
- The GPU is idle about 0.5 ms of every frame, between one frame's screen pass and the next frame's first
  command (A2: submitting earlier does not close it).
- The TLAS rebuild costs the CPU 0.5-0.7 ms in `rt.record` and as much again in `submit` (wgpu's validation
  of thousands of instances), on the GPU 0.1-0.2 ms on a still camera, 0.8 ms in `edit` with its BLAS builds.
- **`edit`'s 12 ms frames:** about six frames in a row at t = 9.5 s after loading, in `edit` and `stand`
  alike, in every rep. They are GPU frames, not CPU: `atrous` goes from 1.2 to 7.6 ms and `temporal` from
  0.25 to 0.67 ms, every other stage stays as it was, and the frame thread's "unnamed 9 ms" is the wait in
  `get_current_texture` for that GPU work. The burst stays at t = 9.5 s with a 6 s or 3 s warm-up and with the
  sky's clock moved by 5 s, does not show in `fly` or `orbit`, and does not come back in a 40 s run. Only the
  two bandwidth-bound passes slow down, so the likeliest cause is the GPU driver changing its memory clock
  under sustained load (Discord, Steam, a browser and the NVIDIA overlay share the GPU on the dev box); not
  something in the code to fix. Package C halves those passes' memory traffic, which shrinks the burst too.
- New in the JSON line: `cpu_p50`, `cpu_p99`, `gpu_p50`, `gpu_p99` objects with every stage, and in each
  `spikes` entry `cpu` (the stretches of the frame that interval belongs to) and `gpu` (its GPU stages, read
  back a few frames later). `gpu_ms` keeps its old meaning (G-buffer start to screen end) so the series stay
  comparable; `gpu_p50.total` is the whole GPU frame. The GPU stages need `TIMESTAMP_QUERY_INSIDE_ENCODERS`
  (the RTX 3070 on Vulkan has it); without it there are no GPU times.

## Decision
Keep each package whose row says keep; the reverted ones stay written down with why.

## Consequences
- The stage breakdown is in every benchmark line, so a later regression names its stage.
