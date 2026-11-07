class_name Comfort
extends RefCounted

## Home comfort (ADR 0162): the room around a bed is rated 0..100 by enclosure (walls and a roof, found by
## a handful of rays through the voxels), light, warmth (a fire near) and furniture count. Sleeping or
## napping in a good room gives the Rested buff (Survival.BUFFS): slower hunger, faster healing, a little speed.
## The maths is static and takes plain numbers, so the tests need no world.

const WALL_MAX := 40.0
const LIGHT_MAX := 20.0
const WARM_MAX := 20.0
const FURNI_MAX := 20.0
const FURNI_FULL := 4          # furniture pieces for the full furniture share
const RAYS := [Vector3i(1, 0, 0), Vector3i(-1, 0, 0), Vector3i(0, 0, 1), Vector3i(0, 0, -1),
	Vector3i(1, 0, 1), Vector3i(1, 0, -1), Vector3i(-1, 0, 1), Vector3i(-1, 0, -1)]


## 0..1: the share of eight horizontal rays that hit a wall within wall_m, 70%, plus the roof, 30%
## (a ray straight up that hits something within roof_m). `solid`: (Vector3i) -> bool.
static func enclosure(solid: Callable, from: Vector3i) -> float:
	var walls := 0
	var reach := int(Settings.m2v(Settings.comfort_wall_m))
	for d: Vector3i in RAYS:
		for i in range(1, reach + 1):
			if solid.call(from + d * i):
				walls += 1
				break
	var roof := 0.0
	for i in range(1, int(Settings.m2v(Settings.comfort_roof_m)) + 1):
		if solid.call(from + Vector3i(0, i, 0)):
			roof = 1.0
			break
	return 0.7 * float(walls) / RAYS.size() + 0.3 * roof


## Points 0..100. heat: Survival's entity heat at the bed (a fire at 2 m is 5), lit: a lamp, torch or fire in the room.
static func score(encl: float, lit: bool, heat: float, furnishings: int) -> int:
	var s := WALL_MAX * clampf(encl, 0.0, 1.0)
	s += LIGHT_MAX if lit else 0.0
	s += WARM_MAX * clampf(heat / Settings.comfort_heat, 0.0, 1.0)
	s += FURNI_MAX * clampf(float(furnishings) / FURNI_FULL, 0.0, 1.0)
	return int(roundf(s))


static func tier(points: int) -> String:
	if points >= 70:
		return "Luxurious"
	if points >= Settings.comfort_rest_min + 15:
		return "Comfortable"
	if points >= Settings.comfort_rest_min:
		return "Cosy"
	return "Plain" if points >= 20 else "Bare"


## The Rested buff a rating earns, {} under comfort_rest_min. `nap`: a daytime rest, half as long.
static func buff(points: int, nap: bool = false) -> Dictionary:
	if points < Settings.comfort_rest_min:
		return {}
	var a := clampf(points / 100.0, 0.0, 1.0)
	return {"kind": "rested", "amount": a, "s": Settings.comfort_rested_s * a * (0.5 if nap else 1.0)}


## Rates the room of `bed` (a FurnitureEntity) in `main`: {points, tier, enclosure, lit, heat, furniture}.
static func of_bed(main: Node, bed: FurnitureEntity) -> Dictionary:
	var seat := bed.seat()
	var from := Vector3i(seat.floor()) + Vector3i(0, 2, 0)
	var own := {}
	for c in bed.cells:
		own[c] = true
	var solid := func(p: Vector3i) -> bool:
		return not own.has(p) and MobManager.is_solid(main.get_world_voxel(p))
	var encl := enclosure(solid, from)
	var r := Settings.m2v(Settings.comfort_light_m)
	var lit := false
	var furni := 0
	for e: RefCounted in main.blocks._by_pos.values():
		if not (e.get("pos") is Vector3i) or e == bed:
			continue
		var d: float = (Vector3(e.pos) - seat).length()
		if e is FurnitureEntity:
			if d <= r:
				furni += 1
				lit = lit or main.items.get_def(e.item_id()).has("light")
		elif d <= r and e.has_method("is_lit") and e.is_lit():
			lit = true
	var heat: float = main.survival._entity_heat(seat) if main.survival != null else 0.0
	var pts := score(encl, lit, heat, furni)
	return {"points": pts, "tier": tier(pts), "enclosure": snappedf(encl, 0.01), "lit": lit, "heat": snappedf(heat, 0.1), "furniture": furni}


static func text(r: Dictionary) -> String:
	return "Room: %s (%d/100)%s" % [r["tier"], r["points"], ", sleeping here rests you" if r["points"] >= Settings.comfort_rest_min else ""]
