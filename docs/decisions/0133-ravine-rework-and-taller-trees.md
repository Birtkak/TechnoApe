# ADR 0133: Winding, tiered, bedded ravines that end short of rivers; sloped river beds; taller trees

Status: accepted (2026-11-03), GEN_VERSION 34. Amends the ravines of ADR 0052 / 0063.

## Context
Bug reports 2026-11-03 15-42-37 and 15-42-46 ("generation issues", seed 314308392), confirmed by the developer:
"Ravines are very ugly indeed". A ravine followed the zero line of a 1/900 noise, so it ran dead straight for
hundreds of metres. Its walls flared only at the rim, so below that each wall was one smooth face about 25 m
high, streaked by the per-column mottle. Nothing kept a ravine away from rivers: at the report spot one ran
along a river's bank and the river's cave pools stepped down its floor. The developer chose "rework them" over
"rarer and gentler" or "remove them", and in the same session asked for trees "a bit taller".

## Decision
- **One way in:** `WorldGen::ravine_col(x, z)` gives a column's ravine (mask, signed line, two wobbles);
  `ravine_value(RavineCol, depth)` is the carve. The cave field, `cave_roof`, `open_depth`, `ravine_near`,
  `find_nearest("ravine")` and the rope bridges all read through it, so the carve, the rims, the opening and
  the bridges agree.
- **Winding:** the line noise is read at a domain-warped spot: ~50 m sways over ~200 m (`n_galoff_`) and ~10 m
  wiggles over ~45 m (`n_meander_`), at offset coordinates so they are their own warps (`ravine_line`).
- **Tiers:** the depth is cut into 5 m tiers. A tier keeps its width for the upper 55 % (a steep face), then
  narrows (a ledge). The tier levels tilt with the wobble, so ledges slope gently along the gorge. Each tier's
  face wobbles by ±20 % of the half-width on its own phase (`wa cos(2.4 t) + wb sin(2.4 t)`), so a wall is
  stacked rock faces instead of one sheet.
- **Rivers:** `stream_near` is 1 within ~2 hydrology cells (32 m) of a stream (drainage past `STREAM_ACC`) or a
  lake. It is blended between cell centres from the seam-blended accumulation, so it has no tile seams. The
  ravine mask drops by 0.45 × that value, so a gorge closes before it reaches a river.
- **Bedding:** where any lattice corner of a column is open to the sky (`open_depth > 0`: ravines, sinkholes,
  ramps) the stone shows bedding: bands 1.5 m tall, shifted per column by a ~20 m wobble of up to 1.25 m, 35 %
  of them the layer's partner rock (`rock_types.json` `bedding.partner`: limestone/chalk, granite/diorite, ...;
  `RockTypes::bedded`). A band is hashed by its index and rock only, so it runs on unbroken across columns.
  Elsewhere the layers stay whole (mining and ore bias unchanged).
- **River beds** (report 15-42-37): the "water wall" was no wall. The generated water stands at most a voxel
  over its bank (`water_surface_probe`: no terraces, 6 glass edges of 1-4 voxels in 32 m). What looked like a wall
  was the flat-bottomed channel: a 0.5 m sandy shelf tinted light, then a ~1 m drop within 0.4 m to the dark
  channel. The bed now keeps full depth over the inner 20 % of the half width (was 45 %) and slopes up to the
  bank over the rest. It is still 3/4 deep at 45 %, so a hull fits off the centre line.
- **Trees:** `tree_at_cell` and the young trees scale `trunk_h` by 5/4 (stunted trees at the tree line stay as
  they were). Everything reads the height from `trunk_h`, so crowns, the far canopy and block tops follow.

## Consequences
- GEN_VERSION 34: old worlds are offered a regeneration (title screen), and the old saves stay on disk.
- The early-outs on the raw mask (`ravine_near`, `big_cave_possible`, the bridge cell) stay conservative,
  because the river fade only lowers the mask.
- Ravine columns cost a hydrology lookup (36 accumulation reads) and four extra noise reads. This only happens
  where the raw mask is open (> 0.25).
- Rivers are slightly narrower at the waterline: part of the old shelf now stands dry as a sand and gravel bank.
- Tests: `test_mountains_carry_cairns` looks at the nearest cairn instead of a fixed square, and the bridge half
  of `test_find_nearest_finds_bridges_and_cairns` counts deck voxels with air under them (`height_at` is the ground
  before the carve, and where a winding ravine crosses a rise that ground stands over the deck).
