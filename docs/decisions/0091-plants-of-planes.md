# 0091 – Plants of crossed painted planes: dense meadows, sway, climate tint, shadows

- **Status:** accepted (flora part B; the overseer's brief and reviews, the developer's taste calls of 2026-11-13)
- **Date:** 2026-11-13

## Context
ADR 0089 set the direction: small plants as crossed flat planes, dense meadows, wind sway, colour by climate,
flowering and fruit, with Vintage Story as the reference. The grass between trees was bare; the few small plants
were 1/8 m voxel models (ADR 0082). The developer then asked for more detail and colour ("still too low quality of
details and colours") and for plants that take and cast light ("should also interact with light so they cast
shadows and feel like they got depth").

## Options considered
- **Shape:** voxel models (kept for pick-ups) / two to four crossed painted planes (chosen). Planes upright or
  leaning: upright planes vanish seen from above, so from 20 m up the meadow was not there. Chosen: each plane
  leans `PLANT_LEAN` (0.26 rad, 15°) to its own side by turns, and tall grass has three planes; a ground colour
  under the plants (the overseer's second idea) was not needed once the planes lean and cast shadows.
- **Texel:** a block's 1/16 m (tried; too coarse for a blade) / **32 px a metre** (the developer's call), a
  column's painting 32 px wide so neighbours merge. Sizes: short grass 32x13, tall grass 32x32, wild wheat 32x40,
  fern 48x22, reeds 32x64, flowers 32x16, berry bush 40x40; variants `.1`, `.2` by the place's hash; the bush's
  `.bloom` (late spring) and `.fruit` (late summer).
- **Tint:** bits per quad (tried: 11 bits, a per-quad climate made seams between columns) / **a chunk's climate at
  its four footprint corners** (`Chunk.clim`, engine `clutter::chunk_climate`), mixed bilinearly in the shader, so
  plants and the grass block's top take one smooth tint and neighbouring chunks share corners.
- **Shadows, in the order the overseer asked:** (1) plants in the traced BLAS as alpha-tested geometry; (2) the
  larger plants only plus a contact patch for the small; (3) a screen-space contact shadow. Option 1 was tried
  first and holds the budget (Costs below), so 2 and 3 were not built.

## Decision
- **Placement:** a plant is a clutter row of kind `plant` (`data/clutter.toml`, `planes = 2..4`, `looks`), placed
  by the same per-column roll as ADR 0082 with no storage, from a biome's new `undergrowth` list in
  `data/biomes.toml` (models stay in `clutter`). Plants roll on their own (`PLANT_ROLL`), and three in four take
  their patch's roll (cells about 7 columns across, ragged), so a kind grows in drifts: tall grass in sweeps,
  flowers and wheat in patches. Open grassland is 60–80 % covered at normal (tested).
- **Vegetation setting** (Graphics: few / normal / many = 0.4 / 1.0 / 1.6 of the density; `vegetation=`), and the
  far distance 40 / 64 / 96 m. Each plant shrinks into the ground over the last fifth of its own reach (55 to
  100 % of the far one, by its hash): the plants thin, none pops.
- **Sway** in `vs_still`: `sway(wind, height, column, hash)` from the weather's wind at the camera (ADR 0076) on
  the sky's clock; zero in a `shot=`, so fixed views stand still.
- **Tint:** `climate_tint(heat, wet, part, season)` (WGSL) = `clutter::tint` (Rust, tested): heat from the bake's
  air at the column's height (−5 to 35 °C), wetness from its rain (root scale). Deep green where wet, straw where
  dry and hot, the dry end squared so a middling climate stays green; for plants also fresh in spring, yellowing
  from late summer, dull in winter. Only leafy pixels (g > r and g > b) take it, as a gentle multiply, so the
  paintings keep their ramps. The grass block's top takes the same tint without the season (`ground_tint`); each
  plant is also shifted a little warmer or cooler and lighter or darker by its column's hash.
- **Depth:** darker at the root, lit at the tips; the planes of a cross a shade apart by their turn; shadows
  between neighbours and from each plant onto the ground.
- **Shadows:** the still quads (plants and voxel clutter models alike) join the BLAS's second, non-opaque
  geometry. `expand` places them with the same `still_local` as `vs_still` (unswayed, unfaded). In `trace` a
  still candidate is confirmed only for shadow and lamp rays (first hit), more than 4 cm off the ray's start (a
  blade does not shade itself), within `still_far()` of the camera (where plants are drawn), and where its
  painting is opaque; rays that look (bounce, reflections) pass through plants, so `seen` never meets one. The
  sun's, the moon's and the lamps' rays all take them. `plant_shadows=0` leaves them out for measuring.

### For part A (crowns) and later users
`sway`, `Globals.plants` (wind xyz, far w), `chunk_climate(chunks[ci], local.xy / 32)`, `year_part(season(..))`,
`climate_tint` and `leafy` are built for leaves to reuse: a leaf texel multiplied by
`climate_tint(cl.x, cl.y, year_part(sn), sn.y)` matches the ground and the plants under it.

