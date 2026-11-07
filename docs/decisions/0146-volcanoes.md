# ADR 0146: Volcanoes

Status: accepted (2026-11-07). Generator only (`world_gen.cpp` `volcano_height` and column_base, `fine_gen.cpp`
macro_point and the water fill). No new materials. GEN_VERSION 37.

## Context
The developer: "Add volcanoes." The volcanic biome (ADR 0101) was a climate band on ordinary mountains: basalt
ground and spires, but nothing you would call a volcano. The game has still lava (ADR 0069 C) and a monolith
landmark placed per hash cell (ADR 0051), so a volcano is one more landmark of that kind.

## Decision
- **A cone landmark**, one chance in four per 4000-macro-voxel cell (2 km): radius 120-180 m, rim 75-115 m over
  the sea, sides steepening toward the top (`((1 - d))^1.6`), a wobbly foot and rain grooves from the detail
  noise. The centre must be open land (region noise ≤ 0, continentalness ≥ 0.08), like a monolith.
- **The cone takes over** where it stands higher than the land (`above_f = cone`), so it rises from plains or a
  hill country alike; in the crater its own floor always wins.
- **A crater** a sixth of the radius wide and a quarter of the height deep, a bowl with a flat floor; still lava
  fills it to 45 % of the depth. `Column.lava` carries the lava surface; `FineGen::macro_point` treats it as a
  lake bed with that water level and the fill writes `LAVA_STATIC`. Caves stop 20 m under the crater.
- The whole cone is `col.rock = BASALT` (bare basalt, top and sides, no bed sediment under the lava) and is
  classified `VOLCANIC`, so dead scrub, fire lilies and basalt cairns follow from biomes.json.
- `find_nearest("volcano")` and `/goto volcano`; `column_info` reports `lava`.

## Consequences
- Worlds made before open empty (GEN_VERSION 37).
- No snow on a cone (the basalt rule runs before the snow rule); no eruptions, no lava flows down the flank.
- A cone can stand next to a river or lake: hydrology does not know about it (a river may be dammed by the
  cone's foot and pool there). Upgrade when it shows in play.
