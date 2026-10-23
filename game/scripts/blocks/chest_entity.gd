class_name ChestEntity
extends RefCounted

## A single chest (ADR 0007): 27 slots, Minecraft's shift-click order; the
## storage pot of ADR 0030 is the same container with 9 slots. Pure
## logic; InventoryScreen shows it, BlockEntities owns it by voxel position.
## Container interface shared with FurnaceEntity (duck-typed): kind, title,
## pos, inv, may_place(), is_output(), shift_targets(), shift_out_reverse(),
## tick(), is_lit(), contents(), summary(), signal changed.

signal changed

const SIZE := 27

var kind := "chest"
var title := "Chest"
var pos: Vector3i
var inv: Inventory
var size := SIZE


func _init(p_items: ItemRegistry, p_pos: Vector3i, p_kind: String = "chest") -> void:
	pos = p_pos
	kind = p_kind
	if kind == "storage_pot":
		size = 9
		title = "Storage Pot"
	inv = Inventory.new(p_items, size)
	inv.changed.connect(_on_slot_changed)   # method, not a lambda: no reference cycle


func _on_slot_changed(_i: int) -> void:
	changed.emit()


func may_place(_slot: int, _id: String) -> bool:
	return true


func is_output(_slot: int) -> bool:
	return false


## Slots a shift-clicked player stack tries, in order (vanilla: forward).
func shift_targets(_id: String) -> Array:
	return range(size)


## Vanilla moves chest stacks into the player inventory back to front.
func shift_out_reverse(_slot: int) -> bool:
	return true


## Chests do not tick. Returns true when the lit state flipped (never).
func tick() -> bool:
	return false


func is_lit() -> bool:
	return false


## Non-empty stacks, for dropping when the block breaks.
func contents() -> Array:
	var out := []
	for s in inv.to_array():
		if not s.is_empty():
			out.append(s)
	return out


## Save data (BlockEntities writes it next to the world save).
func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "slots": inv.to_array()}


func load_from(d: Dictionary) -> void:
	var slots: Array = d.get("slots", [])
	for i in mini(slots.size(), size):
		inv.set_slot(i, slots[i])


func summary() -> Dictionary:
	var c := {}
	for s in contents():
		c[s["id"]] = int(c.get(s["id"], 0)) + int(s["count"])
	return {"kind": kind, "pos": pos, "items": c}
