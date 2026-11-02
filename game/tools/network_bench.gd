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
	print(JSON.stringify({"network_bench": out}))
	quit(0)
