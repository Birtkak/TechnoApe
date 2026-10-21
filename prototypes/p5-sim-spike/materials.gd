class_name SimMaterials
extends RefCounted

## Material table for the P5 spike, using Noita's materials.xml attribute names.
## Index = material id (0 is air, implicit). Colours feed the blocky library.
## In the real game this is a data file (JSON) validated by schema + tests.

const AIR := 0
const STONE := 1
const SAND := 2
const WATER := 3
const STEAM := 4
const OIL := 5
const LAVA := 6
const WOOD := 7
const FIRE := 8
const SMOKE := 9
const ASH := 10

static func defs() -> Array:
	return [
		# 1 stone: static solid ground
		{"name": "stone", "cell_type": "solid", "density": 10, "hp": 1000, "color": Color(0.5, 0.5, 0.52)},
		# 2 sand: powder (liquid + liquid_sand), denser than water so it sinks
		{"name": "sand", "cell_type": "liquid", "liquid_sand": true, "density": 3, "color": Color(0.85, 0.75, 0.4)},
		# 3 water: liquid, flows fast, extinguishes
		{"name": "water", "cell_type": "liquid", "density": 1, "liquid_flow_speed": 1.0, "color": Color(0.2, 0.4, 0.9)},
		# 4 steam: gas with a lifetime
		{"name": "steam", "cell_type": "gas", "density": 0, "gas_upwards_speed": 200, "gas_horizontal_speed": 60, "lifetime": 180, "color": Color(0.9, 0.9, 0.95)},
		# 5 oil: lighter than water, burnable, viscous
		{"name": "oil", "cell_type": "liquid", "density": 0.8, "liquid_flow_speed": 0.6, "burnable": true, "fire_hp": 60, "autoignition_temperature": 20, "on_fire_flame_material": 8, "on_fire_smoke_material": 9, "generates_smoke": 6, "color": Color(0.25, 0.2, 0.1)},
		# 6 lava: dense, slow, very hot
		{"name": "lava", "cell_type": "liquid", "density": 5, "liquid_flow_speed": 0.3, "liquid_gravity": 0.3, "temperature_of_fire": 150, "color": Color(0.95, 0.4, 0.1)},
		# 7 wood: burnable solid, turns to ash
		{"name": "wood", "cell_type": "solid", "density": 10, "burnable": true, "fire_hp": 160, "autoignition_temperature": 40, "temperature_of_fire": 50, "on_fire_convert_to_material": 10, "on_fire_flame_material": 8, "on_fire_smoke_material": 9, "generates_smoke": 4, "color": Color(0.55, 0.35, 0.18)},
		# 8 fire: the flame itself, short-lived, hot, rises
		{"name": "fire", "cell_type": "fire", "density": 0, "lifetime": 24, "temperature_of_fire": 60, "gas_upwards_speed": 120, "gas_horizontal_speed": 80, "color": Color(1.0, 0.6, 0.15)},
		# 9 smoke: gas, slow, fades
		{"name": "smoke", "cell_type": "gas", "density": 0, "gas_upwards_speed": 120, "gas_horizontal_speed": 90, "lifetime": 240, "color": Color(0.3, 0.3, 0.32)},
		# 10 ash: light powder left by burnt wood
		{"name": "ash", "cell_type": "liquid", "liquid_sand": true, "density": 2, "color": Color(0.6, 0.58, 0.55)},
	]


static func reactions() -> Array:
	return [
		# lava + water -> stone + steam
		{"input_cell1": LAVA, "input_cell2": WATER, "output_cell1": STONE, "output_cell2": STEAM, "probability": 0.5},
		# fire + water -> air + steam (water quenches flames)
		{"input_cell1": FIRE, "input_cell2": WATER, "output_cell1": AIR, "output_cell2": STEAM, "probability": 0.6},
		# lava ignites oil it touches: lava + oil -> lava + fire
		{"input_cell1": LAVA, "input_cell2": OIL, "output_cell1": LAVA, "output_cell2": FIRE, "probability": 0.3},
		# lava sets wood ablaze: lava + wood -> lava + fire
		{"input_cell1": LAVA, "input_cell2": WOOD, "output_cell1": LAVA, "output_cell2": FIRE, "probability": 0.1},
	]


static func colors() -> Array:
	var out := [Color.BLACK]
	for d in defs():
		out.append(d["color"])
	return out
