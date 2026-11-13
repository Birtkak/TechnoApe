# Speed debt

What features cost that has not been won back yet (CLAUDE.md principle 14, ADR 0096). One row per feature,
written when it is merged, from one measurement on a quiet PC (`play` build, quick runs, the median of three).
An optimisation batch works down this list, most expensive first, CPU before GPU, and strikes rows out.

Budget (ADR 0004): frame p99 <= 8.3 ms, no frame > 16.7 ms. Main on 2026-11-13: fly GPU p50 2.9 ms,
frame p99 6.3 ms, frame-thread work 0.58 ms; sim work 0.59 ms.

| Feature (ADR) | Run | CPU: frame thread, jobs | GPU | Frame p99 | Ideas to make it cheaper |
|---|---|---|---|---|---|
| Trees with branches, denser forests (0092) | `tree_chunk_cost`, 621 forest chunks, one thread | frame thread none; a forest chunk's first generation 1190 us (main 509-585), cached 133 us (same), mesh 139 us (main 89-137) | quads per forest chunk 3285 fast, 6055 fancy (main 1920, 2248) | not measured | build each tree's limb segments once per tile, not per cell; skip cells under the fork that are not trunk; look up only the trees whose crown reaches a column (REACH 6 is the oak's, most crowns are 2-5); merge a crown's inner leaf faces in fancy mode |
