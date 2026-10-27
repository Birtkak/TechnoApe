class_name BoilerEntity
extends FirepitEntity

## The small boiler (ADR 0097): a riveted iron drum on a brick firebox. Coal or charcoal goes under
## it like on a forge, a water bucket fills the tank a quarter, a firestarter lights it. Lit with
## water it builds `pressure` (1.0 = working pressure) and boils the water away; bronze pipes laid
## from it (feed(), every second) hand the pressure to the steam hammer and steam sieve at their
## ends, which draw it down. Above 1.0 the whistle screams and above boiler_burst_at it bursts;
## lit and dry it screams for boiler_dry_s, then bursts. An empty hand with nothing to take vents
## half the pressure. It rusts while lit (`wear`): steam falls with wear and an iron plate mends it.

signal burst(p: Vector3i)

var water := 0.0          # of the tank, 0..1
var pressure := 0.0
var wear := 0.0           # 0..1, rusted through at 1
var dry_ticks := 0
var _whistle := 0


func _init(p_items: ItemRegistry, p_recipes: RecipeRegistry, p_pos: Vector3i) -> void:
	super(p_items, p_recipes, p_pos)
	kind = "boiler"
	title = "Boiler"
	station = "boiler"


func burns(id: String) -> bool:
	return items.has_tag(id, "coal") and fuel_ticks(id) > 0


func heats(_id: String) -> bool:
	return false


func may_place(slot: int, id: String) -> bool:
	return slot == FUEL and burns(id)


func screaming() -> bool:
	return is_lit() and (pressure > 1.0 or water <= 0.0)


## A machine takes `amount` of pressure. False when there is not that much.
func draw(amount: float) -> bool:
	if pressure < amount:
		return false
	pressure -= amount
	return true


func tick() -> bool:
	super.tick()
	if is_lit():
		wear = minf(1.0, wear + TICK / Settings.boiler_wear_s)
		if water > 0.0:
			water = maxf(0.0, water - TICK / Settings.boiler_boil_s)
			pressure += TICK / Settings.boiler_build_s * (1.0 - wear)
			dry_ticks = 0
		else:
			dry_ticks += 1
		if pressure >= Settings.boiler_burst_at or dry_ticks * TICK >= Settings.boiler_dry_s:
			lit_time = 0   # the blast puts it out (WorkSystem removes the piece); once
			pressure = 0.0
			water = 0.0
			dry_ticks = 0
			burst.emit(pos)
			return true
	else:
		pressure = maxf(0.0, pressure - TICK / Settings.boiler_build_s)
	_whistle = (_whistle + 1) % 20
	if _whistle == 0:
		changed.emit()   # the gauge and the water level move
	return false


func use(stack: Dictionary, def: Dictionary, dry := false, all := false) -> Dictionary:
	var id: String = stack.get("id", "")
	var res := {"consume": 0, "give": [], "ignited": false, "act": false, "msg": ""}
	if id == "water_bucket":
		if water >= 1.0:
			res["msg"] = "The tank is full."
			return res
		res["act"] = true
		if not dry:
			water = minf(1.0, water + Settings.boiler_bucket_fill)
			res["consume"] = 1
			res["give"] = [{"id": String(def.get("empties_to", "bucket")), "count": 1}]
			changed.emit()
		return res
	if id == "iron_plate" and wear > 0.0:
		res["act"] = true
		if not dry:
			wear = 0.0
			res["consume"] = 1
			res["msg"] = "The boiler is patched up."
			changed.emit()
		return res
	if id == "" and (is_lit() or inv.is_empty_slot(FUEL)):
		if pressure > 0.0:
			res["act"] = true
			if not dry:
				pressure *= 0.5
				res["msg"] = "Steam vents with a hiss."
			return res
		res["msg"] = "Coal under it, water in it, then light it with a firestarter."
		return res
	var out := super(stack, def, dry, all)
	if out["msg"].begins_with("%s does not go on" % items.display_name(id)):
		out["msg"] = "The boiler takes coal, water buckets and iron plates."
	return out


func look_lines() -> Array[String]:
	var out: Array[String] = []
	if is_lit():
		out.append("Burning (%d%% of this fuel left)" % roundi(flame() * 100.0))
	else:
		out.append("Not lit: light the firebox with a firestarter" if not inv.is_empty_slot(FUEL) else "Not lit: lay coal under it")
	var fuel := inv.get_slot(FUEL)
	if not fuel.is_empty():
		out.append("Fuel: %s" % _count_name(fuel))
	out.append("Water: %d%%%s" % [roundi(water * 100.0), " (DRY: it will burst!)" if is_lit() and water <= 0.0 else ""])
	out.append("Pressure: %.2f bar%s" % [pressure, " (over pressure: vent it!)" if pressure > 1.0 else ""])
	if wear >= 1.0:
		out.append("Rusted through: fit an iron plate")
	elif wear > 0.5:
		out.append("Rusting: an iron plate mends it")
	return out


