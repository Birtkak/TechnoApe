# 0120 – Optimisation batch: the LOD walk at detail 8, dispatch, the looks in slots, the bots' clock

- **Status:** accepted
- **Date:** 2026-11-14

## Context

An optimisation batch (CLAUDE.md principle 14, ADR 0096) after the feature work of 2026-11-14 (ADR 0106 to
0114), CPU before GPU, the look unchanged. The gate had not run on main since those merges, and three
`crates/bot` tests failed inside it under load.

**How it was measured.** Each run under `bash tools/slot.sh measure`. The table of where main stood and the
closing table are the release build (fat LTO), quick runs; the steps between are the play build. Every
before/after is an **interleaved A/B** (ADR 0111): base and branch run alternately, medians. 1080p runs take
default settings; the 1440p runs the developer's settings (`settings=out/opt/dev2560.txt`: 2560 x 1440,
detail 8, leaves fancy, clutter many, shadows soft), the forest of ADR 0111:

```
technoape.exe run=stand at=0.343729,0.523580,0.779560 yaw=73.9 pitch=-16.7 eye=20.83 time=153357.3 \
  seed=933105400 planet=6144 weather=0 save=0 quick=1 settings=out/opt/dev2560.txt k=4     # "forest"
technoape.exe run=fly at=0.343729,0.523580,0.779560 alt=21 time=153357.3 weather=0 seed=933105400 \
  planet=6144 save=0 quick=1 settings=out/opt/dev2560.txt                                  # "flight 8"
```

## Where main stood

Release build, quick runs, median of 3 (the flight of 2), 1920 x 1080 unless said:

| Run | Frame p50 / p99 / max (ms) | Frame-thread work (ms) | GPU p50 (ms) | Jobs (us) | RAM / VRAM (MB) | Budget |
|---|---|---|---|---|---|---|
| stand | 4.19 / 4.76 / 12.1 | 0.66 | 3.75 | 887 | 779 / 869 | ok |
| fly | 3.97 / 5.20 / 9.2 | 0.60 (`select` p99 2.1) | 3.48 | 582 | 768 / 871 | ok |
| sim | 4.65 / 5.98 / 6.8 | 0.88 (tick p99 3.9) | 4.22 | 1093 | 1056 / 1078 | ok |
| orbit | 1.72 / 7.59 / 11.6 | 0.48 | 1.46 | 690 | 1145 / 1078 | ok 2 of 3: one frame of 27.7 ms, `present` 23.9 |
| edit | 6.11 / 7.57 / 8.3 | 1.62 | 5.15 | 998 | 1017 / 1076 | ok |
| fx | 4.21 / 4.78 / 9.2 | 1.06 | 3.77 | 890 | 524 / 869 | ok 2 of 3: one frame of 27.2 ms, `acquire` 25.5 |
| forest, 2560 x 1440, detail 4 | 16.9 / 17.6 / 18.6 | 0.83 | 16.5 (G-buffer 12.5) | 902 | 675 / 1672 | over: GPU |
| flight 8, 2560 x 1440 | 13.5 / 18.7 / 24.2 | 9.4 (`select` 5.5-6.2, p99 13-15; `dispatch` 1.1) | 12.5 | 380 | 720 / 3000 | over: GPU, and the frame thread at p99 |

- **At 1080p principle 9 already held**: every p99 under 8.3 ms, every number at or under
  `tests/perf/baseline.json`. The two frames over 16.7 ms were waits in the driver (`present`, `acquire`), with
  our own stages at their usual cost.
- **The frame thread's one real cost is the LOD walk** (`Streamer::select`): about 1.2 ms a walk at detail 4, a
  third of `fly`'s frames; at the developer's detail 8 in flight 5.5 to 6.2 ms a frame, p99 13 to 15.
- The features of 2026-11-14 are not in a benchmark run: the hotbar's pictures (ADR 0114) and the inventory
  (0113) were measured by a micro-benchmark of their own (below).

## Decision: the changes kept

| # | Commit | Change | Gain |
|---|---|---|---|
| 1 | f810f3ad | A slot remembers `complete`'s answer for one walk (`complete_at`): every level asked it of the whole subtree under each child again. The slots live in a slab, each knowing its eight children's places, so the walk follows links instead of hashing keys (about 5 % by itself) | `walk_cost` (us a walk): detail 4 534 -> 256, detail 8 1999 -> 860. `select` p50/p99 (ms), play build: fly 0.10/2.37 -> 0.10/1.56, orbit 0.05/2.14 -> 0.05/1.56, flight at detail 4 1.65/2.56 -> 1.13/1.77, flight 8 7.10/15.52 -> 4.61/9.88 |
| 2 | 33aa9afb, 465aca23 | `dispatch` returns at once when no worker is free, and sorts only the nearest 256 requests (the rest only if those did not fill the workers), in the stable sort's order; a pending chunk is not handed over while the quad buffer is full | flight 8 `dispatch` p50/p99 1.17/2.79 -> 0.18/2.15 ms |
| 3 | e9c41d67, ec190acd | `hud::Looks` keeps the rects of a look drawn in a slot (a cube's 192, a picture's runs) by all they are made from, and lays them at a slot by moving them | `hud_cost` (us a frame, frame thread): the hotbar's layer 23.2 -> 3.6; 63 cubes of creative's grid 163 -> 33 |
| 4 | 41da5293, b9eb4d29 | A bot's waits, a walk's give-up and a held dig count the server's ticks, not wall time; the tests' servers tick at 20 Hz, not 200 | The three bot tests that failed in the gate under load pass three times in a row beside a full engine test run |

