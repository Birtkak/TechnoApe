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

const ATLAS_TILES := Vector2i(8, 7)
const MICRO_GRID := 8       # cells per voxel edge for micro models and chisels (6.25 cm)


static func defs() -> Array:
	return [
		{"name": "stone", "cell_type": "solid", "density": 10, "hp": 1000, "tiles": [0, 0, 0], "transparent": false},
		{"name": "dirt", "cell_type": "powder", "liquid_static": true, "density": 3, "tiles": [1, 1, 1], "transparent": false},
		{"name": "grass", "cell_type": "powder", "liquid_static": true, "density": 3, "tiles": [2, 3, 1], "transparent": false},
		{"name": "sand", "cell_type": "powder", "density": 3, "tiles": [4, 4, 4], "transparent": false},
		{"name": "water", "cell_type": "liquid", "density": 1, "liquid_flow_speed": 1.0, "tiles": [5, 5, 5], "transparent": true},
		{"name": "wood", "cell_type": "solid", "density": 10, "burnable": true, "fire_hp": 160, "autoignition_temperature": 40, "temperature_of_fire": 50, "on_fire_convert_to_material": 8, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 4, "tiles": [6, 6, 6], "transparent": false},
		{"name": "lava", "cell_type": "liquid", "density": 5, "liquid_flow_speed": 0.3, "liquid_gravity": 0.3, "temperature_of_fire": 150, "tiles": [7, 7, 7], "transparent": false},
		{"name": "ash", "cell_type": "powder", "density": 2, "tiles": [8, 8, 8], "transparent": false},
		{"name": "oil", "cell_type": "liquid", "density": 0.8, "liquid_flow_speed": 0.6, "burnable": true, "fire_hp": 60, "autoignition_temperature": 20, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 6, "tiles": [9, 9, 9], "transparent": true},
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
		{"name": "water_static", "cell_type": "liquid", "liquid_static": true, "density": 1, "tiles": [5, 5, 5], "transparent": true, "loose": 5},
		{"name": "sandstone", "cell_type": "solid", "density": 10, "hp": 600, "tiles": [21, 21, 21], "transparent": false},
		{"name": "clay", "cell_type": "solid", "density": 9, "hp": 300, "tiles": [22, 22, 22], "transparent": false},
		{"name": "tall_grass", "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 40, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 3, "tiles": [23, 23, 23], "transparent": false, "micro": "tuft"},
		{"name": "flower", "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 40, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2, "tiles": [24, 24, 24], "transparent": false, "micro": "flower"},
		{"name": "cactus", "cell_type": "solid", "density": 10, "hp": 40, "burnable": true, "fire_hp": 30, "autoignition_temperature": 40, "temperature_of_fire": 50, "on_fire_convert_to_material": 8, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 4, "tiles": [25, 25, 25], "transparent": false},
		{"name": "dead_bush", "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 24, "autoignition_temperature": 15, "temperature_of_fire": 55, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 3, "tiles": [26, 26, 26], "transparent": false, "micro": "bush"},
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
		{"name": "blood", "cell_type": "liquid", "density": 1.1, "liquid_flow_speed": 0.7, "lifetime": 200, "tiles": [47, 47, 47], "transparent": true},
		# ground pickups: hardness 0 in mining.json, they drop loose items (ADR 0019)
		{"name": "copper_bearing_stones", "cell_type": "solid", "density": 10, "hp": 10, "tiles": [18, 18, 18], "transparent": false, "micro": "ore_stones"},
		{"name": "tin_bearing_stones", "cell_type": "solid", "density": 10, "hp": 10, "tiles": [19, 19, 19], "transparent": false, "micro": "ore_stones"},
		{"name": "iron_bearing_stones", "cell_type": "solid", "density": 10, "hp": 10, "tiles": [17, 17, 17], "transparent": false, "micro": "ore_stones"},
		{"name": "flint_stones", "cell_type": "solid", "density": 10, "hp": 10, "tiles": [13, 13, 13], "transparent": false, "micro": "pebbles"},
		{"name": "twigs", "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 20, "autoignition_temperature": 20, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2, "tiles": [26, 26, 26], "transparent": false, "micro": "twigs"},
		# sulfur caves (biomes.json sulfur_cave) and Noita chemistry (reactions below)
		{"name": "toxic_sludge", "cell_type": "liquid", "density": 1.3, "liquid_flow_speed": 0.4, "tiles": [48, 48, 48], "transparent": true},
		{"name": "toxic_sludge_static", "cell_type": "liquid", "liquid_static": true, "density": 1.3, "tiles": [48, 48, 48], "transparent": true, "loose": 53},
		{"name": "sulfur", "cell_type": "solid", "density": 10, "hp": 400, "tiles": [49, 49, 49], "transparent": false},
		{"name": "obsidian", "cell_type": "solid", "density": 12, "hp": 6000, "tiles": [50, 50, 50], "transparent": false},
	]


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
		{"input_cell1": FIRE, "input_cell2": ICE, "output_cell1": AIR, "output_cell2": WATER, "probability": 0.4},
		{"input_cell1": LAVA, "input_cell2": ICE, "output_cell1": STONE, "output_cell2": STEAM, "probability": 0.5},
	]


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
static func build_library(atlas_material: Material, liquid_material: Material = null) -> VoxelBlockyLibrary:
	var lib := VoxelBlockyLibrary.new()
	var models: Array = [VoxelBlockyModelEmpty.new()]
	for d in defs():
		if d.has("micro"):
			models.append(_micro_model(d["micro"], d["tiles"], atlas_material))
			continue
		if d.get("model", "") == "empty":
			var empty := VoxelBlockyModelEmpty.new()
			empty.culls_neighbors = false
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
	lib.models = models
	lib.bake()
	return lib


## Micro-voxel model (ADR 0006): a pattern on the MICRO_GRID^3 cell grid (6.25 cm
## cells), meshed by the native MicroMesher with only exposed faces, each cell
## showing its 1/8 of an opaque atlas tile. Cells hold "paint" = atlas tile + 1
## (0 = air), so stems and petals can use colour tiles that are not materials.
## Plants never cull neighbours (the ground under them stays visible) and have
## no collision; stalactites/stalagmites get a core collision box.
## tiles: the material's [top, side, bottom] tiles; patterns read what they need.
static func _micro_model(kind: String, tiles: Array, material: Material) -> VoxelBlockyModelMesh:
	var cells := micro_pattern(kind, tiles)
	var mesh: ArrayMesh = MicroMesher.build(cells, Vector3i(MICRO_GRID, MICRO_GRID, MICRO_GRID), MICRO_GRID, micro_paint_tiles(), ATLAS_TILES, Vector3.ZERO)
	var model := VoxelBlockyModelMesh.new()
	model.mesh = mesh
	model.set_material_override(0, material)
	model.culls_neighbors = false
	model.collision_aabbs = []
	if kind == "stalactite" or kind == "stalagmite":
		model.collision_aabbs = [AABB(Vector3(0.25, 0.0, 0.25), Vector3(0.5, 1.0, 0.5))]
	model.set_mesh_collision_enabled(0, false)
	return model


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
	match kind:
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
