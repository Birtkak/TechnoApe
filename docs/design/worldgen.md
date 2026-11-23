# World generation and the living world: Terra and Luna

- **Status:** design, 2026-11-10. The developer's calls are in ADR 0026; everything marked *proposal* or
  listed under "Taste calls still open" is not decided. **Built 2026-11-11 to 13:** stages W0 to W6 (plan: `docs/plans/worldgen-plan.md`), tides, rivers and
  lakes; living plants (W7) are not.
- **Names:** the planet is **Terra**, its moon is **Luna** (the developer, 2026-11-10).
- **Comes after** the optimisation pass (`docs/plans/optimization-handoff.md`); its package E, the height
  cache, is the seam this plugs into.

## The idea in one paragraph
A body is **simulated once, when the world is made**: plates, uplift, erosion, rivers, climate and geology
run over a coarse map of the whole body, and that map (the **bake**) is saved with the world. Every block
then gets its detail from the bake, so coasts, ranges, valleys, rain shadows, soils and ore all have a
cause and can be read by a player who learns the land. Terra is small enough to do this: 226 km² of
surface is 3.5 million map cells at 8 m. Each seed makes its own Terra and Luna; one default seed is
picked and checked by hand.

## Why a bake and not more noise
- Noise cannot make a river that runs downhill to the sea, a valley cut by it, a delta, a rain shadow, or
  copper nuggets in the gravel downstream of a copper lode. A simulation gives all of these for free,
  because each is the result of the step before.
- "Exploration pays" (vision, pillar 3) needs a world that can be reasoned about: tin sits at granite
  margins, clay in floodplains, bauxite where it is hot and wet. Prospecting becomes reading the land.
- The finite planet (ADR 0008) and the material ledger (roadmap) need to know what the body holds. Ore
  bodies made by the bake are counted objects, so the ledger is exact from the first tick.
- It is also faster to play: far chunks read the map's coarse levels and need no noise at all.

## Scale
Terra is a miniature: 1 : 1500 of Earth across, with relief of tens of metres. Rules of thumb for every
number below: horizontal features shrink with the planet; heights are in metres of play (a mountain is a
climb, not a hike of days); climate bands are compressed so the walk from the equator to a pole (6.7 km)
crosses every zone.

## The bake: stages, in order
Each stage reads the ones before it. All run on the cube-sphere's own grid (ADR 0003) at coarse levels, so
the map lines up with chunks and seams fold as they already do.

| # | Stage | What it computes | Technique |
|---|---|---|---|
| 1 | Plates | 8 to 14 plates with a motion each, continental or oceanic crust, crust age | Voronoi growth on the sphere from random seeds; continental area forced to the land share (45 %, ADR 0021) |
| 2 | Boundaries | where plates collide, part and slide; an uplift rate per cell | relative motion at each boundary: fold belts and volcanic arcs where they meet, rifts and ridges where they part, old worn shields far inside (after Cortial 2019, without stepping time) |
| 3 | Uplift and erosion | the height map, drainage, river network, lakes | stream power erosion against the uplift, solved implicitly (Cordonnier 2016, Schott 2023), coarse first (32 m) then refined (8 m); hillslope smoothing; depressions filled to lakes or cut through |
| 4 | Sea and coast | sea level at the land share; shelves, beaches, cliffs, deltas, islands at hot spots | quantile cut as today; coast type from slope and rock hardness |
| 5 | Climate | temperature (mean and yearly swing), rain, wind, per season | the planet's air model (ADR 0010) run to a steady year on the finished heights; see "One climate model, used twice" |
| 6 | Water on land | river discharge and width, lake levels, groundwater level, wetlands | rain routed down the drainage of stage 3 |
| 7 | Soil | soil depth and kind (sand, loam, clay, gravel, peat) | parent rock, slope, climate, and what the rivers of stage 3 carried and dropped |
| 8 | Life | biome, plant cover and its starting state | temperature and rain (a Whittaker table), soil, height, distance to water |
| 9 | Rock | rock provinces and layers down to the bottom of the crust | from plate history: see "Geology" |
| 10 | Ore | ore bodies as counted objects, each with a place, a shape, a host rock and a tonnage | genesis rules per mineral: see "Ore" |
| 11 | The start | the spawn | scored: gentle ground, temperate, fresh water, wood, clay and copper gravel near, tin a real walk away |

