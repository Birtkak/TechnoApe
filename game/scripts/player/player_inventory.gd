class_name PlayerInventory
extends RefCounted

## The player's Minecraft-exact inventory model (ADR 0005): one 36-slot
## Inventory where slots 0..8 are the hotbar and 9..35 the main grid, a
## cursor stack for the inventory screen, a personal 2x2 crafting grid, the
## selected hotbar index and health in half-hearts. Pure logic, no nodes.

signal changed
signal selection_changed(index: int)
signal health_changed(health: int)

const HOTBAR := 9
const MAIN := 27
const SIZE := HOTBAR + MAIN

var items: ItemRegistry
var recipes: RecipeRegistry
var inv: Inventory
var craft: CraftingGrid
var cursor := {}
var container: RefCounted = null   # ChestEntity / FurnaceEntity while its screen is open (ADR 0007)
var selected := 0
var max_health := 20
var health := 20


func _init(p_items: ItemRegistry, p_recipes: RecipeRegistry, p_max_health: int = 20) -> void:
	items = p_items
	recipes = p_recipes
	inv = Inventory.new(items, SIZE)
	craft = CraftingGrid.new(items, recipes, 2, "")
	max_health = p_max_health
	health = p_max_health
	inv.changed.connect(func(_i: int) -> void: changed.emit())
	craft.grid.changed.connect(func(_i: int) -> void: changed.emit())


## Swaps in a bigger grid (crafting table: width 3) or back to the personal 2x2.
## Contents of the old grid return to the inventory; leftovers are returned.
func set_crafting_grid(width: int, station: String) -> Array:
	var dropped: Array = craft.return_contents(inv)
	craft = CraftingGrid.new(items, recipes, width, station)
	craft.grid.changed.connect(func(_i: int) -> void: changed.emit())
	changed.emit()
	return dropped


# ---------------- hotbar ----------------

func select(index: int) -> void:
	index = posmod(index, HOTBAR)
	if index == selected:
		return
	selected = index
	selection_changed.emit(selected)


## Mouse wheel: +1 selects the next slot to the right (wraps), -1 the previous.
func scroll(direction: int) -> void:
	select(selected + direction)


func held() -> Dictionary:
	return inv.get_slot(selected)


func held_def() -> Dictionary:
	var h := held()
	return items.get_def(h["id"]) if not h.is_empty() else {}


## Removes `count` from the held stack. Returns how many were removed.
func consume_held(count: int = 1) -> int:
	var t := inv.take(selected, count)
	return int(t.get("count", 0))


## Replaces one of the held items with another one (bucket -> water bucket).
## The rest of the old stack stays; the new item goes to the hand when the
## slot frees up, else into the inventory. Returns the leftover (to drop).
func replace_held_one(new_id: String) -> int:
	var h := held()
	if h.is_empty():
		return 0
	inv.take(selected, 1)
	if inv.is_empty_slot(selected):
		inv.set_slot(selected, {"id": new_id, "count": 1})
		return 0
	return pickup(new_id, 1)


# ---------------- pick up / drop ----------------

## Picks up a stack: merges into existing stacks first, then fills empty
## slots hotbar-first (vanilla order). Returns the leftover that did not fit.
func pickup(id: String, count: int) -> int:
	var left := _add_in_range(id, count, 0, SIZE, true)
	if left > 0:
		left = _add_in_range(id, left, 0, SIZE, false)
	return left


## Drops from the held stack (all when `whole` else one). Returns the dropped stack.
func drop_held(whole: bool = false) -> Dictionary:
	var h := held()
	if h.is_empty():
		return {}
	return inv.take(selected, -1 if whole else 1)


# ---------------- inventory screen: cursor operations ----------------

## Click on one of the 36 player slots. button: MOUSE_BUTTON_LEFT/RIGHT.
## Shift with a container open moves the stack into it (vanilla
## quickMoveStack); the furnace takes only smeltables and fuel and refuses
## the rest without falling back, everything else moves hotbar <-> main.
func click_slot(i: int, button: int, shift: bool) -> void:
	if shift:
		var s: Dictionary = inv.get_slot(i)
		if container != null and not s.is_empty():
			var targets: Array = container.shift_targets(s["id"])
			if not targets.is_empty():
				_move_stack(inv, i, container.inv, targets)
				return
			if container.kind == "chest":
				return
		if i < HOTBAR:
			_move_slot(inv, i, HOTBAR, SIZE)
		else:
			_move_slot(inv, i, 0, HOTBAR)
		return
	if button == MOUSE_BUTTON_RIGHT:
		cursor = inv.right_click(i, cursor)
	else:
		cursor = inv.left_click(i, cursor)
	changed.emit()


## Opens a container screen (chest, furnace). The personal grid stays 2x2.
func open_container(c: RefCounted) -> void:
	container = c
	changed.emit()


## Click on a container slot. Slots that refuse the cursor's item (furnace
## output, fuel slot with a non-fuel) only hand items out: vanilla moves as
## much as fits into a matching cursor and ignores a different one.
func click_container_slot(i: int, button: int, shift: bool) -> void:
	if container == null:
		return
	var c_inv: Inventory = container.inv
	if shift:
		_move_stack(c_inv, i, inv, _player_order(container.shift_out_reverse(i)))
		return
	var s: Dictionary = c_inv.get_slot(i)
	if not cursor.is_empty() and not container.may_place(i, cursor["id"]):
		if not s.is_empty() and s["id"] == cursor["id"]:
			var room: int = items.max_stack(cursor["id"]) - int(cursor["count"])
			if room > 0:
				var t := c_inv.take(i, room)
				cursor["count"] += int(t.get("count", 0))
		changed.emit()
		return
	if button == MOUSE_BUTTON_RIGHT:
		cursor = c_inv.right_click(i, cursor)
	else:
		cursor = c_inv.left_click(i, cursor)
	changed.emit()


