class_name HopperEntity
extends RefCounted

## The hopper and the chute (ADR 0100). A hopper (`item` "hopper") holds SLOTS stacks; every hopper_s
## it pulls one item out of the piece above it and pushes one into the piece it faces, or under it
## when nothing faces it. A chute ("chute", wood, cheap) also pulls from the piece behind it but only
## drops into the piece below: the way down a ledge. A right click with items loads it, an empty
## hand empties it. A wrench turns it.

signal changed

const SLOTS := 5

var kind := "hopper"
var item := "hopper"
var pos: Vector3i
var facing := 0
var inv: Inventory
var moved := 0
var _cooldown := 0.0
var _store: BlockEntities


func _init(p_store: BlockEntities, p_pos: Vector3i) -> void:
	_store = p_store
	pos = p_pos
	inv = Inventory.new(p_store.items, SLOTS)
	inv.changed.connect(func(_i: int) -> void: changed.emit())


func is_chute() -> bool:
	return item == "chute"


func is_lit() -> bool:
	return false


## Takes one item in. False when full.
func accept(one: Dictionary) -> bool:
	return inv.add(one["id"], 1, one) == 0


func tick() -> bool:
	_cooldown -= BlockEntities.TICK_S
	if _cooldown > 0.0:
		return false
	_cooldown = Settings.hopper_s
	var n := Settings.BLOCK
	var sources := [_store.at(pos + Vector3i(0, n, 0))]
	if is_chute():
		sources.append(_store.at(pos - Transport.dir_of(facing)))
	for s in sources:
		if s != null and s != self:
			var got := Transport.pull(s)
			if not got.is_empty():
				if inv.add(got["id"], int(got["count"]), got) > 0:
					Transport.give(s, got, _store.items)   # full: the one item pulled too many goes back
				moved += 1
				break
	for i in inv.size():
		if inv.is_empty_slot(i):
			continue
		var s := inv.get_slot(i)
		var targets := [_store.at(pos + Vector3i(0, -n, 0))] if is_chute() else [_store.at(pos + Transport.dir_of(facing)), _store.at(pos + Vector3i(0, -n, 0))]
		for t in targets:
			if t != null and t != self and Transport.give(t, s, _store.items) > 0:
				inv.take(i, 1)
				moved += 1
				return false
		break
	return false


## Right click with the held `stack` (changed in place). Returns {consume, give, msg}.
func use(stack: Dictionary) -> Dictionary:
	var res := {"consume": 0, "give": [], "msg": ""}
	if stack.is_empty():
		for i in inv.size():
			if not inv.is_empty_slot(i):
				res["give"].append(inv.take(i))
		if res["give"].is_empty():
			res["msg"] = "Empty. It pulls from the piece above%s and pushes %s." % [" or behind" if is_chute() else "", "down" if is_chute() else "the way it faces, or down"]
		return res
	if not accept({"id": stack["id"], "count": 1}):
		res["msg"] = "It holds no more."
		return res
	res["consume"] = 1
	return res


func contents() -> Array:
	var out := [{"id": item, "count": 1}]
	for s in inv.to_array():
		if not s.is_empty():
			out.append(s)
	return out


func look_lines() -> Array[String]:
	var out: Array[String] = []
	for s in inv.to_array():
		if not s.is_empty():
			out.append("%d %s" % [s["count"], _store.items.display_name(s["id"])])
	if out.is_empty():
		out.append("Empty")
	return out


## A funnel: a wide square mouth narrowing to a spout at the bottom, with a short pipe out of its
## facing side (the hopper) or just the spout (the chute, in wood).
func view_parts() -> Dictionary:
	var size := Vector3i(8, 8, 8)
	var c := WorkPalette.new_cells(size)
	var tone := WorkPalette.WOOD if is_chute() else WorkPalette.IRON_DARK
	WorkPalette.wall_box(c, size, Vector3i(0, 5, 0), Vector3i(8, 8, 8), [tone], tone)
	WorkPalette.wall_box(c, size, Vector3i(1, 3, 1), Vector3i(7, 5, 7), [tone], tone)
	WorkPalette.box(c, size, Vector3i(3, 0, 3), Vector3i(5, 3, 5), WorkPalette.STONE_DARK if is_chute() else WorkPalette.ANVIL)   # the spout
	if not is_chute():
		WorkPalette.box(c, size, Vector3i(3, 1, 0), Vector3i(5, 3, 3), WorkPalette.BRONZE)   # the side pipe, toward -z (facing 0)
	var fill := 0
	for s in inv.to_array():
		if not s.is_empty():
			fill += 1
	if fill > 0:
		WorkPalette.box(c, size, Vector3i(1, 5, 1), Vector3i(7, 5 + mini(fill, 3), 7), WorkPalette.GRAVEL)
	return {"solid": {"cells": WorkPalette.rotate_y(c, size, facing), "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "item": item, "facing": facing, "slots": inv.to_array()}


func load_from(d: Dictionary) -> void:
	item = String(d.get("item", "hopper"))
	facing = int(d.get("facing", 0))
	var slots: Array = d.get("slots", [])
	for i in mini(slots.size(), inv.size()):
		inv.set_slot(i, slots[i] if slots[i] is Dictionary else {})


func summary() -> Dictionary:
	return {"kind": kind, "item": item, "moved": moved}
