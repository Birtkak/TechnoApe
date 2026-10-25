class_name ClothingPanel
extends Control

## The worn clothing on the inventory screen (ADR 0059): three slots (head, body, feet) docked left of
## the inventory panel through part D's InventoryScreen.add_side_panel, drawn with its SlotView so they
## look and click like the rest. A click with a fitting piece on the cursor puts it on (what was worn
## comes onto the cursor); an empty cursor takes the piece off.

var player_inv: PlayerInventory
var clothing: Clothing
var _slots := {}   # slot name -> InventoryScreen.SlotView


func setup(p_inv: PlayerInventory, p_clothing: Clothing) -> void:
	player_inv = p_inv
	clothing = p_clothing
	var s := InventoryScreen.S
	size = Vector2(s, s * Clothing.SLOTS.size() + 4 * (Clothing.SLOTS.size() - 1))
	for i in Clothing.SLOTS.size():
		var name: String = Clothing.SLOTS[i]
		var sv := InventoryScreen.SlotView.new()
		sv.position = Vector2(0, i * (s + 4))
		sv.hint = name.capitalize()
		sv.clicked.connect(func(_b: int, _shift: bool) -> void: click(name))
		add_child(sv)
		_slots[name] = sv
	clothing.changed.connect(refresh)
	player_inv.changed.connect(refresh)
	refresh()


func refresh() -> void:
	for name in _slots:
		_slots[name].set_stack(player_inv.items, clothing.worn.get(name, {}))


## A click on the `slot` (head, body, feet).
func click(slot: String) -> void:
	var cur: Dictionary = player_inv.cursor
	if cur.is_empty():
		if clothing.worn.has(slot):
			player_inv.cursor = clothing.take_off(slot)
	elif Clothing.slot_of(player_inv.items.get_def(cur.get("id", ""))) == slot and int(cur.get("count", 1)) == 1:
		player_inv.cursor = clothing.wear(cur)
	player_inv.changed.emit()
	refresh()
