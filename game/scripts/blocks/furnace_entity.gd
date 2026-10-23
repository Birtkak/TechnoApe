class_name FurnaceEntity
extends RefCounted

## A furnace (ADR 0007) with Minecraft's exact tick rules
## (AbstractFurnaceBlockEntity.serverTick): slots input / fuel / output, 20
## ticks per second, smelt recipes from recipes.json (type "smelt", station
## "furnace", 200 ticks), fuel times from items.json "fuel_ticks". Fuel is
## only lit when the input can smelt; a lava bucket leaves its empty bucket in
## the fuel slot; without fire the cook progress falls back 2 ticks per tick.
## Same container interface as ChestEntity.

signal changed

const INPUT := 0
const FUEL := 1
const OUTPUT := 2

var kind := "furnace"
var title := "Furnace"
var station := "furnace"
var pos: Vector3i
var inv: Inventory
var items: ItemRegistry
var recipes: RecipeRegistry
var lit_time := 0        # ticks of fire left
var lit_duration := 0    # ticks the current fuel item gave (for the flame gauge)
var cook := 0            # ticks cooked on the current item
var cook_total := 200
var smelted := 0         # items produced, for debug JSON
var _last_input := ""


func _init(p_items: ItemRegistry, p_recipes: RecipeRegistry, p_pos: Vector3i) -> void:
	items = p_items
	recipes = p_recipes
	pos = p_pos
	inv = Inventory.new(items, 3)
	inv.changed.connect(_on_slot_changed)


## Vanilla resets the cook progress when the input becomes a different item.
func _on_slot_changed(i: int) -> void:
	if i == INPUT:
		var id: String = inv.get_slot(INPUT).get("id", "")
		if id != _last_input:
			_last_input = id
			cook = 0
			var r := recipes.smelting_for(id, station)
			cook_total = int(r.get("ticks", 200)) if not r.is_empty() else 200
	changed.emit()


func fuel_ticks(id: String) -> int:
	if id == "" or not items.has(id):
		return 0
	return int(items.get_def(id).get("fuel_ticks", 0))


func is_fuel(id: String) -> bool:
	return fuel_ticks(id) > 0


func is_lit() -> bool:
	return lit_time > 0


func may_place(slot: int, id: String) -> bool:
	match slot:
		INPUT:
			return true
		FUEL:
			return is_fuel(id) or id == "bucket"
	return false


func is_output(slot: int) -> bool:
	return slot == OUTPUT


## Vanilla quick-move from the player: smeltable -> input, else fuel -> fuel
## slot, else nothing here (the player screen moves it hotbar <-> main).
func shift_targets(id: String) -> Array:
	if not recipes.smelting_for(id, station).is_empty():
		return [INPUT]
	if is_fuel(id):
		return [FUEL]
	return []


## Vanilla: the result slot empties into the player inventory back to front,
## input and fuel front to back.
func shift_out_reverse(slot: int) -> bool:
	return slot == OUTPUT


## Lit fraction 0..1 (flame gauge) and cook fraction 0..1 (arrow).
func flame() -> float:
	return float(lit_time) / float(maxi(lit_duration, 1)) if is_lit() else 0.0


func progress() -> float:
	return float(cook) / float(maxi(cook_total, 1))


func _can_burn(r: Dictionary) -> bool:
	if r.is_empty() or inv.is_empty_slot(INPUT) or int(inv.get_slot(INPUT)["count"]) < items.units(inv.get_slot(INPUT)["id"]):
		return false
	var out: Dictionary = r["output"]
	var o: Dictionary = inv.get_slot(OUTPUT)
	if o.is_empty():
		return true
	if o["id"] != out["id"]:
		return false
	return int(o["count"]) + int(out["count"]) * items.units(out["id"]) <= items.max_stack(out["id"])


func _smelt(r: Dictionary) -> void:
	var out: Dictionary = r["output"]
	inv.take(INPUT, items.units(inv.get_slot(INPUT)["id"]))   # a block's worth of a voxel item (ADR 0034)
	var n := int(out["count"]) * items.units(out["id"])
	var o: Dictionary = inv.get_slot(OUTPUT)
	if o.is_empty():
		inv.set_slot(OUTPUT, {"id": out["id"], "count": n})
	else:
		inv.set_slot(OUTPUT, {"id": out["id"], "count": int(o["count"]) + n})
	smelted += n


## One game tick (1/20 s). Returns true when the lit state flipped (Main then
## swaps the voxel between furnace and furnace_lit).
func tick() -> bool:
	var was_lit := is_lit()
	if is_lit():
		lit_time -= 1
	var input: Dictionary = inv.get_slot(INPUT)
	var fuel: Dictionary = inv.get_slot(FUEL)
	var has_input := not input.is_empty()
	var has_fuel := not fuel.is_empty()
	if is_lit() or (has_fuel and has_input):
		var r := recipes.smelting_for(input.get("id", ""), station) if has_input else {}
		if not is_lit() and _can_burn(r) and has_fuel and int(fuel["count"]) >= items.units(fuel["id"]):
			lit_time = fuel_ticks(fuel.get("id", ""))
			lit_duration = lit_time
			if is_lit() and has_fuel:
				var rest: String = items.get_def(fuel["id"]).get("empties_to", "")
				inv.take(FUEL, items.units(fuel["id"]))
				if rest != "" and inv.is_empty_slot(FUEL):
					inv.set_slot(FUEL, {"id": rest, "count": 1})
		if is_lit() and _can_burn(r):
			cook += 1
			if cook >= cook_total:
				cook = 0
				cook_total = int(r.get("ticks", 200))
				_smelt(r)
		else:
			cook = 0
	elif not is_lit() and cook > 0:
		cook = clampi(cook - 2, 0, cook_total)
	return was_lit != is_lit()


func contents() -> Array:
	var out := []
	for s in inv.to_array():
		if not s.is_empty():
			out.append(s)
	return out


## Save data. Slots first: setting the input resets the cook progress.
func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "slots": inv.to_array(), "lit_time": lit_time,
		"lit_duration": lit_duration, "cook": cook, "cook_total": cook_total}


func load_from(d: Dictionary) -> void:
	var slots: Array = d.get("slots", [])
	for i in mini(slots.size(), 3):
		inv.set_slot(i, slots[i])
	lit_time = int(d.get("lit_time", 0))
	lit_duration = int(d.get("lit_duration", 0))
	cook = int(d.get("cook", 0))
	cook_total = int(d.get("cook_total", cook_total))


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos, "input": inv.get_slot(INPUT), "fuel": inv.get_slot(FUEL),
		"output": inv.get_slot(OUTPUT), "lit": is_lit(), "lit_time": lit_time, "cook": cook,
		"cook_total": cook_total, "smelted": smelted}