## 2.5 m on a side (SCALE 5), drawn at one cell per voxel (grid 20).
func view_parts() -> Dictionary:
	var n := BlockEntities.width_of(kind)
	var size := Vector3i(n, n, n)
	var c := WorkPalette.new_cells(size)
	var lit := is_lit()
	# a brick firebox seven courses high with an arched fire door at the front (+z) and the fire glowing in it;
	# on it a riveted iron drum lying along x with rounded ends, rivet rings every fourth voxel and a seam along
	# the top; a chimney at the back, a round gauge on the front end that glows with the pressure, the steam
	# outlet (bronze) on the +x end, a filler cap on top
	WorkPalette.wall_box(c, size, Vector3i(0, 0, 0), Vector3i(n, 7, n), WorkPalette.WALL_BRICK, WorkPalette.BRICK_DARK)
	for z in range(2, n):   # the hollow and the door
		for y in range(0, 5):
			for x in range(7, 13):
				if z < n - 2 or y < 4 and x > 7 and x < 12:
					c[WorkPalette.idx(size, x, y, z)] = 0
	WorkPalette.box(c, size, Vector3i(7, 0, 2), Vector3i(13, 1, n - 2), WorkPalette.EMBER if lit else WorkPalette.ASH)
	if lit:
		WorkPalette.box(c, size, Vector3i(8, 1, 3), Vector3i(12, 3, n - 3), WorkPalette.GLOW)
	WorkPalette.box(c, size, Vector3i(6, 4, n - 2), Vector3i(14, 5, n), WorkPalette.SOOT)   # soot over the door
	var cy := 13.0
	var cz := n / 2.0
	var r := 6.0
	for x in range(1, n - 1):
		var rx := r if x > 2 and x < n - 3 else r - 1.0   # the ends round off
		for y in range(7, n):
			for z in n:
				var d2 := (y - cy) * (y - cy) + (z + 0.5 - cz) * (z + 0.5 - cz)
				if d2 > rx * rx:
					continue
				var tone := WorkPalette.IRON_DARK
				if d2 >= (rx - 1.0) * (rx - 1.0):
					if x % 4 == 2 and x > 2 and x < n - 3:
						tone = WorkPalette.ANVIL     # a rivet ring
					elif absi(int(z + 0.5 - cz)) == 0 and y > cy:
						tone = WorkPalette.STONE_DARK   # the top seam
				c[WorkPalette.idx(size, x, y, z)] = tone + 1
	WorkPalette.box(c, size, Vector3i(n - 5, 17, 9), Vector3i(n - 3, n, 11), WorkPalette.SOOT)        # the chimney
	WorkPalette.box(c, size, Vector3i(8, 19, 9), Vector3i(11, n, 11), WorkPalette.BRONZE)             # the filler cap
	var gauge := WorkPalette.AVOID if pressure > 1.0 else (WorkPalette.GLOW if pressure > 0.3 else WorkPalette.COPPER)
	WorkPalette.box(c, size, Vector3i(0, 11, 8), Vector3i(1, 15, 12), WorkPalette.BRONZE)              # the gauge's rim
	WorkPalette.box(c, size, Vector3i(0, 12, 9), Vector3i(1, 14, 11), gauge)                           # its face
	WorkPalette.box(c, size, Vector3i(n - 1, 9, 8), Vector3i(n, 12, 12), WorkPalette.BRONZE)           # the steam outlet
	WorkPalette.box(c, size, Vector3i(n - 1, 5, 9), Vector3i(n, 9, 11), WorkPalette.BRONZE)            # ...down to the pipes
	if wear > 0.5:
		WorkPalette.box(c, size, Vector3i(3, 7, 2), Vector3i(8, 10, 3), WorkPalette.BRICK_DARK)          # a rust streak
	return {"solid": {"cells": c, "size": size, "grid": n}}


func to_dict() -> Dictionary:
	var d := super()
	d["water"] = water
	d["pressure"] = pressure
	d["wear"] = wear
	return d


func load_from(d: Dictionary) -> void:
	super(d)
	water = float(d.get("water", 0.0))
	pressure = float(d.get("pressure", 0.0))
	wear = float(d.get("wear", 0.0))


func summary() -> Dictionary:
	var s := super()
	s["water"] = snappedf(water, 0.01)
	s["pressure"] = snappedf(pressure, 0.01)
	s["wear"] = snappedf(wear, 0.01)
	return s


# ---------------- the steam network ----------------

## Every boiler hands itself to the machines at the ends of its bronze pipes: a walk from the boiler
## over pipe entities, one block step at a time on the flat, at most steam_pipe_max pipes long.
## Machines not reached this round lose their boiler. shortcut: pipes connect on one level only,
## add vertical steps when pipes climb walls.
static func feed(store: BlockEntities) -> void:
	for m in store.all_steam_machines():
		m.boiler = null
	for b in store.all_of("boiler"):
		var seen := {b.pos: 0}
		var queue := [b]
		while not queue.is_empty():
			var e: RefCounted = queue.pop_front()
			var w := BlockEntities.width_of(e.kind)
			var n := Settings.BLOCK
			for d in [Vector3i(-n, 0, 0), Vector3i(w, 0, 0), Vector3i(0, 0, -n), Vector3i(0, 0, w)]:
				var o: RefCounted = store.at(e.pos + d)
				if o == null or seen.has(o.pos):
					continue
				seen[o.pos] = seen[e.pos] + 1
				if o.kind == "bronze_pipe":
					if seen[o.pos] <= Settings.steam_pipe_max:
						queue.append(o)
				elif "boiler" in o:
					o.boiler = b
