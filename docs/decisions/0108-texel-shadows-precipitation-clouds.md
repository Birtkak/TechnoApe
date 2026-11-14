# 0108 – Light snapped to texels, rain only under the clouds, fairer clouds

- **Status:** accepted
- **Date:** 2026-11-14

## Context
Three bug reports of 2026-11-14:

- `01-59-35` "shadows are way to grainy for having a blocky voxel world" (night in a rain forest, moonlight).
  The sun's soft shadow was one ray a pixel to a dithered point of its disc (`in_disc`), the lamps' one ray a
  pixel to a random point of their block (`lamp_target`, ADR 0072), the bounce one ray a pixel's 2 x 2 with a
  per-pixel seed (ADR 0022); `direct_smooth` (post.wgsl, ADR 0098) blurred the direct dither over 3 x 3 pixels.
  The developer's decision: **snap the light to the block texture's texels.**
- `01-55-53` "its snowing while i am above the clouds": the drops were spawned a few metres over the camera
  wherever the column's air said it rained; the developer stood at 327 m, the slab is 250 to 314 m.
- `02-03-05` "clouds still feel a bit bunched up and unforgivingly dense". A look call left to Claude: scattered
  cumulus with blue between in fair weather, overcast only at a really high cloud share, softer and lighter
  bodies, shadows to match; the cover still the air grid's (ADR 0010, 0104, 0105).

## Options considered
For the texels:
1. Snap through screen derivatives of `local` and `rel` – no use in `gi_trace`, which has no derivatives, and
   fragile on grazing faces.
2. **Snap on the cube-sphere grid itself** – a body-frame point to (face, column i, column j, layer), the two
   in-face coordinates to their texel's middle, back to the body frame; the face's own axis left alone. The same
   free functions serve the fragment (from `local`, the painting's own point, so a swaying crown's light rides
   with its texels) and `gi_trace` (from the G-buffer's point and normal). Testable on the GPU alone.
3. Random disc point seeded by the texel – the grain becomes texel-sized dither, not the flat steps asked for.
4. **No random numbers for direct light at all** – rays to two opposite points of the sun's disc (or the lamp's
   seen sides), two more across them only where those disagree: a penumbra is flat steps of a quarter.

## Decision
Options 2 and 4.

- **Where.** `terrain_lit` (every block face, cutout blocks and the crowns' fluff cards) takes its direct light at
  `lit_at`, the middle of its painting's texel (16 a cell edge; a coarse LOD level's as large as it draws them).
  Fine 32 px paintings (ADR 0090) are lit 2 x 2 painting texels a light texel. The functions live between
  `// ---- the light's texels` markers in shader.wgsl (`tx_grid_of`, `tx_point`, `tx_snap`, `tx_seed`, `tx_k`).
- **Past a texel a pixel.** `tx_k`: where one pixel spans more than a texel (about 54 m at 1440p, 40 m at 1080p
  facing a face, nearer on a face seen edge-on), the light's texels double until one spans a pixel again, on the
  same grid of whole texels; light never varies finer than the pixels, so nothing aliases or crawls. The step is a
  power of two by distance, like mips.
- **Sun and lamps.** `sun_seen` and `lamp_seen`: 2 rays, 4 in a penumbra, no random numbers; the sharp-shadow
  setting keeps one ray to the sun's middle. The moon was always one ray to its middle. The glass tint is the mean
  of the rays that got out.
- **Bounce.** `gi_trace` leaves from the texel's middle, its direction from the texel's seed and the frame, so
  every pixel of a texel shares one sample a frame and the history smooths it texel by texel.
- **No texel grid** (plants' stills, models, particles, water, glass panes' own pixels): their own pixel's point
  as before, but they take the same noiseless `sun_seen` / `lamp_seen`, so they have no grain either. Water's
  ripples are already on the blocks' 1/16 m grid; snapping its light was not needed for the reports.
- **`direct_smooth`** is kept: with no dither left in the direct light it only softens a texel step's edge by a
  pixel; the stills did not prove it can go (not compared with it off).

