class_name PlayerInventory
extends RefCounted

## The player's Minecraft-exact inventory model (ADR 0005): one 36-slot
## Inventory where slots 0..8 are the hotbar and 9..35 the main grid, a
## cursor stack for the inventory screen, a personal 2x2 crafting grid, the
## selected hotbar index and health in half-hearts. Pure logic, no nodes.
## A worn bag (ADR 0030, Vintage Story's backpack) adds BAG slots of its own:
## pickups that do not fit go there, and the bag comes off only when empty.
## ADR 0055 (bug reports 2026-10-25): an offhand slot (F swaps it with the held stack; work pieces
## use the tool in it), a trash slot that keeps the last stack thrown away until the next one, and
## drops from the screen (a click outside the panel, Q over a slot).

signal changed
signal selection_changed(index: int)
signal health_changed(health: int)
signal tool_broke(id: String)          # a tool wore out in the hand or the grid (ADR 0042)
signal tool_low(id: String, left: int) # the held tool is nearly worn out (warning sound, plan item D11)
signal refilled(id: String)            # the hand ran out and the same item moved into it (plan item D11)

const HOTBAR := 9
const MAIN := 27
const SIZE := HOTBAR + MAIN
const BAG := 9

var items: ItemRegistry
var recipes: RecipeRegistry
var inv: Inventory
var craft: CraftingGrid
var cursor := {}
var bag_item := {}                 # the worn bag stack (leather_bag), or {}
var bag: Inventory                 # its slots; usable while bag_item is set
var container: RefCounted = null   # ChestEntity / FurnaceEntity while its screen is open (ADR 0007)
var off: Inventory                 # the offhand: one slot (ADR 0055)
var trash := {}                    # the last stack thrown in the trash slot; the next one replaces it
var favourites := {}               # player slot -> true: sort, quick-stack, shift-click and refill leave it alone (D11)
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
	inv.changed.connect(_on_slot_changed)
	craft.grid.changed.connect(_on_slot_changed)
	bag = Inventory.new(items, BAG)
	bag.changed.connect(_on_slot_changed)
	off = Inventory.new(items, 1)
	off.changed.connect(_on_slot_changed)


## Swaps in a bigger grid (crafting table: width 3) or back to the personal 2x2.
## Contents of the old grid return to the inventory; leftovers are returned.
func set_crafting_grid(width: int, station: String) -> Array:
	var dropped: Array = craft.return_contents(inv)
	craft = CraftingGrid.new(items, recipes, width, station)
	craft.grid.changed.connect(_on_slot_changed)
	changed.emit()
	return dropped


## Method callables, not lambdas: a lambda capturing self on a child's
## signal is a reference cycle and leaks at exit.
func _on_slot_changed(_i: int) -> void:
	changed.emit()


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


## Removes `count` from the held stack. Returns how many were removed. A hand that runs out takes the
## same item from the rest of the inventory (auto-refill).
func consume_held(count: int = 1) -> int:
	var id: String = held().get("id", "")
	var t := inv.take(selected, count)
	if inv.is_empty_slot(selected):
		refill_hand(id)
	return int(t.get("count", 0))


## Wears the held tool by `uses` (ADR 0042); at its durability it breaks and the hand empties (and
## takes another of the same tool, if there is one). Returns true when it broke.
func wear_held(uses: int = 1) -> bool:
	var before := Inventory.uses_left(items, held())
	var broke := inv.wear_slot(selected, uses)
	if broke != "":
		tool_broke.emit(broke)
		refill_hand(broke)
	else:
		_warn_low(held(), before)
	return broke != ""


## Warns once when a tool crosses into its last tenth (at least its last 3 uses).
func _warn_low(s: Dictionary, before: int) -> void:
	var left := Inventory.uses_left(items, s)
	if left < 0:
		return
	var dur := int(items.get_def(s["id"]).get("durability", 0))
	var low := maxi(3, dur / 10)
	if left <= low and before > low:
		tool_low.emit(s["id"], left)


## The empty hand takes a stack of `id` from the main grid (else another hotbar slot); favourites
## stay where they are. True when it found one.
func refill_hand(id: String) -> bool:
	if id == "" or not inv.is_empty_slot(selected):
		return false
	for i in range(HOTBAR, SIZE) + range(0, HOTBAR):
		if i == selected or favourites.has(i):
			continue
		var s := inv.get_slot(i)
		if not s.is_empty() and s["id"] == id:
			inv.set_slot(selected, s.duplicate(true))
			inv.set_slot(i, {})
			refilled.emit(id)
			return true
	return false


