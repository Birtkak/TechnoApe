extends SceneTree

## P5 CPU benchmark (v4, data-driven materials): runs SimWorld scenarios and
## thread counts, prints one JSON line per run.
##   godot_console.exe --headless --path . --script res://bench_sim.gd -- threads=1,6,12

var thread_sets: Array = [1, 6, 12]


func _init() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2 and kv[0] == "threads":
			thread_sets = Array(kv[1].split(",")).map(func(s): return int(s))
	if not ClassDB.class_exists("SimWorld"):
		push_error("SimWorld extension not loaded")
		quit(1)
		return
	for threads in thread_sets:
		_run("A_sand_column", threads, 200, _setup_sand)
		_run("B_water_release", threads, 200, _setup_water)
		_run("C_worst_case_30pct", threads, 60, _setup_worst)
		_run("D_settled_idle", threads, 200, _setup_idle)
		_run("E_fire_wood_oil", threads, 300, _setup_fire)
		_run("F_lava_into_water", threads, 200, _setup_lava)
	quit(0)


func _floor(w: SimWorld) -> void:
	w.fill_box(Vector3i(0, 0, 0), Vector3i(128, 4, 128), SimMaterials.STONE)


func _setup_sand(w: SimWorld) -> void:
	_floor(w)
	w.fill_box(Vector3i(32, 60, 32), Vector3i(96, 108, 96), SimMaterials.SAND)


func _setup_water(w: SimWorld) -> void:
	_floor(w)
	w.fill_box(Vector3i(40, 4, 40), Vector3i(88, 52, 88), SimMaterials.WATER)


func _setup_worst(w: SimWorld) -> void:
	_floor(w)
	w.scatter(Vector3i(0, 8, 0), Vector3i(128, 128, 128), SimMaterials.SAND, 0.30, 99)


func _setup_idle(w: SimWorld) -> void:
	w.fill_box(Vector3i(0, 0, 0), Vector3i(128, 64, 128), SimMaterials.STONE)


func _setup_fire(w: SimWorld) -> void:
	_floor(w)
	# a wooden block house next to an oil pool, lit at one corner
	w.fill_box(Vector3i(30, 4, 30), Vector3i(70, 44, 70), SimMaterials.WOOD)
	w.fill_box(Vector3i(34, 8, 34), Vector3i(66, 40, 66), SimMaterials.AIR)  # hollow
	w.fill_box(Vector3i(80, 4, 30), Vector3i(120, 10, 70), SimMaterials.OIL)
	for i in 4:
		w.ignite(Vector3i(70 + 0, 4 + i, 30 + i))
		w.ignite(Vector3i(69, 4 + i, 31 + i))
	w.fill_box(Vector3i(78, 5, 48), Vector3i(80, 7, 50), SimMaterials.FIRE)


func _setup_lava(w: SimWorld) -> void:
	_floor(w)
	w.fill_box(Vector3i(20, 4, 20), Vector3i(108, 24, 108), SimMaterials.WATER)
	w.fill_box(Vector3i(48, 70, 48), Vector3i(80, 100, 80), SimMaterials.LAVA)


func _run(name: String, threads: int, ticks: int, setup: Callable) -> void:
	var w := SimWorld.new()
	w.configure(Vector3i(8, 8, 8), threads, 1234)
	w.set_materials(SimMaterials.defs())
	w.set_reactions(SimMaterials.reactions())
	setup.call(w)
	var before: PackedInt32Array = w.material_histogram()
	w.tick(1)
	var r: Dictionary = w.tick(ticks)
	var after: PackedInt32Array = w.material_histogram()
	r["scenario"] = name
	r["world"] = "128^3 (512 chunks)"
	r["sand_conserved"] = after[SimMaterials.SAND] == before[SimMaterials.SAND]
	r["water_conserved"] = after[SimMaterials.WATER] == before[SimMaterials.WATER]
	r["burning_end"] = w.count_burning()
	r["materials_end"] = {"wood": after[SimMaterials.WOOD], "ash": after[SimMaterials.ASH], "fire": after[SimMaterials.FIRE],
		"smoke": after[SimMaterials.SMOKE], "steam": after[SimMaterials.STEAM], "stone": after[SimMaterials.STONE], "lava": after[SimMaterials.LAVA], "oil": after[SimMaterials.OIL]}
	r["active_chunks_end"] = w.get_active_chunk_count()
	for k in ["total_ms", "ms_per_tick", "max_tick_ms", "active_chunks_per_tick", "active_cells_per_tick", "moved_cells_per_tick", "reactions_per_tick", "active_cells_per_ms"]:
		r[k] = snappedf(r[k], 0.01)
	print(JSON.stringify(r))
