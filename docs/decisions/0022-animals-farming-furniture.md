# 0022 – Animals, farming and furniture

- **Status:** accepted (built 2026-10-23)
- **Date:** 2026-10-23

## Context
Requested 2026-10-23: sheep and pigs that spawn in plains and roam, graze now
and then, follow the player holding their food and breed when fed "like
Minecraft"; they drop meat, sheep drop wool for carpets, beds and chairs.
Then: wheat and carrots with farming "exactly like Minecraft", and wild
carrots and wheat seeds that are rare enough to be a nice find and a reason
to start a farm. "Make a nice mob AI."

The game had no entities besides dropped items (ItemEntity on VoxelBody,
ADR 0011), no hunger, no light data, no random ticks over the world, and no
block bigger than one 0.5 m voxel. GDScript must not iterate voxels
(principle 8); every system ships with a check and a benchmark.

## Options considered
1. **Mobs as godot_voxel / physics bodies** (CharacterBody3D, navmesh).
   Terrain colliders are off (ADR 0011) and a navmesh over a streaming,
   editable voxel world is a large system of its own. Rejected.
2. **Mobs as VoxelBody boxes with a Minecraft goal brain** (chosen): the same
   box mover as the player and items, steering instead of path finding, a
   brain at 10 Hz, voxel probes only in the brain, a budget of target searches
   per frame. Cheap and close to how Minecraft animals behave.
3. **Farming as world-wide random ticks** like Minecraft's chunk sections.
   Needs a native pass over every loaded voxel. Rejected for now:
4. **Farming state for player-made farmland and crops only** (chosen): a
   dictionary of the voxels the player tilled or sowed (hundreds), random
   ticks rolled per tracked voxel with Minecraft's probability. Exact
   Minecraft statistics without touching terrain.
5. **Furniture as micro-voxel terrain models** (one material per piece and
   facing): a 1 x 2 m bed is 8 voxels, 4 facings, separate head/foot models:
   dozens of material ids. Rejected for:
6. **Furniture as a multi-voxel block entity** (chosen): an invisible
   `furniture` voxel with a full collision box in every covered cell, one
   rotated mesh per piece built with MicroMesher on the 6.25 cm chisel grid
   (ADR 0006), its own palette. One material id for every piece of furniture.

## Decision
Options 2, 4 and 6. Data: `data/mobs.json`, `data/crops.json`, new items and
recipes in `items.json` / `recipes.json`, drop lists in `mining.json`.

**Animals** (`scripts/mobs/`): `MobManager` spawns herds the first time a
16 m cell's ground loads within 48 m of the player (deterministic per seed and
cell, 10 % per cell, species by Minecraft spawn weight among those whose
`biomes` match: sheep 12, pig 10, plains only for now), stores animals
farther than 64 m as data and wakes them when the player returns (their
timers catch up), saves them as JSON next to the world. Animals never
despawn. `Mob` brain, highest goal first (Minecraft's animal goals): panic
when hurt (5 s, faster), breed (walk to a partner in love, 3 s together, a
baby; 5 min cooldown), tempt (follow a player holding its food within 10 m,
stop 2.5 m away, 5 s cooldown), babies follow an adult, graze (1/1000 per
tick, lambs 1/50, 2 s head down; eats the tuft it stands on, regrows wool,
ages a baby by a minute), stroll (~every 6 s to a spot up to 10 m away,
grass scores highest, the herd pulls a little), look at the player or around.
Steering: straight at the target, 1-voxel step-up, a hop over 2-voxel ledges,
a 1 s slide along walls when stuck; never walks off more than 3 m or into
water or lava. Babies grow up in 20 min; feeding one takes 10 % off.
Hits: hand 1, tools 2 + tier; 0.5 s invulnerable, knockback, red flash; death
tips over and drops (adults only): sheep 1-2 raw meat + 1 wool unless shorn,
pigs 1-3 raw meat. Shears give 1-3 wool. Food: sheep wheat, pigs carrot.
Models: `MobModel`, Minecraft proportions on a 16-cell grid (one Minecraft
pixel = 2 cells), meshes shared per species, built during the loading screen.

**Farming** (`scripts/farming/farming.gd`), Minecraft 1.20 rules with one
voxel as one block: a hoe tills grass or dirt with air above; seeds and
carrots plant on a farmland top face; random ticks at 3 per 4096 blocks per
20 Hz tick (~68 s per block); farmland with water within 4 voxels at its level
or one above goes to moisture 7 (dark texture), else it dries by one and
reverts to dirt at 0 without a crop; growth chance 1 / (floor(25 / f) + 1)
with Minecraft's f (the 3 x 3 farmland under the crop, moist counts 3, the
eight around a quarter, halved for same-crop rows on both axes or diagonals);
landing on farmland from more than 0.5 m tramples it with chance fall - 0.5
(metres, as jumps are tuned in metres); a solid block placed on it turns it to
dirt; the crop pops off when its farmland goes. Drops: ripe wheat 1 wheat +
0-3 seeds (3 tries at 4/7), young wheat 1 seed, ripe carrots 1-4, young 1.
Wheat has 8 looks, carrots 4 (ages 0-1, 2-3, 4-6, 7). Not built: light (no
light data; crops grow in the dark), bone meal (no bones), rain.