# ---------------- the offhand (ADR 0055) ----------------

func offhand() -> Dictionary:
	return off.get_slot(0)


func offhand_def() -> Dictionary:
	var h := offhand()
	return items.get_def(h["id"]) if not h.is_empty() else {}


## F: the held stack and the offhand stack change places, per-stack data and all.
func swap_hands() -> void:
	var a := inv.get_slot(selected).duplicate(true)
	var b := off.get_slot(0).duplicate(true)
	if a.is_empty() and b.is_empty():
		return
	inv.set_slot(selected, b)
	off.set_slot(0, a)


## Removes `count` from the offhand stack. Returns how many were removed.
func consume_offhand(count: int = 1) -> int:
	return int(off.take(0, count).get("count", 0))


## Wears the offhand tool (a hammer smithing from the offhand). Returns true when it broke.
func wear_offhand(uses: int = 1) -> bool:
	var broke := off.wear_slot(0, uses)
	if broke != "":
		tool_broke.emit(broke)
	return broke != ""


## Click on the offhand slot of the screen: like a main slot; shift moves it into the inventory.
func click_offhand(button: int, shift: bool) -> void:
	if shift:
		_move_stack(off, 0, inv, _player_order(false))
		return
	if button == MOUSE_BUTTON_RIGHT:
		cursor = off.right_click(0, cursor)
	else:
		cursor = off.left_click(0, cursor)
	changed.emit()


# ---------------- trash and drops from the screen (ADR 0055) ----------------

## Click on the trash slot: a stack on the cursor goes in (the one there before is gone for good);
## with an empty cursor the last trashed stack comes back.
func click_trash() -> void:
	if cursor.is_empty():
		cursor = trash
		trash = {}
	else:
		trash = cursor
		cursor = {}
	changed.emit()


## The cursor stack leaves the screen (a click outside the panel): all of it, or one item (one
## block's worth of a voxel item). Returns the stack to drop in the world.
func drop_cursor(whole: bool) -> Dictionary:
	if cursor.is_empty():
		return {}
	var n: int = int(cursor["count"]) if whole else mini(items.units(cursor["id"]), int(cursor["count"]))
	var out := Inventory.make_stack(items, cursor["id"], n, cursor)
	cursor["count"] = int(cursor["count"]) - n
	if int(cursor["count"]) <= 0:
		cursor = {}
	changed.emit()
	return out


## Q over a slot of the screen: one item (Ctrl: the stack) out of it. Returns the stack to drop.
func drop_from(group: String, i: int, whole: bool) -> Dictionary:
	var src := _group(group)
	if src == null or i < 0 or i >= src.size() or src.is_empty_slot(i):
		return {}
	var s := src.get_slot(i)
	var out := src.take(i, -1 if whole else mini(items.units(s["id"]), int(s["count"])))
	changed.emit()
	return out


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


# ---------------- pick block / creative picker (ADR 0018) ----------------

## Middle click on a block (vanilla pick block). A hotbar slot holding `id` is
## selected. Survival swaps a stack from the main grid into the selected slot;
## creative conjures a full stack into the selected slot if empty, else the
## first empty hotbar slot, else over the held stack. Returns true on change.
func pick_block(id: String, creative: bool) -> bool:
	for i in HOTBAR:
		var s: Dictionary = inv.get_slot(i)
		if not s.is_empty() and s["id"] == id:
			select(i)
			return true
	if not creative:
		for i in range(HOTBAR, SIZE):
			var s: Dictionary = inv.get_slot(i)
			if not s.is_empty() and s["id"] == id:
				var h := held()
				inv.set_slot(selected, s)
				inv.set_slot(i, h)
				return true
		return false
	if not items.has(id):
		return false
	var slot := selected
	if not inv.is_empty_slot(slot):
		for i in HOTBAR:
			if inv.is_empty_slot(i):
				slot = i
				break
	inv.set_slot(slot, {"id": id, "count": items.max_stack(id)})
	select(slot)
	return true


## Creative item picker (JEI cheat mode, the recipe browser's item list): a
## full stack onto an empty cursor, or straight into the inventory with
## `into_inventory`. With a stack on the cursor the click deletes it instead
## (vanilla's creative inventory trashes what is dropped on the item list).
func creative_take(id: String, into_inventory: bool) -> bool:
	if not cursor.is_empty():
		cursor = {}
		changed.emit()
		return true
	if not items.has(id):
		return false
	var n := items.max_stack(id)
	if into_inventory:
		pickup(id, n)
	else:
		cursor = {"id": id, "count": n}
	changed.emit()
	return true


