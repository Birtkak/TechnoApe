class_name PowerSourceEntity
extends RefCounted

## The small 48 V sources (ADR 0123): a water wheel, a wind turbine and a hand crank. One class; the item's
## `"dynamo": {"volts", "ohms"}` is the EMF at full drive behind an internal resistance, and `drive` (0..1) is
## how hard the world turns it: WorkSystem sets it once a second (the wheel from the water under it, the
## wind from the weather and the hour), a right click cranks the crank for crank_s. On the circuit it is an
## EMF of volts * drive, so a slow wheel or a lull cannot hold a big machine: physics, not a flag.

signal changed

var kind: String
var pos: Vector3i
var drive := 0.0
var cranked_until := 0.0   # Inventory.clock_s the crank keeps turning until
var amps := 0.0
var volts := 0.0
var spin := 0
var _spec: Dictionary


func _init(p_items: ItemRegistry, p_pos: Vector3i, p_kind: String) -> void:
	pos = p_pos
	kind = p_kind
	_spec = p_items.get_def(p_kind).get("dynamo", {"volts": 50.0, "ohms": 1.0})


func emf() -> float:
	return float(_spec.get("volts", 50.0)) * clampf(drive, 0.0, 1.0)


func net(channel: String) -> Dictionary:
	if channel != "energy":
		return {}
	if kind == "hand_crank":
		drive = 1.0 if Inventory.clock_s < cranked_until else 0.0
	if drive <= 0.0:
		return {"kind": "conductor", "ohms": Settings.circuit_device_ohms}   # still, it only passes the line through
	return {"kind": "source", "volts": emf(), "ohms": float(_spec.get("ohms", 1.0))}


func net_in(channel: String, d) -> bool:
	if channel == "energy":
		amps = float(d.get("amps", 0.0))
		volts = float(d.get("volts", 0.0))
	return false


func tick() -> bool:
	if drive > 0.0:
		spin += 1
		if spin % 5 == 0:
			changed.emit()   # the wheel or the blades turn a step
	return false


func is_lit() -> bool:
	return false


## Right click: the crank turns for crank_s; the others tell what drives them.
func use(_stack: Dictionary) -> Dictionary:
	var res := {"consume": 0, "give": [], "msg": ""}
	match kind:
		"hand_crank":
			cranked_until = Inventory.clock_s + Settings.crank_s
			drive = 1.0
			res["msg"] = "You crank it: %.0f V for %d s, about %.0f W if something draws." % [emf(), Settings.crank_s, emf() * emf() / (4.0 * float(_spec.get("ohms", 1.0)))]
		"water_wheel":
			res["msg"] = "A water wheel: %s." % ("turning at %d%%, %.1f V, %.1f A" % [roundi(drive * 100.0), volts, amps] if drive > 0.0 else "set it over running water")
		_:
			res["msg"] = "A wind turbine: %s." % ("the wind at %d%%, %.1f V, %.1f A" % [roundi(drive * 100.0), volts, amps] if drive > 0.0 else "no wind")
	changed.emit()
	return res


func contents() -> Array:
	return [{"id": kind, "count": 1}]


func look_lines() -> Array[String]:
	if drive <= 0.0:
		return [{"hand_crank": "Still: right click to crank it", "water_wheel": "Still: no water under it", "wind_turbine": "Still: no wind"}.get(kind, "Still")]
	return ["%d%% drive: %.1f V, %.1f A, %.0f W" % [roundi(drive * 100.0), volts, amps, maxf(volts * amps, 0.0)]]


