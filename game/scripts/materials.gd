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

const ATLAS_TILES := Vector2i(4, 4)


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
	]


static func reactions() -> Array:
	return [
		{"input_cell1": LAVA, "input_cell2": WATER, "output_cell1": STONE, "output_cell2": STEAM, "probability": 0.5},
		{"input_cell1": FIRE, "input_cell2": WATER, "output_cell1": AIR, "output_cell2": STEAM, "probability": 0.6},
		{"input_cell1": LAVA, "input_cell2": OIL, "output_cell1": LAVA, "output_cell2": FIRE, "probability": 0.3},
		{"input_cell1": LAVA, "input_cell2": WOOD, "output_cell1": LAVA, "output_cell2": FIRE, "probability": 0.1},
	]


static func name_of(id: int) -> String:
	if id == 0:
		return "air"
	var d := defs()
	return d[id - 1]["name"] if id - 1 < d.size() else "?"


## Builds the godot_voxel blocky library with atlas tiles per face.
static func build_library(atlas_material: Material) -> VoxelBlockyLibrary:
	var lib := VoxelBlockyLibrary.new()
	var models: Array = [VoxelBlockyModelEmpty.new()]
	for d in defs():
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
