class_name TradingPostEntity
extends RefCounted

## A trading post (ADR 0164): a wooden stall with an owner. Its 26 slots are a chest's to the screen
## (duck-typed container, ADR 0007) but have three roles: 0..7 are four offers as pairs (2k = what the
## customer gets, 2k+1 = what the customer pays; templates, nothing real), 8..16 the stock for sale,
## 17..25 the takings (the owner takes, a sale puts the payment here). Only the owner opens the stock.
## Pure logic; the server runs the same sell() on its own copy of the slots (PostLedger).

signal changed

const OFFERS := 4
const STOCK_FROM := 8
const TAKE_FROM := 17
const SIZE := 26

var kind := "trading_post"
var title := "Trading Post"
var pos: Vector3i
var owner_name := ""
var facing := 0          # quarter turns: the front (price board) looks at the player who set it
var items: ItemRegistry
var inv: Inventory
var busy := false        # online: the owner has the stock screen open, so the server sells nothing meanwhile


func _init(p_items: ItemRegistry, p_pos: Vector3i) -> void:
	items = p_items
	pos = p_pos
	inv = Inventory.new(p_items, SIZE)
	inv.changed.connect(_on_slot_changed)


func _on_slot_changed(_i: int) -> void:
	changed.emit()


## Tools (anything with durability) and unknown ids are not traded: worn goods would be swapped for new.
func tradable(id: String) -> bool:
	return items.has(id) and int(items.get_def(id).get("durability", 0)) <= 0


func is_owner(player: String) -> bool:
	return owner_name == "" or owner_name == player


# ---- the container interface the inventory screen and PlayerInventory use ----

func is_template(i: int) -> bool:
	return i < STOCK_FROM


func may_place(slot: int, id: String) -> bool:
	return slot >= STOCK_FROM and slot < TAKE_FROM and tradable(id)


func is_output(slot: int) -> bool:
	return slot >= TAKE_FROM


func shift_targets(id: String) -> Array:
	return range(STOCK_FROM, TAKE_FROM) if tradable(id) else []


func shift_out_reverse(_slot: int) -> bool:
	return true


func tick() -> bool:
	return false


func is_lit() -> bool:
	return false


## Offer pairs along the top, then stock, then takings, on the screen's 36 px grid.
func slot_positions() -> Array:
	var out := []
	for i in SIZE:
		if i < STOCK_FROM:
			out.append(Vector2(14 + (i / 2) * 82 + (i % 2) * 36, 36))
		else:
			out.append(Vector2(16 + ((i - STOCK_FROM) % 9) * 36, 72 + ((i - STOCK_FROM) / 9) * 36))
	return out


## A click on an offer slot sets the template from the cursor stack (left: its count, right: one more)
## or clears it; it never moves the cursor's items. True when the click was handled here.
func click_override(i: int, button: int, shift: bool, cursor: Dictionary) -> bool:
	if not is_template(i):
		return false
	var s := inv.get_slot(i)
	if shift or cursor.is_empty():
		if button == MOUSE_BUTTON_RIGHT and not shift and not s.is_empty() and int(s["count"]) > 1:
			inv.set_slot(i, {"id": s["id"], "count": int(s["count"]) - 1})
		else:
			inv.set_slot(i, {})
		return true
	var id: String = cursor["id"]
	if not tradable(id):
		return true
	var cap := items.max_stack(id)
	if button == MOUSE_BUTTON_RIGHT:
		var n: int = mini(int(s["count"]) + 1, cap) if not s.is_empty() and s["id"] == id else 1
		inv.set_slot(i, {"id": id, "count": n})
	else:
		inv.set_slot(i, {"id": id, "count": mini(int(cursor["count"]), cap)})
	return true


# ---- offers and sales ----

## {give: {id, count}, want: {id, count}} for offer k, or {} when it is not fully set.
func offer(k: int) -> Dictionary:
	if k < 0 or k >= OFFERS:
		return {}
	var g := inv.get_slot(k * 2)
	var w := inv.get_slot(k * 2 + 1)
	if g.is_empty() or w.is_empty():
		return {}
	return {"give": {"id": g["id"], "count": int(g["count"])}, "want": {"id": w["id"], "count": int(w["count"])}}


func stock_count(id: String) -> int:
	var n := 0
	for i in range(STOCK_FROM, TAKE_FROM):
		var s := inv.get_slot(i)
		if not s.is_empty() and s["id"] == id:
			n += int(s["count"])
	return n


func _takings_room(id: String) -> int:
	var cap := items.max_stack(id)
	var room := 0
	for i in range(TAKE_FROM, SIZE):
		var s := inv.get_slot(i)
		if s.is_empty():
			room += cap
		elif s["id"] == id and Inventory.heat_of(s) <= 0.0:
			room += cap - int(s["count"])
	return room


## "" when the shop can sell offer k now, else why not.
func check_sale(k: int) -> String:
	if busy:
		return "The owner is busy at the stock"
	var o := offer(k)
	if o.is_empty():
		return "That offer is not set"
	if stock_count(o["give"]["id"]) < int(o["give"]["count"]):
		return "Out of stock"
	if _takings_room(o["want"]["id"]) < int(o["want"]["count"]):
		return "The takings are full"
	return ""


