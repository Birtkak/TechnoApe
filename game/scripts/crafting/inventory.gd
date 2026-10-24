class_name Inventory
extends RefCounted

## Minecraft-style slot inventory. A slot is {id: String, count: int} or empty {}; a perishable item
## (food with "spoil_days", ADR 0040) also carries spoil: 0 fresh .. 1 rotten, kept through every move
## and averaged by count when stacks merge.
## Pure logic, no nodes. The UI drives it through the cursor operations below,
## which reproduce vanilla behaviour: left-click picks up / places a whole
## stack, right-click picks up half / places one, shift-click moves a stack
## to another inventory, merging into existing stacks first.

signal changed(slot: int)

var items: ItemRegistry
var slots: Array = []


func _init(p_items: ItemRegistry, size: int) -> void:
	items = p_items
	slots.resize(size)
	for i in size:
		slots[i] = {}


func size() -> int:
	return slots.size()


func is_empty_slot(i: int) -> bool:
	return slots[i].is_empty()


func get_slot(i: int) -> Dictionary:
	return slots[i]


func set_slot(i: int, stack: Dictionary) -> void:
	slots[i] = {} if stack.is_empty() or int(stack.get("count", 0)) <= 0 else _mk(stack["id"], int(stack["count"]), float(stack.get("spoil", 0.0)))
	changed.emit(i)


## A stack dictionary; spoil only on perishable items.
func _mk(id: String, count: int, spoil: float) -> Dictionary:
	if items.get_def(id).has("spoil_days"):
		return {"id": id, "count": count, "spoil": clampf(spoil, 0.0, 1.0)}
	return {"id": id, "count": count}


## Merges `count` items of `spoil` into stack s (same id): the spoil becomes the count-weighted mean.
static func merge_into(s: Dictionary, count: int, spoil: float) -> void:
	if s.has("spoil"):
		var n := int(s["count"]) + count
		s["spoil"] = (float(s["spoil"]) * int(s["count"]) + spoil * count) / maxf(n, 1)
	s["count"] = int(s["count"]) + count


func count_of(id: String) -> int:
	var n := 0
	for s in slots:
		if not s.is_empty() and s["id"] == id:
			n += s["count"]
	return n


## Adds count of id (perishable: at `spoil`, fresh by default), merging into existing stacks first,
## then empty slots. Returns the leftover that did not fit.
func add(id: String, count: int, spoil: float = 0.0) -> int:
	var max_stack := items.max_stack(id)
	for i in slots.size():
		if count <= 0:
			break
		var s: Dictionary = slots[i]
		if not s.is_empty() and s["id"] == id and s["count"] < max_stack:
			var take := mini(count, max_stack - s["count"])
			merge_into(s, take, spoil)
			count -= take
			changed.emit(i)
	for i in slots.size():
		if count <= 0:
			break
		if slots[i].is_empty():
			var take := mini(count, max_stack)
			slots[i] = _mk(id, take, spoil)
			count -= take
			changed.emit(i)
	return count


## Removes up to count of id anywhere. Returns how many were removed.
func remove(id: String, count: int) -> int:
	var removed := 0
	for i in range(slots.size() - 1, -1, -1):
		if count <= 0:
			break
		var s: Dictionary = slots[i]
		if not s.is_empty() and s["id"] == id:
			var take := mini(count, s["count"])
			s["count"] -= take
			count -= take
			removed += take
			if s["count"] <= 0:
				slots[i] = {}
			changed.emit(i)
	return removed


func has(id: String, count: int = 1) -> bool:
	return count_of(id) >= count


## Takes `count` from slot i (default all). Returns the taken stack.
func take(i: int, count: int = -1) -> Dictionary:
	var s: Dictionary = slots[i]
	if s.is_empty():
		return {}
	var n: int = s["count"] if count < 0 else mini(count, s["count"])
	var out := _mk(s["id"], n, float(s.get("spoil", 0.0)))
	s["count"] -= n
	if s["count"] <= 0:
		slots[i] = {}
	changed.emit(i)
	return out


## Puts a stack into slot i: fills if same id (up to max), swaps if different.
## Returns what is left in the "cursor" afterwards.
func put(i: int, stack: Dictionary) -> Dictionary:
	if stack.is_empty():
		return {}
	var s: Dictionary = slots[i]
	var sp := float(stack.get("spoil", 0.0))
	if s.is_empty():
		var max_stack := items.max_stack(stack["id"])
		var n := mini(stack["count"], max_stack)
		slots[i] = _mk(stack["id"], n, sp)
		changed.emit(i)
		var left: int = stack["count"] - n
		return {} if left <= 0 else _mk(stack["id"], left, sp)
	if s["id"] == stack["id"]:
		var max_stack := items.max_stack(stack["id"])
		var n := mini(stack["count"], max_stack - s["count"])
		merge_into(s, n, sp)
		changed.emit(i)
		var left: int = stack["count"] - n
		return {} if left <= 0 else _mk(stack["id"], left, sp)
	# different item: swap
	slots[i] = _mk(stack["id"], stack["count"], sp)
	changed.emit(i)
	return s


# ---------------- cursor interactions (Minecraft semantics) ----------------

## Left click with `cursor` on slot i. Returns the new cursor.
func left_click(i: int, cursor: Dictionary) -> Dictionary:
	if cursor.is_empty():
		return take(i)
	return put(i, cursor)


## Right click: empty cursor picks up half (rounded up); cursor with items places
## one (a voxel item: one recipe unit, a block's worth of voxels, ADR 0034).
func right_click(i: int, cursor: Dictionary) -> Dictionary:
	if cursor.is_empty():
		var s: Dictionary = slots[i]
		if s.is_empty():
			return {}
		return take(i, int(ceil(s["count"] / 2.0)))
	var n := mini(items.units(cursor["id"]), int(cursor["count"]))
	var one := _mk(cursor["id"], n, float(cursor.get("spoil", 0.0)))
	var left := put(i, one)
	if left.is_empty() or left["id"] == cursor["id"]:
		cursor["count"] -= n - int(left.get("count", 0))
		return {} if cursor["count"] <= 0 else cursor
	if left["id"] != cursor["id"]:
		# swapped with a different stack: Minecraft keeps the swapped stack in the cursor
		# only on left click; on right click nothing happens. Undo the swap.
		slots[i] = _mk(left["id"], left["count"], float(left.get("spoil", 0.0)))
		changed.emit(i)
	return cursor


## Shift click: move the whole stack from slot i into `other`, merging first.
## Returns true when anything moved.
func shift_click(i: int, other: Inventory) -> bool:
	var s: Dictionary = slots[i]
	if s.is_empty():
		return false
	var left := other.add(s["id"], s["count"], float(s.get("spoil", 0.0)))
	if left == s["count"]:
		return false
	if left <= 0:
		slots[i] = {}
	else:
		s["count"] = left
	changed.emit(i)
	return true


func to_array() -> Array:
	return slots.duplicate(true)
