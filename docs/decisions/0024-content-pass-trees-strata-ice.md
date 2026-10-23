# 0024 – Content pass: tree kinds, badlands strata, sea ice, ice spikes, basalt, ferns

- **Status:** accepted (built 2026-10-23)
- **Date:** 2026-10-23

## Context
The Fable backlog's content job: grow the full atlas, add materials and
item sprites, and tweak biomes and vegetation. biomes.json already
named a `tree_kind` per biome (oak, spruce, acacia, jungle), but the
generator drew the same oak blob everywhere. Several biome notes said
"later": frozen oceans waiting for an ice material ("snow stands in"),
badlands "terracotta-like layered strata later", ice fields "packed-snow
pillars later", magma caves "basalt-like stone later". The atlas was
8×7 with every tile in use.

## Decision
- **Atlas 8×10.** Rows grow at the bottom and COLS stays 8, so every
  existing tile index keeps its meaning (the old 8×7 region is
  byte-identical). Tiles 56..72 are new; 73..79 are free.
  `GameMaterials.ATLAS_TILES` is the only size the code reads.
- **Materials 76..87** (append-only): spruce/acacia/jungle log and leaves,
  terracotta (orange, red, cream), basalt, packed ice, fern. Oak logs get
  a ring top tile. Logs burn like oak, and lava ignites them. Logs are
  items tagged `log`, so the existing recipes take them (planks, charcoal).
  The planks are still `oak_planks`: per-wood planks would double the
  materials for no gameplay yet. Leaves and ferns drop nothing, like oak
  leaves and grass.
- **Tree kinds in the generator.** biomes.json `tree_kind` →
  `TREE_KIND[]` in the generated biome table (BiomeRegistry validates that
  every biome with trees names a kind). `WorldGen::canopy_radius` gives a
  leaf radius per voxel layer for each kind:
  - oak: unchanged; old-growth oaks are taller and wider.
  - spruce: a cone with alternating fringes.
  - acacia: a short trunk with a flat umbrella crown.
  - jungle: 15-22 voxel trunks with a wide crown.

  The column loop still looks up one crown per column, so the cost is the
  same as before. The far LOD canopy (lod ≥ 3) uses the kind's leaves.
  TREE_MAX_RADIUS is now 4.
- **Frozen seas.** A new table column `surface.ice` (`SURFACE_ICE[]`)
  names what the top voxel of open sea water freezes into. Frozen ocean:
  `ice`. Lakes and marshes keep their existing cold-climate rule.
- **Badlands strata.** `terracotta` in a biome's `under`/`steep` stands
  for banded strata. `WorldGen::strata_at` picks 3-voxel bands by world
  height, from a fixed mix of orange, red, cream and sandstone, and lets
  them wobble ±3 voxels over tens of metres. They replace the
  sandstone layer (24 deep in badlands). Steep faces show them instead
  of bare stone. Badlands columns measure their slope, which `column()`
  otherwise only does on mountains and coasts.
- **Ice spikes.** In ice fields, 3 in 8 cells of 12 m hold a packed-ice
  spike 3-12 m tall that tapers from 3 or 5 voxels wide. The spike raises
  the column height, so it shows at every LOD like terrain does, not
  like micro vegetation. Packed ice lies under the snow.
- **Magma caves:** basalt floors, walls and ceilings (data only).
- **Ferns:** a micro-voxel rosette (`micro: "fern"`) that takes part of
  the grass share in taiga, jungle, old growth, swamp and forest.
- **Inside-out ape.** MicroMesher winds counter-clockwise, and ADR 0022
  routed terrain models, animals and furniture through
  `MobModel.outward()`. The player body and the `model_sheet` tool now go
  through it too.

## Addendum: ground decorations (2026-10-23)
- **`decor` in biomes.json**, generated into `DECOR_*` tables. It gives
  the chance per 8 m cell of one bush, boulder or dead tree, and the
  chance per 12 m cell of a spire with a material and height range.
  BiomeRegistry validates it.
- **Stamps.** `WorldGen::stamp_at_cell` / `stamps_near` work like trees:
  a few lookups per block, then a short list per column. The shapes drape
  over each column's own ground:
  - bush: a leaf mound of the biome's tree kind over a log core;
  - boulder: a stone dome of radius 1-3, with moss on top in forests;
  - dead tree: a bare trunk with a stub branch.

  Stamps go to lod 1, like micro vegetation, and stay off water, ruins
  and spires. A stamp on the ground voxel replaces that voxel's plant.
- **Spires** generalise the ice spikes: packed ice in ice fields,
  banded terracotta in badlands, sandstone hoodoos in deserts.
- **Fallen logs wait** for a log block that is bark on every face (or log
  orientation). A log lying down would show its rings on top.

## Addendum 2: birch, fallen logs, berries, pumpkins, flowers (2026-10-23)
- **Atlas 8x11** (tiles 73..81 new, 82..87 free).
- **Materials 90..101**, after ADR 0023's work pieces:
  - birch log and leaves;
  - blueberry bush (micro model, drops 1-3 blueberries, food 2);
  - pumpkin (a block; pumpkin + wheat = pumpkin pie, food 8);
  - cornflower, oxeye daisy and allium (`micro: "bloom"`, tiles [petal, centre, blades]);
  - bark blocks for oak, spruce, acacia, jungle and birch.
- **Bark blocks** are the log's side on every face, so fallen logs can
  lie on their side without log orientation. Breaking one drops the
  log.
- **`tree_mix`** in biomes.json gives a biome a second tree kind with a
  share, generated into `TREE_MIX_KIND` / `TREE_MIX`. Birch takes 30% of
  forests, 25% of plains, 15% of highlands and 10% of taiga. It is a slender
  trunk with a narrow crown.
- **`decor.fallen_log`** adds a bark log of the cell's tree kind, 4-7
  long along x or z. It has moss and the odd brown mushroom on top in
  forests.
- **Ground plants:**
  - blueberry bushes in forests, old growth and taiga;
  - pumpkins as a third wild-patch kind next to ADR 0022's wheat and
    carrots, in plains and forests;
  - flowers in single-kind patches from a per-biome mix of five:
    meadow, woods, upland and wet.

## Consequences
- `test_content_pass.gd` checks:
  - ids, atlas bounds and data;
  - that the fern is a small plant;
  - and, from the real generator, spruce/acacia/jungle wood in their
    biomes, ferns, terracotta bands, sea ice, spikes standing above the
    ground and basalt in magma caves.
- Autopilot screenshots of every changed biome are within the ADR 0009
  budget (`budget_ok`).
- Magma caves lie below the lava line (caves under sea − 180 fill with
  lava), so their basalt mostly lines lava lakes.
- Not done: per-wood planks, saplings, leaf decay, birch, snow on
  spruce, Minecraft's bent acacia trunks. The column-based tree
  model allows a crown offset but not a diagonal trunk.