**What the bake stores** (per 8 m map cell, about 16 bytes; coarser levels as mips): height, rock province,
soil depth and kind, mean temperature and its swing, rain and its season, water table, biome, plant cover.
Beside the grid: the river network as lines with width, depth and discharge (rivers are narrower than a
map cell), lakes with levels, and the ore bodies.

**Rules the bake obeys**
- **Deterministic from the seed** on one machine (fixed order, no thread-dependent sums), so the soak and
  replays hold. It need not be bit-equal across machines: only the server bakes; the save holds the bake
  and clients download it when they join.
- **A world keeps its bake.** A later version of the generator makes new worlds differently and leaves old
  ones as they are.
- **Budget** (*proposal*, to measure in the prototype): 30 s for Terra and 2 min for the huge planet on
  the dev PC, on worker threads with progress shown; under 10 MB compressed on disk and on the wire; under
  100 MB of server RAM.
- **Ore is the server's.** A client gets the heights, surface and rock map (it meshes with them). Ore
  bodies reach a client only with the chunks they are in, so a whole-planet ore map exists only for
  whoever builds the survey satellite (the EV megaproject).

## From the bake to blocks
- **A column's height** is the map's height interpolated smoothly, plus detail whose strength and
  character come from the map: hard rock makes steps and cliffs, soft rock rounds off, steep slopes get
  scree, dunes ripple in dry sand. Then river beds and banks are cut from the river lines, exact to the
  block, so every stream is continuous and runs downhill.
- **Per column, once:** this is what the height cache of the optimisation pass holds (height, surface
  block, soil depth, rock boundaries). The mesher, the physics and the water all read the same tile.
- **Far chunks** (cells of 8 m and more) read the map's mips directly: no interpolation, no detail, no
  noise. Their surface colour is the biome's, with snow and the season.
- **The third dimension.** Today a column is solid below its height. Caves, overhangs and ore need a block
  to be a function of (column, layer): rock layers by depth, cave shapes by rock, ore bodies by place. The
  bake marks which map cells have caves or ore under them, so most chunks never evaluate either.
  **This changes three readers at once:** `generate`, `phys::blocked` and the water's `Solid::gaps` must
  ask one shared function, or a player falls through a cave the mesher drew.
- **Block ids.** Real rocks times real ores will not fit 255 ids. *Proposal:* widen block ids to 16 bits
  before any of this content exists (cells in jobs, edits, the wire, saves). Routine, but it gets its ADR.

## Geology (real rocks, the developer's call)
- **Continental crust:** a granite and gneiss basement; over it sedimentary cover laid in order
  (sandstone, shale, limestone, with coal seams where old swamps were and salt where seas dried).
- **Oceanic crust:** basalt over gabbro; young at ridges, old and deep at the margins.
- **Where plates meet:** folded and baked rock (slate, schist, marble, quartzite) with granite bodies
  pushed up into it; chains of old volcanoes (andesite, tuff) on the overriding side.
- **Where they part:** rift valleys floored with basalt; hot-spot islands of basalt in the ocean.
- **Layers are folded and tilted** near mountains and flat in the shields, so a cliff or a river cut shows
  bands you can follow, and the same layer comes to the surface again a valley away.
- **Caves follow rock:** water-cut cave systems in limestone that follow the old water table and come out
  at springs; lava tubes in basalt; a few faults and sea caves elsewhere. Granite has almost none.
- **Heat:** rock warms with depth (progression: a cave is the first shelter), faster near old volcanoes
  and rifts, where hot springs come up. Since ADR 0188 (2026-11-23) volcanoes are active: chambers, crater
  lakes, vents and eruptions (`volcanoes.md`). Still **no quakes.**

## Ore (real minerals, the developer's call)
Each mineral has a rule for where it forms, so finding one teaches where to look for the next.
*Proposal*, to tune against `progression.md`:

