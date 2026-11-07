class_name TrapEntity
extends RefCounted

## A land trap (ADR 0163). "snare": a cordage noose pegged on the ground (one 0.5 m block, a voxel high);
## a small animal (or the player's foot) that steps into it is held. "pit_trap": sticks and dry grass
## over a dug pit (1 m square, two voxels of WORK_SPOT flush with the rim, so bodies fall straight
## through); whatever walks over it drops in. Fishing checks them and holds the catch (Mob.trap_at);
## the catch's species is kept here and the animal itself is saved with the mobs.

signal changed

var kind := "snare"
var pos: Vector3i
var sprung := false           # it went off: a snare needs setting again, a pit a new cover
var catch := ""               # the species held, "" for none


func _init(p_pos: Vector3i, p_kind: String) -> void:
	pos = p_pos
	kind = p_kind


func is_lit() -> bool:
	return false


func is_pit() -> bool:
	return kind == "pit_trap"


## The middle of the trap's floor (snare) or of its cover (pit), in world voxels.
func centre() -> Vector3:
	var w := float(BlockEntities.width_of(kind))
	return Vector3(pos) + Vector3(w * 0.5, 0.0, w * 0.5)


func armed() -> bool:
	return not sprung and catch == ""


func spring(species: String) -> void:
	sprung = true
	catch = species
	changed.emit()


## Empty again (the catch taken, a snare set again, a pit covered again).
func reset() -> void:
	sprung = false
	catch = ""
	changed.emit()


func look_lines() -> Array:
	if catch != "":
		return ["Caught: %s. Right click to take it." % catch.capitalize()]
	if is_pit():
		return ["The cover has fallen in. Lay a new pit cover over it."] if sprung else \
			["Sticks and dry grass over a pit: something heavy will fall through.", "Empty hand: take the cover back"]
	return ["Sprung. Right click to set it again."] if sprung else \
		["A noose on a bent stick: small animals walk into it. Mind your feet.", "Empty hand: pick it up"]


func contents() -> Array:
	return [] if sprung and is_pit() else [{"id": "pit_cover" if is_pit() else "snare", "count": 1}]


func view_parts() -> Dictionary:
	var size := Vector3i(8, 8, 8)
	var c := WorkPalette.new_cells(size)
	if is_pit():
		# one cell = one voxel: a lattice of sticks under a skin of dry grass and earth with gaps (the sign)
		for i in 8:
			for j in 8:
				var rim := i == 0 or i == 7 or j == 0 or j == 7
				if sprung:
					if rim and (i + j) % 3 == 0:
						c[WorkPalette.idx(size, i, 1, j)] = WorkPalette.WOOD_DARK + 1   # broken ends at the rim
					continue
				if i % 3 == 1 or j % 3 == 1:
					c[WorkPalette.idx(size, i, 0, j)] = WorkPalette.WOOD_DARK + 1
				var h := (i * 7 + j * 13) % 11
				if h != 0:
					c[WorkPalette.idx(size, i, 1, j)] = (WorkPalette.STRAW if h % 3 else WorkPalette.SOIL) + 1
	else:
		# one cell = half a voxel: a bent stick peg in a corner, the noose laid round the middle
		WorkPalette.box(c, size, Vector3i(1, 0, 1), Vector3i(2, 6, 2), WorkPalette.WOOD)
		WorkPalette.box(c, size, Vector3i(1, 5, 1), Vector3i(3, 6, 2), WorkPalette.WOOD)
		var r := 1.0 if sprung else 2.6
		var y := 2 if sprung else 0
		for i in 8:
			for j in 8:
				var d := Vector2(i + 0.5 - 4.0, j + 0.5 - 4.0).length()
				if absf(d - r) < 0.6 and catch == "":
					c[WorkPalette.idx(size, i, y, j)] = WorkPalette.STRAW + 1
		if catch == "":
			WorkPalette.box(c, size, Vector3i(2, y, 2), Vector3i(3, y + 1, 3), WorkPalette.STRAW)   # the line to the peg
	return {"solid": {"cells": c, "size": size}, "items": []}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "sprung": sprung, "catch": catch}


func load_from(d: Dictionary) -> void:
	sprung = bool(d.get("sprung", false))
	catch = String(d.get("catch", ""))


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos, "sprung": sprung, "catch": catch}
