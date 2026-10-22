class_name GameMaterials
extends RefCounted

## Material table for the game, Noita attribute names (see
## docs/design/material-simulation.md). Index = material id, 0 = air.
## `tiles` = atlas tile index [top, side, bottom] (game/assets/textures/atlas.png, 4x4 tiles).
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
const TALL_GRASS := 26      # micro-voxel vegetation (quarter cubes, no collision): tufts, flowers, dead bushes
const FLOWER := 27
const CACTUS := 28
const DEAD_BUSH := 29
const ICE := 30
const FLOWER_RED := 31

const ATLAS_TILES := Vector2i(8, 4)


static func defs() -> Array:
	return [
		{"name": "stone", "cell_type": "solid", "density": 10, "hp": 1000, "tiles": [0, 0, 0], "transparent": false},
		{"name": "dirt", "cell_type": "liquid", "liquid_sand": true, "liquid_static": true, "density": 3, "tiles": [1, 1, 1], "transparent": false},
		{"name": "grass", "cell_type": "liquid", "liquid_sand": true, "liquid_static": true, "density": 3, "tiles": [2, 3, 1], "transparent": false},
		{"name": "sand", "cell_type": "liquid", "liquid_sand": true, "density": 3, "tiles": [4, 4, 4], "transparent": false},
		{"name": "water", "cell_type": "liquid", "density": 1, "liquid_flow_speed": 1.0, "tiles": [5, 5, 5], "transparent": true},
		{"name": "wood", "cell_type": "solid", "density": 10, "burnable": true, "fire_hp": 160, "autoignition_temperature": 40, "temperature_of_fire": 50, "on_fire_convert_to_material": 8, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 4, "tiles": [6, 6, 6], "transparent": false},
		{"name": "lava", "cell_type": "liquid", "density": 5, "liquid_flow_speed": 0.3, "liquid_gravity": 0.3, "temperature_of_fire": 150, "tiles": [7, 7, 7], "transparent": false},
		{"name": "ash", "cell_type": "liquid", "liquid_sand": true, "density": 2, "tiles": [8, 8, 8], "transparent": false},
		{"name": "oil", "cell_type": "liquid", "density": 0.8, "liquid_flow_speed": 0.6, "burnable": true, "fire_hp": 60, "autoignition_temperature": 20, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 6, "tiles": [9, 9, 9], "transparent": true},
		{"name": "steam", "cell_type": "gas", "density": 0, "gas_upwards_speed": 200, "gas_horizontal_speed": 60, "lifetime": 180, "tiles": [10, 10, 10], "transparent": true},
		{"name": "smoke", "cell_type": "gas", "density": 0, "gas_upwards_speed": 120, "gas_horizontal_speed": 90, "lifetime": 240, "tiles": [11, 11, 11], "transparent": true},
		{"name": "fire", "cell_type": "fire", "density": 0, "lifetime": 24, "temperature_of_fire": 60, "gas_upwards_speed": 120, "gas_horizontal_speed": 80, "tiles": [12, 12, 12], "transparent": true},
		{"name": "bedrock", "cell_type": "solid", "density": 15, "hp": 1000000, "tiles": [13, 13, 13], "transparent": false},
		{"name": "snow", "cell_type": "liquid", "liquid_sand": true, "liquid_static": true, "density": 2, "tiles": [14, 14, 14], "transparent": false},
		{"name": "gravel", "cell_type": "liquid", "liquid_sand": true, "density": 4, "tiles": [15, 15, 15], "transparent": false},
		{"name": "coal_ore", "cell_type": "solid", "density": 10, "hp": 900, "burnable": true, "fire_hp": 240, "autoignition_temperature": 90, "on_fire_convert_to_material": 1, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 8, "tiles": [16, 16, 16], "transparent": false},
		{"name": "iron_ore", "cell_type": "solid", "density": 11, "hp": 1200, "tiles": [17, 17, 17], "transparent": false},
		{"name": "copper_ore", "cell_type": "solid", "density": 11, "hp": 1100, "tiles": [18, 18, 18], "transparent": false},
		{"name": "tin_ore", "cell_type": "solid", "density": 11, "hp": 1100, "tiles": [19, 19, 19], "transparent": false},
		{"name": "leaves", "cell_type": "solid", "density": 10, "hp": 20, "burnable": true, "fire_hp": 40, "autoignition_temperature": 30, "temperature_of_fire": 55, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 6, "tiles": [20, 20, 20], "transparent": true},
		{"name": "sand_static", "cell_type": "liquid", "liquid_sand": true, "liquid_static": true, "density": 3, "tiles": [4, 4, 4], "transparent": false, "loose": 4},
		{"name": "gravel_static", "cell_type": "liquid", "liquid_sand": true, "liquid_static": true, "density": 4, "tiles": [15, 15, 15], "transparent": false, "loose": 15},
		{"name": "water_static", "cell_type": "liquid", "liquid_static": true, "density": 1, "tiles": [5, 5, 5], "transparent": true, "loose": 5},
		{"name": "sandstone", "cell_type": "solid", "density": 10, "hp": 600, "tiles": [21, 21, 21], "transparent": false},
		{"name": "clay", "cell_type": "solid", "density": 9, "hp": 300, "tiles": [22, 22, 22], "transparent": false},
		{"name": "tall_grass", "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 10, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 3, "tiles": [23, 23, 23], "transparent": false, "micro": "tuft"},
		{"name": "flower", "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 8, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2, "tiles": [24, 24, 24], "transparent": false, "micro": "flower"},
		{"name": "cactus", "cell_type": "solid", "density": 10, "hp": 40, "burnable": true, "fire_hp": 30, "autoignition_temperature": 40, "temperature_of_fire": 50, "on_fire_convert_to_material": 8, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 4, "tiles": [25, 25, 25], "transparent": false},
		{"name": "dead_bush", "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 6, "autoignition_temperature": 15, "temperature_of_fire": 55, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 3, "tiles": [26, 26, 26], "transparent": false, "micro": "bush"},
		{"name": "ice", "cell_type": "solid", "density": 9, "hp": 120, "tiles": [27, 27, 27], "transparent": false},
		{"name": "flower_red", "cell_type": "solid", "density": 10, "hp": 5, "burnable": true, "fire_hp": 8, "autoignition_temperature": 25, "temperature_of_fire": 50, "on_fire_convert_to_material": 0, "on_fire_flame_material": 12, "on_fire_smoke_material": 11, "generates_smoke": 2, "tiles": [28, 28, 28], "transparent": false, "micro": "flower"},
	]


static func reactions() -> Array:
	return [
		{"input_cell1": LAVA, "input_cell2": WATER, "output_cell1": STONE, "output_cell2": STEAM, "probability": 0.5},
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
## Materials with a "micro" key are built from quarter-voxel cubes (12.5 cm)
## instead of a full cube: vegetation that is still voxels, no alpha cutout.
static func build_library(atlas_material: Material) -> VoxelBlockyLibrary:
	var lib := VoxelBlockyLibrary.new()
	var models: Array = [VoxelBlockyModelEmpty.new()]
	for d in defs():
		if d.has("micro"):
			models.append(_micro_model(d["micro"], d["tiles"][0], atlas_material))
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
		cube.set_material_override(0, atlas_material)
		# Transparent materials share transparency index 1: faces between two
		# voxels of the same index are culled (no water-water faces), faces
		# against air or opaque voxels are kept. culls_neighbors stays true.
		if d.get("transparent", false):
			cube.transparency_index = 1
		models.append(cube)
	lib.models = models
	lib.bake()
	return lib


## Micro-voxel model: a pattern of quarter-size cubes (4x4x4 grid per voxel,
## 12.5 cm each) with only the exposed faces, textured from opaque atlas tiles.
## No collision (walk through like Minecraft tufts), never culls neighbours (the
## ground under it stays visible), same opaque material as the terrain.
## kind: "tuft" | "flower" | "bush"; tile: the atlas tile for the plant body;
## flowers take their petal tile from the head colour (tile 30 yellow, 31 red).
static func _micro_model(kind: String, tile: int, material: Material) -> VoxelBlockyModelMesh:
	var cells := {}  # Vector3i -> tile
	var rng := RandomNumberGenerator.new()
	rng.seed = hash(kind + str(tile))
	match kind:
		"tuft":
			for i in 4:
				var x := rng.randi_range(0, 3)
				var z := rng.randi_range(0, 3)
				var hgt := rng.randi_range(1, 3)
				for y in hgt:
					cells[Vector3i(x, y, z)] = tile
				if hgt >= 3 and rng.randf() < 0.6:
					var lean := Vector3i(x + rng.randi_range(-1, 1), hgt - 1, z + rng.randi_range(-1, 1))
					if lean.x >= 0 and lean.x <= 3 and lean.z >= 0 and lean.z <= 3:
						cells[lean] = tile
		"flower":
			var petal := 31 if tile == 28 else 30
			for y in 3:
				cells[Vector3i(1, y, 1)] = 29  # stem, leaf green
			cells[Vector3i(0, 1, 2)] = 29      # a leaf
			# plus-shaped head: petals around a contrasting centre
			cells[Vector3i(1, 3, 1)] = 30 if petal == 31 else 31
			for off in [Vector3i(1, 0, 0), Vector3i(-1, 0, 0), Vector3i(0, 0, 1), Vector3i(0, 0, -1)]:
				cells[Vector3i(1, 3, 1) + off] = petal
			for i in 2:  # a couple of grass blades around it
				var x := rng.randi_range(0, 3)
				var z := rng.randi_range(0, 3)
				if not cells.has(Vector3i(x, 0, z)):
					cells[Vector3i(x, 0, z)] = tile
					if rng.randf() < 0.5:
						cells[Vector3i(x, 1, z)] = tile
		"bush":
			cells[Vector3i(1, 0, 1)] = tile
			cells[Vector3i(1, 1, 1)] = tile
			for i in 5:  # twigs
				var x := rng.randi_range(0, 3)
				var z := rng.randi_range(0, 3)
				var y := rng.randi_range(1, 2)
				cells[Vector3i(x, y, z)] = tile
				cells[Vector3i(x, y - 1, z)] = tile
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	var q := 0.25
	# faces: normal, the four corners (in units of the quarter cube), tile uv origin
	var faces := [
		[Vector3i(0, 1, 0), [Vector3(0, 1, 0), Vector3(0, 1, 1), Vector3(1, 1, 1), Vector3(1, 1, 0)]],
		[Vector3i(0, -1, 0), [Vector3(0, 0, 1), Vector3(0, 0, 0), Vector3(1, 0, 0), Vector3(1, 0, 1)]],
		[Vector3i(1, 0, 0), [Vector3(1, 0, 0), Vector3(1, 1, 0), Vector3(1, 1, 1), Vector3(1, 0, 1)]],
		[Vector3i(-1, 0, 0), [Vector3(0, 0, 1), Vector3(0, 1, 1), Vector3(0, 1, 0), Vector3(0, 0, 0)]],
		[Vector3i(0, 0, 1), [Vector3(1, 0, 1), Vector3(1, 1, 1), Vector3(0, 1, 1), Vector3(0, 0, 1)]],
		[Vector3i(0, 0, -1), [Vector3(0, 0, 0), Vector3(0, 1, 0), Vector3(1, 1, 0), Vector3(1, 0, 0)]],
	]
	for c in cells:
		var t: int = cells[c]
		var u0 := float(t % ATLAS_TILES.x) / ATLAS_TILES.x
		var v0 := float(t / ATLAS_TILES.x) / ATLAS_TILES.y
		var du := 1.0 / ATLAS_TILES.x
		var dv := 1.0 / ATLAS_TILES.y
		for f in faces:
			var n: Vector3i = f[0]
			if cells.has(c + n):
				continue  # face shared with another cube of the pattern
			var corners: Array = f[1]
			var p: Array = []
			for k in 4:
				p.append((Vector3(c) + corners[k]) * q)
			# each quarter cube shows a quarter of the tile so the texture scale matches the terrain
			var uq := [Vector2(0, 1), Vector2(0, 0), Vector2(1, 0), Vector2(1, 1)]
			var uo := Vector2(u0 + du * 0.25 * (c.x % 4), v0 + dv * 0.25 * (c.z % 4))
			var uvs: Array = []
			for k in 4:
				uvs.append(uo + Vector2(uq[k].x * du * 0.25, uq[k].y * dv * 0.25))
			var nn := Vector3(n)
			for idx in [0, 1, 2, 0, 2, 3]:
				st.set_normal(nn)
				st.set_uv(uvs[idx])
				st.add_vertex(p[idx])
	st.index()
	var model := VoxelBlockyModelMesh.new()
	model.mesh = st.commit()
	model.set_material_override(0, material)
	model.culls_neighbors = false
	model.collision_aabbs = []
	model.set_mesh_collision_enabled(0, false)
	return model
