# 0101 – Shorelines, four new biomes, bunched ore

- **Status:** accepted (built 2026-10-28, GEN_VERSION 26)
- **Date:** 2026-10-28

## Context
Asked for 2026-10-27/28: "fix beaches and add more biomes", "make ocean floor and the way land meets sea more
realistic", "make ores bunch up more so they are easier to mine". The developer picked the biomes: birch forest +
flower meadow, volcanic peaks, mangrove (not the cherry grove).

Beaches were picked by height alone (the coast band, 0..3 macro voxels = 1.5 m above the sea). Continentalness has
an ~8 km period, so its spline left 100-200 m of land within 1.5 m of the sea: all of it sand, grass specks dithered
through it where the detail noise poked above the band, inland flats far from any water too, and as wide a plate of
sea under 1 m of water. `/goto beach` near spawn went 400 m - 3.7 km.

## Decision
- **Shore profile** (`WorldGen::column_base`): where -0.35 < c < 0.3, `Column::shore` estimates the signed
  distance to the contour where the spline crosses sea level, from the local gradient of c (two extra noise reads).
  On land the base follows a beach up to a berm (1.75 m at 18 m in), then rises 0.5 m per 25 m, blending into the
  spline by 350 m. At sea: a foreshore to ~3 m deep at 35 m out, a sandbar ~22 m out, then a shelf sloping down
  1 m per 25 m until the spline's continental slope is deeper. The detail noise fades near the waterline (no
  puddles on the beach, no sand specks offshore).
- **Beach = a strip by the sea**: `classify(..., shore)` takes the coast band only within 15-26 m of the coastline
  (jittered), up to 3 m above the sea (`coast_max` 6); low ground further in is lowland. Mirrored in
  `BiomeRules.alt_band` (`col.shore`, default true).
- **Sea floor** (`WorldGen::seabed`, near and far meshes): sand on the foreshore and in the shallows, sand with mud
  (clay) and gravel patches on the shelf, mud in the deep, rock on steep slopes. Kelp still needs sand or gravel.
- **Band overrides** (biomes.json `band_overrides`, codegen `BAND_OVERRIDES`, rules `classify` and `cells_of`):
  coast after the stony-shore slope check, mountain, alpine.
- **New biomes** (ids 37..40, append only):
  - `birch_forest`: cold + humid lowland (was taiga), birches with a few spruces.
  - `flower_meadow`: warm + neutral lowland (was plains), flowers in every patch, a lone oak or birch.
  - `volcanic`: warm/hot + arid..neutral, mountain and alpine bands; basalt ground and cliffs, obsidian on the
    steeper slopes, basalt spires, dead scrub, no alpine grass or scree. Snow still caps the highest peaks.
  - `mangrove`: warm/hot + humid/wet coast; mud over clay, reeds and ferns, jungle trees right at the water
    (trees may stand at sea + 1 there).
  Mobs, clutter, ruins and discovery texts list them.
- **Wrecks** stand only where their whole footprint is dry beach, and have stem and stern posts (the planks past
  the last rib hung free).
- **`/goto` for strip biomes** (beach, snowy beach, stony shore, mangrove) checks a patch 8 voxels out, not 24.
- **Ore** (`fine_gen.cpp`): lumps 7 voxels apart (was 5), pockets ~4 m across (were ~3 m), strays between pockets
  0.03 (were 0.2), cave-wall stringer clumps 6 voxels (were 3.5). `tools/ore_mass_probe.gd`, eight veins sampled
  10 m from their centres: 1.76 M ore voxels, 59 % in 1 m cubes at least 40 % ore before; 1.86 M (+6 %), 75 % after.

## Consequences
- GEN_VERSION 26: worlds made before open empty (saves are keyed by version).
- Coasts are mostly grass or forest to within ~20 m of the water; beaches are narrower and everywhere along the sea.
- Shore estimate is linear: where c curves sharply (headlands, narrow inlets) the beach width wanders.
- `test_far_mesh` placed-cube check allows the tile's 4-voxel rounding (it only passed while the ground height there
  was a multiple of 4).
