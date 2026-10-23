class_name Farming
extends RefCounted

## Minecraft farming (ADR 0022), data in data/crops.json: a hoe tills grass or
## dirt with air above into farmland; seeds (or a carrot) plant a crop on its
## top; random ticks grow crops and wet or dry the farmland. Rules follow
## Minecraft 1.20's FarmBlock and CropBlock:
##   - random ticks: random_tick_speed (3) of the 4096 blocks of a section per
##     20 Hz tick, so a block gets one every ~68 s on average;
##   - farmland: water within 4 blocks horizontally at its height or one above
##     sets moisture 7, else it loses 1 per random tick and turns back to dirt
##     at 0 when no crop stands on it; a solid block placed on it turns it to
##     dirt; landing on it from more than 0.5 m tramples it (chance = fall - 0.5);
##   - crops: grow one age per random tick with chance 1 / (floor(25 / f) + 1),
##     f = 1 + the farmland under it (1 dry, 3 moist) + a quarter of each of the
##     8 farmland blocks around that, halved when the same crop stands in rows
##     on both axes or on a diagonal; a crop pops off when its farmland goes.
## Minecraft's light check (crops need light 9) is left out: there is no light
## data yet. A voxel is this game's block, so radii and neighbours count voxels
## (a 9x9 field around one water voxel, as in Minecraft); fall heights are in
## metres, because jumps and falls are tuned in metres like Minecraft's.
##
## Only farmland and crops the player made are tracked (a few hundred voxels),
## so the random ticks run over this list, never over terrain. Voxel reads and
## writes go through Callables so tests run on a plain dictionary world.
## tick() and the edit hooks return events {pos, drops} for Main to spawn.

const SAVE_VERSION := 1
const SECTION := 4096.0

var crops_def := {}           # kind -> definition (data/crops.json)
var random_tick_speed := 3
var hydration_radius := 4     # voxels
var max_moisture := 7
var farmland := {}            # Vector3i -> moisture 0..7
var crops := {}               # Vector3i -> {kind, age}
var errors: Array[String] = []
var get_voxel: Callable       # (Vector3i) -> material id
var set_voxel: Callable       # (Vector3i, material id) -> void
var rng := RandomNumberGenerator.new()
var season_growth := 1.0     # Calendar.crop_growth(): 1 spring/summer, 0.5 autumn, 0 winter (ADR 0025)
var dirty := false
var stats := {"random_ticks": 0, "grown": 0, "dried": 0, "trampled": 0, "popped": 0}
var _material_ids := {}       # name -> id
var _look_kind := {}          # crop material id -> kind


static func load_default() -> Farming:
	var f := Farming.new()
	f.load_file("res://data/crops.json")
	return f


func _init() -> void:
	var d := GameMaterials.defs()
	for i in d.size():
		_material_ids[d[i]["name"]] = i + 1


func load_file(path: String) -> bool:
	var fa := FileAccess.open(path, FileAccess.READ)
	if fa == null:
		errors.append("cannot open %s" % path)
		return false
	return load_json(fa.get_as_text())


func load_json(text: String) -> bool:
	var data = JSON.parse_string(text)
	if typeof(data) != TYPE_DICTIONARY or not data.has("crops"):
		errors.append("crops.json: expected {crops: {...}}")
		return false
	random_tick_speed = int(data.get("random_tick_speed", 3))
	hydration_radius = int(data.get("hydration_radius", 4))
	max_moisture = int(data.get("max_moisture", 7))
	for kind in data["crops"]:
		var c: Dictionary = data["crops"][kind]
		var looks: Array = c.get("looks", [])
		if looks.size() != int(c.get("ages", 0)):
			errors.append("crops.json: %s needs one look per age" % kind)
			continue
		var ok := true
		for l in looks:
			if not _material_ids.has(l):
				errors.append("crops.json: %s look %s is not a material" % [kind, l])
				ok = false
			else:
				_look_kind[_material_ids[l]] = kind
		if ok:
			crops_def[kind] = c
	return errors.is_empty()


func material_id(name: String) -> int:
	return int(_material_ids.get(name, -1))


func look_of(kind: String, age: int) -> int:
	return material_id(crops_def[kind]["looks"][age])


func is_crop_material(m: int) -> bool:
	return _look_kind.has(m)


func is_farmland_material(m: int) -> bool:
	return m == GameMaterials.FARMLAND or m == GameMaterials.FARMLAND_MOIST


## Hoe on a voxel: grass or dirt with air above becomes dry farmland.
func till(p: Vector3i) -> bool:
	var m: int = get_voxel.call(p)
	if m != GameMaterials.GRASS and m != GameMaterials.DIRT:
		return false
	if get_voxel.call(p + Vector3i.UP) != GameMaterials.AIR:
		return false
	set_voxel.call(p, GameMaterials.FARMLAND)
	farmland[p] = 0
	dirty = true
	return true


