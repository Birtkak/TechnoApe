extends SceneTree

## The network solver's cost (ADR 0113): 10 000 cables and 500 machines as one long line (one component)
## and as 500 islands of 20 cables, the median of 7 solves each.
##   Godot_..._console.exe --headless --path game --script res://tools/network_bench.gd


func _median(v: Array) -> float:
	v.sort()
	return float(v[v.size() / 2])


func _run(s: NetworkSolver, boxes: PackedInt32Array, roles: PackedInt32Array, amounts: PackedFloat32Array) -> Dictionary:
	var times := []
	for i in 7:
		var t0 := Time.get_ticks_usec()
		s.solve(boxes, roles, amounts)
		times.append(Time.get_ticks_usec() - t0)
	var st := s.stats()
	return {"nodes": roles.size(), "components": st["count"], "flow": st["flow"], "solve_us": _median(times)}


func _init() -> void:
	var B := 4   # Settings.BLOCK: a --script run has no autoloads
	var s := NetworkSolver.new()
	var out := {}
	# one line: source, 10 000 cables with a machine every 20
	var boxes := PackedInt32Array()
	var roles := PackedInt32Array()
	var amounts := PackedFloat32Array()
	for i in 10500:
		boxes.append_array(PackedInt32Array([i * B, 0, 0, B, B, B]))
		if i == 0:
			roles.append(NetworkSolver.SOURCE)
			amounts.append(100000.0)
		elif i % 21 == 20:
			roles.append(NetworkSolver.SINK)
			amounts.append(16.0)
		else:
			roles.append(NetworkSolver.CONDUCTOR)
			amounts.append(32.0)
	out["line_10k"] = _run(s, boxes, roles, amounts)
	# 500 islands of a source, 20 cables and a sink, 2 blocks apart
	boxes = PackedInt32Array()
	roles = PackedInt32Array()
	amounts = PackedFloat32Array()
	for k in 500:
		for i in 22:
			boxes.append_array(PackedInt32Array([i * B, 0, k * 3 * B, B, B, B]))
			roles.append(NetworkSolver.SOURCE if i == 0 else (NetworkSolver.SINK if i == 21 else NetworkSolver.CONDUCTOR))
			amounts.append(32.0 if i == 0 else (16.0 if i == 21 else 32.0))
	out["islands_500"] = _run(s, boxes, roles, amounts)
	# a steam main: one boiler, 10 000 bronze pipes, a hammer every 20 (the fluid channels, ADR 0122)
	boxes = PackedInt32Array()
	roles = PackedInt32Array()
	amounts = PackedFloat32Array()
	for i in 10500:
		boxes.append_array(PackedInt32Array([i * B, 0, 0, B, B, B]))
		roles.append(NetworkSolver.SOURCE if i == 0 else (NetworkSolver.SINK if i % 21 == 20 else NetworkSolver.CONDUCTOR))
		amounts.append(1.0 if i == 0 else (0.02 if i % 21 == 20 else 1.0))
	out["steam_10k"] = _run(s, boxes, roles, amounts)
	# the DC circuit (ADR 0123): a dynamo, 10 000 cables and a 200 W machine every 20; then 500 islands
	for name in ["dc_10k", "dc_islands_500"]:
		boxes = PackedInt32Array()
		var kinds := PackedInt32Array()
		var av := PackedFloat32Array()
		var bv := PackedFloat32Array()
		var islands := 500 if name == "dc_islands_500" else 1
		var per := 22 if islands > 1 else 10500
		for k in islands:
			for i in per:
				boxes.append_array(PackedInt32Array([i * B, 0, k * 3 * B, B, B, B]))
				var load := (i == per - 1) if islands > 1 else (i % 21 == 20)
				kinds.append(NetworkSolver.DC_SOURCE if i == 0 else (NetworkSolver.DC_LOAD if load else NetworkSolver.DC_CONDUCTOR))
				av.append(52.0 if i == 0 else (200.0 / (48.0 * 48.0) if load else 0.0014))
				bv.append(0.3 if i == 0 else 0.0)
		var times := []
		for rep in 7:
			var t0 := Time.get_ticks_usec()
			s.solve_dc(boxes, kinds, av, bv)
			times.append(Time.get_ticks_usec() - t0)
		var st := s.dc_stats()
		out[name] = {"nodes": kinds.size(), "reduced": st["reduced_nodes"], "iterations": st["iterations"], "delivered_w": st["delivered_watts"], "solve_us": _median(times)}
	print(JSON.stringify({"network_bench": out}))
	quit(0)
