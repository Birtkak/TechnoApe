# Biomes

- **Date:** 2026-10-22
- **Data:** `game/data/biomes.json` (source of truth: biome list, climate
  tunables, placement tables, neighbour lists).
- **Logic:** `game/scripts/world/biome_registry.gd` (load + validate),
  `biome_rules.gd` (reference classifier, latitude, 3D layer, adjacency),
  `biome_codegen.gd` → `prototypes/p5-sim-spike/src/biome_table.gen.h`
  (the same tables and an inline `classify()` / `classify_3d()` for the
  native generator; regenerate with `tools/biome_codegen.gd`).
- **Tests:** `game/test/test_biomes.gd` (23, headless). **Probe:**
  `game/tools/biome_probe.gd` (north–south transect, agreement with the
  native generator, observed border pairs, cave mix per depth, map PNG).
- Terrain shape, hydrology, caves-as-holes: `world-generation.md`. This doc
  only decides *which biome* a column (and, underground, a voxel) belongs to.

## Climate model

Two surface noises (temperature, humidity, ~2.5 km period, jittered ±0.09
by a 50 m noise so borders dither) plus three deterministic terms:

| term | formula | effect |
|---|---|---|
| latitude | `−0.8 · clamp(−z / 32000, −1, 1)` | **north (−Z) is colder.** 16 km north of spawn the latitude term alone is −0.8: everything is frozen or cold. 16 km south: hot. Spawn (z = 0) is temperate with cold and warm pockets from the noise. |
| noise | `0.5 · temp_noise` | local variation: ±0.5, so within 8 km of spawn every band occurs somewhere, but beyond 16 km the noise can no longer undo the latitude band |
| lapse | `−0.0015 / voxel above sea + 70` | highlands and mountains are colder: the snow line and tree line drop, frozen highlands appear on cold hills |

`temperature = clamp(noise + latitude + lapse, −1, 1)`. Five temperature bands
(frozen < −0.6 < cold < −0.2 < temperate < 0.2 < warm < 0.55 < hot) and five
humidity bands (arid < −0.5 < dry < −0.15 < neutral < 0.15 < humid < 0.5 < wet).

Altitude bands (voxels above sea 256): deep ocean (continentalness < −0.55),
ocean (< −1), coast (≤ 3), lowland (≤ 70), highland (≤ 170), mountain, alpine
(above the tree line, 220 − 40·cold), snow (above the snow line, 330 − 90·cold).
Constraint checked by the registry: the coldest tree line (180) stays above
the highland limit, so alpine never touches highlands without mountains between.

## Surface biomes (27)

Lowland table, rows = temperature, columns = humidity:

| | arid | dry | neutral | humid | wet |
|---|---|---|---|---|---|
| frozen | ice_fields | snowy_tundra | snowy_tundra | snowy_taiga | snowy_taiga |
| cold | tundra | tundra | taiga | taiga | taiga |
| temperate | shrubland | plains | plains | forest | old_growth_forest* |
| warm | savanna | savanna | plains | forest | jungle* |
| hot | desert | desert | savanna | jungle | jungle |

\* **swamp** overrides temperate/warm + wet when the ground is low (≤ 24 above
sea) and flat (erosion ≥ 0.05).

Highland table: frozen → frozen_highlands; cold/temperate/warm → highlands;
hot + arid/dry → badlands, else highlands. Mountain band → mountains, alpine →
alpine, snow → snowy_peaks. Coast → beach / snowy_beach (frozen) /
stony_shore (slope ≥ 0.6). Ocean → ocean / frozen_ocean (frozen); deep ocean
→ deep_ocean. River and lake overlay everything (hydrology pass).

Each biome carries: `surface` {top, under, optional steep/bank/pool/ice}
material names, `tree_density`, `tree_kind` (oak / spruce / acacia / jungle,
for the tree stamper later), `color` (maps), `tags`, `borders`, `notes`.

## Underground layer (9 cave biomes) — the 3D map

`depth = surface height − y`. The top 6 voxels (soil) keep the surface biome.
Below that the cave biome comes from `[depth band][surface humidity]` with
overrides on surface temperature and a slow 3D cave noise (120 m period):

| depth | arid | dry | neutral | humid | wet |
|---|---|---|---|---|---|
| shallow 6–60 | dripstone_cave | dripstone_cave | cave | lush_cave | lush_cave |
| mid 60–160 | dripstone_cave | cave | cave | lush_cave | fungal_cave |
| deep > 160 | deep_cave | deep_cave | deep_cave | deep_cave | deep_cave |

Overrides in order: shallow + frozen surface → **frozen_cave**; deep + hot
surface → **magma_cave**; deep + cave noise < −0.35 → **deep_dark**; deep +
cave noise > 0.45 → **crystal_cave** (the spell-material gate: gem ores and
mana crystals live only there).

So Brent's example works as asked: a mountain, humid forest at its foot, and
straight down from the foot: forest (soil) → lush_cave (shallow and mid) →
deep_cave, or deep_dark / crystal_cave where the cave noise says so. Under a
desert the same column gives dripstone_cave → deep_cave / magma_cave.

The native side gets `classify_3d(surface_biome, t, h, depth, cave_noise)` in
the generated header; it is a handful of band lookups, cheap enough to call per
voxel inside the existing cave-carving loop (it only matters where the carve
leaves air, for the floor/ceiling materials and later decorations).

