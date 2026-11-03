class_name CircuitEntity
extends RefCounted

## Circuit protection and control on the LV network (ADR 0123), one block of conductor each, the kind from the
## item's `"circuit": {"amps": rating}`: a knife switch (right click opens and closes it), a fuse box (its fuse
## wire blows above the rating; a right click with fuse wire in hand refits it), a breaker (trips above the
## rating; a right click resets it), a daylight switch (WorkSystem opens it by day and closes it at night, so lamps
## on it burn only after dark). Open, it is no conductor at all: the circuit beyond it is dead.

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
	if kind in ["fuse_box", "breaker"] and last_amps > amps and not open:
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
		"daylight_switch":
			res["msg"] = "A daylight switch: %s." % ("open by day" if open else "closed for the night")
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
		"daylight_switch":
			return ["Open: daylight" if open else "Closed: night, %.1f A through it" % last_amps]
	return ["Blown: refit with fuse wire" if open else "%d A fuse, %.1f A through it" % [roundi(amps), last_amps]]


## A small iron box on the ground with a copper lever (switch), a glass window (fuse) or a red button (breaker).
## ADR 0129: a riveted iron base with copper lugs at both ends (the cable runs along x), and on it the kind:
## a slate-mounted copper blade with a wooden grip that stands up when open, a steel-lidded fuse box with its
## cartridge in a window (sooted when blown), a green breaker with a hazard lever that drops and a red flag when tripped.
func view_parts() -> Dictionary:
	var n := 16
	var size := Vector3i(n, 8, n)
	var c := WorkPalette.new_cells(size)
	WorkPalette.box(c, size, Vector3i(2, 0, 2), Vector3i(14, 2, 14), WorkPalette.IRON_DARK)   # the base
	for p in [Vector2i(3, 3), Vector2i(12, 3), Vector2i(3, 12), Vector2i(12, 12)]:
		c[WorkPalette.idx(size, p.x, 1, p.y)] = WorkPalette.RIVET + 1
	WorkPalette.box(c, size, Vector3i(0, 1, 6), Vector3i(n, 3, 10), WorkPalette.COPPER)   # the conductor through it
	for x in [0, n - 2]:
		WorkPalette.box(c, size, Vector3i(x, 1, 5), Vector3i(x + 2, 4, 11), WorkPalette.BRONZE)   # the lugs
	match kind:
		"knife_switch":
			WorkPalette.box(c, size, Vector3i(3, 2, 3), Vector3i(13, 3, 13), WorkPalette.STONE_DARK)   # the slate
			for x in [4, 11]:
				WorkPalette.box(c, size, Vector3i(x, 3, 6), Vector3i(x + 1, 5, 10), WorkPalette.COPPER)   # hinge and jaw
			if open:   # the blade stands up from its hinge, grip on top
				WorkPalette.box(c, size, Vector3i(4, 5, 7), Vector3i(5, 8, 9), WorkPalette.COPPER)
				WorkPalette.box(c, size, Vector3i(3, 7, 7), Vector3i(6, 8, 9), WorkPalette.WOOD_DARK)
			else:
				WorkPalette.box(c, size, Vector3i(4, 4, 7), Vector3i(12, 5, 9), WorkPalette.COPPER)
				WorkPalette.box(c, size, Vector3i(11, 4, 6), Vector3i(13, 6, 10), WorkPalette.WOOD_DARK)
		"breaker":
			WorkPalette.box(c, size, Vector3i(3, 2, 3), Vector3i(13, 7, 13), WorkPalette.PAINT)
			WorkPalette.box(c, size, Vector3i(3, 6, 3), Vector3i(13, 7, 13), WorkPalette.STEEL)
			WorkPalette.box(c, size, Vector3i(6, 7, 7), Vector3i(10, 8, 9), WorkPalette.HAZARD if not open else WorkPalette.SOOT)   # the lever
			WorkPalette.box(c, size, Vector3i(5, 3, 12), Vector3i(11, 6, 13), WorkPalette.IRON_DARK)   # the flag window
			WorkPalette.box(c, size, Vector3i(6, 4, 12), Vector3i(10, 5, 13), WorkPalette.EMBER if open else WorkPalette.SOOT)
		"daylight_switch":   # a photocell under a glass dome on the lid (ADR 0123 round 2)
			WorkPalette.box(c, size, Vector3i(3, 2, 3), Vector3i(13, 7, 13), WorkPalette.IRON_DARK)
			WorkPalette.box(c, size, Vector3i(3, 6, 3), Vector3i(13, 7, 13), WorkPalette.STEEL)
			WorkPalette.box(c, size, Vector3i(6, 7, 6), Vector3i(10, 10, 10), WorkPalette.GLOW if open else WorkPalette.TIN)   # the cell: lit by day
			WorkPalette.box(c, size, Vector3i(5, 7, 5), Vector3i(11, 8, 11), WorkPalette.COPPER)
		_:
			WorkPalette.box(c, size, Vector3i(3, 2, 3), Vector3i(13, 7, 13), WorkPalette.IRON_DARK)
			WorkPalette.box(c, size, Vector3i(3, 6, 3), Vector3i(13, 7, 13), WorkPalette.STEEL)   # the lid
			for p in [Vector2i(4, 4), Vector2i(11, 4), Vector2i(4, 11), Vector2i(11, 11)]:
				c[WorkPalette.idx(size, p.x, 6, p.y)] = WorkPalette.RIVET + 1
			WorkPalette.box(c, size, Vector3i(4, 3, 12), Vector3i(12, 6, 13), WorkPalette.SOOT)   # the window
			WorkPalette.box(c, size, Vector3i(6, 4, 12), Vector3i(10, 5, 13), WorkPalette.SOOT if open else WorkPalette.TIN)   # the cartridge
			for x in [5, 10]:
				WorkPalette.box(c, size, Vector3i(x, 4, 12), Vector3i(x + 1, 5, 13), WorkPalette.COPPER)
	return {"solid": {"cells": c, "size": size, "grid": n}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "open": open}


func load_from(d: Dictionary) -> void:
	open = bool(d.get("open", false))


func summary() -> Dictionary:
	return {"kind": kind, "open": open, "amps": last_amps}
