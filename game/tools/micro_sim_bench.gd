extends SceneTree

## P10: the existing native kernel (SimWorld, P5) run at micro resolution
## (ADR 0012 option B) under heavy activity. Scenes are defined in metres, so
## cell=0.0625 (6.25 cm) and cell=0.125 compare the same events. Every scene
## runs in its own SimWorld of 32 x 16 x 32 m (16^3-cell chunks, only active
## chunks tick) and prints one JSON line: tick ms p50/p95/max, active chunks,
## moved cells and dirty chunks per tick (each dirty chunk is a 4 KB upload
## to the near-field volume), seconds until settled, and the physical speeds
## that matter at this scale.
##   godot --headless --path game --script res://tools/micro_sim_bench.gd [-- only=tnt+flood cell=0.0625 hz=30 seconds=20 threads=10]

var cell := 0.0625        # metres per cell
var hz := 30.0
var seconds := 20.0
var threads := maxi(2, OS.get_processor_count() - 2)
var only := ""
var boxes := true        # v5 dirty boxes (false = P5 whole-chunk scan)
var gravity := false     # v5 fall velocity at 9.8 m/s^2 (false = one cell per tick, P5)
var budget := 0          # v6 cells scanned per tick (0 = unlimited)
var fullness := false    # P10 liquid fullness (eighths, mass-conserving spread)
var lod := -1            # v6 full-rate radius in metres around the world centre (-1 = off), far chunks every 3rd tick
const WORLD_M := Vector3(32.0, 16.0, 32.0)
const FLOOR_M := 1.0


func _init() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() != 2:
			continue
		match kv[0]:
			"cell": cell = float(kv[1])
			"hz": hz = float(kv[1])
			"seconds": seconds = float(kv[1])
			"threads": threads = int(kv[1])
			"only": only = kv[1]
			"boxes": boxes = kv[1] == "1"
			"gravity": gravity = kv[1] == "1"
			"budget": budget = int(kv[1])
			"lod": lod = int(kv[1])
			"fullness": fullness = kv[1] == "1"
	var scenes := {"fall": _fall, "avalanche": _avalanche, "flood": _flood, "tnt_static": _tnt_static,
		"tnt_loose": _tnt_loose, "black_hole": _black_hole, "fire": _fire}
	for name in scenes:
		if only != "" and not name in only.split("+"):
			continue
		print(JSON.stringify(_run(name, scenes[name])))
	quit()


func m(metres: float) -> int:
	return int(round(metres / cell))


## Box from metre corners.
func box(w: SimWorld, a: Vector3, b: Vector3, mat: int) -> void:
	w.fill_box(Vector3i(m(a.x), m(a.y), m(a.z)), Vector3i(m(b.x), m(b.y), m(b.z)), mat)


func size_chunks() -> Vector3i:
	return Vector3i(m(WORLD_M.x) / 16, m(WORLD_M.y) / 16, m(WORLD_M.z) / 16)


func _world() -> SimWorld:
	var w := SimWorld.new()
	w.configure(size_chunks(), threads, 7)
	w.set_materials(GameMaterials.defs())
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	w.set_dirty_boxes(boxes)
	if gravity:
		w.set_gravity(9.8 / (cell * hz * hz))
	w.set_focus(Vector3i(m(WORLD_M.x / 2.0), m(FLOOR_M + 6.0), m(WORLD_M.z / 2.0)))
	w.set_liquid_fullness(fullness)
	if budget > 0:
		w.set_cell_budget(budget)
	if lod >= 0:
		w.set_lod(int(ceil(m(float(lod)) / 16.0)), 3)
	box(w, Vector3(0, 0, 0), Vector3(WORLD_M.x, FLOOR_M, WORLD_M.z), GameMaterials.STONE)
	return w


# ---- scenes (metres): return {"step": Callable(w, tick)?, "probe": Callable(w) -> Dictionary?} ----

## A 4 m column of sand dropped from 6 m: when does the first grain land.
func _fall(w: SimWorld) -> Dictionary:
	var f := FLOOR_M
	box(w, Vector3(15.5, f + 6.0, 15.5), Vector3(16.5, f + 10.0, 16.5), GameMaterials.SAND)
	return {"probe": func(_w: SimWorld) -> Dictionary:
		return {"landed": _w.count_in_box(Vector3i(m(14), m(f), m(14)), Vector3i(m(18), m(f) + 1, m(18)), GameMaterials.SAND)}}


