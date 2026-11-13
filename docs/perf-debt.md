# Speed debt

What features cost that has not been won back yet (CLAUDE.md principle 14, ADR 0096). One row per feature,
written when it is merged, from one measurement on a quiet PC (`play` build, quick runs, the median of three).
An optimisation batch works down this list, most expensive first, CPU before GPU, and strikes rows out.

Budget (ADR 0004): frame p99 <= 8.3 ms, no frame > 16.7 ms. Main on 2026-11-13: fly GPU p50 2.9 ms,
frame p99 6.3 ms, frame-thread work 0.58 ms; sim work 0.59 ms.

| Feature (ADR) | Run | CPU: frame thread, jobs | GPU | Frame p99 | Ideas to make it cheaper |
|---|---|---|---|---|---|
| Fluffy crowns, sway, twigs from the wood, 32 px flora (0090; carries 0088) | fly | +0.06 ms frame thread (0.68 -> 0.74), jobs +8 % (352 -> 381 us, the walk to wood, leaf faces unmerged); sim +0.10 ms | fly +1.0 ms GPU p50 (2.90 -> 3.91; G-buffer 0.99 -> 1.97: fluff 0.5, the rest the leaf passes and 742k quads against 659k) | 6.82 -> 7.52 | Fluff only within some 30 m (it is invisible beyond); one fluff plane per block, not per face; merge leaf faces again with the root bits per corner; skip the depth pre-pass for chunks far off; `fluff=0` as a Leaves setting |
| Crowns' look fixes: fast leaves as ray candidates (bare crowns let light through), fluff in its own plane, tilted (0090) | fly | +0.23 ms frame thread (0.61 -> 0.84: BLAS builds of the second geometry, `rt` +0.04, `submit` +0.12, `table` +0.06); sim flat | fly +0.4 ms GPU p50 (3.64 -> 4.04; G-buffer 1.94 -> 2.05, the rest any-hit work on shadow rays through fast leaves) | 7.28 -> 8.13 | Keep fast leaves opaque to rays outside winter (rebuild the BLAS geometry flag on a season change instead of a candidate test every hit); ray candidates only within the shadow cascade's reach; the clumps' `vnoise` per vertex could be one value per block from the mesher |
| Trees with branches, denser forests (0092) | `tree_chunk_cost`, 621 forest chunks, one thread | frame thread none; a forest chunk's first generation 1190 us (main 509-585), cached 133 us (same), mesh 139 us (main 89-137) | quads per forest chunk 3285 fast, 6055 fancy (main 1920, 2248) | not measured | build each tree's limb segments once per tile, not per cell; skip cells under the fork that are not trunk; look up only the trees whose crown reaches a column (REACH 6 is the oak's, most crowns are 2-5); merge a crown's inner leaf faces in fancy mode |
