class_name GraveEntity
extends RefCounted

## A grave (plan item D12, ADR 0055): dying in survival puts everything the player carried here, at the
## death spot, per-stack data and all (Inventory.make_stack). It never despawns, gets a "grave" waypoint
## and its coordinates in chat; a right click hands it all back (what does not fit drops), breaking it
## spills it. A low stone slab with a headstone, drawn by WorkSystem from view_parts().

signal changed

var kind := "grave"
var pos: Vector3i
var items: ItemRegistry
var stacks: Array = []    # everything the player had
var died_at := ""         # the date line from the calendar, for the look-at panel


func _init(p_items: ItemRegistry, p_pos: Vector3i) -> void:
	items = p_items
	pos = p_pos


func set_stacks(p_stacks: Array) -> void:
	stacks.clear()
	for s in p_stacks:
		if s is Dictionary and not s.is_empty() and int(s.get("count", 0)) > 0:
			stacks.append(Inventory.make_stack(items, s["id"], int(s["count"]), s))
	changed.emit()


## Hands everything out (the grave is then empty and is removed by the caller).
func take_all() -> Array:
	var out := stacks
	stacks = []
	changed.emit()
	return out


func count() -> int:
	return stacks.size()


func contents() -> Array:
	return stacks.duplicate(true)   # breaking the grave spills it; the grave itself drops nothing


func tick() -> bool:
	return false


func is_lit() -> bool:
	return false


func view_parts() -> Dictionary:
	var size := Vector3i(8, 8, 8)
	var c := WorkPalette.new_cells(size)
	WorkPalette.box(c, size, Vector3i(1, 0, 1), Vector3i(7, 1, 7), WorkPalette.SOIL)    # a mound
	WorkPalette.box(c, size, Vector3i(2, 1, 5), Vector3i(6, 6, 7), WorkPalette.STONE)   # the headstone
	WorkPalette.box(c, size, Vector3i(3, 6, 5), Vector3i(5, 7, 7), WorkPalette.STONE)
	WorkPalette.box(c, size, Vector3i(2, 3, 4), Vector3i(6, 4, 5), WorkPalette.ASH)     # a cross on its face
	WorkPalette.box(c, size, Vector3i(3, 1, 4), Vector3i(5, 5, 5), WorkPalette.ASH)
	return {"solid": {"cells": c, "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "stacks": stacks, "died_at": died_at}


func load_from(d: Dictionary) -> void:
	set_stacks(d.get("stacks", []))
	died_at = String(d.get("died_at", ""))


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos, "stacks": stacks.size()}
