# ADR 0047: Clutter is made of voxels and goes as one piece

Status: accepted (2026-10-25)

## Context
Playtest 2026-10-25: "make flowers a 30 or 50 voxel thing instead of 1"; "reeds are still fake green
things instead of items" (the kelp and seagrass of OceanLife, a MultiMesh of thin boxes, ADR 0027);
then: "have clutter items mostly like mushrooms, pick ups, grass, kelp, seashells whatever be a mesh
of voxels that act like 1 piece", "make kelp waterloggable like minecraft", "metals and ores too"
(asked: ore nuggets in the rock), "anything that makes sense to be a bigger piece".

Before: a flower was one voxel holding a micro model of a flower on top of a grass tuft, a mushroom
one voxel with a micro mushroom; kelp was not in the world at all; ore veins were 25 cm grains.
ADR 0043 already made sticks and stone nodules whole pieces.

## Decision
1. **Clutter is whole voxels.** Materials with `"clutter": "<group>"` are plain cubes of their tile,
   walked through (no body collision, an aim box), crushed by falling pieces. Flower materials
   (flower, flower_red, cornflower, oxeye_daisy, allium) and mushrooms are now clutter; new ids
   111 plant_stem, 112 kelp, 113 seagrass, 114 seashell, 115 mushroom_stem; atlas tiles 101 kelp,
   102 seagrass, 103 seashell (the atlas is full).
2. **FineGen lays them as stamps** on a 6-voxel (0.75 m) grid, one per cell, centred, so the next is
   at least a voxel away: flowers in meadow patches (a stem of 4-5 with two leaves and a head of
   petals, ~28 voxels; alliums a ball on a 6-voxel stem, 27), mushrooms on forest floors (stem and
   cap, 16; a big one in three, ~30), seashells on beaches (a ribbed fan of 8). Measured with
   `tools/clutter_probe.gd`.
3. **Kelp and seagrass stand in the water**, Minecraft's waterlogging: columns of kelp in patches
   2-30 m deep (up to 90 % of the depth, 10 m at most), seagrass tufts of 1-3 in the shallows. An
   opaque cube already hides the water faces against it, and generated water is a reservoir (ADR
   0045) that refills a cut cell. A body in kelp counts as in water (swimming, breath). OceanLife
   keeps only the fish.
4. **Pieces** (`GroundPickups`): the touching voxels of one group are one piece (a flower's petals and
   stem); plants in stalks (grass, ferns, reeds, dead bushes, kelp, seagrass) go a stalk at a time.
   Digging any voxel takes the whole piece; a held right click pops clutter and ground pickups by
   hand. A piece drops once, as its first non-stem material; stalks and ore nuggets drop per voxel,
   so grass gives the dry grass it did and a nugget gives its voxels of ore.
5. **Ore nuggets.** A vein lens is cut into 0.5 m cells; a cell may hold one nugget, a 3x3x3 lump with
   chipped corners in its upper corner (~22 voxels), so stone parts it from the next. Cells hold one
   2.4x as often as the old grains were ore, so a vein gives about what it did.
6. New items: brown_mushroom (food), red_mushroom, kelp (food), seashell.

## Consequences
- Old saves keep their single-voxel flowers and mushrooms, now drawn as one cube of petals or cap.
- Kelp costs faces: about one stalk per 32 columns in its patches, LOD 0 only; flowers and mushrooms
  are LOD 0 only too (they vanish beyond the 24 m detail ring as grass detail did).
- Grass stays micro-model tufts (water would wash it away; the developer agreed it stays as it is).