## Costs
Play build, dev PC, three runs each, medians; `run=fly quick=1` (the forest fly-through from the spawn) and
`run=stand` in a dense grassland meadow at noon of southern late summer. Main is `build/TechnoApe.exe` before
this branch.

| Run | GPU p50 | of it: gbuffer | trace (shadows, bounce) | frame p99 | VRAM |
|---|---|---|---|---|---|
| main, fly | 2.96 ms | 1.02 | 0.42 | 6.36 ms | 1084 MB |
| few, fly | 3.16 | 1.37 | 0.23 | 6.95 | 1094 |
| **normal, fly** | **3.24** | 1.42 | 0.25 | **7.13** | 1094 |
| normal without plant shadows, fly | 3.07 | 1.37 | 0.19 | 6.65 | 1094 |
| many, fly | 3.32 | 1.51 | 0.28 | 7.57 | 1094 |
| main, meadow | 2.96 | 0.86 | 0.46 | 4.15 | 828 |
| few, meadow | 3.38 | 1.40 | 0.34 | 4.48 | 838 |
| **normal, meadow** | **3.88** | 1.65 | 0.43 | **5.11** | 836 |
| normal without plant shadows, meadow | 3.38 | 1.51 | 0.22 | 4.51 | 836 |
| many, meadow | 4.56 | 2.07 | 0.82 | 5.65 | 1092 |

Every setting holds the 8.3 ms p99 budget; normal's worst is the fly-through at 7.13 ms (main 6.36). The shadows
cost about 0.2 ms of trace and 0.5 ms GPU in the dense meadow (0.2 ms in the fly-through); `few` is the setting
that saves most. Some first runs had one 21–27 ms frame (few, normal and many alike, in the first of the three
reps; main's first fly rep 7.8 ms), so `budget_ok` is false there; the medians are of three. Main's trace in the
fly-through (0.42 ms against 0.25 with plants) is unexplained and not chased. The series ran back to back under
`out/.lock`. Numbers before the review's fixes, which only remove work (fewer shadow candidates
confirmed).

CPU (separate, ADR "CPU over GPU"): the frame thread's `work_ms` and the mesh job per chunk.

| | main | few | normal | many |
|---|---|---|---|---|
| frame thread `work_ms`, fly | 0.63 | 0.83 | 0.81 | 0.81 |
| frame thread `work_ms`, meadow | 0.65 | 0.69 | 0.73 | 0.73 |
| clutter per chunk (meshbench, grassland) | 33 µs, 569 quads | 45 µs, 556 | 80 µs, 889 | 112 µs, 1139 |
| mesh per chunk | 74 µs | 59 | 59 | 63 |

The frame thread pays about 0.1–0.2 ms (more still quads to sort into draws and BLAS instances); the mesh workers
pay about 47 µs a level-0 chunk at normal for the plants' rolls (two rolls a column, plus the patch roll), off
the frame thread. Mesh time itself is unchanged (main's 74 µs is a different build's noise).

## Review
Two fresh reviews (subagents given the diff and the task).

First (the planes, placement, sway, tint): plants shared the models' roll (fixed: `PLANT_ROLL`); the taken list
was scanned for every thing (now pick-ups only); the climate took the sea's air (now the column's height); tall
plants against the chunk's culling bounds (answered with a shortcut comment); no painting size check (added: at most 2 m wide, 2.5 m high); berry bush
in winter (now spring to autumn); a comment mismatch (fixed); `part` per column (kept: cheap); sway and the
temporal reuse (answered: no ghosting seen in the storm film); quads past the fade still drawn (answered: the
measured cost holds the budget; a cull by distance can come later).

Second (32 px, leaning planes, shadows): (1) a plant's full-size BLAS copy cast shadows after the drawn plant had
faded, out to the far distance: fixed, `still_reach` stops a plant's shadow where its own shrink begins. (2) A
swaying blade's shadow ray crossed its unswayed BLAS copy and flickered: fixed, a plant's own pixels skip still
hits within 35 cm (`still_skip`), its root darkening stands in for that close shade. (3) Grazing sun on a leaning
plane: covered by the same 35 cm skip. (4) Voxel models' shadows stopped at the plant distance: fixed, models
cast as far as they are drawn. (5) An argument looked up per chunk: hoisted.

## Consequences
- The fixed views change wherever grass grows: every view with grassland, savanna, forest floor or wetland in
  sight gains plants and their shadows, and the grass top takes the climate tint (`tools/views.sh accept` after
  the overseer's gate).
- Paintings: `data/plants/*.png` are placeholders (`tools/paint_plants.py`, on `data/drafts.txt`); the textures
  session's 32 px paintings replace them after the merge (`tools/paint_textures.py`), with their palette colours.
- Shadows do not sway (the BLAS is built once per mesh); the drawn plant moves a few cm in wind over a still
  shadow. Acceptable at the sway's size; revisit if the developer sees it.
- A plant's shadow reaches only `still_far()` from the camera, like the plants themselves.
