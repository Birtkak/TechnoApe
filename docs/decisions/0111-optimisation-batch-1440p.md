# 0111 – Optimisation batch: the orbit spike, the walk at detail 8, the forest at 2560 x 1440

- **Status:** accepted
- **Date:** 2026-11-14

## Context

An optimisation batch (CLAUDE.md principle 14, ADR 0096) after the 11-14 bug round, in order: (1) the 1080p
budget (p99 <= 8.3 ms, no frame > 16.7) in `fly`, `sim` and `orbit`, the orbit spike first; (2) the forest
of the 2026-11-14 report at 2560 x 1440, its view and a flight over it, as far under 8.3 ms as possible
without changing the look; (3) CPU before GPU.

**How it was measured.** Play build, quick runs, each under `bash tools/slot.sh measure`, median of three.
Run-to-run drift over an hour was larger than most effects (fly GPU p50 3.17 at 06:30, 3.56 at 07:30 on
the same code; every CPU stage 1.4x slower in some series, `present` too), so the before/after table is
an **interleaved A/B**: the base (a05c3b80) and this branch built side by side, run alternately, three
rounds (`out/opt/ab.sh`, not kept in the repo).

**How the size is set.** A run has no size argument: the window size comes from the settings file's
`width=` and `height=`. The 1440p runs take a copy of the developer's `settings.txt` (2560 x 1440,
detail 8, leaves fancy, clutter many, shadows soft, clouds low, bounce on) as `settings=out/opt/dev2560.txt`.
The report's quads (2.59 M) match **detail 4** (`k=4`: 2.62 M); the settings file now says detail 8, so both
are measured. The exact command lines:

```
# 1080p (default settings)
technoape.exe run=fly quick=1 save=0          # likewise run=sim, run=orbit
# the report's view, 2560 x 1440
technoape.exe run=stand at=0.343729,0.523580,0.779560 yaw=73.9 pitch=-16.7 eye=20.83 time=153357.3 \
  seed=933105400 planet=6144 weather=0 save=0 quick=1 settings=out/opt/dev2560.txt k=4
# a flight over the same forest, 21 m up
technoape.exe run=fly at=0.343729,0.523580,0.779560 alt=21 time=153357.3 weather=0 seed=933105400 \
  planet=6144 save=0 quick=1 settings=out/opt/dev2560.txt k=4
# (drop k=4 for detail 8)
```

## What was found

