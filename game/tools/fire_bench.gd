extends SceneTree

## Fire pacing + cost benchmark (headless). Builds small scenes in a SimWorld
## with the game's material table, ignites them and reports, in seconds at
## Settings.sim_tick_hz: how far and how fast the fire front travels, how long
## a voxel burns, when the scene is burnt out, peak burning voxels, and cost
## (tick ms, dirty chunks per tick = terrain pushes the bubble would make).
##   godot --headless --path game --script res://tools/fire_bench.gd [-- hz=30 seconds=90]

var hz := 30.0
var seconds := 90.0
var overrides := {}   # set=tall_grass.fire_hp:40,tall_grass.ignite_chance:0.2  (tries data values before they go into materials.gd)
var only := ""


func _init() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2 and kv[0] == "hz":
			hz = float(kv[1])
		elif kv.size() == 2 and kv[0] == "seconds":
			seconds = float(kv[1])
		elif kv.size() == 2 and kv[0] == "only":
			only = kv[1]
		elif kv.size() == 2 and kv[0] == "set":
			for item in kv[1].split(","):
				var nv := item.split(":")
				var mk := nv[0].split(".")
				if not overrides.has(mk[0]):
					overrides[mk[0]] = {}
				overrides[mk[0]][mk[1]] = float(nv[1])
	var results := []
	results.append(_run("wood_beam", _beam))
	results.append(_run("wood_wall", _wall))
	results.append(_run("tree", _tree))
	results.append(_run("grass_field", _grass))
	results.append(_run("forest_floor", _floor))
	results.append(_run("wood_by_water", _wet))
	for r in results:
		if not r.get("skipped", false):
			print(JSON.stringify(r))
	quit()


func _world() -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(6, 3, 6), maxi(2, OS.get_processor_count() - 2), 7)
	var defs := GameMaterials.defs()
	for d in defs:
		if overrides.has(d["name"]):
			for k in overrides[d["name"]]:
				d[k] = overrides[d["name"]][k]
	w.set_materials(defs)
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(96, 4, 96), GameMaterials.STONE)   # floor
	return w


# ---- scenes: return {"ignite": [points], "front_axis": Vector3i dir, "origin": Vector3i, "mat": id, "count": n} ----

func _beam(w: SimWorld) -> Dictionary:
	w.fill_box(Vector3i(8, 4, 40), Vector3i(72, 5, 41), GameMaterials.WOOD)
	return {"ignite": [Vector3i(8, 4, 40)], "origin": Vector3i(8, 4, 40), "axis": Vector3i(1, 0, 0), "mat": GameMaterials.WOOD}


func _wall(w: SimWorld) -> Dictionary:
	w.fill_box(Vector3i(8, 4, 40), Vector3i(40, 20, 41), GameMaterials.PLANKS)
	return {"ignite": [Vector3i(8, 4, 40)], "origin": Vector3i(8, 4, 40), "axis": Vector3i(1, 0, 0), "mat": GameMaterials.PLANKS}


func _tree(w: SimWorld) -> Dictionary:
	w.fill_box(Vector3i(46, 4, 46), Vector3i(48, 20, 48), GameMaterials.WOOD)          # 1 m trunk, 8 m high
	w.fill_box(Vector3i(38, 16, 38), Vector3i(56, 26, 56), GameMaterials.LEAVES)        # 9 m canopy
	w.fill_box(Vector3i(46, 16, 46), Vector3i(48, 20, 48), GameMaterials.WOOD)
	return {"ignite": [Vector3i(46, 4, 46)], "origin": Vector3i(46, 4, 46), "axis": Vector3i(0, 1, 0), "mat": GameMaterials.LEAVES}


func _grass(w: SimWorld) -> Dictionary:
	w.fill_box(Vector3i(0, 3, 0), Vector3i(96, 4, 96), GameMaterials.DIRT)
	w.fill_box(Vector3i(0, 4, 0), Vector3i(96, 5, 96), GameMaterials.TALL_GRASS)
	return {"ignite": [Vector3i(48, 4, 48)], "origin": Vector3i(48, 4, 48), "axis": Vector3i(1, 0, 0), "mat": GameMaterials.TALL_GRASS}