## A 6 m cube of loose sand released in the open.
func _avalanche(w: SimWorld) -> Dictionary:
	box(w, Vector3(13, FLOOR_M, 13), Vector3(19, FLOOR_M + 6.0, 19), GameMaterials.SAND)
	return {}


## 8 x 4 x 8 m of water released: how fast does the front run.
func _flood(w: SimWorld) -> Dictionary:
	var f := FLOOR_M
	box(w, Vector3(12, f, 12), Vector3(20, f + 4.0, 20), GameMaterials.WATER)
	return {"probe": func(_w: SimWorld) -> Dictionary:
		var reach := 0.0
		for d in range(0, 12):
			if _w.count_in_box(Vector3i(m(20.0 + d), m(f), m(12)), Vector3i(m(20.0 + d) + 1, m(f) + 2, m(20)), GameMaterials.WATER) > 0:
				reach = d
		return {"front_m": reach}}


## Hill as the generator makes it (static ground: stone core, 6 m of
## sand_static) with a 2 m water pocket; an 8 m crater (the 1000-TNT event)
## blown into it at t = 0. Static ground keeps its walls (Minecraft-like);
## only the released water and debris move.
func _tnt_static(w: SimWorld) -> Dictionary:
	_hill(w, GameMaterials.SAND_STATIC)
	_carve(w, Vector3(16, FLOOR_M + 7.0, 16), 4.0, 0)
	return {}


## Worst case: the same hill made of loose sand (as if every disturbed grain
## turned loose, Noita-style): the whole hill slumps.
func _tnt_loose(w: SimWorld) -> Dictionary:
	_hill(w, GameMaterials.SAND)
	_carve(w, Vector3(16, FLOOR_M + 7.0, 16), 4.0, 0)
	return {}


## A 2.5 m black hole moving at 4 m/s through the static hill for 3 s.
func _black_hole(w: SimWorld) -> Dictionary:
	_hill(w, GameMaterials.SAND_STATIC)
	return {"step": func(_w: SimWorld, t: int) -> void:
		var secs := t / hz
		if secs <= 3.0:
			_carve(_w, Vector3(8.0 + secs * 4.0, FLOOR_M + 6.0, 16), 2.5, 0)}


func _hill(w: SimWorld, ground: int) -> void:
	var f := FLOOR_M
	box(w, Vector3(6, f, 6), Vector3(26, f + 4.0, 26), GameMaterials.STONE)
	box(w, Vector3(6, f + 4.0, 6), Vector3(26, f + 10.0, 26), ground)
	box(w, Vector3(18.5, f + 7.0, 12.5), Vector3(21, f + 9.0, 15), GameMaterials.WATER)


## A 6 m tree (1 m trunk, 6 x 3 x 6 m canopy) over grass, lit at the base.
func _fire(w: SimWorld) -> Dictionary:
	var f := FLOOR_M
	w.fill_box(Vector3i(0, m(f), 0), Vector3i(m(WORLD_M.x), m(f) + 1, m(WORLD_M.z)), GameMaterials.TALL_GRASS)
	box(w, Vector3(15.5, f, 15.5), Vector3(16.5, f + 6.0, 16.5), GameMaterials.WOOD)
	box(w, Vector3(13, f + 4.0, 13), Vector3(19, f + 7.0, 19), GameMaterials.LEAVES)
	box(w, Vector3(15.5, f + 4.0, 15.5), Vector3(16.5, f + 6.0, 16.5), GameMaterials.WOOD)
	for x in range(m(15.5), m(16.5)):
		w.ignite(Vector3i(x, m(f), m(15.5)))
	return {"probe": func(_w: SimWorld) -> Dictionary:
		return {"burning": _w.count_burning()}}


## Sphere (metres) of `mat` written as fill_box rows: C++ does the cells.
func _carve(w: SimWorld, centre_m: Vector3, radius_m: float, mat: int) -> void:
	var c := Vector3i(m(centre_m.x), m(centre_m.y), m(centre_m.z))
	var r := m(radius_m)
	for dz in range(-r, r + 1):
		for dy in range(-r, r + 1):
			var rr := r * r - dz * dz - dy * dy
			if rr < 0:
				continue
			var hx := int(sqrt(float(rr)))
			w.fill_box(Vector3i(c.x - hx, c.y + dy, c.z + dz), Vector3i(c.x + hx + 1, c.y + dy + 1, c.z + dz + 1), mat)


