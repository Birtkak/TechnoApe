class_name MachineScreen
extends RefCounted

## An LV machine's screen (ADR 0168, GregTech style): the inventory screen's container layout with the
## machine's input slots on the left, a progress arrow, its output slot on the right and a power bar.
## The machine keeps its inputs in `inv` and its result in `output` (ADR 0117); this view mirrors both
## into one Inventory for the screen (inputs, then the output) and writes every change the player makes
## back, so hoppers, saves and right clicks keep working on the machine itself.

signal changed

var kind := "machine_screen"
var title: String
var pos: Vector3i
var inv: Inventory
var machine: MachineEntity
var _syncing := false


func _init(p_machine: MachineEntity) -> void:
	machine = p_machine
	pos = p_machine.pos
	title = p_machine.title
	inv = Inventory.new(p_machine.items, p_machine.inv.size() + 1)
	inv.changed.connect(_write_back)
	refresh()
	machine.changed.connect(refresh)


func inputs() -> int:
	return machine.inv.size()


## Re-reads the machine's slots (it finished a recipe, a hopper fed it).
func refresh() -> void:
	_syncing = true
	for i in inputs():
		inv.set_slot(i, machine.inv.get_slot(i))
	inv.set_slot(inputs(), machine.output)
	_syncing = false
	changed.emit()


## A slot the player changed goes back into the machine at once.
func _write_back(i: int) -> void:
	if _syncing:
		return
	var s := inv.get_slot(i)
	if i < inputs():
		machine.inv.set_slot(i, s)
	else:
		machine.output = {} if s.is_empty() else s.duplicate(true)
	machine.changed.emit()


## The screen closed: stop following the machine.
func detach() -> void:
	if machine.changed.is_connected(refresh):
		machine.changed.disconnect(refresh)


func may_place(slot: int, id: String) -> bool:
	return slot < inputs() and machine.takes(id)


func is_output(slot: int) -> bool:
	return slot == inputs()


## Shift from the player: an ingredient goes to the input slots, anything else stays.
func shift_targets(id: String) -> Array:
	return range(inputs()) if machine.takes(id) else []


func shift_out_reverse(slot: int) -> bool:
	return slot == inputs()


func progress() -> float:
	return machine.progress_frac() if not machine.current().is_empty() else 0.0


## How much of its watts it got in the last solve, 0..1 (1 for an unpowered machine that runs).
func power() -> float:
	if machine.load == null:
		return 1.0 if machine.powered() else 0.0
	return clampf(machine.load.drawn / maxf(machine.load.watts, 1.0), 0.0, 1.0) if machine.powered() else 0.0


## The line under the slots: what it is making, or why it is not.
func status() -> String:
	var r := machine.current()
	var watts := "%d W" % int(machine.load.watts) if machine.load != null else ""
	if r.is_empty():
		var why := "Waiting for input" if machine.inv.to_array().all(func(s: Dictionary) -> bool: return s.is_empty()) else "No recipe for these inputs, or the output is full"
		if Waste.blocked(machine.waste):
			why = Waste.status(machine.waste)
		return why + ("  |  needs " + watts if watts != "" else "")
	var out := machine.items.display_name(String(r["output"]["id"]))
	if not machine.powered():
		return "%s: no power (needs %s at 48 V)" % [out, watts] if watts != "" else "%s: not running" % out
	return "Making %s  %d%%%s" % [out, roundi(progress() * 100.0), ("  |  " + watts) if watts != "" else ""]


## Inputs in rows of three left of the arrow, the output right of it (the furnace's 232, 70).
func slot_positions() -> Array:
	var out := []
	var n := inputs()
	var rows := ceili(n / 3.0)
	for i in n:
		var row := i / 3
		var in_row := mini(3, n - row * 3)
		out.append(Vector2(148 - in_row * 36 + (i % 3) * 36, 70 - (rows - 1) * 18 + row * 36))
	out.append(Vector2(232, 70))
	return out


func tick() -> bool:
	return false


func is_lit() -> bool:
	return machine.is_lit()


func contents() -> Array:
	return []
