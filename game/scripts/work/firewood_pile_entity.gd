class_name FirewoodPileEntity
extends RefCounted

## A pile of firewood (ADR 0030): Vintage Story's charcoal pit, built in the
## world. A right click with firewood adds one (Settings.firewood_per_pile
## per voxel; a full pile takes the next one in front of the clicked face).
## Piles that touch are one pit. A firestarter lights the whole pit, and
## WorkSystem then checks every second that no face of its piles is open to
## air or to anything that burns. Still open once Settings.charcoal_pit_grace_s
## has passed, or opened while it smoulders, the wood bursts into flames.
## Sealed for Settings.charcoal_pit_burn_s, every pile turns into charcoal
## (the "charcoal_pit" recipe: one per `per` firewood), dug out by breaking it.
## Solid WORK_BLOCK voxel, drawn by its own micro mesh.

signal changed
signal charred   # the pile turned into charcoal (quest event, a chat line)

var kind := "firewood_pile"
var pos: Vector3i
var count := 1
var state := "wood"    # wood | lit | charcoal
var burn := 0          # ticks of smouldering left
var grace := 0         # ticks left to finish sealing the pit after lighting
var sealed := false    # WorkSystem's last seal check of this pile's pit
var charcoal := 0
var recipes: RecipeRegistry


func _init(p_recipes: RecipeRegistry, p_pos: Vector3i) -> void:
	recipes = p_recipes
	pos = p_pos


func is_lit() -> bool:
	return state == "lit"


func is_full() -> bool:
	return count >= Settings.firewood_per_pile


## Adds up to n firewood; returns how many it took.
func add(n: int) -> int:
	if state != "wood":
		return 0
	var took := mini(n, Settings.firewood_per_pile - count)
	if took > 0:
		count += took
		changed.emit()
	return took


func light() -> void:
	if state != "wood":
		return
	state = "lit"
	burn = int(Settings.charcoal_pit_burn_s * 20.0)
	grace = int(Settings.charcoal_pit_grace_s * 20.0)
	sealed = false
	changed.emit()


## The charcoal this pile gives: one per recipe `per` firewood, at least one.
func charcoal_yield() -> int:
	var r := recipes.first_of("charcoal_pit", {"input": "firewood"})
	var per := int(r.get("per", 2)) if not r.is_empty() else 2
	var each := int(r["output"].get("count", 1)) if not r.is_empty() else 1
	return maxi(1, count / per) * each


## One tick (1/20 s). The pit chars only while sealed; an open one waits for
## WorkSystem to set it on fire.
func tick() -> bool:
	if state != "lit":
		return false
	grace = maxi(0, grace - 1)
	if burn > 0:
		burn -= 1
	if burn == 0 and sealed:
		state = "charcoal"
		charcoal = charcoal_yield()
		changed.emit()
		charred.emit()
	return false


func contents() -> Array:
	match state:
		"charcoal":
			return [{"id": "charcoal", "count": charcoal}]
		"lit":
			return []   # broken open while it smoulders: the wood is half burnt
	return [{"id": "firewood", "count": count}]


func view_parts() -> Dictionary:
	var size := Vector3i(8, 8, 8)
	var c := WorkPalette.new_cells(size)
	if state == "charcoal":
		var h := clampi(ceili(charcoal * 16.0 / Settings.firewood_per_pile), 1, 8)
		for y in h:
			for z in 8:
				for x in 8:
					if y < h - 1 or (x + z) % 3 != 0:
						c[WorkPalette.idx(size, x, y, z)] = WorkPalette.CHARCOAL + 1
		return {"solid": {"cells": c, "size": size}}
	# one split log (2 x 2 cells, 8 long) per firewood of a full pile's 16:
	# four to a layer, layers crossing like a Vintage Story pile
	var logs := clampi(count * 16 / Settings.firewood_per_pile, 1, 16)
	for i in logs:
		var l := i / 4
		var k := i % 4
		for a in 8:
			for t in 2:
				for w in 2:
					var y := l * 2 + t
					var b := k * 2 + w
					var x := a if l % 2 == 0 else b
					var z := b if l % 2 == 0 else a
					var end := a == 0 or a == 7
					# end grain dark with a light heart; along the log a split face and a bark side
					var paint := WorkPalette.FIREWOOD_LIGHT if (end and t == w) or (not end and w == 0) else WorkPalette.FIREWOOD
					if state == "lit" and (x * 5 + y * 3 + z) % 7 == 0:
						paint = WorkPalette.EMBER
					c[WorkPalette.idx(size, x, y, z)] = paint + 1
	return {"solid": {"cells": c, "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "count": count, "state": state, "burn": burn,
		"grace": grace, "charcoal": charcoal}


func load_from(d: Dictionary) -> void:
	count = int(d.get("count", 1))
	state = String(d.get("state", "wood"))
	burn = int(d.get("burn", 0))
	grace = int(d.get("grace", 0))
	charcoal = int(d.get("charcoal", 0))


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos, "count": count, "state": state, "burn": burn, "grace": grace,
		"sealed": sealed, "charcoal": charcoal}