func _run(name: String, build: Callable) -> Dictionary:
	var w := _world()
	var t_build := Time.get_ticks_usec()
	var s: Dictionary = build.call(w)
	var build_ms := (Time.get_ticks_usec() - t_build) / 1000.0
	w.tick(1)          # the fills activate every touched chunk once; measure from the second tick
	w.clear_dirty()
	var ticks := int(seconds * hz)
	var tick_ms := PackedFloat32Array()
	var active_max := 0
	var moved_max := 0
	var moved_total := 0
	var dirty := PackedInt32Array()
	var scanned_max := 0
	var deferred_max := 0
	var settled_s := -1.0
	var quiet := 0
	var probes := []
	var first_land_s := -1.0
	for t in ticks:
		if s.has("step"):
			s["step"].call(w, t)
		var r: Dictionary = w.tick(1)
		tick_ms.append(r["ms_per_tick"])
		active_max = maxi(active_max, int(r["active_chunks_per_tick"]))
		moved_max = maxi(moved_max, int(r["moved_cells_per_tick"]))
		moved_total += int(r["moved_cells_per_tick"])
		scanned_max = maxi(scanned_max, int(r.get("scanned_cells_per_tick", 0)))
		deferred_max = maxi(deferred_max, int(r.get("deferred_chunks_per_tick", 0)))
		var dc: int = w.get_dirty_chunks().size()
		w.clear_dirty()
		dirty.append(dc)
		if s.has("probe") and (t % int(hz / 2) == 0):
			var p: Dictionary = s["probe"].call(w)
			p["t"] = snappedf(t / hz, 0.1)
			probes.append(p)
			if name == "fall" and first_land_s < 0.0 and int(p["landed"]) > 0:
				first_land_s = t / hz
		var still: bool = int(r["moved_cells_per_tick"]) == 0 and int(r["reactions_per_tick"]) == 0
		if still and not (s.has("step") and t / hz <= 3.0):
			quiet += 1
			if quiet >= 5 and settled_s < 0.0:
				settled_s = (t - 4) / hz
				break
		else:
			quiet = 0
	var sorted := tick_ms.duplicate()
	sorted.sort()
	var dsorted := dirty.duplicate()
	dsorted.sort()
	var n := sorted.size()
	var sum := 0.0
	for v in tick_ms:
		sum += v
	var out := {"scene": name, "cell_m": cell, "hz": hz, "threads": threads, "boxes": boxes, "gravity": gravity, "budget": budget, "lod_m": lod, "fullness": fullness,
		"cells_m": snappedf(size_chunks().x * size_chunks().y * size_chunks().z * 4096 / 1.0e6, 0.1), "build_ms": snappedf(build_ms, 0.1),
		"ticks": n, "settled_s": snappedf(settled_s, 0.1),
		"tick_ms": {"p50": snappedf(sorted[n / 2], 0.01), "p95": snappedf(sorted[int(n * 0.95)], 0.01), "max": snappedf(sorted[n - 1], 0.01), "sum_s": snappedf(sum / 1000.0, 0.01)},
		"active_chunks_max": active_max, "scanned_cells_max": scanned_max, "deferred_chunks_max": deferred_max, "moved_cells_max": moved_max, "moved_per_ms": int(moved_total / maxf(sum, 0.001)),
		"dirty_chunks": {"p50": dsorted[n / 2], "p95": dsorted[int(n * 0.95)], "max": dsorted[n - 1]},
		"upload_mb_per_s_p95": snappedf(dsorted[int(n * 0.95)] * 4096.0 * hz / 1048576.0, 0.1)}
	if name == "fall":
		out["first_landing_s"] = snappedf(first_land_s, 0.01)
		out["avg_fall_speed_m_s"] = snappedf(6.0 / maxf(first_land_s, 0.001), 0.01)
		out["free_fall_s_real"] = snappedf(sqrt(2.0 * 6.0 / 9.8), 0.01)
	if not probes.is_empty():
		out["probes"] = probes.slice(0, 41, 4)
	return out
