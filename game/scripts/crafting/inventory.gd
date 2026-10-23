class_name Inventory
extends RefCounted

## Minecraft-style slot inventory. A slot is {id: String, count: int} or empty {}.
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
	slots[i] = {} if stack.is_empty() or int(stack.get("count", 0)) <= 0 else {"id": stack["id"], "count": int(stack["count"])}
	changed.emit(i)


func count_of(id: String) -> int:
	var n := 0
	for s in slots:
		if not s.is_empty() and s["id"] == id:
			n += s["count"]
	return n


## Adds count of id, merging into existing stacks first, then empty slots.
## Returns the leftover that did not fit.
func add(id: String, count: int) -> int:
	var max_stack := items.max_stack(id)
	for i in slots.size():
		if count <= 0:
			break
		var s: Dictionary = slots[i]
		if not s.is_empty() and s["id"] == id and s["count"] < max_stack:
			var take := mini(count, max_stack - s["count"])
			s["count"] += take
			count -= take
			changed.emit(i)
	for i in slots.size():
		if count <= 0:
			break
		if slots[i].is_empty():
			var take := mini(count, max_stack)
			slots[i] = {"id": id, "count": take}
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
	var out := {"id": s["id"], "count": n}
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
	if s.is_empty():
		var max_stack := items.max_stack(stack["id"])
		var n := mini(stack["count"], max_stack)
		slots[i] = {"id": stack["id"], "count": n}
		changed.emit(i)
		var left: int = stack["count"] - n
		return {} if left <= 0 else {"id": stack["id"], "count": left}
	if s["id"] == stack["id"]:
		var max_stack := items.max_stack(stack["id"])
		var n := mini(stack["count"], max_stack - s["count"])
		s["count"] += n
		changed.emit(i)
		var left: int = stack["count"] - n
		return {} if left <= 0 else {"id": stack["id"], "count": left}
	# different item: swap
	slots[i] = {"id": stack["id"], "count": stack["count"]}
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
	var one := {"id": cursor["id"], "count": n}
	var left := put(i, one)
	if left.is_empty() or left["id"] == cursor["id"]:
		cursor["count"] -= n - int(left.get("count", 0))
		return {} if cursor["count"] <= 0 else cursor
	if left["id"] != cursor["id"]:
		# swapped with a different stack: Minecraft keeps the swapped stack in the cursor
		# only on left click; on right click nothing happens. Undo the swap.
		slots[i] = {"id": left["id"], "count": left["count"]}
		changed.emit(i)
	return cursor


## Shift click: move the whole stack from slot i into `other`, merging first.
## Returns true when anything moved.
func shift_click(i: int, other: Inventory) -> bool:
	var s: Dictionary = slots[i]
	if s.is_empty():
		return false
	var left := other.add(s["id"], s["count"])
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
