extends SceneTree

## Water pacing + cost benchmark (headless; bug report 2026-10-27 17-59-05 "water still needs to update faster").
## A still pond (generated water, 4 m x 1 m deep x 4 m) and a 2 m pit dug beside it (ADR 0068: the pond wakes at
## the opening and drains into the pit). Prints the share of the pit filled after 1, 2 and 4 s, the seconds until
## nothing moved for 20 ticks, and the tick cost while it moved (ms avg / max).
##   godot --headless --path game --script res://tools/water_bench.gd [-- speed=N]   (N overrides water's liquid_speed)

const HZ := 30.0   # Settings.sim_tick_hz (autoloads do not run under --script)


func _init() -> void:
	var speed := -1
	for a in OS.get_cmdline_user_args():
		if a.begins_with("speed="):
			speed = int(a.substr(6))
	var defs := GameMaterials.defs()
	for d in defs:
		if d["name"] == "water" and speed > 0:
			d["liquid_speed"] = speed
	var w := SimWorld.new()
	w.configure(Vector3i(6, 2, 6), maxi(2, OS.get_processor_count() - 2), 7)
	w.set_materials(defs)
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	w.set_liquid_fullness(true)
	w.set_cascade(true)
	w.set_gravity(9.8 / 0.125 / (HZ * HZ))   # as SimBubble sets it (12.5 cm voxels, the game's 30 Hz)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(96, 24, 96), GameMaterials.STONE)
	w.fill_box(Vector3i(8, 16, 32), Vector3i(40, 24, 64), GameMaterials.WATER_STATIC)
	for t in 3:
		w.tick(1)
	var pit_min := Vector3i(40, 8, 40)
	var pit_max := Vector3i(56, 24, 56)
	w.fill_box(pit_min, pit_max, 0)   # the pit, dug against the pond's side
	var pit_cells := (pit_max.x - pit_min.x) * (pit_max.y - pit_min.y) * (pit_max.z - pit_min.z)
	var out := {"liquid_speed": speed, "settled_s": -1.0}
	var quiet := 0
	var ms_sum := 0.0
	var ms_max := 0.0
	var moving_ticks := 0
	for t in int(HZ * 60):
		var r: Dictionary = w.tick(1)
		if int(r["moved_cells_per_tick"]) > 0:
			ms_sum += float(r["ms_per_tick"])
			ms_max = maxf(ms_max, float(r["ms_per_tick"]))
			moving_ticks += 1
			quiet = 0
		else:
			quiet += 1
		for s in [1, 2, 4, 8, 16]:
			if t + 1 == int(HZ * s):
				out["pit_full_%ds" % s] = snappedf(float(w.count_in_box(pit_min, pit_max, GameMaterials.WATER)) / pit_cells, 0.01)
		if quiet >= 20:
			out["settled_s"] = snappedf((t - 19) / HZ, 0.1)
			break
	out["pond_still_left"] = w.count_material(GameMaterials.WATER_STATIC)
	out["pit_full_end"] = snappedf(float(w.count_in_box(pit_min, pit_max, GameMaterials.WATER)) / pit_cells, 0.01)
	out["tick_ms_avg"] = snappedf(ms_sum / maxf(moving_ticks, 1), 0.001)
	out["tick_ms_max"] = snappedf(ms_max, 0.01)
	print(JSON.stringify(out))
	quit()
