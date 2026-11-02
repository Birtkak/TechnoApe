class_name Networks
extends RefCounted

## Power and fluid networks (ADR 0113): once a second BlockEntities gathers every entity that answers
## `net(channel)` (a box on the block grid with a role and an amount), the native NetworkSolver joins the
## boxes that touch and shares each component's flow, and every entity hears `net_in(channel, amount)`.

static var solver: NetworkSolver
static var stats := {"nodes": 0, "ms_max": 0.0, "components": 0, "flow": 0.0}
## The last solve per channel: entity instance id -> its network's {supply, demand, flow, cap, nodes}
## (sums over the component), for the look-at panel (ADR 0130).
static var networks := {}


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
		networks[channel] = {}
		return
	var r: PackedFloat32Array = solver.solve(boxes, roles, amounts)
	var comps := {}
	for i in nodes.size():
		nodes[i].net_in(channel, r[i * 2 + 1])
		var c: Dictionary = comps.get(int(r[i * 2]), {})
		if c.is_empty():
			c = {"supply": 0.0, "demand": 0.0, "flow": 0.0, "cap": INF, "nodes": 0}
			comps[int(r[i * 2])] = c
		c["nodes"] += 1
		match roles[i]:
			NetworkSolver.SOURCE:
				c["supply"] += amounts[i]
				c["flow"] += r[i * 2 + 1]
			NetworkSolver.SINK, NetworkSolver.SINK_LOW:
				c["demand"] += amounts[i]
			_:
				c["cap"] = minf(c["cap"], amounts[i])
	var by_node := {}
	for i in nodes.size():
		by_node[nodes[i].get_instance_id()] = comps[int(r[i * 2])]
	networks[channel] = by_node
	var s := solver.stats()
	stats["nodes"] = nodes.size()
	stats["components"] = int(s.get("count", 0))
	stats["flow"] = float(s.get("flow", 0.0))
	stats["ms_max"] = maxf(float(stats["ms_max"]), (Time.get_ticks_usec() - t0) / 1000.0)


## "Network: 32 EU/t made, 24 wanted, 24 moving (5 parts)" for an entity on the energy channel, else "".
static func look_line(e: Object) -> String:
	var c: Dictionary = networks.get("energy", {}).get(e.get_instance_id(), {}) if e != null else {}
	if c.is_empty() or c["nodes"] < 2:
		return ""
	var line := "Network: %d EU/t made, %d wanted, %d moving (%d parts)" % [roundi(c["supply"]), roundi(c["demand"]), roundi(c["flow"]), c["nodes"]]
	if c["cap"] < minf(c["supply"], c["demand"]):
		line += "\nThe cables carry only %d EU/t" % roundi(c["cap"])
	return line
