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
const WATER_STATIC := 23    # generated river/lake water: static until disturbed (else it drains downhill)
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

const ATLAS_TILES := Vector2i(8, 11)   # rows grow at the bottom, so tile indices keep their meaning
# Liquids with a "fluid_slot" are Minecraft-style fluids in the terrain (godot_voxel
# VoxelBlockyFluid): a full cell is the material's own voxel id, a cell with 1..7
# eighths (SimWorld fullness) is FLUID_LEVEL_BASE + slot * 8 + eighths - 1. Fixed
# above every material id, so new materials never move them (saves stay valid).
# Same constant as SimWorld::FLUID_LEVEL_BASE.
const FLUID_LEVEL_BASE := 256
const MICRO_GRID := 8       # cells per voxel edge for micro models and chisels (6.25 cm)
const MASK_TARGET := 2      # collision bit of boxes only the aim ray hits (VoxelBody uses bit 1, VoxelBody.MASK_SOLID)


static func defs() -> Array:
	return [
		{"name": "stone", "cell_type": "solid", "density": 10, "hp": 1000, "tiles": [0, 0, 0], "transparent": false},
		{"name": "dirt", "cell_type": "powder", "liquid_static": true, "density": 3, "tiles": [1, 1, 1], "transparent": false},
		{"name": "grass", "cell_type": "powder", "liquid_static": true, "density": 3, "tiles": [2, 3, 1], "transparent": false},
		{"name": "sand", "cell_type": "powder", "density": 3, "tiles": [4, 4, 4], "transparent": false},
		{"name": "water", "cell_type": "liquid", "fluid_slot": 0, "density": 1, "liquid_flow_speed": 1.0, "tiles": [5, 5, 5], "transparent": true},
		{"name": "wood", "cell_type": "solid", "density": 10, "burnable": true, "fire_hp": 160, "autoignition_temperature": 40, "temperature_of_fire": 50, "on_fire_convert_to_material": 8, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 4, "tiles": [56, 6, 56], "transparent": false},
		{"name": "lava", "cell_type": "liquid", "fluid_slot": 1, "density": 5, "liquid_flow_speed": 0.3, "liquid_gravity": 0.3, "temperature_of_fire": 150, "tiles": [7, 7, 7], "transparent": false},
		{"name": "ash", "cell_type": "powder", "density": 2, "tiles": [8, 8, 8], "transparent": false},
		{"name": "oil", "cell_type": "liquid", "fluid_slot": 2, "density": 0.8, "liquid_flow_speed": 0.6, "burnable": true, "fire_hp": 60, "autoignition_temperature": 20, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 6, "tiles": [9, 9, 9], "transparent": true},
		{"name": "steam", "cell_type": "gas", "density": 0, "gas_upwards_speed": 200, "gas_horizontal_speed": 60, "lifetime": 180, "tiles": [10, 10, 10], "transparent": true},
		{"name": "smoke", "cell_type": "gas", "density": 0, "gas_upwards_speed": 120, "gas_horizontal_speed": 90, "lifetime": 240, "tiles": [11, 11, 11], "transparent": true},
		{"name": "fire", "cell_type": "fire", "density": 0, "lifetime": 24, "temperature_of_fire": 60, "gas_upwards_speed": 120, "gas_horizontal_speed": 80, "tiles": [12, 12, 12], "transparent": true},
		{"name": "bedrock", "cell_type": "solid", "density": 15, "hp": 1000000, "tiles": [13, 13, 13], "transparent": false},
		{"name": "snow", "cell_type": "powder", "liquid_static": true, "density": 2, "tiles": [14, 14, 14], "transparent": false},
		{"name": "gravel", "cell_type": "powder", "density": 4, "tiles": [15, 15, 15], "transparent": false},
		{"name": "coal_ore", "cell_type": "solid", "density": 10, "hp": 900, "burnable": true, "fire_hp": 240, "autoignition_temperature": 90, "on_fire_convert_to_material": 1, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 8, "tiles": [16, 16, 16], "transparent": false},
		{"name": "iron_ore", "cell_type": "solid", "density": 11, "hp": 1200, "tiles": [17, 17, 17], "transparent": false},
		{"name": "copper_ore", "cell_type": "solid", "density": 11, "hp": 1100, "tiles": [18, 18, 18], "transparent": false},
		{"name": "tin_ore", "cell_type": "solid", "density": 11, "hp": 1100, "tiles": [19, 19, 19], "transparent": false},
		{"name": "leaves", "cell_type": "solid", "density": 10, "hp": 20, "burnable": true, "fire_hp": 40, "autoignition_temperature": 30, "temperature_of_fire": 55, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 6, "tiles": [20, 20, 20], "transparent": true},
		{"name": "sand_static", "cell_type": "powder", "liquid_static": true, "density": 3, "tiles": [4, 4, 4], "transparent": false, "loose": 4},
		{"name": "gravel_static", "cell_type": "powder", "liquid_static": true, "density": 4, "tiles": [15, 15, 15], "transparent": false, "loose": 15},
		{"name": "water_static", "cell_type": "liquid", "fluid_slot": 0, "liquid_static": true, "density": 1, "tiles": [5, 5, 5], "transparent": true, "loose": 5},
		{"name": "sandstone", "cell_type": "solid", "density": 10, "hp": 600, "tiles": [21, 21, 21], "transparent": false},
		{"name": "clay", "cell_type": "solid", "density": 9, "hp": 300, "tiles": [22, 22, 22], "transparent": false},
		{"name": "tall_grass", "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 40, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 3, "tiles": [23, 23, 23], "transparent": false, "micro": "tuft", "replaceable": true},
		{"name": "flower", "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 40, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2, "tiles": [24, 24, 24], "transparent": false, "micro": "flower"},
		{"name": "cactus", "cell_type": "solid", "density": 10, "hp": 40, "burnable": true, "fire_hp": 30, "autoignition_temperature": 40, "temperature_of_fire": 50, "on_fire_convert_to_material": 8, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 4, "tiles": [25, 25, 25], "transparent": false},
		{"name": "dead_bush", "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 24, "autoignition_temperature": 15, "temperature_of_fire": 55, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 3, "tiles": [26, 26, 26], "transparent": false, "micro": "bush", "replaceable": true},
		{"name": "ice", "cell_type": "solid", "density": 9, "hp": 120, "tiles": [27, 27, 27], "transparent": false},
		{"name": "flower_red", "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 40, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2, "tiles": [28, 28, 28], "transparent": false, "micro": "flower"},
		{"name": "crafting_table", "cell_type": "solid", "density": 8, "hp": 200, "tiles": [32, 33, 39], "transparent": false, "interact": "crafting_table"},
		{"name": "furnace", "cell_type": "solid", "density": 10, "hp": 800, "tiles": [34, 35, 34], "transparent": false, "interact": "furnace", "entity": "furnace"},
		{"name": "furnace_lit", "cell_type": "solid", "density": 10, "hp": 800, "tiles": [34, 36, 34], "transparent": false, "interact": "furnace", "entity": "furnace"},
		{"name": "chest", "cell_type": "solid", "density": 8, "hp": 200, "tiles": [37, 38, 37], "transparent": false, "interact": "chest", "entity": "chest"},
		{"name": "planks", "cell_type": "solid", "density": 8, "burnable": true, "fire_hp": 120, "autoignition_temperature": 40, "temperature_of_fire": 50, "on_fire_convert_to_material": 8, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 4, "tiles": [39, 39, 39], "transparent": false},
		{"name": "pebbles", "cell_type": "solid", "density": 10, "hp": 10, "tiles": [0, 0, 0], "transparent": false, "micro": "pebbles"},
		{"name": "mushroom_brown", "cell_type": "solid", "density": 10, "hp": 5, "tiles": [40, 42, 42], "transparent": false, "micro": "mushroom"},
		{"name": "mushroom_red", "cell_type": "solid", "density": 10, "hp": 5, "tiles": [41, 42, 42], "transparent": false, "micro": "mushroom"},
		{"name": "reed", "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 40, "autoignition_temperature": 30, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 3, "tiles": [23, 44, 23], "transparent": false, "micro": "reed"},
		{"name": "lily_pad", "cell_type": "solid", "density": 10, "hp": 5, "tiles": [43, 43, 43], "transparent": false, "micro": "lilypad"},
		{"name": "stalactite", "cell_type": "solid", "density": 10, "hp": 300, "tiles": [0, 0, 0], "transparent": false, "micro": "stalactite"},
		{"name": "stalagmite", "cell_type": "solid", "density": 10, "hp": 300, "tiles": [0, 0, 0], "transparent": false, "micro": "stalagmite"},
		{"name": "chiseled", "cell_type": "solid", "density": 10, "hp": 1000, "tiles": [0, 0, 0], "transparent": true, "model": "empty"},
		{"name": "glow_mushroom", "cell_type": "solid", "density": 10, "hp": 5, "tiles": [45, 42, 42], "transparent": false, "micro": "mushroom"},
		{"name": "moss", "cell_type": "solid", "density": 10, "hp": 8, "burnable": true, "fire_hp": 40, "autoignition_temperature": 35, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 3, "tiles": [46, 46, 46], "transparent": false, "micro": "carpet"},
		# lifetime in sim ticks (30 Hz): the puddle soaks into the ground after ~7 s; the stain stays
		{"name": "blood", "cell_type": "liquid", "fluid_slot": 3, "density": 1.1, "liquid_flow_speed": 0.7, "lifetime": 200, "tiles": [47, 47, 47], "transparent": true},
		# ground pickups: hardness 0 in mining.json, they drop loose items (ADR 0019)
		{"name": "copper_bearing_stones", "cell_type": "solid", "density": 10, "hp": 10, "tiles": [18, 18, 18], "transparent": false, "micro": "ore_stones"},
		{"name": "tin_bearing_stones", "cell_type": "solid", "density": 10, "hp": 10, "tiles": [19, 19, 19], "transparent": false, "micro": "ore_stones"},
		{"name": "iron_bearing_stones", "cell_type": "solid", "density": 10, "hp": 10, "tiles": [17, 17, 17], "transparent": false, "micro": "ore_stones"},
		{"name": "flint_stones", "cell_type": "solid", "density": 10, "hp": 10, "tiles": [13, 13, 13], "transparent": false, "micro": "pebbles"},
		{"name": "twigs", "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 20, "autoignition_temperature": 20, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2, "tiles": [26, 26, 26], "transparent": false, "micro": "twigs"},
		# sulfur caves (biomes.json sulfur_cave) and Noita chemistry (reactions below)
		{"name": "toxic_sludge", "cell_type": "liquid", "fluid_slot": 4, "density": 1.3, "liquid_flow_speed": 0.4, "tiles": [48, 48, 48], "transparent": true},
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
	] + _content_pass2_defs()


static var _content2 := []

## Ids 90..101: birch log and leaves, blueberry bush, pumpkin, three "bloom"
## flowers (tiles [petal, centre, blades]) and bark blocks (the log's side on all
## faces) for fallen logs.
static func _content_pass2_defs() -> Array:
	if not _content2.is_empty():
		return _content2
	var log_burn := {"burnable": true, "fire_hp": 160, "autoignition_temperature": 40, "temperature_of_fire": 50, "on_fire_convert_to_material": 8, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 4}
	var plant_burn := {"burnable": true, "fire_hp": 40, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2}
	var birch := {"name": "birch_log", "cell_type": "solid", "density": 10, "tiles": [74, 73, 74], "transparent": false}
	birch.merge(log_burn)
	var birch_leaves := {"name": "birch_leaves", "cell_type": "solid", "density": 10, "hp": 20, "tiles": [75, 75, 75], "transparent": true}
	birch_leaves.merge(plant_burn)
	birch_leaves["generates_smoke"] = 6
	var bush := {"name": "blueberry_bush", "cell_type": "solid", "density": 10, "hp": 10, "tiles": [76, 29, 29], "transparent": false, "micro": "berry_bush"}
	bush.merge(plant_burn)
	var pumpkin := {"name": "pumpkin", "cell_type": "solid", "density": 6, "hp": 60, "tiles": [78, 77, 77], "transparent": false}
	_content2.append_array([birch, birch_leaves, bush, pumpkin])
	for f in [["cornflower", 79, 30], ["oxeye_daisy", 80, 30], ["allium", 81, 81]]:
		var d := {"name": f[0], "cell_type": "solid", "density": 10, "hp": 5, "tiles": [f[1], f[2], 23], "transparent": false, "micro": "bloom", "replaceable": true}
		d.merge(plant_burn)
		_content2.append(d)
	for b in [["oak_bark", 6], ["spruce_bark", 57], ["acacia_bark", 60], ["jungle_bark", 63], ["birch_bark", 73]]:
		var d := {"name": b[0], "cell_type": "solid", "density": 10, "tiles": [b[1], b[1], b[1]], "transparent": false}
		d.merge(log_burn)
		_content2.append(d)
	return _content2


static var _content := []

## Ids 76..87: logs and leaves burn like oak's, terracotta and basalt are stone-like,
## packed ice melts slower than ice, ferns burn like tall grass.
static func _content_pass_defs() -> Array:
	if not _content.is_empty():
		return _content
	var log_burn := {"burnable": true, "fire_hp": 160, "autoignition_temperature": 40, "temperature_of_fire": 50, "on_fire_convert_to_material": 8, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 4}
	var leaf_burn := {"burnable": true, "fire_hp": 40, "autoignition_temperature": 30, "temperature_of_fire": 55, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 6}
	for k in [["spruce", 57, 58, 59], ["acacia", 60, 61, 62], ["jungle", 63, 64, 65]]:
		var trunk := {"name": "%s_log" % k[0], "cell_type": "solid", "density": 10, "tiles": [k[2], k[1], k[2]], "transparent": false}
		trunk.merge(log_burn)
		var leaves := {"name": "%s_leaves" % k[0], "cell_type": "solid", "density": 10, "hp": 20, "tiles": [k[3], k[3], k[3]], "transparent": true}
		leaves.merge(leaf_burn)
		_content.append_array([trunk, leaves])
	var fern := {"name": "fern", "cell_type": "solid", "density": 10, "hp": 5, "tiles": [72, 29, 72], "transparent": false, "micro": "fern", "replaceable": true}
	fern.merge(leaf_burn)
	fern["generates_smoke"] = 3
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

## wheat_0..wheat_7 and carrots_0..carrots_3: micro models "wheat:<age>" /
## "carrots:<look>" (tiles: [ripe, stalk, root]). Burnable like tall grass.
static func _crop_defs() -> Array:
	if _crops.is_empty():
		var burn := {"burnable": true, "fire_hp": 30, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2}
		for age in 8:
			var d := {"name": "wheat_%d" % age, "cell_type": "solid", "density": 10, "hp": 5, "tiles": [54, 29, 23], "transparent": false, "micro": "wheat:%d" % age, "crop": "wheat"}
			d.merge(burn)
			_crops.append(d)
		for look in 4:
			var d := {"name": "carrots_%d" % look, "cell_type": "solid", "density": 10, "hp": 5, "tiles": [55, 29, 23], "transparent": false, "micro": "carrots:%d" % look, "crop": "carrots"}
			d.merge(burn)
			_crops.append(d)
	return _crops


static func reactions() -> Array:
	return [
		# water quenches lava into obsidian (static lake water too, once the sim wakes it)
		{"input_cell1": LAVA, "input_cell2": WATER, "output_cell1": OBSIDIAN, "output_cell2": STEAM, "probability": 0.5},
		{"input_cell1": LAVA, "input_cell2": WATER_STATIC, "output_cell1": OBSIDIAN, "output_cell2": STEAM, "probability": 0.5},
		# Noita: water dilutes toxic sludge until only water is left
		{"input_cell1": TOXIC_SLUDGE, "input_cell2": WATER, "output_cell1": WATER, "output_cell2": WATER, "probability": 0.15},
		{"input_cell1": TOXIC_SLUDGE_STATIC, "input_cell2": WATER, "output_cell1": WATER, "output_cell2": WATER, "probability": 0.15},
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
static func build_library(atlas_material: Material, liquid_material: Material = null, fluid_materials: Dictionary = {}) -> VoxelBlockyLibrary:
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
		if d.has("micro"):
			models.append(_micro_model(d["micro"], d["tiles"], atlas_material))
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
		# Transparent materials share transparency index 1: faces between two
		# voxels of the same index are culled (no water-water faces), faces
		# against air or opaque voxels are kept. culls_neighbors stays true.
		# Passable transparent ones (water, oil, gases, fire) get the alpha material.
		if d.get("transparent", false):
			cube.transparency_index = 1
		cube.set_material_override(0, liquid_material if (liquid_material != null and passable and d.get("transparent", false)) else atlas_material)
		if passable:
			cube.collision_mask = 0
			cube.set_mesh_collision_enabled(0, false)
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
	return c


static func _cell(c: PackedInt32Array, x: int, y: int, z: int, paint: int) -> void:
	var g := MICRO_GRID
	if x < 0 or y < 0 or z < 0 or x >= g or y >= g or z >= g:
		return
	c[x + g * (y + g * z)] = paint