## Adjacency: who borders whom

Derived, not hand-authored (the `borders` lists in the data are the human
summary and a test checks they match). Every biome occupies cells in climate
space (surface: temperature × humidity × altitude band; caves: temperature ×
humidity × depth × cave-noise band). The noises are continuous, so two
biomes can only touch when their cells are at most one band apart in every
axis:

- **common** border: exactly one axis differs by one band (plains ↔ forest,
  highlands ↔ mountains, cave ↔ lush_cave, a surface biome ↔ the shallow
  cave beneath it).
- **rare** border: two or three axes change at once (desert ↔ plains at a
  corner), a dithered speck rather than a frontier.
- impossible: anything further apart. Desert never touches a frozen biome,
  snowy peaks only meet alpine, deep dark never meets the surface.
- Rivers border everything but the deep ocean (springs sit above the tree
  line too); lakes everything but the sea; both cut into shallow caves only.

Queries: `BiomeRules.neighbors(name)` → {common, rare}, `can_border(a, b)`,
`colder_than(name)` (what you reach walking north at the same altitude),
`below(name)` (cave biomes that can lie under a surface biome).

## Measured (seed 1337, `biome_probe.gd`, native noises + reference rules)

Transect from 48 km south to 48 km north, land share of a 4 km band
(ocean/beach/water left out):

| band | top land biomes |
|---|---|
| +48 … +16 km | desert 10–12 %, jungle 10–13 %, savanna 9–10 %, highlands, badlands |
| +12 … +4 km | savanna 12 %, highlands 9 %, forest 6–8 %, jungle, plains |
| +4 … −4 km (spawn) | plains 12–15 %, highlands 7–9 %, forest 5–6 %, taiga 3–8 %, tundra, shrubland, swamp, old growth |
| −4 … −12 km | taiga 12–13 %, tundra 8 %, snowy taiga, snowy tundra, frozen highlands |
| −16 … −48 km | snowy tundra 11–15 %, snowy taiga 9–13 %, frozen highlands 8–11 %, ice fields, alpine; frozen ocean 25–29 % of all samples |

Cave mix over the same 2 km (sampled 10 voxels into each depth band):
shallow dripstone 39 % / lush 36 % / plain cave 24 % / frozen 2 %; mid plain
49 % / lush 22 % / dripstone 15 % / fungal 14 %; deep plain 90 % / deep dark
8 % / crystal 2 %.

Within 2 km of spawn the rules agree with the current native if-chain on
91 % of columns; the differences are the new biomes (old growth 9 %,
shrubland 7 %, jungle, taiga, tundra, swamp, savanna) and the retuned band
edges. Observed border pairs: 71, none the rules forbid except 7 samples of
alpine next to highlands where the 8 m sample grid steps over a cliff.

## Hand-off to the native generator - done 2026-10-22 (GEN_VERSION 6)

Integrated in `world_gen.cpp`: `column_base()` computes the climate with
`technosim_biome::temperature()` and classifies with `classify()` (river and
lake are overlaid afterwards by the hydrology pass, as before); surface
materials come from `SURFACE_TOP/UNDER/STEEP`, tree density from
`TREE_DENSITY` (thinned over the last 60 voxels below `tree_line(t)`), map
colours from `COLORS`, names from `NAMES`. Cave floors use `cave_biome()`
from the column's bands plus a 120 m 3D noise: lush caves get grass floors,
fungal caves dirt, frozen caves snow; the top 6 voxels keep the surface
biome. The generated cave table is `CAVE_TABLE` (the enum already has
`Id::CAVE`). Jitter: altitude bands see the height jittered by +-12 voxels,
the coast and the sea the real one. Original steps, kept for reference:


1. `#include "biome_table.gen.h"` and replace the if-chain in
   `column_base()` with `technosim_biome::classify(t, h, above, c, e, slope,
   river, lake)`; `t` must come from `technosim_biome::temperature(noise, z,
   above)` so the latitude and lapse apply. `WorldGen::Biome` can alias the
   generated `Id` (ids 0..12 are unchanged, `BIOME_COUNT` becomes 36).
2. Surface materials from `SURFACE_TOP/UNDER/STEEP[biome]` instead of the
   switch; `TREE_DENSITY[biome]` for the tree stamper; `COLORS` for `map_image`.
3. In the cave carve, `classify_3d(surface, t, h, depth, cave_noise)` for
   floor/ceiling materials (cave noise: one more FastNoiseLite, period 120 m).
4. `biome_name()` → `NAMES`. The gdUnit test
   `test_generated_header_is_up_to_date` fails whenever the JSON and the
   header drift.

## Open (taste calls for Brent)

- ~~Latitude scale~~: **16 km to fully frozen/hot, decided by Brent 2026-10-22**
  (each climate zone should feel like a country, not a short walk).
- Whether spawn should be forced temperate (today the latitude term is 0 at
  z = 0, the noise still allows cold or warm pockets there).
- Extra biomes are one row in the JSON each: mangrove (hot + wet + coast),
  volcanic (hot + mountain), cherry grove, mushroom island; underground:
  a flooded cave (humid + lake above), an ore-rich "deep veins" band.