Precipitation: `weather::from_cloud(height over the sea)` is 1 under the slab's base, falling to 0 at its top; a
drop's spawn point draws against it (no draw under the base, so drops there are exactly as before) and the rain's
sound at the head is scaled by it. `CL_BASE`/`CL_THICK` live in weather.rs too; a test holds them to clouds.wgsl's
numbers (the lightning's height used a bare 250).

Clouds: `cl_cover` maps the air grid's share to the cover the cells fill to, `c - 1.4 max(c - 0.3, 0) (1 - c)`
(0.3 → 0.3, 0.6 → 0.43, 0.95 → 0.90), in the clouds, their shadow field and the sky's grey (frame.rs through
`weather::cl_cover`, which a test holds to the shader's line). A cell's edge fades over
a margin of 0.15 (was 0.04), and the light falloff `CL_K` is 0.09 again (0.14 since ADR 0098): bodies are softer and
thin edges are see-through. The shadow is `1 - 0.8 smoothstep(field)`, so a cloud's thin edge lets most of the sun
through and a thick body still takes 80 %.

Stills (before and after, `out/bugs-1114/`): `shadow-*` the bug's night view, a noon meadow with the plants'
shadows on grass (spawn forest, ADR 0060's summer view), lamps at night; `snow-*` the bug's alpine top under
`/weather snow`; `clouds-*` the spawn at a forced cover of 0.3, 0.6, 0.95 (`weather=` now takes a number), from
400 m and from orbit.

## Cost
Measured once after all three (ADR 0096), `play` build, `bash tools/slot.sh measure`, one run each of the commit
before (ffc9f557 plus `weather=<number>`) and this branch, default size and `settings=` at 2560 x 1440. GPU stages
p50 in ms (before → after); `gbuffer` holds the direct light (sun, moon, lamps), `trace` the bounce, `screen` the
composite and the clouds:

| run | gbuffer | trace | screen | GPU total p50 / p99 | frame p50 / p99 |
|---|---|---|---|---|---|
| stand quick, 1920 x 1080 | 0.74 → 1.05 | 0.12 → 0.14 | 1.57 → 1.73 | 2.95 → 3.43 / 3.13 → 3.74 | 3.46 → 3.96 / 8.03 → 8.56 |
| stand quick, 2560 x 1440 | 1.36 → 1.81 | 0.20 → 0.23 | 2.79 → 3.01 | 4.89 → 5.79 / 5.21 → 6.37 | 5.98 → 7.36 / 10.7 → 11.5 |
| fly quick, 1920 x 1080 | 3.07 → 3.15 | 0.44 → 0.46 | 1.24 → 1.48 | 5.28 → 5.62 / 6.72 → 7.13 | 5.81 → 6.23 / 9.36 → 9.94 |
| fly quick, 2560 x 1440 | 4.35 → 4.55 | 0.83 → 0.92 | 1.97 → 2.47 | 7.84 → 8.53 / 10.2 → 10.5 | 8.47 → 11.2 / 15.4 → 18.2 |

- **GPU:** +0.3 to +0.45 ms in `gbuffer` near the ground (the second sun ray everywhere the sun lights, a third and
  fourth in penumbras, up to four a lamp) and +0.2 to +0.5 ms in `screen` (the clouds' softer, more see-through
  bodies end their march later: fewer rays stop at `tr < 0.02`). About +0.5 ms at 1080p, +0.7 to +0.9 at 1440p.
- **Frame thread:** its own work (select, table, rt, submit) is unchanged within noise; the larger frame times are
  `acquire`, the wait for the GPU. The 1440p fly's p99 of 18.2 ms is over principle 9's budget (it was 15.4).
- **Ideas to make it cheaper:** trace the second sun ray only where the first one's texel neighbours disagree
  (a cheap screen-space test on the G-buffer); skip the penumbra rays past `tx_k`'s first doubling, where a texel
  step is under a pixel anyway; cap the cloud march at a lower transmittance where the slab is thin.
- Not accepted into `tests/perf/baseline.json` (the gate was not run on this branch).

After the review's fixes (c9b03232), measured again the same way, before (ffc9f557) and after in one series:

| run | gbuffer | trace | screen | GPU total p50 / p99 | frame p50 / p99 |
|---|---|---|---|---|---|
| stand quick, 1920 x 1080 | 0.75 → 1.44 | 0.12 → 0.14 | 1.53 → 1.83 | 2.91 → 3.73 / 3.10 → 4.06 | 3.43 → 4.17 / 4.17 → 4.80 |
| stand quick, 2560 x 1440 | 1.35 → 2.13 | 0.20 → 0.23 | 2.72 → 3.23 | 4.81 → 6.36 / 5.10 → 6.86 | 5.43 → 6.82 / 6.10 → 7.68 |
| fly quick, 1920 x 1080 | 2.94 → 3.30 | 0.43 → 0.52 | 1.18 → 1.43 | 4.94 → 5.75 / 6.24 → 7.18 | 5.49 → 6.22 / 9.18 → 9.77 |
| fly quick, 2560 x 1440 | 4.34 → 4.81 | 0.81 → 1.02 | 1.93 → 2.40 | 7.78 → 8.84 / 9.73 → 10.8 | 8.20 → 9.34 / 13.1 → 14.1 |

- `gbuffer` near the ground costs more than in the first round (+0.7 ms against +0.3 at 1080p stand). Not
  pinned down (ADR 0096: measured once, not optimised); suspects are `lamp_seen`'s arrays of ends, answers and
  tints (registers in the largest fragment shader, even where no lamp shines) and the diagonal sun pairs
  disagreeing more often in grass. `trace` is +0.1 to +0.2 ms in fly though `seen` now casts fewer rays: plants
  and slopes keep per-pixel seeds, so the history converges less evenly; not looked into.
- Frame p99 stays under 16.7 ms on all four (fly 1440p 14.1 in this series; the 18.2 of the first round was that
  run's own spike).

## Review
A fresh reviewer read the first three commits. Its findings and what was done:

- **Stills lit on garbage texels (must).** `fs_still` handed `terrain_lit` a VOut whose `local`/`dir` are the
  painting's pixel and an `sfaces` index; the dotted pattern on the reed at noon. `terrain_lit` takes the
  pixel's footprint, below 0 for "light where you are"; `fs_still` passes -1, a test holds it.
- **Light texels larger than a block (must).** `tx_k` now stops at a block (or a coarse level's painted texel)
  and past that returns 0, the pixel's own point; a GPU test holds the cap.
- **Slopes snapped (must).** Sloped tops keep the pixel's point; `gi_trace` snaps only where the normal is within
  about 8 degrees of a grid axis (no slope, plant, model or effect snaps), and keeps the pixel's seed there.
- **Sun pairs leaned on a direction and flipped at |sd.y| = 0.9 (should).** The pairs lie on the disc's
  diagonals in a frame from the local up (across the sun's path), which turns smoothly over the day. A thin
  occluder between the four points is still missed far away, where its true shadow is faint anyway.
- **Lamp ends repeated (should).** A repeated end takes the earlier ray's answer; at a bounce's hit (`seen`) one
  ray to the sun and one a lamp (`one_ray`), as before this ADR.
- **Bounce texel blotches (should).** Not seen: the bug's save at night with a held lamp and a daytime walk over
  grass show no texel-sized blotches or flicker in the bounce.
- **`cl_cover` in the ADR (should)** fixed above; frame.rs calls `weather::cl_cover`, a test holds it to the shader.
- **Notes.** Derivatives before `fs_cut`'s discard (`fs_still` needs none now); the GPU test skips with a reason
  without Vulkan and covers a cube face's edge; the rain's sound weighs `from_cloud` where drops start
  (`RAIN_UP` + 2.5 m); `tx_seed`'s comment says only the axis goes in (only one face of a plane is drawn).

The developer's grain (bug `01-59-35`, the save with a lamp in hand) was the held lamp's one random shadow ray a
pixel: speckle over the lit dirt wall and logs. With the texel light it is flat texels
(`out/bugs-1114/shadow-bug-*`).

## Consequences
- Shadows read as pixel art: a tree's shadow on grass has stepped edges along texel borders.
- Twice the sun's shadow rays everywhere the sun lights (four in a penumbra), up to 2-4 rays a lamp for the three
  strongest. Logged under Cost; not optimised here (ADR 0096).
- The fixed viewpoints change (shadows and clouds); they were not accepted on this branch.
- The bounce's texel is found from a depth-rebuilt point: a face seen exactly edge-on may pick the wrong axis; the
  bounce is smoothed anyway.
- `seen()` still takes a `px` it no longer uses past the bounce direction.