# ---------------- pick up / drop ----------------

## Picks up a stack: merges into existing stacks first, then fills empty
## slots hotbar-first (vanilla order). Returns the leftover that did not fit.
func pickup(id: String, count: int, data: Dictionary = {}) -> int:
	var left := _add_in_range(id, count, 0, SIZE, true, data)
	if left > 0:
		left = _add_in_range(id, left, 0, SIZE, false, data)
	if left > 0 and has_bag():
		left = bag.add(id, left, data)
	return left


# ---------------- the bag (ADR 0030) ----------------

func has_bag() -> bool:
	return not bag_item.is_empty()


func bag_is_empty() -> bool:
	for i in BAG:
		if not bag.is_empty_slot(i):
			return false
	return true


## Click on the bag slot: put a bag on from the cursor, or take an empty one off.
func click_bag_item() -> void:
	var cursor_bag: bool = not cursor.is_empty() and items.has_tag(cursor["id"], "bag")
	if not cursor.is_empty() and not cursor_bag:
		return
	if has_bag() and not bag_is_empty():
		return   # a bag with things in it stays on
	var was := bag_item
	bag_item = {"id": cursor["id"], "count": 1} if cursor_bag else {}
	if cursor_bag:
		cursor["count"] = int(cursor["count"]) - 1
		cursor = cursor if cursor["count"] > 0 else {}
	if not was.is_empty():
		if cursor.is_empty():
			cursor = was
		else:
			pickup(was["id"], 1)
	changed.emit()


## Click on one of the bag's slots: like a main slot; shift moves it into the inventory.
func click_bag_slot(i: int, button: int, shift: bool) -> void:
	if not has_bag():
		return
	if shift:
		_move_stack(bag, i, inv, _player_order(false))
		return
	if button == MOUSE_BUTTON_RIGHT:
		cursor = bag.right_click(i, cursor)
	else:
		cursor = bag.left_click(i, cursor)
	changed.emit()


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
		if favourites.has(i):
			return   # a favourite slot stays put (plan item D11)
		var s: Dictionary = inv.get_slot(i)
		if container != null and not s.is_empty():
			var targets: Array = container.shift_targets(s["id"])
			if not targets.is_empty():
				_move_stack(inv, i, container.inv, targets)
				return
			if container.kind == "chest" or container.kind == "storage_pot":
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
		_report_broken()
		changed.emit()
		return
	if cursor.is_empty():
		cursor = craft.craft_once()
	elif cursor["id"] == out["id"] and cursor["count"] + out["count"] <= items.max_stack(out["id"]):
		var c := craft.craft_once()
		cursor["count"] += c["count"]
	_report_broken()
	changed.emit()


## The inventories the grid fills from and returns to: the player's slots, then the worn bag.
func craft_sources() -> Array:
	return [inv, bag] if has_bag() else [inv]


## The recipe browser's Fill button (bug report 2026-10-25): lays recipe r out in the grid from the
## inventory, once or (all) as many times as it allows. Returns CraftingGrid.fill's answer.
func fill_craft_grid(r: Dictionary, all: bool, dry_run := false) -> Dictionary:
	var res := craft.fill(r, craft_sources(), 1 << 20 if all else 1, dry_run)
	if not dry_run:
		changed.emit()
	return res


## The panel's Clear button: the grid's contents back into the inventory. False when some did not fit.
func clear_craft_grid() -> bool:
	var ok := craft.return_to(craft_sources())
	changed.emit()
	return ok


func _report_broken() -> void:
	for id in craft.tools_broken:
		tool_broke.emit(id)
	craft.tools_broken.clear()


# ---------------- sorting, quick-stack, favourites (plan item D11) ----------------

## Alt + click: the slot becomes a favourite (or stops being one).
func toggle_favourite(i: int) -> void:
	if favourites.has(i):
		favourites.erase(i)
	else:
		favourites[i] = true
	changed.emit()


