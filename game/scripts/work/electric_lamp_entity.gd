class_name ElectricLampEntity
extends RefCounted

## An electric lamp (ADR 0117, 0123): an iron post with a glass bulb, a `load.watts` load on the LV network
## (PowerLoad); lit while its voltage holds, so TorchLights lights the room from it like a fire.

signal changed

var kind := "electric_lamp"
var pos: Vector3i
var lit := false
var load: PowerLoad


func _init(p_items: ItemRegistry, p_pos: Vector3i) -> void:
	pos = p_pos
	load = PowerLoad.new(float(p_items.get_def(kind).get("load", {}).get("watts", 40.0)))


func net(channel: String) -> Dictionary:
	return load.net(true) if channel == "energy" else {}


func net_in(channel: String, d) -> bool:
	if channel != "energy":
		return false
	var started := load.net_in(d)
	if load.on != lit:
		lit = load.on
		changed.emit()
	return started


func tick() -> bool:
	return false


func is_lit() -> bool:
	return lit


func use(_stack: Dictionary) -> Dictionary:
	return {"consume": 0, "give": [], "msg": "The lamp is %s (%s)." % ["on" if lit else "dark", load.status()]}


func contents() -> Array:
	return [{"id": kind, "count": 1}]


func look_lines() -> Array[String]:
	return [("Lit, " if lit else "Dark, ") + load.status()]


## A post up the middle with a bulb on top that glows while lit.
## An iron post lamp (ADR 0129): a riveted foot, bronze collars on the post, a caged bulb under a steel cap.
func view_parts() -> Dictionary:
	var n := 16
	var size := Vector3i(n, 24, n)
	var c := WorkPalette.new_cells(size)
	WorkPalette.box(c, size, Vector3i(3, 0, 3), Vector3i(13, 2, 13), WorkPalette.IRON_DARK)   # the foot
	WorkPalette.box(c, size, Vector3i(4, 2, 4), Vector3i(12, 3, 12), WorkPalette.STEEL)
	for p in [Vector2i(4, 4), Vector2i(11, 4), Vector2i(4, 11), Vector2i(11, 11)]:
		c[WorkPalette.idx(size, p.x, 2, p.y)] = WorkPalette.RIVET + 1
	WorkPalette.box(c, size, Vector3i(7, 3, 7), Vector3i(9, 15, 9), WorkPalette.IRON_DARK)   # the post
	for y in [3, 9, 14]:
		WorkPalette.box(c, size, Vector3i(6, y, 6), Vector3i(10, y + 1, 10), WorkPalette.BRONZE)
	WorkPalette.box(c, size, Vector3i(4, 15, 4), Vector3i(12, 16, 12), WorkPalette.STEEL)   # the lamp's base
	WorkPalette.box(c, size, Vector3i(5, 16, 5), Vector3i(11, 22, 11), WorkPalette.GLOW if lit else WorkPalette.TIN)   # the bulb
	for x in [4, 11]:
		for z in [4, 11]:
			WorkPalette.box(c, size, Vector3i(x, 16, z), Vector3i(x + 1, 22, z + 1), WorkPalette.IRON_DARK)   # the cage
	for x in [7, 8]:
		for z in [4, 11]:
			WorkPalette.box(c, size, Vector3i(x, 16, z), Vector3i(x + 1, 22, z + 1), WorkPalette.IRON_DARK)
			WorkPalette.box(c, size, Vector3i(z, 16, x), Vector3i(z + 1, 22, x + 1), WorkPalette.IRON_DARK)
	WorkPalette.box(c, size, Vector3i(4, 22, 4), Vector3i(12, 23, 12), WorkPalette.STEEL)   # the cap
	WorkPalette.box(c, size, Vector3i(6, 23, 6), Vector3i(10, 24, 10), WorkPalette.IRON_DARK)
	c[WorkPalette.idx(size, 7, 23, 7)] = WorkPalette.RIVET + 1
	return {"solid": {"cells": c, "size": size, "grid": n}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z]}


func load_from(_d: Dictionary) -> void:
	pass


func summary() -> Dictionary:
	return {"kind": kind, "lit": lit}
