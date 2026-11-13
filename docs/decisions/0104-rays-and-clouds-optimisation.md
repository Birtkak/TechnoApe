# 0104 – Rays and clouds: the bounce skips the plants, open sky skips the cloud cells

- **Status:** accepted
- **Date:** 2026-11-13

## Context
An optimisation batch (ADR 0096) on the shadow rays, the trace stage, the screen stage and the clouds
(`docs/perf-debt.md`). The developer saw 10-13 ms of GPU at 2560 x 1440 in forest and meadow; the off-screen
1920 x 1080 fly run hides per-pixel cost. Measured here at 2560 x 1440 with the developer's settings (fancy
leaves, soft shadows, low clouds; `settings=out/dev2560.txt`), two stand views of seed 1337 at 9:10 (`time=300`):
a meadow (`at=0.0347,0.8675,0.4975 eye=2 pitch=-10`) and a forest edge (`at=0.0347,0.8675,0.4775 eye=2
pitch=5`), `weather=cloudy` so the clouds do not change with the server's wall clock; and `run=fly quick=1
seed=1337 save=0`. `play` build, `slot.sh measure`, other agents sharing the PC between runs.

## Where the time is (one run each, the meadow / the forest, GPU p50 12.7 / 14.0 ms)
Toggles built in for the survey and taken out again:
- the sun's shadow ray off: G-buffer -2.7 / -3.8 ms. It is all in the any-hit loop: with every candidate
  forced opaque the G-buffer drops 1.8 / 2.7 ms; skipping still candidates past 30 m saves 0.2 ms, past 100 m
  nothing; still candidates that only `continue` cost the same as the full checks.
- `plant_shadows=0` (no plants in the BLAS): GPU -2.6 / -3.0 ms: G-buffer -1.5 / -2.1, **trace -1.0 / -0.9**.
  The bounce rays (flags 0) never confirm a plant, yet walked every blade.
- clouds off: screen -1.0 / -1.4 ms with the air's own weather (-0.55 / -0.7 overcast), G-buffer -0.6
  (`cl_shadow` on every lit pixel).
- stop a shadow ray below 10 % light (fast leaves): fly trace -0.06 ms, G-buffer flat; nothing with fancy
  leaves. Clamping the sun ray's `tmax` to the terrain's shell: nothing. Sharp shadows: -0.1 ms.

## Options considered
1. **Stills as instances of their own** (kept): a chunk's still models and plants get their own BLAS and TLAS
   instance (mask `16 << body`, custom index plus `STILL_INST`, `Chunk::sto` the first still quad); shadow and
   lamp rays ask for them, the bounce and reflections do not. Exact: those rays never confirmed a still.
   Costs on the frame thread were the trap: splitting every chunk with a pebble, or one with room for twice the
   instances, cost the fly run's p99 2 ms (wgpu's TLAS build walks its slots: 4096 extra slots alone cost
   0.5 ms); splitting and joining chunks as the camera moved added TLAS builds. Kept: only chunks first built
   within 128 m are split, they stay split until their mesh goes, at most 4096 and never past the slots the
   chunks leave free, in a TLAS of the old size.
2. Shadow rays skip plants past 30 m (the debt list's idea): -0.2 ms in the meadow and a look change; dropped.
3. Stop a ray after its second leaf face / under 10 % light: nothing measurable; dropped (ADR 0090's shade).
4. Fast leaves opaque outside winter: not tried, it is ADR 0090's look (the green shade through a crown).
5. Cheaper still candidates (kept, exact, no measurable gain alone): `still_texel`'s corner arrays written
   out (a run-time indexed array spills), and a plant hit nearer than 0.44 of the far distance skips its reach
   hash (every plant reaches that far; `frame.rs` asserts `PLANTS_FAR` stays under 120 m, where a shrub's
   bound holds).
6. Clouds (kept, exact): a cell under `CL_OPEN` cover cannot fill, so `cl_sample` skips its hashes, and
   `cl_soft` returns 0 at once; `cl_shadow` reads the wind from the same locate and texel as the cover (was a
   second `cl_locate` and fetch). Clear sky: screen 1.78 -> 1.49 ms in the meadow; overcast: nothing (every
   column has cover). A coarser first step was not tried: the cells are 2 m across, the steps already 4 m.

## Decision
1, 5 and 6. Numbers (median of 3 unless said, GPU p50 / trace / screen / G-buffer / frame p99, ms):

| Run | Before (HEAD 7c840305) | After |
|---|---|---|
| fly, 1920 x 1080 | 5.77 / 0.48 / 1.27 / 3.54 / 9.90 | 5.80 / 0.58 / 1.29 / 3.52 / 9.96 (no change: trace 0.46-0.63 both) |
| meadow, 2560 x 1440 (5 before) | 11.99 / 1.56 / 1.33 / 8.31 / 13.16 | 11.30 / 0.79 / 1.37 / 8.52 / 12.41 |
| forest, 2560 x 1440 (5 before) | 13.08 / 1.45 / 1.40 / 9.47 / 14.11 | 12.39 / 0.85 / 1.41 / 9.58 / 13.47 |
| meadow, clear sky (one each) | 12.25 / 1.54 / 1.78 / 8.12 / 13.54 | 10.88 / 0.57 / 1.49 / 8.06 / 12.18 |

The G-buffer rises 0.1-0.2 ms in the stands: a shadow ray now walks two instances where a chunk had one.

The pictures: `shot=` stills of the meadow and the forest (clear), the clouds from the meadow looking up and
from the 350 m summit (overcast), before and after: identical but for the bounce's noise (mean difference
0.002 of 255 at most, no pixel off by more than 8). A grazing animal walked into one meadow shot.

## Consequences
- The stand views at 2560 lose 0.6-0.7 ms of GPU p50 and 0.65-0.75 ms of frame p99 (the trace stage about
  halves); fly at 1080p is unchanged (few chunks are split there: they come in from farther than 128 m) and
  so is its frame p99. Still over the 8.3 ms budget everywhere at 2560.
- `Chunk` (shader.wgsl) and `ChunkGpu` have a sixth word, `sto`; the expand pass builds them with six.
- Left on the debt list: shadow rays through blades (about 1.5 ms in the meadow, 2 in the forest at 2560),
  fancy leaves' candidates (about 1 ms in the forest), the cloud march under partial cover (1.0-1.4 ms) and
  `cl_shadow` (0.6 ms). The next win is fewer blade crossings per ray, not cheaper checks.
- Review (a fresh reviewer read the diff): no correctness finding; the exact claims checked (the uv formula,
  the 0.44 bound, the cover bounds, quad offsets, masks, slot counts). Answered: (1) the comments said "split at
  first build" while the code rebuilt chunks on approach: now it splits only at first build, which also cut
  the TLAS builds back to the old count; (2) the BLAS budget can be passed by one (harmless, the builds buffer
  is sized by chunks); (3) the 0.44 bound needs `still_far() <= 120`: a const assert in `frame.rs`.
