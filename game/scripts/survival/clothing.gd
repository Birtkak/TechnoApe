class_name Clothing
extends RefCounted

## What the player wears (ADR 0059): one stack per slot (head, body, feet), put on by right
## clicking with a clothing item (items.json "clothing": {slot, warmth_c}); the piece that was in
## that slot comes back to the hand. Warmth raises the temperature the body feels while the air is
## colder than Settings.comfort_max_c, never past it (a fur cloak does not cook you in summer).
## Saved with the player; Survival reads warm().

signal changed

const SLOTS := ["head", "body", "feet", "back"]   # back: the jetpack (ADR 0099)

var items: ItemRegistry
var worn := {}   # slot -> stack


func _init(p_items: ItemRegistry) -> void:
	items = p_items


static func slot_of(def: Dictionary) -> String:
	return String(def.get("clothing", {}).get("slot", ""))


## Puts `stack` on; returns the stack that came off its slot ({} when it was free).
func wear(stack: Dictionary) -> Dictionary:
	var slot := slot_of(items.get_def(stack.get("id", "")))
	if not slot in SLOTS:
		return stack
	var old: Dictionary = worn.get(slot, {})
	worn[slot] = Inventory.make_stack(items, stack["id"], 1, stack)
	changed.emit()
	return old


func take_off(slot: String) -> Dictionary:
	var old: Dictionary = worn.get(slot, {})
	worn.erase(slot)
	changed.emit()
	return old


## Armor points of all worn pieces (items.json clothing.armor): leather 5 for the set, copper 9.
func armor() -> int:
	var a := 0
	for slot in worn:
		a += int(items.get_def(worn[slot]["id"]).get("clothing", {}).get("armor", 0))
	return a


## A mob hit of `damage` after armor: 4% less per point (Minecraft-like), the fraction rounded by
## chance so one-heart bites still get blocked sometimes.
func armor_cut(damage: int) -> int:
	var d := float(damage) * maxf(0.2, 1.0 - 0.04 * armor())
	return int(d) + (1 if randf() < d - floorf(d) else 0)


## Degrees C all worn pieces add.
func warmth_c() -> float:
	var w := 0.0
	for slot in worn:
		w += float(items.get_def(worn[slot]["id"]).get("clothing", {}).get("warmth_c", 0.0))
	return w


## The temperature the body feels in air of env (C) with warmth w: up to w warmer, but clothes never
## push it past the top of the comfort band.
static func felt_c(env: float, w: float) -> float:
	if env >= Settings.comfort_max_c or w <= 0.0:
		return env
	return minf(env + w, Settings.comfort_max_c)


func to_dict() -> Dictionary:
	return worn.duplicate(true)


func load_dict(d: Dictionary) -> void:
	worn.clear()
	for slot in d:
		var s = d[slot]
		if slot in SLOTS and s is Dictionary and items.has(String(s.get("id", ""))):
			worn[slot] = Inventory.make_stack(items, String(s["id"]), 1, s)
	changed.emit()


func summary() -> Dictionary:
	var out := {}
	for slot in worn:
		out[slot] = worn[slot]["id"]
	out["warmth_c"] = warmth_c()
	out["armor"] = armor()
	return out
