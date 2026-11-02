class_name Networks
extends RefCounted

## Power and fluid networks (ADR 0113): once a second BlockEntities gathers every entity that answers
## `net(channel)` (a box on the block grid with a role and an amount), the native NetworkSolver joins the
## boxes that touch and shares each component's flow, and every entity hears `net_in(channel, amount)`.

static var solver: NetworkSolver
static var stats := {"nodes": 0, "ms_max": 0.0, "components": 0, "flow": 0.0}


static func solve(store: BlockEntities, channel: String) -> void:
	if solver == null:
		solver = NetworkSolver.new()
	var t0 := Time.get_ticks_usec()
	var nodes := []
	var boxes := PackedInt32Array()
	var roles := PackedInt32Array()
	var amounts := PackedFloat32Array()
	for e in store.all_entities():
		if not e.has_method("net"):
			continue
		var n: Dictionary = e.net(channel)
		if n.is_empty():
			continue
		var w := BlockEntities.width_of(e.kind)
		boxes.append_array(PackedInt32Array([e.pos.x, e.pos.y, e.pos.z, w, BlockEntities.height_of(e.kind), w]))
		roles.append(int(n["role"]))
		amounts.append(float(n["amount"]))
		nodes.append(e)
	if nodes.is_empty():
		stats["nodes"] = 0
		return
	var r: PackedFloat32Array = solver.solve(boxes, roles, amounts)
	for i in nodes.size():
		nodes[i].net_in(channel, r[i * 2 + 1])
	var s := solver.stats()
	stats["nodes"] = nodes.size()
	stats["components"] = int(s.get("count", 0))
	stats["flow"] = float(s.get("flow", 0.0))
	stats["ms_max"] = maxf(float(stats["ms_max"]), (Time.get_ticks_usec() - t0) / 1000.0)