## Plants `kind` on the farmland at `soil` (the crop goes in the voxel above).
func plant(soil: Vector3i, kind: String) -> bool:
	if not crops_def.has(kind) or not is_farmland_material(get_voxel.call(soil)):
		return false
	var p := soil + Vector3i.UP
	if get_voxel.call(p) != GameMaterials.AIR:
		return false
	if not farmland.has(soil):
		farmland[soil] = 7 if get_voxel.call(soil) == GameMaterials.FARMLAND_MOIST else 0   # generated or untracked farmland
	crops[p] = {"kind": kind, "age": 0}
	set_voxel.call(p, look_of(kind, 0))
	dirty = true
	return true


## A voxel was broken by the player: crop drops (by age), and a crop standing
## on broken farmland pops off. Returns events [{pos, drops}].
func on_broken(p: Vector3i) -> Array:
	var out := []
	if crops.has(p):
		out.append({"pos": p, "drops": _crop_drops(crops[p])})
		crops.erase(p)
		dirty = true
	if farmland.has(p):
		farmland.erase(p)
		dirty = true
		out.append_array(_pop_crop(p + Vector3i.UP))
	return out


## Drops for a crop material broken without tracked state (an untracked crop voxel).
func drops_for_material(m: int) -> Array:
	if not is_crop_material(m):
		return []
	var kind: String = _look_kind[m]
	var looks: Array = crops_def[kind]["looks"]
	var age := looks.rfind(GameMaterials.name_of(m))
	return _crop_drops({"kind": kind, "age": age})


## Something was placed at p: a solid block on farmland turns it to dirt.
func on_placed(p: Vector3i, m: int) -> Array:
	var below := p + Vector3i.DOWN
	if farmland.has(below) and m > 0 and not is_crop_material(m) and _blocks_farmland(m):
		return _to_dirt(below)
	return []


## A body landed on `p` (the voxel under its feet) after falling fall_m metres.
func trample(p: Vector3i, fall_m: float) -> Array:
	if not is_farmland_material(get_voxel.call(p)):
		return []
	if rng.randf() >= fall_m - 0.5:
		return []
	stats["trampled"] += 1
	if not farmland.has(p):
		farmland[p] = 0
	return _to_dirt(p)


## Advances `n` game ticks (20 Hz) of random ticks over every tracked voxel.
func tick(n: int = 20) -> Array:
	var out := []
	var p_tick := 1.0 - pow(1.0 - float(random_tick_speed) / SECTION, n)
	for p in farmland.keys():
		if rng.randf() < p_tick:
			out.append_array(_farmland_tick(p))
	for p in crops.keys():
		if crops.has(p) and rng.randf() < p_tick:
			out.append_array(_crop_tick(p))
	return out


func _farmland_tick(p: Vector3i) -> Array:
	stats["random_ticks"] += 1
	var m: int = get_voxel.call(p)
	if not is_farmland_material(m):
		farmland.erase(p)   # replaced by the simulation or an edit we did not see
		dirty = true
		return _pop_crop(p + Vector3i.UP)
	var moisture: int = farmland[p]
	if near_water(p):
		if moisture < max_moisture:
			_set_moisture(p, max_moisture)
	elif moisture > 0:
		_set_moisture(p, moisture - 1)
	elif not crops.has(p + Vector3i.UP):
		stats["dried"] += 1
		return _to_dirt(p)
	return []


func _crop_tick(p: Vector3i) -> Array:
	stats["random_ticks"] += 1
	var c: Dictionary = crops[p]
	if get_voxel.call(p) != look_of(c["kind"], c["age"]):
		crops.erase(p)   # burnt or overwritten
		dirty = true
		return []
	if not farmland.has(p + Vector3i.DOWN) and not is_farmland_material(get_voxel.call(p + Vector3i.DOWN)):
		return _pop_crop(p)
	var max_age: int = int(crops_def[c["kind"]]["ages"]) - 1
	if c["age"] < max_age:
		var f := growth_speed(p)
		# seasons (ADR 0025): the Minecraft roll, then the season's share of it
		if rng.randi_range(0, int(25.0 / f)) == 0 and rng.randf() < season_growth:
			grow(p)
	return []


## One age up (bone meal, tests): swaps the crop's look when it changes.
func grow(p: Vector3i, ages: int = 1) -> void:
	var c: Dictionary = crops[p]
	var max_age: int = int(crops_def[c["kind"]]["ages"]) - 1
	var before := look_of(c["kind"], c["age"])
	c["age"] = mini(int(c["age"]) + ages, max_age)
	stats["grown"] += 1
	dirty = true
	var after := look_of(c["kind"], c["age"])
	if after != before:
		set_voxel.call(p, after)


