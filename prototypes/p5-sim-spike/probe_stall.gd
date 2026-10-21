extends SceneTree

## Debug probe: run the mixed scenario headless for N ticks and ask the sim
## whether any sand is left unsupported (floating). Prints JSON.
##   godot_console.exe --headless --path . --script res://probe_stall.gd -- ticks=900

var ticks := 900


func _init() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2 and kv[0] == "ticks":
			ticks = int(kv[1])
	var w := SimWorld.new()
	w.configure(Vector3i(8, 8, 8), 6, 42)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(128, 4, 128), SimWorld.STONE)
	w.fill_box(Vector3i(16, 70, 16), Vector3i(56, 110, 56), SimWorld.SAND)
	w.fill_box(Vector3i(72, 40, 72), Vector3i(112, 80, 112), SimWorld.WATER)
	w.fill_box(Vector3i(50, 8, 50), Vector3i(78, 20, 78), SimWorld.GAS)
	for i in 6:
		w.fill_box(Vector3i(20 + i * 16, 4, 60), Vector3i(24 + i * 16, 40, 64), SimWorld.STONE)
	var samples := []
	var step := 100
	var done := 0
	while done < ticks:
		w.tick(step)
		done += step
		var hist: PackedInt32Array = w.layer_histogram(SimWorld.SAND)
		var top := -1
		for y in range(hist.size() - 1, -1, -1):
			if hist[y] > 0:
				top = y
				break
		samples.append({"tick": done, "unsupported_sand": w.count_unsupported(SimWorld.SAND),
			"sand_top_y": top, "active_chunks": w.get_active_chunk_count(),
			"sand_above_60": w.count_in_box(Vector3i(0, 60, 0), Vector3i(128, 128, 128), SimWorld.SAND)})
	# Which chunks still hold unsupported sand, and are they awake?
	var stuck := []
	for cz in 8:
		for cy in 8:
			for cx in 8:
				var info: Dictionary = w.chunk_info(Vector3i(cx, cy, cz))
				if info.get("unsupported_sand", 0) > 0:
					info["chunk"] = Vector3i(cx, cy, cz)
					stuck.append(info)
	print(JSON.stringify({"samples": samples, "stuck_chunks": stuck.slice(0, 12), "stuck_count": stuck.size(), "stats": w.get_stats()}))
	quit(0)
