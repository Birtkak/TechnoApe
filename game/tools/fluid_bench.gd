extends SceneTree

## Fluid benchmark (ADR 0119, headless): the moving-liquid kernel alone, no terrain, no meshing. Three scenes, each
## printed as one JSON line: "basin" (an 8^3 block of water dropped into a 30 x 30 basin: ticks until under 8 cells
## move for 10 ticks (films drying are not motion), the surface's spread in eighths, tick ms avg / max) and "bombs" (ten 1.5 m water balls over an uneven
## stone floor with pits, one every 15 ticks: ticks to quiet after the last, the most chunks awake in one tick, tick ms).
##   godot --headless --path game --script res://tools/fluid_bench.gd [-- column=0]   (column=0: the eighth kernel of ADR 0069)
## Run old and new back to back and compare medians: the machine is shared.

const HZ := 30.0


func _world(column: bool, chunks: int) -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(chunks, 3, chunks), maxi(2, OS.get_processor_count() - 2), 7)
	w.set_materials(GameMaterials.defs())
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	w.set_liquid_fullness(true)
	w.set_column_flow(column)
	w.set_cascade(true)
	w.set_gravity(3.0 * 9.8 / 0.125 / (HZ * HZ))
	return w


## Ticks until nothing moved for 10 ticks (-1 when never), with the tick cost and the most chunks awake.
func _settle(w: SimWorld, max_ticks: int, out: Dictionary) -> int:
	var still := 0
	var sum := 0.0
	var mx := 0.0
	var n := 0
	var awake := 0
	for t in max_ticks:
		var t0 := Time.get_ticks_usec()
		var s: Dictionary = w.tick(1)
		var ms := (Time.get_ticks_usec() - t0) / 1000.0
		sum += ms
		mx = maxf(mx, ms)
		n += 1
		awake = maxi(awake, int(s.get("active_chunks_per_tick", 0)))
		if int(s.get("moved_cells_per_tick", 1)) < 8:   # the odd film drying is not motion
			still += 1
			if still >= 10:
				out["tick_ms_avg"] = snappedf(sum / n, 0.01)
				out["tick_ms_max"] = snappedf(mx, 0.01)
				out["chunks_awake_max"] = awake
				return t - 9
		else:
			still = 0
	out["tick_ms_avg"] = snappedf(sum / n, 0.01)
	out["tick_ms_max"] = snappedf(mx, 0.01)
	out["chunks_awake_max"] = awake
	return -1


func _surface_spread(w: SimWorld, x0: int, x1: int, z0: int, z1: int, y0: int, y1: int) -> int:
	var lo := 1 << 30
	var hi := -1
	for h in load("res://test/test_sim_0119.gd").surface_heights(w, x0, x1, z0, z1, y0, y1, GameMaterials.WATER).values():
		lo = mini(lo, h)
		hi = maxi(hi, h)
	return hi - lo


func _init() -> void:
	var column := true
	for a in OS.get_cmdline_user_args():
		if a == "column=0":
			column = false
	# basin
	var w := _world(column, 3)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 8, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(8, 8, 8), Vector3i(40, 20, 40), GameMaterials.STONE)
	w.fill_box(Vector3i(9, 8, 9), Vector3i(39, 20, 39), 0)
	w.fill_box(Vector3i(20, 8, 20), Vector3i(28, 16, 28), GameMaterials.WATER)
	var mass := w.count_liquid_eighths(GameMaterials.WATER)
	var out := {"scene": "basin", "column": column}
	out["quiet_tick"] = _settle(w, 600, out)
	out["quiet_s"] = snappedf(out["quiet_tick"] / HZ, 0.01)
	out["surface_spread_eighths"] = _surface_spread(w, 9, 39, 9, 39, 8, 20)
	out["mass_kept"] = w.count_liquid_eighths(GameMaterials.WATER) == mass
	print(JSON.stringify(out))
	# bombs: an uneven floor (steps and pits) over 6 x 6 chunks, ten balls
	w = _world(column, 6)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(96, 10, 96), GameMaterials.STONE)
	for k in 12:
		var sx := 8 + (k * 37) % 70
		var sz := 8 + (k * 53) % 70
		w.fill_box(Vector3i(sx, 10, sz), Vector3i(sx + 12, 10 + 2 + (k % 3) * 2, sz + 12), GameMaterials.STONE)   # a step
		w.fill_box(Vector3i(sx + 14, 4, sz + 2), Vector3i(sx + 20, 10, sz + 8), 0)   # a pit beside it
	out = {"scene": "bombs", "column": column}
	var sum := 0.0
	var mx := 0.0
	var awake := 0
	var n := 0
	for b in 10:
		var cx := 16 + (b * 29) % 64
		var cz := 16 + (b * 41) % 64
		w.fill_sphere(Vector3(cx, 28, cz), 6.0, GameMaterials.WATER, PackedInt32Array())
		for t in 15:
			var t0 := Time.get_ticks_usec()
			var s: Dictionary = w.tick(1)
			var ms := (Time.get_ticks_usec() - t0) / 1000.0
			sum += ms
			mx = maxf(mx, ms)
			n += 1
			awake = maxi(awake, int(s.get("active_chunks_per_tick", 0)))
	out["tick_ms_avg_while_bombing"] = snappedf(sum / n, 0.01)
	out["tick_ms_max_while_bombing"] = snappedf(mx, 0.01)
	out["chunks_awake_max_while_bombing"] = awake
	var after := {}
	out["quiet_ticks_after_last"] = _settle(w, 900, after)
	out["tick_ms_avg_after"] = after["tick_ms_avg"]
	out["chunks_awake_max_after"] = after["chunks_awake_max"]
	print(JSON.stringify(out))
	# steam box (ADR 0119 C): a sealed 16^3 room at 110 C with a quarter of water: boiling raises the pressure until it
	# stops; tick cost with and without the state pass
	for states in [false, true]:
		w = _world(column, 3)
		w.set_states(states)
		w.set_ambient_c(110.0, true)
		w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 48, 48), GameMaterials.STONE)
		w.fill_box(Vector3i(16, 16, 16), Vector3i(32, 32, 32), 0)
		w.fill_box(Vector3i(16, 16, 16), Vector3i(32, 20, 32), GameMaterials.WATER)
		out = {"scene": "steam_box", "states": states}
		sum = 0.0
		mx = 0.0
		for t in 300:
			var t0 := Time.get_ticks_usec()
			w.tick(1)
			var ms := (Time.get_ticks_usec() - t0) / 1000.0
			sum += ms
			mx = maxf(mx, ms)
		out["tick_ms_avg"] = snappedf(sum / 300.0, 0.01)
		out["tick_ms_max"] = snappedf(mx, 0.01)
		out["steam_cells"] = w.count_material(GameMaterials.STEAM)
		out["water_eighths_left"] = w.count_liquid_eighths(GameMaterials.WATER)
		out["pressure"] = snappedf(w.chunk_pressure(Vector3i(24, 24, 24)), 0.01)
		out["temp_c"] = int(w.cell_info(Vector3i(24, 24, 24)).get("temp_c", 0))
		print(JSON.stringify(out))
	quit()