## Minecraft CropBlock.getGrowthSpeed for the crop at p.
func growth_speed(p: Vector3i) -> float:
	var kind: String = crops[p]["kind"]
	var f := 1.0
	var below := p + Vector3i.DOWN
	for i in range(-1, 2):
		for j in range(-1, 2):
			var q := below + Vector3i(i, 0, j)
			var g := 0.0
			var m: int = get_voxel.call(q)
			if is_farmland_material(m):
				g = 1.0
				if int(farmland.get(q, max_moisture if m == GameMaterials.FARMLAND_MOIST else 0)) > 0:
					g = 3.0
			if i != 0 or j != 0:
				g /= 4.0
			f += g
	var same := func(d: Vector3i) -> bool: return crops.has(p + d) and crops[p + d]["kind"] == kind
	var west_east: bool = same.call(Vector3i(-1, 0, 0)) or same.call(Vector3i(1, 0, 0))
	var north_south: bool = same.call(Vector3i(0, 0, -1)) or same.call(Vector3i(0, 0, 1))
	if west_east and north_south:
		f /= 2.0
	elif same.call(Vector3i(-1, 0, -1)) or same.call(Vector3i(1, 0, -1)) or same.call(Vector3i(1, 0, 1)) or same.call(Vector3i(-1, 0, 1)):
		f /= 2.0
	return f


## Water within hydration_radius voxels horizontally, at the farmland's level
## or one above (at most 162 reads, once per random tick of a farmland voxel).
func near_water(p: Vector3i) -> bool:
	var r := hydration_radius
	for dy in range(0, 2):
		for dx in range(-r, r + 1):
			for dz in range(-r, r + 1):
				var m: int = get_voxel.call(p + Vector3i(dx, dy, dz))
				if m == GameMaterials.WATER or m == GameMaterials.WATER_STATIC:
					return true
	return false


func _set_moisture(p: Vector3i, moisture: int) -> void:
	var was: int = farmland[p]
	farmland[p] = moisture
	dirty = true
	if (was == max_moisture) != (moisture == max_moisture):
		set_voxel.call(p, GameMaterials.FARMLAND_MOIST if moisture == max_moisture else GameMaterials.FARMLAND)


func _to_dirt(p: Vector3i) -> Array:
	farmland.erase(p)
	set_voxel.call(p, GameMaterials.DIRT)
	dirty = true
	return _pop_crop(p + Vector3i.UP)


func _pop_crop(p: Vector3i) -> Array:
	if not crops.has(p):
		return []
	var c: Dictionary = crops[p]
	crops.erase(p)
	dirty = true
	stats["popped"] += 1
	if get_voxel.call(p) == look_of(c["kind"], c["age"]):
		set_voxel.call(p, GameMaterials.AIR)
	return [{"pos": p, "drops": _crop_drops(c)}]


func _crop_drops(c: Dictionary) -> Array:
	var d: Dictionary = crops_def[c["kind"]]
	var ripe: bool = int(c["age"]) >= int(d["ages"]) - 1
	return MiningTable.roll_list(d["drops_ripe"] if ripe else d["drops_young"], rng)


## Solid, collidable placements smother farmland; plants, carpets and liquids do not.
func _blocks_farmland(m: int) -> bool:
	var d: Dictionary = GameMaterials.defs()[m - 1]
	var ct: String = d.get("cell_type", "solid")
	return (ct == "solid" or ct == "powder") and not d.has("micro")


# ---------------- persistence ----------------

func to_dict() -> Dictionary:
	var fl := []
	for p in farmland:
		fl.append([p.x, p.y, p.z, farmland[p]])
	var cl := []
	for p in crops:
		cl.append([p.x, p.y, p.z, crops[p]["kind"], crops[p]["age"]])
	return {"version": SAVE_VERSION, "farmland": fl, "crops": cl}


func load_dict(d: Dictionary) -> bool:
	if int(d.get("version", 0)) != SAVE_VERSION:
		return false
	farmland.clear()
	crops.clear()
	for e in d.get("farmland", []):
		farmland[Vector3i(int(e[0]), int(e[1]), int(e[2]))] = int(e[3])
	for e in d.get("crops", []):
		if crops_def.has(String(e[3])):
			crops[Vector3i(int(e[0]), int(e[1]), int(e[2]))] = {"kind": String(e[3]), "age": int(e[4])}
	dirty = false
	return true


func summary() -> Dictionary:
	var ages := {}
	for p in crops:
		var k := "%s_%d" % [crops[p]["kind"], crops[p]["age"]]
		ages[k] = int(ages.get(k, 0)) + 1
	return {"farmland": farmland.size(), "crops": crops.size(), "ages": ages, "stats": stats}