| Mineral | Gives | Forms |
|---|---|---|
| native copper, malachite | copper (Starter) | nuggets in the gravel of streams below a copper body; green-stained caps where a body reaches the surface |
| chalcopyrite | copper, with iron and sulfur | large low-grade bodies under old volcanic chains |
| cassiterite | tin (Starter) | veins at the edge of granite bodies, and gravels downstream; never near the spawn's copper |
| bog iron, hematite, magnetite | iron | bog iron in wetlands (early, small); banded iron in the old shields (huge) |
| coal | fuel, coke | seams in sedimentary basins |
| clay, sand, gravel, limestone, salt | the starter's clay, glass, flux, cement, food | floodplains and lake beds; beaches and river bars; shallow-sea layers; dried basins |
| galena, sphalerite | lead, zinc, silver | pockets in limestone |
| gold | gold | quartz veins in the folded belts, and gravels below them |
| bauxite | aluminium (MV) | the weathered soil of hot, wet lowlands: it follows the climate, not the rock |
| pitchblende | uranium (EV) | granite and old river sandstones |
| sulfur | chemistry | old volcanic vents and hot springs |
| graphite, diamond | later tiers | folded belts; rare pipes in the oldest shields |

- An ore body is an object: a shape (vein, layer, pipe, gravel bed), a grade that falls off toward its
  edge, a host rock, and a block count. The body's count is the ledger's opening entry.
- One ore, several products (the GregTech depth): chalcopyrite processed well gives copper, iron and
  sulfur; processed badly, copper only.
- Erosion spreads signs: gravels downstream, stained rock, certain plants on certain soils. A pan and a
  walk upstream find the lode.
- The ring railway megaproject (ore from other faces) falls out of the geology: no single plate has
  everything.

## Climate, seasons and biomes
- **One climate model, used twice.** The air grid of ADR 0010 is the weather during play. The bake runs
  the same code on the new heights until its year repeats, and stores the averages. Biomes are chosen from
  those, so the forest stands where the live weather really brings rain. Two separate models would
  disagree, and a desert would sit under daily rain. So the air grid's spike comes **before** stage 5.
