class_name SteamHammerEntity
extends AnvilEntity

## The steam hammer (ADR 0097): a bronze anvil under a steam-driven head. Worked like the anvil (a
## hot ingot on, a shape picked, a hand hammer still works it), but with a boiler on its pipes it
## strikes by itself every steam_hammer_strike_s at working pressure, faster above: each strike
## drives one extra cell toward the nearest column still short of the shape, or cuts it off when
## no column wants it. A finished piece waits on the anvil for an empty hand. The hammer rusts
## with every strike (`wear`) and stops at 1 until an iron plate mends it.

signal struck(p: Vector3i, col: Vector2i)   # a steam blow landed on this column (sound, sparks)

var boiler: BoilerEntity = null
var ready: Dictionary = {}
var wear := 0.0
var strikes := 0
var _cooldown := 0.0
var _head_down := 0   # ticks the head is drawn down after a blow


func _init(p_recipes: RecipeRegistry, p_pos: Vector3i) -> void:
	super(p_recipes, p_pos, "steam_hammer")
	item = "steam_hammer"
	tier = 1


func powered() -> bool:
	return boiler != null and boiler.pressure > 0.0 and wear < 1.0


func tick() -> bool:
	super.tick()
	if _head_down > 0:
		_head_down -= 1
		if _head_down == 0:
			changed.emit()   # the head rises again
	if not powered() or not has_recipe() or not ready.is_empty() or short_of() > 0 or heat() < Settings.smith_min_heat:
		return false
	_cooldown -= BlockEntities.TICK_S * clampf(boiler.pressure, 0.25, 2.0)
	if _cooldown > 0.0:
		return false
	_cooldown = Settings.steam_hammer_strike_s
	if not boiler.draw(Settings.steam_per_use):
		return false
	var r := strike()
	if r == "done":
		ready = finish()
	return false


## One automatic blow. Returns what AnvilEntity.hit / split return, "" with nothing to do.
func strike() -> String:
	var from := Vector2i(-1, -1)
	for z in N:
		for x in N:
			if height(x, z) > target(x, z):
				from = Vector2i(x, z)
				break
		if from.x >= 0:
			break
	if from.x < 0:
		return ""
	strikes += 1
	wear = minf(1.0, wear + 1.0 / (Settings.boiler_wear_s / Settings.steam_hammer_strike_s))
	_head_down = 4
	struck.emit(pos, from)
	var to := _nearest_short(from)
	if to.x >= 0:
		var d := to - from
		var dirs := [Vector2i(signi(d.x), 0), Vector2i(0, signi(d.y))] if absi(d.x) >= absi(d.y) else [Vector2i(0, signi(d.y)), Vector2i(signi(d.x), 0)]
		for dir in dirs:
			if dir == Vector2i.ZERO:
				continue
			var r := hit(from.x, from.y, dir)
			if r != "stuck":
				return r
	return split(from.x, from.y)


func _nearest_short(from: Vector2i) -> Vector2i:
	var best := Vector2i(-1, -1)
	var best_d := 1 << 20
	for z in N:
		for x in N:
			if height(x, z) < target(x, z):
				var d := absi(x - from.x) + absi(z - from.y)
				if d < best_d:
					best_d = d
					best = Vector2i(x, z)
	return best


## An empty hand takes the finished piece first, then the metal on the anvil.
func take_piece() -> Dictionary:
	if not ready.is_empty():
		var out := ready
		ready = {}
		changed.emit()
		return out
	return super()


func contents() -> Array:
	var out := super()
	if not ready.is_empty():
		out.append(ready)
	return out


func look_lines() -> Array[String]:
	var out: Array[String] = []
	if not ready.is_empty():
		out.append("Done: %s (empty hand takes it)" % recipes.items.display_name(ready["id"]))
		return out
	if wear >= 1.0:
		out.append("Rusted through: fit an iron plate")
	elif boiler == null:
		out.append("No steam: lay bronze pipes from a boiler")
	elif boiler.pressure <= 0.0:
		out.append("No pressure: the boiler is cold")
	else:
		out.append("Steam at %.2f bar" % boiler.pressure)
	out.append_array(super())
	return out


## 1.5 m wide (SCALE 3) and 3 m tall: the anvil's own view, and over it an iron frame: two posts at the
## back carrying a crossbeam, the steam cylinder on the beam with the hammer head hanging under it
## just above the work face; the steam inlet runs down the back post. Cells are 1.5 voxels.
func view_parts() -> Dictionary:
	var parts := super()
	var old: Vector3i = parts["solid"]["size"]
	var size := Vector3i(old.x, 16, old.z)
	var c := _taller(parts["solid"]["cells"], old, size)
	var top := AnvilEntity.BASE_H + AnvilEntity.MAX_H   # 11: the highest a metal cell stands
	var post := WorkPalette.IRON_DARK
	WorkPalette.box(c, size, Vector3i(0, 0, 0), Vector3i(2, size.y, 2), post)             # the posts, behind the anvil
	WorkPalette.box(c, size, Vector3i(6, 0, 0), Vector3i(8, size.y, 2), post)
	WorkPalette.box(c, size, Vector3i(0, size.y - 2, 0), Vector3i(8, size.y, 8), post)    # the crossbeam
	WorkPalette.box(c, size, Vector3i(2, top + 3, 2), Vector3i(6, size.y - 2, 6), WorkPalette.ANVIL if powered() else WorkPalette.STONE_DARK)   # the cylinder
	# the head hangs just over the face, and drops onto the metal for a few ticks after a blow
	var drop := 1 if _head_down > 0 else 0
	WorkPalette.box(c, size, Vector3i(3, top + 2 - drop, 3), Vector3i(5, top + 3, 5), WorkPalette.ANVIL)   # the piston rod
	WorkPalette.box(c, size, Vector3i(1, top - drop, 1), Vector3i(7, top + 2 - drop, 7), post)
	WorkPalette.box(c, size, Vector3i(6, 2, 0), Vector3i(7, top + 3, 1), WorkPalette.BRONZE)   # the steam inlet up the post
	if wear > 0.5:
		WorkPalette.box(c, size, Vector3i(0, 2, 0), Vector3i(2, 5, 2), WorkPalette.BRICK_DARK)   # rust
	if parts.has("ghost"):
		parts["ghost"] = {"cells": _taller(parts["ghost"]["cells"], old, size), "size": size}
	parts["solid"] = {"cells": c, "size": size}
	return parts


static func _taller(cells: PackedInt32Array, old: Vector3i, size: Vector3i) -> PackedInt32Array:
	var c := WorkPalette.new_cells(size)
	for z in old.z:
		for y in old.y:
			for x in old.x:
				c[WorkPalette.idx(size, x, y, z)] = cells[WorkPalette.idx(old, x, y, z)]
	return c


func to_dict() -> Dictionary:
	var d := super()
	d["ready"] = ready
	d["wear"] = wear
	return d


func load_from(d: Dictionary) -> void:
	super(d)
	ready = d.get("ready", {})
	wear = float(d.get("wear", 0.0))


func summary() -> Dictionary:
	var s := super()
	s["strikes"] = strikes
	s["ready"] = ready.get("id", "")
	s["powered"] = powered()
	return s