## The shop's half of a sale: the goods leave the stock, the payment lands in the takings. Returns
## {give: [stacks], want: stack}, or {} when check_sale refuses. The buyer's half is the caller's.
func sell(k: int) -> Dictionary:
	if check_sale(k) != "":
		return {}
	var o := offer(k)
	var left: int = o["give"]["count"]
	var give := []
	for i in range(STOCK_FROM, TAKE_FROM):
		var s := inv.get_slot(i)
		if left <= 0:
			break
		if not s.is_empty() and s["id"] == o["give"]["id"]:
			var t := inv.take(i, left)
			left -= int(t["count"])
			give.append(t)
	var pay: int = o["want"]["count"]
	for i in range(TAKE_FROM, SIZE):
		if pay <= 0:
			break
		var s := inv.get_slot(i)
		if s.is_empty() or (s["id"] == o["want"]["id"] and Inventory.heat_of(s) <= 0.0):
			pay = int(inv.put(i, {"id": o["want"]["id"], "count": pay}).get("count", 0))
	return {"give": give, "want": o["want"].duplicate()}


## "" when `buyer` can pay offer `o`'s price and hold the goods (a copy is tried), else why not.
static func buyer_check(p_items: ItemRegistry, o: Dictionary, buyer: Inventory) -> String:
	if o.is_empty():
		return "That offer is not set"
	if buyer.count_of(o["want"]["id"]) < int(o["want"]["count"]):
		return "You need %d %s" % [o["want"]["count"], String(p_items.get_def(o["want"]["id"]).get("name", o["want"]["id"]))]
	var t := Inventory.new(p_items, buyer.size())
	for i in buyer.size():
		t.set_slot(i, buyer.get_slot(i))
	t.remove(o["want"]["id"], int(o["want"]["count"]))
	if t.add(o["give"]["id"], int(o["give"]["count"])) > 0:
		return "No room in your inventory"
	return ""


## Single-player (and tests): one whole trade of offer k with `buyer`. {ok, why}; nothing moves on a refusal.
func buy_local(k: int, buyer: Inventory) -> Dictionary:
	var why := check_sale(k)
	if why == "":
		why = buyer_check(items, offer(k), buyer)
	if why != "":
		return {"ok": false, "why": why}
	var o := offer(k)
	buyer.remove(o["want"]["id"], int(o["want"]["count"]))
	var sale := sell(k)
	for s in sale["give"]:
		buyer.add(s["id"], int(s["count"]), Inventory.data_of(s))
	return {"ok": true, "why": "", "give": o["give"], "want": o["want"]}


# ---- the block-entity interface ----

func contents() -> Array:
	var out := []
	for i in range(STOCK_FROM, SIZE):
		var s := inv.get_slot(i)
		if not s.is_empty():
			out.append(s)
	return out


func slots_array() -> Array:
	return inv.to_array()


func load_slots(slots: Array) -> void:
	for i in SIZE:
		inv.set_slot(i, slots[i] if i < slots.size() and slots[i] is Dictionary else {})


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "owner": owner_name, "facing": facing, "slots": inv.to_array()}


func load_from(d: Dictionary) -> void:
	owner_name = SignText.name_of(String(d.get("owner", "")))
	facing = clampi(int(d.get("facing", 0)), 0, 3)
	load_slots(d.get("slots", []))


func summary() -> Dictionary:
	var n := 0
	for k in OFFERS:
		if not offer(k).is_empty():
			n += 1
	return {"kind": kind, "pos": pos, "owner": owner_name, "offers": n}


func look_lines() -> Array[String]:
	return ["%s's trading post" % (owner_name if owner_name != "" else "Your")]


## A plank stall: a crate base, posts, a striped awning and a price board on the front.
func view_parts() -> Dictionary:
	var n := 16
	var size := Vector3i(n, n, n)
	var c := WorkPalette.new_cells(size)
	WorkPalette.box(c, size, Vector3i(1, 0, 3), Vector3i(15, 8, 15), WorkPalette.WOOD)
	WorkPalette.box(c, size, Vector3i(1, 0, 3), Vector3i(15, 1, 15), WorkPalette.WOOD_DARK)
	WorkPalette.box(c, size, Vector3i(1, 7, 3), Vector3i(15, 8, 15), WorkPalette.WOOD_DARK)
	WorkPalette.box(c, size, Vector3i(2, 8, 3), Vector3i(4, 14, 5), WorkPalette.WOOD_DARK)
	WorkPalette.box(c, size, Vector3i(12, 8, 3), Vector3i(14, 14, 5), WorkPalette.WOOD_DARK)
	WorkPalette.box(c, size, Vector3i(0, 14, 2), Vector3i(16, 16, 10), WorkPalette.PAINT)
	WorkPalette.box(c, size, Vector3i(0, 14, 2), Vector3i(16, 15, 3), WorkPalette.LINEN)
	WorkPalette.box(c, size, Vector3i(4, 4, 0), Vector3i(12, 7, 3), WorkPalette.WOOD_DARK)
	WorkPalette.box(c, size, Vector3i(5, 5, 0), Vector3i(11, 6, 1), WorkPalette.LINEN)
	return {"solid": {"cells": WorkPalette.rotate_y(c, size, facing), "size": size, "grid": n}}
