# ADR 0134: Walkable cave halls on the storeys, with a rubble talus and rare giant chambers

Status: accepted (2026-11-03), GEN_VERSION 35. Replaces the caverns of ADR 0051 / 0077; keeps the galleries of
ADR 0080 and the cave pools of ADR 0109.

## Context
Bug report 2026-11-03_16-37-55 ("underground dont load", seed 1932036364, voxel 974 692 -1191): the developer
fell into a dark hall about 40 m tall. They added "caves are ugly and not usable", "we need realism", "we need
immersion". `cavern_value` carved one smooth 40-60 m void wherever a 2D noise passed a threshold, at a fixed
depth. Walls stood vertical to the floor. Tunnels, galleries and shafts crossed the void wherever their own noise
said, so most of them ended in a wall high above the floor. Deep cheese rooms stacked into spaces over 50 m tall,
and steep spaghetti tunnels made long vertical drops. Nothing on the floor said "you can walk here", and the
only glow near the floor was what the biome dressing happened to place.

## Decision
- **Halls live on the gallery storeys** (every 14 m, `GAL_SP` / `GAL_HH`), so a hall's floor is a gallery floor
  and the passages of that storey enter it at floor level. A footprint is where `n_hall_` (1/110) passes
  `HALL_THR`. `foot_dist` turns the excess over the noise gradient into a distance from the wall, so the shape is
  the same whatever the noise does: a roof rising over 5 m to 12 m, a talus 3 m high at the wall falling to a
  flat floor over 5 m.
- **Giant chambers stay as a rare landmark**: a 1/2400 mask over the old `n_cavern_` footprint, 22 m tall with a
  25 m talus, pillars kept as solid rock, only on even storeys, and no regular hall above, below or beside one.
  `find_nearest("cavern")` now finds these.
- **Passages join at the floor**: inside a hall's wall band, tunnel air above the talus line, under the floor and
  in the roof slab is cut. Spaghetti tunnels pinch shut where they run steeper than about 55 degrees. Deep cheese
  rooms use a fixed threshold and a flatter squash. Mouth shafts get a spiral ledge round the wall.
- **Floor and dressing**: stone just under hall air on the talus becomes gravel or cobble rubble. Cave pools never
  sit on a talus and take their liquid from the biome at the floor. Near the wall foot, floors get glow mushroom
  and moss patches; these are emissive tiles and glow-worm anchors, no new lights.
- Materials and atlas tiles: none added.

## Measured
`tools/hall_probe.gd`, seed 1932036364, 160 m round (974, -1191); "drops" are falls from a walkable floor.

| | before | after |
|---|---|---|
| drops of 4 m or more | 12292 | 2939 |
| drops of 10 m or more | 4607 | 357 |
| drops of 20 m or more | 926 | 84 (to the lava line or down shafts) |
| worst drop | 56.9 m | 32.4 m (onto the lava sea floor) |
| steps onto a hall floor: dropping 10 m or more | 565 of 1549 | 6 of 248 |
| worst drop into a hall | 56.9 m | 12.5 m |
| down shafts: 10 m or more | 1059 | 217 |
| hall across p50 / p90 / max | 9.9 / 24.7 / 256 m | 12.8 / 28.5 / 57.6 m |
| hall height p50 / p90 / max | 8.5 / 16.5 / 25 m, tallest 60.9 m | 9.5 / 12 / 22 m, tallest 27.5 m |
| hall floor that is rubble | 0 | 18 % |
| floor squares with glow | 0 | 6 % (34 glow voxels per 100 m²) |

The bug spot itself (48 m round): one 77.7 m hall 40.6 m tall with a 37.4 m drop before; after, halls of 11.8 / 22.4 / 34.3 m across (p50 / p90 / max), all 12 m tall, no drop of 20 m, no step into a hall deeper than 3 m, glow on 12 % of floor squares.
Seed 1337 round the origin, same probe: drops of 10 m or more 1830 before, 40 after; of 20 m or more 384 before, 0 after; worst into a hall 36.5 m before, 6 m after; largest open space 125 m before, 58 m after; glow 0 before, 5 % of floor squares after.

Generation cost (`tools/fine_gen_bench.gd seed=1337`, average ms per block, two interleaved rounds):

| | before | after |
|---|---|---|
| LOD 0 surface | 0.19 | 0.22 |
| LOD 2 surface | 0.55 | 0.69 |
| LOD 0 deep | 0.27 | 0.30 |
| LOD 1 deep | 0.34 | 0.42 |
| LOD 2 deep | 0.73 | 1.13 |

The hall column is evaluated per macro column at LOD 0-2; the other LODs are unchanged.

## Consequences
- Saved worlds keep their chunks; new chunks generate with the new caves (GEN_VERSION 35).
- Shafts are still open pits in the middle; the ledge only runs round the wall. The deep rift storey (ADR 0077)
  is unchanged, and drops into it remain the largest left (to the lava line).
- Roof moss in lush regular halls stays heavy, as it was before.
- Check with `tools/hall_probe.gd` and `test_landforms_caves.gd` (`test_halls_have_a_talus_and_a_roof`).