## Tufts on 30 % of the ground (a FineGen meadow or forest floor): near the cover where touching tufts stop
## connecting, so "left" says whether a grass fire keeps going (at 20 % it dies out whatever the tuning).
func _floor(w: SimWorld) -> Dictionary:
	w.fill_box(Vector3i(0, 3, 0), Vector3i(96, 4, 96), GameMaterials.DIRT)
	w.scatter(Vector3i(0, 4, 0), Vector3i(96, 5, 96), GameMaterials.TALL_GRASS, 0.3, 11)
	w.fill_box(Vector3i(46, 4, 46), Vector3i(51, 5, 51), GameMaterials.TALL_GRASS)   # a lit patch to start from
	return {"ignite": [Vector3i(48, 4, 48)], "origin": Vector3i(48, 4, 48), "axis": Vector3i(1, 0, 0), "mat": GameMaterials.TALL_GRASS}


func _wet(w: SimWorld) -> Dictionary:
	w.fill_box(Vector3i(8, 4, 40), Vector3i(40, 5, 41), GameMaterials.WOOD)
	w.fill_box(Vector3i(24, 4, 41), Vector3i(25, 5, 42), GameMaterials.WATER_STATIC)  # water touching the beam at x=24
	return {"ignite": [Vector3i(8, 4, 40)], "origin": Vector3i(8, 4, 40), "axis": Vector3i(1, 0, 0), "mat": GameMaterials.WOOD}


func _run(name: String, build: Callable) -> Dictionary:
	if only != "" and not name in only.split("+"):
		return {"scene": name, "skipped": true}
	var w := _world()
	var s: Dictionary = build.call(w)
	var start: int = w.count_material(s["mat"])
	for p in s["ignite"]:
		w.ignite(p)
	w.clear_dirty()
	var ticks := int(seconds * hz)
	var front := 0
	var front_t := {}          # distance reached -> first second
	var peak_burning := 0
	var burnt_out_s := -1.0
	var max_tick_ms := 0.0
	var sum_tick_ms := 0.0
	var max_dirty := 0
	var sum_dirty := 0
	var first_out_s := -1.0
	var origin: Vector3i = s["origin"]
	var axis: Vector3i = s["axis"]
	for t in ticks:
		var r: Dictionary = w.tick(1)
		var ms: float = r["ms_per_tick"]
		max_tick_ms = maxf(max_tick_ms, ms)
		sum_tick_ms += ms
		var dirty: int = w.get_dirty_chunks().size()
		w.clear_dirty()
		max_dirty = maxi(max_dirty, dirty)
		sum_dirty += dirty
		var burning := w.count_burning()
		peak_burning = maxi(peak_burning, burning)
		if first_out_s < 0.0 and w.get_voxel(origin) != s["mat"]:
			first_out_s = t / hz
		# front: furthest voxel along the axis that is burning or burnt (no longer the material)
		if t % 3 == 0:
			for d in range(front + 1, 64):
				var p := origin + axis * d
				var m := w.get_voxel(p)
				if m == s["mat"] and not _burning(w, p):
					break
				front = d
				if not front_t.has(d):
					front_t[d] = snappedf(t / hz, 0.1)
		if burning == 0 and t > 10 and burnt_out_s < 0.0:
			burnt_out_s = t / hz
			break
	var left: int = w.count_material(s["mat"])
	var speed := 0.0
	if front >= 8 and front_t.has(front):
		speed = front / maxf(float(front_t[front]), 0.1)
	return {"scene": name, "hz": hz, "voxels": start, "left": left, "front_vox": front,
		"front_t_s": {"4": front_t.get(4, -1), "8": front_t.get(8, -1), "16": front_t.get(16, -1), "32": front_t.get(32, -1)},
		"spread_vox_per_s": snappedf(speed, 0.01), "first_voxel_burnt_s": snappedf(first_out_s, 0.1),
		"burnt_out_s": snappedf(burnt_out_s, 0.1), "peak_burning": peak_burning,
		"tick_ms_avg": snappedf(sum_tick_ms / maxf(ticks, 1), 0.001), "tick_ms_max": snappedf(max_tick_ms, 0.01),
		"dirty_chunks_avg": snappedf(float(sum_dirty) / ticks, 0.1), "dirty_chunks_max": max_dirty}


func _burning(w: SimWorld, p: Vector3i) -> bool:
	return (w.get_flags(p) & 1) != 0
