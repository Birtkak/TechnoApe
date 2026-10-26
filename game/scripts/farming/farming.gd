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
## data yet. Every voxel of farmland and every crop voxel is tracked and ticks,
## but Minecraft's distances are in 0.5 m blocks (Settings.BLOCK voxels, ADR 0034):
## water within 4 blocks (2 m) at the farmland's height or one block above, and
## the growth neighbours and rows one block away, so a brushed patch of 16 crops
## is one Minecraft crop. Fall heights are in metres, like Minecraft's.
##
## Crops are multi-voxel plants (ADR 0053, PlantStamps.crop): one plant per 2 x 2 voxels, its anchor
## (the voxel above the farmland) holding the age material, its stalks, leaves and ears around and
## above it; `parts` maps every voxel of a plant to its anchor, so breaking any of them takes the plant.
## Tests on one-voxel blocks (block = 1) keep single-voxel crops.
##
## Only farmland and crops the player made are tracked (a few hundred voxels),
## so the random ticks run over this list, never over terrain. Voxel reads and
## writes go through Callables so tests run on a plain dictionary world.
## tick() and the edit hooks return events {pos, drops} for Main to spawn.

const SAVE_VERSION := 1
const SECTION := 4096.0

var crops_def := {}           # kind -> definition (data/crops.json)
var random_tick_speed := 3
var hydration_radius := 4     # blocks (BLOCK voxels each)
var block := Settings.BLOCK   # voxels per Minecraft block (tests may change it)
var max_moisture := 7
var farmland := {}            # Vector3i -> moisture 0..7
var crops := {}               # Vector3i -> {kind, age}
var parts := {}               # Vector3i -> anchor: every voxel of a multi-voxel crop (not saved: rebuilt)
var _owned := {}              # anchor -> [Vector3i]: its parts besides itself
var _ear := GameMaterials.id_of("wheat_ear")
var errors: Array[String] = []
var get_voxel: Callable       # (Vector3i) -> material id
var set_voxel: Callable       # (Vector3i, material id) -> void
var is_editable: Callable     # (Vector3i) -> bool: the terrain there is loaded (unset in tests: always)
var rng := RandomNumberGenerator.new()
var season_growth := 1.0     # Calendar.crop_growth(): 1 spring/summer, 0.5 autumn, 0 winter (ADR 0025)
var dirty := false
var stats := {"random_ticks": 0, "grown": 0, "dried": 0, "trampled": 0, "popped": 0}
var _material_ids := {}       # name -> id
var _look_kind := {}          # crop material id -> kind
var _ticks := 0               # game ticks so far (water cache)
var _water_cache := {}        # block coordinate -> [near water, valid until tick]
const WATER_CACHE_TICKS := 400   # 20 s: a scan reads up to ~5 k voxels (~2 ms), shared by the 16 voxels of a block
const SCANS_PER_TICK_CALL := 1   # water scans per tick() call; farmland that needs another skips this random tick
var _scans_left := 0


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
	if not GameMaterials.is_grass(m) and m != GameMaterials.DIRT and m != GameMaterials.RED_EARTH:
		return false
	if get_voxel.call(p + Vector3i.UP) != GameMaterials.AIR:
		return false
	set_voxel.call(p, GameMaterials.FARMLAND)
	farmland[p] = 0
	dirty = true
	return true


## Plants `kind` on the farmland at `soil` (the crop goes in the voxel above, snapped to the crop grid).
func plant(soil: Vector3i, kind: String) -> bool:
	if stamped():
		soil = PlantStamps.crop_anchor(soil) + Vector3i.DOWN
	if not crops_def.has(kind) or not is_farmland_material(get_voxel.call(soil)):
		return false
	var p := soil + Vector3i.UP
	if get_voxel.call(p) != GameMaterials.AIR or parts.has(p):
		return false
	if not farmland.has(soil):
		farmland[soil] = 7 if get_voxel.call(soil) == GameMaterials.FARMLAND_MOIST else 0   # generated or untracked farmland
	crops[p] = {"kind": kind, "age": 0}
	set_voxel.call(p, look_of(kind, 0))
	_restamp(p)
	dirty = true
	return true


## Multi-voxel crops at the game's scale (blocks of 2 voxels or more).
func stamped() -> bool:
	return block >= PlantStamps.CROP_GRID


## The voxels the crop at anchor `p` would take at its age (anchor included).
func stamp_of(p: Vector3i) -> Dictionary:
	var c: Dictionary = crops[p]
	if not stamped():
		return {Vector3i.ZERO: look_of(c["kind"], c["age"])}
	return PlantStamps.crop(c["kind"], int(c["age"]), look_of(c["kind"], c["age"]), p)


## Rewrites the plant at anchor `p` for its age: old parts it no longer has go, new ones fill free cells.
func _restamp(p: Vector3i) -> void:
	var st := stamp_of(p)
	var want := {}
	for o in st:
		want[p + o] = st[o]
	_clear_parts(p, want)
	var owned: Array = _owned.get(p, [])
	for q in want:
		if q == p:
			continue
		if parts.has(q) and parts[q] != p:
			continue   # a neighbour's leaf
		if parts.has(q) or PlantStamps.is_free_cell(get_voxel.call(q)):
			set_voxel.call(q, want[q])
			if not parts.has(q):
				owned.append(q)
			parts[q] = p
	parts[p] = p
	_owned[p] = owned