Each keeps its slow path as its test's reference: the slow walk keeps the unremembered `complete` by key
(`the_walk_passes_over_empty_leaves_and_picks_the_same`; a remembered answer kept stale over walks fails three
streamer tests, tried); `dispatch_sends_the_nearest_that_need_a_job_once_each` holds dispatch to the
sorted-first reference, with the jobs that go out lying past the first 256; `a_kept_look_lies_where_it_is_drawn`
holds a moved look to the directly drawn one (a thousandth of a pixel). New ignored benchmarks: `walk_cost`
(engine), `hud_cost` (client). The bot tests were seen failing every time with the tick clock against a 200 Hz
server (a dig held a tenth of its time), the same fault as under load, made certain.

**Not exact, on purpose** (both small, neither seen): a slot's cube of a seasonal block may show a colour up to
1/4096 behind the season (a sixteenth of a step of the screen's 8 bits; else it was made again every frame);
the look-ahead's requests for pending chunks, kept back while the quad buffer is full, are asked for up to
`prefetch_every` (4) frames later once room comes.

The look: `bash tools/views.sh` in the gate (below); the hotbar and creative's inventory filmed with the base
and the branch, the opaque middles of 54 cubes compared: 35 of 5,400 pixels differ, by 1 of 255.

## Before and after

Release build, interleaved A/B against main (727df052), 2 rounds:

| Run | Frame p50 / p99 / max (ms) | Frame-thread work (ms) | `select` p50 / p99 (ms) | GPU p50 (ms) |
|---|---|---|---|---|
| stand | 3.79 / 4.74 / 6.6 -> 4.01 / 4.80 / 7.2 | 0.69 -> 0.80 | 0.07 / 0.11 -> 0.09 / 0.11 | 3.40 -> 3.57 |
| fly | 3.95 / 5.25 / 10.7 -> 3.88 / 5.21 / 8.4 | 0.71 -> 0.65 | 0.08 / 2.12 -> 0.08 / 1.36 | 3.49 -> 3.46 |
| sim | 4.73 / 5.96 / 7.6 -> 4.83 / 5.97 / 6.3 | 0.90 -> 0.76 | 0.08 / 0.11 -> 0.07 / 0.10 | 4.35 -> 4.53 |
| orbit | 1.64 / 7.45 / 11.7 -> 1.80 / 7.69 / 16.3 | 0.39 -> 0.43 | 0.04 / 1.74 -> 0.04 / 1.45 | 1.44 -> 1.50 |
| edit | 6.16 / 7.59 / 9.5 -> 6.26 / 7.73 / 8.6 | 1.38 -> 1.58 | 0.07 / 0.11 -> 0.08 / 0.11 | 5.20 -> 5.23 |
| fx | 4.14 / 4.88 / 5.8 -> 3.93 / 4.87 / 7.5 | 1.03 -> 1.06 | 0.07 / 0.11 -> 0.07 / 0.10 | 3.75 -> 3.49 |
| forest, 1440p, detail 4 | 17.1 / 18.2 / 18.9 -> 17.1 / 17.9 / 19.3 | 1.13 -> 1.14 | 0.07 / 0.11 -> 0.07 / 0.10 | 16.69 -> 16.70 |
| flight 8, 1440p | 13.6 / 18.9 / 23.9 -> 13.3 / 18.3 / 19.3 | **9.44 -> 6.03** | **6.51 / 14.40 -> 4.08 / 8.65** (`dispatch` 1.20 / 3.37 -> 0.18 / 0.34) | 12.65 -> 12.34 |

With two rounds the 1080p rows' differences in work (0.1 to 0.2 ms either way) and GPU are run-to-run spread;
what moved is `select`'s p99 and the flight at detail 8. RAM, VRAM and job time are unchanged (a slot is some
60 bytes larger; 27,000 slots at detail 8).

**Still over budget.** Only the 1440p runs, and by the GPU: the forest's G-buffer (12.6 ms of 16.7), which
ADR 0111 traced to the sun's rays; nothing in this batch touches it. The flight at detail 8 is GPU-bound at
12.3 ms with 6.0 ms of frame-thread work under it.

## Tried and dropped

