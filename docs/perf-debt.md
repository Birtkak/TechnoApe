# Speed debt

What features cost that has not been won back yet (CLAUDE.md principle 14, ADR 0096). One row per feature,
written when it is merged, from one measurement on a quiet PC (`play` build, quick runs, the median of three).
An optimisation batch works down this list, most expensive first, CPU before GPU, and strikes rows out.

Budget (ADR 0004): frame p99 <= 8.3 ms, no frame > 16.7 ms. Main on 2026-11-13: fly GPU p50 2.9 ms,
frame p99 6.3 ms, frame-thread work 0.58 ms; sim work 0.59 ms.

| Feature (ADR) | Run | CPU: frame thread, jobs | GPU | Frame p99 | Ideas to make it cheaper |
|---|---|---|---|---|---|
| Plants of planes and their shadows (0091) | fly quick; grassland stand at noon, Vegetation normal | frame thread 0.75 ms fly (main 0.58), 0.71 stand; mesh workers +47 µs a level-0 chunk for the plant rolls (clutter 80 µs, main 33) | fly p50 3.20 ms (+0.3), stand 4.33 ms (+1.4: gbuffer 2.0, trace 0.69); the shadows alone about +0.5 ms in the meadow | fly 6.97 ms (+0.7), stand 5.43 | Cull plant quads past their reach on the CPU (today the shader shrinks them, so they still cost vertices); a cheaper far layer of one plane per column for the outer half; shadow rays skip plants for pixels past 30 m; plants in the BLAS only within 40 m (a second, nearer instance set); share the patch roll between neighbouring columns in the mesh job |
