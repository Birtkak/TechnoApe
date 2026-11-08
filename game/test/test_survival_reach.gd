extends GdUnitTestSuite

## Softlock check against real worlds (not the data walk): for a few seeds, from the spawn the game picks,
## every raw the road to LV needs (docs/plans/progression-walk.md) is generated within reach, ores at a
## depth a stair can get to, and there are caves to go down. Prints a table of the distances per seed.

const SEEDS := [1337, 1, 42, 9001, 20261108]
const STEP := 128          # 16 m between sample points
const HALF := 2560         # the 640 m world, +-320 m around the origin, in voxels
const MAX_M := 260.0       # walk from spawn to the nearest of each raw (the world is 640 m wide)
const VEIN_MAX_M := 360.0 # a whole vein (the bulk ore of the steam age): seed 1337 has its nearest tin at 284 m
const MAX_DEPTH_M := 40.0  # vein top below the ground

var _cache := {}


## {material name: [nearest metres from spawn, ...]} for one seed, plus "_spawn" and the find_nearest kinds.
func _survey(s: int) -> Dictionary:
	if _cache.has(s):
		return _cache[s]
	var tg := TerrainGenerator.new()
	tg.set_seed(s)
	var gen := FineGen.new()
	gen.configure(s, Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	var sp: Vector2i = tg.find_spawn()
	var want := {}   # id -> name
	for n in ["sand", "sand_static", "clay", "gravel", "gravel_static", "flint_stones", "pebbles", "granite_stones", "diorite_stones", "limestone_stones",
			"slate_stones", "marble_stones", "chalk_stones", "basalt_stones", "twigs",
			"copper_bearing_stones", "tin_bearing_stones", "iron_bearing_stones", "chalk", "pyrite", "sulfur",
			"driftwood", "coal_ore", "copper_ore", "tin_ore", "iron_ore", "oak_log", "birch_log", "acacia_log", "spruce_log"]:
		want[GameMaterials.id_of(n)] = n
	var near := {}
	for z in range(-HALF, HALF, STEP):
		for x in range(-HALF, HALF, STEP):
			var d := Vector2(x - sp.x, z - sp.y).length() / 8.0
			var h: int = gen.height_at(x, z)
			# the ground and what stands on it (trees, pickups), then a core 48 m down
			_note(gen.generate_block(Vector3i(x, h - 16, z), Vector3i(8, 96, 8), 0, 2), want, near, d)
			_note(gen.generate_block(Vector3i(x, h - 400, z), Vector3i(2, 384, 2), 0, 2), want, near, d)
	# pickups are a few per 100 m² (world_gen.cpp ground cover): the 16 m grid misses them, so scan every
	# column of a 24 m square round the spawn instead
	var start := {}
	for tz in range(-96, 96, 32):
		for tx in range(-96, 96, 32):
			var x0: int = sp.x + tx
			var z0: int = sp.y + tz
			_note(gen.generate_block(Vector3i(x0, gen.height_at(x0 + 16, z0 + 16) - 24, z0), Vector3i(32, 64, 32), 0, 2), want, start, 0.0)
	var out := {"_spawn": sp, "_spawn_info": tg.column_info(sp.x, sp.y), "_start": start}
	for n in want.values():
		out[n] = near.get(n, INF)
	for k in ["coal_vein", "copper_vein", "cassiterite_vein", "iron_vein", "copper_iron_vein", "cave", "sinkhole", "ravine", "sulfur_cave"]:
		var p: Vector3i = gen.find_nearest(k, sp.x, sp.y, 4096)
		out[k] = INF if p.y < 0 else Vector2(p.x - sp.x, p.z - sp.y).length() / 8.0
		out[k + "_depth"] = 0.0 if p.y < 0 else maxf(0.0, gen.height_at(p.x, p.z) - p.y) / 8.0
	_cache[s] = out
	return out


func _note(bytes: PackedByteArray, want: Dictionary, near: Dictionary, d: float) -> void:
	for i in range(0, bytes.size(), 2):
		var id := bytes.decode_u16(i)
		if want.has(id):
			var n: String = want[id]
			if d < float(near.get(n, INF)):
				near[n] = d


func _either(r: Dictionary, a: Array) -> float:
	var best := INF
	for n in a:
		best = minf(best, float(r[n]))
	return best


func test_spawn_is_dry_plains_or_forest() -> void:
	for s in SEEDS:
		var info: Dictionary = _survey(s)["_spawn_info"]
		assert_bool(int(info["biome"]) in [WorldGen.BIOME_PLAINS, WorldGen.BIOME_FOREST] and int(info["height"]) > Settings.SEA_LEVEL_VOXEL + 4) \
			.override_failure_message("seed %d spawns at %s in %s" % [s, _survey(s)["_spawn"], info]).is_true()


const STONES := ["pebbles", "granite_stones", "diorite_stones", "limestone_stones", "slate_stones", "marble_stones",
	"chalk_stones", "basalt_stones"]


## The first ten minutes (crafting table: flint and logs; knapping: a stone; sticks) from the 24 m round the spawn.
func test_the_start_pickups_lie_round_the_spawn() -> void:
	var bad := []
	for s in SEEDS:
		var st: Dictionary = _survey(s)["_start"]
		print("start seed %d: %s" % [s, st.keys()])
		var need := {"flint": ["flint_stones"], "sticks": ["twigs"], "stones": STONES, "wood": ["oak_log", "birch_log", "acacia_log", "spruce_log"]}
		for k in need:
			if not need[k].any(func(n: String) -> bool: return st.has(n)):
				bad.append("seed %d: no %s" % [s, k])
	assert_array(bad).override_failure_message("; ".join(bad)).is_empty()


func test_every_raw_on_the_road_to_lv_is_in_reach() -> void:
	var need := {   # what the progression walk asks the world for, with its alternatives
		"wood": ["oak_log", "birch_log", "acacia_log", "spruce_log"], "sand": ["sand", "sand_static"], "clay": ["clay"], "gravel": ["gravel", "gravel_static"],
		"sulfur": ["sulfur"],
		"coal": ["coal_ore"], "copper": ["copper_ore", "copper_bearing_stones"], "tin": ["tin_ore", "tin_bearing_stones"],
		"iron": ["iron_ore", "iron_bearing_stones"]}
	var bad := []
	for s in SEEDS:
		var r := _survey(s)
		var row := []
		for k in need:
			var m := _either(r, need[k])
			row.append("%s %s" % [k, "-" if m == INF else str(int(m))])
			if m > MAX_M:
				bad.append("seed %d: %s nearest %s m" % [s, k, "none" if m == INF else str(int(m))])
		for k in ["chalk", "pyrite"]:   # side quests only (pyrite_striker, flint_mine): printed, not required
			row.append("%s %s" % [k, "-" if r[k] == INF else str(int(r[k]))])
		for k in ["coal_vein", "copper_vein", "cassiterite_vein", "iron_vein", "cave", "sinkhole", "ravine", "sulfur_cave"]:
			row.append("%s %s@%d" % [k, "-" if r[k] == INF else str(int(r[k])), int(r[k + "_depth"])])
		print("reach seed %d spawn %s: %s" % [s, r["_spawn"], ", ".join(row)])
	assert_array(bad).override_failure_message("out of reach: " + "; ".join(bad)).is_empty()


## Veins share their ores (ore_veins.json: tin is the copper vein's middle, iron the copper_iron vein's top), so an
## ore is in reach when any vein that holds it is, at a depth a stair gets to.
func test_ore_veins_and_caves_are_near_enough() -> void:
	var holds := {"coal": ["coal_vein"], "copper": ["copper_vein", "copper_iron_vein", "cassiterite_vein"],
		"tin": ["cassiterite_vein", "copper_vein"], "iron": ["iron_vein", "copper_iron_vein"], "cave": ["cave"]}
	var bad := []
	for s in SEEDS:
		var r := _survey(s)
		for ore in holds:
			var best := ""
			for k in holds[ore]:
				if float(r[k]) <= VEIN_MAX_M and float(r[k + "_depth"]) <= MAX_DEPTH_M and (best == "" or float(r[k]) < float(r[best])):
					best = k
			if best == "":
				bad.append("seed %d: no vein with %s within %d m" % [s, ore, int(VEIN_MAX_M)])
	assert_array(bad).override_failure_message("; ".join(bad)).is_empty()


## Ground stones lie as the local rock's kind (granite_stones, chalk_stones ... ADR 0057): a recipe that names the
## plain loose_stone can not be made where no plain stone province is near (seed 9001 has none). Use #rock.
func test_no_recipe_asks_for_the_plain_loose_stone() -> void:
	var bad := []
	for f in ["res://data/recipes.json", "res://data/stone_age_recipes.json", "res://data/lv_recipes.json"]:
		if FileAccess.file_exists(f) and FileAccess.get_file_as_string(f).contains("\"loose_stone\""):
			bad.append(f)
	assert_array(bad).override_failure_message("ask for #rock instead of loose_stone in " + ", ".join(bad)).is_empty()
