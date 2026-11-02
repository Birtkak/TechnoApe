class_name ElectricLampEntity
extends RefCounted

## An electric lamp (ADR 0117): an iron post with a glass bulb that wants Settings.lamp_eu_t from the
## cables and counts as lit while it gets it, so TorchLights lights the room from it like a fire.

signal changed

var kind := "electric_lamp"
var pos: Vector3i
var lit := false


func _init(p_pos: Vector3i) -> void:
	pos = p_pos


func net(channel: String) -> Dictionary:
	return {"role": NetworkSolver.SINK, "amount": Settings.lamp_eu_t} if channel == "energy" else {}


func net_in(channel: String, amount: float) -> void:
	if channel != "energy":
		return
	var on := amount >= Settings.lamp_eu_t - 1e-3
	if on != lit:
		lit = on
		changed.emit()


func tick() -> bool:
	return false


func is_lit() -> bool:
	return lit


func use(_stack: Dictionary) -> Dictionary:
	return {"consume": 0, "give": [], "msg": "The lamp is %s." % ("on" if lit else "dark: cable it to a generator")}


func contents() -> Array:
	return [{"id": kind, "count": 1}]


func look_lines() -> Array[String]:
	return ["Lit" if lit else "Dark: no power (%d EU/t)" % roundi(Settings.lamp_eu_t)]


## A post up the middle with a bulb on top that glows while lit.
func view_parts() -> Dictionary:
	var size := Vector3i(8, 12, 8)
	var c := WorkPalette.new_cells(size)
	WorkPalette.box(c, size, Vector3i(2, 0, 2), Vector3i(6, 1, 6), WorkPalette.IRON_DARK)   # the foot
	WorkPalette.box(c, size, Vector3i(3, 1, 3), Vector3i(5, 8, 5), WorkPalette.IRON_DARK)   # the post
	WorkPalette.box(c, size, Vector3i(2, 8, 2), Vector3i(6, 12, 6), WorkPalette.GLOW if lit else WorkPalette.STONE_LIGHT)   # the bulb
	return {"solid": {"cells": c, "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z]}


func load_from(_d: Dictionary) -> void:
	pass


func summary() -> Dictionary:
	return {"kind": kind, "lit": lit}
