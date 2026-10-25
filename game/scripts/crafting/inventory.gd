class_name Inventory
extends RefCounted

## Minecraft-style slot inventory. A slot is {id: String, count: int} or empty {}, plus per-stack data
## kept through every move:
## - spoil (ADR 0040): perishable food ("spoil_days"), 0 fresh .. 1 rotten, averaged by count on merge;
## - wear (ADR 0042): uses a tool ("durability") has spent; tools stack to 1, so wear never merges;
## - hot_until (ADR 0042): heatable metal ("heatable" tag): the clock_s when it is cold again, averaged
##   on merge; dropped once it has passed;
## - work (ADR 0042): a smithing work piece's metal cells and recipe (stack 1);
## - dur_x (ADR 0059): a durability multiplier (tar-hafted tools, heads of tempered flint), averaged on merge.
## Pure logic, no nodes. The UI drives it through the cursor operations below,
## which reproduce vanilla behaviour: left-click picks up / places a whole
## stack, right-click picks up half / places one, shift-click moves a stack
## to another inventory, merging into existing stacks first.

signal changed(slot: int)

## Play-time seconds (Main advances it and saves it with the player): hot metal cools against it.
static var clock_s := 0.0

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
	slots[i] = {} if stack.is_empty() or int(stack.get("count", 0)) <= 0 else _mk(stack["id"], int(stack["count"]), stack)
	changed.emit(i)


## A stack's per-stack data: everything but id and count.
static func data_of(s: Dictionary) -> Dictionary:
	var d := s.duplicate(true)
	d.erase("id")
	d.erase("count")
	return d


## A stack dictionary with the data that applies to `id` (keys of `data` other than id and count):
## spoil only on perishables (fresh by default), wear above 0, hot_until while still hot.
func _mk(id: String, count: int, data: Dictionary = {}) -> Dictionary:
	return make_stack(items, id, count, data)


static func make_stack(p_items: ItemRegistry, id: String, count: int, data: Dictionary = {}) -> Dictionary:
	var out := {"id": id, "count": count}
	for k in data:
		if k != "id" and k != "count":
			out[k] = data[k] if not (data[k] is Dictionary or data[k] is Array) else data[k].duplicate(true)
	var def := p_items.get_def(id)
	if def.has("spoil_days"):
		out["spoil"] = clampf(float(out.get("spoil", 0.0)), 0.0, 1.0)
	else:
		out.erase("spoil")
	if int(out.get("wear", 0)) <= 0 or not def.has("durability"):
		out.erase("wear")
	if float(out.get("hot_until", 0.0)) <= clock_s:
		out.erase("hot_until")
	return out


## Merges `count` items carrying `data` into stack s (same id): spoil and the time left hot become
## count-weighted means. Items whose data tells them apart (wear, work) stack to 1, so never get here.
static func merge_into(s: Dictionary, count: int, data: Dictionary = {}) -> void:
	var n := int(s["count"]) + count
	if s.has("spoil"):
		s["spoil"] = (float(s["spoil"]) * int(s["count"]) + float(data.get("spoil", 0.0)) * count) / maxf(n, 1)
	if s.has("hot_until") or data.has("hot_until"):
		var a := maxf(float(s.get("hot_until", 0.0)) - clock_s, 0.0)
		var b := maxf(float(data.get("hot_until", 0.0)) - clock_s, 0.0)
		var left := (a * int(s["count"]) + b * count) / maxf(n, 1)
		if left > 0.0:
			s["hot_until"] = clock_s + left
		else:
			s.erase("hot_until")
	if s.has("dur_x") or data.has("dur_x"):
		s["dur_x"] = (float(s.get("dur_x", 1.0)) * (n - count) + float(data.get("dur_x", 1.0)) * count) / maxf(n, 1)
	s["count"] = n


## A tool stack's durability in uses: its item's durability times the stack's dur_x (ADR 0059), 0 for none.
static func durability_of(p_items: ItemRegistry, s: Dictionary) -> int:
	var dur := int(p_items.get_def(s.get("id", "")).get("durability", 0))
	return int(round(dur * float(s.get("dur_x", 1.0)))) if dur > 0 else 0


## A tool stack after `uses` more uses (ADR 0042): wear grows; at the item's durability the stack comes
## back with broken = true (the caller empties the slot). Items without durability are returned as they are.
static func wear_stack(p_items: ItemRegistry, s: Dictionary, uses: int = 1) -> Dictionary:
	var dur := durability_of(p_items, s)
	if s.is_empty() or dur <= 0 or uses <= 0:
		return s
	var out := s.duplicate(true)
	out["wear"] = int(s.get("wear", 0)) + uses
	if int(out["wear"]) >= dur:
		out["broken"] = true
	return out


## Wears the tool in slot i by `uses`; a tool at its durability breaks and the slot empties.
## Returns the id of the tool that broke, or "".
func wear_slot(i: int, uses: int = 1) -> String:
	var s: Dictionary = slots[i]
	if s.is_empty():
		return ""
	var w := wear_stack(items, s, uses)
	if w.get("broken", false):
		slots[i] = {}
		changed.emit(i)
		return s["id"]
	if w != s:
		slots[i] = w
		changed.emit(i)
	return ""


## Uses left of a tool stack (its durability minus wear), or -1 for items without durability.
static func uses_left(p_items: ItemRegistry, s: Dictionary) -> int:
	var dur := durability_of(p_items, s)
	return dur - int(s.get("wear", 0)) if dur > 0 else -1


## How hot a stack is: 1 just out of the fire .. 0 cold (Settings.metal_cool_s from 1 to 0).
static func heat_of(s: Dictionary) -> float:
	return clampf((float(s.get("hot_until", 0.0)) - clock_s) / Settings.metal_cool_s, 0.0, 1.0)


func count_of(id: String) -> int:
	var n := 0
	for s in slots:
		if not s.is_empty() and s["id"] == id:
			n += s["count"]
	return n


## Adds count of id carrying `data` (perishable: fresh by default), merging into existing stacks first,
## then empty slots. Returns the leftover that did not fit.
func add(id: String, count: int, data: Dictionary = {}) -> int:
	var max_stack := items.max_stack(id)
	for i in slots.size():
		if count <= 0:
			break
		var s: Dictionary = slots[i]
		if not s.is_empty() and s["id"] == id and s["count"] < max_stack:
			var take := mini(count, max_stack - s["count"])
			merge_into(s, take, data)
			count -= take
			changed.emit(i)
	for i in slots.size():
		if count <= 0:
			break
		if slots[i].is_empty():
			var take := mini(count, max_stack)
			slots[i] = _mk(id, take, data)
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
	var out := _mk(s["id"], n, s)
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
	var sp := data_of(stack)
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
	var one := _mk(cursor["id"], n, cursor)
	var left := put(i, one)
	if left.is_empty() or left["id"] == cursor["id"]:
		cursor["count"] -= n - int(left.get("count", 0))
		return {} if cursor["count"] <= 0 else cursor
	if left["id"] != cursor["id"]:
		# swapped with a different stack: Minecraft keeps the swapped stack in the cursor
		# only on left click; on right click nothing happens. Undo the swap.
		slots[i] = _mk(left["id"], left["count"], left)
		changed.emit(i)
	return cursor


## Shift click: move the whole stack from slot i into `other`, merging first.
## Returns true when anything moved.
func shift_click(i: int, other: Inventory) -> bool:
	var s: Dictionary = slots[i]
	if s.is_empty():
		return false
	var left := other.add(s["id"], s["count"], data_of(s))
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