## Sorts slots [from, to) of `target` (favourites of the player's own slots stay): equal stacks merge,
## then by item id, fullest first. Per-stack data rides along; stacks that differ in it (worn tools,
## work pieces) stay apart because they stack to 1.
func sort_slots(target: Inventory, from: int, to: int, skip_favourites: bool) -> void:
	var idx: Array[int] = []
	var stacks: Array[Dictionary] = []
	for i in range(from, to):
		if skip_favourites and favourites.has(i):
			continue
		idx.append(i)
		var s := target.get_slot(i)
		if not s.is_empty():
			stacks.append(s.duplicate(true))
	# merge equal items (count-weighted data, Inventory.merge_into), then order
	var merged: Array[Dictionary] = []
	for s in stacks:
		var left := int(s["count"])
		var max_stack := items.max_stack(s["id"])
		for m in merged:
			if left <= 0:
				break
			if m["id"] == s["id"] and int(m["count"]) < max_stack:   # tools and work pieces stack to 1: never here
				var n := mini(left, max_stack - int(m["count"]))
				Inventory.merge_into(m, n, s)
				left -= n
		if left > 0:
			merged.append(Inventory.make_stack(items, s["id"], left, s))
	merged.sort_custom(func(a: Dictionary, b: Dictionary) -> bool:
		if a["id"] != b["id"]:
			return String(a["id"]) < String(b["id"])
		return int(a["count"]) > int(b["count"]))
	for k in idx.size():
		target.set_slot(idx[k], merged[k] if k < merged.size() else {})
	changed.emit()


## The main grid (not the hotbar, not favourites) sorted.
func sort_inventory() -> void:
	sort_slots(inv, HOTBAR, SIZE, true)


## The open chest sorted.
func sort_container() -> void:
	if container != null:
		sort_slots(container.inv, 0, container.inv.size(), false)


## Quick-stack (Terraria, Minecraft mods): every main-grid stack (not favourites, not the hotbar) whose
## item a chest in `chests` already holds goes into that chest, as much as fits. Returns items moved.
func quick_stack(chests: Array) -> int:
	var moved := 0
	for i in range(HOTBAR, SIZE):
		if favourites.has(i) or inv.is_empty_slot(i):
			continue
		var id: String = inv.get_slot(i)["id"]
		for c in chests:
			var c_inv: Inventory = c.inv
			if c_inv.count_of(id) == 0:
				continue
			var before := int(inv.get_slot(i).get("count", 0))
			_move_stack(inv, i, c_inv, range(0, c_inv.size()))
			moved += before - int(inv.get_slot(i).get("count", 0))
			if inv.is_empty_slot(i):
				break
	changed.emit()
	return moved


# ---------------- Mouse Tweaks (the Minecraft mod) ----------------
# Slots are addressed by group: "inv" (the 36 player slots), "bag", "craft"
# (the crafting grid inputs) and "container" (the open chest or furnace).

func _group(group: String) -> Inventory:
	match group:
		"inv":
			return inv
		"bag":
			return bag if has_bag() else null
		"craft":
			return craft.grid
		"offhand":
			return off
		"container":
			return container.inv if container != null else null
	return null


## Right button dragged over a slot with a stack on the cursor: one item goes
## into it if it is empty or holds the same item with room (vanilla's RMB drag).
func drag_place_one(group: String, i: int) -> void:
	var dst := _group(group)
	if dst == null or cursor.is_empty():
		return
	if group == "container" and not container.may_place(i, cursor["id"]):
		return
	var s := dst.get_slot(i)
	if not s.is_empty() and (s["id"] != cursor["id"] or int(s["count"]) >= items.max_stack(s["id"])):
		return
	cursor = dst.right_click(i, cursor)
	changed.emit()


## Left button dragged over a slot with a stack on the cursor: that slot's
## items of the same kind join the cursor, as many as fit.
func drag_collect(group: String, i: int) -> void:
	var src := _group(group)
	if src == null or cursor.is_empty():
		return
	var s := src.get_slot(i)
	if s.is_empty() or s["id"] != cursor["id"]:
		return
	var room: int = items.max_stack(cursor["id"]) - int(cursor["count"])
	if room <= 0:
		return
	var t := src.take(i, room)
	cursor["count"] = int(cursor["count"]) + int(t.get("count", 0))
	changed.emit()


## Shift held while dragging over slots: each one is shift-clicked.
func drag_shift(group: String, i: int) -> void:
	match group:
		"inv":
			click_slot(i, MOUSE_BUTTON_LEFT, true)
		"bag":
			click_bag_slot(i, MOUSE_BUTTON_LEFT, true)
		"craft":
			click_craft_slot(i, MOUSE_BUTTON_LEFT, true)
		"offhand":
			click_offhand(MOUSE_BUTTON_LEFT, true)
		"container":
			click_container_slot(i, MOUSE_BUTTON_LEFT, true)


## Where a slot's items go on a shift-click: [inventory, slot order], or [] for nowhere.
func _other_side(group: String, i: int, id: String) -> Array:
	if group == "inv":
		if container != null:
			var targets: Array = container.shift_targets(id)
			if not targets.is_empty():
				return [container.inv, targets]
			if container.kind == "chest" or container.kind == "storage_pot":
				return []
		return [inv, range(HOTBAR, SIZE) if i < HOTBAR else range(0, HOTBAR)]
	if group == "container":
		return [inv, _player_order(container.shift_out_reverse(i))]
	return [inv, _player_order(false)]


