class_name BeltEntity
extends RefCounted

## A conveyor belt (ADR 0100): a half-block of leather belt over rollers that carries up to CARRY
## items the way it faces, one block in belt_s, and hands each into the next belt, hopper or
## machine at its end (Transport.give). Belts take from hoppers and other belts; a right click
## with an item drops it on, an empty hand takes what rides. The items are drawn riding along.

signal changed

const CARRY := 4
const GAP := 0.25   # of a block between items

var kind := "belt"
var pos: Vector3i
var facing := 0
var carried: Array = []   # [{stack, t}] from the back (t 0) to the front (t 1)
var moved := 0
var _store: BlockEntities
var _frame := 0


func _init(p_store: BlockEntities, p_pos: Vector3i) -> void:
	_store = p_store
	pos = p_pos


func is_lit() -> bool:
	return not carried.is_empty()   # BlockEntities.tick redraws it: the items move


func accept(one: Dictionary) -> bool:
	if carried.size() >= CARRY or (not carried.is_empty() and float(carried[-1]["t"]) < GAP):
		return false
	carried.append({"stack": one, "t": 0.0})
	changed.emit()
	return true


func tick() -> bool:
	if carried.is_empty():
		return false
	var step := BlockEntities.TICK_S / Settings.belt_s
	var head: Dictionary = carried[0]
	if float(head["t"]) >= 1.0:
		var t := _store.at(pos + Transport.dir_of(facing))
		if t != null and t != self and Transport.give(t, head["stack"], _store.items) > 0:
			carried.pop_front()
			moved += 1
	var limit := 1.0
	for c in carried:
		c["t"] = minf(float(c["t"]) + step, limit)
		limit = float(c["t"]) - GAP
	_frame += 1
	if _frame % 4 == 0:
		changed.emit()
	return false


func use(stack: Dictionary) -> Dictionary:
	var res := {"consume": 0, "give": [], "msg": ""}
	if stack.is_empty():
		for c in carried:
			res["give"].append(c["stack"])
		carried.clear()
		if res["give"].is_empty():
			res["msg"] = "An empty belt: hoppers and belts behind it feed it."
		changed.emit()
		return res
	if not accept({"id": stack["id"], "count": 1}):
		res["msg"] = "The belt is full."
		return res
	res["consume"] = 1
	return res


func contents() -> Array:
	var out := [{"id": "belt", "count": 1}]
	for c in carried:
		out.append(c["stack"])
	return out


func look_lines() -> Array[String]:
	var out: Array[String] = []
	for c in carried:
		out.append(_store.items.display_name(c["stack"]["id"]))
	if out.is_empty():
		out.append("Empty belt")
	return out


## Two iron rails with rollers between them under a leather belt; the items ride on top, from the
## back to the front (facing).
func view_parts() -> Dictionary:
	var size := Vector3i(8, 4, 8)
	var c := WorkPalette.new_cells(size)
	WorkPalette.box(c, size, Vector3i(0, 0, 0), Vector3i(1, 3, 8), WorkPalette.IRON_DARK)
	WorkPalette.box(c, size, Vector3i(7, 0, 0), Vector3i(8, 3, 8), WorkPalette.IRON_DARK)
	for z in range(0, 8, 2):
		WorkPalette.box(c, size, Vector3i(1, 0, z), Vector3i(7, 2, z + 1), WorkPalette.ANVIL)   # the rollers
	WorkPalette.box(c, size, Vector3i(1, 2, 0), Vector3i(7, 3, 8), WorkPalette.LEATHER)         # the belt
	var d := Vector3(Transport.DIRS[posmod(facing, 4)])
	var laid := []
	for item in carried:
		var t := float(item["t"])
		laid.append({"id": item["stack"]["id"], "at": Vector3(0.5, 0.4, 0.5) + d * (t - 0.5), "yaw": 0.0, "scale": 0.5})
	return {"solid": {"cells": WorkPalette.rotate_y(c, size, facing), "size": size}, "items": laid}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "facing": facing, "carried": carried}


func load_from(d: Dictionary) -> void:
	facing = int(d.get("facing", 0))
	carried = d.get("carried", [])


func summary() -> Dictionary:
	return {"kind": kind, "carrying": carried.size(), "moved": moved}
