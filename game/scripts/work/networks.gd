class_name Networks
extends RefCounted

## Power and fluid networks (ADR 0113, 0122): once a second BlockEntities gathers every entity that answers
## `net(channel)` (a box on the block grid with a role and an amount), the native NetworkSolver joins the
## boxes that touch and shares each component's flow, and every entity hears `net_in(channel, amount)`.
## Channels: "energy" (EU/t), "steam" (bar a second: boilers give, every machine with a `boiler` slot wants
## steam_per_use and gets its component's boiler to draw from), "water" (buckets a second: pumps give, tanks
## store, boilers drink). Pipes conduct steam and water, cables energy.

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
		var n: Dictionary = {}
		if e.has_method("net"):
			n = e.net(channel)
		if n.is_empty() and channel == "steam" and "boiler" in e:   # the steam age's machines (the turbine answers net for energy only): a sink of one use a second
			n = {"role": NetworkSolver.SINK, "amount": Settings.steam_per_use}
			e.boiler = null
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
	var boiler_of := {}   # steam: component -> its first boiler
	if channel == "steam":
		for i in nodes.size():
			if nodes[i] is BoilerEntity and not boiler_of.has(int(r[i * 2])):
				boiler_of[int(r[i * 2])] = nodes[i]
	for i in nodes.size():
		if channel == "steam" and "boiler" in nodes[i] and roles[i] == NetworkSolver.SINK:
			nodes[i].boiler = boiler_of.get(int(r[i * 2])) if r[i * 2 + 1] > 0.0 else null   # it draws from that boiler itself
		if nodes[i].has_method("net_in"):
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


const CHANNELS := {"energy": ["Power", "EU/t", "cables"], "steam": ["Steam", "bar/s", "pipes"], "water": ["Water", "buckets/s", "pipes"]}   # title, unit, carrier (ADR 0122)


## The networks an entity is on, one line per channel: "Power: 32 EU/t made, 24 wanted, 24 moving (5 parts)",
## a second line when the carriers cap it; "" when it is on none (the look-at panel, ADR 0130, and the EU meter).
static func look_line(e: Object) -> String:
	var lines: PackedStringArray = []
	for ch in CHANNELS:
		var c: Dictionary = networks.get(ch, {}).get(e.get_instance_id(), {}) if e != null else {}
		if c.is_empty() or c["nodes"] < 2:
			continue
		var unit: String = CHANNELS[ch][1]
		var f := "%d" if ch == "energy" else "%.2f"
		lines.append(("%s: " + f + " %s made, " + f + " wanted, " + f + " moving (%d parts)") % [CHANNELS[ch][0], c["supply"], unit, c["demand"], c["flow"], c["nodes"]])
		if c["cap"] < minf(c["supply"], c["demand"]):
			lines.append(("The %s carry only " + f + " %s") % [CHANNELS[ch][2], c["cap"], unit])
	return "\n".join(lines)