- **Seasons** (the developer's call): Terra's spin axis is tilted (*proposal:* 20°). Over the 54-day year
  the day's length, the sun's height and the temperature swing; snow lines move, rivers swell in spring,
  crops have a season, solar power has a winter. ADR 0022's "no tilt" shortcut ends here.
- **Biomes** come from temperature and rain, then soil and height: ice, tundra, cold forest, temperate
  forest, grassland, dry scrub, desert, savanna, rain forest, wetland, alpine, beach, and the sea's own
  (shelf, deep, reef in warm shallows). Borders blend over tens of metres.

## Rivers, lakes and tides
- **Rivers and lakes hold real, counted water** (ADR 0020), fed by the rain of their catchment.
- **A river at rest costs nothing**, as the still sea does: a reach in steady flow is described by the
  bake (surface, speed, discharge). Dam it, dig it or bucket it, and that reach becomes live water with
  the upstream discharge as its inflow. *To spike:* the hand-over between the two states, and the ledger
  across it.
- **Tides** (the developer's call): the sea's level follows Luna's real position, a pure function of
  place and time like the sky. Two high tides a day. *Proposal:* 1 m between low and high. It gives tidal
  flats, harbours that dry, tide mills as early power, and spring tides when the sun and Luna line up. The
  sea's volume stays the ledger's; the bulge only moves it. *To spike:* the still sea is drawn and swum at
  one exact level today (shader, physics, the wet/dry rule at the waterline).

## Living plants (the developer's call)
- **Two levels.** A coarse grid over the whole body holds plant cover per kind (grass, shrub, the forest
  types) and runs for the whole planet on a slow tick: growth and spread by soil water, temperature, light
  and pollution. Near players, plants are real blocks placed from that state.
- **It answers the player.** A felled forest regrows over days if its soil and neighbours remain; a
  clear-cut slope loses soil; bad rain kills a forest downwind of a smelter; a dammed valley drowns one.
- Depends on fine cells and felled trees (roadmap), so it is the last stage built.

## Luna
Luna is about 1 km in radius (ADR 0027; 177 m before): 14 km² of surface, so the stages below have room.
(The 8 cube corners were to hold something made; dropped by ADR 0136, the bake treats them as any ground.)

No air, no water cycle, no plates. Its bake is a different, shorter pipeline:

| # | Stage | What it computes |
|---|---|---|
| 1 | Old crust | pale highland rock (anorthosite), rough from the start |
| 2 | Basins | a few giant impacts; the ones on the side that faces Terra flood with dark basalt (the maria), as on the real Moon |
| 3 | Craters | stamped oldest first from the largest down to a few blocks across, counts following a power law; small ones are bowls, large ones get flat floors, terraces and a central peak; each throws a blanket of debris that thins with distance, and the youngest have bright rays |
| 4 | Ageing | older craters are worn soft by later debris, so ages can be read from sharpness |
| 5 | Dust | regolith: deep on old highlands, thin on young maria and fresh crater floors |
| 6 | Under the surface | lava tubes and collapsed pits in the maria (the first shelter from radiation, progression's "bases under rock") |
| 7 | Ore | what Terra lacks |

- **Luna's materials** (*proposal*): ilmenite in mare basalt (titanium, and oxygen to breathe);
  anorthite in the highlands (aluminium); iron-nickel and platinum metals where metal impactors fell;
  thorium and rare earths in one province by the largest mare; helium-3 soaked into old dust (the fusion
  fuel of `progression.md`); ice.
- **Ice needs an honest answer.** Luna's orbit is tilted 40°, so if its spin axis follows its orbit there
  are no craters in permanent shadow. *Proposal:* shadow is computed in the bake, not assumed; if none
  exists, ice lies buried under dust at the poles and in the deepest tubes. Either way it is scarce.
- The mass driver, the planet eater and the ledger read the same counted bodies as on Terra.

## Taste calls still open (the developer's, before or at the stage named)
1. ~~**Relief** (stage 3)~~ **decided 2026-11-11** (the developer, on the W1 prototype's sheets, as its
   coding session reported): **ranges to 120 m and seas to 60 m**, and **higher plains**: more uplift inside
   the continents, so the median land of seed 1 stands at 20 m, not 1.5 m (`prototypes/world-bake`,
   commits f376991 and 93cff69). It was 50 m and 25 m (ADR 0021). Also decided there: **the bake's size on
   disk matters less than its speed**; the huge planet's 13.3 MB against the 10 MB proposed is accepted.
2. **How deep the ground goes** (stage 9): the crust is 64 m today. *Proposal:* 192 m, with the hot,
   undiggable-for-now zone below; the save format already may not assume it stays (ADR 0009).
3. ~~**The look of the default Terra**~~ **decided 2026-11-11** (the developer, from the sheet of 12 baked
   seeds, as the W1 coding session reported): **seed 5** (land median 30 m, the highest 1 % over 78 m;
   `prototypes/world-bake` has it as its default). The engine's default seed, 1337 today, moves to it in W2.
4. **The wait** when a world is made: is 30 s with a progress view fine?
5. **Tilt and season strength** (20°?), and **tide height** (1 m?).
6. ~~**The biome list**~~ **decided 2026-11-11** (the developer; ADR 0042): the full list above, 12 on land
   and 3 in the sea, each shown on a look sheet before it is accepted. Also decided then: biome maps first
   in the prototype, then the bake with its biomes into the engine (W2 with W3); the climate from the air
   grid itself, not a simpler stand-in; per biome its ground blocks and still plants as plain generated
   blocks (living plants stay W7). **Still open:** each biome's look (ground blocks, colours, plants).
7. **Luna's ice.**
8. **How much the player is told:** a map item, a compass, rock names on hover, or all learned by eye.

## Sources
- Cortial, Peytavie, Galin, Guérin: Procedural Tectonic Planets (2019), https://diglib.eg.org/handle/10.1111/cgf13614
- Cordonnier et al.: Large Scale Terrain Generation from Tectonic Uplift and Fluvial Erosion (2016), https://diglib.eg.org/handle/10.1111/cgf12820
- Schott et al.: Large-scale terrain authoring through interactive erosion simulation (2023), https://bohrium.dp.tech/paper/arxiv/864973098166780080
- Analytical stream power solutions with multigrid (CGF 43(2), 2024), https://diglib7.eg.org/bitstream/handle/10.1111/cgf15033/v43i2_31_15033.pdf
- Minecraft's multi-noise terrain and noise caves (what a noise-only world gives), https://minecraft.wiki/w/Terrain
- TerraFirmaCraft world generation: rock layers, ore by rock, climate settings, https://terrafirmacraft.github.io/Documentation/1.18.x/worldgen/dimension
- Hadley cells, rain shadows and a Whittaker table in generators, https://engineered.at/articles/generating-worlds-faking-realism and https://space.geometrian.com/calcs/climate-sim.php
- Crater shapes, debris falloff and size counts: Rosenburg (2014), https://thesis.library.caltech.edu/8408/25/Rosenburg_Margaret_2014_Ch4.pdf ; Minton et al. (2019), https://arxiv.org/pdf/1902.07746 ; cratermaker, https://cratermaker.readthedocs.io/
