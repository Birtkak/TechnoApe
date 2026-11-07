class_name GameMaterials
extends RefCounted

## Material table for the game, Noita attribute names (see
## docs/design/material-simulation.md). Index = material id, 0 = air.
## `tiles` = atlas tile index [top, side, bottom] (game/assets/textures/atlas.png, ATLAS_TILES).
## `interact` = what right-clicking the block opens (ADR 0007); `entity` = block
## entity kind created on placement (chest / furnace inventories).
## TODO Phase 2: load from data/materials/*.json validated by schema + tests.

const AIR := 0
const STONE := 1
const DIRT := 2
const GRASS := 3
const SAND := 4
const WATER := 5
const WOOD := 6
const LAVA := 7
const ASH := 8
const OIL := 9
const STEAM := 10
const SMOKE := 11
const FIRE := 12
const BEDROCK := 13
const SNOW := 14
const GRAVEL := 15
const COAL_ORE := 16
const IRON_ORE := 17
const COPPER_ORE := 18
const TIN_ORE := 19
const LEAVES := 20
const SAND_STATIC := 21     # natural ground sand: static until disturbed (Noita's static ground)
const GRAVEL_STATIC := 22
const WATER_STATIC := 23    # generated rivers, lakes, sea: still until something opens beside it, then finite water (ADR 0068)
const SANDSTONE := 24
const CLAY := 25
const TALL_GRASS := 26      # micro-voxel vegetation (8^3 cells of 6.25 cm, MicroMesher, no collision)
const FLOWER := 27
const CACTUS := 28
const DEAD_BUSH := 29
const ICE := 30
const FLOWER_RED := 31
const CRAFTING_TABLE := 32  # station blocks (ADR 0007)
const FURNACE := 33
const FURNACE_LIT := 34     # same block entity, swapped in while the furnace burns (Minecraft's lit state)
const CHEST := 35
const PLANKS := 36
const PEBBLES := 37         # micro-voxel ground / cave detail (ADR 0006 grid): generator only
const MUSHROOM_BROWN := 38
const MUSHROOM_RED := 39
const REED := 40            # two stacked voxels at marsh puddle edges
const LILY_PAD := 41        # floats on a puddle's water voxel
const STALACTITE := 42      # hangs from cave ceilings
const STALAGMITE := 43      # cave floors; both have a core collision box
const CHISELED := 44        # reserved (ADR 0006): empty non-culling model, shape lives in the block entity
const GLOW_MUSHROOM := 45   # deep caves; its cap tile is emissive in the terrain shader
const MOSS := 46            # carpet on lush cave floors
const BLOOD := 47           # bleeding (StainMap.bleed): a short-lived liquid that soaks in and leaves a stain
# Ground pickups (Vintage Story style, ADR 0019): micro detail you pick up by hand.
const COPPER_BEARING_STONES := 48  # surface indicator above a copper vein (ore_veins.json)
const TIN_BEARING_STONES := 49
const IRON_BEARING_STONES := 50
const FLINT_STONES := 51           # flint lying on gravel and stony ground
const TWIGS := 52                  # fallen sticks on forest floors
const GROUND_PICKUPS := [PEBBLES, COPPER_BEARING_STONES, TIN_BEARING_STONES, IRON_BEARING_STONES, FLINT_STONES, TWIGS]
# Sulfur caves and Noita chemistry
const TOXIC_SLUDGE := 53           # Noita's toxic sludge: stains you toxic; water dilutes it into water
const TOXIC_SLUDGE_STATIC := 54    # generated sludge pools in sulfur caves: static until disturbed
const SULFUR := 55                 # yellow crust on sulfur cave floors and ceilings
const OBSIDIAN := 56               # lava quenched by water
# Animals, farming and furniture (ADR 0022)
const WOOL := 57
const CARPET := 58                 # 6.25 cm rug on the voxel floor
const FARMLAND := 59               # hoed dirt; Farming tracks its moisture and swaps in FARMLAND_MOIST
const FARMLAND_MOIST := 60
const WHEAT_0 := 61                # wheat crop ages 0..7 = ids 61..68 (Minecraft's 8 stages)
const CARROTS_0 := 69              # carrot crop: 4 looks for ages 0..7 (Minecraft: 0-1, 2-3, 4-6, 7) = ids 69..72
const WILD_WHEAT := 73             # rare generated patches in plains (ADR 0022): seeds to start a farm
const WILD_CARROT := 74
const FURNITURE := 75              # invisible full-collision voxel under a bed or chair; the shape is the furniture entity's mesh
# Content pass: tree kinds (biomes.json tree_kind), badlands strata, magma cave basalt, ice fields, ferns
const SPRUCE_LOG := 76
const SPRUCE_LEAVES := 77
const ACACIA_LOG := 78
const ACACIA_LEAVES := 79
const JUNGLE_LOG := 80
const JUNGLE_LEAVES := 81
const TERRACOTTA := 82             # badlands strata: orange, red-brown and cream bands
const TERRACOTTA_RED := 83
const TERRACOTTA_CREAM := 84
const BASALT := 85                 # magma cave floors and walls
const PACKED_ICE := 86             # ice fields ground and spikes
const FERN := 87                   # micro-voxel fronds in taiga, jungle, swamp and old growth
# Vintage Story work pieces (ADR 0023): drawn by their block entity (scripts/work/)
const WORK_SPOT := 88              # knapping, clay forming, ground storage, firepit: only the aim ray hits it
const WORK_BLOCK := 89             # sieve: solid to bodies
# Content pass 2 (ADR 0024): birch, blueberries, pumpkins, more flowers, bark for fallen logs
const BIRCH_LOG := 90
const BIRCH_LEAVES := 91
const BLUEBERRY_BUSH := 92         # micro-voxel bush; drops blueberries
const PUMPKIN := 93
const CORNFLOWER := 94             # micro "bloom" flowers: blue, white, purple
const OXEYE_DAISY := 95
const ALLIUM := 96
const OAK_BARK := 97               # bark on every face: fallen logs lie on their side (no log orientation)
const SPRUCE_BARK := 98
const ACACIA_BARK := 99
const JUNGLE_BARK := 100
const BIRCH_BARK := 101
# Wildlife (ADR 0027)
const CARCASS := 102               # half-eaten remains: what a bear leaves, and the odd old kill in the wild
# Flint Age depth (ADR 0030): shelter, light, storage
const THATCH := 103                # dry grass bundles: a roof that burns
const WATTLE := 104                # woven sticks (micro lattice, solid)
const DAUB_WALL := 105             # wattle plastered with clay, straw and dirt
const TORCH := 106                 # stands on a top face; TorchLights gives the nearest ones a light
const STORAGE_POT := 107           # fired clay jar: a 9-slot container (micro model, solid)
const LEAF_LITTER := 108           # the shaded forest floor: fallen leaves over soil, dug like grass
# Patterned building materials (ADR 0039)
const COBBLESTONE := 109           # what stone breaks into: rounded stones in a dark bed
const BRICKS := 110                # kiln-fired clay bricks in courses
# Clutter as voxel pieces (ADR 0047): whole voxels, walked through, dug and picked up as one piece
const PLANT_STEM := 111            # a flower's stem and leaves
const KELP := 112                  # stands in water like Minecraft's waterlogged blocks: water refills its cell when cut
const SEAGRASS := 113
const SEASHELL := 114
const MUSHROOM_STEM := 115
# 116..119 reserved for part A of the 2026-10-25 split (placeholders until it lands)
const CRYSTAL := 120               # geode lining (ADR 0052): glows in the terrain shader
const SPRING_STONE := 122          # the core of an aquifer's chamber (ADR 0110)
const MAGMA_HEART := 123           # the core of a magma conduit's chamber (ADR 0110)
const LAVA_STATIC := 121           # generated cave lava pools and the deep lava sea: still until disturbed, then finite lava (ADR 0069 C)
# 121..126 reserved for world generation; 130..159 parts C and D
# Giant flowers in the flower meadow (bug report 2026-11-02 18-57-25): tree-sized, felled like a tree
const GIANT_STEM := 124
const GIANT_PETAL := 125
const GIANT_HEART := 126
# The dry savanna floor (bug report 2026-10-25 20-37-22): straw grass, its tussocks, bare red earth
const SAVANNA_GRASS := 127
const SAVANNA_TALL_GRASS := 128
const RED_EARTH := 129
const METEORIC_IRON_HOT := 141     # ADR 0157: the same iron while it glows (fresh crater)
const METEORIC_IRON := 147         # ADR 0157: iron-nickel left in a meteorite crater, smelts straight to iron
const SCORCHED_EARTH := 148        # ADR 0157: the burnt ground around a crater
# Rock kinds, their cobbles, bricks and loose stones, more clutter, ruin pots (ADR 0057)
const GRANITE := 160
const DIORITE := 161
const LIMESTONE := 162
const SLATE := 163
const MARBLE := 164
const CHALK := 165
const ROCK_KINDS := ["granite", "diorite", "limestone", "slate", "marble", "chalk"]   # natural, ids 160..165
const COBBLE_KINDS := ["granite", "diorite", "limestone", "slate", "marble", "chalk", "basalt"]   # <kind>_cobble 166..172, <kind>_stones 181..187
const BRICK_KINDS := ["stone", "granite", "diorite", "limestone", "slate", "marble", "chalk", "basalt"]   # <kind>_bricks 173..180
const WILD_STRAWBERRIES := 188
const CAVE_CRYSTAL := 197
const CLAY_POT := 198              # ruin pots: any hit shatters them and rolls data/loot.json
const CLAY_POT_CRACKED := 199
const PYRITE := 200                # placed by E's generator for part F (striker): ids 200..202 are the start of F's range
const RED_OCHRE := 201
const YELLOW_OCHRE := 202
const TNT := 203                   # placeable explosive (ADR 0065): a charge per 0.5 m block, lit by Tnt, never burnt by the sim
const FUSE_CORD := 137             # ADR 0150: burns slowly along itself (Settings.fuse_speed_mps), lights TNT it reaches
const FUSE_SPARK := 138            # ADR 0150: the cord's cold flame: a still, short-lived fire cell that lights nothing
const MONOLITH := 204              # the Gelvin monolith (ADR 0098): ashlar, rune bands, the glowing seam; never mined or blasted
const MONOLITH_RUNE := 205
const MONOLITH_SEAM := 206
# Per-wood planks and saplings (ADR 0112): oak keeps PLANKS (36) and grows from OAK_SAPLING
const WOOD_KINDS := ["oak", "spruce", "acacia", "jungle", "birch"]   # the sapling order; planks skip oak
const SPRUCE_PLANKS := 207         # 207..210: spruce, acacia, jungle, birch planks
const OAK_SAPLING := 211           # 211..215: a sapling per WOOD_KINDS, grows into its tree (Saplings)
# Cave dressing (2026-11-02 overnight, world generation): stalactites, vines, cave fungi, magma crust
const CAVE_IDS := 228              # 228..243 world generation (216..255 are split over four lanes)
const TOXIC_GAS := 254             # toxic sludge boiled (ADR 0119 C); 255 reserved for the fluid lane
const DRIPSTONE := 228
const CAVE_VINE := 229
const GLOW_BERRY_VINE := 230
const FUNGUS_STEM := 231
const FUNGUS_CAP := 232
const MAGMA_ROCK := 233
const RUSTED_RAIL := 234           # an abandoned mine's rails (the "mine" ruin)
const BARREL := 216                # ADR 0121: an 18-slot wooden barrel (a 0.75 x 1 m station like the chest); 217..227 this lane's
const STORAGE_SHELF := 217         # ADR 0121: open plank shelving, 27 slots, the goods on it show (a 1 m cube station)
const FOULED_WATER := 218          # ADR 0141 (222..227 stay this lane's): water sludge touched; wilts crops, sickens
const WASTE_GROUND := 219          # soil and soft rock sludge ate, and sludge settled out on a gravel bed
const SLAG := 220                  # smelting waste: a powder that piles at a machine's port
const CHIMNEY := 221               # a brick flue: smoke of the machine under a stack of these leaves at its top

