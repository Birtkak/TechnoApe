# Speed debt

What features cost that has not been won back yet (CLAUDE.md principle 14, ADR 0096). One row per feature,
written when it is merged, from one measurement on a quiet PC (`play` build, quick runs, the median of three).
An optimisation batch works down this list, most expensive first, CPU before GPU, and strikes rows out.

Budget (ADR 0004): frame p99 <= 8.3 ms, no frame > 16.7 ms. Main on 2026-11-13: fly GPU p50 2.9 ms,
frame p99 6.3 ms, frame-thread work 0.58 ms; sim work 0.59 ms.

| Feature (ADR) | Run | CPU: frame thread, jobs | GPU | Frame p99 | Ideas to make it cheaper |
|---|---|---|---|---|---|
| Fluffy crowns, sway, twigs from the wood, 32 px flora (0090; carries 0088) | fly | +0.06 ms frame thread (0.68 -> 0.74), jobs +8 % (352 -> 381 us, the walk to wood, leaf faces unmerged); sim +0.10 ms | fly +1.0 ms GPU p50 (2.90 -> 3.91; G-buffer 0.99 -> 1.97: fluff 0.5, the rest the leaf passes and 742k quads against 659k) | 6.82 -> 7.52 | Fluff only within some 30 m (it is invisible beyond); one fluff plane per block, not per face; merge leaf faces again with the root bits per corner; skip the depth pre-pass for chunks far off; `fluff=0` as a Leaves setting |