## Scroll wheel over a slot: down moves one item from it to where a shift-click
## would send it; up pulls one item of the same kind from there into it.
func wheel_move(group: String, i: int, down: bool) -> void:
	var here := _group(group)
	if here == null:
		return
	var s := here.get_slot(i)
	if s.is_empty():
		return
	var side := _other_side(group, i, s["id"])
	if side.is_empty():
		return
	var there: Inventory = side[0]
	if down:
		_move_count(here, i, there, side[1], 1)
		return
	if int(s["count"]) >= items.max_stack(s["id"]):
		return
	if group == "container" and not container.may_place(i, s["id"]):
		return
	for j in side[1]:
		if there == here and j == i:
			continue
		var d := there.get_slot(j)
		if not d.is_empty() and d["id"] == s["id"]:
			var one := there.take(j, 1)
			var merged := s.duplicate()
			Inventory.merge_into(merged, 1, one)
			here.set_slot(i, merged)
			changed.emit()
			return


## Closing the screen: grid contents go back, cursor too. Returns stacks
## that did not fit (to drop in the world).
func close_screen() -> Array:
	container = null
	var dropped: Array = craft.return_contents(inv)
	if not cursor.is_empty():
		var sp := Inventory.data_of(cursor)
		var left := inv.add(cursor["id"], cursor["count"], sp)
		if left > 0 and has_bag():
			left = bag.add(cursor["id"], left, sp)
		if left > 0:
			var rest := cursor.duplicate()
			rest["count"] = left
			dropped.append(rest)
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
func _add_in_range(id: String, count: int, from: int, to: int, merge_only: bool, data: Dictionary = {}) -> int:
	var max_stack := items.max_stack(id)
	for i in range(from, to):
		if count <= 0:
			break
		var s: Dictionary = inv.get_slot(i)
		if merge_only:
			if not s.is_empty() and s["id"] == id and s["count"] < max_stack:
				var take := mini(count, max_stack - s["count"])
				var merged := s.duplicate()
				Inventory.merge_into(merged, take, data)
				inv.set_slot(i, merged)
				count -= take
		elif s.is_empty():
			var take := mini(count, max_stack)
			inv.set_slot(i, Inventory.make_stack(items, id, take, data))
			count -= take
	return count


## Shift-click: move slot i of `src` into player slots [from, to), merging first.
func _move_slot(src: Inventory, i: int, from: int, to: int) -> void:
	var s: Dictionary = src.get_slot(i)
	if s.is_empty():
		return
	var id: String = s["id"]
	var count: int = s["count"]
	var sp := Inventory.data_of(s)
	# take first so the source slot never counts as a merge target
	src.take(i, count)
	var left := _add_in_range(id, count, from, to, true, sp)
	if left > 0:
		left = _add_in_range(id, left, from, to, false, sp)
	if left > 0:
		src.put(i, Inventory.make_stack(items, id, left, sp))
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
	_move_count(src, i, dst, order, -1)


## _move_stack for at most n items of the slot (-1 = all of them).
func _move_count(src: Inventory, i: int, dst: Inventory, order: Array, n_max: int) -> void:
	var s: Dictionary = src.get_slot(i)
	if s.is_empty():
		return
	var id: String = s["id"]
	var count: int = int(s["count"]) if n_max < 0 else mini(n_max, int(s["count"]))
	var sp := Inventory.data_of(s)
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
				var merged := d.duplicate()
				Inventory.merge_into(merged, n, sp)
				dst.set_slot(j, merged)
				count -= n
			elif not pass_merge and d.is_empty():
				var n := mini(count, max_stack)
				dst.set_slot(j, Inventory.make_stack(items, id, n, sp))
				count -= n
	if count > 0:
		src.put(i, Inventory.make_stack(items, id, count, sp))
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
	var in_bag := {}
	for i in BAG:
		var s: Dictionary = bag.get_slot(i)
		if not s.is_empty():
			in_bag[s["id"]] = int(in_bag.get(s["id"], 0)) + int(s["count"])
	return {"selected": selected, "hotbar": hotbar, "main": main, "cursor": cursor, "health": health,
		"container": container.kind if container != null else "", "bag": bag_item.get("id", ""), "in_bag": in_bag,
		"offhand": offhand(), "trash": trash}