const ATLAS_TILES := Vector2i(8, 28)   # rows grow at the bottom, so tile indices keep their meaning
# Liquids with a "fluid_slot" are Minecraft-style fluids in the terrain (godot_voxel
# VoxelBlockyFluid): a full cell is the material's own voxel id, a cell with 1..7
# eighths (SimWorld fullness) is FLUID_LEVEL_BASE + slot * 8 + eighths - 1. Fixed
# above every material id, so new materials never move them (saves stay valid).
# Same constant as SimWorld::FLUID_LEVEL_BASE.
const FLUID_LEVEL_BASE := 256
const MICRO_GRID := 8       # cells per voxel edge for micro models and chisels (6.25 cm)
const SOLID_MICRO := ["wattle", "storage_pot"]   # micro models with full body collision
const MASK_TARGET := 2      # collision bit of boxes only the aim ray hits (VoxelBody uses bit 1, VoxelBody.MASK_SOLID)


static var _defs := []


## Every material definition (index = id - 1). Built once and shared: read it, never change it (it was
## rebuilt on every call, and a 1 m dig called it per cell: 60 ms).
## "rigid": "wood" carries support and breaks off with what it holds (RigidPieces, P12);
## "leaf" hangs on wood and carries nothing.
static func defs() -> Array:
	if _defs.is_empty():
		_defs = _build_defs()
	return _defs