## Clears the parts of the plant at `p` that are not in `keep` (cell -> material); with nothing to
## keep the plant is gone (the anchor voxel itself is the caller's).
func _clear_parts(p: Vector3i, keep: Dictionary = {}) -> void:
	var owned: Array = _owned.get(p, [])
	var left := []
	for q in owned:
		if keep.has(q):
			left.append(q)
			continue
		parts.erase(q)
		var m: int = get_voxel.call(q)
		if is_crop_material(m) or m == _ear:
			set_voxel.call(q, GameMaterials.AIR)
	if keep.is_empty():
		parts.erase(p)
		_owned.erase(p)
	else:
		_owned[p] = left


## A voxel was broken by the player: crop drops (by age), and a crop standing
## on broken farmland pops off. Returns events [{pos, drops}].
func on_broken(p: Vector3i) -> Array:
	var out := []
	if parts.has(p) and not crops.has(p):
		var a: Vector3i = parts[p]
		parts.erase(p)
		if crops.has(a):
			out.append({"pos": a, "drops": _crop_drops(crops[a])})
			_remove_crop(a)
			if is_crop_material(get_voxel.call(a)):
				set_voxel.call(a, GameMaterials.AIR)
		return out
	if crops.has(p):
		out.append({"pos": p, "drops": _crop_drops(crops[p])})
		_remove_crop(p)
	if farmland.has(p):
		farmland.erase(p)
		dirty = true
		out.append_array(_pop_crop(p + Vector3i.UP))
	return out


## Takes the crop at anchor `p` out: its other voxels become air (the anchor is the caller's).
func _remove_crop(p: Vector3i) -> void:
	_clear_parts(p)
	crops.erase(p)
	dirty = true


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
	_ticks += n
	_scans_left = SCANS_PER_TICK_CALL
	var p_tick := 1.0 - pow(1.0 - float(random_tick_speed) / SECTION, n)
	# a random tick on unloaded terrain is skipped: its writes were dropped ("area not editable") after
	# the crop was already forgotten, leaving ghost crops (bug report logs 2026-10-25)
	for p in farmland.keys():
		if rng.randf() < p_tick and _loaded(p):
			out.append_array(_farmland_tick(p))
	for p in crops.keys():
		if crops.has(p) and rng.randf() < p_tick and _loaded(p):
			out.append_array(_crop_tick(p))
	return out


func _loaded(p: Vector3i) -> bool:
	return not is_editable.is_valid() or bool(is_editable.call(p))


func _farmland_tick(p: Vector3i) -> Array:
	stats["random_ticks"] += 1
	var m: int = get_voxel.call(p)
	if not is_farmland_material(m):
		farmland.erase(p)   # replaced by the simulation or an edit we did not see
		dirty = true
		return _pop_crop(p + Vector3i.UP)
	var moisture: int = farmland[p]
	if not _water_known(p):
		if _scans_left <= 0:
			return []   # the scan budget is spent: this random tick is lost, the next one scans
		_scans_left -= 1
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
		_remove_crop(p)   # burnt or overwritten
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
	if stamped():
		_restamp(p)


## Minecraft CropBlock.getGrowthSpeed for the crop at p, its neighbours one block away.
func growth_speed(p: Vector3i) -> float:
	var kind: String = crops[p]["kind"]
	var f := 1.0
	var below := p + Vector3i.DOWN
	var b := block
	for i in range(-1, 2):
		for j in range(-1, 2):
			var q := below + Vector3i(i * b, 0, j * b)
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
	var west_east: bool = same.call(Vector3i(-b, 0, 0)) or same.call(Vector3i(b, 0, 0))
	var north_south: bool = same.call(Vector3i(0, 0, -b)) or same.call(Vector3i(0, 0, b))
	if west_east and north_south:
		f /= 2.0
	elif same.call(Vector3i(-b, 0, -b)) or same.call(Vector3i(b, 0, -b)) or same.call(Vector3i(b, 0, b)) or same.call(Vector3i(-b, 0, b)):
		f /= 2.0
	return f


## Water within hydration_radius blocks horizontally, at the farmland's level
## or up to one block above. The answer is kept per block for WATER_CACHE_TICKS,
## so the voxels of one block share a scan.
func near_water(p: Vector3i) -> bool:
	var key := _block_of(p)
	var hit: Array = _water_cache.get(key, [])
	if not hit.is_empty() and _ticks < int(hit[1]):
		return hit[0]
	var wet := _scan_water(p)
	if _water_cache.size() > 4096:
		_water_cache.clear()
	_water_cache[key] = [wet, _ticks + WATER_CACHE_TICKS]
	return wet


func _block_of(p: Vector3i) -> Vector3i:
	return Vector3i(floori(float(p.x) / block), floori(float(p.y) / block), floori(float(p.z) / block))


func _water_known(p: Vector3i) -> bool:
	var hit: Array = _water_cache.get(_block_of(p), [])
	return not hit.is_empty() and _ticks < int(hit[1])


func _scan_water(p: Vector3i) -> bool:
	var r := hydration_radius * block
	for dy in range(0, block + 1):
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
	_remove_crop(p)
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
	# the parts are where the stamps put them (the world is not loaded yet: trust the save)
	parts.clear()
	_owned.clear()
	for p in crops:
		var st := stamp_of(p)
		var owned := []
		for o in st:
			if not parts.has(p + o):
				parts[p + o] = p
				if o != Vector3i.ZERO:
					owned.append(p + o)
		_owned[p] = owned
	dirty = false
	return true


func summary() -> Dictionary:
	var ages := {}
	for p in crops:
		var k := "%s_%d" % [crops[p]["kind"], crops[p]["age"]]
		ages[k] = int(ages.get(k, 0)) + 1
	return {"farmland": farmland.size(), "crops": crops.size(), "ages": ages, "stats": stats}