- **Keeping the drawn chunks' bounds from the walk** instead of a look-up each at its end: nothing measurable.
- **A filter of the requests before dispatch's sort** (07d6baf7): 983 -> 216 us in the benchmark's full-buffer
  case, but slower in the game (flight 8 `dispatch` p50 1.26 -> 2.18 ms: the old loop stopped after a handful,
  the filter looked up all 20,000). **A heap** (33aa9afb) took p50 to 0.23 but p99 from 2.94 to 3.49 ms (every
  request popped when few jobs go out). The partial sort replaced both.
- **The pending leaves of a full-buffer walk**: timed by section, the cost is spread evenly over the leaves; no
  hot spot left to take.
- **The undercut's flood on the server** (ADR 0112): not done. A surface dig costs 82 us against a 50 ms tick,
  2 ms at worst, once a dig; stopping the flood at open sky changes which openings count and is not worth it.
- **Piles (`piles::lay`) and painted boxes as greedy quads** (ADR 0114): not done. Their row measured 0.5 us a
  stack on the frame thread and +0.14 ms of GPU for 200 lying axes; a play scene holds tens of stacks. The
  limits in that row (65,536 model quads shared, one quad a pixel) stand.
- **`grid_items()` twice a frame, the ape as colour runs** (ADR 0113): not done; microseconds, and only with the
  inventory open.

## Review

Two fresh reviewers (subagents given the diff and the task, not the author's account), the second for the
commits made after the first.

- No correctness bug found in the walk's slab and remembered answers, dispatch's order, the kept looks, or the
  bots' clock; each claim of "the same as before" was traced.
- A bot could wait forever on a server that keeps its link open but ticks no more: fixed (b9eb4d29, silent for
  120 s is gone).
- A seasonal block missed the kept looks every frame: fixed (ec190acd, the 1/4096 step above).
- The textures' reload count was bumped before the paintings were written: moved after (same commit).
- Not from this batch, left as a follow-up: after a remesh in place frees room in a full quad buffer, a still
  camera's frames only cull, so pending chunks are not asked for until something forces a walk
  (`Streamer::apply`: mark the walk dirty when a release clears `Alloc::full`).

## The gate

`bash tools/slot.sh measure bash tools/check.sh`, once, on 465aca23: **not green**. Tests ok (the bot tests
among them), the soak ok (seed 82248, 50 bots), its replay not diverged, every benchmark run inside the speed
guard's baseline, the played run's feel ok (0 corrections, a click shown in 12.3 ms), 11 of 12 views within
tolerance with no cracks. The four failures, and what a second look found:

- **`season-winter` 0.619 % of pixels off its baseline (0.5 allowed).** The view is not the same twice: main's
  build gives 0.283 and 0.489 on two runs, this branch 0.395 and 0.401, and the two builds differ from each
  other by as much as either from itself. Its baseline predates the day's merges and it sits at its tolerance
  on main. Not accepted here: looking at it and accepting it is a look call.
- **`fly` one frame of 29.9 ms, `sim` one of 24.4, `edit` p99 8.314 ms.** The PC was not alone (the tests took
  461 s where they take about 150, and another TechnoApe ran on the GPU). The three runs again with the gate's
  own build under the measure lock, twice each: all six inside the budget (fly p99 5.5, max 6.6; edit p99 7.8,
  max 9.0; sim p99 6.6, max 10.8) and inside the baseline. `accept-perf` was not run: at 1080p the batch moves
  nothing past the spread.

## Options that change the look or the memory (not taken; the developer's call)

Measured on today's play build, 2560 x 1440, 2 runs each:

| Option | Effect |
|---|---|
| Quad buffer 16 M -> 32 M (`quad_cap_m=32`) at detail 8 | In the forest flight 3,800 chunks are missing with the buffer full (20 failed allocations); with 32 M none are missing, `select` p50 3.92 -> 2.59 ms, `dispatch` 0.19 -> 0.05. Costs 128 MB more buffer and the missing chunks' own memory: wgpu's total 2,150 -> 2,490 MB (budget 6 GB). The scene needs 127 MB of quads, a hair under the 128 MB there are: first-fit leaves gaps |
| `shadows=sharp` in the forest (detail 4) | GPU p50 16.84 -> 13.41 ms (G-buffer 12.67 -> 9.23): one sun ray a texel, a hard shadow edge (stills: ADR 0111) |
| `leaves=fast` in the forest (detail 4) | GPU p50 16.84 -> 15.53 ms: leaves as whole blocks to the light |

## Consequences

- The walk costs about 40 % less at every detail; at detail 8 in flight it is still 4 ms a frame, on every
  frame the camera moves 4 m or a job lands. Left, ranked: the bigger quad buffer (above; the missing chunks
  are also seen), a ceiling per region so the walk makes no slots in the air (perf-debt, ADR 0105's row), and
  walking on a worker with the frame thread only culling.
- The forest at 1440p stays GPU-bound: ADR 0111's list (sun rays once a texel in a pass of their own) is
  unchanged and is the next GPU batch.
- A bot script's `wait s` is `s` seconds of the server's ticks; on a real server nothing changes.
