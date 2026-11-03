class_name Networks
extends RefCounted

## Power and fluid networks (ADR 0113, 0122): once a second BlockEntities gathers every entity that answers
## `net(channel)` (a box on the block grid with a role and an amount), the native NetworkSolver joins the
## boxes that touch and shares each component's flow, and every entity hears `net_in(channel, amount)`.
## Channels: "energy" (a DC circuit in real units, ADR 0123: sources are an EMF behind an internal resistance,
## cables a resistance, loads watts at lv_volts; every node hears its volts, amps and watts), "steam" (bar a
## second: boilers give, every machine with a `boiler` slot wants steam_per_use and gets its component's boiler
## to draw from), "water" (buckets a second: pumps give, tanks store, boilers drink). Pipes conduct steam and
## water, cables energy.

static var solver: NetworkSolver
static var stats := {"nodes": 0, "ms_max": 0.0, "components": 0, "flow": 0.0}
## The last solve per channel: entity instance id -> its network's {supply, demand, flow, cap, nodes}
## (sums over the component), for the look-at panel (ADR 0130).
static var networks := {}


static func solve(store: BlockEntities, channel: String) -> void:
	if solver == null:
		solver = NetworkSolver.new()
	if channel == "energy":
		solve_dc(store)
		return
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


## The energy channel (ADR 0123): every entity answering net("energy") with {kind: "conductor", ohms, amps} /
## {kind: "source", volts, ohms} / {kind: "load", watts, volts} is a node of a DC circuit; NetworkSolver.solve_dc
## gives each its volts, amps and watts, and net_in("energy", {volts, amps, watts, may_start}) hands them over. One
## load may start per circuit per solve (the first that asks), so a weak dynamo settles instead of flapping.
static func solve_dc(store: BlockEntities) -> void:
	var t0 := Time.get_ticks_usec()
	var nodes := []
	var boxes := PackedInt32Array()
	var kinds := PackedInt32Array()
	var a := PackedFloat32Array()
	var b := PackedFloat32Array()
	for e in store.all_entities():
		if not e.has_method("net"):
			continue
		var n: Dictionary = e.net("energy")
		if n.is_empty():
			continue
		var w := BlockEntities.width_of(e.kind)
		boxes.append_array(PackedInt32Array([e.pos.x, e.pos.y, e.pos.z, w, BlockEntities.height_of(e.kind), w]))
		match String(n["kind"]):
			"source":
				kinds.append(NetworkSolver.DC_SOURCE)
				a.append(float(n["volts"]))
				b.append(float(n["ohms"]))
			"load":
				kinds.append(NetworkSolver.DC_LOAD)
				var v := maxf(float(n.get("volts", Settings.lv_volts)), 1.0)
				a.append(float(n["watts"]) / (v * v))   # conductance: watts at rated volts
				b.append(0.0)
			_:
				kinds.append(NetworkSolver.DC_CONDUCTOR)
				a.append(float(n.get("ohms", 0.001)))
				b.append(0.0)
		nodes.append(e)
	if nodes.is_empty():
		stats["nodes"] = 0
		networks["energy"] = {}
		return
	var r: PackedFloat32Array = solver.solve_dc(boxes, kinds, a, b)
	var comps := {}
	for i in nodes.size():
		var c: Dictionary = comps.get(int(r[i * 4]), {})
		if c.is_empty():
			c = {"supply": 0.0, "demand": 0.0, "volts_min": INF, "volts_max": 0.0, "amps_max": 0.0, "nodes": 0, "started": false}
			comps[int(r[i * 4])] = c
		c["nodes"] += 1
		c["volts_min"] = minf(c["volts_min"], r[i * 4 + 1])
		c["volts_max"] = maxf(c["volts_max"], r[i * 4 + 1])
		c["amps_max"] = maxf(c["amps_max"], absf(r[i * 4 + 2]))
		if kinds[i] == NetworkSolver.DC_SOURCE and r[i * 4 + 3] > 0.0:
			c["supply"] += r[i * 4 + 3]
		elif kinds[i] == NetworkSolver.DC_LOAD:
			c["demand"] += r[i * 4 + 3]
	var by_node := {}
	for i in nodes.size():
		var c: Dictionary = comps[int(r[i * 4])]
		var d := {"volts": r[i * 4 + 1], "amps": r[i * 4 + 2], "watts": r[i * 4 + 3], "may_start": not c["started"]}
		if nodes[i].has_method("net_in") and nodes[i].net_in("energy", d):
			c["started"] = true
		by_node[nodes[i].get_instance_id()] = c
	networks["energy"] = by_node
	var s := solver.dc_stats()
	stats["nodes"] = nodes.size()
	stats["components"] = int(s.get("count", 0))
	stats["flow"] = float(s.get("delivered_watts", 0.0))
	stats["dc_iterations"] = int(s.get("iterations", 0))
	stats["ms_max"] = maxf(float(stats["ms_max"]), (Time.get_ticks_usec() - t0) / 1000.0)


const CHANNELS := {"steam": ["Steam", "bar/s", "pipes"], "water": ["Water", "buckets/s", "pipes"]}   # title, unit, carrier (ADR 0122)


## The networks an entity is on, one line per channel: "Power: 47.6 V here (46.9-52.0 V), 12.3 A at most,
## 590 W drawn of 640 W made (5 parts)"; "Steam: 1.00 bar/s made, 0.02 wanted, 0.02 moving (32 parts)", a second
## line when the pipes cap it; "" when it is on none (the look-at panel, ADR 0130, and the multimeter).
static func look_line(e: Object) -> String:
	var lines: PackedStringArray = []
	var p: Dictionary = networks.get("energy", {}).get(e.get_instance_id(), {}) if e != null else {}
	if not p.is_empty() and p["nodes"] >= 2:
		var here := "here" if not e.has_method("net") else "%.1f V here" % float(e.get("volts") if "volts" in e else (e.load.volts if "load" in e and e.load != null else p["volts_max"]))
		lines.append("Power: %s (%.1f-%.1f V), %.1f A at most, %.0f W drawn of %.0f W made (%d parts)" % [here, p["volts_min"], p["volts_max"], p["amps_max"], p["demand"], p["supply"], p["nodes"]])
	for ch in CHANNELS:
		var c: Dictionary = networks.get(ch, {}).get(e.get_instance_id(), {}) if e != null else {}
		if c.is_empty() or c["nodes"] < 2:
			continue
		var unit: String = CHANNELS[ch][1]
		lines.append("%s: %.2f %s made, %.2f wanted, %.2f moving (%d parts)" % [CHANNELS[ch][0], c["supply"], unit, c["demand"], c["flow"], c["nodes"]])
		if c["cap"] < minf(c["supply"], c["demand"]):
			lines.append("The %s carry only %.2f %s" % [CHANNELS[ch][2], c["cap"], unit])
	return "\n".join(lines)
