class_name LaidEntity
extends RefCounted

## A tool or ingots set down in the world (ADR 0095): Vintage Story's ground storage for things that
## are not clay ware. Sneak + right click lays the held tool flat on a top face ("laid_tool"), leans
## it against a wall ("leaning_tool"), or starts an ingot pile ("ingot_pile") that more ingots of the
## same kind grow, Settings.ingot_pile_max to a block in layers that cross like Vintage Story's.
## An empty hand takes the tool or one ingot back; breaking the piece drops everything.
## The stack keeps its per-stack data (a tool's wear, a hot ingot's hot_until) while it lies here.

signal changed

const KINDS := ["laid_tool", "leaning_tool", "ingot_pile"]
## Ingot colours in the pile by metal tag; any other ingot shows STONE_LIGHT.
const METAL_PAINT := {"metal.copper": WorkPalette.COPPER, "metal.bronze": WorkPalette.BRONZE,
	"metal.tin": WorkPalette.TIN, "metal.iron": WorkPalette.IRON_DARK}

var kind := "laid_tool"
var pos: Vector3i
var stack := {}        # the item lying here with its per-stack data; an ingot pile's count
var yaw := 0           # degrees: a laid tool's turn, a leaning tool's wall side (the way the wall's face looks)
var items: ItemRegistry


func _init(p_items: ItemRegistry, p_pos: Vector3i, p_kind: String) -> void:
	items = p_items
	pos = p_pos
	kind = p_kind


## The kind a sneaking right click with `def` makes on a face with outward `normal`, or "" (nothing to lay).
static func kind_for(def: Dictionary, normal: Vector3i) -> String:
	var tags: Array = def.get("tags", [])
	if "ingot" in tags:
		return "ingot_pile" if normal == Vector3i.UP else ""
	if "tool" in tags:
		if normal == Vector3i.UP:
			return "laid_tool"
		return "leaning_tool" if normal.y == 0 else ""
	return ""


## The anchor of a leaning tool's block in front of a wall: flush with the wall face (natural walls
## are not on the block grid), on the block grid along the wall, standing on `front`'s floor.
static func lean_anchor(front: Vector3i, normal: Vector3i) -> Vector3i:
	var a := BlockEntities.anchor_for(front)
	if normal.x != 0:
		a.x = front.x if normal.x > 0 else front.x - Settings.BLOCK + 1
	if normal.z != 0:
		a.z = front.z if normal.z > 0 else front.z - Settings.BLOCK + 1
	return a


## The wall side as a yaw in degrees: the sprite's face (+Z) looks along `normal`.
static func yaw_of(normal: Vector3i) -> int:
	return roundi(rad_to_deg(atan2(float(normal.x), float(normal.z))))


func is_full() -> bool:
	return kind != "ingot_pile" or int(stack.get("count", 0)) >= Settings.ingot_pile_max


## Right click on the piece with `held` (the hand's stack, {} when empty).
## Returns {add: n ingots taken from the hand, give: [stacks], remove: bool}.
func use(held: Dictionary) -> Dictionary:
	var res := {"add": 0, "give": [], "remove": false}
	if held.is_empty():
		var n := 1 if kind == "ingot_pile" else int(stack.get("count", 1))
		res["give"] = [Inventory.make_stack(items, stack["id"], n, Inventory.data_of(stack))]
		stack["count"] = int(stack.get("count", 1)) - n
		res["remove"] = stack["count"] <= 0
		changed.emit()
	elif kind == "ingot_pile" and not is_full() and Inventory.same_kind(stack, held["id"], held):
		stack["count"] = int(stack["count"]) + 1
		res["add"] = 1
		changed.emit()
	return res


func contents() -> Array:
	return [stack.duplicate(true)] if not stack.is_empty() else []


func view_parts() -> Dictionary:
	var id: String = stack.get("id", "")
	match kind:
		"laid_tool":
			return {"items": [{"id": id, "at": Vector3(0.5, 0.03, 0.5), "yaw": yaw, "scale": 0.9}]}
		"leaning_tool":
			# upright, turned so the sprite's diagonal handle stands up, its head tipped back onto the wall
			var n := Vector3(sin(deg_to_rad(yaw)), 0.0, cos(deg_to_rad(yaw)))
			var at := Vector3(0.5, 0.47, 0.5) - n * 0.3
			return {"items": [{"id": id, "at": at, "yaw": yaw, "upright": true, "tilt": -18.0, "roll": 45.0, "scale": 0.68}]}
	return {"solid": {"cells": pile_cells(int(stack.get("count", 0)), _paint()), "size": PILE, "grid": PILE.x}}


const PILE := Vector3i(16, 16, 16)   # a pile's cells: 3.125 cm, fine enough for an ingot's sloped sides

## An ingot pile's cells: cast ingots 4 cells wide, 8 long, 2 high (the top cell inset by one all round),
## 8 to a layer (4 across, 2 along), each layer crossing the one below; 8 layers fill the block.
static func pile_cells(n: int, paint: int) -> PackedInt32Array:
	var c := WorkPalette.new_cells(PILE)
	for i in mini(n, 64):
		var l := i / 8
		var k := i % 8
		var across := (k % 4) * 4
		var along := (k / 4) * 8
		for t in 2:
			for a in range(t, 8 - t):
				for w in range(t, 4 - t):
					var x := across + w if l % 2 == 0 else along + a
					var z := along + a if l % 2 == 0 else across + w
					c[WorkPalette.idx(PILE, x, l * 2 + t, z)] = paint + 1
	return c


func _paint() -> int:
	for t in items.get_def(stack.get("id", "")).get("tags", []):
		if METAL_PAINT.has(t):
			return METAL_PAINT[t]
	return WorkPalette.STONE_LIGHT


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "stack": stack.duplicate(true), "yaw": yaw}


func load_from(d: Dictionary) -> void:
	var s: Dictionary = d.get("stack", {})
	if items.has(String(s.get("id", ""))):
		stack = Inventory.make_stack(items, s["id"], int(s.get("count", 1)), s)
	yaw = int(d.get("yaw", 0))


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos, "stack": stack.duplicate(true), "yaw": yaw}
