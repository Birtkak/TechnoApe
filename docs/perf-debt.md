# Speed debt

What features cost that has not been won back yet (CLAUDE.md principle 14, ADR 0096). One row per feature,
written when it is merged, from one measurement on a quiet PC (`play` build, quick runs, the median of three).
An optimisation batch works down this list, most expensive first, CPU before GPU, and strikes rows out.

Budget (ADR 0004): frame p99 <= 8.3 ms, no frame > 16.7 ms. Main on 2026-11-13: fly GPU p50 2.9 ms,
frame p99 6.3 ms, frame-thread work 0.58 ms; sim work 0.59 ms.

| Feature (ADR) | Run | CPU: frame thread, jobs | GPU | Frame p99 | Ideas to make it cheaper |
|---|---|---|---|---|---|
| Ore halo (0095) | `tests/gen_cost.rs`: the 6 705 level-0 chunks within reach of a deep body, 4 km planet, seed 1337 | frame thread none; jobs: `generate` 214 µs a chunk against 133 (+81 µs, +61 %), 0.54 s of worker CPU for all of them once | none | not measured (no run changes) | the halo pass costs cells that are not rock or lie outside every grown body: cut each column to its grown body's z span from (u, v) instead of the box; keep `Gen::Solid` for a chunk wholly inside one grade; a sin/cos table per body |
