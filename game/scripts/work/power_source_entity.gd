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


## The wheel: a disc of spokes on an iron axle over a frame; the turbine: a tall post with three blades; the
## crank: an iron box with a copper handle. The turning ones step with `spin`.
func view_parts() -> Dictionary:
	var size := Vector3i(8, 8, 8)
	var c := WorkPalette.new_cells(size)
	match kind:
		"water_wheel":
			size = Vector3i(16, 16, 16)
			c = WorkPalette.new_cells(size)
			WorkPalette.box(c, size, Vector3i(0, 0, 6), Vector3i(2, 10, 10), WorkPalette.WOOD_DARK)    # the frame
			WorkPalette.box(c, size, Vector3i(14, 0, 6), Vector3i(16, 10, 10), WorkPalette.WOOD_DARK)
			WorkPalette.box(c, size, Vector3i(0, 9, 7), Vector3i(16, 10, 9), WorkPalette.IRON_DARK)   # the axle
			var phase := (spin / 5) % 4
			for i in 8:   # eight paddles round the axle, turned by the phase
				var a := (i + phase * 0.5) * PI / 4.0
				for r in range(2, 7):
					var y := 9 + int(round(sin(a) * r))
					var z := 8 + int(round(cos(a) * r))
					if y >= 0 and y < 16 and z >= 0 and z < 16:
						for x in range(3, 13):
							c[WorkPalette.idx(size, x, y, z)] = WorkPalette.WOOD + 1
			WorkPalette.box(c, size, Vector3i(14, 7, 7), Vector3i(16, 12, 9), WorkPalette.COPPER)   # the dynamo on the axle's end
			return {"solid": {"cells": c, "size": size, "grid": 16}}
		"wind_turbine":
			size = Vector3i(8, 24, 8)
			c = WorkPalette.new_cells(size)
			WorkPalette.box(c, size, Vector3i(2, 0, 2), Vector3i(6, 1, 6), WorkPalette.IRON_DARK)
			WorkPalette.box(c, size, Vector3i(3, 1, 3), Vector3i(5, 20, 5), WorkPalette.STEEL)   # the post
			WorkPalette.box(c, size, Vector3i(2, 19, 3), Vector3i(6, 22, 5), WorkPalette.COPPER)  # the dynamo
			var phase := (spin / 5) % 3
			for i in 3:   # three blades in the x-y plane at the front
				var a := (i * 2.0 + phase * 0.33) * PI / 3.0
				for r in range(1, 4):
					var x := 4 + int(round(cos(a) * r))
					var y := 21 + int(round(sin(a) * r))
					if x >= 0 and x < 8 and y >= 0 and y < 24:
						c[WorkPalette.idx(size, x, y, 2)] = WorkPalette.TIN + 1
			return {"solid": {"cells": c, "size": size}}
		_:
			WorkPalette.box(c, size, Vector3i(1, 0, 1), Vector3i(7, 5, 7), WorkPalette.IRON_DARK)
			WorkPalette.box(c, size, Vector3i(3, 5, 3), Vector3i(5, 6, 5), WorkPalette.STEEL)
			var side := 1 if (spin / 5) % 2 == 0 else -1
			WorkPalette.box(c, size, Vector3i(4 + 2 * side if side > 0 else 1, 5, 3), Vector3i(7 if side > 0 else 4, 7, 5), WorkPalette.COPPER)   # the handle
			return {"solid": {"cells": c, "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z]}


func load_from(_d: Dictionary) -> void:
	pass


func summary() -> Dictionary:
	return {"kind": kind, "drive": drive, "volts": volts, "amps": amps}
