# 0042 – Terra's climate from the air grid, and fifteen biomes

- **Status:** accepted (the developer's calls and the prototype); each biome's look is still open
- **Date:** 2026-11-11

## Context
The developer wants biomes on Terra, and the world generation improved as far as they need. W1 left the
bake of the land (stages 1 to 4) in `prototypes/world-bake`; the game still uses the old noise terrain.
There was no air grid (ADR 0010), no climate and no biome. `worldgen.md` lists stages 5 to 8 (climate,
water on land, soil, life) and says the climate must be the live weather's own model.

## The developer's calls (2026-11-11)
1. **Order:** biome maps first, in the prototype, shown as sheets to pick from; then the bake with its
   biomes moves into the engine (W2 with W3).
2. **Climate:** the air grid first, as the design says: one model for the bake and the later live weather,
   not a separate simple climate.
3. **Biome list:** the design's full list. Land: ice, tundra, cold forest, temperate forest, grassland,
   dry scrub, desert, savanna, rain forest, wetland, alpine, beach. Sea: shelf, deep, warm reef. Each shown
   on a look sheet before it is accepted.
4. **Content:** per biome its ground (surface blocks such as sand, snow, ice, gravel, clay, grass tints)
   and still plants generated as plain blocks (trees, shrubs, cacti). They do not grow; living plants stay
   W7.
- Not chosen: seasons and axial tilt. The air grid does not need a year (its day repeats), so they stay out.

## Options considered (routine technical calls)
1. **The air grid as designed, run to a steady day in the bake** (chosen). One model, so the forest stands
   where the live weather rains. Costs a spike and seconds of bake.
2. A latitude-and-noise climate for now, the air grid later. Rejected by call 2: two models disagree.
3. Biomes by noise (Minecraft's way). No cause, so no rain shadow or wet coast to read.

## Decision
- **The air grid** (`prototypes/air`, README there): columns of 128 m, four layers, the passes of
  `air.md`; water and dirt in integer units (an exact ledger), heat and wind f32 in a fixed order.
  - Terra costs **0.23 ms of a tick** on one thread (target 0.5).
  - The day repeats after about 20 days; there is no drift.
  - The bake's **10 s step** matches the live 1 s step; 30 s does not.
- **Stages 5 to 8 in the bake** (`prototypes/world-bake/src/life.rs`, README section "Climate, water on
  land, soil and biomes"):
  - Climate: the air model settles on the air columns built from the bake; its means come to the 8 m cells
    smoothly, with the lapse by each cell's height.
  - Water on land: rain less evaporation is routed down the receivers as discharge; wetlands where ground
    is flat, fed and humid.
  - Soil: sand, loam, clay, gravel, peat, rock, from slope over 24 m, coast, wetness, floodplains, aridity.
  - Biome: a Whittaker table by temperature band and rain, after ice, alpine, tundra, beach and wetland;
    the sea's three by depth and warmth. Borders blend: the inputs are jittered by 40 m noise, so
    neighbouring biomes interfinger.
  - Every threshold is in `LifeParams`; every air constant in `air::Params` (later `data/air.toml`).
- **Measured** on 8 of 12 threads:
  - 4 km Terra, seed 5: **+5.4 s** (17.0 to 22.5 s), +136 MB at the peak (349 MB).
  - 12 km: +66 s, 2.7 GB, and the air model does not settle within its 45-day cap.
  - The same hash twice; all 15 biomes on 8 seeds.

## Consequences
- **Phase 3 (the engine) waits for the developer's look sheet:** ground blocks, colours and plants per biome.
- **Desert is the largest biome** (22 to 42 % of the land) and **wetland the rarest** (about 1 %); both
  follow from the model and the hilly land, and both are thresholds if the developer wants otherwise.
- **The planet-size question stays open:** at 12 km the bake's climate needs a coarser air grid in the bake,
  a longer settle, or more time. The game stays at `planet=6144` unless the developer decides.
- **For the live weather (W3 proper):** the sea breeze is weak at 128 m, first-order carry smears a plume
  over about 3 columns a minute, and the day's swing is mild (6 K at the equator). These are knobs for when
  weather is played, not for the biomes.
- **Review:** prototype code only, no engine, server or shader code changed, so no reviewer pass yet
  (CLAUDE.md principle 12 starts with Phase 3).