## ADR 0129: a paddle wheel on timber trestles over the water, a wind turbine with a green nacelle and
## hazard-tipped blades on a 3 m steel post, a riveted green dynamo with a crank and a wooden grip. Each turns
## by `spin`: the steps divide the shape's symmetry (a paddle's 45, a blade's 120, the crank's 360 degrees), so
## the turn runs on without a jump back when the step count wraps.
func view_parts() -> Dictionary:
	var step := spin / 5   # tick() redraws every fifth spin
	match kind:
		"water_wheel":
			return {"solid": {"cells": _wheel_cells(step), "size": Vector3i(16, 16, 16), "grid": 16}}
		"wind_turbine":   # three widths across, centred on the post's footprint
			return {"solid": {"cells": _turbine_cells(step), "size": TURBINE_SIZE, "offset": Vector3(-1, 0, 0)}}
		_:
			return {"solid": {"cells": _crank_cells(step), "size": Vector3i(16, 16, 16), "grid": 16}}


const TURBINE_SIZE := Vector3i(24, 56, 8)   # cells of 1/8 block: the rotor is 1.4 m across, its hub 2.75 m up


static func _wheel_cells(step: int) -> PackedInt32Array:
	var size := Vector3i(16, 16, 16)
	var c := WorkPalette.new_cells(size)
	for x in [1, 13]:   # the trestles: a stone sill, two legs, a cross beam, the iron bearing
		WorkPalette.box(c, size, Vector3i(x, 0, 3), Vector3i(x + 2, 1, 13), WorkPalette.STONE_DARK)
		for z in [4, 10]:
			WorkPalette.box(c, size, Vector3i(x, 1, z), Vector3i(x + 2, 9, z + 2), WorkPalette.WOOD_DARK)
		WorkPalette.box(c, size, Vector3i(x, 8, 4), Vector3i(x + 2, 10, 12), WorkPalette.WOOD_DARK)
		WorkPalette.box(c, size, Vector3i(x, 7, 7), Vector3i(x + 2, 10, 9), WorkPalette.IRON_DARK)
	var turn := float(step % 4) * PI / 16.0   # a quarter of the 45 degrees between paddles each step
	for z in size.z:
		for y in size.y:
			var d := Vector2(y + 0.5 - 8.0, z + 0.5 - 8.0)
			var r := d.length()
			if r > 7.6:
				continue
			var on_paddle := fposmod(atan2(d.x, d.y) + turn, PI / 4.0) < 0.3
			for x in range(3, 13):
				var rim := x == 3 or x == 12
				var paint := -1
				if r < 2.0:
					paint = WorkPalette.IRON_DARK   # the hub
				elif rim and r > 6.4:
					paint = WorkPalette.WOOD_DARK   # the rims
				elif on_paddle:
					paint = WorkPalette.WOOD_DARK if rim else WorkPalette.WOOD   # spokes in the rims, boards between
				if paint >= 0:
					c[WorkPalette.idx(size, x, y, z)] = paint + 1
	WorkPalette.box(c, size, Vector3i(0, 7, 7), Vector3i(16, 9, 9), WorkPalette.IRON_DARK)   # the axle
	WorkPalette.box(c, size, Vector3i(14, 10, 5), Vector3i(16, 14, 11), WorkPalette.PAINT)   # the dynamo on its end
	WorkPalette.box(c, size, Vector3i(14, 10, 5), Vector3i(16, 11, 11), WorkPalette.IRON_DARK)
	WorkPalette.box(c, size, Vector3i(14, 14, 7), Vector3i(16, 15, 9), WorkPalette.COPPER)
	c[WorkPalette.idx(size, 15, 12, 5)] = WorkPalette.RIVET + 1
	c[WorkPalette.idx(size, 15, 12, 10)] = WorkPalette.RIVET + 1
	return c