static func _build_defs() -> Array:
	return [
		{"name": "stone", "cell_type": "solid", "density": 10, "hp": 1000, "tiles": [0, 0, 0], "transparent": false},
		{"name": "dirt", "cell_type": "powder", "liquid_static": true, "density": 3, "tiles": [1, 1, 1], "transparent": false},
		{"name": "grass", "cell_type": "powder", "liquid_static": true, "density": 3, "tiles": [2, 2, 1], "transparent": false},
		{"name": "sand", "cell_type": "powder", "density": 3, "tiles": [4, 4, 4], "transparent": false},
		{"name": "water", "boil_c": 100, "boils_to": "steam", "boil_rise_c": 60, "freeze_c": -2, "freezes_to": "ice", "cell_type": "liquid", "fluid_slot": 0, "density": 1, "liquid_flow_speed": 1.0, "liquid_speed": 8, "film_lifetime_ticks": 300, "tiles": [5, 5, 5], "transparent": true},
		{"name": "wood", "cell_type": "solid", "rigid": "wood", "density": 10, "burnable": true, "fire_hp": 160, "autoignition_temperature": 40, "temperature_of_fire": 50, "on_fire_convert_to_material": 8, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 4, "tiles": [56, 6, 56], "transparent": false},
		{"name": "lava", "heat_c": 1200, "cell_type": "liquid", "fluid_slot": 1, "density": 5, "liquid_flow_speed": 0.3, "liquid_gravity": 0.3, "temperature_of_fire": 150, "film_lifetime_ticks": 300, "film_convert_to_material": 85, "tiles": [7, 7, 7], "transparent": false},
		{"name": "ash", "cell_type": "powder", "density": 2, "tiles": [8, 8, 8], "transparent": false},
		{"name": "oil", "cell_type": "liquid", "fluid_slot": 2, "density": 0.8, "liquid_flow_speed": 0.6, "liquid_speed": 2, "burnable": true, "fire_hp": 60, "autoignition_temperature": 20, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 6, "tiles": [9, 9, 9], "transparent": true},
		{"name": "steam", "condense_c": 60, "condenses_to": "water", "cell_type": "gas", "density": 0, "gas_upwards_speed": 220, "gas_horizontal_speed": 50, "lifetime": 900, "tiles": [10, 10, 10], "transparent": true},
		{"name": "smoke", "cell_type": "gas", "density": 0, "gas_upwards_speed": 190, "gas_horizontal_speed": 60, "lifetime": 330, "wind_drift": 1.0, "tiles": [11, 11, 11], "transparent": true},
		{"name": "fire", "heat_c": 600, "cell_type": "fire", "density": 0, "lifetime": 14, "temperature_of_fire": 60, "gas_upwards_speed": 120, "gas_horizontal_speed": 80, "tiles": [12, 12, 12], "transparent": true},
		{"name": "bedrock", "cell_type": "solid", "density": 15, "hp": 1000000, "tiles": [13, 13, 13], "transparent": false},
		{"name": "snow", "cell_type": "powder", "liquid_static": true, "density": 2, "tiles": [14, 14, 14], "transparent": false},
		{"name": "gravel", "cell_type": "powder", "density": 4, "tiles": [15, 15, 15], "transparent": false},
		{"name": "coal_ore", "cell_type": "solid", "pattern": true, "density": 10, "hp": 900, "burnable": true, "fire_hp": 240, "autoignition_temperature": 90, "on_fire_convert_to_material": 1, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 8, "tiles": [16, 16, 16], "transparent": false},
		{"name": "iron_ore", "cell_type": "solid", "pattern": true, "density": 11, "hp": 1200, "tiles": [17, 17, 17], "transparent": false},
		{"name": "copper_ore", "cell_type": "solid", "pattern": true, "density": 11, "hp": 1100, "tiles": [18, 18, 18], "transparent": false},
		{"name": "tin_ore", "cell_type": "solid", "pattern": true, "density": 11, "hp": 1100, "tiles": [19, 19, 19], "transparent": false},
		{"name": "leaves", "cell_type": "solid", "rigid": "leaf", "density": 10, "hp": 20, "burnable": true, "fire_hp": 40, "autoignition_temperature": 30, "temperature_of_fire": 55, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 6, "tiles": [20, 20, 20], "transparent": true},
		{"name": "sand_static", "cell_type": "powder", "liquid_static": true, "density": 3, "tiles": [4, 4, 4], "transparent": false, "loose": 4},
		{"name": "gravel_static", "cell_type": "powder", "liquid_static": true, "density": 4, "tiles": [15, 15, 15], "transparent": false, "loose": 15},
		{"name": "water_static", "cell_type": "liquid", "fluid_slot": 0, "liquid_static": true, "density": 1, "tiles": [5, 5, 5], "transparent": true, "loose": 5},
		{"name": "sandstone", "cell_type": "solid", "density": 10, "hp": 600, "tiles": [21, 21, 21], "transparent": false},
		{"name": "clay", "cell_type": "solid", "density": 9, "hp": 300, "tiles": [22, 22, 22], "transparent": false},
		{"name": "tall_grass", "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 60, "ignite_chance": 0.6, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 3, "tiles": [23, 23, 23], "transparent": false, "micro": "tuft", "replaceable": true},
		{"name": "flower", "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 40, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2, "tiles": [30, 30, 30], "transparent": false, "clutter": "flower", "floor_only": true},
		{"name": "cactus", "cell_type": "solid", "density": 10, "hp": 40, "burnable": true, "fire_hp": 30, "autoignition_temperature": 40, "temperature_of_fire": 50, "on_fire_convert_to_material": 8, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 4, "tiles": [25, 25, 25], "transparent": false},
		{"name": "dead_bush", "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 24, "autoignition_temperature": 15, "temperature_of_fire": 55, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 3, "tiles": [26, 26, 26], "transparent": false, "micro": "bush", "replaceable": true},
		{"name": "ice", "melt_c": 4, "melts_to": "water", "cell_type": "solid", "density": 9, "hp": 120, "tiles": [27, 27, 27], "transparent": false},
		{"name": "flower_red", "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 40, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2, "tiles": [31, 31, 31], "transparent": false, "clutter": "flower", "floor_only": true},
		{"name": "crafting_table", "cell_type": "solid", "density": 8, "hp": 200, "tiles": [32, 33, 39], "transparent": false, "interact": "crafting_table", "entity": "station"},
		{"name": "furnace", "cell_type": "solid", "density": 10, "hp": 800, "tiles": [34, 35, 34], "transparent": false, "interact": "furnace", "entity": "furnace"},
		{"name": "furnace_lit", "cell_type": "solid", "density": 10, "hp": 800, "tiles": [34, 36, 34], "transparent": false, "interact": "furnace", "entity": "furnace"},
		{"name": "chest", "cell_type": "solid", "density": 8, "hp": 200, "tiles": [37, 38, 37], "transparent": false, "interact": "chest", "entity": "chest"},
		_planks_def(),
		{"name": "pebbles", "cell_type": "solid", "pattern": true, "density": 10, "hp": 10, "tiles": [86, 86, 86], "transparent": false, "micro": "pebbles"},
		{"name": "mushroom_brown", "cell_type": "solid", "pattern": true, "density": 10, "hp": 5, "tiles": [40, 40, 40], "transparent": false, "clutter": "mushroom"},
		{"name": "mushroom_red", "cell_type": "solid", "pattern": true, "density": 10, "hp": 5, "tiles": [41, 41, 41], "transparent": false, "clutter": "mushroom"},
		{"name": "reed", "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 40, "autoignition_temperature": 30, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 3, "tiles": [23, 44, 23], "transparent": false, "micro": "reed"},
		{"name": "lily_pad", "cell_type": "solid", "density": 10, "hp": 5, "tiles": [43, 43, 43], "transparent": false, "micro": "lilypad"},
		{"name": "stalactite", "cell_type": "solid", "density": 10, "hp": 300, "tiles": [0, 0, 0], "transparent": false, "micro": "stalactite"},
		{"name": "stalagmite", "cell_type": "solid", "density": 10, "hp": 300, "tiles": [0, 0, 0], "transparent": false, "micro": "stalagmite"},
		{"name": "chiseled", "cell_type": "solid", "density": 10, "hp": 1000, "tiles": [0, 0, 0], "transparent": true, "model": "empty"},
		{"name": "glow_mushroom", "cell_type": "solid", "density": 10, "hp": 5, "tiles": [45, 45, 45], "transparent": false, "clutter": "mushroom"},
		{"name": "moss", "cell_type": "solid", "density": 10, "hp": 8, "burnable": true, "fire_hp": 40, "autoignition_temperature": 35, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 3, "tiles": [46, 46, 46], "transparent": false, "micro": "carpet"},
		# lifetime in sim ticks (30 Hz): the puddle soaks into the ground after ~7 s; the stain stays
		{"name": "blood", "cell_type": "liquid", "fluid_slot": 3, "density": 1.1, "liquid_flow_speed": 0.7, "liquid_speed": 2, "lifetime": 200, "tiles": [47, 47, 47], "transparent": true},
		# ground pickups: hardness 0 in mining.json, they drop loose items (ADR 0019)
		{"name": "copper_bearing_stones", "cell_type": "solid", "pattern": true, "density": 10, "hp": 10, "tiles": [94, 94, 94], "transparent": false, "micro": "ore_stones"},
		{"name": "tin_bearing_stones", "cell_type": "solid", "pattern": true, "density": 10, "hp": 10, "tiles": [95, 95, 95], "transparent": false, "micro": "ore_stones"},
		{"name": "iron_bearing_stones", "cell_type": "solid", "pattern": true, "density": 10, "hp": 10, "tiles": [96, 96, 96], "transparent": false, "micro": "ore_stones"},
		{"name": "flint_stones", "cell_type": "solid", "pattern": true, "density": 10, "hp": 10, "tiles": [87, 87, 87], "transparent": false, "micro": "pebbles"},
		{"name": "twigs", "cell_type": "solid", "pattern": true, "density": 10, "hp": 5, "burnable": true, "fire_hp": 20, "autoignition_temperature": 20, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2, "tiles": [93, 97, 97], "transparent": false, "micro": "twigs"},
		# sulfur caves (biomes.json sulfur_cave) and Noita chemistry (reactions below)
		{"name": "toxic_sludge", "boil_c": 80, "boils_to": "toxic_gas", "boil_rise_c": 40, "cell_type": "liquid", "fluid_slot": 4, "density": 1.3, "liquid_flow_speed": 0.4, "liquid_speed": 2, "tiles": [48, 48, 48], "transparent": true, "react_awake": true},
		{"name": "toxic_sludge_static", "cell_type": "liquid", "fluid_slot": 4, "liquid_static": true, "density": 1.3, "tiles": [48, 48, 48], "transparent": true, "loose": 53},
		{"name": "sulfur", "cell_type": "solid", "density": 10, "hp": 400, "tiles": [49, 49, 49], "transparent": false},
		{"name": "obsidian", "cell_type": "solid", "density": 12, "hp": 6000, "tiles": [50, 50, 50], "transparent": false},
		# animals, farming and furniture (ADR 0022); crops are micro models on the farmland voxel below them
		{"name": "wool", "cell_type": "solid", "density": 4, "hp": 80, "burnable": true, "fire_hp": 60, "autoignition_temperature": 30, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 3, "tiles": [51, 51, 51], "transparent": false},
		{"name": "carpet", "cell_type": "solid", "density": 4, "hp": 10, "burnable": true, "fire_hp": 30, "autoignition_temperature": 30, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2, "tiles": [51, 51, 51], "transparent": false, "micro": "rug"},
		{"name": "farmland", "cell_type": "powder", "liquid_static": true, "density": 3, "tiles": [52, 1, 1], "transparent": false},
		{"name": "farmland_moist", "cell_type": "powder", "liquid_static": true, "density": 3, "tiles": [53, 1, 1], "transparent": false},
	] + _crop_defs() + [
		{"name": "wild_wheat", "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 30, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2, "tiles": [54, 23, 54], "transparent": false, "micro": "wild_wheat"},
		{"name": "wild_carrot", "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 30, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2, "tiles": [51, 29, 55], "transparent": false, "micro": "wild_carrot"},
		{"name": "furniture", "cell_type": "solid", "density": 8, "hp": 200, "tiles": [39, 39, 39], "transparent": true, "model": "empty", "collision": true, "interact": "furniture"},
	] + _content_pass_defs() + [
		{"name": "work_spot", "cell_type": "solid", "density": 8, "hp": 50, "tiles": [39, 39, 39], "transparent": true, "model": "empty", "collision": "aim", "cross": true, "interact": "work"},
		{"name": "work_block", "cell_type": "solid", "density": 8, "hp": 200, "tiles": [39, 39, 39], "transparent": true, "model": "empty", "collision": true, "interact": "work"},
	] + _content_pass2_defs() + _flint_age_defs() + [
		# leaf litter carries a forest fire along the floor and leaves bare dirt (ADR 0056)
		{"name": "leaf_litter", "cell_type": "powder", "liquid_static": true, "density": 3, "tiles": [84, 84, 1], "transparent": false,
			"burnable": true, "fire_hp": 45, "autoignition_temperature": 30, "temperature_of_fire": 50, "on_fire_convert_to_material": 2,
			"on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 5},
		{"name": "cobblestone", "cell_type": "solid", "pattern": true, "density": 10, "hp": 1000, "tiles": [98, 98, 98], "transparent": false},
		{"name": "bricks", "cell_type": "solid", "pattern": true, "density": 9, "hp": 1200, "tiles": [99, 99, 99], "transparent": false},
	] + _clutter_defs() + _reserved_defs(116, 119) + [
		{"name": "crystal", "cell_type": "solid", "pattern": true, "density": 10, "hp": 800, "tiles": [104, 104, 104], "transparent": false},
	] + [
		# 121 (was reserved): still lava for cave pools and the deep lava sea, finite moving lava once something opens
		# beside or under it, like water_static (ADR 0069 C)
		{"name": "lava_static", "heat_c": 1200, "cell_type": "liquid", "fluid_slot": 1, "liquid_static": true, "density": 5, "temperature_of_fire": 150, "tiles": [7, 7, 7], "transparent": false, "loose": 7},
	] + [
		# 122, 123 (ADR 0110): the cores of conduit chambers, hard enough to need a good pick
		{"name": "spring_stone", "cell_type": "solid", "pattern": true, "density": 12, "hp": 2400, "tiles": [105, 105, 105], "transparent": false},
		{"name": "magma_heart", "cell_type": "solid", "pattern": true, "density": 12, "hp": 2400, "tiles": [106, 106, 106], "transparent": false},
	] + _giant_flower_defs() + _savanna_defs() + _looks_defs() + _reserved_defs(133, 136) + _fuse_defs() + [
		# 139 (ADR 0149): the lightning rod, a 2 m pole stamped from the placed voxel (PlantStamps.rod); draws strikes
		{"name": "lightning_rod", "cell_type": "solid", "pattern": true, "density": 11, "hp": 400, "tiles": [215, 215, 215], "transparent": false, "micro": "torch", "entity": "lightning_rod"},
	] + _reserved_defs(140, 140) + [
		# 141 (ADR 0157): a fresh meteorite's iron, glowing; Meteors cools it to meteoric_iron after Settings.meteor_hot_h
		{"name": "meteoric_iron_hot", "cell_type": "solid", "pattern": true, "density": 13, "hp": 1300, "tiles": [216, 216, 216], "transparent": false},
	] + _reserved_defs(142, 146) + [
		# 147, 148 (ADR 0157): what a meteorite leaves; Meteors writes them, the generator never does
		{"name": "meteoric_iron", "cell_type": "solid", "pattern": true, "density": 13, "hp": 1300, "tiles": [186, 186, 186], "transparent": false},
		{"name": "scorched_earth", "cell_type": "powder", "liquid_static": true, "density": 4, "tiles": [187, 187, 187], "transparent": false},
	] + _reserved_defs(149, 159) + _rock_defs() + [
		# 203 (ADR 0065): TNT. Not burnable: the sim would burn it away; Tnt lights it when fire touches it.
		{"name": "tnt", "cell_type": "solid", "pattern": true, "density": 6, "hp": 50, "tiles": [172, 171, 172], "transparent": false},
		# 204..206 (ADR 0098): the Gelvin monolith and its plaza. Not in mining.json and hp past Tnt.NEVER_HP: indestructible.
		{"name": "monolith", "cell_type": "solid", "pattern": true, "density": 15, "hp": 1000000, "tiles": [173, 173, 173], "transparent": false},
		{"name": "monolith_rune", "cell_type": "solid", "pattern": true, "density": 15, "hp": 1000000, "tiles": [174, 174, 174], "transparent": false},
		{"name": "monolith_seam", "cell_type": "solid", "pattern": true, "density": 15, "hp": 1000000, "tiles": [175, 175, 175], "transparent": false},
	] + _wood_defs() + [
		# 216 (ADR 0121): storage drawn by StationEntity like the chest
		{"name": "barrel", "cell_type": "solid", "density": 8, "hp": 200, "tiles": [39, 39, 39], "transparent": false, "interact": "chest", "entity": "barrel"},
		{"name": "storage_shelf", "cell_type": "solid", "density": 8, "hp": 200, "tiles": [39, 39, 39], "transparent": false, "interact": "chest", "entity": "storage_shelf"},
	] + _waste_defs() + _reserved_defs(222, CAVE_IDS - 1) + _cave_defs() + _plant_defs() + _structure_defs() + _flora_defs() + _reserved_defs(CAVE_IDS + 22, 253) + [
		# 254 (ADR 0119 C): what toxic sludge boils into: heavy, sinks and pools in low caves, fades in a minute
		{"name": "toxic_gas", "condense_c": 30, "condenses_to": "toxic_sludge", "cell_type": "gas", "density": 1, "gas_upwards_speed": 40, "gas_horizontal_speed": 90, "gas_downwards_speed": 120, "lifetime": 1800, "tiles": [11, 11, 11], "transparent": true},
	] + _reserved_defs(255, 255)


## Ids 218..221 (ADR 0141): factory waste as matter. Fouled water has water's look and flow in its own fluid slot;
## react_awake keeps it rolling its reactions at rest (reactions()). Atlas tiles 190, 191, 215.
## Ids 137..138 (ADR 0150): fuse cord and its spark. A burning cord lights every face or edge neighbour of cord
## after fuse_ticks ticks (native fuse rule): a steady Settings.fuse_speed_mps, up steps, round diagonals and
## under water. Its fire is colder (12) than anything else catches (15+) and its flame is the spark
## (temperature 0, still), so the ground beside it never burns. fire_hp is short: the glow is a moving spark.
static func _fuse_defs() -> Array:
	var ft := clampf(Settings.sim_tick_hz * Settings.VOXEL_M / maxf(Settings.fuse_speed_mps, 0.01), 1.0, 250.0)
	return [
		{"name": "fuse_cord", "cell_type": "solid", "density": 6, "hp": 5, "burnable": true, "fire_hp": maxi(24, ceili(ft) + 4), "fuse_ticks": ft, "ignite_chance": 0.5,
			"autoignition_temperature": 10, "temperature_of_fire": 12, "on_fire_convert_to_material": 8, "on_fire_flame_material": FUSE_SPARK,
			"on_fire_smoke_material": 11, "generates_smoke": 40, "tiles": [199, 199, 199], "transparent": false},
		{"name": "fuse_spark", "cell_type": "fire", "density": 0, "lifetime": 8, "temperature_of_fire": 0, "gas_upwards_speed": 0,
			"gas_horizontal_speed": 0, "tiles": [12, 12, 12], "transparent": true},
	]


static func _waste_defs() -> Array:
	return [
		{"name": "fouled_water", "boil_c": 100, "boils_to": "steam", "boil_rise_c": 60, "cell_type": "liquid", "fluid_slot": 5, "density": 0.95, "liquid_flow_speed": 1.0, "liquid_speed": 8, "film_lifetime_ticks": 300, "tiles": [5, 5, 5], "transparent": true, "react_awake": true},
		{"name": "waste_ground", "cell_type": "solid", "density": 8, "hp": 60, "tiles": [190, 190, 190], "transparent": false},
		{"name": "slag", "cell_type": "powder", "density": 6, "tiles": [191, 191, 191], "transparent": false},
		{"name": "chimney", "cell_type": "solid", "pattern": true, "density": 9, "hp": 1200, "tiles": [215, 99, 215], "transparent": false},
	]


## Ids 207..215 (ADR 0112): planks of the other woods (the oak planks' rules, their own tile 176..179),
## then a sapling per WOOD_KINDS: a micro sprout of the kind's bark and leaf tiles [leaf, bark, bark].
static func _wood_defs() -> Array:
	var out := []
	var oak: Dictionary = _planks_def()
	for i in 4:
		var d := oak.duplicate()
		d["name"] = "%s_planks" % WOOD_KINDS[i + 1]
		d["tiles"] = [176 + i, 176 + i, 176 + i]
		out.append(d)
	var plant_burn := {"burnable": true, "fire_hp": 40, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2}
	for k in [["oak", 20, 85], ["spruce", 59, 57], ["acacia", 62, 60], ["jungle", 65, 63], ["birch", 75, 73]]:
		var d := {"name": "%s_sapling" % k[0], "cell_type": "solid", "density": 10, "hp": 5, "tiles": [k[1], k[2], k[2]], "transparent": false, "micro": "sapling", "floor_only": true, "sapling": k[0]}
		d.merge(plant_burn)
		out.append(d)
	return out


## Ids 238..: what the generator's structures are built of. 238 rope (rope bridges over ravines).
static func _structure_defs() -> Array:
	return [
		{"name": "rope", "cell_type": "solid", "density": 6, "hp": 30, "burnable": true, "fire_hp": 30, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2, "tiles": [199, 199, 199], "transparent": false},
	]


## Ids 239..249: each biome's own flora (FineGen's FLORA table), tufts and bushes like the grasses, tiles 200..203, 208..214.
static func _flora_defs() -> Array:
	var out := []
	for e in [
		["sagebrush", 200, "bush"],
		["desert_marigold", 201, "tuft"],
		["moss_campion", 202, "tuft"],
		["fire_lily", 203, "tuft"],
		["lingonberry", 208, "tuft"],
		["bromeliad", 209, "tuft"],
		["cattail", 210, "tuft"],
		["aloe", 211, "tuft"],
		["bluebell", 212, "tuft"],
		["frost_lichen", 213, "tuft"],
		["sea_aster", 214, "tuft"],
	]:
		out.append({"name": e[0], "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 50, "ignite_chance": 0.6, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 3, "tiles": [e[1], e[1], e[1]], "transparent": false, "micro": e[2], "replaceable": true})
	return out


## Ids 235..237: plants the generator lays per column (FineGen, the vegetation pass): dune grass on beaches, heather
## on the highlands and scrub, sedge through the tundra's snow. Tufts like tall grass, tiles 196..198.
static func _plant_defs() -> Array:
	var out := []
	for e in [["dune_grass", 196], ["heather", 197], ["tundra_sedge", 198]]:
		out.append({"name": e[0], "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 50, "ignite_chance": 0.6, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 3, "tiles": [e[1], e[1], e[1]], "transparent": false, "micro": "tuft", "replaceable": true})
	return out


## Ids 228..233 (228..243 is this lane): what the generator dresses cave floors and ceilings with (FineGen::cave_dressing):
## dripstone spikes, vines (tufts you walk through), glow berries, cave fungi and glowing magma crust.
## Tiles 192..194 and 204..206 (204..207 glow, bright texels only).
static func _cave_defs() -> Array:
	var plant_burn := {"burnable": true, "fire_hp": 40, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2}
	var vine := {"name": "cave_vine", "cell_type": "solid", "density": 10, "hp": 5, "tiles": [193, 193, 193], "transparent": false, "micro": "tuft", "replaceable": true}
	vine.merge(plant_burn)
	var berries := vine.duplicate()
	berries["name"] = "glow_berry_vine"
	berries["tiles"] = [204, 204, 204]
	var stem := {"name": "fungus_stem", "cell_type": "solid", "density": 8, "hp": 40, "tiles": [194, 194, 194], "transparent": false}
	stem.merge(plant_burn)
	var cap := {"name": "fungus_cap", "cell_type": "solid", "density": 6, "hp": 20, "tiles": [206, 206, 206], "transparent": false}
	cap.merge(plant_burn)
	return [
		{"name": "dripstone", "cell_type": "solid", "density": 10, "hp": 600, "tiles": [192, 192, 192], "transparent": false},
		vine, berries, stem, cap,
		{"name": "magma_rock", "cell_type": "solid", "density": 11, "hp": 900, "tiles": [205, 205, 205], "transparent": false},
		{"name": "rusted_rail", "cell_type": "solid", "pattern": true, "density": 12, "hp": 300, "tiles": [195, 195, 195], "transparent": false},
	]


static func _planks_def() -> Dictionary:
	return {"name": "planks", "cell_type": "solid", "rigid": "wood", "pattern": true, "density": 8, "burnable": true, "fire_hp": 120, "autoignition_temperature": 40, "temperature_of_fire": 50, "on_fire_convert_to_material": 8, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 4, "tiles": [39, 39, 39], "transparent": false}


## Giant flowers, ids 124..126: a stalk that fells like a trunk (rigid wood) under a head of petals round a
## heart that fall with it (rigid leaf); tiles of the plant stem, the poppy and the dandelion.
static func _giant_flower_defs() -> Array:
	var burn := {"burnable": true, "fire_hp": 60, "autoignition_temperature": 30, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 3}
	var out := [
		{"name": "giant_stem", "cell_type": "solid", "rigid": "wood", "density": 8, "hp": 60, "tiles": [29, 29, 29], "transparent": false},
		{"name": "giant_petal", "cell_type": "solid", "rigid": "leaf", "density": 6, "hp": 10, "tiles": [31, 31, 31], "transparent": false},
		{"name": "giant_heart", "cell_type": "solid", "rigid": "leaf", "density": 6, "hp": 10, "tiles": [30, 30, 30], "transparent": false},
	]
	for d in out:
		d.merge(burn)
	return out


## The savanna floor, ids 127..129 (bug report 2026-10-25 20-37-22): grass and tussocks that act like
## grass and tall grass (is_grass, is_tall_grass), and bare red earth dug like dirt.
static func _savanna_defs() -> Array:
	return [
		{"name": "savanna_grass", "cell_type": "powder", "liquid_static": true, "density": 3, "tiles": [109, 109, 1], "transparent": false},
		# tall grass, drier: catches and burns a little faster
		{"name": "savanna_tall_grass", "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 45, "ignite_chance": 0.7, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 3, "tiles": [110, 110, 110], "transparent": false, "micro": "tuft", "replaceable": true},
		{"name": "red_earth", "cell_type": "powder", "liquid_static": true, "density": 3, "tiles": [111, 111, 111], "transparent": false},
	]


## Placeholders for ids other branches append (2026-10-25 split: A 116-119, B 120-129, C 130-149,
## D 150-159; E from 160), so every branch keeps its ids whatever order they merge in; a merge swaps a
## placeholder for the real definition. Nothing draws, collides or generates them.
static func _reserved_defs(from: int, to: int) -> Array:
	var out := []
	for id in range(from, to + 1):
		out.append({"name": "reserved_%d" % id, "cell_type": "solid", "density": 10, "hp": 5, "tiles": [0, 0, 0], "transparent": true, "model": "empty"})
	return out


static var _looks := []

## Ids 130..132 (ADR 0053): the parts of multi-voxel torches and crops (PlantStamps). A torch is its
## flame (the "torch" voxel, the block entity's anchor) on a wrap of ember cloth on a stick; all of
## them belong to the torch's entity and break with it. Wheat ears crown ripe wheat stalks.
static func _looks_defs() -> Array:
	if _looks.is_empty():
		_looks.append_array([
			{"name": "torch_stick", "cell_type": "solid", "pattern": true, "density": 5, "hp": 5, "tiles": [118, 118, 118], "transparent": false, "micro": "torch", "entity": "torch"},
			{"name": "torch_wrap", "cell_type": "solid", "pattern": true, "density": 5, "hp": 5, "tiles": [119, 119, 119], "transparent": false, "micro": "torch", "entity": "torch"},
		])
		var ear := {"name": "wheat_ear", "cell_type": "solid", "pattern": true, "density": 10, "hp": 5, "tiles": [115, 115, 115], "transparent": false, "micro": "wheat:7", "crop": "wheat"}
		ear.merge(_crop_defs()[0], false)
		_looks.append(ear)
	return _looks


static var _rocks := []

## Ids 160..202 (ADR 0057): the six new rock kinds (stone and basalt make eight), a cobble for each
## kind but stone (which has cobblestone), cut-stone bricks for all eight, loose stones lying on the
## ground in each kind's colours (clutter pieces like the pebbles), new clutter, ruin pots, and pyrite
## and the ochres for part F. Hardness lives in mining.json.
static func _rock_defs() -> Array:
	if not _rocks.is_empty():
		return _rocks
	var plant_burn := {"burnable": true, "fire_hp": 40, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2}
	var wood_burn := {"burnable": true, "fire_hp": 60, "autoignition_temperature": 30, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2}
	var dens := {"granite": 11, "diorite": 11, "limestone": 9, "slate": 10, "marble": 11, "chalk": 7, "basalt": 11, "stone": 10}
	for i in ROCK_KINDS.size():
		var k: String = ROCK_KINDS[i]
		_rocks.append({"name": k, "cell_type": "solid", "density": dens[k], "hp": 1000, "tiles": [136 + i, 136 + i, 136 + i], "transparent": false})
	for i in COBBLE_KINDS.size():
		_rocks.append({"name": COBBLE_KINDS[i] + "_cobble", "cell_type": "solid", "pattern": true, "density": dens[COBBLE_KINDS[i]], "hp": 1000, "tiles": [142 + i, 142 + i, 142 + i], "transparent": false})
	for i in BRICK_KINDS.size():
		_rocks.append({"name": BRICK_KINDS[i] + "_bricks", "cell_type": "solid", "pattern": true, "density": dens[BRICK_KINDS[i]], "hp": 1200, "tiles": [149 + i, 149 + i, 149 + i], "transparent": false})
	for i in COBBLE_KINDS.size():
		_rocks.append({"name": COBBLE_KINDS[i] + "_stones", "cell_type": "solid", "pattern": true, "density": dens[COBBLE_KINDS[i]], "hp": 10, "tiles": [142 + i, 142 + i, 142 + i], "transparent": false, "clutter": COBBLE_KINDS[i] + "_stones"})
	var strawberries := {"name": "wild_strawberries", "cell_type": "solid", "pattern": true, "density": 8, "hp": 5, "tiles": [157, 157, 157], "transparent": false, "clutter": "flower"}   # on plant_stem leaves: one piece
	strawberries.merge(plant_burn)
	var pine_cone := {"name": "pine_cone", "cell_type": "solid", "pattern": true, "density": 6, "hp": 5, "tiles": [162, 162, 162], "transparent": false, "clutter": "pine_cone"}
	pine_cone.merge(wood_burn)
	var driftwood := {"name": "driftwood", "cell_type": "solid", "pattern": true, "density": 6, "hp": 10, "tiles": [163, 163, 163], "transparent": false, "clutter": "driftwood"}
	driftwood.merge(wood_burn)
	var feathers := {"name": "feathers", "cell_type": "solid", "pattern": true, "density": 2, "hp": 2, "tiles": [161, 161, 161], "transparent": false, "clutter": "feathers"}
	feathers.merge(plant_burn)
	_rocks.append_array([
		strawberries,
		{"name": "hazelnuts", "cell_type": "solid", "pattern": true, "density": 8, "hp": 5, "tiles": [158, 158, 158], "transparent": false, "clutter": "hazelnuts"},
		{"name": "acorns", "cell_type": "solid", "pattern": true, "density": 8, "hp": 5, "tiles": [159, 159, 159], "transparent": false, "clutter": "acorns"},
		{"name": "bones", "cell_type": "solid", "pattern": true, "density": 9, "hp": 20, "tiles": [160, 160, 160], "transparent": false, "clutter": "bones"},
		feathers,
		pine_cone,
		driftwood,
		{"name": "antler", "cell_type": "solid", "pattern": true, "density": 9, "hp": 20, "tiles": [164, 164, 164], "transparent": false, "clutter": "antler"},
		{"name": "ice_shard", "cell_type": "solid", "density": 9, "hp": 10, "tiles": [27, 27, 27], "transparent": false, "clutter": "ice_shard"},
		{"name": "cave_crystal", "cell_type": "solid", "pattern": true, "density": 12, "hp": 30, "tiles": [167, 167, 167], "transparent": false, "clutter": "cave_crystal"},
		{"name": "clay_pot", "cell_type": "solid", "pattern": true, "density": 8, "hp": 10, "tiles": [165, 165, 165], "transparent": false, "clutter": "clay_pot", "shatter": "ruin_pot"},
		{"name": "clay_pot_cracked", "cell_type": "solid", "pattern": true, "density": 8, "hp": 5, "tiles": [166, 166, 166], "transparent": false, "clutter": "clay_pot", "shatter": "ruin_pot"},
		{"name": "pyrite", "cell_type": "solid", "pattern": true, "density": 12, "hp": 600, "tiles": [168, 168, 168], "transparent": false},
		{"name": "red_ochre", "cell_type": "powder", "liquid_static": true, "density": 4, "tiles": [169, 169, 169], "transparent": false},
		{"name": "yellow_ochre", "cell_type": "powder", "liquid_static": true, "density": 4, "tiles": [170, 170, 170], "transparent": false},
	])
	return _rocks


static var _clutter := []

## Ids 111..115 (ADR 0047). "clutter" names the piece group: voxels of one group that touch are one
## piece (GroundPickups); "stem" parts never decide what a piece drops; "waterlogged" ones stand in water.
static func _clutter_defs() -> Array:
	if not _clutter.is_empty():
		return _clutter
	var plant_burn := {"burnable": true, "fire_hp": 40, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2}
	var stem := {"name": "plant_stem", "cell_type": "solid", "density": 10, "hp": 5, "tiles": [29, 29, 29], "transparent": false, "clutter": "flower", "stem": true}
	stem.merge(plant_burn)
	_clutter.append_array([
		stem,
		{"name": "kelp", "cell_type": "solid", "pattern": true, "density": 10, "hp": 5, "tiles": [101, 101, 101], "transparent": false, "clutter": "kelp", "waterlogged": true},
		{"name": "seagrass", "cell_type": "solid", "pattern": true, "density": 10, "hp": 5, "tiles": [102, 102, 102], "transparent": false, "clutter": "seagrass", "waterlogged": true},
		{"name": "seashell", "cell_type": "solid", "pattern": true, "density": 8, "hp": 10, "tiles": [103, 103, 103], "transparent": false, "clutter": "seashell"},
		{"name": "mushroom_stem", "cell_type": "solid", "pattern": true, "density": 10, "hp": 5, "tiles": [42, 42, 42], "transparent": false, "clutter": "mushroom", "stem": true},
	])
	return _clutter


static var _content2 := []

## Ids 90..101: birch log and leaves, blueberry bush, pumpkin, three "bloom"
## flowers (tiles [petal, centre, blades]) and bark blocks (the log's side on all
## faces) for fallen logs.
static func _content_pass2_defs() -> Array:
	if not _content2.is_empty():
		return _content2
	var log_burn := {"burnable": true, "fire_hp": 160, "autoignition_temperature": 40, "temperature_of_fire": 50, "on_fire_convert_to_material": 8, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 4}
	var plant_burn := {"burnable": true, "fire_hp": 40, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2}
	var birch := {"name": "birch_log", "cell_type": "solid", "rigid": "wood", "density": 10, "tiles": [74, 73, 74], "transparent": false}
	birch.merge(log_burn)
	var birch_leaves := {"name": "birch_leaves", "cell_type": "solid", "rigid": "leaf", "density": 10, "hp": 20, "tiles": [75, 75, 75], "transparent": true}
	birch_leaves.merge(plant_burn)
	birch_leaves["generates_smoke"] = 6
	var bush := {"name": "blueberry_bush", "cell_type": "solid", "density": 10, "hp": 10, "tiles": [100, 100, 100], "transparent": false, "micro": "berry_bush"}
	bush.merge(plant_burn)
	var pumpkin := {"name": "pumpkin", "cell_type": "solid", "density": 6, "hp": 60, "tiles": [78, 77, 77], "transparent": false}
	_content2.append_array([birch, birch_leaves, bush, pumpkin])
	for f in [["cornflower", 79, 30], ["oxeye_daisy", 80, 30], ["allium", 81, 81]]:
		var d := {"name": f[0], "cell_type": "solid", "density": 10, "hp": 5, "tiles": [f[1], f[1], f[1]], "transparent": false, "clutter": "flower", "floor_only": true}
		d.merge(plant_burn)
		_content2.append(d)
	for b in [["oak_bark", 85], ["spruce_bark", 57], ["acacia_bark", 60], ["jungle_bark", 63], ["birch_bark", 73]]:
		var d := {"name": b[0], "cell_type": "solid", "rigid": "wood", "pattern": true, "density": 10, "tiles": [b[1], b[1], b[1]], "transparent": false}
		d.merge(log_burn)
		_content2.append(d)
	# wildlife (ADR 0027): a ribcage with scraps of meat, tiles [meat, bone, blood]
	_content2.append({"name": "carcass", "cell_type": "solid", "density": 6, "hp": 20, "tiles": [83, 82, 47], "transparent": false, "micro": "carcass"})
	return _content2


static var _flint_age := []

## Ids 103..107 (ADR 0030). The torch flame is painted with the fire tile, so
## the terrain shader's fire glow lights it up.
static func _flint_age_defs() -> Array:
	if not _flint_age.is_empty():
		return _flint_age
	var grass_burn := {"burnable": true, "fire_hp": 60, "autoignition_temperature": 25, "temperature_of_fire": 55, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 4}
	var wood_burn := {"burnable": true, "fire_hp": 100, "autoignition_temperature": 35, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 3}
	var thatch := {"name": "thatch", "cell_type": "solid", "pattern": true, "rigid": "wood", "density": 4, "hp": 60, "tiles": [88, 89, 88], "transparent": false}
	thatch.merge(grass_burn)
	var wattle := {"name": "wattle", "cell_type": "solid", "rigid": "wood", "density": 6, "hp": 120, "tiles": [91, 6, 6], "transparent": false, "micro": "wattle"}
	wattle.merge(wood_burn)
	_flint_age.append_array([
		thatch,
		wattle,
		{"name": "daub_wall", "cell_type": "solid", "rigid": "wood", "density": 9, "hp": 400, "tiles": [90, 90, 90], "transparent": false},
		# the flame of a multi-voxel torch (ADR 0053): tile 120 glows and flickers (flame_tile); stands on floors and hangs on walls
		{"name": "torch", "cell_type": "solid", "pattern": true, "density": 5, "hp": 5, "tiles": [120, 120, 120], "transparent": false, "micro": "torch", "entity": "torch", "light": true},
		{"name": "storage_pot", "cell_type": "solid", "density": 9, "hp": 80, "tiles": [66, 3, 66], "transparent": false, "micro": "storage_pot", "interact": "chest", "entity": "storage_pot"},
	])
	return _flint_age


static var _content := []

## Ids 76..87: logs and leaves burn like oak's, terracotta and basalt are stone-like,
## packed ice melts slower than ice, ferns burn like tall grass.
static func _content_pass_defs() -> Array:
	if not _content.is_empty():
		return _content
	var log_burn := {"burnable": true, "fire_hp": 160, "autoignition_temperature": 40, "temperature_of_fire": 50, "on_fire_convert_to_material": 8, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 4}
	var leaf_burn := {"burnable": true, "fire_hp": 40, "autoignition_temperature": 30, "temperature_of_fire": 55, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 6}
	for k in [["spruce", 57, 58, 59], ["acacia", 60, 61, 62], ["jungle", 63, 64, 65]]:
		var trunk := {"name": "%s_log" % k[0], "cell_type": "solid", "rigid": "wood", "density": 10, "tiles": [k[2], k[1], k[2]], "transparent": false}
		trunk.merge(log_burn)
		var leaves := {"name": "%s_leaves" % k[0], "cell_type": "solid", "rigid": "leaf", "density": 10, "hp": 20, "tiles": [k[3], k[3], k[3]], "transparent": true}
		leaves.merge(leaf_burn)
		_content.append_array([trunk, leaves])
	var fern := {"name": "fern", "cell_type": "solid", "density": 10, "hp": 5, "tiles": [72, 29, 72], "transparent": false, "micro": "fern", "replaceable": true}
	fern.merge(leaf_burn)
	fern["generates_smoke"] = 3
	fern.merge({"fire_hp": 60, "ignite_chance": 0.6}, true)   # the forest floor carries a fire like tall grass (bug report 2026-10-27 18-12-07)
	_content.append_array([
		{"name": "terracotta", "cell_type": "solid", "density": 10, "hp": 500, "tiles": [66, 66, 66], "transparent": false},
		{"name": "terracotta_red", "cell_type": "solid", "density": 10, "hp": 500, "tiles": [67, 67, 67], "transparent": false},
		{"name": "terracotta_cream", "cell_type": "solid", "density": 10, "hp": 500, "tiles": [68, 68, 68], "transparent": false},
		{"name": "basalt", "cell_type": "solid", "density": 11, "hp": 1100, "tiles": [69, 70, 69], "transparent": false},
		{"name": "packed_ice", "cell_type": "solid", "density": 9, "hp": 200, "tiles": [71, 71, 71], "transparent": false},
		fern,
	])
	return _content


static var _crops := []

## wheat_0..wheat_7 and carrots_0..carrots_3: the anchor voxel of a multi-voxel crop (ADR 0053,
## PlantStamps) and its stalks and leaves; the tile follows the age (green, turning, golden; carrot
## leaves, the orange shoulder of a ripe carrot). Burnable like tall grass.
const WHEAT_TILES := [112, 112, 112, 112, 112, 113, 113, 114]
const CARROT_TILES := [116, 116, 116, 117]

static func _crop_defs() -> Array:
	if _crops.is_empty():
		var burn := {"burnable": true, "fire_hp": 30, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2}
		for age in 8:
			var t: int = WHEAT_TILES[age]
			var d := {"name": "wheat_%d" % age, "cell_type": "solid", "pattern": true, "density": 10, "hp": 5, "tiles": [t, t, t], "transparent": false, "micro": "wheat:%d" % age, "crop": "wheat"}
			d.merge(burn)
			_crops.append(d)
		for look in 4:
			var t: int = CARROT_TILES[look]
			var d := {"name": "carrots_%d" % look, "cell_type": "solid", "pattern": true, "density": 10, "hp": 5, "tiles": [t, t, t], "transparent": false, "micro": "carrots:%d" % look, "crop": "carrots"}
			d.merge(burn)
			_crops.append(d)
	return _crops


static func reactions() -> Array:
	return [
		# ADR 0081: lava boils the water it touches (the water cell becomes steam, the lava stays hot) far more often than
		# it crusts into obsidian: a lava cell boils about six cells of water before it is quenched, so the steam cloud
		# grows with the water. Same pair twice: the sim rolls them in order (SimWorld::Reaction::next).
		{"input_cell1": LAVA, "input_cell2": WATER, "output_cell1": LAVA, "output_cell2": STEAM, "probability": 0.6},
		{"input_cell1": LAVA, "input_cell2": WATER, "output_cell1": OBSIDIAN, "output_cell2": STEAM, "probability": 0.25},
		{"input_cell1": LAVA, "input_cell2": WATER_STATIC, "output_cell1": LAVA, "output_cell2": STEAM, "probability": 0.6},
		{"input_cell1": LAVA, "input_cell2": WATER_STATIC, "output_cell1": OBSIDIAN, "output_cell2": STEAM, "probability": 0.25},
		{"input_cell1": LAVA_STATIC, "input_cell2": WATER, "output_cell1": LAVA_STATIC, "output_cell2": STEAM, "probability": 0.6},   # a still pool a stream of water reaches
		{"input_cell1": LAVA_STATIC, "input_cell2": WATER, "output_cell1": OBSIDIAN, "output_cell2": STEAM, "probability": 0.25},
		# ADR 0141 (was Noita's dilution to clean water): sludge fouls the water it touches
		{"input_cell1": TOXIC_SLUDGE, "input_cell2": WATER, "output_cell1": FOULED_WATER, "output_cell2": FOULED_WATER, "probability": 0.15, "passive": true},
		{"input_cell1": TOXIC_SLUDGE_STATIC, "input_cell2": WATER, "output_cell1": FOULED_WATER, "output_cell2": FOULED_WATER, "probability": 0.15, "passive": true},
		{"input_cell1": TOXIC_SLUDGE, "input_cell2": WATER_STATIC, "output_cell1": FOULED_WATER, "output_cell2": FOULED_WATER, "probability": 0.15, "passive": true},
		{"input_cell1": TOXIC_SLUDGE, "input_cell2": FOULED_WATER, "output_cell1": FOULED_WATER, "output_cell2": FOULED_WATER, "probability": 0.05, "passive": true},
	] + waste_reactions() + [
		{"input_cell1": FIRE, "input_cell2": WATER, "output_cell1": AIR, "output_cell2": STEAM, "probability": 0.6},
		{"input_cell1": LAVA, "input_cell2": OIL, "output_cell1": LAVA, "output_cell2": FIRE, "probability": 0.3},
		{"input_cell1": LAVA, "input_cell2": WOOD, "output_cell1": LAVA, "output_cell2": FIRE, "probability": 0.1},
		{"input_cell1": LAVA, "input_cell2": SPRUCE_LOG, "output_cell1": LAVA, "output_cell2": FIRE, "probability": 0.1},
		{"input_cell1": LAVA, "input_cell2": ACACIA_LOG, "output_cell1": LAVA, "output_cell2": FIRE, "probability": 0.1},
		{"input_cell1": LAVA, "input_cell2": JUNGLE_LOG, "output_cell1": LAVA, "output_cell2": FIRE, "probability": 0.1},
		{"input_cell1": LAVA, "input_cell2": BIRCH_LOG, "output_cell1": LAVA, "output_cell2": FIRE, "probability": 0.1},
		{"input_cell1": FIRE, "input_cell2": ICE, "output_cell1": AIR, "output_cell2": WATER, "probability": 0.4},
		{"input_cell1": LAVA, "input_cell2": ICE, "output_cell1": STONE, "output_cell2": STEAM, "probability": 0.5},
		{"input_cell1": FIRE, "input_cell2": PACKED_ICE, "output_cell1": AIR, "output_cell2": WATER, "probability": 0.1},
		{"input_cell1": LAVA, "input_cell2": PACKED_ICE, "output_cell1": STONE, "output_cell2": STEAM, "probability": 0.3},
	]


## ADR 0141: fouled water creeps through the water it touches; a generated (still) body, river, lake or sea, dilutes
## it back a little faster than it spreads, so a plume dies out once the sludge stops, while a dug pond of moving water
## stays fouled. Sludge eats the soil and soft rock it rests on into waste ground (all "passive": the partner does not wake
## or roll, only the sludge does; one layer deep, waste ground is not eaten) and settles out on gravel (a settling
## tank's bed) and slowly on its own waste ground. Probabilities per tick a pair touches; sludge and fouled water keep
## rolling at rest (react_awake), so each rule ends when its partner is used up.
static func waste_reactions() -> Array:
	var out := [
		{"input_cell1": FOULED_WATER, "input_cell2": WATER_STATIC, "output_cell1": FOULED_WATER, "output_cell2": FOULED_WATER, "probability": 0.001, "passive": true},
		{"input_cell1": FOULED_WATER, "input_cell2": WATER_STATIC, "output_cell1": WATER_STATIC, "output_cell2": WATER_STATIC, "probability": 0.0015, "passive": true},
		{"input_cell1": FOULED_WATER, "input_cell2": WATER, "output_cell1": FOULED_WATER, "output_cell2": FOULED_WATER, "probability": 0.002, "passive": true},
		{"input_cell1": TOXIC_SLUDGE, "input_cell2": GRAVEL, "output_cell1": WASTE_GROUND, "output_cell2": GRAVEL, "probability": 0.02, "passive": true},
		{"input_cell1": TOXIC_SLUDGE, "input_cell2": GRAVEL_STATIC, "output_cell1": WASTE_GROUND, "output_cell2": GRAVEL_STATIC, "probability": 0.02, "passive": true},
		{"input_cell1": TOXIC_SLUDGE, "input_cell2": WASTE_GROUND, "output_cell1": WASTE_GROUND, "output_cell2": WASTE_GROUND, "probability": 0.004, "passive": true},
	]
	for soft in ERODES:
		out.append({"input_cell1": TOXIC_SLUDGE, "input_cell2": id_of(soft), "output_cell1": TOXIC_SLUDGE, "output_cell2": WASTE_GROUND, "probability": 0.003, "passive": true})
	return out


## What sludge eats into waste ground (ADR 0141): soil and the soft rocks.
const ERODES := ["dirt", "grass", "clay", "sandstone", "farmland", "farmland_moist", "limestone", "chalk", "red_earth", "savanna_grass"]


## The material a terrain voxel value stands for (partly full liquids -> their liquid).
static func material_of_voxel(v: int) -> int:
	if v < FLUID_LEVEL_BASE:
		return v
	return _slot_materials().get((v - FLUID_LEVEL_BASE) / 8, 0)


## Eighths of liquid in a terrain voxel value (8 for anything that is not a partial level).
static func eighths_of_voxel(v: int) -> int:
	return (v - FLUID_LEVEL_BASE) % 8 + 1 if v >= FLUID_LEVEL_BASE else 8


static var _slot_mats := {}

## fluid_slot -> the moving liquid (static kinds share their liquid's slot).
static func _slot_materials() -> Dictionary:
	if _slot_mats.is_empty():
		var d := defs()
		for i in d.size():
			if d[i].has("fluid_slot") and not d[i].get("liquid_static", false) and not _slot_mats.has(int(d[i]["fluid_slot"])):
				_slot_mats[int(d[i]["fluid_slot"])] = i + 1
	return _slot_mats


static var _blocks_bodies := PackedByteArray()

## True when bodies collide with this material, as the library's collision masks
## say (build_library): solid and powder cubes, stalactites, SOLID_MICRO models
## and furniture voxels. Plants, pickups and work spots are walked through.
static func blocks_bodies(id: int) -> bool:
	if _blocks_bodies.is_empty():
		var d := defs()
		_blocks_bodies.resize(d.size() + 1)
		for i in d.size():
			var e: Dictionary = d[i]
			var ct: String = e.get("cell_type", "solid")
			var solid := ct == "solid" or ct == "powder"
			if e.has("fluid_slot"):
				solid = false
			elif e.has("micro"):
				solid = e["micro"] in ["stalactite", "stalagmite"] or e["micro"] in SOLID_MICRO
			elif e.has("clutter"):
				solid = false   # clutter is walked through (ADR 0047)
			elif e.get("model", "") == "empty":
				solid = e.get("collision", false) is bool and e.get("collision", false)
			_blocks_bodies[i + 1] = 1 if solid else 0
	return id > 0 and id < _blocks_bodies.size() and _blocks_bodies[id] == 1


static var _liquids := PackedByteArray()

## True for liquid cell types (water, lava, oil, their static kinds).
## Grass ground: meadow grass or the savanna's straw grass (both are hoed, grazed and spawned on).
static func is_grass(id: int) -> bool:
	return id == GRASS or id == SAVANNA_GRASS


## Ground a sapling takes root in: grass, dirt, red earth, leaf litter.
static func is_soil(id: int) -> bool:
	return is_grass(id) or id == DIRT or id == RED_EARTH or id == LEAF_LITTER


## A tuft of grass: meadow or savanna (both are eaten by grazers and cut for dry grass).
static func is_tall_grass(id: int) -> bool:
	return id == TALL_GRASS or id == SAVANNA_TALL_GRASS


static func is_liquid(id: int) -> bool:
	if _liquids.is_empty():
		var d := defs()
		_liquids.resize(d.size() + 1)
		for i in d.size():
			_liquids[i + 1] = 1 if d[i].get("cell_type", "solid") == "liquid" else 0
	return id > 0 and id < _liquids.size() and _liquids[id] == 1


## Powder cell types (sand, gravel, dirt, snow ...): a head buried in one suffocates.
static func is_powder(id: int) -> bool:
	return id > 0 and id <= defs().size() and defs()[id - 1].get("cell_type", "solid") == "powder"


## Waterlogged clutter (kelp, seagrass, ADR 0047): a body in it is in water.
static func is_waterlogged(id: int) -> bool:
	return id > 0 and id <= defs().size() and defs()[id - 1].get("waterlogged", false)


static var _ids := {}

## Material id for a name (0 when unknown).
## The atlas tiles drawn as patterns (ADR 0039): one texel per voxel, laid over the world grid, instead
## of a texel picked by the voxel's hash. Bit t of the result's int t / 32 is set for tile t: every tile
## of a material with "pattern": true (terrain_common.gdshaderinc, pattern_tiles).
static func pattern_tiles() -> PackedInt32Array:
	var bits := PackedInt32Array([0, 0, 0, 0, 0, 0, 0, 0])   # tiles 0..255 (terrain_common.gdshaderinc)
	for d in defs():
		if d.get("pattern", false):
			for t in d.get("tiles", []):
				bits[int(t) / 32] |= 1 << (int(t) % 32)
	return bits


static func id_of(name: String) -> int:
	if _ids.is_empty():
		var d := defs()
		for i in d.size():
			_ids[d[i]["name"]] = i + 1
	return _ids.get(name, 0)


## A crafting table, chest, furnace or storage pot (ADR 0034): a block entity
## whose footprint (BLOCK^3 voxels) only collides and whose view draws the block.
static func is_station(id: int) -> bool:
	if id <= 0 or id > defs().size():
		return false
	var d: Dictionary = defs()[id - 1]
	return d.has("interact") and d.has("entity") and d.get("entity") != "torch" and d.get("model", "") != "empty"


## True for a plain terrain voxel: a solid or powder cube with no entity,
## station or model of its own. Items placing one count in voxels (ADR 0034).
## The Gelvin monolith (ADR 0098): no dig, brush or paint touches it, not even in creative.
static func is_monolith(id: int) -> bool:
	return id >= MONOLITH and id <= MONOLITH_SEAM


static func is_plain_voxel(id: int) -> bool:
	if id <= 0 or id > defs().size():
		return false
	var d: Dictionary = defs()[id - 1]
	if d.has("micro") or d.has("clutter") or d.has("interact") or d.has("entity") or d.get("model", "") == "empty":
		return false
	return d.get("cell_type", "solid") in ["solid", "powder"]


## The still kind of a live powder (sand -> sand_static), the one the generator lays: a placed
## powder keeps its shape until something disturbs it, like the ground does (bug report
## 2026-10-26_19-38-35: sand built on a beach slid apart). Liquids and everything else stay as they are.
static func still_of(id: int) -> int:
	var d := defs()
	if id <= 0 or id > d.size() or d[id - 1].get("cell_type", "") != "powder":
		return id
	for i in d.size():
		if int(d[i].get("loose", 0)) == id and d[i].get("liquid_static", false):
			return i + 1
	return id


static func name_of(id: int) -> String:
	if id == 0:
		return "air"
	var d := defs()
	return d[id - 1]["name"] if id - 1 < d.size() else "?"


## Builds the godot_voxel blocky library with atlas tiles per face.
## Materials with a "micro" key are patterns of 6.25 cm cells (MicroMesher, ADR
## 0006) instead of a full cube: vegetation and detail that is still voxels.
## "model": "empty" is the reserved CHISELED id: nothing drawn by the terrain
## mesher, neighbours keep their faces, the shape comes from the block entity.
## `liquid_material`: see-through material for non-powder liquids, gases and
## fire (null = draw them opaque with the atlas material).
## `fluid_materials`: fluid_slot -> material for Minecraft-style liquid surfaces
## (terrain_fluid.gdshader); empty = liquids stay cubes. With them, every liquid
## with a fluid_slot is a VoxelBlockyModelFluid at full level, and the partial
## levels follow at FLUID_LEVEL_BASE (empty models fill the gap).
## `gas_material`: soft cloud material for gases (terrain_gas.gdshader); null = the liquid material.
static func build_library(atlas_material: Material, liquid_material: Material = null, fluid_materials: Dictionary = {}, gas_material: Material = null, gas_materials: Dictionary = {}) -> VoxelBlockyLibrary:
	var lib := VoxelBlockyLibrary.new()
	var models: Array = [VoxelBlockyModelEmpty.new()]
	var fluids := {}   # slot -> VoxelBlockyFluid
	for slot in fluid_materials:
		var f := VoxelBlockyFluid.new()
		f.material = fluid_materials[slot]
		f.dip_when_flowing_down = true
		fluids[slot] = f
	for d in defs():
		if d.has("fluid_slot") and fluids.has(int(d["fluid_slot"])):
			models.append(_fluid_model(fluids[int(d["fluid_slot"])], 8, d.get("transparent", false)))
			continue
		# Stations, chests, furnaces and pots (ADR 0034): their footprint voxels only
		# collide; the block entity draws the block over the whole footprint.
		if d.has("interact") and d.get("model", "") != "empty":
			var station := VoxelBlockyModelEmpty.new()
			station.culls_neighbors = false
			station.collision_aabbs = [AABB(Vector3.ZERO, Vector3.ONE)]
			models.append(station)
			continue
		if d.get("model", "") == "empty":
			var empty := VoxelBlockyModelEmpty.new()
			empty.culls_neighbors = false
			if d.get("collision", false) is String:
				# work spots (ADR 0023): only the aim ray hits the box, bodies walk through
				empty.collision_aabbs = [AABB(Vector3.ZERO, Vector3.ONE)]
				empty.collision_mask = MASK_TARGET
			elif d.get("collision", false):
				empty.collision_aabbs = [AABB(Vector3.ZERO, Vector3.ONE)]   # furniture: solid to bodies, drawn by its entity
			models.append(empty)
			continue
		var cube := VoxelBlockyModelCube.new()
		cube.atlas_size_in_tiles = ATLAS_TILES
		var t: Array = d["tiles"]
		var top := Vector2i(t[0] % ATLAS_TILES.x, t[0] / ATLAS_TILES.x)
		var side := Vector2i(t[1] % ATLAS_TILES.x, t[1] / ATLAS_TILES.x)
		var bottom := Vector2i(t[2] % ATLAS_TILES.x, t[2] / ATLAS_TILES.x)
		cube.set_tile(VoxelBlockyModel.SIDE_POSITIVE_Y, top)
		cube.set_tile(VoxelBlockyModel.SIDE_NEGATIVE_Y, bottom)
		for s in [VoxelBlockyModel.SIDE_NEGATIVE_X, VoxelBlockyModel.SIDE_POSITIVE_X, VoxelBlockyModel.SIDE_NEGATIVE_Z, VoxelBlockyModel.SIDE_POSITIVE_Z]:
			cube.set_tile(s, side)
		# Movement collision (ADR 0011, VoxelBody): only solids and powders
		# block bodies; liquids, gases and fire are swum or walked through.
		var ct: String = d.get("cell_type", "solid")
		var passable: bool = not (ct == "solid" or ct == "powder")
		# Plants, pickups and detail (the old 6.25 cm "micro" models) are plain
		# voxels at 12.5 cm (ADR 0033): walked through unless blocks_bodies says so.
		if (d.has("micro") or d.has("clutter")) and not blocks_bodies(defs().find(d) + 1):
			passable = true
		# Transparent materials share transparency index 1: faces between two
		# voxels of the same index are culled (no water-water faces), faces
		# against air or opaque voxels are kept. culls_neighbors stays true.
		# Passable transparent ones (water, oil, gases, fire) get the alpha material.
		# Leaves and plants are opaque voxels now; only see-through passables keep the index.
		if d.get("transparent", false) and passable and not d.has("micro"):
			cube.transparency_index = 1
		cube.set_material_override(0, liquid_material if (liquid_material != null and passable and d.get("transparent", false)) else atlas_material)
		if gas_material != null and ct in ["gas", "fire"]:
			# a cloud of its own index: water under steam keeps its surface, gas against gas is culled;
			# its own copy of the gas material where data/gas_looks.json gives it a look (ADR 0119 C4)
			cube.transparency_index = 2
			cube.set_material_override(0, gas_materials.get(d["name"], gas_material))
		if passable:
			cube.collision_mask = 0
			cube.set_mesh_collision_enabled(0, false)
			if defs().find(d) + 1 in GROUND_PICKUPS or d.has("micro") or d.has("clutter"):
				# a box only the aim ray hits: sticks and stones are aimed at, dug and picked up whole,
				# but walked through (ADR 0043); plants too, or the ray passes through grass, ferns and
				# reeds into the ground and they never drop their fiber (bug report 2026-10-24)
				cube.collision_aabbs = [AABB(Vector3.ZERO, Vector3.ONE)]
				cube.collision_mask = MASK_TARGET
		models.append(cube)
	if not fluids.is_empty():
		while models.size() < FLUID_LEVEL_BASE:
			models.append(VoxelBlockyModelEmpty.new())
		var transparent := {}
		for d in defs():
			if d.has("fluid_slot"):
				transparent[int(d["fluid_slot"])] = d.get("transparent", false)
		for slot in range(0, fluids.keys().max() + 1):
			for e in range(1, 9):
				if e < 8 and fluids.has(slot):
					models.append(_fluid_model(fluids[slot], e, transparent.get(slot, false)))
				else:
					models.append(VoxelBlockyModelEmpty.new())
	lib.models = models
	lib.bake()
	return lib


static func _fluid_model(fluid: VoxelBlockyFluid, level: int, transparent: bool) -> VoxelBlockyModelFluid:
	var m := VoxelBlockyModelFluid.new()
	m.fluid = fluid
	m.level = level
	m.collision_mask = 0   # swum through, like every liquid (ADR 0011)
	if transparent:
		m.transparency_index = 1
	return m


## Micro-voxel model (ADR 0006): a pattern on the MICRO_GRID^3 cell grid (6.25 cm
## cells), meshed by the native MicroMesher with only exposed faces, each cell
## showing its 1/8 of an opaque atlas tile. Cells hold "paint" = atlas tile + 1
## (0 = air), so stems and petals can use colour tiles that are not materials.
## Plants never cull neighbours (the ground under them stays visible) and have
## no collision; stalactites/stalagmites get a core collision box.
## tiles: the material's [top, side, bottom] tiles; patterns read what they need.
static func _micro_model(kind: String, tiles: Array, material: Material) -> VoxelBlockyModelMesh:
	var cells := micro_pattern(kind, tiles)
	# MicroMesher winds counter-clockwise; Godot's front faces are clockwise (MobModel.outward)
	var mesh: ArrayMesh = MobModel.outward(MicroMesher.build(cells, Vector3i(MICRO_GRID, MICRO_GRID, MICRO_GRID), MICRO_GRID, micro_paint_tiles(), ATLAS_TILES, Vector3.ZERO))
	var model := VoxelBlockyModelMesh.new()
	model.mesh = mesh
	model.set_material_override(0, material)
	model.culls_neighbors = false
	if kind == "stalactite" or kind == "stalagmite":
		model.collision_aabbs = [AABB(Vector3(0.25, 0.0, 0.25), Vector3(0.5, 1.0, 0.5))]
	elif kind in SOLID_MICRO:
		model.collision_aabbs = [_cells_aabb(cells)]   # walls and pots block bodies (ADR 0030)
	else:
		# a box only the aim ray sees (VoxelBody collides with MASK_SOLID only): plants and
		# pickups can be broken and picked up but are walked through (ADR 0022)
		model.collision_aabbs = [_cells_aabb(cells)]
		model.collision_mask = MASK_TARGET
	model.set_mesh_collision_enabled(0, false)
	return model


## Bounds of the filled cells of a micro pattern, in voxel units (at least 1/4 voxel tall,
## so thin carpets and lily pads stay easy to aim at).
static func _cells_aabb(cells: PackedInt32Array) -> AABB:
	var g := MICRO_GRID
	var lo := Vector3i(g, g, g)
	var hi := Vector3i(-1, -1, -1)
	for i in cells.size():
		if cells[i] != 0:
			var p := Vector3i(i % g, (i / g) % g, i / (g * g))
			lo = lo.min(p)
			hi = hi.max(p)
	if hi.x < 0:
		return AABB(Vector3.ZERO, Vector3.ONE)
	var a := Vector3(lo) / g
	var b := Vector3(hi + Vector3i.ONE) / g
	b.y = maxf(b.y, a.y + 0.25)
	return AABB(a, b - a)


## paint id -> atlas tile for MicroMesher (paint = tile + 1).
static func micro_paint_tiles() -> PackedInt32Array:
	var t := PackedInt32Array()
	t.resize(ATLAS_TILES.x * ATLAS_TILES.y + 1)
	t[0] = -1
	for i in range(1, t.size()):
		t[i] = i - 1
	return t


## The cell pattern of a micro model kind. Deterministic per (kind, tiles).
static func micro_pattern(kind: String, tiles: Array) -> PackedInt32Array:
	var g := MICRO_GRID
	var c := PackedInt32Array()
	c.resize(g * g * g)
	c.fill(0)
	var rng := RandomNumberGenerator.new()
	rng.seed = hash(kind + str(tiles))
	var body: int = tiles[0] + 1
	var stage := 0
	if kind.contains(":"):
		# crops: "wheat:<age>", "carrots:<look>"; the plant positions are seeded per crop, not per stage,
		# so a field grows in place instead of reshuffling every stage
		stage = int(kind.get_slice(":", 1))
		kind = kind.get_slice(":", 0)
		rng.seed = hash(kind)
	match kind:
		"rug":
			# carpet: one 6.25 cm layer over the whole floor
			for x in g:
				for z in g:
					_cell(c, x, 0, z, body)
		"wheat":
			# Minecraft's 8 stages: 12 stalks on a loose 4x3 grid grow from 1 to 8 cells; green until age 5,
			# gold heads from age 5, all gold at age 7 (ripe)
			var stalk: int = tiles[1] + 1
			var ripe := body
			var hgt: int = [1, 2, 3, 4, 5, 6, 7, 8][stage]
			for i in 12:
				var x := (i % 4) * 2 + rng.randi_range(0, 1)
				var z := (i / 4) * 3 + rng.randi_range(0, 1)
				var h := clampi(hgt - rng.randi_range(0, 1), 1, g)
				var col := ripe if stage == 7 else stalk
				for y in h:
					_cell(c, x, y, z, col)
				if stage >= 5:
					_cell(c, x, h - 1, z, ripe)
					if h >= 2:
						_cell(c, x, h - 2, z, ripe)
				if stage >= 6 and rng.randf() < 0.5:
					_cell(c, x + rng.randi_range(-1, 1), h - 1, z, ripe)   # heavy heads lean
		"carrots":
			# leafy tufts at 6 spots growing 1..4 cells; the ripe look shows the orange tops in the soil line
			var leaf: int = tiles[1] + 1
			var root := body
			var hgt: int = [1, 2, 3, 4][stage]
			for i in 6:
				var x := (i % 3) * 3 + rng.randi_range(0, 1)
				var z := (i / 3) * 4 + rng.randi_range(0, 2)
				for y in hgt:
					_cell(c, x, y, z, leaf)
				if stage >= 2:
					_cell(c, x + 1, hgt - 1, z, leaf)
					_cell(c, x, hgt - 1, z + 1, leaf)
				if stage == 3:
					_cell(c, x, 0, z, root)
					_cell(c, x + 1, 0, z, root)
		"wild_wheat":
			# a wild tuft: 7 gold stalks with heads among a few green blades
			var blade: int = tiles[1] + 1
			for i in 7:
				var x := rng.randi_range(1, g - 2)
				var z := rng.randi_range(1, g - 2)
				var h := rng.randi_range(5, 7)
				for y in h:
					_cell(c, x, y, z, body)
				_cell(c, x + rng.randi_range(-1, 1), h, z + rng.randi_range(-1, 1), body)
			for i in 5:
				var x := rng.randi_range(0, g - 1)
				var z := rng.randi_range(0, g - 1)
				for y in rng.randi_range(2, 4):
					_cell(c, x, y, z, blade)
		"wild_carrot":
			# Queen Anne's lace: 2 green stems with flat white flower umbels, an orange root top in the ground line
			var stem: int = tiles[1] + 1
			var root: int = tiles[2] + 1
			for i in 2:
				var x := 2 + i * 3 + rng.randi_range(0, 1)
				var z := 2 + rng.randi_range(0, 3)
				var h := rng.randi_range(5, 6) - i
				for y in h:
					_cell(c, x, y, z, stem)
				for dx in range(-1, 2):
					for dz in range(-1, 2):
						_cell(c, x + dx, h, z + dz, body)
				_cell(c, x, 1, z + 1, stem)   # a leaf
				_cell(c, x + 1, 1, z + 1, stem)
				_cell(c, x, 0, z, root)
		"fern":
			# a rosette of 6 fronds from a short crown: each frond rises to 4-6 cells and arches
			# 3 cells outward, dropping a cell per step, with leaflets on both sides near the
			# crown; the crown uses the darker leaf-green tile
			var stem: int = tiles[1] + 1
			var cx := 3 + rng.randi_range(0, 1)
			var cz := 3 + rng.randi_range(0, 1)
			var dirs := [Vector2i(1, 0), Vector2i(1, 1), Vector2i(0, 1), Vector2i(-1, 1), Vector2i(-1, 0), Vector2i(-1, -1), Vector2i(0, -1), Vector2i(1, -1)]
			var start := rng.randi_range(0, 7)
			var crown := rng.randi_range(2, 3)
			for y in crown:
				_cell(c, cx, y, cz, stem)
			for i in 6:
				var dv: Vector2i = dirs[(start + i + i / 2) % 8]   # 6 of the 8 directions, spread round
				var x := cx
				var z := cz
				var y := crown + rng.randi_range(1, 3)
				for yy in range(crown, y):
					_cell(c, cx + dv.x * int(yy > crown), yy, cz + dv.y * int(yy > crown), body)
				for k in 3:
					x += dv.x
					z += dv.y
					_cell(c, x, y, z, body)
					if dv.x != 0 and dv.y != 0:
						_cell(c, x - dv.x, y, z, body)   # keep diagonals connected
					if k < 2:
						_cell(c, x + dv.y, y, z + dv.x, body)   # leaflets
						_cell(c, x - dv.y, y, z - dv.x, body)
					y -= 1
		"bloom":
			# a flower of tiles [petal, centre, blades]: a stem with a leaf, a petal plus around the
			# centre (alliums: a petal ball on a taller stem), a few grass blades
			var petal: int = tiles[0] + 1
			var centre: int = tiles[1] + 1
			var blade: int = tiles[2] + 1
			var stem: int = 29 + 1
			var ball: bool = tiles[0] == tiles[1]
			var sx := rng.randi_range(2, 5)
			var sz := rng.randi_range(2, 5)
			var top := rng.randi_range(6, 7) if ball else rng.randi_range(4, 6)
			for y in top:
				_cell(c, sx, y, sz, stem)
			_cell(c, sx + 1, 2, sz, stem)
			if ball:
				for dx in range(-1, 2):
					for dz in range(-1, 2):
						for dy in range(-1, 1):
							if abs(dx) + abs(dz) + abs(dy) <= 2:
								_cell(c, sx + dx, top + dy, sz + dz, petal)
			else:
				for dx in range(-1, 2):
					for dz in range(-1, 2):
						if abs(dx) + abs(dz) <= 1:
							_cell(c, sx + dx, top, sz + dz, petal)
				_cell(c, sx, top, sz, centre)
			for i in 4:
				var x := rng.randi_range(0, g - 1)
				var z := rng.randi_range(0, g - 1)
				for y in rng.randi_range(2, 4):
					_cell(c, x, y, z, blade)
		"carcass":
			# a half-eaten animal lying along x: spine, a ribcage open on one side, the skull at
			# one end, scraps of meat on the ribs and a dark stain under it
			var bone: int = tiles[1] + 1
			var stain: int = tiles[2] + 1
			for x in range(0, g):
				for z in range(1, g - 1):
					if rng.randf() < 0.35:
						_cell(c, x, 0, z, stain)
			for x in range(1, 7):
				_cell(c, x, 1, 4, bone)                     # spine
			for x in range(2, 6):
				if x % 2 == 0:
					for h in range(1, 4):
						_cell(c, x, h, 2, bone)          # ribs on the near side, standing
					_cell(c, x, 4, 3, bone)
					_cell(c, x, 1, 6, bone)              # far side: broken off short
					if rng.randf() < 0.7:
						_cell(c, x + 1, rng.randi_range(1, 2), 3 + rng.randi_range(0, 1), body)   # meat left on them
			_box_cells(c, Vector3i(6, 1, 3), Vector3i(8, 3, 6), bone)   # skull
			_cell(c, 7, 2, 4, 0)                            # eye socket
			_cell(c, 0, 1, 5, bone)                         # a leg bone
			_cell(c, 1, 1, 6, bone)
		"sapling":
			# a thin bark stem 6 cells tall with leaf tufts on top and two side twigs, tiles [leaf, bark]
			var bark: int = tiles[1] + 1
			for y in 6:
				_cell(c, 3, y, 3, bark)
			for o in [Vector3i(0, 0, 0), Vector3i(1, 0, 0), Vector3i(-1, 0, 0), Vector3i(0, 0, 1), Vector3i(0, 0, -1), Vector3i(0, 1, 0)]:
				_cell(c, 3 + o.x, 6 + o.y, 3 + o.z, body)
			for side in [[Vector3i(1, 0, 0), 3], [Vector3i(0, 0, -1), 4]]:
				var d: Vector3i = side[0]
				_cell(c, 3 + d.x, side[1], 3 + d.z, bark)
				_cell(c, 3 + 2 * d.x, side[1] + 1, 3 + 2 * d.z, body)
				_cell(c, 3 + 2 * d.x + d.z, side[1] + 1, 3 + 2 * d.z + d.x, body)
		"berry_bush":
			# a low leafy dome (leaf-green tile) dotted with berries (tile 0)
			var leaf: int = tiles[1] + 1
			for x in g:
				for z in g:
					for y in 6:
						var d := Vector3(x - 3.5, y * 1.2, z - 3.5).length()
						if d < 4.2 and rng.randf() < 0.8:
							_cell(c, x, y, z, body if (rng.randf() < 0.12 and d > 3.0) else leaf)
		"tuft":
			# 9-11 blades, 1 cell (6 cm) wide, 3-7 cells tall, some leaning at the top
			for i in rng.randi_range(9, 11):
				var x := rng.randi_range(0, g - 1)
				var z := rng.randi_range(0, g - 1)
				var hgt := rng.randi_range(3, 7)
				for y in hgt:
					_cell(c, x, y, z, body)
				if hgt >= 5 and rng.randf() < 0.6:
					_cell(c, x + rng.randi_range(-1, 1), hgt - 1, z + rng.randi_range(-1, 1), body)
				if rng.randf() < 0.3:
					_cell(c, x + 1, 0, z, body)
		"flower":
			# 1-cell stem with a leaf, a 3x3 plus of petals around a contrasting centre at ~35 cm, a few blades
			var petal: int = (31 if tiles[0] == 28 else 30) + 1
			var centre: int = (30 if tiles[0] == 28 else 31) + 1
			var stem: int = 29 + 1
			var sx := rng.randi_range(2, 5)
			var sz := rng.randi_range(2, 5)
			var top := rng.randi_range(4, 6)
			for y in top:
				_cell(c, sx, y, sz, stem)
			_cell(c, sx + 1, 2, sz, stem)
			_cell(c, sx + 1, 2, sz + 1, stem)
			for dx in range(-1, 2):
				for dz in range(-1, 2):
					if abs(dx) + abs(dz) <= 1:
						_cell(c, sx + dx, top, sz + dz, petal)
			_cell(c, sx, top, sz, centre)
			_cell(c, sx, top + 1, sz, petal) if top + 1 < g else null
			for i in 4:
				var x := rng.randi_range(0, g - 1)
				var z := rng.randi_range(0, g - 1)
				for y in rng.randi_range(2, 4):
					_cell(c, x, y, z, body)
		"bush":
			# dead bush: 1-cell trunk, 6 twigs rising diagonally
			var tx := rng.randi_range(3, 4)
			var tz := rng.randi_range(3, 4)
			for y in 4:
				_cell(c, tx, y, tz, body)
			for i in 6:
				var dx := rng.randi_range(-1, 1)
				var dz := rng.randi_range(-1, 1)
				if dx == 0 and dz == 0:
					dx = 1
				# twigs step out then up so every cell shares a face with the previous one
				var x := tx
				var z := tz
				var y := rng.randi_range(1, 3)
				for k in rng.randi_range(2, 4):
					x += dx
					z += dz
					_cell(c, x, y, z, body)
					if dx != 0 and dz != 0:
						_cell(c, x - dx, y, z, body)  # corner cell to keep the diagonal connected
					y += 1
					_cell(c, x, y, z, body)
		"pebbles":
			# 3-5 small stones of 1-2 cells, flat on the ground
			for i in rng.randi_range(3, 5):
				var x := rng.randi_range(0, g - 2)
				var z := rng.randi_range(0, g - 2)
				var w := rng.randi_range(1, 2)
				var d := rng.randi_range(1, 2)
				for dx in w:
					for dz in d:
						_cell(c, x + dx, 0, z + dz, body)
				if w == 2 and d == 2 and rng.randf() < 0.5:
					_cell(c, x, 1, z, body)
		"ore_stones":
			# 2-3 chunky stones (2x2 cells, some 2 tall) showing the ore tile: ore-bearing rocks
			for i in rng.randi_range(2, 3):
				var x := rng.randi_range(0, g - 3)
				var z := rng.randi_range(0, g - 3)
				var w := rng.randi_range(2, 3)
				for dx in w:
					for dz in 2:
						_cell(c, x + dx, 0, z + dz, body)
				if rng.randf() < 0.6:
					_cell(c, x, 1, z, body)
					_cell(c, x + 1, 1, z, body)
		"twigs":
			# 2-3 sticks lying flat, 4-6 cells long along x, z or a diagonal, one with a side branch
			for i in rng.randi_range(2, 3):
				var dirs := [Vector2i(1, 0), Vector2i(0, 1), Vector2i(1, 1), Vector2i(1, -1)]
				var dv: Vector2i = dirs[rng.randi_range(0, 3)]
				var x := rng.randi_range(1, 3)
				var z := rng.randi_range(1, 6) if dv.y < 0 else rng.randi_range(0, 3)
				var y := 1 if i == 2 else 0
				var n := rng.randi_range(4, 6)
				for k in n:
					_cell(c, x + dv.x * k, y, z + dv.y * k, body)
					if dv.x != 0 and dv.y != 0 and k > 0:
						_cell(c, x + dv.x * k - dv.x, y, z + dv.y * k, body)  # keep diagonals connected
				if i == 0:
					_cell(c, x + dv.x * 2 + dv.y, y, z + dv.y * 2 + dv.x, body)
		"mushroom":
			# cap tile = tiles[0], stem tile = tiles[1]; one big and one small mushroom
			var cap: int = tiles[0] + 1
			var stem: int = tiles[1] + 1
			var mx := rng.randi_range(1, 4)
			var mz := rng.randi_range(1, 4)
			var sh := rng.randi_range(2, 3)
			for y in sh:
				_cell(c, mx, y, mz, stem)
				_cell(c, mx + 1, y, mz, stem)
			for dx in range(-1, 3):
				for dz in range(-1, 3):
					_cell(c, mx + dx, sh, mz + dz, cap)
					if abs(dx - 0.5) + abs(dz - 0.5) < 1.5:
						_cell(c, mx + dx, sh + 1, mz + dz, cap)
			var ox := rng.randi_range(0, g - 1)
			var oz := rng.randi_range(0, g - 1)
			_cell(c, ox, 0, oz, stem)
			_cell(c, ox, 1, oz, cap)
		"reed":
			# 4 stalks the full voxel height (stacked twice by the generator = 1 m reeds), heads on some
			var head: int = tiles[1] + 1
			for i in 4:
				var x := rng.randi_range(0, g - 1)
				var z := rng.randi_range(0, g - 1)
				for y in g:
					_cell(c, x, y, z, body)
				if rng.randf() < 0.5:
					_cell(c, x, g - 1, z, head)
					_cell(c, x, g - 2, z, head)
		"lilypad":
			# a 5x5 pad minus corners, one cell thick on the water surface, with a notch and a raised rim
			var px := rng.randi_range(0, 3)
			var pz := rng.randi_range(0, 3)
			for dx in 5:
				for dz in 5:
					if (dx == 0 or dx == 4) and (dz == 0 or dz == 4):
						continue
					if dx == 2 and dz == 4:
						continue
					_cell(c, px + dx, 0, pz + dz, body)
					if (dx == 0 or dx == 4 or dz == 0) and rng.randf() < 0.5:
						_cell(c, px + dx, 1, pz + dz, body)
		"carpet":
			# moss: a 1-cell layer over ~70 % of the floor with a few 2-cell bumps
			for x in g:
				for z in g:
					if rng.randf() < 0.7:
						_cell(c, x, 0, z, body)
						if rng.randf() < 0.15:
							_cell(c, x, 1, z, body)
		"stalactite":
			# cone hanging from the ceiling: 4x4 at the top down to a 1-cell tip
			var ox := rng.randi_range(2, 3)
			var oz := rng.randi_range(2, 3)
			var widths := [4, 4, 3, 2, 2, 2, 1, 1]
			for y in g:
				var w: int = widths[g - 1 - y]
				var o: int = (4 - w) / 2
				for dx in w:
					for dz in w:
						_cell(c, ox + o + dx, y, oz + o + dz, body)
		"stalagmite":
			var ox := rng.randi_range(2, 3)
			var oz := rng.randi_range(2, 3)
			var widths := [4, 4, 3, 3, 2, 2, 1, 1]
			for y in g:
				var w: int = widths[y]
				var o: int = (4 - w) / 2
				for dx in w:
					for dz in w:
						_cell(c, ox + o + dx, y, oz + o + dz, body)
		"wattle":
			# woven wall (ADR 0030): upright stakes every other cell, withies weaving
			# over and under them in alternating rows; tiles [withy, stake, stake]
			var stake: int = tiles[1] + 1
			for x in g:
				for z in g:
					for y in g:
						if x % 3 == 0 and z % 3 == 0:
							_cell(c, x, y, z, stake)
						elif y % 2 == 0 and ((x + z + y / 2) % 2 == 0 or x == 0 or z == 0 or x == g - 1 or z == g - 1):
							_cell(c, x, y, z, body)
						elif y % 2 == 1 and (x == 0 or z == 0 or x == g - 1 or z == g - 1) and (x + z) % 2 == 1:
							_cell(c, x, y, z, body)
		"torch":
			# a 2x2 stick 5 cells tall with a 2x2x2 flame and a tip; tiles [flame, stick, ember]
			var stick: int = tiles[1] + 1
			var ember: int = tiles[2] + 1
			for y in 5:
				for dx in 2:
					for dz in 2:
						_cell(c, 3 + dx, y, 3 + dz, stick)
			for dx in 2:
				for dz in 2:
					_cell(c, 3 + dx, 4, 3 + dz, ember)
					_cell(c, 3 + dx, 5, 3 + dz, body)
					_cell(c, 3 + dx, 6, 3 + dz, body)
			_cell(c, 3, 7, 4, body)
		"storage_pot":
			# a fired clay jar: wide belly, shoulder, narrow neck with a lip; tiles [clay, -, -]
			var widths := [5, 6, 7, 7, 7, 6, 4, 4]
			for y in g:
				var w: int = widths[y]
				var lo := (g - w) / 2
				for x in range(lo, lo + w):
					for z in range(lo, lo + w):
						var corner := (x == lo or x == lo + w - 1) and (z == lo or z == lo + w - 1)
						var inner := y >= 6 and x > lo and z > lo and x < lo + w - 1 and z < lo + w - 1
						if (not corner or w <= 4) and not inner:
							_cell(c, x, y, z, body)
	return c


static func _cell(c: PackedInt32Array, x: int, y: int, z: int, paint: int) -> void:
	var g := MICRO_GRID
	if x < 0 or y < 0 or z < 0 or x >= g or y >= g or z >= g:
		return
	c[x + g * (y + g * z)] = paint


## Fills the box [from, to) of a micro pattern with `paint` (cells outside the grid are skipped).
static func _box_cells(c: PackedInt32Array, from: Vector3i, to: Vector3i, paint: int) -> void:
	for x in range(from.x, to.x):
		for y in range(from.y, to.y):
			for z in range(from.z, to.z):
				_cell(c, x, y, z, paint)
