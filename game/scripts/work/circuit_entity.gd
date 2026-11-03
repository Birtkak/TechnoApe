class_name CircuitEntity
extends RefCounted

## Circuit protection and control on the LV network (ADR 0123), one block of conductor each, the kind from the
## item's `"circuit": {"amps": rating}`: a knife switch (right click opens and closes it), a fuse box (its fuse
## wire blows above the rating; a right click with fuse wire in hand refits it), a breaker (trips above the
## rating; a right click resets it). Open, it is no conductor at all: the circuit beyond it is dead.

signal changed

var kind: String          # "knife_switch", "fuse_box", "breaker"
var pos: Vector3i
var amps := 30.0          # the rating
var open := false         # a switch opened, a fuse blown, a breaker tripped
var last_amps := 0.0


func _init(p_items: ItemRegistry, p_pos: Vector3i, p_kind: String) -> void:
	pos = p_pos
	kind = p_kind
	amps = float(p_items.get_def(p_kind).get("circuit", {}).get("amps", 30.0))
	open = p_kind == "fuse_box" and false


func net(channel: String) -> Dictionary:
	if channel != "energy" or open:
		return {}
	return {"kind": "conductor", "ohms": Settings.circuit_device_ohms, "amps": amps}


func net_in(channel: String, d) -> bool:
	if channel != "energy":
		return false
	last_amps = float(d.get("amps", 0.0))
	if kind != "knife_switch" and last_amps > amps and not open:
		open = true   # the fuse wire melts, the breaker trips
		Sound.play("elec/fuse" if kind == "fuse_box" else "elec/breaker", Vector3(pos) + Vector3(0.5, 0.5, 0.5))
		changed.emit()
	return false


func tick() -> bool:
	return false


func is_lit() -> bool:
	return false


## Right click with the held stack: {consume, give, msg}.
func use(stack: Dictionary) -> Dictionary:
	var res := {"consume": 0, "give": [], "msg": ""}
	match kind:
		"knife_switch":
			open = not open
			Sound.play("elec/switch", Vector3(pos) + Vector3(0.5, 0.5, 0.5), {"pitch": 0.85 if open else 1.0})
			res["msg"] = "The switch is %s." % ("open: the circuit beyond it is dead" if open else "closed")
		"breaker":
			if open:
				open = false
				Sound.play("elec/switch", Vector3(pos) + Vector3(0.5, 0.5, 0.5), {"pitch": 1.4, "db": -6.0})   # the lever pushed back up
				res["msg"] = "Reset. It trips above %d A." % roundi(amps)
			else:
				res["msg"] = "Armed: it trips above %d A (%.1f A now)." % [roundi(amps), last_amps]
		"fuse_box":
			if not open:
				res["msg"] = "The %d A fuse is good (%.1f A now)." % [roundi(amps), last_amps]
			elif stack.get("id", "") == "fuse":
				open = false
				res["consume"] = 1
				res["msg"] = "A new fuse. It blows above %d A." % roundi(amps)
			else:
				res["msg"] = "The fuse is blown: refit it with fuse wire in hand."
	changed.emit()
	return res


func contents() -> Array:
	return [{"id": kind, "count": 1}]


func look_lines() -> Array[String]:
	match kind:
		"knife_switch":
			return ["Open" if open else "Closed, %.1f A through it" % last_amps]
		"breaker":
			return ["Tripped: right click to reset" if open else "Armed at %d A, %.1f A through it" % [roundi(amps), last_amps]]
	return ["Blown: refit with fuse wire" if open else "%d A fuse, %.1f A through it" % [roundi(amps), last_amps]]


## A small iron box on the ground with a copper lever (switch), a glass window (fuse) or a red button (breaker).
func view_parts() -> Dictionary:
	var size := Vector3i(8, 4, 8)
	var c := WorkPalette.new_cells(size)
	WorkPalette.box(c, size, Vector3i(1, 0, 1), Vector3i(7, 3, 7), WorkPalette.IRON_DARK)
	WorkPalette.box(c, size, Vector3i(0, 1, 3), Vector3i(8, 2, 5), WorkPalette.COPPER)   # the terminals through it
	match kind:
		"knife_switch":
			WorkPalette.box(c, size, Vector3i(3, 3, 2 if open else 3), Vector3i(5, 4, 4 if open else 6), WorkPalette.COPPER)
		"breaker":
			WorkPalette.box(c, size, Vector3i(3, 3, 3), Vector3i(5, 4, 5), WorkPalette.EMBER if open else WorkPalette.SOOT)
		_:
			WorkPalette.box(c, size, Vector3i(2, 3, 2), Vector3i(6, 4, 6), WorkPalette.SOOT if open else WorkPalette.TIN)
	return {"solid": {"cells": c, "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "open": open}


func load_from(d: Dictionary) -> void:
	open = bool(d.get("open", false))


func summary() -> Dictionary:
	return {"kind": kind, "open": open, "amps": last_amps}
