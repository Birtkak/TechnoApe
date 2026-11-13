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
  their patch's roll (cells about 12 columns across, ragged), so a kind grows in drifts: stretches of short grass,
  stands of tall grass, patches of wheat and of one flower colour (four plants in five follow their patch). Short grass fills what the tall leaves
  open: open grassland is at least 85 % covered at normal (tested), so no bare ground shows between tufts.
- **No grid:** each plant turns to any angle, stands up to a third of a block off its cell's middle and has its
  own width and height, all from its column's hash in the shader (`still_local`, shared with the BLAS so its
  shadow matches), so no row shows from any side.
- **Shrubs** (four planes, the berry bush; `SHRUB` flag): three leaning crosses of three planes at fixed offsets
  and sizes (`plant_faces`), a lobed clump wider than tall, lopsided by the hash; seen twice as far as the grass
  (up to 120 m, inside the level-0 chunks they are meshed in). Grassland has a berry bush in about forty plants.
- **Vegetation setting** (Graphics: few / normal / many = 0.4 / 1.0 / 1.6 of the density; `vegetation=`), and the
  far distance 48 / 80 / 112 m. Each plant shrinks into the ground over the last fifth of its own reach (55 to
  100 % of the far one, by its hash): the plants thin in a hashed order, no ring, none pops.
- **Far away:** the textures session's `meadow ground` painting (96 x 32, three tiles of 1 m at 32 px) is still
  face 0 (`MEADOW` flag). Beyond half the far distance the grass block's top fades to it, a tile per column by
  hash, climate-tinted (`ground_tint`, `meadow_texel`); the block's four sides take it in a fringe over their
  top 0.15 to 0.45 m, faded the same way, so a step's edge reads as grass hanging over it. From a hill the valley
  reads as meadow with no line where the plants end. Its `.far` plant paintings are not read yet.
- **Sway** in `vs_still`: `sway(wind, height, column, hash)` from the weather's wind at the camera (ADR 0076) on
  the sky's clock; zero in a `shot=`, so fixed views stand still.
- **Tint:** `climate_tint(heat, wet, part, season)` (WGSL) = `clutter::tint` (Rust, tested): heat from the bake's
  air at the column's height (−5 to 35 °C), wetness from its rain (root scale). Deep green where wet, straw where
  dry and hot, the dry end squared so a middling climate stays green; for plants also fresh in spring, yellowing
  from late summer, dull in winter. Only leafy pixels (g > r and g > b) take it, as a gentle multiply, so the
  paintings keep their ramps. The grass block's top takes the same tint without the season (`ground_tint`); each
  plant is also shifted a little warmer or cooler and lighter or darker by its column's hash.
- **Depth:** darker at the root, lit at the tips; the planes of a cross a shade apart by their turn; each plane
  lit by how squarely it faces the sun (`|n . sun|`, 0.8 to 1.08); shadows between neighbours and from each
  plant onto the ground.
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
Measured once at the default setting (ADR 0096), play build, three runs each, medians, under `tools/slot.sh
measure`: `run=fly quick=1` (the forest fly-through) and `run=stand` in a dense grassland meadow at noon. Main's
figures are `docs/perf-debt.md`'s (fly GPU p50 2.9 ms, p99 6.3 ms, frame-thread work 0.58 ms).

| Run | GPU p50 | gbuffer | trace | frame p99 | frame-thread work | VRAM |
|---|---|---|---|---|---|---|
| fly, normal | 3.20 ms | 1.42 | 0.25 | 6.97 ms | 0.75 ms | 1094 MB |
| meadow stand, normal | 4.33 | 2.00 | 0.69 | 5.43 | 0.71 | 1092 |

CPU apart: the frame thread pays about 0.15 ms (more still quads into draws and BLAS instances); the mesh
workers about 47 µs more a level-0 chunk (clutter 80 µs against 33 on main; meshbench in grassland). An earlier
series (before the far distance grew, without and with the plants' shadows) put the shadows at about 0.5 ms GPU
in the meadow and 0.2 ms in the fly-through; every Vegetation level held the 8.3 ms p99 then.

With the crowns (part A, about +0.7 ms fly GPU over main by its own measurement) the two add to about 1 ms GPU
over main in the fly-through; not measured together, as neither branch is on main yet. The first run of a
series had one 15 ms frame (a cold start); the medians are of three. The debt row is in `docs/perf-debt.md`.

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

Third (turn and offset per place, shrubs, the far tone): the drawn plant, its BLAS copy and its shadow reach
agree (checked). (1) The 120 m cap on a shrub's reach clamped most shrubs to one ring: fixed, the cap is taken
before the hash. (2) The far mottle reached the horizon and would crawl as sub-pixel noise: fixed, it fades to its
mean from 150 to 400 m. (3) The far tone darkens every grass top, also where no plants grow (a dug plot, a
player's lawn): answered, a call for the developer (above); the shader cannot tell cheaply. (4) A shrub's
lopsided side and its warm tint came from one hash: fixed, separate hashes.

## Calls for the developer to confirm
1. **Texel 32 px a metre** for plants (blocks stay 16): taken from your verdict on detail; the paintings are the
   textures session's (ADR 0084), merged here.
2. **Shadows as traced alpha-tested geometry** for every plant and clutter model, on sun, moon and lamp rays;
   they do not sway with the plant. The alternative was a cheaper contact shadow, not built.
3. **Density:** open meadow 85 % or more covered at normal, drifts of one kind (tall grass sweeps, flower and
   wheat patches). Vegetation few / normal / many scales it and the distance (48 / 80 / 112 m).
4. **Tint:** deep green where wet, straw only where really dry (squared), the grass top and plants alike; a
   middling climate stays green.
5. **Far meadow ground:** the grass top and a fringe on its sides take the painted meadow ground where the plants
   have thinned, instead of a far plant layer (one plane per column, the `.far` paintings), which was not built.
6. **Shrub = the berry bush** drawn as three lopsided clumps; the trees session's leaf-block shrubs take the
   crowns' fluff. Whether the two read as one kind of thing waits for both to be on main.

## Not done (asked, left for a later round)
- Far plane cards from the `.far` paintings, and keeping far planes longer on a step's edge (asked by the
  overseer's fourth review): the meadow ground's fringe stands in.
- From 40 m straight above, the grass tops within half the plant distance are the plain grass block between the
  tufts; the meadow ground only starts beyond it.
- A meadow's edge fraying into scattered tufts on a biome border: plants follow the biome's ground blocks.

## Consequences
- The fixed views change wherever grass grows: every view with grassland, savanna, forest floor or wetland in
  sight gains plants and their shadows, and the grass top takes the climate tint (`tools/views.sh accept` after
  the overseer's gate).
- Paintings: `data/plants/*.png` are placeholders (`tools/paint_plants.py`, on `data/drafts.txt`); the textures
  session's 32 px paintings replace them after the merge (`tools/paint_textures.py`), with their palette colours.
- Shadows do not sway (the BLAS is built once per mesh); the drawn plant moves a few cm in wind over a still
  shadow. Acceptable at the sway's size; revisit if the developer sees it.
- A plant's shadow reaches only `still_far()` from the camera, like the plants themselves.