**Wild crops** (generator, `vegetation_at`): a 24 m cell holds a small patch
(1.5 m disc, a quarter of its grass columns) with a per-mille chance per
biome: plains wild wheat 30 and wild carrot 20, savanna wheat 15, forest
carrot 12. Measured (`tools/wild_crop_probe.gd`, seed 1337, 1 km^2 around
spawn): 3 wheat patches (3-11 plants), 3 carrot patches (5-6 plants), the
nearest 340 m from spawn. Wild wheat drops 1-3 seeds and half the time a
wheat; wild carrot 1-2 carrots. Tall grass drops a seed 4 % of the time
(Minecraft: 12.5 %). GEN_VERSION stays 10: only vegetation changed, so saved
worlds keep their terrain and grow wild crops in newly generated land.

**Furniture**: bed 1 x 2 m (2 x 4 voxels, head away from the player), chair
0.5 x 1 m (1 x 2 voxels, facing the player). Right click a bed: at night
(Minecraft's 12542..23459 ticks) sleep to morning; always set the respawn
point. Right click a chair: sit (jump or sneak stands up). Breaking any
covered voxel removes the piece and drops its item. Carpet (two wool, three
carpets) is a one-cell micro rug, wool a full block.

**Eating** (no hunger yet): hold right click with food for 1.6 s when hurt;
heals the item's `food.hunger` in half-hearts (raw meat 2, carrot 3, bread 5,
cooked meat 6).

**Fixes found on the way**: MicroMesher emits counter-clockwise triangles and
Godot's front faces are clockwise, so every micro model drew its inside
(visible on the flat wild-carrot umbels and the animals). `MobModel.outward`
reverses the triangles for animals, furniture and the terrain's micro models.
The ape (ApeModel) is still drawn inside out: its face was authored on the
side that showed, so it needs its own pass. Micro plants also had no
collision box at all, so the aim ray could not hit them (tall grass, crops,
pebbles could not be broken); they now have a box on collision bit 2 that
only the aim ray sees (VoxelBody collides with bit 1).

## Consequences
- Measured on the dev box (`autopilot=1 walk=1 speed=0 duration=15 animals=40
  mob_spawn=0 vsync=0`): 40 animals cost 0.15 ms per frame on average
  (~1 ms per 60 Hz physics step, ~25 us per animal); p99 5.3 ms vs 3.1 ms
  without animals, max 7.6 ms, budget_ok. The cap is 60 active animals.
  Natural density is a few herds near the player.
- New material ids 57..75 (append only): wool, carpet, farmland,
  farmland_moist, wheat_0..7, carrots_0..3, wild_wheat 73, wild_carrot 74,
  furniture 75. The atlas (8 x 7) is now full: tiles 51 wool, 52/53
  farmland, 54 ripe wheat, 55 carrot.
- Saves: `mobs_<seed>_g<gen>.json` and `farms_<seed>_g<gen>.json` next to the
  block entities; furniture is a block entity kind.
- Tests: `test/test_animals_farming.gd` (growth formula, hydration, drying,
  trampling, drops, feeding, footprints, saves). Checks: `hands_test=1`
  (till, sow, chair, sit, bed, sleep, feed, shear, kill, eat through the
  player's own input code), `animals=N`, `farm_test=1 farm_speed=N`,
  `tools/mob_sheet.gd`, `tools/wild_crop_probe.gd`.
- Later: hunger (food already carries `hunger`/`saturation`), cooking meat at
  a campfire, light for crops, bone meal, grass spreading back over dirt
  (grazing keeps grass blocks for now), more species and biomes (data only),
  dyes and coloured wool, the ape's winding.
