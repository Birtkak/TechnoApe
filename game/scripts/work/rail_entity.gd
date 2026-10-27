class_name RailEntity
extends RefCounted

## A rail (ADR 0100): a flat piece along one axis that joins the rails next to it, straight on or
## round a corner. A minecart set on a rail stays there as the rail's `cart`; a right click rides
## it: the player (Player.ride) rolls along the rails at minecart_speed_m with forward and back, and
## sneaking gets off. Sneak and right click takes the cart back. Flat only.

signal changed

var kind := "rail"
var pos: Vector3i
var axis := 0   # 0 along z, 1 along x (like a pipe)
var cart := false


func _init(p_pos: Vector3i) -> void:
	pos = p_pos


## The rail reached from this one going `dir` (a block step), straight on first, else round either
## corner; null at the end of the line. Returns {rail, dir}.
static func next_of(store: BlockEntities, from: RailEntity, dir: Vector3i) -> Dictionary:
	var n := Settings.BLOCK
	var tries := [dir]
	if dir.x != 0:
		tries.append(Vector3i(0, 0, n))
		tries.append(Vector3i(0, 0, -n))
	else:
		tries.append(Vector3i(n, 0, 0))
		tries.append(Vector3i(-n, 0, 0))
	for d in tries:
		var e: RefCounted = store.at(from.pos + d)
		if e is RailEntity:
			return {"rail": e, "dir": d}
	return {}


func contents() -> Array:
	var out := [{"id": "rail", "count": 1}]
	if cart:
		out.append({"id": "minecart", "count": 1})
	return out


func look_lines() -> Array[String]:
	return ["A minecart: right click to ride, sneak and right click to take it" if cart else "Rail"]


## Two iron rails on wooden sleepers; with a cart on it, an iron tub on four wheels.
func view_parts() -> Dictionary:
	var size := Vector3i(8, 6, 8)
	var c := WorkPalette.new_cells(size)
	for z in range(1, 8, 3):
		WorkPalette.box(c, size, Vector3i(0, 0, z), Vector3i(8, 1, z + 1), WorkPalette.WOOD_DARK)   # the sleepers
	WorkPalette.box(c, size, Vector3i(1, 1, 0), Vector3i(2, 2, 8), WorkPalette.IRON_DARK)
	WorkPalette.box(c, size, Vector3i(6, 1, 0), Vector3i(7, 2, 8), WorkPalette.IRON_DARK)
	if cart:
		for q in [Vector3i(1, 1, 1), Vector3i(6, 1, 1), Vector3i(1, 1, 6), Vector3i(6, 1, 6)]:
			WorkPalette.box(c, size, q, q + Vector3i(1, 2, 1), WorkPalette.ANVIL)                  # the wheels
		WorkPalette.wall_box(c, size, Vector3i(1, 3, 0), Vector3i(7, 6, 8), [WorkPalette.IRON_DARK], WorkPalette.IRON_DARK)
		WorkPalette.box(c, size, Vector3i(1, 3, 0), Vector3i(7, 4, 8), WorkPalette.IRON_DARK)       # the tub floor
	return {"solid": {"cells": WorkPalette.rotate_y(c, size, axis), "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "axis": axis, "cart": cart}


func load_from(d: Dictionary) -> void:
	axis = int(d.get("axis", 0))
	cart = bool(d.get("cart", false))


func summary() -> Dictionary:
	return {"kind": kind, "axis": axis, "cart": cart}
