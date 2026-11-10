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
- The GPU is idle about 0.5 ms of every frame: nothing is submitted until the swapchain image is acquired,
  and the CPU then needs about 1.3 ms (`rt.record`, `submit`) before the GPU has work again.
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