## Click on one of the crafting grid's input slots.
func click_craft_slot(i: int, button: int, shift: bool) -> void:
	if shift:
		_move_slot(craft.grid, i, 0, SIZE)
		return
	if button == MOUSE_BUTTON_RIGHT:
		cursor = craft.grid.right_click(i, cursor)
	else:
		cursor = craft.grid.left_click(i, cursor)
	changed.emit()


## Click on the crafting output: left takes one result into the cursor (if
## it stacks), shift crafts everything straight into the inventory.
func click_craft_output(shift: bool) -> void:
	var out := craft.preview_output()
	if out.is_empty():
		return
	if shift:
		var total := craft.craft_all()
		if not total.is_empty():
			var left := pickup(total["id"], total["count"])
			if left > 0:
				# inventory full: the rest waits in the cursor like vanilla would refuse; keep it visible
				cursor = {"id": total["id"], "count": left} if cursor.is_empty() else cursor
		changed.emit()
		return
	if cursor.is_empty():
		cursor = craft.craft_once()
	elif cursor["id"] == out["id"] and cursor["count"] + out["count"] <= items.max_stack(out["id"]):
		var c := craft.craft_once()
		cursor["count"] += c["count"]
	changed.emit()


## Closing the screen: grid contents go back, cursor too. Returns stacks
## that did not fit (to drop in the world).
func close_screen() -> Array:
	container = null
	var dropped: Array = craft.return_contents(inv)
	if not cursor.is_empty():
		var left := inv.add(cursor["id"], cursor["count"])
		if left > 0:
			dropped.append({"id": cursor["id"], "count": left})
		cursor = {}
	changed.emit()
	return dropped


# ---------------- health ----------------

func damage(amount: int) -> void:
	if amount <= 0:
		return
	health = maxi(0, health - amount)
	health_changed.emit(health)


func heal(amount: int) -> void:
	health = mini(max_health, health + amount)
	health_changed.emit(health)


func is_dead() -> bool:
	return health <= 0


func respawn() -> void:
	health = max_health
	health_changed.emit(health)


# ---------------- helpers ----------------

## Adds into slots [from, to): merge pass (into same-id stacks) or fill pass (empties).
func _add_in_range(id: String, count: int, from: int, to: int, merge_only: bool) -> int:
	var max_stack := items.max_stack(id)
	for i in range(from, to):
		if count <= 0:
			break
		var s: Dictionary = inv.get_slot(i)
		if merge_only:
			if not s.is_empty() and s["id"] == id and s["count"] < max_stack:
				var take := mini(count, max_stack - s["count"])
				inv.set_slot(i, {"id": id, "count": s["count"] + take})
				count -= take
		elif s.is_empty():
			var take := mini(count, max_stack)
			inv.set_slot(i, {"id": id, "count": take})
			count -= take
	return count


## Shift-click: move slot i of `src` into player slots [from, to), merging first.
func _move_slot(src: Inventory, i: int, from: int, to: int) -> void:
	var s: Dictionary = src.get_slot(i)
	if s.is_empty():
		return
	var id: String = s["id"]
	var count: int = s["count"]
	# take first so the source slot never counts as a merge target
	src.take(i, count)
	var left := _add_in_range(id, count, from, to, true)
	if left > 0:
		left = _add_in_range(id, left, from, to, false)
	if left > 0:
		src.put(i, {"id": id, "count": left})
	changed.emit()


## Player slot order for stacks coming out of a container: vanilla walks the
## menu's player slots (main 9..35 then hotbar 0..8), reversed for chests and
## the furnace output, so those fill the hotbar from slot 9 leftwards first.
func _player_order(reverse: bool) -> Array:
	var order := range(HOTBAR, SIZE) + range(0, HOTBAR)
	if reverse:
		order.reverse()
	return order


## Moves slot i of `src` into `dst` slots in `order`: merge pass, then empty
## pass (vanilla moveItemStackTo). What does not fit stays in the source.
func _move_stack(src: Inventory, i: int, dst: Inventory, order: Array) -> void:
	var s: Dictionary = src.get_slot(i)
	if s.is_empty():
		return
	var id: String = s["id"]
	var count: int = s["count"]
	var max_stack := items.max_stack(id)
	src.take(i, count)
	for pass_merge in [true, false]:
		for j in order:
			if count <= 0:
				break
			if dst == src and j == i:
				continue
			var d: Dictionary = dst.get_slot(j)
			if pass_merge and not d.is_empty() and d["id"] == id and d["count"] < max_stack:
				var n := mini(count, max_stack - int(d["count"]))
				dst.set_slot(j, {"id": id, "count": int(d["count"]) + n})
				count -= n
			elif not pass_merge and d.is_empty():
				var n := mini(count, max_stack)
				dst.set_slot(j, {"id": id, "count": n})
				count -= n
	if count > 0:
		src.put(i, {"id": id, "count": count})
	changed.emit()


## Serialisable summary for debug JSON.
func summary() -> Dictionary:
	var hotbar := []
	for i in HOTBAR:
		hotbar.append(inv.get_slot(i))
	var main := {}
	for i in range(HOTBAR, SIZE):
		var s: Dictionary = inv.get_slot(i)
		if not s.is_empty():
			main[s["id"]] = int(main.get(s["id"], 0)) + int(s["count"])
	return {"selected": selected, "hotbar": hotbar, "main": main, "cursor": cursor, "health": health,
		"container": container.kind if container != null else ""}