static func _turbine_cells(step: int) -> PackedInt32Array:
	var size := TURBINE_SIZE
	var c := WorkPalette.new_cells(size)
	WorkPalette.box(c, size, Vector3i(9, 0, 1), Vector3i(15, 2, 7), WorkPalette.IRON_DARK)   # the footing
	WorkPalette.hazard_box(c, size, Vector3i(9, 1, 1), Vector3i(15, 2, 7))
	WorkPalette.box(c, size, Vector3i(10, 2, 2), Vector3i(14, 10, 6), WorkPalette.STEEL)   # the post, wide at its foot
	WorkPalette.box(c, size, Vector3i(11, 10, 3), Vector3i(13, 43, 5), WorkPalette.STEEL)
	for y in range(9, 43, 8):
		WorkPalette.box(c, size, Vector3i(10, y, 2), Vector3i(14, y + 1, 6), WorkPalette.IRON_DARK)   # its flanges
	WorkPalette.box(c, size, Vector3i(10, 42, 1), Vector3i(14, 47, 7), WorkPalette.PAINT)   # the nacelle
	WorkPalette.box(c, size, Vector3i(10, 46, 1), Vector3i(14, 47, 7), WorkPalette.IRON_DARK)
	WorkPalette.box(c, size, Vector3i(11, 43, 0), Vector3i(13, 50, 1), WorkPalette.STEEL)   # the tail vane
	var turn := float(step % 6) * PI / 9.0   # a sixth of the 120 degrees between blades each step
	for y in range(30, size.y):
		for x in size.x:
			var d := Vector2(x + 0.5 - 12.0, y + 0.5 - 44.5)
			var r := d.length()
			if r > 11.6:
				continue
			var off := fposmod(atan2(d.y, d.x) + turn, 2.0 * PI / 3.0)
			off = minf(off, 2.0 * PI / 3.0 - off)
			var paint := -1
			if r < 1.6:
				paint = WorkPalette.IRON_DARK   # the hub
			elif off * r < 2.0 - r * 0.08:   # a blade, tapering to its tip
				paint = WorkPalette.HAZARD if r > 9.5 else WorkPalette.TIN
			if paint >= 0:
				c[WorkPalette.idx(size, x, y, size.z - 1)] = paint + 1
	return c


static func _crank_cells(step: int) -> PackedInt32Array:
	var size := Vector3i(16, 16, 16)
	var c := WorkPalette.new_cells(size)
	WorkPalette.box(c, size, Vector3i(2, 0, 2), Vector3i(14, 2, 14), WorkPalette.IRON_DARK)   # the base
	for p in [Vector2i(3, 3), Vector2i(12, 3), Vector2i(3, 12), Vector2i(12, 12)]:
		c[WorkPalette.idx(size, p.x, 1, p.y)] = WorkPalette.RIVET + 1
	for x in [4, 10]:
		WorkPalette.box(c, size, Vector3i(x, 2, 5), Vector3i(x + 2, 4, 11), WorkPalette.IRON_DARK)   # the saddles
	WorkPalette.drum(c, size, 3, 12, 7.5, 8.0, 4.0, WorkPalette.PAINT)   # the dynamo
	for x in [3, 11]:
		WorkPalette.drum(c, size, x, x + 1, 7.5, 8.0, 4.0, WorkPalette.IRON_DARK)
		WorkPalette.drum_rivets(c, size, x, 7.5, 8.0, 4.0)
	WorkPalette.box(c, size, Vector3i(6, 11, 7), Vector3i(9, 13, 9), WorkPalette.COPPER)   # the terminal
	WorkPalette.box(c, size, Vector3i(12, 7, 7), Vector3i(14, 8, 9), WorkPalette.IRON_DARK)   # the shaft
	var dirs := [Vector2i(1, 0), Vector2i(0, 1), Vector2i(-1, 0), Vector2i(0, -1)]   # (y, z), a quarter turn a step
	var dv: Vector2i = dirs[step % 4]
	for r in range(0, 5):
		c[WorkPalette.idx(size, 13, 7 + dv.x * r, 8 + dv.y * r)] = WorkPalette.IRON_DARK + 1   # the arm
	WorkPalette.box(c, size, Vector3i(14, 7 + dv.x * 4, 8 + dv.y * 4), Vector3i(16, 8 + dv.x * 4, 9 + dv.y * 4), WorkPalette.WOOD_DARK)   # the grip
	return c


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z]}


func load_from(_d: Dictionary) -> void:
	pass


func summary() -> Dictionary:
	return {"kind": kind, "drive": drive, "volts": volts, "amps": amps}