- **The orbit spike** (t = 4.04 s every run, frame 14-15 ms, `rt` 11.5-14.2 ms on the frame thread) was the
  TLAS build: the TLAS was made with `max_instances` = 65536 (the chunk table's size), and wgpu 30 walks every
  slot on a build (the wrapper and wgpu-core collect a `Vec<Option<_>>` of the full size), about 2.7 ms a build
  and up to 14 ms when the build met a burst of new BLAS. The scene uses a few hundred to a few thousand.
- **The walk at detail 8.** With the 16 M-quad buffer full (`alloc_fail`), a coarse chunk's hidden child never
  came, and a left-out cover marked the walk dirty: `select` walked every frame with a still camera, 6.2 ms
  a frame (1440p forest stand, detail 8: frame-thread work 8.3 ms).
- **The forest at 1440p is GPU-bound** (detail 4: GPU p50 16.5 ms, G-buffer 12.4 of it, frame-thread work
  1.1 ms). Of the G-buffer: all sun rays about 5.5 ms, the second ray and the penumbra's two more 3.3 ms
  (`shadows=sharp`), all direct light 6.8 ms, fancy leaves' extra 0.8 ms; plants (`vegetation=off`) and
  the bounce's effect on the G-buffer nothing; 640 x 360 still 3.3 ms (the vertex and raster share).

## Decision: the changes kept

| # | Commit | Change | Gain (interleaved A/B, median of 3) |
|---|---|---|---|
| 1 | a9f30360 | The TLAS starts at 4,096 slots and is made anew at twice its need (power of two, at most the old 65,536) when instances and splits outgrow it; the G-buffer's and the bounce's bind groups are made anew with it | `rt` p99: orbit 2.09 -> 0.45 ms, fly 2.06 -> 0.64, sim 1.98 -> 0.60, forest flight 2.81 -> 1.01. Frame max: orbit 14.9 -> 10.4, fly 11.9 -> 8.2, sim 10.5 -> 7.1; p99 fly 6.96 -> 5.25, sim 8.06 -> 6.22 |
| 2 | d1296658, d2e6f39a | A left-out cover asks for a new walk only when the view's planes changed or the camera moved (the still check's measure without its 4 m slack; the moon's orbit does not count); jobs and evictions still mark the walk dirty | forest stand at detail 8: frame-thread work 8.16 -> 1.07 ms, `select` p99 8.2 -> 0.1; frame p50 21.0 -> 20.8 (GPU-bound). Nothing elsewhere (a moving camera walks anyway) |

Tests: `rt::tests::the_tlas_grows_to_twice_its_need_and_no_further_than_the_most` (seen failing with the need
not doubled); `stream::tests::a_coarse_chunk_does_not_cover_once_its_visible_children_are_in` now also asks
that the same look and place do not walk again, a step aside and a turn do (both seen failing first). A grow
forced from 64 slots was filmed and looked right. Views: 0 cracks, every view within tolerance (not accepted).

**Review** (a fresh reviewer, given the diff and the task): nothing wrong in the TLAS change (every bind
group that holds the TLAS is rebuilt, the grow rebuilds all instances, the cap holds). One gap in the walk:
the camera's place, not only its look, decides what is hidden, so a strafe without turning under 4 m could
keep a hole; fixed in d2e6f39a (the first try, an exact compare of the camera, walked every frame for the
moon's motion and was replaced by the still check's measure).

## Before and after

1920 x 1080 default settings; 2560 x 1440 with the developer's settings (interleaved A/B, median of 3):

| Run | Frame p50 / p99 / max (ms) | Frame-thread work (ms) | GPU p50 (ms) |
|---|---|---|---|
| fly | 4.04 / 6.96 / 11.9 -> 4.01 / 5.25 / 8.2 | 0.67 -> 0.69 | 3.49 -> 3.54 |
| sim | 5.30 / 8.06 / 10.5 -> 5.23 / 6.22 / 7.1 | 0.84 -> 0.80 | 4.68 -> 4.76 |
| orbit | 1.81 / 7.66 / 14.9 -> 1.86 / 7.61 / 10.4 | 0.37 -> 0.42 | 1.52 -> 1.48 |
| forest stand 1440, detail 4 | 17.0 / 20.6 / 21.0 -> 17.0 / 18.5 / 18.9 | 1.07 -> 1.20 | 16.6 -> 16.6 (G-buffer 12.5) |
| forest flight 1440, detail 4 | 12.0 / 17.8 / 19.6 -> 12.1 / 16.6 / 20.1 | 2.67 -> 2.59 | 11.4 -> 11.6 (G-buffer 7.6) |
| forest stand 1440, detail 8 | 21.0 / 21.7 / 22.1 -> 20.8 / 21.6 / 22.0 | 8.16 -> 1.07 | 20.4 -> 20.3 (G-buffer 16.1) |

Goal 1 holds: no 1080p frame over 16.7 ms and every p99 under 8.3. Goal 2 does not: the forest at 1440p
is GPU-bound at 11.6-16.6 ms, and what is left to win without changing the look is a restructure (below).
The forest flight at detail 8 (not in the A/B; earlier series) is frame-thread bound: `select` p50 8.2 ms,
p99 17, dispatch 1.8, with the quad buffer full (30 failed allocations, 3,874 chunks missing).

## Tried and dropped

- **Shared sun rays in a 2 x 2 quad** (the pixels of one light texel in a quad split the rays with
  `quadBroadcast`): slower, G-buffer 12.3 -> 13.2-15.0 ms; a warp with both kinds of quad runs both paths.
- **An opaque terrain depth pre-pass**: G-buffer 12.37 against 12.32, nothing.
- **Dropping old BLAS a few a frame** (a "graves" queue): orbit max 10.9 against 10.6 without, nothing.
- **An alpha-only texel for ray candidates** (not built): ADR 0104 measured that a candidate's cost is
  the round trip, not its checks (one that only continues costs the same), and the driver drops the unused
  colour work after inlining.

## Options that change the look (not taken; the developer's call)

Measured on the report's view at 2560 x 1440, detail 4 (median of 3); stills in
`out/bugs-1114/opt-forest-now.png`, `-sharp.png`, `-fast.png`, and the three side by side, the middle-lower
crop at 2x, in `opt-forest-compare.png`:

| Option | GPU p50 (ms) | G-buffer (ms) | What changes |
|---|---|---|---|
| now | 16.5 | 12.4 | |
| `shadows=sharp` | 13.2 (-3.3) | 9.1 | One sun ray a texel: a hard shadow edge, no quarter steps in the penumbra |
| `leaves=fast` | 15.6 (-0.8) | 12.1 | Leaves as whole blocks to the light (quads 2.6 M -> 2.0 M) |

## Consequences

- The TLAS is now sized by its scene; a first grow costs one TLAS creation and full build on the frame
  thread, once per doubling (a log line names it).
- Left for the next batch, ranked:
  1. **Sun rays once a texel, not once a pixel** (exact: the light is a texel's already, ADR 0108). At 20 m
     a texel covers some 25 pixels at 1440p; a pass that lists the visible texels, traces them once and lets
     the G-buffer read them back is the one big win left that keeps the look (up to about 5 ms of 5.5). It
     needs a deferred restructure; the 2 x 2 quad share above shows a shortcut does not do it.
  2. **The quad buffer at detail 8**: 16 M quads (128 MB) fill in the forest, chunks go missing and the walk
     works hard every frame in flight (`select` p99 17 ms). A bigger buffer is a VRAM and capacity call (the
     developer's), the walk's own cost (about 6 ms at detail 8) a ceiling per region (perf-debt, ADR 0105 row).
  3. The vertex and raster share (3.3 ms at 640 x 360 with 2.6 M quads): fewer quads far off.
